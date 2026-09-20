//! Test bloğunda sabit yayılımı (ADR-0060).
//!
//! `let n = 8; dut.addr = n` yazıldığında `n`nin değeri derleme zamanında
//! bellidir; ADR-0059 port genişliği denetimi (E8512) koşuyu beklemeden
//! çalışmalıdır. Test dili bunu kolaylaştırır: `if` yoktur, yeniden atama
//! yoktur, aynı adla ikinci `let` hatadır (E8506) — her bağlama tek
//! atamalıdır. Bu yüzden veri akışı analizi gerekmez; kapsam çerçeveleri
//! boyunca ad → değer ortamı yeterlidir.
//!
//! Sabit OLMAYANLAR ortamda "bilinmiyor" olarak durur (dış kapsamdaki ya
//! da üst düzeydeki aynı adlı sabiti gölgelerler): port okuması, döngü
//! sayacı, dizi elemanı, `len()`, sıfıra bölme ve bunlara bağlı her ifade.
//! Bunlar sessiz kalmaz — sürücü çalışma zamanı denetimi üretir.

use std::collections::HashMap;

use volt_ast::{Expr, ExprKind, Idx, ItemKind, SourceFile, TestExpr, TestExprKind, TestUnOp, UnOp};

use crate::sim_port::fold_binary;

/// Test gövdesinden görülen sabitler: üst düzey `const`lar + kapsam
/// çerçevelerindeki `let` bağlamaları.
#[derive(Debug, Clone, Default)]
pub struct TestConsts {
    /// Üst düzey `const`: `Some` = düz literal; `None` = var ama değeri
    /// burada çözülmüyor (hesaplanmış ifade ya da 64 bite sığmayan).
    globals: HashMap<String, Option<u64>>,
    /// Yerel adlar: `Some` = sabit, `None` = çalışma zamanı değeri.
    frames: Vec<HashMap<String, Option<u64>>>,
}

impl TestConsts {
    /// `sources`taki üst düzey `const`ları toplar (ilk tanım kazanır —
    /// `collect_modules` ile aynı kural).
    pub fn new(sources: &[&SourceFile]) -> Self {
        let mut globals = HashMap::new();
        for src in sources {
            for idx in &src.items {
                let ItemKind::Const(decl) = &src.items_arena[*idx].kind else {
                    continue;
                };
                let value = plain_literal(src, decl.value);
                globals.entry(decl.name.text.clone()).or_insert(value);
            }
        }
        Self {
            globals,
            frames: Vec::new(),
        }
    }

    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
    }

    pub fn pop(&mut self) {
        self.frames.pop();
    }

    /// Yerel adı bağlar; `None` çalışma zamanı değeridir (döngü sayacı,
    /// dizi, port okumasına bağlı `let`).
    pub fn bind(&mut self, name: &str, value: Option<u64>) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.to_string(), value);
        }
    }

    /// `let ad = <ifade>;` — ifade sabitse değeriyle, değilse bilinmiyor
    /// olarak bağlar. Değer `ad` bağlanmadan ÖNCE hesaplanır.
    pub fn bind_let(&mut self, name: &str, value: &TestExpr) {
        let constant = self.eval(value);
        self.bind(name, constant);
    }

    fn local(&self, name: &str) -> Option<Option<u64>> {
        self.frames.iter().rev().find_map(|f| f.get(name).copied())
    }

    /// Yerel olmayan `ad` üst düzey bir `const` mı? `Some(None)`: var ama
    /// düz literal değil.
    pub fn global(&self, name: &str) -> Option<Option<u64>> {
        if self.local(name).is_some() {
            return None;
        }
        self.globals.get(name).copied()
    }

    fn lookup(&self, name: &str) -> Option<u64> {
        match self.local(name) {
            Some(value) => value,
            None => self.globals.get(name).copied().flatten(),
        }
    }

    /// İfadenin derleme zamanı değeri. Anlam üretilen C++ ile aynıdır:
    /// 64 bitte sarar, 64 ve üstü kaydırma 0 verir; sıfıra bölme sabit
    /// değildir (çalışma zamanında testi düşürür).
    pub fn eval(&self, expr: &TestExpr) -> Option<u64> {
        match &expr.kind {
            TestExprKind::Int(n) => Some(*n),
            TestExprKind::Bool(b) => Some(u64::from(*b)),
            TestExprKind::Var(name) => self.lookup(&name.text),
            TestExprKind::Unary {
                op: TestUnOp::Not,
                operand,
            } => Some(u64::from(self.eval(operand)? == 0)),
            TestExprKind::Binary { op, lhs, rhs } => {
                fold_binary(*op, self.eval(lhs)?, self.eval(rhs)?)
            }
            _ => None,
        }
    }

    /// İfadede geçen sabit adları ve değerleri (ilk görülme sırası,
    /// tekil) — tanıda değerin nereden geldiğini göstermek için.
    pub fn bindings_in(&self, expr: &TestExpr) -> Vec<(String, u64)> {
        let mut found = Vec::new();
        self.collect_bindings(expr, &mut found);
        found
    }

    fn collect_bindings(&self, expr: &TestExpr, found: &mut Vec<(String, u64)>) {
        match &expr.kind {
            TestExprKind::Var(name) => {
                if let Some(value) = self.lookup(&name.text) {
                    if !found.iter().any(|(n, _)| *n == name.text) {
                        found.push((name.text.clone(), value));
                    }
                }
            }
            TestExprKind::Unary { operand, .. } => self.collect_bindings(operand, found),
            TestExprKind::Binary { lhs, rhs, .. } => {
                self.collect_bindings(lhs, found);
                self.collect_bindings(rhs, found);
            }
            _ => {}
        }
    }
}

/// Düz literal: `8`, `true` ya da eksi işaretli literal `-1`. Negatif
/// sayı test değerleriyle aynı biçimde 64 bitte sarar (`0 - 1`).
fn plain_literal(src: &SourceFile, expr: Idx<Expr>) -> Option<u64> {
    match &src.exprs[expr].kind {
        ExprKind::IntLit { value, .. } => u64::try_from(*value).ok(),
        ExprKind::BoolLit(b) => Some(u64::from(*b)),
        ExprKind::Unary {
            op: UnOp::Neg,
            operand,
        } => match &src.exprs[*operand].kind {
            ExprKind::IntLit { value, .. } => {
                u64::try_from(*value).ok().map(|v| 0u64.wrapping_sub(v))
            }
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ast::TestStmt;
    use volt_span::SourceMap;

    /// `let` deyimlerini sırayla bağlar, son `dut.d = <ifade>` değerini
    /// hesaplar.
    fn eval_last_set_port(header: &str, body: &str) -> Option<u64> {
        let src = format!(
            "{header}module M {{\n    in d : u8\n    out q : u8\n    q = d\n}}\n\ntest \"t\" {{\n    let dut = M {{ }};\n{body}\n}}\n"
        );
        let mut map = SourceMap::new();
        let fid = map.add_file("m_test.volt", src.clone());
        let parsed = volt_syntax::parser::parse(fid, &src);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let test = parsed
            .ast
            .items
            .iter()
            .find_map(|i| match &parsed.ast.items_arena[*i].kind {
                ItemKind::Test(t) => Some(t),
                _ => None,
            })
            .expect("test bloğu");
        let mut consts = TestConsts::new(&[&parsed.ast]);
        consts.push();
        let mut last = None;
        for stmt in &test.stmts {
            match stmt {
                TestStmt::LetVar { name, value, .. } => consts.bind_let(&name.text, value),
                TestStmt::SetPort { value, .. } => last = Some(consts.eval(value)),
                _ => {}
            }
        }
        last.expect("SetPort deyimi")
    }

    #[test]
    fn literal_let_is_constant() {
        assert_eq!(
            eval_last_set_port("", "    let n = 8;\n    dut.d = n;"),
            Some(8)
        );
    }

    #[test]
    fn let_chain_folds_through_earlier_constants() {
        let body = "    let n = 8;\n    let m = n + 1;\n    let k = m * 2;\n    dut.d = k - n;";
        assert_eq!(eval_last_set_port("", body), Some(10));
    }

    #[test]
    fn top_level_literal_const_is_visible() {
        let header = "const LIMIT : u8 = 9\n\n";
        assert_eq!(
            eval_last_set_port(header, "    let k = LIMIT;\n    dut.d = k + 1;"),
            Some(10)
        );
    }

    #[test]
    fn computed_top_level_const_is_known_but_not_constant() {
        let header = "const A : u8 = 4\nconst B : u8 = A + 1\n\n";
        let src_value = eval_last_set_port(header, "    dut.d = B;");
        assert_eq!(src_value, None);
    }

    #[test]
    fn port_read_poisons_every_dependent_binding() {
        let body = "    let x = dut.q;\n    let y = x + 1;\n    dut.d = y;";
        assert_eq!(eval_last_set_port("", body), None);
    }

    #[test]
    fn division_by_zero_is_left_to_the_run() {
        let body = "    let z = 0;\n    let bad = 4 / z;\n    dut.d = bad;";
        assert_eq!(eval_last_set_port("", body), None);
    }

    #[test]
    fn runtime_local_shadows_a_global_constant() {
        // Arrange
        let mut consts = TestConsts::default();
        consts.globals.insert("N".to_string(), Some(3));
        consts.push();

        // Act + Assert: döngü sayacı `N` üst düzey sabiti gizler.
        assert_eq!(consts.global("N"), Some(Some(3)));
        consts.push();
        consts.bind("N", None);
        assert_eq!(consts.global("N"), None);
        assert_eq!(consts.lookup("N"), None);
        consts.pop();
        assert_eq!(consts.lookup("N"), Some(3));
    }

    #[test]
    fn inner_frame_bindings_end_with_the_block() {
        let mut consts = TestConsts::default();
        consts.push();
        consts.bind("outer", Some(1));
        consts.push();
        consts.bind("inner", Some(2));
        assert_eq!(consts.lookup("outer"), Some(1));
        assert_eq!(consts.lookup("inner"), Some(2));
        consts.pop();
        assert_eq!(consts.lookup("inner"), None);
    }

    #[test]
    fn bind_without_a_frame_is_ignored() {
        let mut consts = TestConsts::default();
        consts.bind("n", Some(1));
        assert_eq!(consts.lookup("n"), None);
    }
}
