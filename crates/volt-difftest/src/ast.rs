//! Üretecin ifade ağacı. Derleyicinin AST'sinden bağımsızdır: yalnız
//! sınanan işlemleri taşır ve kaynak metne yazıcıyla (`print`) iner.

/// Giriş portlarının, çıkışların ve dönüşüm hedeflerinin tipi.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PortTy {
    Bool,
    UInt(u32),
    SInt(u32),
    Bits(u32),
}

impl PortTy {
    pub fn width(self) -> u32 {
        match self {
            PortTy::Bool => 1,
            PortTy::UInt(w) | PortTy::SInt(w) | PortTy::Bits(w) => w,
        }
    }

    pub fn signed(self) -> bool {
        matches!(self, PortTy::SInt(_))
    }

    pub fn is_int(self) -> bool {
        matches!(self, PortTy::UInt(_) | PortTy::SInt(_))
    }

    /// Volt yazımı: `u8`, `i5`, `bits<70>`, `bool`.
    pub fn volt(self) -> String {
        match self {
            PortTy::Bool => "bool".into(),
            PortTy::UInt(w) => format!("u{w}"),
            PortTy::SInt(w) => format!("i{w}"),
            PortTy::Bits(w) => format!("bits<{w}>"),
        }
    }

    /// Testbench bildirimi: `logic signed [4:0]`.
    pub fn sv(self) -> String {
        match self {
            PortTy::Bool => "logic".into(),
            PortTy::SInt(w) => format!("logic signed [{}:0]", w - 1),
            PortTy::UInt(w) | PortTy::Bits(w) => format!("logic [{}:0]", w - 1),
        }
    }

    pub fn int(signed: bool, w: u32) -> PortTy {
        if signed {
            PortTy::SInt(w)
        } else {
            PortTy::UInt(w)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn sym(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::And => "&",
            BinOp::Or => "|",
            BinOp::Xor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }

    /// `docs/spec/operator-precedence.md` §1.
    pub fn prec(self) -> u8 {
        match self {
            BinOp::Eq | BinOp::Ne => 3,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 4,
            BinOp::Or => 5,
            BinOp::Xor => 6,
            BinOp::And => 7,
            BinOp::Shl | BinOp::Shr => 8,
            BinOp::Add | BinOp::Sub => 9,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 10,
        }
    }

    pub fn is_arith(self) -> bool {
        matches!(
            self,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem
        )
    }

    /// `/` ya da `%`: genişlemez (type-inference.md §3.3).
    pub fn is_div(self) -> bool {
        matches!(self, BinOp::Div | BinOp::Rem)
    }

    pub fn is_bitwise(self) -> bool {
        matches!(self, BinOp::And | BinOp::Or | BinOp::Xor)
    }

    pub fn is_shift(self) -> bool {
        matches!(self, BinOp::Shl | BinOp::Shr)
    }

    pub fn is_cmp(self) -> bool {
        matches!(
            self,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
        )
    }

    /// Yer değiştirme biçimi: değişmeli işlem kendisi, sıralama tersi.
    pub fn swapped(self) -> Option<BinOp> {
        match self {
            BinOp::Add | BinOp::Mul | BinOp::And | BinOp::Or | BinOp::Xor => Some(self),
            BinOp::Eq | BinOp::Ne => Some(self),
            BinOp::Lt => Some(BinOp::Gt),
            BinOp::Gt => Some(BinOp::Lt),
            BinOp::Le => Some(BinOp::Ge),
            BinOp::Ge => Some(BinOp::Le),
            BinOp::Sub | BinOp::Div | BinOp::Rem | BinOp::Shl | BinOp::Shr => None,
        }
    }

    pub const CMP: [BinOp; 6] = [
        BinOp::Eq,
        BinOp::Ne,
        BinOp::Lt,
        BinOp::Le,
        BinOp::Gt,
        BinOp::Ge,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    /// `!`
    Not,
    /// `~`
    BitNot,
    /// `-`
    Neg,
}

impl UnOp {
    pub fn sym(self) -> &'static str {
        match self {
            UnOp::Not => "!",
            UnOp::BitNot => "~",
            UnOp::Neg => "-",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LitFmt {
    Dec,
    Hex,
    Bin,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// Giriş portu (havuzdaki indeks).
    Port(usize),
    /// Tipsiz `let` ile bağlanmış alt ifade (let'e çıkarma biçimi).
    Name(String, Box<Expr>),
    /// Soneksiz tam sayı literali.
    Lit(i128, LitFmt),
    /// Soneksiz, negatif olmayan geniş literal (2^128 - 1'e kadar, #207):
    /// yalnız `bits<N>`'e dönüşümün işleneni olarak üretilir
    /// (`WideLit as bits<N>`, değer N bite sığar; A14).
    WideLit(u128, LitFmt),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Un(UnOp, Box<Expr>),
    /// `x[i]`
    Index(Box<Expr>, Box<Expr>),
    /// `x[hi:lo]`
    Range(Box<Expr>, u32, u32),
    /// `x[i +: w]` (`up`) / `x[i -: w]`
    Part(Box<Expr>, Box<Expr>, u32, bool),
    /// `x as T`
    Cast(Box<Expr>, PortTy),
    /// `concat(a, b, ...)`
    Concat(Vec<Expr>),
}

impl Expr {
    pub fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
        Expr::Bin(op, Box::new(a), Box::new(b))
    }

    pub fn un(op: UnOp, a: Expr) -> Expr {
        Expr::Un(op, Box::new(a))
    }

    pub fn cast(a: Expr, t: PortTy) -> Expr {
        Expr::Cast(Box::new(a), t)
    }

    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Port(_) | Expr::Lit(..) | Expr::WideLit(..) | Expr::Name(..) => vec![],
            Expr::Bin(_, a, b) | Expr::Index(a, b) | Expr::Part(a, b, ..) => vec![a, b],
            Expr::Un(_, a) | Expr::Range(a, ..) | Expr::Cast(a, _) => vec![a],
            Expr::Concat(xs) => xs.iter().collect(),
        }
    }

    fn children_mut(&mut self) -> Vec<&mut Expr> {
        match self {
            Expr::Port(_) | Expr::Lit(..) | Expr::WideLit(..) | Expr::Name(..) => vec![],
            Expr::Bin(_, a, b) | Expr::Index(a, b) | Expr::Part(a, b, ..) => vec![a, b],
            Expr::Un(_, a) | Expr::Range(a, ..) | Expr::Cast(a, _) => vec![a],
            Expr::Concat(xs) => xs.iter_mut().collect(),
        }
    }

    /// Düğüm sayısı (`Name` bağladığı ifadeyle birlikte tek düğüm).
    pub fn size(&self) -> usize {
        1 + self.children().iter().map(|c| c.size()).sum::<usize>()
    }

    /// İşlem iç içeliği: yaprak 0.
    pub fn depth(&self) -> usize {
        self.children()
            .iter()
            .map(|c| 1 + c.depth())
            .max()
            .unwrap_or(0)
    }

    /// Bütün düğümlerin yolları (kök `[]`), ön sırada.
    pub fn paths(&self) -> Vec<Vec<usize>> {
        let mut out = vec![vec![]];
        for (i, c) in self.children().iter().enumerate() {
            for mut p in c.paths() {
                p.insert(0, i);
                out.push(p);
            }
        }
        out
    }

    pub fn at(&self, path: &[usize]) -> &Expr {
        match path.split_first() {
            None => self,
            Some((i, rest)) => self.children()[*i].at(rest),
        }
    }

    pub fn replaced(&self, path: &[usize], new: Expr) -> Expr {
        let mut e = self.clone();
        let mut cur = &mut e;
        for i in path {
            cur = cur.children_mut().into_iter().nth(*i).expect("geçerli yol");
        }
        *cur = new;
        e
    }

    /// Kullanılan portlar (`Name`'in bağladığı ifadedekiler dahil).
    pub fn ports(&self, out: &mut Vec<usize>) {
        match self {
            Expr::Port(k) => out.push(*k),
            Expr::Name(_, b) => b.ports(out),
            _ => {
                for c in self.children() {
                    c.ports(out);
                }
            }
        }
    }

    /// Port indekslerini `offset` kadar kaydırır (regresyon kümesi tek
    /// havuzda birleşir).
    pub fn shift_ports(&self, offset: usize) -> Expr {
        match self {
            Expr::Port(k) => Expr::Port(k + offset),
            Expr::Name(n, b) => Expr::Name(n.clone(), Box::new(b.shift_ports(offset))),
            _ => {
                let mut e = self.clone();
                for c in e.children_mut() {
                    *c = c.shift_ports(offset);
                }
                e
            }
        }
    }

    /// Yer değiştirme biçimi: her değişmeli işlemin işlenenleri yer
    /// değiştirir, sıralama karşılaştırması tersine döner.
    pub fn commuted(&self) -> Expr {
        match self {
            Expr::Bin(op, a, b) => match op.swapped() {
                Some(s) => Expr::bin(s, b.commuted(), a.commuted()),
                None => Expr::bin(*op, a.commuted(), b.commuted()),
            },
            Expr::Name(n, b) => Expr::Name(n.clone(), b.clone()),
            _ => {
                let mut e = self.clone();
                for c in e.children_mut() {
                    *c = c.commuted();
                }
                e
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_replace() {
        let e = Expr::bin(
            BinOp::Add,
            Expr::Port(0),
            Expr::un(UnOp::BitNot, Expr::Port(1)),
        );
        assert_eq!(e.paths().len(), 4);
        assert_eq!(e.at(&[1, 0]), &Expr::Port(1));
        let r = e.replaced(&[1], Expr::Port(2));
        assert_eq!(r, Expr::bin(BinOp::Add, Expr::Port(0), Expr::Port(2)));
        assert_eq!(e.depth(), 2);
    }

    #[test]
    fn commuted_flips_ordering() {
        let e = Expr::bin(BinOp::Lt, Expr::Port(0), Expr::Port(1));
        assert_eq!(
            e.commuted(),
            Expr::bin(BinOp::Gt, Expr::Port(1), Expr::Port(0))
        );
        let s = Expr::bin(BinOp::Sub, Expr::Port(0), Expr::Port(1));
        assert_eq!(s.commuted(), s);
    }
}
