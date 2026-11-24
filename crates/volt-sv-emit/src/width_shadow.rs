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

thread_local! {
    static NOTES: RefCell<Option<Vec<WidthNote>>> = const { RefCell::new(None) };
}

/// `f` koşarken bu iş parçacığında üreticinin verdiği genişlikleri toplar.
pub fn record<R>(f: impl FnOnce() -> R) -> (R, Vec<WidthNote>) {
    NOTES.with(|n| *n.borrow_mut() = Some(Vec::new()));
    let out = f();
    let notes = NOTES.with(|n| n.borrow_mut().take()).unwrap_or_default();
    (out, notes)
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
