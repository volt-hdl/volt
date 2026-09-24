//! Const diziler (ADR-0041): `const COEFFS : [i16; 8] = [1, 2, ...]`.
//!
//! Sabit indeks (`COEFFS[3]`, `COEFFS[i]` döngü değişkeniyle) derleme
//! zamanında elemana katlanır; değişken indeks (`COEFFS[idx]`) modül
//! içinde tek bir sabit tablo bildirimi üretir. Tablonun SV biçimi
//! [`ConstArrayStyle`] ile seçilir — Yosys/Verilator ölçümüne göre.

use volt_ast::{ArrayLitKind, Expr, ExprKind, Idx, NumBase, TypeRefKind};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::expr::widen_sig;
use crate::{Emitter, Sig};

/// Değişken indeksli const dizi için üretilen SV biçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConstArrayStyle {
    /// `localparam logic signed [15:0] COEFFS [0:7] = '{...};` ve
    /// `COEFFS[idx]` — unpacked sabit dizi. Verilator kabul eder, Yosys
    /// (`read_verilog -sv`) unpacked localparam dizisini REDDEDER
    /// (ADR-0041 ölçüm notu); bu yüzden varsayılan değil.
    LocalparamArray,
    /// `function automatic ... COEFFS_at(input logic [2:0] i) case ...`
    /// ve `COEFFS_at(idx)` — tablo işlevi. Yosys ve Verilator -Wall
    /// ikisinde de temiz; varsayılan (ADR-0041).
    #[default]
    CaseFunction,
}

/// Açılabilecek en uzun sabit tablo.
const MAX_TABLE_LEN: u32 = 4096;

impl<'a> Emitter<'a> {
    /// Ad modül sinyali, örnek adı ya da döngü değişkeniyle gölgeleniyor mu?
    pub(crate) fn is_shadowed(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
            || self.builtin_insts.contains_key(name)
            || self.user_insts.contains_key(name)
            || self.loop_var(name).is_some()
    }

    /// Gölgelenmemiş üst düzey dizi sabiti: (eleman imzası, uzunluk,
    /// değer ifadesi). Dizi tipli değilse sessizce None.
    pub(crate) fn const_array_info(&mut self, name: &str) -> Option<(Sig, u32, Idx<Expr>)> {
        if self.is_shadowed(name) {
            return None;
        }
        let &(ty, value) = self.consts.get(name)?;
        if !matches!(
            self.ast.types[crate::alias::resolve(self.ast, ty)].kind,
            TypeRefKind::Array { .. }
        ) {
            return None;
        }
        let span = self.ast.types[ty].span;
        let (sig, len) = self.array_reg_sig(ty, span)?;
        Some((sig, len, value))
    }

    /// Bildirilen tipi dizi olan, gölgelenmemiş sabit mi?
    pub(crate) fn is_const_array(&self, name: &str) -> bool {
        !self.is_shadowed(name)
            && self.consts.get(name).is_some_and(|&(ty, _)| {
                matches!(
                    self.ast.types[crate::alias::resolve(self.ast, ty)].kind,
                    TypeRefKind::Array { .. }
                )
            })
    }

    /// Dizi sabitinin `i`. elemanı (derleme zamanı değeri); sınır dışı
    /// ya da sabit olmayan elemanda None.
    pub(crate) fn const_array_element(&self, name: &str, i: i128) -> Option<i128> {
        if !self.is_const_array(name) {
            return None;
        }
        let &(_, value) = self.consts.get(name)?;
        let elems = self.const_array_elements(value)?;
        usize::try_from(i).ok().and_then(|i| elems.get(i).copied())
    }

    /// Dizi literalinin elemanları: `[a, b, c]` ya da `[v; N]`; başka
    /// bir dizi sabitine referans da izlenir.
    pub(crate) fn const_array_elements(&self, value: Idx<Expr>) -> Option<Vec<i128>> {
        match &self.ast.exprs[value].kind {
            ExprKind::ArrayLit(ArrayLitKind::List(items)) => {
                items.iter().map(|&e| self.eval_const(e)).collect()
            }
            ExprKind::ArrayLit(ArrayLitKind::Repeat { value, count }) => {
                let v = self.eval_const(*value)?;
                let n = usize::try_from(self.eval_const(*count)?).ok()?;
                (n as u128 <= u128::from(MAX_TABLE_LEN)).then(|| vec![v; n])
            }
            ExprKind::Path(p) if p.segments.len() == 1 => {
                let name = &p.segments[0].text;
                if !self.is_const_array(name) {
                    return None;
                }
                let &(_, inner) = self.consts.get(name)?;
                self.const_array_elements(inner)
            }
            _ => None,
        }
    }

    /// `COEFFS[k]` üretimi. Taban dizi sabiti değilse None (normal yol).
    /// Sabit indeks → eleman literali; değişken indeks → tablo referansı
    /// (tablo bildirimi modül başına bir kez, `emit_const_array_decl`).
    pub(crate) fn try_emit_const_array_index(
        &mut self,
        base: Idx<Expr>,
        index: Idx<Expr>,
        ctx: Option<Sig>,
        span: Span,
    ) -> Option<String> {
        let name = crate::path_single(self.ast, base)?.to_string();
        let (sig, len, _) = self.const_array_info(&name)?;
        match self.eval_const(index) {
            Some(i) => match self.const_array_element(&name, i) {
                // Eleman literali bağlam genişliğiyle boyutlanır (ADR-0041).
                Some(v) => Some(self.fmt_int(v, NumBase::Dec, widen_sig(Some(sig), ctx), span)),
                None => {
                    self.error(
                        ErrorCode::E2006,
                        lstr!(
                            en: "constant array index {i} out of bounds (length {len})";
                            tr: "sabit dizi indeksi {i} sınır dışı (uzunluk {len})"
                        ),
                        span,
                        &lstr!(en: "valid range: 0..{len}"; tr: "geçerli aralık: 0..{len}"),
                    );
                    Some("1'b0".to_string())
                }
            },
            None => {
                if !self.array_consts_used.contains(&name) {
                    self.array_consts_used.push(name.clone());
                }
                let idx = self.emit_plain(index);
                Some(match self.const_array_style {
                    ConstArrayStyle::LocalparamArray => format!("{name}[{idx}]"),
                    ConstArrayStyle::CaseFunction => format!("{name}_at({idx})"),
                })
            }
        }
    }

    /// Değişken indeksle kullanılan dizi sabitinin modül içi tablo
    /// bildirimi. TEK üretim noktası: biçim `const_array_style`'a bağlı.
    pub(crate) fn emit_const_array_decl(&mut self, name: &str) -> Option<String> {
        let (sig, len, value) = self.const_array_info(name)?;
        let span = self.ast.exprs[value].span;
        let Some(elems) = self.const_array_elements(value) else {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "the elements of constant array '{name}' cannot be evaluated at compile time";
                    tr: "'{name}' sabit dizisinin elemanları derleme zamanında hesaplanamıyor"
                ),
                span,
                &lstr!(en: "use integer literals or constants"; tr: "tam sayı literali ya da sabit kullanın"),
            );
            return None;
        };
        if elems.len() as u32 != len {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "constant array '{name}' has {} elements, its type declares {len}", elems.len();
                    tr: "'{name}' sabit dizisinin {} elemanı var, tipi {len} bildiriyor", elems.len()
                ),
                span,
                &lstr!(en: "match the literal to the declared length"; tr: "literali bildirilen uzunluğa eşitleyin"),
            );
            return None;
        }
        let lits: Vec<String> = elems
            .iter()
            .map(|&v| self.fmt_int(v, NumBase::Dec, Some(sig), span))
            .collect();
        Some(match self.const_array_style {
            ConstArrayStyle::LocalparamArray => format!(
                "    localparam {} {name} [0:{}] = '{{{}}};",
                sig.decl_type(),
                len - 1,
                lits.join(", ")
            ),
            ConstArrayStyle::CaseFunction => case_function(name, sig, len, &lits),
        })
    }
}

/// `function automatic <T> NAME_at(input logic [W-1:0] i)` tablo işlevi;
/// W = clog2(len) (en az 1), tanımsız indeks 0 döner.
fn case_function(name: &str, sig: Sig, len: u32, lits: &[String]) -> String {
    let iw = (32 - (len - 1).max(1).leading_zeros()).max(1);
    let zero = crate::zero_of(sig);
    let mut out = format!(
        "    function automatic {} {name}_at(input logic [{}:0] i);\n        case (i)\n",
        sig.decl_type(),
        iw - 1
    );
    for (k, lit) in lits.iter().enumerate() {
        out.push_str(&format!("            {k}: {name}_at = {lit};\n"));
    }
    out.push_str(&format!(
        "            default: {name}_at = {zero};\n        endcase\n    endfunction"
    ));
    out
}
