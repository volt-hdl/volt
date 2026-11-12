//! W2016'nın susturma kuralı sağlam mı (ADR-0116, #266): rastgele
//! genişlik ve değerlerle özellik testi.
//!
//! İki değerlendirici, derleyiciden bağımsız, bu dosyadaki küçük bir
//! modelle (tipler ADR-0025'ten, type-inference.md §3.3'ten elle yazıldı):
//!
//! - **A, bugünkü kural:** `+ - *` iki işlenenin aralıklarının
//!   kesişiminde yapılır, işlem genişliği `C` kesişimin üst ucu + 1 (`*`
//!   için iki katı); işlenen `C` bite dönüştürülür (doğal genişliği
//!   `C`'den genişse kesilir), sonuç `C` bitte sarar.
//! - **B, kesilmeyen işlenen:** aynı ifade, her `+ - *` `max(C, N_l, N_r)`
//!   bitte (`N` işlenenin doğal genişliği): hiçbir işlenen doğal
//!   genişliğinden aşağı kesilmez. A ile B yalnız bir işlenenin kesildiği
//!   düğümlerde ayrışabilir.
//!
//! Bağlamlar: bildirilmiş genişliğe atama (`out o : uT = e`; ADR-0041'in
//! itmesi dahil) ve bir literalle karşılaştırma (`e < K`).
//!
//! Özellik: derleyici W2016 vermediyse her girdide A ile B aynı çıktıyı
//! verir (susturma sağlam). Ayrıca: model kesme gördüğü hâlde derleyicinin
//! sustuğu durumlar iki bağlamda da gerçekten üretilir (susturma kuralları
//! sınanıyor) ve uyarı çıkan programların en az birinde A ile B ayrışır.

use volt_hir::analyze;
use volt_syntax::{parse, FileId};

/// xorshift64*: bağımlılıksız, tohumlu, belirlenimci.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i128, hi: i128) -> i128 {
        let span = (hi - lo + 1) as u128;
        let r = (u128::from(self.next()) << 64 | u128::from(self.next())) % span;
        lo + r as i128
    }
}

#[derive(Clone, Copy)]
enum Op {
    Add,
    Sub,
    Mul,
}

impl Op {
    fn sym(self) -> &'static str {
        match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
        }
    }
    fn apply(self, a: i128, b: i128) -> i128 {
        match self {
            Op::Add => a + b,
            Op::Sub => a - b,
            Op::Mul => a * b,
        }
    }
    /// İşlem genişliği: kesişimin üst ucu + 1, `*` için iki katı.
    fn width(self, meet_hi: u32) -> u32 {
        match self {
            Op::Mul => 2 * meet_hi,
            _ => meet_hi + 1,
        }
    }
}

enum E {
    Port(usize),
    Lit(i128),
    Bin(Op, Box<E>, Box<E>),
}

/// Modelin tipi: literal ya da `lo..=hi` aralıklı tam sayı.
#[derive(Clone, Copy)]
enum Ty {
    Lit,
    Int { lo: u32, hi: u32 },
}

/// Tipli ifade ağacı: düğümün tipi ve (ikili düğümde) işlem genişliği.
struct Node {
    ty: Ty,
    kind: NodeKind,
}

enum NodeKind {
    Port(usize),
    Lit(i128),
    Bin {
        op: Op,
        width: u32,
        l: Box<Node>,
        r: Box<Node>,
    },
}

struct Prog {
    signed: bool,
    widths: Vec<u32>,
}

impl Prog {
    fn fits(&self, v: i128, w: u32) -> bool {
        if self.signed {
            v >= -(1i128 << (w - 1)) && v < 1i128 << (w - 1)
        } else {
            v >= 0 && v < 1i128 << w
        }
    }

    fn wrap(&self, v: i128, w: u32) -> i128 {
        let m = v.rem_euclid(1i128 << w);
        if self.signed && m >= 1i128 << (w - 1) {
            m - (1i128 << w)
        } else {
            m
        }
    }

    /// ADR-0025 tip kuralları; derleyicinin reddedeceği biçim `None`
    /// (E2001: kesişim boş; E2010: literal sığmıyor).
    fn type_of(&self, e: &E) -> Option<Node> {
        Some(match e {
            E::Port(k) => Node {
                ty: Ty::Int {
                    lo: self.widths[*k],
                    hi: self.widths[*k],
                },
                kind: NodeKind::Port(*k),
            },
            E::Lit(v) => Node {
                ty: Ty::Lit,
                kind: NodeKind::Lit(*v),
            },
            E::Bin(op, l, r) => {
                let l = self.type_of(l)?;
                let r = self.type_of(r)?;
                let (lo, meet_hi) = match (l.ty, r.ty) {
                    (Ty::Lit, Ty::Lit) => return None,
                    // Literal karşı yanın tipine uyarlanır (§4) ve ona sığmalı.
                    (Ty::Lit, Ty::Int { lo, hi }) | (Ty::Int { lo, hi }, Ty::Lit) => {
                        let v = match (&l.kind, &r.kind) {
                            (NodeKind::Lit(v), _) | (_, NodeKind::Lit(v)) => *v,
                            _ => unreachable!(),
                        };
                        if !self.fits(v, hi) {
                            return None;
                        }
                        (lo, hi)
                    }
                    (Ty::Int { lo: a, hi: b }, Ty::Int { lo: c, hi: d }) => {
                        let (lo, hi) = (a.max(c), b.min(d));
                        if lo > hi {
                            return None;
                        }
                        (lo, hi)
                    }
                };
                let width = op.width(meet_hi);
                Node {
                    ty: Ty::Int { lo, hi: width },
                    kind: NodeKind::Bin {
                        op: *op,
                        width,
                        l: Box::new(l),
                        r: Box::new(r),
                    },
                }
            }
        })
    }

    /// Değer: `natural` false ise A (işlenen `C` bite), true ise B
    /// (işlenen doğal genişliğinden aşağı kesilmez).
    fn eval(&self, n: &Node, inp: &[i128], natural: bool) -> i128 {
        match &n.kind {
            NodeKind::Port(k) => inp[*k],
            NodeKind::Lit(v) => *v,
            NodeKind::Bin { op, width, l, r } => {
                let w = if natural {
                    [l.ty, r.ty]
                        .iter()
                        .filter_map(|t| match t {
                            Ty::Int { hi, .. } => Some(*hi),
                            Ty::Lit => None,
                        })
                        .fold(*width, u32::max)
                } else {
                    *width
                };
                self.at(*op, w, l, r, inp, natural)
            }
        }
    }

    /// `op`, işlenenleri `w` bite dönüştürerek `w` bitte.
    fn at(&self, op: Op, w: u32, l: &Node, r: &Node, inp: &[i128], natural: bool) -> i128 {
        let a = self.wrap(self.eval(l, inp, natural), w);
        let b = self.wrap(self.eval(r, inp, natural), w);
        self.wrap(op.apply(a, b), w)
    }

    /// Model bir işlenenin kesildiği düğüm görüyor mu.
    fn has_cut(n: &Node) -> bool {
        match &n.kind {
            NodeKind::Bin { width, l, r, .. } => {
                [l, r]
                    .iter()
                    .any(|c| matches!(c.ty, Ty::Int { hi, .. } if hi > *width))
                    || Self::has_cut(l)
                    || Self::has_cut(r)
            }
            _ => false,
        }
    }

    fn text(e: &E) -> String {
        match e {
            E::Port(k) => format!("p{k}"),
            E::Lit(v) => v.to_string(),
            E::Bin(op, l, r) => format!("({} {} {})", Self::text(l), op.sym(), Self::text(r)),
        }
    }

    fn input(&self, rng: &mut Rng) -> Vec<i128> {
        self.widths
            .iter()
            .map(|&w| {
                let (lo, hi) = if self.signed {
                    (-(1i128 << (w - 1)), (1i128 << (w - 1)) - 1)
                } else {
                    (0, (1i128 << w) - 1)
                };
                // Uçlar sık: kesme en çok büyük değerlerde görünür.
                match rng.below(4) {
                    0 => lo,
                    1 => hi,
                    _ => rng.range(lo, hi),
                }
            })
            .collect()
    }
}

fn gen(rng: &mut Rng, ports: usize, depth: u32) -> E {
    if depth == 0 || rng.below(10) < 3 {
        return if rng.below(100) < 15 {
            E::Lit(rng.below(4) as i128)
        } else {
            E::Port(rng.below(ports as u64) as usize)
        };
    }
    let op = [Op::Add, Op::Sub, Op::Mul, Op::Mul][rng.below(4) as usize];
    let l = gen(rng, ports, depth - 1);
    let mut r = gen(rng, ports, depth - 1);
    if matches!((&l, &r), (E::Lit(_), E::Lit(_))) {
        r = E::Port(rng.below(ports as u64) as usize);
    }
    E::Bin(op, Box::new(l), Box::new(r))
}

/// Derleyicinin W2016 sayısı; hata varsa model ile derleyici ayrışmıştır.
fn w2016(src: &str) -> usize {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "{src}\n{:?}",
        parsed.error_codes()
    );
    let result = analyze(&parsed.ast);
    assert!(
        !result.has_errors(),
        "model kabul etti, derleyici reddetti:\n{src}\n{:?}",
        result.error_codes()
    );
    result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "W2016")
        .count()
}

#[derive(Default, Debug)]
struct Tally {
    programs: usize,
    warned: usize,
    /// Model kesme görüyor, derleyici susuyor.
    suppressed: usize,
    /// Uyarılı ve A ile B'nin ayrıştığı bir girdi bulunan program.
    warned_and_differs: usize,
}

const PROGRAMS: usize = 4000;
const INPUTS: usize = 48;

/// Bağlam: ifadenin sonucu nereye gidiyor.
#[derive(Clone, Copy, PartialEq)]
enum Ctx {
    /// `out o : uT = e`: hedef `T` bit tutar.
    Assign,
    /// `out c : bool = e < K`: değerin tamamı okunur.
    Compare,
    /// `out o : uK = e as uK`: daraltan dönüşüm alt `K` biti, genişleten
    /// değerin tamamını okur.
    Cast,
    /// `out o : uW = e / d` (`d : uW`): bölme `W` bitte; `e` daha genişse
    /// (W2015) yalnız alt `W` biti okunur.
    Div,
}

/// Bir programın kökü.
enum Root {
    /// Hedef genişlik; `widen`: ADR-0041 itmesi (işlem hedef genişlikte).
    Assign {
        tw: u32,
        widen: bool,
    },
    Compare {
        k: i128,
    },
    Cast {
        k: u32,
    },
    /// Bölen girdinin son elemanıdır.
    Div {
        w: u32,
    },
}

/// Bağlamın okuduğu çıktı (A ya da B).
fn observe(prog: &Prog, node: &Node, root: &Root, inp: &[i128], natural: bool) -> i128 {
    match *root {
        Root::Assign { tw, widen: true } => {
            let NodeKind::Bin { op, l, r, .. } = &node.kind else {
                unreachable!("kök ikili işlem")
            };
            prog.at(*op, tw, l, r, inp, natural)
        }
        Root::Assign { tw, widen: false } => prog.wrap(prog.eval(node, inp, natural), tw),
        Root::Compare { k } => i128::from(prog.eval(node, inp, natural) < k),
        Root::Cast { k } => prog.wrap(prog.eval(node, inp, natural), k),
        Root::Div { w } => {
            let x = prog.wrap(prog.eval(node, inp, natural), w);
            let y = prog.wrap(inp[inp.len() - 1], w);
            // Bölmenin SV'deki anlamı burada önemsiz: A ve B aynı işlevi
            // aynı `w` bitlik işlenenlere uygular.
            if y == 0 {
                0
            } else {
                prog.wrap(x / y, w)
            }
        }
    }
}

fn run(seed: u64, ctx: Ctx) -> Tally {
    let mut rng = Rng(seed);
    let mut t = Tally::default();
    while t.programs < PROGRAMS {
        let signed = rng.below(2) == 0;
        let ports = 3;
        let prog = Prog {
            signed,
            widths: (0..ports).map(|_| 1 + rng.below(10) as u32).collect(),
        };
        let depth = 1 + rng.below(3) as u32;
        let e = gen(&mut rng, ports, depth);
        let Some(node) = prog.type_of(&e) else {
            continue;
        };
        let (Ty::Int { lo, hi: c }, NodeKind::Bin { l, r, .. }) = (node.ty, &node.kind) else {
            continue;
        };
        let p = if signed { "i" } else { "u" };
        let text = Prog::text(&e);
        let (root, decl, body) = match ctx {
            Ctx::Assign => {
                // Hedef aralığın içinde ya da itmeyle (ADR-0041) ondan
                // geniş; uN/iN en çok 64 bit.
                let tw = lo + rng.below(u64::from(c + 3 - lo + 1)) as u32;
                let widen = [l.ty, r.ty]
                    .iter()
                    .all(|t| !matches!(t, Ty::Int { hi, .. } if *hi > tw));
                let literal_fits = [&**l, &**r]
                    .iter()
                    .all(|n| !matches!(n.kind, NodeKind::Lit(v) if !prog.fits(v, tw)));
                if tw > 64 || (tw > c && !widen) || (widen && !literal_fits) {
                    continue;
                }
                (
                    Root::Assign { tw, widen },
                    format!("    out o : {p}{tw}\n"),
                    format!("    o = {text}\n"),
                )
            }
            Ctx::Compare => {
                // Literal ifadenin tipine uyarlanır, ona sığmalı.
                let (klo, khi) = if signed {
                    (-(1i128 << (c - 1)), (1i128 << (c - 1)) - 1)
                } else {
                    (0, (1i128 << c) - 1)
                };
                let k = rng.range(klo, khi);
                (
                    Root::Compare { k },
                    "    out q : bool\n".to_string(),
                    format!("    q = {text} < {k}\n"),
                )
            }
            Ctx::Cast => {
                let k = 1 + rng.below(u64::from(c.min(60) + 4)) as u32;
                (
                    Root::Cast { k },
                    format!("    out o : {p}{k}\n"),
                    format!("    o = {text} as {p}{k}\n"),
                )
            }
            Ctx::Div => {
                let w = lo + rng.below(u64::from(c - lo + 1)) as u32;
                if w > 64 {
                    continue;
                }
                (
                    Root::Div { w },
                    format!("    in  d : {p}{w}\n    out o : {p}{w}\n"),
                    format!("    o = {text} / d\n"),
                )
            }
        };
        let ports_src: String = prog
            .widths
            .iter()
            .enumerate()
            .map(|(k, w)| format!("    in  p{k} : {p}{w}\n"))
            .collect();
        let src = format!("module P {{\n{ports_src}{decl}{body}}}\n");
        t.programs += 1;
        let warned = w2016(&src) > 0;
        let cut = Prog::has_cut(&node);
        t.warned += usize::from(warned);
        t.suppressed += usize::from(cut && !warned);
        let mut differs = false;
        for _ in 0..INPUTS {
            let mut inp = prog.input(&mut rng);
            if let Root::Div { w } = root {
                let d = Prog {
                    signed,
                    widths: vec![w],
                };
                inp.extend(d.input(&mut rng));
            }
            let a = observe(&prog, &node, &root, &inp, false);
            let b = observe(&prog, &node, &root, &inp, true);
            if a != b {
                assert!(
                    warned,
                    "W2016 susturuldu ama kesme değeri değiştiriyor:\n{src}girdi {inp:?}: bugünkü kural {a}, kesilmeyen işlenen {b}"
                );
                differs = true;
            }
        }
        t.warned_and_differs += usize::from(warned && differs);
    }
    t
}

fn assert_tally(name: &str, t: &Tally) {
    eprintln!("arith_cut_property_tests [{name}]: {t:?}");
    assert!(t.suppressed > 0, "{name}: susturma hiç sınanmadı: {t:?}");
    assert!(
        t.warned_and_differs > 0,
        "{name}: uyarılı ve ayrışan program yok: {t:?}"
    );
}

/// Atama bağlamı: iki susturma kuralı da (okunan genişlik, sonucun değer
/// aralığı) burada çalışır.
#[test]
fn suppressed_cuts_never_change_an_assigned_value() {
    let t = run(0x5EED_0266_0001, Ctx::Assign);
    assert_tally("assign", &t);
}

/// Karşılaştırma bağlamı: değerin tamamı okunur, yalnız değer aralığı
/// kuralı susturabilir.
#[test]
fn suppressed_cuts_never_change_a_comparison() {
    let t = run(0x5EED_0266_0002, Ctx::Compare);
    assert_tally("compare", &t);
}

/// Dönüşüm bağlamı: daraltan `as` alt bitleri okur (W2016 geri alınır),
/// genişleten `as` değerin tamamını (uyarı kalır).
#[test]
fn suppressed_cuts_never_change_a_cast() {
    let t = run(0x5EED_0266_0003, Ctx::Cast);
    assert_tally("cast", &t);
}

/// Bölme bağlamı: işleneni kesen bölme (W2015) alt bitleri okur, kesmeyen
/// bölme değerin tamamını.
#[test]
fn suppressed_cuts_never_change_a_quotient() {
    let t = run(0x5EED_0266_0004, Ctx::Div);
    assert_tally("div", &t);
}
