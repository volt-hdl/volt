//! R biçimi: ad hijyeninin metamorfik ilişkisi (#128).
//!
//! İfade bir `fn`'in gövdesine taşınır ve çağrı, gövdenin adlarıyla
//! çakışan adlar arasında yapılır; anlam değişmemeli (name-resolution.md
//! §2, §6; ADR-0081 Karar 12.1: fn kapsamının ebeveyni kök kapsamdır):
//!
//! - seçim konumundaki literaller (`x[3]`, `x[2 +: 4]`) kök `const`'lara
//!   çıkar (`t<id>r`, `K<id>r<n>`);
//! - C biçiminin alt ifadesi gövdede bir `let`'e bağlanır; adı (`s<id>r`)
//!   aynı adlı bir kök const'u gölgeler;
//! - çağrı, ilk const'la aynı adlı ve başka bir değer alan döngü
//!   değişkeninin (`for t<id>r in V..V+1`) içindedir;
//! - bir argüman, `s<id>r` adlı bir blok `let`'inden geçer (çağıranın
//!   yereli, gövdenin `let`'i ve kök const aynı adı taşır).
//!
//! Gövdenin bir adı çağıranın adına bağlanırsa değer değişir: #128'de
//! `x[t<id>r]` döngü değişkeninin değerini seçiyordu.

use crate::ast::{Expr, PortTy};
use crate::print::{port_name, print, Style};

/// Bir ifadenin R biçimi.
#[derive(Clone, Debug)]
pub struct Hygiene {
    /// Kök const'lar: (ad, değer); hepsi `u32`.
    pub consts: Vec<(String, i128)>,
    /// fn gövdesinin `let`'i: `Name(let_name, değer)` düğümünün değeri.
    pub body_let: Option<Expr>,
    /// fn gövdesinin son ifadesi.
    pub body: Expr,
    /// fn parametreleri: ifadenin portları (adları port adları).
    pub params: Vec<usize>,
    /// Döngü değişkeni ve tek yinelemesinin değeri.
    pub loop_var: String,
    pub loop_value: i128,
    /// Blok `let`'inden geçen argümanın portu.
    pub caller_let: Option<usize>,
}

pub fn fn_name(id: usize) -> String {
    format!("h{id}r")
}

/// İlk çıkarılan literalin const'u ve döngü değişkeni aynı adı taşır.
pub fn lit_name(id: usize, n: usize) -> String {
    if n == 0 {
        format!("t{id}r")
    } else {
        format!("K{id}r{n}")
    }
}

/// fn gövdesinin `let`'i, çağıranın blok `let`'i ve bir kök const.
pub fn let_name(id: usize) -> String {
    format!("s{id}r")
}

impl Hygiene {
    /// `extracted`: C biçiminin ağacı (alt ifadesi `Name(sub, …)`).
    /// Çakışacak ad yoksa (seçim literali, gövde `let`'i, blok `let`'i
    /// geçen argüman) `None`.
    pub fn new(id: usize, expr: &Expr, extracted: Option<&Expr>, sub: &str) -> Option<Hygiene> {
        let mut base = expr.clone();
        let mut with_let = false;
        if let Some(ex) = extracted {
            // İkame kipinde bitleri seçilen tipsiz `let`'in yerine seçilebilir
            // bir ifade gerekir (ADR-0081, E0003): o durumda gövde `let`'siz.
            if !name_is_select_base(ex, sub) {
                base = renamed(ex, sub, &let_name(id));
                with_let = true;
            }
        }
        let mut consts = Vec::new();
        lift(&mut base, id, &mut consts);
        let body_let = if with_let {
            find_name(&base, &let_name(id)).cloned()
        } else {
            None
        };
        let mut params = Vec::new();
        expr.ports(&mut params);
        params.sort_unstable();
        params.dedup();
        let selected = select_bases(expr);
        let caller_let = params.iter().copied().find(|k| !selected.contains(k));
        if consts.is_empty() && body_let.is_none() && caller_let.is_none() {
            return None;
        }
        let (loop_var, loop_value) = match consts.first() {
            Some((n, v)) => (n.clone(), v + 1),
            None => (format!("K{id}r"), 1),
        };
        // Gövdenin `let`'i ve çağıranın yereli bu kök const'u gölgeler.
        consts.push((let_name(id), 0));
        Some(Hygiene {
            consts,
            body_let,
            body: base,
            params,
            loop_var,
            loop_value,
            caller_let,
        })
    }

    /// Modül dışındaki öğeler: const'lar ve fn.
    pub fn item_lines(&self, id: usize, ports: &[PortTy], ret: PortTy) -> Vec<String> {
        let mut out: Vec<String> = self
            .consts
            .iter()
            .map(|(n, v)| format!("const {n} : u32 = {v}"))
            .collect();
        let params: Vec<String> = self
            .params
            .iter()
            .map(|k| format!("{}: {}", port_name(*k), ports[*k].volt()))
            .collect();
        out.push(format!(
            "fn {}({}) -> {} {{",
            fn_name(id),
            params.join(", "),
            ret.volt()
        ));
        // `;` gerekli: son ifade `(` ile başlarsa `let`'in değeri çağrı
        // gibi sürerdi (`LetDecl = … [";"]`, grammar-full.ebnf).
        if let Some(v) = &self.body_let {
            out.push(format!(
                "    let {} = {};",
                let_name(id),
                print(v, Style::Min)
            ));
        }
        out.push(format!("    {}", print(&self.body, Style::Min)));
        out.push("}".into());
        out
    }

    /// Modül gövdesindeki `comb` bloğu; `out` R biçiminin çıkışı.
    pub fn module_lines(&self, id: usize, out: &str) -> Vec<String> {
        let mut lines = vec!["    comb {".to_string()];
        if let Some(k) = self.caller_let {
            lines.push(format!("        let {} = {}", let_name(id), port_name(k)));
        }
        let args: Vec<String> = self
            .params
            .iter()
            .map(|k| {
                if Some(*k) == self.caller_let {
                    let_name(id)
                } else {
                    port_name(*k)
                }
            })
            .collect();
        lines.push(format!(
            "        for {} in {}..{} {{",
            self.loop_var,
            self.loop_value,
            self.loop_value + 1
        ));
        lines.push(format!(
            "            {out} = {}({})",
            fn_name(id),
            args.join(", ")
        ));
        lines.push("        }".into());
        lines.push("    }".into());
        lines
    }
}

/// Seçim konumundaki literaller (`x[3]`'ün indeksi, `x[2 +: 4]`'ün
/// başlangıcı) sırayla kök const'lara çıkar.
fn lift(e: &mut Expr, id: usize, consts: &mut Vec<(String, i128)>) {
    match e {
        Expr::Index(b, i) => {
            lift(b, id, consts);
            lift_lit(i, id, consts);
        }
        Expr::Part(b, s, ..) => {
            lift(b, id, consts);
            lift_lit(s, id, consts);
        }
        Expr::Name(_, b) | Expr::Un(_, b) | Expr::Cast(b, _) | Expr::Range(b, ..) => {
            lift(b, id, consts)
        }
        Expr::Bin(_, a, b) => {
            lift(a, id, consts);
            lift(b, id, consts);
        }
        Expr::Concat(xs) => {
            for x in xs {
                lift(x, id, consts);
            }
        }
        Expr::Port(_) | Expr::Lit(..) => {}
    }
}

fn lift_lit(e: &mut Expr, id: usize, consts: &mut Vec<(String, i128)>) {
    if let Expr::Lit(v, _) = e {
        let name = lit_name(id, consts.len());
        consts.push((name.clone(), *v));
        let lit = std::mem::replace(e, Expr::Port(0));
        *e = Expr::Name(name, Box::new(lit));
    } else {
        lift(e, id, consts);
    }
}

fn renamed(e: &Expr, from: &str, to: &str) -> Expr {
    let mut e = e.clone();
    rename_in(&mut e, from, to);
    e
}

fn rename_in(e: &mut Expr, from: &str, to: &str) {
    match e {
        Expr::Name(n, b) => {
            if n == from {
                *n = to.to_string();
            }
            rename_in(b, from, to);
        }
        Expr::Index(a, b) | Expr::Bin(_, a, b) | Expr::Part(a, b, ..) => {
            rename_in(a, from, to);
            rename_in(b, from, to);
        }
        Expr::Un(_, b) | Expr::Cast(b, _) | Expr::Range(b, ..) => rename_in(b, from, to),
        Expr::Concat(xs) => {
            for x in xs {
                rename_in(x, from, to);
            }
        }
        Expr::Port(_) | Expr::Lit(..) => {}
    }
}

/// `Name(name, …)` düğümünün değeri.
fn find_name<'e>(e: &'e Expr, name: &str) -> Option<&'e Expr> {
    if let Expr::Name(n, b) = e {
        if n == name {
            return Some(b);
        }
    }
    e.children().into_iter().find_map(|c| find_name(c, name))
}

fn is_select_base(e: &Expr, pred: &dyn Fn(&Expr) -> bool) -> bool {
    match e {
        Expr::Index(b, _) | Expr::Range(b, ..) | Expr::Part(b, ..) if pred(b) => true,
        _ => e.children().into_iter().any(|c| is_select_base(c, pred)),
    }
}

fn name_is_select_base(e: &Expr, name: &str) -> bool {
    is_select_base(e, &|b| matches!(b, Expr::Name(n, _) if n == name))
}

/// Bitleri doğrudan seçilen portlar (`p0[3]`): ikame kipinde onlara bir
/// yerel geçirilemez (ADR-0081, E0003).
fn select_bases(e: &Expr) -> Vec<usize> {
    let mut out = Vec::new();
    collect_select_bases(e, &mut out);
    out
}

fn collect_select_bases(e: &Expr, out: &mut Vec<usize>) {
    if let Expr::Index(b, _) | Expr::Range(b, ..) | Expr::Part(b, ..) = e {
        if let Expr::Port(k) = **b {
            out.push(k);
        }
    }
    for c in e.children() {
        collect_select_bases(c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinOp, LitFmt};

    fn p(k: usize) -> Expr {
        Expr::Port(k)
    }

    fn lit(v: i128) -> Expr {
        Expr::Lit(v, LitFmt::Dec)
    }

    /// #128'in biçimi: `p0[1]` gövdede `p0[t7r]`; çağrı `for t7r in 2..3`
    /// içinde. Gövde t7r'yi kök const'ta (1) çözmeli, döngü değişkeninde
    /// (2) değil.
    #[test]
    fn the_select_literal_becomes_a_const_named_like_the_loop_variable() {
        let ports = [PortTy::UInt(4), PortTy::UInt(4)];
        let e = Expr::bin(
            BinOp::Add,
            Expr::cast(
                Expr::Index(Box::new(p(0)), Box::new(lit(1))),
                PortTy::UInt(4),
            ),
            p(1),
        );
        let h = Hygiene::new(7, &e, None, "s7c").expect("çakışma var");
        assert_eq!(
            h.item_lines(7, &ports, PortTy::UInt(5)),
            vec![
                "const t7r : u32 = 1",
                "const s7r : u32 = 0",
                "fn h7r(p0: u4, p1: u4) -> u5 {",
                "    p0[t7r] as u4 + p1",
                "}",
            ]
        );
        assert_eq!(
            h.module_lines(7, "o7r"),
            vec![
                "    comb {",
                "        let s7r = p1",
                "        for t7r in 2..3 {",
                "            o7r = h7r(p0, s7r)",
                "        }",
                "    }",
            ]
        );
    }

    /// C biçiminin alt ifadesi gövdenin `let`'i olur ve aynı adlı kök
    /// const'u gölgeler; seçim tabanı olan alt ifade `let`'e çıkmaz.
    #[test]
    fn the_extracted_sub_expression_is_a_body_let_unless_it_is_selected() {
        let ports = [PortTy::UInt(4), PortTy::UInt(4)];
        let sum = Expr::bin(BinOp::Add, p(0), p(1));
        let ex = Expr::bin(
            BinOp::Xor,
            Expr::Name("s3c".into(), Box::new(sum.clone())),
            p(0),
        );
        let e = Expr::bin(BinOp::Xor, sum.clone(), p(0));
        let h = Hygiene::new(3, &e, Some(&ex), "s3c").expect("let var");
        let text = h.item_lines(3, &ports, PortTy::UInt(5));
        assert!(
            text.contains(&"    let s3r = p0 + p1;".to_string()),
            "{text:?}"
        );
        assert!(text.contains(&"    s3r ^ p0".to_string()), "{text:?}");
        assert!(
            text.contains(&"const s3r : u32 = 0".to_string()),
            "{text:?}"
        );
        // Seçilen alt ifade: (p0 + p1)[2] — let yok, ifade olduğu gibi.
        let ex = Expr::Index(
            Box::new(Expr::Name("s3c".into(), Box::new(sum.clone()))),
            Box::new(p(1)),
        );
        let e = Expr::Index(Box::new(sum), Box::new(p(1)));
        let h = Hygiene::new(3, &e, Some(&ex), "s3c").expect("argüman yereli var");
        assert!(h.body_let.is_none());
        assert_eq!(print(&h.body, Style::Min), "(p0 + p1)[p1]");
    }

    /// Bitleri seçilen port bir yerelden geçirilmez; başka çakışma yoksa
    /// biçim yok.
    #[test]
    fn no_form_without_a_name_to_collide() {
        let e = Expr::Index(Box::new(p(0)), Box::new(p(1)));
        let h = Hygiene::new(1, &e, None, "s1c").expect("p1 yerelden geçer");
        assert_eq!(h.caller_let, Some(1));
        let e = Expr::Index(
            Box::new(p(0)),
            Box::new(Expr::Index(Box::new(p(0)), Box::new(p(0)))),
        );
        assert!(Hygiene::new(1, &e, None, "s1c").is_none());
    }
}
