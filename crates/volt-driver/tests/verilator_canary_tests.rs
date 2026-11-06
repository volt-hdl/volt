//! Hakem kanaryası (ADR-0107, "Hakem sürümü" ve "Ek: DFG kapalı"; #145,
//! #148).
//!
//! Davranış testleri, fark testi ve mutasyon testi doğru değeri Verilator
//! simülasyonundan alır: hakemin kendisi yanlışsa ya sahte bulgu çıkar ya
//! da gerçek bir hata gizlenir. Bu test Volt'u değil, PATH'teki
//! Verilator'ı Volt'un yapılandırmasıyla sınar. İki doğru SV, DFG
//! eniyileyicisinin aynı sınıftan iki yanlış derlemesini yeniden üretir:
//!
//! - `verilator_canary/issue145.sv` (#145): DFG açıkken 5.053 devel (OSS
//!   CAD Suite 2026-09-21) `o151a = 4'(p247) - 4'(p247)`'yi `4'hf` basar;
//!   5.052 doğru (0) basar.
//! - `verilator_canary/issue148.sv` (#148): aynı tasarım, `o150a`'yı okuyan
//!   sözleşme yok; DFG açıkken 5.052 de 5.053 devel de `4'hf` basar.
//!
//! İkisi de `-fno-dfg` ile doğrudur. Volt bu bayrağı Verilator'ın her
//! derlemesine verir (`volt_tools::VERILATOR_BUILD_FLAGS`); kanarya o
//! listeyle derler. Verilator güncellenirse (ayrı bir PR,
//! `scripts/ci/verilator.env`) bu test aynı sınıftan bir yanlışın geri
//! gelip gelmediğini gösterir; bayrağı kaldıran PR listeyi boşaltır ve iki
//! kanarya bayraksız geçmelidir (ADR-0107 ekindeki ölçüt).
//!
//! Beklenen değerler SV anlamından elle hesaplandı (aşağıda, alan alan);
//! derleyicinin ya da bir Verilator'ın çıktısı beklenen değer değildir. İki
//! tasarım aynı alanları aynı girişlerle (hepsi 0) basar: beklenenler
//! ortaktır. Komut satırı fark testininkiyle aynıdır (`crates/volt-difftest`,
//! `verilator_args`): Volt'un seçenekleri, `-O0`.
//!
//! Verilator yoksa test atlar; araçlı CI işinde (`VOLT_REQUIRE_TOOLS=all`)
//! yokluk testi düşürür (ADR-0107).

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

const ISSUE_145: &str = include_str!("verilator_canary/issue145.sv");
const ISSUE_148: &str = include_str!("verilator_canary/issue148.sv");

/// Gösterilen birleştirmenin alanları, en anlamlı bitten başlayarak:
/// (ad, genişlik, beklenen değer). Tek vektör: bütün girişler 0
/// (`p46 = p145 = p158 = p192 = p245 = p246 = p247 = p248 = 0`).
const FIELDS: &[(&str, u32, u64)] = &[
    // R152r: 25'd0 < 25'h1FFFFFF → 1.
    ("o152r", 1, 1),
    // (25'h1FFFFFF > 0) == t152b, t152b = (0 < 25'h1FFFFFF) = 1 → 1 == 1.
    ("o152f", 1, 1),
    // 25'h1FFFFFF > 0.
    ("o152e", 1, 1),
    // 0 < 25'h1FFFFFF.
    ("o152d", 1, 1),
    // t152b.
    ("o152b", 1, 1),
    // 0 < 25'h1FFFFFF.
    ("o152a", 1, 1),
    // R151r: 4'(3'd0) - 4'(3'd0) = 0.
    ("o151r", 4, 0),
    // 6'(p247) - 6'(p247): x - x = 0 (mod 2^6) for every x.
    ("o151w", 6, 0),
    // (x - x = 0) >= 4'd12 → 0.
    ("o151k", 1, 0),
    // (x - x) == t151b, t151b = x - x → 0 == 0 → 1.
    ("o151f", 1, 1),
    // 4'(p247) - 4'(p247) = 0 (o151e, o151d), t151b = 0 (o151b), and the
    // line of #145: o151a = 4'(p247) - 4'(p247) = 0.
    ("o151e", 4, 0),
    ("o151d", 4, 0),
    ("o151b", 4, 0),
    ("o151a", 4, 0),
    // R150r: 13'({6'b0, 0}) - 13'(0) = 0 <= 13'(14'(0) - 14'(13'd0)) = 0 → 1.
    ("o150r", 1, 1),
    // u = 13'({6'b0, p245}) - 13'(p158) = 0, s = 13'(14'(p246[12:0]) - 14'(p46))
    // = 0. o150f = (s >= u) == t150b = (1 == (u <= s = 1)) → 1.
    ("o150f", 1, 1),
    // s >= u: 0 >= 0.
    ("o150e", 1, 1),
    // u <= s.
    ("o150d", 1, 1),
    // u <= s150c, s150c = s = 0.
    ("o150c", 1, 1),
    // t150b = u <= s.
    ("o150b", 1, 1),
    // u <= s.
    ("o150a", 1, 1),
    // o149*: assign '0.
    ("o149r", 7, 0),
    ("o149w", 8, 0),
    ("o149k", 1, 0),
    ("o149f", 1, 0),
    ("o149e", 7, 0),
    ("o149d", 7, 0),
    ("o149c", 7, 0),
    ("o149b", 7, 0),
    ("o149a", 7, 0),
    // o148*: assign '0 (67-bit fields).
    ("o148r", 67, 0),
    ("o148f", 1, 0),
    ("o148e", 67, 0),
    ("o148d", 67, 0),
    ("o148c", 67, 0),
    ("o148b", 67, 0),
    ("o148a", 67, 0),
    // R147r: 6'({53'b0, p192}) = 0 <= ~6'd0 = 63 → 1.
    ("o147r", 1, 1),
    // ~p145 = 63; (63 >= 0) == t147b, t147b = (0 <= 63) = 1 → 1.
    ("o147f", 1, 1),
    // 63 >= 0.
    ("o147e", 1, 1),
    // 0 <= 63.
    ("o147d", 1, 1),
    // 0 <= s147c, s147c = ~p145 = 63.
    ("o147c", 1, 1),
    // t147b.
    ("o147b", 1, 1),
    // 0 <= 63.
    ("o147a", 1, 1),
    // o146*, o141*: assign '0.
    ("o146r", 19, 0),
    ("o146w", 25, 0),
    ("o141f", 1, 0),
    ("o141b", 12, 0),
];

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-canary-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Onaltılık metni bitlere çevirir, en anlamlı bit önde.
fn hex_bits(hex: &str) -> Vec<bool> {
    hex.chars()
        .flat_map(|c| {
            let d = c
                .to_digit(16)
                .unwrap_or_else(|| panic!("onaltılık değil: {c:?} ({hex})"));
            (0..4).rev().map(move |i| (d >> i) & 1 == 1)
        })
        .collect()
}

/// `R 0 17 <hex>` satırını alanlara ayırır: (ad, okunan, beklenen).
fn decode(hex: &str) -> Vec<(&'static str, u64, u64)> {
    let total: u32 = FIELDS.iter().map(|f| f.1).sum();
    let bits = hex_bits(hex);
    // %h genişliği 4'ün katına yuvarlar; baştaki dolgu bitleri 0'dır.
    let pad = bits.len() - total as usize;
    assert!(
        bits[..pad].iter().all(|b| !b),
        "dolgu bitleri 0 değil: {hex}"
    );
    let mut at = pad;
    FIELDS
        .iter()
        .map(|&(name, width, expected)| {
            let field = &bits[at..at + width as usize];
            at += width as usize;
            // 64 bitten geniş alanlar yalnız 0 beklenir: bir bit bile 1 ise
            // değer 0 sayılmaz.
            let value = field.iter().fold(0u64, |v, &b| {
                if width > 64 {
                    v | u64::from(b)
                } else {
                    (v << 1) | u64::from(b)
                }
            });
            (name, value, expected)
        })
        .collect()
}

/// `design`'ı `file` adıyla derler ve koşturur; standart çıktıyı döndürür.
/// Verilator seçenekleri Volt'unkiler (`VERILATOR_BUILD_FLAGS`, #148).
fn run(dir: &Path, verilator: &Path, file: &str, design: &str) -> String {
    std::fs::write(dir.join(file), design).expect(file);
    let build = Command::new(verilator)
        .current_dir(dir)
        .args([
            "--binary",
            "--timing",
            "--assert",
            "-Wall",
            "-Wno-fatal",
            "-Wno-DECLFILENAME",
            "--top-module",
            "tb",
            "-Mdir",
            "obj",
            "-o",
            "vtb",
            "-j",
            "2",
            "-CFLAGS",
            "-O0",
        ])
        .args(volt_tools::VERILATOR_BUILD_FLAGS)
        .arg(file)
        .output()
        .expect("verilator çalıştırılamadı");
    assert!(
        build.status.success(),
        "Verilator kanarya tasarımını ({file}) derleyemedi:
{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let sim = Command::new(dir.join("obj").join("vtb"))
        .current_dir(dir)
        .arg("+verilator+error+limit+1000000")
        .output()
        .expect("simülasyon çalıştırılamadı");
    String::from_utf8_lossy(&sim.stdout).into_owned()
}

/// Tasarımı koşturur ve gösterilen her alanı elle hesaplanmış değerle
/// karşılaştırır.
fn check(issue: u32, design: &str) {
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    let tag = format!("issue{issue}");
    let dir = temp_dir(&tag);
    let out = run(&dir, &verilator, &format!("{tag}.sv"), design);
    let line = out
        .lines()
        .find(|l| l.starts_with("R 0 17 "))
        .unwrap_or_else(|| {
            panic!(
                "simülasyon `R 0 17` satırını basmadı:
{out}"
            )
        });
    let hex = line.trim_start_matches("R 0 17 ").trim();
    let wrong: Vec<String> = decode(hex)
        .into_iter()
        .filter(|&(_, got, expected)| got != expected)
        .map(|(name, got, expected)| format!("{name}: expected {expected:#x}, got {got:#x}"))
        .collect();
    assert!(
        wrong.is_empty(),
        "this Verilator, with Volt's options {:?}, simulates correct SystemVerilog          wrongly (the #{issue} design; the #145 class: DFG inlines comparisons into          a wide concatenation and a signed shift spreads the sign bit). It must not          be the reference of the tests; keep the release pinned in          scripts/ci/verilator.env and -fno-dfg in volt_tools::VERILATOR_BUILD_FLAGS.
{}
line: {line}",
        volt_tools::VERILATOR_BUILD_FLAGS,
        wrong.join("
")
    );
    eprintln!("canary #{issue}: {line}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_pinned_verilator_simulates_the_issue_145_design_correctly() {
    check(145, ISSUE_145);
}

#[test]
fn the_pinned_verilator_simulates_the_issue_148_design_correctly() {
    check(148, ISSUE_148);
}

#[test]
fn the_canary_expectation_rejects_the_value_verilator_5_053_devel_prints() {
    // Elle: doğru Verilator ve DFG'nin yanlış derlemesi (#145'te 5.053
    // devel; #148'de 5.052 de) iki tasarım için şu satırları basar. Çözücü
    // doğruyu kabul etmeli, yanlışı o151a dahil reddetmeli; yoksa kanarya
    // hiçbir şey yakalamaz.
    // 560 bit = 140 onaltılık hane: ilk 11 hane o152r..o149r'nin üç biti,
    // son 16 hane o147r..o141b (bit 63..57 birler), arası sıfır.
    let zeros = "0".repeat(140 - 11 - 16);
    let good = format!("fc0040003f8{zeros}fe00000000000000");
    let devel = format!("fc00ffffff8{zeros}fe00000000000000");
    assert_eq!(good.len(), 140);
    assert!(
        decode(&good).iter().all(|&(_, got, exp)| got == exp),
        "{good}"
    );
    let bad: Vec<&str> = decode(&devel)
        .into_iter()
        .filter(|&(_, got, exp)| got != exp)
        .map(|(name, ..)| name)
        .collect();
    assert_eq!(
        bad,
        ["o151k", "o151e", "o151d", "o151b", "o151a"],
        "{devel}"
    );
}
