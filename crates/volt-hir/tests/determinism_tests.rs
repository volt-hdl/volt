//! Determinizm regresyon testleri: aynı girdi → bayt bayt aynı tanılar.
//!
//! `HashMap` iterasyon sırası süreç içinde bile örnekten örneğe değişir
//! (`RandomState`); bu yüzden aynı kaynağı art arda çözümlemek, sıraya
//! sızan her iterasyonu yakalar. Adaylar bilerek ÇOK tutulur ki şans
//! eseri geçme olasılığı ihmal edilebilir olsun.

use volt_hir::analyze;
use volt_syntax::{parse, FileId};

const RUNS: usize = 5;

/// Kaynağı baştan (ayrıştırma dahil) çözümler, tanıları bayt dizisine döker.
fn diagnostics_bytes(src: &str) -> Vec<u8> {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "test kaynağı ayrışmalı: {:?}",
        parsed.error_codes()
    );
    format!("{:#?}", analyze(&parsed.ast).diagnostics).into_bytes()
}

/// Kaynağı `RUNS` kez derler; tüm koşular ilkine bayt bayt eşit olmalı.
fn assert_deterministic(src: &str) -> String {
    let first = diagnostics_bytes(src);
    for run in 1..RUNS {
        assert!(
            diagnostics_bytes(src) == first,
            "koşu {run} ilk koşudan farklı tanı üretti"
        );
    }
    String::from_utf8(first).expect("Debug çıktısı UTF-8")
}

/// Sekiz aday da `z`'ye 1 uzaklıkta; bildirim sırası alfabetik DEĞİL.
const TIED_SUGGESTIONS: &str = "module M { in h : u8 in g : u8 in f : u8 in e : u8 \
     in d : u8 in c : u8 in b : u8 in a : u8 out y : u8 y = z }";

/// Top döngüye ortadan (C'den) girer; döngünün en küçük DefId'si B'dir.
const CYCLE_ENTERED_MIDWAY: &str = "module Top { in x : u8 out y : u8 let c = C { p: x } y = c.q }
     module B { in p : u8 out q : u8 let c = C { p: p } q = c.q }
     module C { in p : u8 out q : u8 let b = B { p: p } q = b.q }";

/// On modüllük halka: `edges` haritasında on başlangıç adayı.
fn ring_source() -> String {
    const RING: usize = 10;
    (0..RING)
        .map(|i| {
            format!(
                "module R{i} {{ in p : u8 out q : u8 let n = R{} {{ p: p }} q = n.q }}\n",
                (i + 1) % RING
            )
        })
        .collect()
}

#[test]
fn equidistant_suggestions_are_byte_identical_across_runs() {
    assert_deterministic(TIED_SUGGESTIONS);
}

#[test]
fn equidistant_suggestion_picks_first_declared_name() {
    // Eşit uzaklıkta bağ, DefId (bildirim) sırasıyla bozulur: önce `h`.
    let text = assert_deterministic(TIED_SUGGESTIONS);
    assert!(text.contains("did you mean 'h'?"), "{text}");
}

#[test]
fn equidistant_suggestion_prefers_inner_scope() {
    // İç kapsamdaki aday, dış kapsamdaki eşit uzaklıktaki adayı yener.
    let src = "const a : u8 = 1
         module M { in b : u8 out y : u8 y = z }";
    let text = assert_deterministic(src);
    assert!(text.contains("did you mean 'b'?"), "{text}");
}

#[test]
fn instance_cycle_is_byte_identical_across_runs() {
    assert_deterministic(&ring_source());
}

#[test]
fn instance_cycle_starts_at_smallest_def_id() {
    let text = assert_deterministic(&ring_source());
    assert!(text.contains("cycle: R0 → R1 →"), "{text}");
    assert_eq!(text.matches("E1006").count(), 1, "{text}");
}

#[test]
fn instance_cycle_entered_midway_is_rotated_to_smallest_def_id() {
    let text = assert_deterministic(CYCLE_ENTERED_MIDWAY);
    assert!(text.contains("cycle: B → C → B"), "{text}");
    assert_eq!(text.matches("E1006").count(), 1, "{text}");
}
