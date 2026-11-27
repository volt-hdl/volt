//! Beklenen ret ölçütü (#458): her tip için dilin tanımlamadığı işlemler
//! tek tabloda ([`UNDEFINED`]), her satır spesifikasyonun ya da bir
//! ADR'nin adlandırdığı tanı koduyla. Mod tanımsız birleşimi üretecin
//! işlenenleriyle (port, `-t`, `t * u`, iç içe ifade) üretir, `volt check`
//! ile denetler ve her ifadenin satırında tam o kodu bekler. Derleyici
//! ifadeyi kabul ederse ya da başka bir kodla reddederse fark sayılır.
//! Verilator gerekmez.
//!
//! Bugünkü spesifikasyona uyan bir üreteç `~t`'yi geçerli program olarak
//! hiç üretmez (ADR-0142); #434'ün sınıfı (tanımsız bir işlemin kabul
//! edilip sessizce yanlış kod üretmesi) ancak böyle görülür.
//!
//! Tablonun her satırı modelle de denetlenir: üretilen ifadeye modelin
//! verdiği kod satırın kodudur (`run_reject` aksi hâlde düşer). Tablo ile
//! model aynı kuralı iki yerde yazmaz: tablo hangi birleşimlerin
//! deneneceğini ve kaynağını, model kodun kuralını taşır.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use crate::ast::{BinOp, Expr, LitFmt, PortTy, UnOp};
use crate::gen::{Gen, Pool};
use crate::model::{Model, Reject};
use crate::print::{port_name, print, Style};
use crate::rng::Rng;

/// Tanımsız işlemin işleneninin tipi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Trit,
    UInt,
    SInt,
    Bits,
    Bool,
    /// Soneksiz tam sayı literali.
    Lit,
}

impl Operand {
    pub fn name(self) -> &'static str {
        match self {
            Operand::Trit => "Trit",
            Operand::UInt => "uN",
            Operand::SInt => "iN",
            Operand::Bits => "bits<N>",
            Operand::Bool => "bool",
            Operand::Lit => "integer literal",
        }
    }
}

/// Dönüşüm hedefi (`a as T`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Trit,
    /// `uN` (N, 2 ya da işlenenin genişliği).
    UInt,
    /// `i1`: §3.6 `Trit → iN` yalnız N >= 2.
    SInt1,
    Bool,
    /// `bits<N>` (N, 2 ya da işlenenin genişliği).
    Bits,
}

/// Tanımsız işlem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Un(UnOp),
    Bin(BinOp),
    /// Bit seçme `a[i]`.
    Index,
    Cast(Target),
    /// Tipli `let`: literal `K` Trit bağlamında (`let r : Trit = K`).
    LetTrit,
}

impl Op {
    pub fn text(self) -> String {
        match self {
            Op::Un(op) => format!("{}a", op.sym()),
            Op::Bin(op) => format!("a {} b", op.sym()),
            Op::Index => "a[i]".into(),
            Op::Cast(Target::Trit) => "a as Trit".into(),
            Op::Cast(Target::UInt) => "a as uN".into(),
            Op::Cast(Target::SInt1) => "a as i1".into(),
            Op::Cast(Target::Bool) => "a as bool".into(),
            Op::Cast(Target::Bits) => "a as bits<N>".into(),
            Op::LetTrit => "let r : Trit = K (K not in {-1, 0, +1})".into(),
        }
    }
}

/// Tablonun bir satırı: tip, tanımsız işlem, beklenen kod, kaynak.
#[derive(Clone, Copy, Debug)]
pub struct Undefined {
    pub on: Operand,
    pub op: Op,
    pub code: &'static str,
    pub source: &'static str,
}

impl Undefined {
    pub fn text(&self) -> String {
        format!("{}: {}", self.on.name(), self.op.text())
    }
}

const fn row(on: Operand, op: Op, code: &'static str, source: &'static str) -> Undefined {
    Undefined {
        on,
        op,
        code,
        source,
    }
}

const A142_1: &str = "ADR-0142 §1, type-inference.md §3.4";
const A142_4: &str = "ADR-0142 §4";
const CAST: &str = "type-inference.md §3.6";

/// Her tipin tanımsız işlemleri.
pub const UNDEFINED: &[Undefined] = &[
    row(Operand::Trit, Op::Un(UnOp::BitNot), "E2003", A142_1),
    row(Operand::Trit, Op::Bin(BinOp::And), "E2003", A142_4),
    row(Operand::Trit, Op::Bin(BinOp::Or), "E2003", A142_4),
    row(Operand::Trit, Op::Bin(BinOp::Xor), "E2003", A142_4),
    row(Operand::Trit, Op::Bin(BinOp::Shl), "E2003", A142_4),
    row(Operand::Trit, Op::Bin(BinOp::Shr), "E2003", A142_4),
    row(Operand::Trit, Op::Index, "E2003", A142_4),
    row(Operand::Trit, Op::Cast(Target::UInt), "E2009", CAST),
    row(Operand::Trit, Op::Cast(Target::SInt1), "E2009", CAST),
    row(Operand::Trit, Op::Cast(Target::Bool), "E2009", CAST),
    row(Operand::Trit, Op::Cast(Target::Bits), "E2009", CAST),
    row(
        Operand::UInt,
        Op::Cast(Target::Trit),
        "E2009",
        "type-inference.md §3.6, §10",
    ),
    row(
        Operand::SInt,
        Op::Cast(Target::Trit),
        "E2009",
        "type-inference.md §3.6, §10",
    ),
    row(Operand::Bits, Op::Cast(Target::Trit), "E2009", CAST),
    row(Operand::Bool, Op::Cast(Target::Trit), "E2009", CAST),
    row(Operand::Lit, Op::Cast(Target::Trit), "E2009", CAST),
    row(
        Operand::Lit,
        Op::LetTrit,
        "E2011",
        "type-inference.md §4, §7; ADR-0003",
    ),
    row(
        Operand::Bits,
        Op::Bin(BinOp::Add),
        "E2004",
        "type-inference.md §3.3",
    ),
    row(
        Operand::Bits,
        Op::Bin(BinOp::Sub),
        "E2004",
        "type-inference.md §3.3",
    ),
    row(
        Operand::Bits,
        Op::Bin(BinOp::Mul),
        "E2004",
        "type-inference.md §3.3",
    ),
    row(
        Operand::Bits,
        Op::Bin(BinOp::Div),
        "E2004",
        "type-inference.md §3.3",
    ),
    row(
        Operand::Bits,
        Op::Bin(BinOp::Rem),
        "E2004",
        "type-inference.md §3.3",
    ),
    row(
        Operand::UInt,
        Op::Un(UnOp::Neg),
        "E2002",
        "type-inference.md §3.4",
    ),
];

/// Bir ret örneği: tablo satırı, ifade, tipli `let`'in hedefi.
#[derive(Clone, Debug)]
pub struct RejectCase {
    pub row: usize,
    pub expr: Expr,
    pub target: Option<PortTy>,
}

impl RejectCase {
    /// Modüldeki satırı: `let r<k> = e` ya da `let r<k> : T = e`.
    pub fn line(&self, k: usize) -> String {
        match self.target {
            Some(t) => format!(
                "    let r{k} : {} = {}",
                t.volt(),
                print(&self.expr, Style::Min)
            ),
            None => format!("    let r{k} = {}", print(&self.expr, Style::Min)),
        }
    }

    /// Modelin kodu (tablonun koduyla aynı olmalı).
    pub fn model_code(&self, ports: &[PortTy]) -> Option<&'static str> {
        let m = Model { ports };
        let r = match self.target {
            Some(t) => m.check(&self.expr, t).map(|_| ()),
            None => m.synth(&self.expr).map(|_| ()),
        };
        match r {
            Err(Reject::Invalid(c)) => Some(c),
            _ => None,
        }
    }
}

fn operand(g: &mut Gen, on: Operand, w: u32, d: u32) -> Expr {
    match on {
        Operand::Trit => g.trit_any(d),
        Operand::UInt => g.int_exact(false, w, d),
        Operand::SInt => g.int_exact(true, w, d),
        Operand::Bits => g.bits(w, d),
        Operand::Bool => g.boolean(d),
        Operand::Lit => Expr::Lit(i128::from(g.rng.range(0, 1)), LitFmt::Dec),
    }
}

/// Satırın bir örneği; işlenenler üretecten, iç içelik 0–2.
fn instance(g: &mut Gen, u: &Undefined) -> (Expr, Option<PortTy>) {
    let d = g.rng.range(0, 2);
    let w = g.rng.range(2, 24);
    let a = operand(g, u.on, w, d);
    match u.op {
        Op::Un(op) => (Expr::un(op, a), None),
        Op::Bin(op) if op.is_shift() => {
            let amt = if g.rng.chance(50) {
                Expr::Lit(i128::from(g.rng.range(0, 2)), LitFmt::Dec)
            } else {
                let aw = g.rng.range(1, 3);
                g.pool.leaf(g.rng, PortTy::UInt(aw))
            };
            (Expr::bin(op, a, amt), None)
        }
        Op::Bin(op) => {
            let sd = g.rng.range(0, 2);
            let b = operand(g, u.on, w, sd);
            (Expr::bin(op, a, b), None)
        }
        Op::Index => {
            let i = if g.rng.chance(50) {
                Expr::Lit(i128::from(g.rng.range(0, 1)), LitFmt::Dec)
            } else {
                g.pool.leaf(g.rng, PortTy::UInt(1))
            };
            (Expr::Index(Box::new(a), Box::new(i)), None)
        }
        Op::Cast(t) => {
            let n = if g.rng.chance(50) { 2 } else { w };
            let t = match t {
                Target::Trit => PortTy::Trit,
                Target::UInt => PortTy::UInt(n),
                Target::SInt1 => PortTy::SInt(1),
                Target::Bool => PortTy::Bool,
                Target::Bits => PortTy::Bits(n),
            };
            (Expr::cast(a, t), None)
        }
        Op::LetTrit => {
            let k = i128::from(g.rng.range(2, 9));
            let fmt = g.rng.pick(&[LitFmt::Dec, LitFmt::Hex, LitFmt::Bin]);
            (Expr::Lit(k, fmt), Some(PortTy::Trit))
        }
    }
}

/// Ret modunun ifadeleri ve portları: her satırdan `per_row` örnek.
pub fn generate(seed: u64, per_row: usize) -> (Vec<RejectCase>, Vec<PortTy>) {
    let mut rng = Rng::new(seed);
    let mut pool = Pool::default();
    let mut cases = Vec::new();
    for (ri, u) in UNDEFINED.iter().enumerate() {
        for _ in 0..per_row {
            let mut g = Gen {
                rng: &mut rng,
                pool: &mut pool,
            };
            let (expr, target) = instance(&mut g, u);
            cases.push(RejectCase {
                row: ri,
                expr,
                target,
            });
        }
    }
    (cases, pool.ports)
}

pub const MODULE: &str = "Rj";

/// Ret modunun Volt modülü; `r<k>` satırı `cases[k]`'nindir.
pub fn module(cases: &[RejectCase], ports: &[PortTy]) -> (String, Vec<Option<usize>>) {
    let mut lines: Vec<(String, Option<usize>)> = vec![(format!("module {MODULE} {{"), None)];
    let mut used = Vec::new();
    for c in cases {
        c.expr.ports(&mut used);
    }
    used.sort_unstable();
    used.dedup();
    for k in used {
        lines.push((
            format!("    in {} : {}", port_name(k), ports[k].volt()),
            None,
        ));
    }
    for (k, c) in cases.iter().enumerate() {
        lines.push((c.line(k), Some(k)));
    }
    lines.push(("}".into(), None));
    let mut text = String::new();
    let mut owner = Vec::new();
    for (l, o) in lines {
        text.push_str(&l);
        text.push('\n');
        owner.push(o);
    }
    (text, owner)
}

/// Bir satırın sonucu.
#[derive(Clone, Debug, Default)]
pub struct RowResult {
    pub cases: usize,
    /// Beklenen kodla reddedilen.
    pub as_expected: usize,
    /// Tanısız kabul edilen.
    pub accepted: usize,
    /// Başka kodla reddedilen: kod → sayı.
    pub other: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Default)]
pub struct RejectReport {
    pub seed: u64,
    pub per_row: usize,
    /// `UNDEFINED` sırasıyla.
    pub rows: Vec<RowResult>,
    /// Her fark: satırın metni, beklenen kod, derleyicinin cevabı.
    pub findings: Vec<String>,
}

impl RejectReport {
    pub fn cases(&self) -> usize {
        self.rows.iter().map(|r| r.cases).sum()
    }

    pub fn diffs(&self) -> usize {
        self.rows
            .iter()
            .map(|r| r.accepted + r.other.values().sum::<usize>())
            .sum()
    }

    pub fn ok(&self) -> bool {
        self.diffs() == 0 && self.rows.iter().all(|r| r.cases > 0)
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        let accepted: usize = self.rows.iter().map(|r| r.accepted).sum();
        let as_expected: usize = self.rows.iter().map(|r| r.as_expected).sum();
        let _ = writeln!(
            s,
            "difftest-reject: seed={} rows={} cases={} rejected-as-expected={} accepted={} other-code={} diffs={}",
            self.seed,
            self.rows.len(),
            self.cases(),
            as_expected,
            accepted,
            self.diffs() - accepted,
            self.diffs()
        );
        for (u, r) in UNDEFINED.iter().zip(&self.rows) {
            let mut line = format!(
                "  {} — expected {} ({}): {} cases, {} {}",
                u.text(),
                u.code,
                u.source,
                r.cases,
                u.code,
                r.as_expected
            );
            if r.accepted > 0 {
                let _ = write!(line, ", ACCEPTED {}", r.accepted);
            }
            for (c, n) in &r.other {
                let _ = write!(line, ", {c} {n}");
            }
            let _ = writeln!(s, "{line}");
        }
        for f in &self.findings {
            let _ = writeln!(s, "difftest-reject: diff: {f}");
        }
        s
    }
}

/// `volt check --format=short` çıktısının hata satırları: (satır, kod).
pub fn errors(text: &str, file: &str) -> Vec<(usize, String)> {
    let prefix = format!("{file}:");
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix(&prefix) else {
            continue;
        };
        let mut it = rest.splitn(3, ':');
        let (Some(l), Some(_), Some(msg)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let Some(code) = msg.trim_start().strip_prefix("error[") else {
            continue;
        };
        if let Ok(l) = l.parse() {
            out.push((l, code.chars().take_while(|c| *c != ']').collect()));
        }
    }
    out
}

/// Derleyicinin çıktısını tabloyla karşılaştırır. Bir satırın ilk kodu
/// sayılır; sahipsiz satırda hata koşunun kendisinin bozuk olduğunu
/// söyler (`Err`).
pub fn classify(
    seed: u64,
    per_row: usize,
    cases: &[RejectCase],
    owner: &[Option<usize>],
    module_text: &str,
    check_output: &str,
) -> Result<RejectReport, String> {
    let mut first: BTreeMap<usize, String> = BTreeMap::new();
    for (l, code) in errors(check_output, "rj.volt") {
        match owner.get(l.wrapping_sub(1)).copied().flatten() {
            Some(k) => {
                first.entry(k).or_insert(code);
            }
            None => {
                return Err(format!(
                    "error on line {l}, which no case owns:\n{check_output}\n--- rj.volt\n{module_text}"
                ))
            }
        }
    }
    let mut rep = RejectReport {
        seed,
        per_row,
        rows: vec![RowResult::default(); UNDEFINED.len()],
        findings: Vec::new(),
    };
    for (k, c) in cases.iter().enumerate() {
        let u = &UNDEFINED[c.row];
        let r = &mut rep.rows[c.row];
        r.cases += 1;
        let text = c.line(k).trim().to_string();
        match first.get(&k) {
            Some(code) if code == u.code => r.as_expected += 1,
            Some(code) => {
                *r.other.entry(code.clone()).or_default() += 1;
                rep.findings.push(format!(
                    "{} — expected {}, compiler {code}: {text}",
                    u.text(),
                    u.code
                ));
            }
            None => {
                r.accepted += 1;
                rep.findings.push(format!(
                    "{} — expected {}, compiler accepted: {text}",
                    u.text(),
                    u.code
                ));
            }
        }
    }
    Ok(rep)
}

/// Ret modu: `volt` ikilisiyle (yolu çağıran verir; v0.1.3 gibi bir sürüm
/// de olabilir) tek `volt check`. `work` dizini koşunun dosyasını tutar.
pub fn run_reject(volt: &Path, seed: u64, per_row: usize, work: &Path) -> RejectReport {
    let (cases, ports) = generate(seed, per_row);
    for c in &cases {
        let u = &UNDEFINED[c.row];
        assert_eq!(
            c.model_code(&ports),
            Some(u.code),
            "tablo ile model ayrışıyor: {} — {}",
            u.text(),
            c.line(0).trim()
        );
    }
    let (text, owner) = module(&cases, &ports);
    std::fs::create_dir_all(work).expect("çalışma dizini");
    std::fs::write(work.join("rj.volt"), &text).expect("rj.volt");
    let out = Command::new(volt)
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(work)
        .args(["check", "--format=short", "--max-diagnostics=0", "rj.volt"])
        .output()
        .unwrap_or_else(|e| panic!("{} çalıştırılamadı: {e}", volt.display()));
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    classify(seed, per_row, &cases, &owner, &text, &output).unwrap_or_else(|e| panic!("{e}"))
}
