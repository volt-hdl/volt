//! Açılım düğüm bütçesi (ADR-0068 §4 ve §6) — `for` açılımı (ADR-0056)
//! ile generic monomorfizasyonun (ADR-0041) AST'ye yazdığı HER şey tek
//! sayaçtan düşer.
//!
//! İlk sürüm yalnız `stmts + exprs` arenalarının büyümesini sayıyordu;
//! `Cloner` desen, tip ve blok arenalarını da her kopyada yazar. Gecelik
//! fuzz (ADR-0068 §6) hata kurtarmanın bıraktığı n düğümlük desen
//! ağacıyla sayılmayan arenadan 1,47 GB kurdu. Sayaç bu yüzden arena
//! listesi değil, YAZMA KAPISIDIR: `Cloner` `&mut SourceFile` tutmaz,
//! [`AstWriter`] tutar — okuma `Deref` ile serbest, yazma yalnız
//! [`AstWriter::write`] ile ve her çağrı bir düğüm düşer. İleride
//! eklenen bir arena ya da yan tabloya yazım bu kapıdan geçmeden
//! derlenmez; bütçenin dışında kalamaz.

use std::ops::Deref;

use volt_ast::SourceFile;

/// Derleme birimi başına açılım bütçesi: `Cloner`'ın her yazımı (arena
/// düğümü ya da yan tablo girişi) ve açıcının kendi deyim/yineleme
/// kayıtları. 4096 yineleme × 64 düğümlük gövde sığar; `examples/` ve
/// test külliyatının en büyüğü (hybrid_accel, 3 838 düğüm KAYNAK
/// dahil) bütçenin %1,5'i. Tek tanım `volt_ast`'te: fonksiyon açılımı
/// (ADR-0081, volt-hir) aynı sınırı kullanır.
pub(crate) use volt_ast::MAX_EXPANSION_NODES;

/// Kopyalanan dizgenin (ad, yeniden ad, dizge literali) düğüm karşılığı:
/// her `TEXT_BYTES_PER_NODE` bayt bir düğüm daha. Bütçe düğüm sayarken
/// 3 900 karakterlik tek bir ad 4 KB'lık girdiden 459 MB kuruyordu
/// (bayt sayılmıyordu — yine "yalnız bir kısmı sayan bütçe").
const TEXT_BYTES_PER_NODE: usize = 64;

/// Tek sayaç; `monomorphize` başına bir tane (derleme birimi, ADR-0042).
#[derive(Default)]
pub(super) struct ExpansionBudget {
    used: usize,
    reported: bool,
}

impl ExpansionBudget {
    pub(super) fn charge(&mut self, nodes: usize) {
        self.used = self.used.saturating_add(nodes);
    }

    /// Kalıcı dizge kopyası: `len / TEXT_BYTES_PER_NODE` düğüm (kısa
    /// adlar bedava — taşıyıcı düğüm zaten sayıldı).
    pub(super) fn charge_text(&mut self, len: usize) {
        self.charge(len / TEXT_BYTES_PER_NODE);
    }

    pub(super) fn exhausted(&self) -> bool {
        self.used > MAX_EXPANSION_NODES
    }

    /// Bütçe aşıldıysa ve henüz tanılanmadıysa true — birimde tek E2027
    /// (kaskad yok); sonraki açılımlar sessizce durur.
    pub(super) fn take_report(&mut self) -> bool {
        if self.exhausted() && !self.reported {
            self.reported = true;
            return true;
        }
        false
    }
}

/// `Cloner`'ın AST'ye tek erişim yolu: okuma `Deref`, yazma [`Self::write`].
pub(super) struct AstWriter<'a> {
    ast: &'a mut SourceFile,
    budget: &'a mut ExpansionBudget,
}

impl<'a> AstWriter<'a> {
    pub(super) fn new(ast: &'a mut SourceFile, budget: &'a mut ExpansionBudget) -> Self {
        Self { ast, budget }
    }

    /// AST'ye bir yazım (arena `alloc`, yan tablo `insert`); bütçeden bir
    /// düğüm düşer.
    pub(super) fn write<R>(&mut self, f: impl FnOnce(&mut SourceFile) -> R) -> R {
        self.budget.charge(1);
        f(self.ast)
    }

    /// Klona kopyalanan dizge (ad metni, dizge literali) bütçeden düşer.
    pub(super) fn charge_text(&mut self, len: usize) {
        self.budget.charge_text(len);
    }
}

impl Deref for AstWriter<'_> {
    type Target = SourceFile;

    fn deref(&self) -> &SourceFile {
        self.ast
    }
}
