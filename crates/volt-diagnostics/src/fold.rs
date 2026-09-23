//! Özdeş tanı katlama (ADR-0068).
//!
//! Parser katmanı desugar'ları bir şablonu çok kez açar: modül seviyesi
//! `for` (ADR-0056), generic monomorfizasyon (ADR-0041), bundle dizileri
//! (ADR-0056). Şablondaki tek bir hata her kopyada yeniden tanılanır —
//! fuzz bulgusu 2'de iç içe iki `for` 283² = 65 303 özdeş E2005 üretti.
//! Aynı hatanın kopyaları bilgi taşımaz; katlanır. FARKLI tanılar
//! (yineleme değerini taşıyan mesaj, farklı etiket, farklı not) ayrı
//! kalır.
//!
//! Kimlik ([`Identity`]): kod + önem + mesaj + tüm etiketli span'ler
//! (`Span.ctx` HARİÇ) + notlar. `ctx` klonu ayırt eden tek alandır
//! (ADR-0041); kaynak konumu aynıysa kullanıcı için aynı yerdir. Çözüm
//! (`help`) ve öneriler kimliğe DAHİL DEĞİLDİR: açılım gövdedeki adları
//! yeniden adlandırır (`t` → `t_0`, `t_1`) ve çözüm metni bu üretilmiş
//! adı taşıyabilir (W2012 "let t_0 : i32 = ..."); aynı kaynak konumu
//! için ilk kopyanın çözümü kalır. Katlanan kopyaların birincil
//! `ctx`'leri kalan tanının [`Diagnostic::folded_ctxs`] listesine
//! eklenir; not metnini üreten katman (volt-hir `annotate_generate`)
//! bunları yineleme/örnekleme bilgisine çevirir.

use std::collections::HashMap;

use volt_span::FileId;

use crate::{Diagnostic, ErrorCode, Note, Severity};

/// Katlama kimliği — hash ve eşitlik TEK tanımdan türer.
#[derive(Hash, PartialEq, Eq)]
struct Identity {
    code: ErrorCode,
    severity: Severity,
    message: String,
    /// (dosya, başlangıç, bitiş, etiket, birincil mi) — `ctx` yok.
    spans: Vec<(FileId, u32, u32, String, bool)>,
    notes: Vec<Note>,
}

fn identity(d: &Diagnostic) -> Identity {
    Identity {
        code: d.code,
        severity: d.severity,
        message: d.message.clone(),
        spans: d
            .spans
            .iter()
            .map(|s| {
                (
                    s.span.file,
                    s.span.start,
                    s.span.end,
                    s.label.clone(),
                    s.primary,
                )
            })
            .collect(),
        notes: d.notes.clone(),
    }
}

/// İki tanı aynı kimlikte mi: kod, önem, mesaj, etiketli span'ler
/// (`Span.ctx` hariç) ve notlar; çözüm/öneri/`folded_ctxs` bakılmaz.
pub fn same_identity(a: &Diagnostic, b: &Diagnostic) -> bool {
    identity(a) == identity(b)
}

/// `diags[from..]` içindeki özdeş tanıları ilk görülene katlar; sıra
/// korunur. Katlanan her kopyanın birincil `ctx`'i (ve kendi
/// `folded_ctxs`'i) kalan tanıya eklenir. `from` öncesi dokunulmaz:
/// açılım döngüsü her yinelemeden sonra yalnız kendi kuyruğunu katlar
/// (bellek O(farklı tanı) kalır), son toplayıcı `from = 0` ile tümünü.
pub fn fold_duplicates(diags: &mut Vec<Diagnostic>, from: usize) {
    if diags.len().saturating_sub(from) < 2 {
        return;
    }
    let tail = diags.split_off(from);
    let mut kept: Vec<Diagnostic> = Vec::with_capacity(tail.len().min(64));
    // Kimlik → `kept` içindeki indeks.
    let mut seen: HashMap<Identity, usize> = HashMap::new();
    for d in tail {
        match seen.get(&identity(&d)) {
            Some(&i) => {
                let k = &mut kept[i];
                if let Some(p) = d.primary_span() {
                    k.folded_ctxs.push(p.span.ctx);
                }
                k.folded_ctxs.extend(d.folded_ctxs);
            }
            None => {
                seen.insert(identity(&d), kept.len());
                kept.push(d);
            }
        }
    }
    diags.extend(kept);
}
