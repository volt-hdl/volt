//! Bir toplu koşunun Volt modülü, SV testbench'i ve giriş vektörleri.
//!
//! Her ifade birkaç biçimde aynı modüle girer (`Variant`); her biçimin
//! satırları kaydedilir, böylece tek `volt check` çağrısının tanıları ve
//! üretilen SV'nin satırları ifadeye ve biçime geri eşlenir. Bütün
//! ifadeler tek Verilator derlemesinde koşar.

use std::collections::BTreeMap;

use crate::ast::{BinOp, Expr, LitFmt, PortTy};
use crate::hygiene::Hygiene;
use crate::model::Model;
use crate::print::{port_name, print, print_operand, Style};
use crate::rng::Rng;
use crate::value::mask;

/// Bir ifadenin modüldeki biçimleri.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Variant {
    /// Satır içi: `o = e`.
    A,
    /// Tipsiz `let`: `let t = e; o = t`.
    B,
    /// Bir alt ifade tipsiz `let`'e çıkarılmış.
    C,
    /// Tam parantez.
    D,
    /// Değişmeli işlemlerin işlenenleri yer değiştirmiş.
    E,
    /// Karşılaştırmada: `(yer değiştirmiş e) == t` (hep doğru).
    F,
    /// Literal ile karşılaştırma: `e < K`.
    K,
    /// Daha geniş hedef: `o : u(N+k) = e` (ADR-0041 itmesi).
    W,
    /// Sözleşmede: `ensures: o == e`.
    G,
    /// Ad hijyeni (#128): ifade bir fn gövdesinde, seçim literalleri kök
    /// const'larda; çağrı aynı adlı döngü değişkeninin içinde, gövdenin ve
    /// çağıranın `let`'leri bir const'la aynı adda (`hygiene`).
    R,
}

impl Variant {
    pub const ALL: [Variant; 10] = [
        Variant::A,
        Variant::B,
        Variant::C,
        Variant::D,
        Variant::E,
        Variant::F,
        Variant::K,
        Variant::W,
        Variant::G,
        Variant::R,
    ];

    pub fn letter(self) -> char {
        match self {
            Variant::A => 'a',
            Variant::B => 'b',
            Variant::C => 'c',
            Variant::D => 'd',
            Variant::E => 'e',
            Variant::F => 'f',
            Variant::K => 'k',
            Variant::W => 'w',
            Variant::G => 'g',
            Variant::R => 'r',
        }
    }

    pub fn from_letter(c: char) -> Option<Variant> {
        Variant::ALL.into_iter().find(|v| v.letter() == c)
    }

    pub fn what(self) -> &'static str {
        match self {
            Variant::A => "inline",
            Variant::B => "untyped let",
            Variant::C => "sub-expression extracted into a let",
            Variant::D => "extra parentheses",
            Variant::E => "commuted operands",
            Variant::F => "compared with its let (always true)",
            Variant::K => "compared with a literal",
            Variant::W => "assigned to a wider target",
            Variant::G => "contract (ensures)",
            Variant::R => "in a function, names colliding with the caller's loop variable and lets",
        }
    }
}

/// Modelin bir ifade için kararı.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// Geçerli; doğal tipi (çıkışın tipi).
    Valid(PortTy),
    /// Spesifikasyona göre hatalı; beklenen kod (`"?"`: adlandırılmamış).
    Invalid(&'static str),
    Unsupported(&'static str),
    Unspecified(&'static str),
}

#[derive(Clone, Debug)]
pub struct Case {
    /// Toplu koşu içinde benzersiz; adlar bundan türer (`o12a`, `t12b`).
    pub id: usize,
    pub expr: Expr,
    pub kind: Kind,
    /// C biçimi: alt ifadesi `Name` olan ağaç.
    pub extracted: Option<Expr>,
    /// K biçimi: işlem ve literal.
    pub cmp_lit: Option<(BinOp, i128)>,
    /// W biçimi: daha geniş hedef.
    pub wide: Option<PortTy>,
    /// R biçimi: fn gövdesi ve çakışan adlar.
    pub hygiene: Option<Hygiene>,
    /// Modüle giren biçimler (modelin geçerli dediği).
    pub variants: Vec<Variant>,
}

pub fn out_name(id: usize, v: Variant) -> String {
    format!("o{id}{}", v.letter())
}

pub fn let_name(id: usize) -> String {
    format!("t{id}b")
}

pub fn sub_name(id: usize) -> String {
    format!("s{id}c")
}

impl Case {
    /// Modelle sınıflandırır ve biçimleri seçer.
    pub fn new(id: usize, expr: Expr, ports: &[PortTy], rng: &mut Rng) -> Case {
        let model = Model { ports };
        let kind = match model.synth(&expr) {
            Ok(t) => match t.ty.natural() {
                Some(nat) if model.check(&expr, nat).is_ok() => Kind::Valid(nat),
                Some(_) => Kind::Invalid("?"),
                None => Kind::Unspecified("untyped literal expression"),
            },
            Err(crate::model::Reject::Invalid(c)) => Kind::Invalid(c),
            Err(crate::model::Reject::Unsupported(r)) => Kind::Unsupported(r),
            Err(crate::model::Reject::Unspecified(r)) => Kind::Unspecified(r),
        };
        let mut case = Case {
            id,
            expr,
            kind,
            extracted: None,
            cmp_lit: None,
            wide: None,
            hygiene: None,
            variants: vec![],
        };
        let Kind::Valid(nat) = case.kind else {
            // Geçersiz ifade tipsiz `let`'lerde: kodlar biçimler arasında aynı olmalı.
            case.variants = vec![Variant::B, Variant::D, Variant::E];
            return case;
        };
        // C: büyükçe bir iç düğüm (yaprak ve literal değil).
        let inner: Vec<Vec<usize>> = case
            .expr
            .paths()
            .into_iter()
            .filter(|p| !p.is_empty())
            .filter(|p| {
                let n = case.expr.at(p);
                !matches!(n, Expr::Port(_) | Expr::Lit(..) | Expr::Name(..))
                    && model.synth(n).is_ok_and(|t| t.ty.natural().is_some())
            })
            .collect();
        if !inner.is_empty() {
            let path = &inner[rng.below(inner.len() as u64) as usize];
            let sub = case.expr.at(path).clone();
            let named = Expr::Name(sub_name(id), Box::new(sub));
            case.extracted = Some(case.expr.replaced(path, named));
        }
        if let PortTy::UInt(w) | PortTy::SInt(w) = nat {
            let signed = nat.signed();
            let raw = rng.bits(w);
            let v = if signed {
                crate::value::to_i128(raw, w, true)
            } else {
                raw as i128
            };
            case.cmp_lit = Some((rng.pick(&BinOp::CMP), v));
            if w < crate::model::MAX_INT {
                let k = rng.range(1, (crate::model::MAX_INT - w).min(6));
                let t = PortTy::int(signed, w + k);
                if model.check(&case.expr, t).is_ok() {
                    case.wide = Some(t);
                }
            }
        }
        case.hygiene = Hygiene::new(id, &case.expr, case.extracted.as_ref(), &sub_name(id));
        case.variants = Variant::ALL
            .into_iter()
            .filter(|v| match v {
                Variant::C => case.extracted.is_some(),
                Variant::K => case.cmp_lit.is_some(),
                Variant::W => case.wide.is_some(),
                Variant::R => case.hygiene.is_some(),
                _ => true,
            })
            .collect();
        case
    }

    /// Sözleşme biçimi (g) iki x'i karşılaştırırdı: modelin herhangi bir
    /// vektörde x dediği ifadede (sınır dışı değişken seçim, ADR-0035)
    /// sözleşme konmaz; değer biçimleri o vektörü zaten atlar.
    pub fn drop_x_contract(&mut self, ports: &[PortTy], vecs: &[Vec<u128>]) {
        if !self.is_valid() {
            return;
        }
        let model = Model { ports };
        if vecs.iter().any(|v| model.eval(&self.expr, v).is_none()) {
            self.variants.retain(|v| *v != Variant::G);
        }
    }

    pub fn is_valid(&self) -> bool {
        matches!(self.kind, Kind::Valid(_))
    }

    /// Biçimin modeldeki beklenen değeri; `None` = karşılaştırılmaz (x).
    pub fn expected(&self, v: Variant, model: &Model, inp: &[u128]) -> Option<u128> {
        let Kind::Valid(nat) = self.kind else {
            return None;
        };
        match v {
            Variant::A | Variant::D => model.eval_check(&self.expr, nat, inp),
            Variant::B => {
                let named = Expr::Name(let_name(self.id), Box::new(self.expr.clone()));
                model.eval_check(&named, nat, inp)
            }
            Variant::C => model.eval_check(self.extracted.as_ref()?, nat, inp),
            Variant::E => model.eval_check(&self.expr.commuted(), nat, inp),
            Variant::F => Some(1),
            Variant::K => {
                let (op, k) = self.cmp_lit?;
                let e = Expr::bin(op, self.expr.clone(), Expr::Lit(k, LitFmt::Dec));
                model.eval(&e, inp)
            }
            Variant::W => model.eval_check(&self.expr, self.wide?, inp),
            Variant::G => None,
            // Gövdenin `let`'i C biçiminin alt ifadesidir.
            Variant::R => match self.hygiene.as_ref()?.body_let {
                Some(_) => model.eval_check(self.extracted.as_ref()?, nat, inp),
                None => model.eval_check(&self.expr, nat, inp),
            },
        }
    }

    /// Biçimin varsayım etiketleri (modelin bu biçim için kullandığı).
    pub fn tags(&self, v: Variant, model: &Model) -> u32 {
        let Kind::Valid(nat) = self.kind else {
            return 0;
        };
        let base = model.synth(&self.expr).map_or(0, |t| t.tags);
        let check = match v {
            Variant::W => self
                .wide
                .map_or(0, |t| model.check(&self.expr, t).unwrap_or(0)),
            Variant::C => self
                .extracted
                .as_ref()
                .map_or(0, |e| model.check(e, nat).unwrap_or(0)),
            Variant::R if self.hygiene.as_ref().is_some_and(|h| h.body_let.is_some()) => self
                .extracted
                .as_ref()
                .map_or(0, |e| model.check(e, nat).unwrap_or(0)),
            _ => model.check(&self.expr, nat).unwrap_or(0),
        };
        base | check
    }

    /// Biçimin çıkış tipi (sözleşmenin çıkışı yok).
    pub fn out_ty(&self, v: Variant) -> Option<PortTy> {
        let Kind::Valid(nat) = self.kind else {
            return None;
        };
        match v {
            Variant::F | Variant::K => Some(PortTy::Bool),
            Variant::W => self.wide,
            Variant::G => None,
            _ => Some(nat),
        }
    }

    /// Biçimin Volt metni (hata raporu için).
    pub fn variant_text(&self, v: Variant) -> String {
        let id = self.id;
        match v {
            Variant::A => format!("{} = {}", out_name(id, v), print(&self.expr, Style::Min)),
            Variant::B => format!(
                "let {} = {}; {} = {}",
                let_name(id),
                print(&self.expr, Style::Min),
                out_name(id, v),
                let_name(id)
            ),
            Variant::C => {
                let ex = self.extracted.as_ref().expect("C biçimi");
                let mut sub = String::new();
                for p in ex.paths() {
                    if let Expr::Name(_, b) = ex.at(&p) {
                        sub = print(b, Style::Min);
                    }
                }
                format!(
                    "let {} = {sub}; {} = {}",
                    sub_name(id),
                    out_name(id, v),
                    print(ex, Style::Min)
                )
            }
            Variant::D => format!("{} = {}", out_name(id, v), print(&self.expr, Style::Full)),
            Variant::E => format!(
                "{} = {}",
                out_name(id, v),
                print(&self.expr.commuted(), Style::Min)
            ),
            Variant::F => format!(
                "{} = {} == {}",
                out_name(id, v),
                print_operand(&self.expr.commuted(), BinOp::Eq, Style::Min),
                let_name(id)
            ),
            Variant::K => {
                let (op, k) = self.cmp_lit.expect("K biçimi");
                format!(
                    "{} = {} {} {}",
                    out_name(id, v),
                    print_operand(&self.expr, op, Style::Min),
                    op.sym(),
                    lit_operand(k)
                )
            }
            Variant::W => format!("{} = {}", out_name(id, v), print(&self.expr, Style::Min)),
            Variant::G => format!(
                "ensures: {} == {}",
                out_name(id, Variant::A),
                print_operand(&self.expr, BinOp::Eq, Style::Min)
            ),
            Variant::R => {
                let h = self.hygiene.as_ref().expect("R biçimi");
                let ret = self.out_ty(v).expect("R biçiminin çıkışı");
                // Rapor için portların tipi bilinmez; gövde ve çağrı yeter.
                let mut lines: Vec<String> = h
                    .consts
                    .iter()
                    .map(|(n, v)| format!("const {n} : u32 = {v}"))
                    .collect();
                if let Some(l) = &h.body_let {
                    lines.push(format!(
                        "fn {}(..) -> {} {{ let {} = {}; {} }}",
                        crate::hygiene::fn_name(id),
                        ret.volt(),
                        crate::hygiene::let_name(id),
                        print(l, Style::Min),
                        print(&h.body, Style::Min)
                    ));
                } else {
                    lines.push(format!(
                        "fn {}(..) -> {} {{ {} }}",
                        crate::hygiene::fn_name(id),
                        ret.volt(),
                        print(&h.body, Style::Min)
                    ));
                }
                lines.extend(h.module_lines(id, &out_name(id, v)));
                lines.join("\n")
            }
        }
    }
}

fn lit_operand(k: i128) -> String {
    if k < 0 {
        format!("({k})")
    } else {
        format!("{k}")
    }
}

/// Bir çıkış sinyali.
#[derive(Clone, Debug)]
pub struct OutSig {
    pub name: String,
    pub ty: PortTy,
    pub case: usize,
    pub variant: Variant,
}

/// Üretilmiş Volt modülü.
#[derive(Clone, Debug)]
pub struct Built {
    pub text: String,
    /// Volt satırı (0 tabanlı) → (ifade kimliği, biçim).
    pub line_owner: Vec<Option<(usize, Variant)>>,
    pub outputs: Vec<OutSig>,
    /// Kullanılan giriş portları, artan sırada.
    pub inputs: Vec<usize>,
}

pub const MODULE: &str = "Dt";

pub fn build(cases: &[Case], ports: &[PortTy]) -> Built {
    let mut used = Vec::new();
    for c in cases {
        c.expr.ports(&mut used);
    }
    used.sort_unstable();
    used.dedup();

    let mut lines: Vec<(String, Option<(usize, Variant)>)> = vec![
        (format!("module {MODULE} {{"), None),
        ("    in clk : clock".into(), None),
    ];
    for k in &used {
        lines.push((
            format!("    in {} : {}", port_name(*k), ports[*k].volt()),
            None,
        ));
    }
    let mut outputs = Vec::new();
    for c in cases.iter().filter(|c| c.is_valid()) {
        for v in &c.variants {
            if let Some(t) = c.out_ty(*v) {
                let name = out_name(c.id, *v);
                lines.push((format!("    out {name} : {}", t.volt()), Some((c.id, *v))));
                outputs.push(OutSig {
                    name,
                    ty: t,
                    case: c.id,
                    variant: *v,
                });
            }
        }
    }
    for c in cases.iter().filter(|c| c.is_valid()) {
        if c.variants.contains(&Variant::G) {
            lines.push((
                format!("    {}", c.variant_text(Variant::G)),
                Some((c.id, Variant::G)),
            ));
        }
    }
    for c in cases {
        if !c.is_valid() {
            // Geçersiz ifade: yalnız tipsiz let'ler.
            for v in &c.variants {
                let e = match v {
                    Variant::D => print(&c.expr, Style::Full),
                    Variant::E => print(&c.expr.commuted(), Style::Min),
                    _ => print(&c.expr, Style::Min),
                };
                lines.push((
                    format!("    let t{}{} = {e}", c.id, v.letter()),
                    Some((c.id, *v)),
                ));
            }
            continue;
        }
        let id = c.id;
        lines.push((
            format!("    let {} = {}", let_name(id), print(&c.expr, Style::Min)),
            Some((id, Variant::B)),
        ));
        for v in &c.variants {
            let text = match v {
                Variant::B => format!("{} = {}", out_name(id, *v), let_name(id)),
                Variant::C => {
                    let ex = c.extracted.as_ref().expect("C biçimi");
                    for p in ex.paths() {
                        if let Expr::Name(n, b) = ex.at(&p) {
                            lines.push((
                                format!("    let {n} = {}", print(b, Style::Min)),
                                Some((id, Variant::C)),
                            ));
                        }
                    }
                    format!("{} = {}", out_name(id, *v), print(ex, Style::Min))
                }
                Variant::G => continue,
                Variant::R => {
                    let h = c.hygiene.as_ref().expect("R biçimi");
                    for l in h.module_lines(id, &out_name(id, *v)) {
                        lines.push((l, Some((id, Variant::R))));
                    }
                    continue;
                }
                _ => c.variant_text(*v),
            };
            lines.push((format!("    {text}"), Some((id, *v))));
        }
    }
    lines.push(("}".into(), None));
    // R biçiminin const'ları ve fn'i modülden sonra (öğe düzeyinde ileri
    // başvuru serbest, name-resolution.md §4).
    for c in cases.iter().filter(|c| c.variants.contains(&Variant::R)) {
        let (Some(h), Some(ret)) = (&c.hygiene, c.out_ty(Variant::R)) else {
            continue;
        };
        for l in h.item_lines(c.id, ports, ret) {
            lines.push((l, Some((c.id, Variant::R))));
        }
    }
    let mut text = String::new();
    let mut line_owner = Vec::new();
    for (l, o) in lines {
        text.push_str(&l);
        text.push('\n');
        line_owner.push(o);
    }
    Built {
        text,
        line_owner,
        outputs,
        inputs: used,
    }
}

/// `$display` ve `$readmemh` parçası başına en çok bu kadar bit: çok geniş
/// tek birleştirmeden kaçınılır (64 çıkışlık, 4096 biti aşan parçada
/// Verilator'un bastığı değerler girişlerle açıklanamıyordu).
pub const CHUNK_BITS: u32 = 1024;

/// Sıralı öğeleri, genişlikleri toplamı `CHUNK_BITS`'i aşmayan parçalara
/// böler (tek öğe sınırı aşsa da kendi parçasıdır); `(başlangıç, bitiş)`.
pub fn chunks_by_width(widths: &[u32]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut acc = 0;
    for (i, w) in widths.iter().enumerate() {
        if i > start && acc + w > CHUNK_BITS {
            out.push((start, i));
            start = i;
            acc = 0;
        }
        acc += w;
    }
    if start < widths.len() {
        out.push((start, widths.len()));
    }
    out
}

fn out_chunks(b: &Built) -> Vec<(usize, usize)> {
    let w: Vec<u32> = b.outputs.iter().map(|o| o.ty.width()).collect();
    chunks_by_width(&w)
}

fn in_chunks<'a>(b: &'a Built, ports: &[PortTy]) -> Vec<&'a [usize]> {
    let w: Vec<u32> = b.inputs.iter().map(|k| ports[*k].width()).collect();
    chunks_by_width(&w)
        .into_iter()
        .map(|(s, e)| &b.inputs[s..e])
        .collect()
}

fn in_chunk_width(ports: &[PortTy], chunk: &[usize]) -> u32 {
    chunk.iter().map(|k| ports[*k].width()).sum::<u32>().max(1)
}

/// SV testbench: vektörleri parça parça `$readmemh` ile okur, her vektörde
/// girişleri geri basar (koşucu denetler), çıkışları parça parça basar ve
/// sözleşmeler için bir saat kenarı üretir.
pub fn testbench(b: &Built, ports: &[PortTy], nvec: usize) -> String {
    let chunks: Vec<&[usize]> = in_chunks(b, ports);
    let mut s = String::new();
    s.push_str("`timescale 1ns/1ps\nmodule tb;\n    logic clk;\n    logic rst;\n");
    for (ci, ch) in chunks.iter().enumerate() {
        let w = in_chunk_width(ports, ch);
        s.push_str(&format!(
            "    logic [{}:0] vecs{ci} [0:{}];\n",
            w - 1,
            nvec - 1
        ));
    }
    for k in &b.inputs {
        s.push_str(&format!("    {} {};\n", ports[*k].sv(), port_name(*k)));
    }
    for o in &b.outputs {
        s.push_str(&format!("    {} {};\n", o.ty.sv(), o.name));
    }
    s.push_str(&format!("    {MODULE} dut (.*);\n"));
    s.push_str("    initial begin\n");
    for ci in 0..chunks.len() {
        s.push_str(&format!("        $readmemh(\"vec{ci}.hex\", vecs{ci});\n"));
    }
    s.push_str("        rst = 1'b1;\n        clk = 1'b0;\n        #1 clk = 1'b1;\n        #1 clk = 1'b0;\n        rst = 1'b0;\n");
    s.push_str(&format!("        for (int i = 0; i < {nvec}; i++) begin\n"));
    let names = |ch: &[usize]| -> String {
        ch.iter()
            .rev()
            .map(|k| port_name(*k))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (ci, ch) in chunks.iter().enumerate() {
        s.push_str(&format!("            {{{}}} = vecs{ci}[i];\n", names(ch)));
    }
    s.push_str("            #1;\n");
    for (ci, ch) in chunks.iter().enumerate() {
        s.push_str(&format!(
            "            $display(\"I %0d {ci} %h\", i, {{{}}});\n",
            names(ch)
        ));
    }
    for (ci, (start, end)) in out_chunks(b).into_iter().enumerate() {
        let chunk = &b.outputs[start..end];
        let names: Vec<&str> = chunk.iter().rev().map(|o| o.name.as_str()).collect();
        s.push_str(&format!(
            "            $display(\"R %0d {ci} %h\", i, {{{}}});\n",
            names.join(", ")
        ));
    }
    s.push_str(
        "            clk = 1'b1;\n            #1 clk = 1'b0;\n            #1;\n        end\n",
    );
    s.push_str("        $display(\"DONE\");\n        $finish;\n    end\nendmodule\n");
    s
}

/// Giriş vektörleri: `[vektör][port]`. İlk vektörler kenar değerleri
/// (sıfır, hepsi bir, yalnız en üst bit, en üst bit dışında hepsi, bir),
/// sonrası rastgele (bir kısmı küçük).
pub fn vectors(ports: &[PortTy], nvec: usize, rng: &mut Rng) -> Vec<Vec<u128>> {
    (0..nvec)
        .map(|i| {
            ports
                .iter()
                .map(|p| {
                    let w = p.width();
                    match i {
                        0 => 0,
                        1 => mask(w),
                        2 => 1u128 << (w - 1),
                        3 => mask(w) >> 1,
                        4 => 1,
                        _ => match rng.weighted(&[70, 15, 15]) {
                            0 => rng.bits(w),
                            1 => rng.bits(w.min(4)),
                            _ => mask(w) ^ rng.bits(w.min(3)),
                        },
                    }
                })
                .collect()
        })
        .collect()
}

/// Bit dizisi (küçük uç sözcükler).
#[derive(Clone, Debug, Default)]
pub struct BitVec(pub Vec<u64>);

impl BitVec {
    pub fn set(&mut self, off: u32, w: u32, v: u128) {
        for i in 0..w {
            if (v >> i) & 1 == 1 {
                let b = off + i;
                let word = (b / 64) as usize;
                if self.0.len() <= word {
                    self.0.resize(word + 1, 0);
                }
                self.0[word] |= 1 << (b % 64);
            }
        }
    }

    pub fn get(&self, off: u32, w: u32) -> u128 {
        let mut v = 0u128;
        for i in 0..w {
            let b = off + i;
            let word = (b / 64) as usize;
            if self.0.get(word).is_some_and(|x| (x >> (b % 64)) & 1 == 1) {
                v |= 1 << i;
            }
        }
        v
    }

    pub fn hex(&self, width: u32) -> String {
        let digits = width.div_ceil(4);
        (0..digits)
            .rev()
            .map(|d| {
                let n = self.get(d * 4, 4) as u32;
                char::from_digit(n, 16).expect("nibble")
            })
            .collect()
    }

    pub fn from_hex(s: &str) -> Option<BitVec> {
        let mut b = BitVec::default();
        for (i, c) in s.chars().rev().enumerate() {
            let n = c.to_digit(16)?;
            b.set(i as u32 * 4, 4, u128::from(n));
        }
        Some(b)
    }
}

/// Giriş parçası başına bir `vec<N>.hex` içeriği.
pub fn vec_hex(b: &Built, ports: &[PortTy], vecs: &[Vec<u128>]) -> Vec<String> {
    in_chunks(b, ports)
        .into_iter()
        .map(|ch| {
            let w = in_chunk_width(ports, ch);
            let mut out = String::new();
            for v in vecs {
                out.push_str(&pack(ch, ports, v).hex(w));
                out.push('\n');
            }
            out
        })
        .collect()
}

fn pack(chunk: &[usize], ports: &[PortTy], v: &[u128]) -> BitVec {
    let mut bits = BitVec::default();
    let mut off = 0;
    for k in chunk {
        let w = ports[*k].width();
        bits.set(off, w, v[*k]);
        off += w;
    }
    bits
}

/// Testbench'in geri bastığı girişler vektörlerle aynı mı? Değilse
/// sınamanın kendisi bozuktur (girişler DUT'a ulaşmadı).
pub fn check_echo(
    b: &Built,
    ports: &[PortTy],
    vecs: &[Vec<u128>],
    stdout: &str,
) -> Result<(), String> {
    let chunks: Vec<&[usize]> = in_chunks(b, ports);
    let mut seen = 0;
    for line in stdout.lines() {
        let Some(rest) = line.trim().strip_prefix("I ") else {
            continue;
        };
        let mut it = rest.split_whitespace();
        let (Some(i), Some(ci), Some(hex)) = (it.next(), it.next(), it.next()) else {
            return Err(format!("bozuk satır: {line}"));
        };
        let (Ok(i), Ok(ci)) = (i.parse::<usize>(), ci.parse::<usize>()) else {
            return Err(format!("bozuk satır: {line}"));
        };
        let (Some(ch), Some(v)) = (chunks.get(ci), vecs.get(i)) else {
            return Err(format!("bilinmeyen parça: {line}"));
        };
        let want = pack(ch, ports, v).hex(in_chunk_width(ports, ch));
        if !hex.eq_ignore_ascii_case(&want) {
            return Err(format!(
                "vektör {i}, giriş parçası {ci}: testbench {hex}, beklenen {want}"
            ));
        }
        seen += 1;
    }
    if seen != chunks.len() * vecs.len() {
        return Err(format!(
            "{seen} giriş satırı, beklenen {}",
            chunks.len() * vecs.len()
        ));
    }
    Ok(())
}

/// Simülasyon çıktısı: `[vektör][çıkış indeksi]`.
pub fn parse_sim(b: &Built, stdout: &str, nvec: usize) -> Result<Vec<Vec<u128>>, String> {
    let chunks = out_chunks(b);
    let mut vals = vec![vec![None; b.outputs.len()]; nvec];
    let mut done = false;
    for line in stdout.lines() {
        let line = line.trim();
        if line == "DONE" {
            done = true;
            continue;
        }
        let Some(rest) = line.strip_prefix("R ") else {
            continue;
        };
        let mut it = rest.split_whitespace();
        let (Some(i), Some(ci), Some(hex)) = (it.next(), it.next(), it.next()) else {
            return Err(format!("bozuk satır: {line}"));
        };
        let i: usize = i.parse().map_err(|_| format!("bozuk satır: {line}"))?;
        let ci: usize = ci.parse().map_err(|_| format!("bozuk satır: {line}"))?;
        let bits = BitVec::from_hex(hex).ok_or_else(|| format!("bozuk onaltılık: {line}"))?;
        let Some(&(start, end)) = chunks.get(ci) else {
            return Err(format!("bilinmeyen çıkış parçası: {line}"));
        };
        let mut off = 0;
        for (j, o) in b.outputs[start..end].iter().enumerate() {
            let w = o.ty.width();
            vals[i][start + j] = Some(bits.get(off, w));
            off += w;
        }
    }
    if !done {
        return Err("simülasyon DONE basmadı".into());
    }
    vals.into_iter()
        .enumerate()
        .map(|(i, row)| {
            row.into_iter()
                .enumerate()
                .map(|(j, v)| v.ok_or_else(|| format!("vektör {i}, çıkış {j} basılmadı")))
                .collect()
        })
        .collect()
}

/// Üretilen SV'nin satır sahipleri: satırdaki ilk `o<id><harf>`,
/// `t<id><harf>`, `s<id>c` adı; `property ens_N` bloğu N'inci sözleşme.
pub fn sv_owners(sv: &str, ensures: &[usize]) -> Vec<Option<(usize, Variant)>> {
    let mut out = Vec::new();
    let mut prop: Option<usize> = None;
    for line in sv.lines() {
        if let Some(n) = find_ens(line) {
            if line.trim_start().starts_with("property") {
                prop = Some(n);
            }
            out.push(ensures.get(n).map(|c| (*c, Variant::G)));
            if line.contains("assert property") {
                prop = None;
            }
            continue;
        }
        if let Some(n) = prop {
            out.push(ensures.get(n).map(|c| (*c, Variant::G)));
            if line.contains("endproperty") {
                // `assert property (ens_N)` satırı da bu sözleşmenin.
            }
            continue;
        }
        out.push(first_owned_name(line));
    }
    out
}

fn find_ens(line: &str) -> Option<usize> {
    let i = line.find("ens_")?;
    let digits: String = line[i + 4..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

fn first_owned_name(line: &str) -> Option<(usize, Variant)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        let start_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        if start_ok && (c == b'o' || c == b't' || c == b's') {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() {
                let letter = bytes[j] as char;
                let end_ok = j + 1 >= bytes.len()
                    || !(bytes[j + 1].is_ascii_alphanumeric() || bytes[j + 1] == b'_');
                if end_ok {
                    if let (Ok(id), Some(v)) =
                        (line[i + 1..j].parse(), Variant::from_letter(letter))
                    {
                        let ok = match c {
                            b'o' => true,
                            b't' => true,
                            _ => v == Variant::C || v == Variant::R,
                        };
                        if ok {
                            return Some((id, v));
                        }
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// Sözleşme sırası: modüldeki `ensures` satırlarının ifade kimlikleri.
pub fn ensures_order(b: &Built) -> Vec<usize> {
    b.line_owner
        .iter()
        .filter_map(|o| match o {
            Some((id, Variant::G)) => Some(*id),
            _ => None,
        })
        .collect()
}

/// `shape`: portları tipine, literalleri `K`'ye indirger (gruplama için).
pub fn shape(e: &Expr, ports: &[PortTy]) -> String {
    let mut s = String::new();
    shape_into(e, ports, &mut s);
    s
}

fn shape_into(e: &Expr, ports: &[PortTy], s: &mut String) {
    match e {
        Expr::Port(k) => s.push_str(&ports[*k].volt()),
        Expr::Name(_, b) => shape_into(b, ports, s),
        Expr::Lit(..) => s.push('K'),
        Expr::Bin(op, a, b) => {
            s.push('(');
            shape_into(a, ports, s);
            s.push_str(&format!(" {} ", op.sym()));
            shape_into(b, ports, s);
            s.push(')');
        }
        Expr::Un(op, a) => {
            s.push_str(op.sym());
            shape_into(a, ports, s);
        }
        Expr::Index(b, i) => {
            shape_into(b, ports, s);
            s.push('[');
            shape_into(i, ports, s);
            s.push(']');
        }
        Expr::Range(b, ..) => {
            shape_into(b, ports, s);
            s.push_str("[h:l]");
        }
        Expr::Part(b, i, _, up) => {
            shape_into(b, ports, s);
            s.push('[');
            shape_into(i, ports, s);
            s.push_str(if *up { " +: w]" } else { " -: w]" });
        }
        Expr::Cast(a, t) => {
            shape_into(a, ports, s);
            s.push_str(&format!(" as {}", t.volt()));
        }
        Expr::Concat(xs) => {
            s.push_str("concat(");
            for x in xs {
                shape_into(x, ports, s);
                s.push(',');
            }
            s.push(')');
        }
    }
}

/// Toplama: biçim → sayı.
pub type Counts = BTreeMap<String, usize>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::UnOp;

    #[test]
    fn bitvec_hex_roundtrip() {
        let mut b = BitVec::default();
        b.set(0, 8, 0xA5);
        b.set(8, 70, (1u128 << 69) | 3);
        let h = b.hex(78);
        let back = BitVec::from_hex(&h).unwrap();
        assert_eq!(back.get(0, 8), 0xA5);
        assert_eq!(back.get(8, 70), (1u128 << 69) | 3);
    }

    #[test]
    fn module_lines_are_owned() {
        let ports = vec![PortTy::UInt(8), PortTy::UInt(8)];
        let mut rng = Rng::new(1);
        let e = Expr::bin(
            BinOp::Add,
            Expr::bin(BinOp::Add, Expr::Port(0), Expr::Port(1)),
            Expr::un(UnOp::BitNot, Expr::Port(0)),
        );
        let c = Case::new(3, e, &ports, &mut rng);
        assert!(c.is_valid(), "{:?}", c.kind);
        let b = build(&[c], &ports);
        // (p0 + p1) [8,9] ile ~p0 [8,8] 8'de buluşur: doğal u9.
        assert!(b.text.contains("out o3a : u9"), "{}", b.text);
        assert!(b.text.contains("let t3b = p0 + p1 + ~p0"), "{}", b.text);
        assert!(
            b.text.contains("ensures: o3a == p0 + p1 + ~p0"),
            "{}",
            b.text
        );
        for (l, o) in b.text.lines().zip(&b.line_owner) {
            if l.contains("o3") || l.contains("t3b") {
                assert!(o.is_some(), "{l}");
            }
        }
        let tb = testbench(&b, &ports, 4);
        assert!(tb.contains("Dt dut (.*);"));
    }

    #[test]
    fn sv_owner_scan() {
        let sv = "    wire [8:0] t12b = 9'(p0) + 9'(p1);\n    assign o12f = (p1 + p0) == t12b;\n    property ens_1;\n        @(posedge clk) disable iff (rst)\n        o5a == p0;\n    endproperty\n    assert property (ens_1);\n    assign o7c = s7c;\n";
        let own = sv_owners(sv, &[4, 5]);
        assert_eq!(own[0], Some((12, Variant::B)));
        assert_eq!(own[1], Some((12, Variant::F)));
        assert_eq!(own[2], Some((5, Variant::G)));
        assert_eq!(own[4], Some((5, Variant::G)));
        assert_eq!(own[6], Some((5, Variant::G)));
        assert_eq!(own[7], Some((7, Variant::C)));
    }
}

#[cfg(test)]
mod echo_tests {
    use super::*;

    #[test]
    fn echo_check_accepts_the_vectors_and_rejects_a_changed_input() {
        let ports = vec![PortTy::UInt(8), PortTy::Bits(70), PortTy::SInt(3)];
        let mut rng = Rng::new(3);
        let e = Expr::bin(BinOp::Eq, Expr::Port(1), Expr::Port(1));
        let mut cases = vec![Case::new(0, e, &ports, &mut rng)];
        cases.push(Case::new(1, Expr::Port(0), &ports, &mut rng));
        let b = build(&cases, &ports);
        let vecs = vectors(&ports, 2, &mut rng);
        let hex = vec_hex(&b, &ports, &vecs);
        assert_eq!(hex.len(), 1);
        let lines: Vec<&str> = hex[0].lines().collect();
        let good = format!("I 0 0 {}\nI 1 0 {}\n", lines[0], lines[1]);
        assert_eq!(check_echo(&b, &ports, &vecs, &good), Ok(()));
        let bad = format!("I 0 0 {}\nI 1 0 {}\n", lines[0], lines[0]);
        assert!(check_echo(&b, &ports, &vecs, &bad).is_err());
        assert!(check_echo(&b, &ports, &vecs, "I 0 0 00\n").is_err());
    }
}

#[cfg(test)]
mod chunk_tests {
    use super::*;

    #[test]
    fn chunks_stay_under_the_bit_limit() {
        let w = vec![700, 400, 70, 70, 2000, 1, 1];
        let c = chunks_by_width(&w);
        assert_eq!(c, vec![(0, 1), (1, 4), (4, 5), (5, 7)]);
        assert_eq!(chunks_by_width(&[]), vec![]);
    }
}
