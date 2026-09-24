//! Test bloğu → testbench betiği indirgemesi ve test veri dosyaları
//! (ADR-0033, ADR-0058).
//!
//! `check_tests_with_files`ten geçmiş AST'de çağrılır; yine de
//! savunmacıdır: çözülemeyen deyim sessizce atlanmaz, test indirgenmez.

use std::path::{Path, PathBuf};

use volt_ast::{Name, SourceFile, TestDecl, TestExpr, TestExprKind, TestStmt, TestUnOp};
use volt_hir::{TestConsts, TestFileError, TestFileLoader};
use volt_span::SourceMap;
use volt_sv_emit::{TbAssertKind, TbPortCheck, TbStep, TbTest, TbValue};

use crate::unit::Manifest;

// ═══ Test veri dosyaları ══════════════════════════════════════════

/// `read_hex` yollarını test dosyasının dizinine göre çözer; proje
/// kökü (en yakın Volt.toml, yoksa test dosyasının dizini) dışına
/// çıkan yolları reddeder.
pub(crate) struct FsTestFiles {
    base_dir: PathBuf,
    root: PathBuf,
}

impl FsTestFiles {
    pub(crate) fn for_test_file(test_file: &Path) -> Self {
        let base_dir = test_file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let base_dir = base_dir.canonicalize().unwrap_or(base_dir);
        let root = Manifest::discover(&base_dir)
            .map(|m| m.root.canonicalize().unwrap_or(m.root))
            .filter(|root| base_dir.starts_with(root))
            .unwrap_or_else(|| base_dir.clone());
        Self { base_dir, root }
    }

    /// Test dosyasının dizini kökün kaç seviye altında?
    fn depth_below_root(&self) -> usize {
        self.base_dir
            .strip_prefix(&self.root)
            .map_or(0, |rel| rel.components().count())
    }
}

impl TestFileLoader for FsTestFiles {
    fn load(&self, rel_path: &str) -> Result<String, TestFileError> {
        let parts = volt_hir::normalize_data_path(rel_path, self.depth_below_root())
            .ok_or(TestFileError::OutsideProject)?;
        let path = parts
            .iter()
            .fold(self.base_dir.clone(), |acc, part| acc.join(part));
        // Sembolik bağ sözcüksel kuralı dolanmasın: gerçek yol da kökün
        // altında kalmalı.
        let real = path
            .canonicalize()
            .map_err(|err| TestFileError::NotFound(err.to_string()))?;
        if !real.starts_with(&self.root) {
            return Err(TestFileError::OutsideProject);
        }
        std::fs::read_to_string(&real).map_err(|err| TestFileError::NotFound(err.to_string()))
    }
}

// ═══ İndirgeme ════════════════════════════════════════════════════

/// İndirgenmiş test: DUT modülü, betik ve `load` ile açılması gereken
/// bellekler (sahip modül, yazmaç adı).
pub(crate) struct LoweredTest {
    pub module: String,
    pub tb: TbTest,
    pub load_targets: Vec<(String, String)>,
    /// Enum değerli `assert_eq`/`assert_ne` konumları: rapor sayının
    /// yanında varyant adını basar (ADR-0074).
    pub enum_asserts: Vec<(String, EnumLabels)>,
}

/// Bir enum'un varyant adları ve kodları — rapordaki `1 (State::Run)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnumLabels {
    pub enum_name: String,
    pub variants: Vec<(String, u64)>,
}

impl EnumLabels {
    /// `State::Run`; hiçbir varyantın kodu değilse `State: invalid code`.
    pub fn label(&self, code: u64) -> String {
        match self.variants.iter().find(|(_, c)| *c == code) {
            Some((v, _)) => format!("{}::{v}", self.enum_name),
            None => format!("{}: invalid code", self.enum_name),
        }
    }
}

/// İndirgeme bağlamı: `load` hedefi çözümü için birimin kaynakları,
/// `read_hex` için dosya erişimi.
pub(crate) struct LowerCtx<'a> {
    pub map: &'a SourceMap,
    pub file_label: &'a str,
    pub sources: &'a [&'a SourceFile],
    pub files: &'a dyn TestFileLoader,
}

/// Test ifadesini tb değerine indirger; sayı olmayan biçimler `None`.
/// Üst düzey `const` betikte değişken değildir: değeriyle yazılır
/// (ADR-0060). Yerel `let` adları betik değişkeni olarak kalır.
fn lower_value(consts: &TestConsts, expr: &TestExpr) -> Option<TbValue> {
    Some(match &expr.kind {
        TestExprKind::Int(n) => TbValue::Lit(*n),
        TestExprKind::Bool(b) => TbValue::Lit(u64::from(*b)),
        TestExprKind::PortRead { port, .. } => TbValue::Port(port.text.clone()),
        TestExprKind::Var(name) => match consts.global(&name.text) {
            Some(value) => TbValue::Lit(value?),
            None => TbValue::Var(name.text.clone()),
        },
        TestExprKind::Variant { enum_name, variant } => {
            TbValue::Lit(consts.variant(&enum_name.text, &variant.text)?)
        }
        TestExprKind::Index { base, index } => TbValue::Index {
            array: base.text.clone(),
            index: Box::new(lower_value(consts, index)?),
        },
        TestExprKind::Unary {
            op: TestUnOp::Not,
            operand,
        } => TbValue::Not(Box::new(lower_value(consts, operand)?)),
        TestExprKind::Binary { op, lhs, rhs } => TbValue::Binary {
            op: *op,
            lhs: Box::new(lower_value(consts, lhs)?),
            rhs: Box::new(lower_value(consts, rhs)?),
        },
        TestExprKind::Call { func, args } if func.text == "len" => match args.first()?.kind {
            TestExprKind::Var(ref name) => TbValue::Len(name.text.clone()),
            _ => return None,
        },
        TestExprKind::Call { .. }
        | TestExprKind::Str(_)
        | TestExprKind::Array(_)
        | TestExprKind::MemberPath { .. } => return None,
    })
}

/// `let ad = [..]` / `let ad = read_hex("..")` içeriği; sayı ise `None`.
fn array_data(value: &TestExpr, files: &dyn TestFileLoader) -> Option<Option<Vec<u64>>> {
    match &value.kind {
        TestExprKind::Array(items) => Some(
            items
                .iter()
                .map(|item| match item.kind {
                    TestExprKind::Int(n) => Some(n),
                    TestExprKind::Bool(b) => Some(u64::from(b)),
                    _ => None,
                })
                .collect(),
        ),
        TestExprKind::Call { func, args } if func.text == "read_hex" => {
            let data = match args.first().map(|a| &a.kind) {
                Some(TestExprKind::Str(path)) => files
                    .load(path)
                    .ok()
                    .and_then(|text| volt_hir::parse_readmemh(&text).ok())
                    .map(|image| image.words),
                _ => None,
            };
            Some(data)
        }
        _ => None,
    }
}

struct Lowering<'a> {
    ctx: &'a LowerCtx<'a>,
    module: Option<String>,
    load_targets: Vec<(String, String)>,
    enum_asserts: Vec<(String, EnumLabels)>,
    /// Derleme zamanında bilinen değerler (ADR-0060) — denetimdeki
    /// (`check_tests_with_files`) ortamın aynısı; E8512 kararıyla
    /// betiğe yazılan desen aynı değerden çıkar.
    consts: TestConsts,
}

impl Lowering<'_> {
    fn block(&mut self, stmts: &[TestStmt]) -> Option<Vec<TbStep>> {
        self.consts.push();
        let steps = self.block_steps(stmts);
        self.consts.pop();
        steps
    }

    fn block_steps(&mut self, stmts: &[TestStmt]) -> Option<Vec<TbStep>> {
        let mut steps = Vec::new();
        for stmt in stmts {
            self.stmt(stmt, &mut steps)?;
        }
        Some(steps)
    }

    fn value(&self, expr: &TestExpr) -> Option<TbValue> {
        lower_value(&self.consts, expr)
    }

    fn loc(&self, span: volt_span::Span) -> String {
        format!("{}:{}", self.ctx.file_label, self.ctx.map.line_col(span).0)
    }

    fn stmt(&mut self, stmt: &TestStmt, steps: &mut Vec<TbStep>) -> Option<()> {
        match stmt {
            TestStmt::LetDut { module: m, .. } => {
                if self.module.is_none() {
                    self.module = Some(m.text.clone());
                }
            }
            TestStmt::SetPort {
                span, port, value, ..
            } => self.set_port(*span, port, value, steps)?,
            TestStmt::LetVar { span, name, value } => {
                steps.push(TbStep::Loc(self.loc(*span)));
                let step = match array_data(value, self.ctx.files) {
                    Some(data) => TbStep::LetArray {
                        name: name.text.clone(),
                        data: data?,
                    },
                    None => TbStep::LetScalar {
                        name: name.text.clone(),
                        value: self.value(value)?,
                    },
                };
                steps.push(step);
                // Dizi `eval`de sabit değildir: adı bilinmiyor olarak bağlanır.
                self.consts.bind_let(&name.text, value);
            }
            TestStmt::For {
                span,
                var,
                start,
                end,
                body,
            } => {
                steps.push(TbStep::Loc(self.loc(*span)));
                let (start, end) = (self.value(start)?, self.value(end)?);
                // Sayaç koşuda değişir: aynı adlı sabiti gölgeler.
                self.consts.push();
                self.consts.bind(&var.text, None);
                let body = self.block(body);
                self.consts.pop();
                steps.push(TbStep::For {
                    var: var.text.clone(),
                    start,
                    end,
                    body: body?,
                });
            }
            TestStmt::Call { span, func, args } => self.call(*span, &func.text, args, steps)?,
        }
        Some(())
    }

    /// `dut.port = v` (ADR-0059). Sabit değer derleme zamanında
    /// denetlendi (E8512) ve bit deseni olarak yazılır; sığdığı
    /// kanıtlanamayan her değer çalışma zamanı denetimiyle yazılır.
    fn set_port(
        &self,
        span: volt_span::Span,
        port: &Name,
        value: &TestExpr,
        steps: &mut Vec<TbStep>,
    ) -> Option<()> {
        let width = self.port_width(&port.text);
        let constant = self.consts.eval(value);
        if let (Some(width), Some(constant)) = (width, constant) {
            if !width.accepts(constant) {
                return None; // E8512'den geçmiş olamaz
            }
            steps.push(TbStep::SetPort {
                port: port.text.clone(),
                value: TbValue::Lit(width.to_pattern(constant)),
            });
            return Some(());
        }
        let value = self.value(value)?;
        let port_name = port.text.clone();
        if self.fits_without_check(width, constant, &value) {
            self.push_loc(span, &value, steps);
            steps.push(TbStep::SetPort {
                port: port_name,
                value,
            });
            return Some(());
        }
        steps.push(TbStep::Loc(self.loc(span)));
        steps.push(TbStep::SetPortChecked {
            port: port_name,
            value,
            check: TbPortCheck {
                bits: width.map(|w| w.bits),
                signed: width.is_some_and(volt_hir::PortWidth::is_signed),
                type_name: width.map_or_else(|| "?".to_string(), volt_hir::PortWidth::type_name),
            },
        });
        Some(())
    }

    fn port_width(&self, port: &str) -> Option<volt_hir::PortWidth> {
        volt_hir::test_port_width(self.ctx.sources, self.module.as_deref()?, port)
    }

    /// Denetimsiz yazılabilir mi? 0/1 her porta sığar; bir `out` portunun
    /// okunan deseni, en az o kadar geniş porta sığar.
    fn fits_without_check(
        &self,
        target: Option<volt_hir::PortWidth>,
        constant: Option<u64>,
        value: &TbValue,
    ) -> bool {
        if constant.is_some_and(|c| c <= 1) {
            return true;
        }
        let (Some(target), TbValue::Port(source)) = (target, value) else {
            return false;
        };
        self.port_width(source)
            .is_some_and(|source| source.max_pattern() <= target.max_pattern())
    }

    /// Konum adımı yalnız gerekince eklenir: düz literal/port deyimleri
    /// ADR-0033 betiğiyle aynı kalır.
    fn push_loc(&self, span: volt_span::Span, value: &TbValue, steps: &mut Vec<TbStep>) {
        if !matches!(value, TbValue::Lit(_) | TbValue::Port(_)) {
            steps.push(TbStep::Loc(self.loc(span)));
        }
    }

    fn call(
        &mut self,
        span: volt_span::Span,
        func: &str,
        args: &[TestExpr],
        steps: &mut Vec<TbStep>,
    ) -> Option<()> {
        let kind = match func {
            "step" => {
                match self.value(args.first()?)? {
                    TbValue::Lit(n) => steps.push(TbStep::Step(n)),
                    count => {
                        steps.push(TbStep::Loc(self.loc(span)));
                        steps.push(TbStep::StepBy(count));
                    }
                }
                return Some(());
            }
            "reset" => {
                steps.push(TbStep::Reset);
                return Some(());
            }
            "load" => return self.load(span, args, steps),
            "assert_eq" => TbAssertKind::Eq,
            "assert_ne" => TbAssertKind::Ne,
            "assert_true" => TbAssertKind::True,
            "assert_false" => TbAssertKind::False,
            _ => return None,
        };
        let left = self.value(args.first()?)?;
        let right = match args.get(1) {
            Some(arg) => self.value(arg)?,
            None => TbValue::Lit(0),
        };
        if matches!(kind, TbAssertKind::Eq | TbAssertKind::Ne) {
            if let Some(labels) = self.enum_of_args(args) {
                self.enum_asserts.push((self.loc(span), labels));
            }
        }
        steps.push(TbStep::Assert {
            kind,
            left,
            right,
            loc: self.loc(span),
        });
        Some(())
    }

    /// Karşılaştırmanın enum'u: bir taraf `Enum::Varyant` ya da enum
    /// tipli DUT portu (ADR-0074).
    fn enum_of_args(&self, args: &[TestExpr]) -> Option<EnumLabels> {
        let module = self.module.as_deref();
        let name = args.iter().find_map(|a| match &a.kind {
            TestExprKind::Variant { enum_name, .. } => Some(enum_name.text.clone()),
            TestExprKind::PortRead { port, .. } => self.ctx.sources.iter().find_map(|src| {
                src.items
                    .iter()
                    .find_map(|&i| match &src.items_arena[i].kind {
                        volt_ast::ItemKind::Module(m) if Some(m.name.text.as_str()) == module => {
                            volt_hir::sim_port_enum(src, m, &port.text)
                        }
                        _ => None,
                    })
            }),
            _ => None,
        })?;
        let variants = self.consts.enum_variants(&name)?.to_vec();
        Some(EnumLabels {
            enum_name: name,
            variants,
        })
    }

    fn load(
        &mut self,
        span: volt_span::Span,
        args: &[TestExpr],
        steps: &mut Vec<TbStep>,
    ) -> Option<()> {
        let module = self.module.clone()?;
        let target = volt_hir::resolve_load_target(self.ctx.sources, &module, args.first()?)?;
        let TestExprKind::Var(source) = &args.get(1)?.kind else {
            return None;
        };
        let reg = target.path.last()?.clone();
        let entry = (target.owner_module.clone(), reg);
        if !self.load_targets.contains(&entry) {
            self.load_targets.push(entry);
        }
        steps.push(TbStep::Loc(self.loc(span)));
        steps.push(TbStep::Load {
            target: format!("{module}__DOT__{}", target.path.join("__DOT__")),
            source: source.text.clone(),
            elem_bits: target.elem_bits,
        });
        Some(())
    }
}

/// Test bloğunu indirger; `None` = betik kurulamadı (denetimden geçmiş
/// AST'de beklenmez — çağıran içsel hata olarak raporlar).
pub(crate) fn lower_test(ctx: &LowerCtx<'_>, test: &TestDecl) -> Option<LoweredTest> {
    let mut lowering = Lowering {
        ctx,
        module: None,
        load_targets: Vec::new(),
        enum_asserts: Vec::new(),
        consts: TestConsts::new(ctx.sources),
    };
    let steps = lowering.block(&test.stmts)?;
    // Testbench iddiayı yalnız `dosya:satır` ile bildirir; aynı satırda
    // birden çok iddia varsa hangisinin düştüğü bilinemez — enum adı
    // yanlış iddiaya yapışmasın (ADR-0074 rapor adları).
    let per_loc = |loc: &str| {
        steps
            .iter()
            .filter(|s| matches!(s, TbStep::Assert { loc: l, .. } if l == loc))
            .count()
    };
    let enum_asserts: Vec<(String, EnumLabels)> = lowering
        .enum_asserts
        .iter()
        .filter(|(loc, _)| per_loc(loc) <= 1)
        .cloned()
        .collect();
    Some(LoweredTest {
        module: lowering.module?,
        tb: TbTest {
            name: test.display_name(),
            steps,
        },
        load_targets: lowering.load_targets,
        enum_asserts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ast::{ItemKind, TestBinOp};

    /// Dosya sistemi yerine sabit içerik veren yükleyici.
    struct FakeFiles(&'static str);

    impl TestFileLoader for FakeFiles {
        fn load(&self, _rel_path: &str) -> Result<String, TestFileError> {
            Ok(self.0.to_string())
        }
    }

    const COUNTER: &str = "module Counter {\n    in  clk    : clock\n    in  enable : bool\n    out count  : u8\n    reg r : u8 = 0\n    reg mem : [u8; 4] = [0; 4]\n    on clk { if enable { r <= r + 1 } }\n    count = r\n}\n\n";

    fn lower_src(body: &str, files: &dyn TestFileLoader) -> Option<LoweredTest> {
        lower_with(COUNTER, body, files)
    }

    fn lower_with(design: &str, body: &str, files: &dyn TestFileLoader) -> Option<LoweredTest> {
        let src = format!("{design}test \"t\" {{\n{body}\n}}\n");
        let mut map = SourceMap::new();
        let fid = map.add_file("counter_test.volt", src.clone());
        let parsed = volt_syntax::parser::parse(fid, &src);
        assert!(
            parsed.diagnostics.is_empty(),
            "ayrışmalı: {:?}",
            parsed.diagnostics
        );
        let test = parsed
            .ast
            .items
            .iter()
            .find_map(|i| match &parsed.ast.items_arena[*i].kind {
                ItemKind::Test(t) => Some(t),
                _ => None,
            })
            .expect("test bloğu");
        let ctx = LowerCtx {
            map: &map,
            file_label: "counter_test.volt",
            sources: &[&parsed.ast],
            files,
        };
        lower_test(&ctx, test)
    }

    const ENUM_DUT: &str = "enum S { A, B, C }\nmodule E {\n    in  clk : clock\n    in  d   : S\n    out q   : S\n    out n   : u8\n    reg r : S = S::A\n    on clk { r <= d }\n    q = r\n    n = 3\n}\n\n";

    #[test]
    fn enum_asserts_carry_variant_labels_by_location() {
        let lowered = lower_with(
            ENUM_DUT,
            "    let dut = E { };\n    dut.d = S::C;\n    step(1);\n    assert_eq(dut.q, S::C);\n    assert_eq(dut.n, 3);",
            &FakeFiles(""),
        )
        .expect("indirgenmeli");
        assert_eq!(lowered.enum_asserts.len(), 1);
        let (loc, labels) = &lowered.enum_asserts[0];
        assert_eq!(labels.label(2), "S::C");
        assert_eq!(labels.label(3), "S: invalid code");
        assert!(lowered.tb.steps.contains(&TbStep::Assert {
            kind: TbAssertKind::Eq,
            left: TbValue::Port("q".into()),
            right: TbValue::Lit(2),
            loc: loc.clone(),
        }));
    }

    #[test]
    fn enum_labels_are_dropped_when_a_line_holds_several_asserts() {
        // Testbench yalnız dosya:satır bildirir; sayısal iddia enum adı almasın.
        let lowered = lower_with(
            ENUM_DUT,
            "    let dut = E { };\n    step(1);\n    assert_eq(dut.q, S::A); assert_eq(dut.n, 3);",
            &FakeFiles(""),
        )
        .expect("indirgenmeli");
        assert!(lowered.enum_asserts.is_empty());
    }

    #[test]
    fn lower_test_maps_statements_to_tb_steps() {
        let lowered = lower_src(
            "    let dut = Counter { };\n    dut.enable = true;\n    step(2);\n    assert_eq(dut.count, 2);\n    reset();\n    assert_false(dut.count);",
            &FakeFiles(""),
        )
        .expect("indirgeme");
        assert_eq!(lowered.module, "Counter");
        assert_eq!(lowered.tb.name, "t");
        let steps = &lowered.tb.steps;
        assert_eq!(steps.len(), 5);
        assert_eq!(
            steps[0],
            TbStep::SetPort {
                port: "enable".into(),
                value: TbValue::Lit(1)
            }
        );
        assert_eq!(steps[1], TbStep::Step(2));
        let TbStep::Assert {
            kind,
            left,
            right,
            loc,
        } = &steps[2]
        else {
            panic!("assert bekleniyor");
        };
        assert_eq!(*kind, TbAssertKind::Eq);
        assert_eq!(*left, TbValue::Port("count".into()));
        assert_eq!(*right, TbValue::Lit(2));
        assert_eq!(loc, "counter_test.volt:15");
        assert_eq!(steps[3], TbStep::Reset);
        let TbStep::Assert { kind, .. } = &steps[4] else {
            panic!("assert bekleniyor");
        };
        assert_eq!(*kind, TbAssertKind::False);
        assert!(lowered.load_targets.is_empty());
    }

    const PORTS: &str = "module Ports {\n    in  clk  : clock\n    in  addr : u3\n    in  sv   : i8\n    in  wide : u16\n    out q3   : u3\n    out q16  : u16\n    reg r : u16 = 0\n    on clk { r <= r + 1 }\n    q3 = addr\n    q16 = r\n}\n\n";

    fn lower_ports(body: &str) -> Vec<TbStep> {
        let src = format!("{PORTS}test \"t\" {{\n    let dut = Ports {{ }};\n{body}\n}}\n");
        let mut map = SourceMap::new();
        let fid = map.add_file("ports_test.volt", src.clone());
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
        let ctx = LowerCtx {
            map: &map,
            file_label: "ports_test.volt",
            sources: &[&parsed.ast],
            files: &FakeFiles(""),
        };
        lower_test(&ctx, test).expect("indirgeme").tb.steps
    }

    fn port_check(steps: &[TbStep]) -> Option<&TbPortCheck> {
        steps.iter().find_map(|s| match s {
            TbStep::SetPortChecked { check, .. } => Some(check),
            TbStep::For { body, .. } => port_check(body),
            _ => None,
        })
    }

    #[test]
    fn constant_port_write_needs_no_runtime_check() {
        let steps = lower_ports("    dut.addr = 3 + 4;");
        assert_eq!(
            steps,
            vec![TbStep::SetPort {
                port: "addr".into(),
                value: TbValue::Lit(7),
            }]
        );
    }

    #[test]
    fn negative_constant_is_written_as_its_bit_pattern() {
        let steps = lower_ports("    dut.sv = 0 - 1;");
        assert_eq!(
            steps,
            vec![TbStep::SetPort {
                port: "sv".into(),
                value: TbValue::Lit(0xFF),
            }]
        );
    }

    #[test]
    fn loop_counter_write_carries_the_port_width() {
        let steps = lower_ports("    for i in 0..16 {\n        dut.addr = i;\n    }");
        let check = port_check(&steps).expect("çalışma zamanı denetimi");
        assert_eq!(
            *check,
            TbPortCheck {
                bits: Some(3),
                signed: false,
                type_name: "u3".into(),
            }
        );
        let TbStep::For { body, .. } = &steps[1] else {
            panic!("for bekleniyor: {steps:?}");
        };
        assert_eq!(body[0], TbStep::Loc("ports_test.volt:17".into()));
    }

    #[test]
    fn signed_port_check_is_marked_signed() {
        let steps = lower_ports("    for i in 0..2 {\n        dut.sv = 0 - i;\n    }");
        let check = port_check(&steps).expect("çalışma zamanı denetimi");
        assert_eq!((check.bits, check.signed), (Some(8), true));
        assert_eq!(check.type_name, "i8");
    }

    // ═══ Sabit yayılımı (ADR-0060) ═══

    /// İlk `dut.<port> = ...` adımı (döngü gövdeleri dahil).
    fn first_write(steps: &[TbStep]) -> Option<&TbStep> {
        steps.iter().find_map(|s| match s {
            TbStep::SetPort { .. } | TbStep::SetPortChecked { .. } => Some(s),
            TbStep::For { body, .. } => first_write(body),
            _ => None,
        })
    }

    fn lit_write(port: &str, value: u64) -> TbStep {
        TbStep::SetPort {
            port: port.into(),
            value: TbValue::Lit(value),
        }
    }

    fn try_lower_ports(body: &str) -> Option<LoweredTest> {
        let src = format!("{PORTS}test \"t\" {{\n    let dut = Ports {{ }};\n{body}\n}}\n");
        let mut map = SourceMap::new();
        let fid = map.add_file("ports_test.volt", src.clone());
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
        let ctx = LowerCtx {
            map: &map,
            file_label: "ports_test.volt",
            sources: &[&parsed.ast],
            files: &FakeFiles(""),
        };
        lower_test(&ctx, test)
    }

    #[test]
    fn constant_let_is_written_as_a_literal_without_a_check() {
        let steps = lower_ports("    let n = 7;\n    dut.addr = n;");
        assert_eq!(first_write(&steps), Some(&lit_write("addr", 7)));
        assert!(port_check(&steps).is_none(), "{steps:?}");
        // Betik değişkeni yine tanımlanır: assert'ler onu okuyabilir.
        assert!(steps.contains(&TbStep::LetScalar {
            name: "n".into(),
            value: TbValue::Lit(7),
        }));
    }

    #[test]
    fn let_chain_folds_to_the_final_pattern() {
        let steps = lower_ports("    let n = 3;\n    let m = n + 1;\n    dut.sv = 0 - m;");
        assert_eq!(first_write(&steps), Some(&lit_write("sv", 0xFC)));
    }

    #[test]
    fn overflowing_constant_let_does_not_lower() {
        // E8512'den kaçan yayılmış sabit sessizce yazılmaz.
        assert!(try_lower_ports("    let n = 8;\n    dut.addr = n;").is_none());
        assert!(try_lower_ports("    let n = 7;\n    dut.addr = n;").is_some());
    }

    #[test]
    fn port_read_let_keeps_the_runtime_check() {
        let steps = lower_ports("    let x = dut.q16;\n    dut.addr = x;");
        assert_eq!(port_check(&steps).expect("denetim").bits, Some(3));
    }

    #[test]
    fn let_depending_on_the_loop_counter_keeps_the_runtime_check() {
        let steps = lower_ports(
            "    for i in 0..4 {\n        let n = i + 6;\n        dut.addr = n;\n    }",
        );
        assert_eq!(port_check(&steps).expect("denetim").bits, Some(3));
    }

    #[test]
    fn constant_let_inside_a_loop_is_folded() {
        let steps =
            lower_ports("    for i in 0..4 {\n        let n = 5;\n        dut.addr = n;\n    }");
        assert_eq!(first_write(&steps), Some(&lit_write("addr", 5)));
    }

    #[test]
    fn block_local_constant_does_not_leak_out_of_the_loop() {
        // Döngü içindeki `n` bloğuyla biter; dışarıdaki `n` ayrı bağlamadır.
        let steps = lower_ports(
            "    for i in 0..2 {\n        let n = 5;\n        dut.addr = n;\n    }\n    let n = dut.q16;\n    dut.addr = n;",
        );
        let outer = steps
            .iter()
            .rev()
            .find(|s| matches!(s, TbStep::SetPort { .. } | TbStep::SetPortChecked { .. }))
            .expect("dış yazım");
        assert!(
            matches!(outer, TbStep::SetPortChecked { value: TbValue::Var(v), .. } if v == "n"),
            "{outer:?}"
        );
    }

    #[test]
    fn narrower_output_port_fits_without_a_check() {
        let steps = lower_ports("    dut.wide = dut.q3;");
        assert!(port_check(&steps).is_none(), "{steps:?}");
    }

    #[test]
    fn wider_output_port_is_checked() {
        let steps = lower_ports("    dut.addr = dut.q16;");
        assert_eq!(port_check(&steps).expect("denetim").bits, Some(3));
    }

    #[test]
    fn overflowing_constant_does_not_lower() {
        // E8512'den kaçan bir sabit sessizce yazılmaz: test indirgenmez.
        let src =
            format!("{PORTS}test \"t\" {{\n    let dut = Ports {{ }};\n    dut.addr = 8;\n}}\n");
        let mut map = SourceMap::new();
        let fid = map.add_file("ports_test.volt", src.clone());
        let parsed = volt_syntax::parser::parse(fid, &src);
        let test = parsed
            .ast
            .items
            .iter()
            .find_map(|i| match &parsed.ast.items_arena[*i].kind {
                ItemKind::Test(t) => Some(t),
                _ => None,
            })
            .expect("test bloğu");
        let ctx = LowerCtx {
            map: &map,
            file_label: "ports_test.volt",
            sources: &[&parsed.ast],
            files: &FakeFiles(""),
        };
        assert!(lower_test(&ctx, test).is_none());
    }

    #[test]
    fn array_literal_and_index_lower_to_embedded_data() {
        let lowered = lower_src(
            "    let dut = Counter { };\n    let expected = [0x63, 0x7c, true];\n    assert_eq(dut.count, expected[1]);",
            &FakeFiles(""),
        )
        .expect("indirgeme");
        assert!(lowered.tb.steps.contains(&TbStep::LetArray {
            name: "expected".into(),
            data: vec![0x63, 0x7c, 1],
        }));
        let Some(TbStep::Assert { right, .. }) = lowered.tb.steps.last() else {
            panic!("assert bekleniyor");
        };
        assert_eq!(
            *right,
            TbValue::Index {
                array: "expected".into(),
                index: Box::new(TbValue::Lit(1)),
            }
        );
    }

    #[test]
    fn for_loop_lowers_to_a_runtime_loop_not_an_unrolling() {
        let lowered = lower_src(
            "    let dut = Counter { };\n    for i in 0..1000 {\n        step(1);\n        assert_eq(dut.count, (i + 1) & 0xFF);\n    }",
            &FakeFiles(""),
        )
        .expect("indirgeme");
        let fors: Vec<_> = lowered
            .tb
            .steps
            .iter()
            .filter(|s| matches!(s, TbStep::For { .. }))
            .collect();
        assert_eq!(fors.len(), 1, "tek döngü adımı, 1000 kopya değil");
        let TbStep::For {
            var,
            start,
            end,
            body,
        } = fors[0]
        else {
            unreachable!();
        };
        assert_eq!(var, "i");
        assert_eq!((start, end), (&TbValue::Lit(0), &TbValue::Lit(1000)));
        assert!(body.contains(&TbStep::Step(1)));
        let Some(TbStep::Assert { right, .. }) = body.last() else {
            panic!("assert bekleniyor");
        };
        let TbValue::Binary { op, .. } = right else {
            panic!("ikili ifade bekleniyor");
        };
        assert_eq!(*op, TestBinOp::And);
    }

    #[test]
    fn read_hex_embeds_file_words() {
        let lowered = lower_src(
            "    let dut = Counter { };\n    let rom = read_hex(\"rom.hex\");\n    step(len(rom));",
            &FakeFiles("@0 0A 0B\n0C\n"),
        )
        .expect("indirgeme");
        assert!(lowered.tb.steps.contains(&TbStep::LetArray {
            name: "rom".into(),
            data: vec![0x0A, 0x0B, 0x0C],
        }));
        assert!(lowered
            .tb
            .steps
            .contains(&TbStep::StepBy(TbValue::Len("rom".into()))));
    }

    #[test]
    fn load_resolves_verilator_name_and_public_target() {
        let lowered = lower_src(
            "    let dut = Counter { };\n    let data = [1, 2];\n    load(dut.mem, data);",
            &FakeFiles(""),
        )
        .expect("indirgeme");
        assert_eq!(
            lowered.load_targets,
            vec![("Counter".to_string(), "mem".to_string())]
        );
        assert!(lowered.tb.steps.contains(&TbStep::Load {
            target: "Counter__DOT__mem".into(),
            source: "data".into(),
            elem_bits: Some(8),
        }));
    }

    #[test]
    fn load_of_unknown_memory_does_not_lower() {
        assert!(lower_src(
            "    let dut = Counter { };\n    let data = [1];\n    load(dut.nope, data);",
            &FakeFiles(""),
        )
        .is_none());
    }

    #[test]
    fn fs_loader_rejects_paths_leaving_the_project() {
        let dir = std::env::temp_dir().join(format!("volt_tf_{}", std::process::id()));
        let inner = dir.join("proj");
        std::fs::create_dir_all(&inner).expect("dizin");
        std::fs::write(dir.join("secret.hex"), "FF\n").expect("yaz");
        std::fs::write(inner.join("ok.hex"), "01 02\n").expect("yaz");
        let files = FsTestFiles::for_test_file(&inner.join("x_test.volt"));
        assert_eq!(files.load("ok.hex"), Ok("01 02\n".to_string()));
        assert_eq!(
            files.load("../secret.hex"),
            Err(TestFileError::OutsideProject)
        );
        assert_eq!(
            files.load(&dir.join("secret.hex").display().to_string()),
            Err(TestFileError::OutsideProject)
        );
        assert!(matches!(
            files.load("missing.hex"),
            Err(TestFileError::NotFound(_))
        ));
        std::fs::remove_dir_all(&dir).expect("temizlik");
    }

    #[test]
    fn fs_loader_allows_parent_dirs_below_the_manifest_root() {
        let dir = std::env::temp_dir().join(format!("volt_tf_root_{}", std::process::id()));
        let tests_dir = dir.join("tests");
        std::fs::create_dir_all(&tests_dir).expect("dizin");
        std::fs::create_dir_all(dir.join("data")).expect("dizin");
        std::fs::write(dir.join("Volt.toml"), "[package]\nname = \"p\"\n").expect("yaz");
        std::fs::write(dir.join("data").join("rom.hex"), "AA\n").expect("yaz");
        let files = FsTestFiles::for_test_file(&tests_dir.join("x_test.volt"));
        assert_eq!(files.load("../data/rom.hex"), Ok("AA\n".to_string()));
        assert_eq!(
            files.load("../../outside.hex"),
            Err(TestFileError::OutsideProject)
        );
        std::fs::remove_dir_all(&dir).expect("temizlik");
    }
}
