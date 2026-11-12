//! Tip yönlü rastgele ifade üreteci. `int(s, w, d)` aralığı `w`'yi içeren
//! (`lo <= w <= hi`) bir tam sayı ifadesi üretir: aynı `w` ile üretilen iki
//! işlenenin aralıkları her zaman kesişir, ama doğal genişlikleri farklı
//! olabilir — ADR-0025'in esnek aralığı ve #106 sınıfı tam orada yaşar.
//! Her düğüm modelle doğrulanır; geçersiz çıkan aday yeniden üretilir.
//! Geçersiz ifadeler (`invalid`) ayrı ve bilerek üretilir.

use crate::ast::{BinOp, Expr, LitFmt, PortTy, UnOp};
use crate::model::{Model, Ty, MAX_BITS, MAX_INT};
use crate::rng::Rng;
use crate::value::mask;

/// Bir toplu koşunun giriş portları. Aynı tipte port çoğu zaman yeniden
/// kullanılır: ifadeler ortak girişleri paylaşır, testbench küçük kalır.
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub ports: Vec<PortTy>,
}

impl Pool {
    pub fn leaf(&mut self, rng: &mut Rng, t: PortTy) -> Expr {
        let same: Vec<usize> = (0..self.ports.len())
            .filter(|i| self.ports[*i] == t)
            .collect();
        if !same.is_empty() && rng.chance(65) {
            Expr::Port(rng.pick(&same))
        } else {
            self.ports.push(t);
            Expr::Port(self.ports.len() - 1)
        }
    }
}

/// Çalışma zamanı `/` ve `%`'nin bileşik işlenenleri (aritmetik ifade,
/// esnek tip) yalnız `VOLT_DIFFTEST_DIV_OPERANDS=1` ile: üretici bölmeyi
/// bağlamın genişliğinde yapıyor, işlenen bölmeden önce kesiliyor ya da
/// sarmıyor (#150; `+ - *` kesmeyle sıra değiştirir, `/` ve `%`
/// değiştirmez). Varsayılan biçimde işlenenler atomdur: port, portun
/// dönüşümü, literal, literal aritmetiği. Issue kapanınca varsayılan açılır.
pub fn div_operands_enabled() -> bool {
    std::env::var("VOLT_DIFFTEST_DIV_OPERANDS").is_ok_and(|v| v == "1")
}

fn floor_log2(n: u32) -> u32 {
    31 - n.leading_zeros()
}

pub struct Gen<'a> {
    pub rng: &'a mut Rng,
    pub pool: &'a mut Pool,
}

impl Gen<'_> {
    fn ty_of(&self, e: &Expr) -> Option<Ty> {
        Model {
            ports: &self.pool.ports,
        }
        .synth(e)
        .ok()
        .map(|t| t.ty)
    }

    fn natural(&self, e: &Expr) -> u32 {
        self.ty_of(e).map_or(1, Ty::width)
    }

    fn leaf(&mut self, t: PortTy) -> Expr {
        self.pool.leaf(self.rng, t)
    }

    /// Genişlik: çoğu dar, bir kısmı 64'e kadar.
    pub fn width(&mut self) -> u32 {
        match self.rng.weighted(&[45, 35, 20]) {
            0 => self.rng.range(1, 16),
            1 => self.rng.range(17, 40),
            _ => self.rng.range(41, MAX_INT),
        }
    }

    fn sub_depth(&mut self, d: u32) -> u32 {
        if d <= 1 || self.rng.chance(60) {
            d.saturating_sub(1)
        } else {
            self.rng.range(0, d - 1)
        }
    }

    /// `w` bitlik (işaretli ya da işaretsiz) aralığa sığan literal.
    pub fn lit(&mut self, signed: bool, w: u32) -> Expr {
        let v: i128 = match self.rng.weighted(&[30, 20, 50]) {
            0 => i128::from(self.rng.range(0, 3)).min(if signed && w == 1 { 0 } else { 3 }),
            1 => {
                if signed {
                    if self.rng.chance(50) {
                        (1i128 << (w - 1)) - 1
                    } else {
                        -(1i128 << (w - 1))
                    }
                } else {
                    mask(w) as i128
                }
            }
            _ => {
                let raw = self.rng.bits(w);
                if signed {
                    crate::value::to_i128(raw, w, true)
                } else {
                    raw as i128
                }
            }
        };
        let v = if signed && w > 1 && v > (1i128 << (w - 1)) - 1 {
            (1i128 << (w - 1)) - 1
        } else {
            v
        };
        let fmt = if v < 0 {
            LitFmt::Dec
        } else {
            match self.rng.weighted(&[60, 25, 15]) {
                0 => LitFmt::Dec,
                1 => LitFmt::Hex,
                _ if v < 1 << 16 => LitFmt::Bin,
                _ => LitFmt::Dec,
            }
        };
        Expr::Lit(v, fmt)
    }

    /// Değeri `w` bitlik (işaretli ya da işaretsiz) aralığa sığan literal
    /// aritmetiği (#144): `/` ve `%` ağırlıklı, yaprakları bilerek
    /// aralıktan geniş. Değer derleme zamanında tam hesaplanır (const-eval.md
    /// §1, §4); yaprakları bağlam genişliğine kesen bir derleyici `/` ve
    /// `%`'de yanlış değer verir.
    pub fn lit_expr(&mut self, s: bool, w: u32, d: u32) -> Expr {
        let Expr::Lit(q, _) = self.lit(s, w) else {
            unreachable!("lit bir literal üretir")
        };
        let d = d.clamp(1, 2);
        self.lit_tree(q, d)
    }

    /// Bölen ya da fark için büyüklük: çoğu küçük, bir kısmı bağlamdan
    /// geniş (en çok 2^12).
    fn lit_magnitude(&mut self) -> i128 {
        let k = self.rng.range(1, 12);
        i128::from(self.rng.range(1, 1 << k))
    }

    /// Değeri tam `q` olan literal ağacı, iç içelik en çok `d`.
    pub fn lit_tree(&mut self, q: i128, d: u32) -> Expr {
        if d == 0 {
            return Expr::Lit(q, LitFmt::Dec);
        }
        let (op, a, b) = match self.rng.weighted(&[35, 30, 13, 12, 10]) {
            0 => {
                // a / b == q: a = q·b + r, |r| < |b|, r bölünenin işaretinde.
                let mut b = self.lit_magnitude();
                if q < 0 && self.rng.chance(50) {
                    b = -b;
                }
                let sign = if q < 0 { -1 } else { 1 };
                let r = i128::from(self.rng.below(b.unsigned_abs() as u64)) * sign;
                let a = q * b + r;
                let a = if b != 0 && a / b == q { a } else { q * b };
                (BinOp::Div, a, b)
            }
            1 => {
                // a % b == q: |b| > |q|, a = ±m·|b| + q (bölünenin işareti q'nunki).
                let bb = q.abs() + self.lit_magnitude();
                let m = i128::from(self.rng.range(0, 64));
                let a = if q < 0 { q - m * bb } else { q + m * bb };
                let b = if self.rng.chance(30) { -bb } else { bb };
                (BinOp::Rem, a, b)
            }
            2 => {
                let b = self.lit_magnitude() << self.rng.range(0, 40);
                (BinOp::Sub, q + b, b)
            }
            3 => {
                let b = self.lit_magnitude();
                (BinOp::Add, q - b, b)
            }
            _ => {
                let b = if q % 3 == 0 { 3 } else { 1 };
                (BinOp::Mul, q / b, b)
            }
        };
        let sub = d - 1;
        let a = if sub > 0 && self.rng.chance(30) {
            self.lit_tree(a, sub)
        } else {
            Expr::Lit(a, LitFmt::Dec)
        };
        let b = if sub > 0 && self.rng.chance(20) {
            self.lit_tree(b, sub)
        } else {
            Expr::Lit(b, LitFmt::Dec)
        };
        Expr::bin(op, a, b)
    }

    fn contains(t: Option<Ty>, s: bool, w: u32) -> bool {
        matches!(t, Some(Ty::Int { signed, lo, hi }) if signed == s && lo <= w && w <= hi)
    }

    /// Aralığı `w`'yi içeren tam sayı ifadesi, iç içelik en çok `d`.
    pub fn int(&mut self, s: bool, w: u32, d: u32) -> Expr {
        if d == 0 {
            return self.int_leaf(s, w);
        }
        for _ in 0..12 {
            let e = self.int_node(s, w, d);
            if Self::contains(self.ty_of(&e), s, w) {
                return e;
            }
        }
        self.int_leaf(s, w)
    }

    /// Doğal genişliği tam `w` olan tam sayı ifadesi.
    pub fn int_exact(&mut self, s: bool, w: u32, d: u32) -> Expr {
        let e = self.int(s, w, d);
        if self.natural(&e) == w {
            e
        } else {
            Expr::cast(e, PortTy::int(s, w))
        }
    }

    fn int_leaf(&mut self, s: bool, w: u32) -> Expr {
        if self.rng.chance(8) {
            let sw = self.width();
            let ss = self.rng.chance(50);
            let src = self.leaf(PortTy::int(ss, sw));
            Expr::cast(src, PortTy::int(s, w))
        } else {
            self.leaf(PortTy::int(s, w))
        }
    }

    fn operand_pair(&mut self, s: bool, w: u32, d: u32) -> (Expr, Expr) {
        let x = self.int(s, w, d - 1);
        let sd = self.sub_depth(d);
        let y = match self.rng.weighted(&[22, 10, 68]) {
            0 => self.lit(s, w),
            1 => self.lit_expr(s, w, sd),
            _ => self.int(s, w, sd),
        };
        if self.rng.chance(15) {
            (y, x)
        } else {
            (x, y)
        }
    }

    /// Bölmenin atom işlenenleri (`div_operands_enabled` kapalıyken).
    fn div_operands(&mut self, s: bool, w: u32, d: u32) -> (Expr, Expr) {
        let x = self.int_leaf(s, w);
        let sd = self.sub_depth(d);
        let y = match self.rng.weighted(&[30, 15, 55]) {
            0 => self.lit(s, w),
            1 => self.lit_expr(s, w, sd),
            _ => self.int_leaf(s, w),
        };
        if self.rng.chance(15) {
            (y, x)
        } else {
            (x, y)
        }
    }

    fn shift_amount(&mut self, w: u32, d: u32) -> Expr {
        match self.rng.weighted(&[40, 40, 20]) {
            0 => Expr::Lit(i128::from(self.rng.range(0, w + 1)), LitFmt::Dec),
            1 => {
                let aw = self.rng.range(1, 7);
                self.leaf(PortTy::UInt(aw))
            }
            _ => {
                let aw = self.rng.range(1, 6);
                let sd = self.sub_depth(d);
                self.int(false, aw, sd)
            }
        }
    }

    fn int_node(&mut self, s: bool, w: u32, d: u32) -> Expr {
        match self.rng.weighted(&[30, 14, 6, 6, 12, 14, 18]) {
            0 => {
                let ops: &[BinOp] = if 2 * w <= MAX_INT {
                    &[BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div, BinOp::Rem]
                } else {
                    &[BinOp::Add, BinOp::Sub, BinOp::Div, BinOp::Rem]
                };
                let op = self.rng.pick(ops);
                let (x, y) = if op.is_div() && !div_operands_enabled() {
                    self.div_operands(s, w, d)
                } else {
                    self.operand_pair(s, w, d)
                };
                Expr::bin(op, x, y)
            }
            1 => {
                let op = self.rng.pick(&[BinOp::And, BinOp::Or, BinOp::Xor]);
                let (x, y) = self.operand_pair(s, w, d);
                Expr::bin(op, x, y)
            }
            2 => Expr::un(UnOp::BitNot, self.int(s, w, d - 1)),
            3 if s && w >= 2 => Expr::un(UnOp::Neg, self.int_exact(true, w - 1, d - 1)),
            3 => Expr::un(UnOp::BitNot, self.int(s, w, d - 1)),
            4 => {
                let op = self.rng.pick(&[BinOp::Shl, BinOp::Shr]);
                let x = self.int(s, w, d - 1);
                let amt = self.shift_amount(w, d);
                Expr::bin(op, x, amt)
            }
            5 => {
                let t = PortTy::int(s, w);
                match self.rng.weighted(&[55, 25, 20]) {
                    0 => {
                        let sw = self.width();
                        let ss = self.rng.chance(50);
                        Expr::cast(self.int(ss, sw, d - 1), t)
                    }
                    1 => Expr::cast(self.bits(w, d - 1), t),
                    _ if w == 1 && !s => Expr::cast(self.boolean(d - 1), t),
                    _ => Expr::cast(self.int(!s, w, d - 1), t),
                }
            }
            _ => {
                let b = self.bits_select(w, d);
                Expr::cast(b, PortTy::int(s, w))
            }
        }
    }

    /// Tam `bits<w>` ifadesi.
    pub fn bits(&mut self, w: u32, d: u32) -> Expr {
        if d == 0 {
            return if self.rng.chance(75) {
                self.leaf(PortTy::Bits(w))
            } else {
                self.bits_select(w, 0)
            };
        }
        for _ in 0..12 {
            let e = self.bits_node(w, d);
            if self.ty_of(&e) == Some(Ty::Bits(w)) {
                return e;
            }
        }
        self.leaf(PortTy::Bits(w))
    }

    fn bits_node(&mut self, w: u32, d: u32) -> Expr {
        match self.rng.weighted(&[22, 8, 8, 40, 22]) {
            0 => {
                let op = self.rng.pick(&[BinOp::And, BinOp::Or, BinOp::Xor]);
                let x = self.bits(w, d - 1);
                let sd = self.sub_depth(d);
                let y = self.bits(w, sd);
                Expr::bin(op, x, y)
            }
            1 => Expr::un(UnOp::BitNot, self.bits(w, d - 1)),
            2 => {
                let op = self.rng.pick(&[BinOp::Shl, BinOp::Shr]);
                let x = self.bits(w, d - 1);
                let amt = self.shift_amount(w, d);
                Expr::bin(op, x, amt)
            }
            4 if w <= MAX_INT => Expr::cast(self.int_exact(false, w, d - 1), PortTy::Bits(w)),
            _ => self.bits_select(w, d),
        }
    }

    /// `w` bitlik seçim: `x[h:l]`, `x[i +: w]`, `x[i -: w]`; taban ad ya da
    /// bileşik değer (ADR-0106 §4, #112), başlangıç sabit ya da değişken.
    fn bits_select(&mut self, w: u32, d: u32) -> Expr {
        let int_base = w <= MAX_INT && self.rng.chance(70);
        let cap = if int_base { MAX_INT } else { MAX_BITS };
        let bw = self.rng.range(w, cap.min(w + 20));
        let bd = d.saturating_sub(1);
        let base = if int_base {
            let s = self.rng.chance(40);
            self.int(s, bw, bd)
        } else {
            self.bits(bw, bd)
        };
        let nat = self.natural(&base).max(w);
        match self.rng.weighted(&[50, 30, 20]) {
            0 => {
                let l = self.rng.range(0, nat - w);
                Expr::Range(Box::new(base), l + w - 1, l)
            }
            1 => {
                let up = self.rng.chance(60);
                let start = if self.rng.chance(50) {
                    let v = if up {
                        self.rng.range(0, nat - w)
                    } else {
                        self.rng.range(w - 1, nat - 1)
                    };
                    Expr::Lit(i128::from(v), LitFmt::Dec)
                } else {
                    // Değişken başlangıç: yukarı seçimde çoğu değer sınır içinde.
                    let span = if up { nat - w + 1 } else { nat };
                    let sw = floor_log2(span.max(2)).max(1);
                    let sw = if self.rng.chance(20) { sw + 1 } else { sw };
                    self.leaf(PortTy::UInt(sw.min(8)))
                };
                Expr::Part(Box::new(base), Box::new(start), w, up)
            }
            _ => {
                let up = self.rng.chance(70);
                let sw = self.rng.range(1, 5);
                let start = self.int(false, sw, 1);
                Expr::Part(Box::new(base), Box::new(start), w, up)
            }
        }
    }

    /// `bool` ifadesi.
    pub fn boolean(&mut self, d: u32) -> Expr {
        if d == 0 {
            return if self.rng.chance(60) {
                self.leaf(PortTy::Bool)
            } else {
                let w = self.rng.range(1, 32);
                let s = self.rng.chance(40);
                let base = self.leaf(PortTy::int(s, w));
                let i = self.rng.range(0, w - 1);
                Expr::Index(
                    Box::new(base),
                    Box::new(Expr::Lit(i128::from(i), LitFmt::Dec)),
                )
            };
        }
        for _ in 0..12 {
            let e = self.bool_node(d);
            if self.ty_of(&e) == Some(Ty::Bool) {
                return e;
            }
        }
        self.leaf(PortTy::Bool)
    }

    fn bool_node(&mut self, d: u32) -> Expr {
        match self.rng.weighted(&[42, 8, 16, 14, 8, 3, 4, 5]) {
            0 => {
                let s = self.rng.chance(40);
                let w = self.width();
                let x = self.int(s, w, d - 1);
                let nat = self.natural(&x);
                let op = self.rng.pick(&BinOp::CMP);
                let sd = self.sub_depth(d);
                let y = match self.rng.weighted(&[25, 10, 65]) {
                    0 => self.lit(s, nat),
                    1 => self.lit_expr(s, nat, sd),
                    _ => self.int_exact(s, nat, sd),
                };
                if self.rng.chance(20) {
                    Expr::bin(op, y, x)
                } else {
                    Expr::bin(op, x, y)
                }
            }
            1 => {
                let w = self.rng.range(1, MAX_BITS);
                let x = self.bits(w, d - 1);
                let sd = self.sub_depth(d);
                let y = self.bits(w, sd);
                let op = self.rng.pick(&[BinOp::Eq, BinOp::Ne]);
                Expr::bin(op, x, y)
            }
            2 => {
                let int_base = self.rng.chance(75);
                let base = if int_base {
                    let s = self.rng.chance(40);
                    let w = self.width();
                    self.int(s, w, d - 1)
                } else {
                    let w = self.rng.range(1, MAX_BITS);
                    self.bits(w, d - 1)
                };
                let nat = self.natural(&base);
                let idx = if nat == 1 || self.rng.chance(50) {
                    Expr::Lit(i128::from(self.rng.range(0, nat - 1)), LitFmt::Dec)
                } else {
                    let iw = floor_log2(nat).max(1);
                    let iw = if self.rng.chance(20) { iw + 1 } else { iw };
                    self.leaf(PortTy::UInt(iw.min(8)))
                };
                Expr::Index(Box::new(base), Box::new(idx))
            }
            3 => {
                let op = self.rng.pick(&[BinOp::And, BinOp::Or, BinOp::Xor]);
                let x = self.boolean(d - 1);
                let sd = self.sub_depth(d);
                let y = self.boolean(sd);
                Expr::bin(op, x, y)
            }
            4 => Expr::un(UnOp::Not, self.boolean(d - 1)),
            5 => Expr::un(UnOp::BitNot, self.boolean(d - 1)),
            6 => Expr::cast(self.int_exact(false, 1, d - 1), PortTy::Bool),
            _ => {
                let op = self.rng.pick(&[BinOp::Eq, BinOp::Ne]);
                let x = self.boolean(d - 1);
                let sd = self.sub_depth(d);
                let y = self.boolean(sd);
                Expr::bin(op, x, y)
            }
        }
    }

    /// Geçerli olması amaçlanan üst düzey ifade: iç içelik 1–5.
    pub fn expr(&mut self) -> Expr {
        let d = self.rng.range(1, 5);
        match self.rng.weighted(&[55, 15, 30, 6]) {
            0 => {
                let s = self.rng.chance(40);
                let w = self.width();
                self.int(s, w, d)
            }
            1 => {
                let w = self.rng.range(1, MAX_BITS);
                self.bits(w, d)
            }
            2 => self.boolean(d),
            _ => self.wide_root(d),
        }
    }

    /// 128 bite kadar geniş değer (#207): ifadenin kendisi, iki geniş
    /// değerin `==`/`!=`'i ya da 64 bite kadar bir dilimi. Genişlik çoğu
    /// zaman 2^127'yi taşıyan 128, bazen 65–127.
    fn wide_root(&mut self, d: u32) -> Expr {
        let w = if self.rng.chance(60) {
            128
        } else {
            self.rng.range(MAX_INT + 1, 127)
        };
        let x = self.wide(w, d);
        match self.rng.weighted(&[40, 35, 25]) {
            0 => x,
            1 => {
                let sd = self.sub_depth(d);
                let y = self.wide(w, sd);
                let op = self.rng.pick(&[BinOp::Eq, BinOp::Ne]);
                Expr::bin(op, x, y)
            }
            _ => {
                let h = self.rng.range(0, w - 1);
                let l = self.rng.range(h.saturating_sub(MAX_INT - 1), h);
                Expr::Range(Box::new(x), h, l)
            }
        }
    }

    /// `bits<w>` (64 < w <= 128): giriş portu, geniş literalin dönüşümü
    /// ve bit işlemleri. Seçim yok: `bits_select`'in tabanı `MAX_BITS`'le
    /// sınırlıdır.
    pub fn wide(&mut self, w: u32, d: u32) -> Expr {
        if d == 0 {
            return if self.rng.chance(45) {
                self.leaf(PortTy::Bits(w))
            } else {
                self.wide_lit(w)
            };
        }
        match self.rng.weighted(&[45, 10, 45]) {
            0 => {
                let op = self.rng.pick(&[BinOp::And, BinOp::Or, BinOp::Xor]);
                let x = self.wide(w, d - 1);
                let sd = self.sub_depth(d);
                let y = self.wide(w, sd);
                Expr::bin(op, x, y)
            }
            1 => Expr::un(UnOp::BitNot, self.wide(w, d - 1)),
            _ => self.wide(w, 0),
        }
    }

    /// `K as bits<w>`, K < 2^w (A14): çoğu zaman sınır değeri (2^127,
    /// 2^127 + 1, 2^128 - 1, 2^(w-1), 2^w - 1), bazen rastgele.
    fn wide_lit(&mut self, w: u32) -> Expr {
        let top = mask(w);
        let edges = [
            1u128 << 127,
            (1u128 << 127) + 1,
            u128::MAX,
            1u128 << (w - 1),
            top,
            top - 1,
        ];
        let v = match self.rng.weighted(&[70, 30]) {
            0 => {
                let fitting: Vec<u128> = edges.into_iter().filter(|&v| v <= top).collect();
                fitting[self.rng.below(fitting.len() as u64) as usize]
            }
            _ => self.rng.bits(w),
        };
        let fmt = self.rng.pick(&[LitFmt::Dec, LitFmt::Hex, LitFmt::Bin]);
        Expr::cast(Expr::WideLit(v, fmt), PortTy::Bits(w))
    }

    /// Spesifikasyona göre hatalı olması amaçlanan ifade.
    pub fn invalid(&mut self) -> Expr {
        let d = self.rng.range(1, 3);
        let w = self.rng.range(2, 32);
        let s = self.rng.chance(40);
        match self
            .rng
            .weighted(&[14, 14, 10, 14, 10, 8, 10, 10, 10, 8, 5, 6])
        {
            0 => {
                let op = self
                    .rng
                    .pick(&[BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Lt, BinOp::Eq]);
                Expr::bin(op, self.int_exact(false, w, d), self.int_exact(true, w, d))
            }
            1 => {
                let w2 = w + self.rng.range(2, 6);
                let op = self
                    .rng
                    .pick(&[BinOp::Add, BinOp::Sub, BinOp::And, BinOp::Xor]);
                Expr::bin(op, self.leaf(PortTy::int(s, w)), self.int_exact(s, w2, d))
            }
            2 => {
                let op = self.rng.pick(&[BinOp::Add, BinOp::Sub, BinOp::Mul]);
                Expr::bin(op, self.bits(w, d), self.bits(w, d))
            }
            3 => {
                let x = self.int_exact(s, w, d);
                let y = Expr::bin(
                    BinOp::Add,
                    self.leaf(PortTy::int(s, w)),
                    self.leaf(PortTy::int(s, w)),
                );
                let op = self.rng.pick(&BinOp::CMP);
                Expr::bin(op, x, y)
            }
            4 => {
                let x = self.int(s, w, d);
                let nat = self.natural(&x);
                let i = nat + self.rng.range(0, 3);
                Expr::Index(Box::new(x), Box::new(Expr::Lit(i128::from(i), LitFmt::Dec)))
            }
            5 => {
                let x = self.int(s, w, d);
                let nat = self.natural(&x);
                let l = self.rng.range(1, nat - 1);
                let h = self.rng.range(0, l - 1);
                Expr::Range(Box::new(x), h, l)
            }
            6 => {
                let t = if self.rng.chance(50) {
                    PortTy::UInt(w + 1)
                } else {
                    PortTy::Bits(w + 1)
                };
                Expr::cast(self.bits(w, d), t)
            }
            7 => Expr::un(UnOp::Neg, self.int(false, w, d)),
            // #144: değeri karşı yanın tipine sığmayan literal aritmetiği
            // (E2010); aritmetik, bit işlemi ve karşılaştırma.
            9 => {
                let x = self.int_exact(s, w, d);
                let v = if s && self.rng.chance(50) {
                    -(1i128 << (w - 1)) - 1
                } else if s {
                    1i128 << (w - 1)
                } else {
                    1i128 << w
                };
                let op = self
                    .rng
                    .pick(&[BinOp::Add, BinOp::Sub, BinOp::And, BinOp::Eq, BinOp::Lt]);
                let y = self.lit_tree(v, 1);
                Expr::bin(op, x, y)
            }
            // #213: işaretli işlenenin yanında çıplak negatif literal,
            // karşı yanın aralığının bir altı (E2010); aritmetik, bit
            // işlemi ve karşılaştırma.
            11 => {
                let x = self.int_exact(true, w, d);
                let v = -(1i128 << (w - 1)) - 1;
                let op = self
                    .rng
                    .pick(&[BinOp::Add, BinOp::Sub, BinOp::And, BinOp::Eq, BinOp::Lt]);
                Expr::bin(op, x, Expr::Lit(v, LitFmt::Dec))
            }
            // #144: literal aritmetiğinde sıfıra bölme ya da sıfıra göre
            // kalan (E2023); bölen literal aritmetiğinin değeri 0.
            10 => {
                let x = self.int_exact(s, w, d);
                let z = self.lit_magnitude();
                let zero = Expr::bin(
                    BinOp::Sub,
                    Expr::Lit(z, LitFmt::Dec),
                    Expr::Lit(z, LitFmt::Dec),
                );
                let op = self.rng.pick(&[BinOp::Div, BinOp::Rem]);
                let k = Expr::Lit(self.lit_magnitude(), LitFmt::Dec);
                Expr::bin(BinOp::Add, x, Expr::bin(op, k, zero))
            }
            _ => {
                let x = self.int_exact(false, w, d);
                let v = 1i128 << w;
                Expr::bin(BinOp::Eq, x, Expr::Lit(v, LitFmt::Dec))
            }
        }
    }

    /// `concat(...)`: belgelenmiş E0003 sınırı (ADR-0072).
    pub fn concat(&mut self) -> Expr {
        let a = self.rng.range(1, 16);
        let b = self.rng.range(1, 16);
        Expr::Concat(vec![self.int(false, a, 1), self.int(false, b, 1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;

    #[test]
    fn generated_expressions_are_mostly_valid_and_deep_enough() {
        let mut rng = Rng::new(42);
        let mut pool = Pool::default();
        let mut valid = 0;
        let mut deep = 0;
        for _ in 0..500 {
            let e = Gen {
                rng: &mut rng,
                pool: &mut pool,
            }
            .expr();
            if (Model { ports: &pool.ports }).synth(&e).is_ok() {
                valid += 1;
            }
            if e.depth() >= 3 {
                deep += 1;
            }
        }
        assert!(valid > 450, "{valid}/500 geçerli");
        assert!(deep > 100, "{deep}/500 derin");
    }

    /// #207: geniş kök (128 bite kadar) modelce geçerlidir ve köklerin
    /// çoğu 2^127 ya da üstünde bir literal taşır; i128'e sığmayan değer
    /// üreticinin doyurduğu yoldur.
    #[test]
    fn wide_roots_are_valid_and_carry_literals_from_two_pow_127() {
        fn has_big(e: &Expr) -> bool {
            matches!(e, Expr::WideLit(v, _) if *v >= 1u128 << 127)
                || e.children().into_iter().any(has_big)
        }
        let mut rng = Rng::new(207);
        let mut pool = Pool::default();
        let mut big = 0;
        for _ in 0..200 {
            let e = Gen {
                rng: &mut rng,
                pool: &mut pool,
            }
            .wide_root(3);
            let t = (Model { ports: &pool.ports }).synth(&e);
            assert!(t.is_ok(), "{e:?}: {t:?}");
            if has_big(&e) {
                big += 1;
            }
        }
        assert!(big > 80, "{big}/200 kökte 2^127 ya da üstü");
    }

    /// #144: literal aritmetiği ağacının değeri istenen aralıkta; ağaçların
    /// çoğunda bir yaprak aralıktan geniş (kesen derleyici yanılır).
    #[test]
    fn literal_trees_fit_their_range_with_wider_leaves() {
        let mut rng = Rng::new(144);
        let mut pool = Pool::default();
        let mut wider = 0;
        for i in 0..400 {
            let s = i % 3 == 0;
            let w = 1 + (i % 12) as u32;
            let mut g = Gen {
                rng: &mut rng,
                pool: &mut pool,
            };
            let e = g.lit_expr(s, w, 2);
            let Ok(t) = (Model { ports: &pool.ports }).synth(&e) else {
                panic!("geçersiz literal ağacı: {e:?}")
            };
            let Ty::Lit(v) = t.ty else {
                panic!("literal değil: {e:?}")
            };
            assert!(crate::value::fits(v, w, s), "{v} {s} {w}: {e:?}");
            let mut leaves = vec![];
            collect_lits(&e, &mut leaves);
            if leaves.iter().any(|&l| !crate::value::fits(l, w, s)) {
                wider += 1;
            }
        }
        assert!(wider > 200, "{wider}/400 geniş yapraklı");
    }

    fn collect_lits(e: &Expr, out: &mut Vec<i128>) {
        match e {
            Expr::Lit(v, _) => out.push(*v),
            Expr::Bin(_, a, b) => {
                collect_lits(a, out);
                collect_lits(b, out);
            }
            _ => {}
        }
    }

    #[test]
    fn invalid_expressions_are_mostly_invalid() {
        let mut rng = Rng::new(9);
        let mut pool = Pool::default();
        let mut invalid = 0;
        for _ in 0..300 {
            let e = Gen {
                rng: &mut rng,
                pool: &mut pool,
            }
            .invalid();
            if (Model { ports: &pool.ports }).synth(&e).is_err() {
                invalid += 1;
            }
        }
        assert!(invalid > 240, "{invalid}/300 geçersiz");
    }
}
