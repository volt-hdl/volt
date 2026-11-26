//! Genişlik gölgesi (#407 Aşama 1, ADR-0136): yalnız testlerin açtığı
//! `width-shadow` cargo özelliğiyle derlenir. Üretici her ifadeye verdiği
//! genişliği burada kaydeder; sürücünün testi (`width_shadow_tests`) onu
//! tip denetiminin `expr_types` tablosuyla karşılaştırır. Kayıt yalnız
//! [`record`] içinde açıktır ve hiçbir şeyi değiştirmez: üretilen SV aynı
//! kalır.

use std::cell::RefCell;
use std::mem::Discriminant;

use volt_ast::{Expr, ExprKind, Idx, SourceFile};
use volt_span::Span;

use crate::Sig;

/// Üreticinin bir ifadeye verdiği genişlik (`width_of`'un bir sonucu).
#[derive(Debug, Clone)]
pub struct WidthNote {
    /// İfadenin ait olduğu AST'nin adresi. Üretici fn açılımında
    /// (ADR-0081) ve struct indirgemesinde (ADR-0077) AST'nin kopyası
    /// üzerinde çalışır; karşılaştırma kök AST'yi bu adresle ayırır.
    pub ast: usize,
    pub expr: Idx<Expr>,
    pub span: Span,
    /// Kopyadaki ifadenin kök AST'deki aynı indeksli ifadeyle aynı tür
    /// olup olmadığını denetlemek için.
    pub kind: Discriminant<ExprKind>,
    pub sig: Option<Sig>,
}

/// Bir `prev()` çağrısının yardımcı register'ı (#413): `past_reg_block`'un
/// kararı, yani SV'de bildirilen genişlik ve işaret.
#[derive(Debug, Clone)]
pub struct HelperNote {
    /// Çağrının ait olduğu AST'nin adresi ([`WidthNote::ast`] gibi).
    pub ast: usize,
    pub call: Idx<Expr>,
    pub call_span: Span,
    /// `prev(x[, N])`'in `x`'i.
    pub arg: Idx<Expr>,
    pub arg_span: Span,
    /// Struct indirgemesinin yaprağı (ADR-0077): bütün-struct argüman
    /// (indeksi ve konumu) ve yaprağın alan yolu.
    pub origin: Option<(Idx<Expr>, Span, Vec<String>)>,
    /// Bildirilen genişlik ve işaret; `None`: genişlik bilinmiyor, register
    /// bildirilmedi (E2005).
    pub sig: Option<Sig>,
}

thread_local! {
    static NOTES: RefCell<Option<Vec<WidthNote>>> = const { RefCell::new(None) };
    static HELPERS: RefCell<Option<Vec<HelperNote>>> = const { RefCell::new(None) };
}

/// `f` koşarken bu iş parçacığında üreticinin verdiği genişlikleri toplar.
pub fn record<R>(f: impl FnOnce() -> R) -> (R, Vec<WidthNote>) {
    let (out, notes, _) = record_all(f);
    (out, notes)
}

/// [`record`] ve `prev()` yardımcı register'larının kararları.
pub fn record_all<R>(f: impl FnOnce() -> R) -> (R, Vec<WidthNote>, Vec<HelperNote>) {
    NOTES.with(|n| *n.borrow_mut() = Some(Vec::new()));
    HELPERS.with(|n| *n.borrow_mut() = Some(Vec::new()));
    let out = f();
    let notes = NOTES.with(|n| n.borrow_mut().take()).unwrap_or_default();
    let helpers = HELPERS.with(|n| n.borrow_mut().take()).unwrap_or_default();
    (out, notes, helpers)
}

/// Yardımcı register kararını kaydeder; [`record_all`] dışında bir şey
/// yapmaz.
pub(crate) fn note_helper(
    ast: &SourceFile,
    call: Idx<Expr>,
    arg: Idx<Expr>,
    origin: Option<(Idx<Expr>, Vec<String>)>,
    sig: Option<Sig>,
) {
    HELPERS.with(|n| {
        if let Some(notes) = n.borrow_mut().as_mut() {
            notes.push(HelperNote {
                ast: ast as *const SourceFile as usize,
                call,
                call_span: ast.exprs[call].span,
                arg,
                arg_span: ast.exprs[arg].span,
                origin: origin.map(|(o, path)| (o, ast.exprs[o].span, path)),
                sig,
            });
        }
    });
}

/// `width_of`'un sonucunu kaydeder; [`record`] dışında bir şey yapmaz.
pub(crate) fn note(ast: &SourceFile, expr: Idx<Expr>, sig: Option<Sig>) {
    NOTES.with(|n| {
        if let Some(notes) = n.borrow_mut().as_mut() {
            let e = &ast.exprs[expr];
            notes.push(WidthNote {
                ast: ast as *const SourceFile as usize,
                expr,
                span: e.span,
                kind: std::mem::discriminant(&e.kind),
                sig,
            });
        }
    });
}
