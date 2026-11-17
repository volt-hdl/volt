//! Hakem kanaryası (ADR-0107, "Hakem sürümü" ve "Ek: DFG kapalı"; #145,
//! #148). Verilator'a bildirildi:
//! https://github.com/verilator/verilator/issues/9060.
//!
//! Davranış testleri, fark testi ve mutasyon testi doğru değeri Verilator
//! simülasyonundan alır: hakemin kendisi yanlışsa ya sahte bulgu çıkar ya
//! da gerçek bir hata gizlenir. Bu test Volt'u değil, PATH'teki
//! Verilator'ı Volt'un yapılandırmasıyla sınar. İki doğru SV, DFG
//! eniyileyicisinin aynı sınıftan iki yanlış derlemesini yeniden üretir:
//!
//! - `verilator_canary/issue145.sv` (#145): DFG açıkken 5.053 devel (OSS
//!   CAD Suite 2026-09-21) ve 5.054 `o151a = 4'(p247) - 4'(p247)`'yi
//!   `4'hf` basar; 5.052 doğru (0) basar.
//! - `verilator_canary/issue148.sv` (#148): aynı tasarım, `o150a`'yı okuyan
//!   sözleşme yok; DFG açıkken 5.052, 5.053 devel ve 5.054 `4'hf` basar.
//!
//! Üçüncü ve dördüncü tasarım DFG'den bağımsız bir hatayı belgeler:
//!
//! - `verilator_canary/issue302.sv` (#302): işaretli `>>>`, genişlik kadar
//!   ya da fazla sabit miktarla. 5.052 `a >>> 64`'e (i63) ve
//!   `(e + f) >>> 130`'a (i65) negatif değerde 0 basar; sabitlenen 5.054
//!   yalnız ikincisinde yanlıştır (`…0001`). Volt'un ürettiği `>>> (w-1)`
//!   biçimi (ADR-0120) ikisinde de doğrudur; test bunu her sürümde, eski
//!   biçimlerin ölçülen yanlışını ölçülen sürümlerde sınar.
//! - `verilator_canary/issue9256.sv` (#309, Verilator #9256): aynı kaydırma,
//!   miktar sabite bağlı bir port (Volt için çalışma anı değeri, `>>> k`).
//!   5.052 dört alana da 0 basar; 5.054 w mod 32 = 1 olan 64 bitten geniş
//!   bileşik değerde (65, 97, 129) `…0001` basar, 66 bitte doğrudur. Volt
//!   bu biçimden kaçınamaz: test kalan hatayı ölçülen sürümlerde belgeler.
//!
//! İlk ikisi `-fno-dfg` ile doğrudur. Volt bu bayrağı Verilator'ın her
//! derlemesine verir (`volt_tools::VERILATOR_BUILD_FLAGS`); kanarya o
//! listeyle derler. Verilator güncellenirse (ayrı bir PR,
//! `scripts/ci/verilator.env`) bu test aynı sınıftan bir yanlışın geri
//! gelip gelmediğini gösterir; bayrağı kaldıran PR listeyi boşaltır ve iki
//! kanarya bayraksız geçmelidir (ADR-0107 ekindeki ölçüt). Bayraksız
//! (DFG açık) sonuç da ölçülür (`DFG_ON_MEASURED`, Verilator #9060): hata
//! bir sürümde düzelirse test bunu gösterir.
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
const ISSUE_302: &str = include_str!("verilator_canary/issue302.sv");
const ISSUE_9256: &str = include_str!("verilator_canary/issue9256.sv");

/// #302 tasarımının elle hesaplanmış değerleri (IEEE 1800-2017 §11.4.10,
/// ADR-0120): girişler a = -1, e = -1, f = 0; işaretli değerin genişlik
/// kadar ya da fazla aritmetik kaydırması işaret bitiyle dolar. 63 birli
/// değer `%h` ile 16 hane, 65 birli değer 17 hane.
const ISSUE_302_Y: &str = "7fffffffffffffff";
const ISSUE_302_S: &str = "1ffffffffffffffff";

/// #302'nin eski biçimlerini (`a >>> 64`, `(e + f) >>> 130`) ölçülen
/// sürümlerin bastığı değerler, Volt'un seçenekleriyle (`-fno-dfg`):
/// (sürüm, y64, s130). 5.052: ikisi de 0 (sabit K ≥ C++ kelime
/// genişliğinde işaret biti cppw-1'den alınır). 5.054: y64 doğru, s130
/// `…0001` (w mod 32 = 1 olan, 64 bitten geniş bileşik değerde kalan dar
/// hata, Verilator #9256). Ölçüm: #302'deki teşhis ve ADR-0120'nin PR'ı;
/// iki satır hakemi 5.054'e taşıyan PR'da (#309) yeniden ölçüldü, aynı.
const ISSUE_302_MEASURED: &[(&str, &str, &str)] = &[
    ("5.052", "0000000000000000", "00000000000000000"),
    ("5.054", ISSUE_302_Y, "00000000000000001"),
];

/// #9256 tasarımının elle hesaplanmış değerleri (IEEE 1800-2017 §11.4.10):
/// a = -1, b = 0, a + b = -1; miktar genişlikten büyük, bütün bitler işaret
/// biti. Sıra y65, y97, y129, y66; `%h` genişliği 4'ün katına yuvarlar:
/// 65 bir = `1` ve 16 `f`, 97 bir = `1` ve 24 `f`, 129 bir = `1` ve 32 `f`,
/// 66 bir = `3` ve 16 `f`.
const ISSUE_9256_RIGHT: [&str; 4] = [
    "1ffffffffffffffff",
    "1ffffffffffffffffffffffff",
    "1ffffffffffffffffffffffffffffffff",
    "3ffffffffffffffff",
];

/// #9256 tasarımında ölçülen sürümlerin bastığı değerler, Volt'un
/// seçenekleriyle (`-fno-dfg`; DFG açıkken de aynı). 5.052: dördü de 0
/// (#302'nin hatası; miktar Verilator için sabit, #309). 5.054: y66 doğru,
/// w mod 32 = 1 olan üçü `…0001` (Verilator #9256). Ölçüm: #309'un PR'ı.
const ISSUE_9256_MEASURED: &[(&str, [&str; 4])] = &[
    (
        "5.052",
        [
            "00000000000000000",
            "0000000000000000000000000",
            "000000000000000000000000000000000",
            "00000000000000000",
        ],
    ),
    (
        "5.054",
        [
            "00000000000000001",
            "0000000000000000000000001",
            "000000000000000000000000000000001",
            ISSUE_9256_RIGHT[3],
        ],
    ),
];

/// DFG açıkken (`VERILATOR_BUILD_FLAGS` olmadan) #145 ve #148
/// tasarımlarında yanlış basılan alanlar, ölçülen sürümlerde: (sürüm,
/// #145, #148). Boş liste: doğru. 5.052: #145 doğru, #148 yanlış; 5.054:
/// ikisi de yanlış (Verilator #9060 açık, düzeltme yok). Bir satırın iki
/// listesi de boşalırsa ADR-0107 ekindeki kaldırma ölçütünün 3. maddesi o
/// sürümde sağlanır. Ölçüm: ADR-0121 (5.052 satırı ADR-0107'nin #148
/// ekindeki tabloyla aynı).
const DFG_ON_MEASURED: &[(&str, &[&str], &[&str])] = &[
    ("5.052", &[], &DFG_WRONG),
    ("5.054", &DFG_WRONG, &DFG_WRONG),
];

/// #145 sınıfının yanlış derlemesinde bozulan alanlar.
const DFG_WRONG: [&str; 5] = ["o151k", "o151e", "o151d", "o151b", "o151a"];

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
/// Verilator seçenekleri `flags`: kanaryalarda Volt'unkiler
/// (`VERILATOR_BUILD_FLAGS`, #148), DFG ölçümünde boş.
fn run(dir: &Path, verilator: &Path, file: &str, design: &str, flags: &[&str]) -> String {
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
        .args(flags)
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
    let out = run(
        &dir,
        &verilator,
        &format!("{tag}.sv"),
        design,
        volt_tools::VERILATOR_BUILD_FLAGS,
    );
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

/// `verilator --version` çıktısındaki sürüm (`Verilator 5.052 2026-09-05
/// rev v5.052` → `5.052`).
fn verilator_version(verilator: &Path) -> String {
    let out = Command::new(verilator)
        .arg("--version")
        .output()
        .expect("verilator --version");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.split_whitespace()
        .nth(1)
        .unwrap_or_else(|| panic!("sürüm okunamadı: {text:?}"))
        .to_string()
}

/// #302: Volt'un bugünkü biçimleri (`a >>> 62`, `(e + f) >>> 64`, ADR-0120)
/// her Verilator'da doğru olmalı; eski biçimlerin (`a >>> 64`,
/// `(e + f) >>> 130`) ölçülen sürümdeki yanlışı belgelenir: sabitlenen
/// sürüm (5.052) ikisine de 0 basar. Ölçülen sürümün davranışı değişirse
/// test düşer: hata ve ADR-0120'nin gerekçesi yeniden ölçülmeli.
#[test]
fn the_issue_302_signed_overshift_canary() {
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    let version = verilator_version(&verilator);
    let dir = temp_dir("issue302");
    let out = run(
        &dir,
        &verilator,
        "issue302.sv",
        ISSUE_302,
        volt_tools::VERILATOR_BUILD_FLAGS,
    );
    let line = out
        .lines()
        .find(|l| l.starts_with("R302 "))
        .unwrap_or_else(|| panic!("simülasyon `R302` satırını basmadı:\n{out}"));
    let f: Vec<&str> = line.split_whitespace().skip(1).collect();
    assert_eq!(f.len(), 4, "{line}");
    assert_eq!(
        (f[1], f[3]),
        (ISSUE_302_Y, ISSUE_302_S),
        "Verilator {version}, with Volt's options {:?}, gets `a >>> (w-1)` wrong: \
         the form Volt emits for a signed `>>` by a constant at or past the \
         width (#302). It must not be the reference of the tests.\nline: {line}",
        volt_tools::VERILATOR_BUILD_FLAGS
    );
    match ISSUE_302_MEASURED.iter().find(|(v, ..)| *v == version) {
        Some(&(_, y64, s130)) => assert_eq!(
            (f[0], f[2]),
            (y64, s130),
            "Verilator {version} no longer prints the measured values for \
             `a >>> 64` and `(e + f) >>> 130` (#302): measure the bug again \
             and update ISSUE_302_MEASURED and the reasoning of ADR-0120.\n\
             line: {line}"
        ),
        None => eprintln!(
            "canary #302: Verilator {version} has no measured row; the old \
             forms print y64 = {}, s130 = {} (correct: {ISSUE_302_Y}, {ISSUE_302_S})",
            f[0], f[2]
        ),
    }
    eprintln!("canary #302 (Verilator {version}): {line}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// #309 ve Verilator #9256: miktarı sabite bağlı bir port olan işaretli
/// `>>>`, 64 bitten geniş bileşik değerde. Volt bu biçimden kaçınamaz
/// (miktar Volt için sabit değil); test beklenen değeri değil, ölçülen
/// sürümün ölçülen yanlışını sınar. Ölçülen sürümün davranışı değişirse
/// test düşer: satırı yeniden ölçün; dördü de doğruysa #9256 o sürümde
/// düzelmiştir (ADR-0121; kitabın `limitations.md`'si).
#[test]
fn the_issue_9256_port_tied_shift_canary() {
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    let version = verilator_version(&verilator);
    let dir = temp_dir("issue9256");
    let out = run(
        &dir,
        &verilator,
        "issue9256.sv",
        ISSUE_9256,
        volt_tools::VERILATOR_BUILD_FLAGS,
    );
    let line = out
        .lines()
        .find(|l| l.starts_with("R9256 "))
        .unwrap_or_else(|| panic!("simülasyon `R9256` satırını basmadı:\n{out}"));
    let f: Vec<&str> = line.split_whitespace().skip(1).collect();
    assert_eq!(f.len(), 4, "{line}");
    let right: Vec<&str> = ["y65", "y97", "y129", "y66"]
        .iter()
        .zip(&f)
        .zip(ISSUE_9256_RIGHT)
        .filter(|&((_, got), want)| **got == *want)
        .map(|((name, _), _)| *name)
        .collect();
    match ISSUE_9256_MEASURED.iter().find(|(v, _)| *v == version) {
        Some((_, measured)) => assert_eq!(
            f, measured,
            "Verilator {version} no longer prints the measured values for \
             the port-tied signed shift (#309, Verilator #9256): measure it \
             again and update ISSUE_9256_MEASURED, the known limit in \
             ADR-0121 and book/src/limitations.md. Correct now: {right:?}.\n\
             line: {line}"
        ),
        None => eprintln!(
            "canary #9256: Verilator {version} has no measured row; correct \
             fields: {right:?}"
        ),
    }
    eprintln!("canary #9256 (Verilator {version}): {line}; correct: {right:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Verilator #9060: #145 ve #148 tasarımları DFG açıkken
/// (`VERILATOR_BUILD_FLAGS` olmadan). Volt bu biçimde derlemez; ölçüm
/// `-fno-dfg`'nin kaldırılma ölçütü içindir (ADR-0107, "Ek: DFG kapalı").
/// Ölçülen sürümün yanlış alanları değişirse test düşer: yeniden ölçün,
/// `DFG_ON_MEASURED`'ı ve ADR-0121'in ölçüm tablosunu güncelleyin.
#[test]
fn the_issue_9060_designs_with_dfg_on_are_measured() {
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    let version = verilator_version(&verilator);
    let mut wrong = Vec::new();
    for (issue, design) in [(145, ISSUE_145), (148, ISSUE_148)] {
        let tag = format!("dfg{issue}");
        let dir = temp_dir(&tag);
        let out = run(&dir, &verilator, &format!("{tag}.sv"), design, &[]);
        let line = out
            .lines()
            .find(|l| l.starts_with("R 0 17 "))
            .unwrap_or_else(|| panic!("simülasyon `R 0 17` satırını basmadı:\n{out}"));
        let hex = line.trim_start_matches("R 0 17 ").trim();
        let fields: Vec<&str> = decode(hex)
            .into_iter()
            .filter(|&(_, got, expected)| got != expected)
            .map(|(name, ..)| name)
            .collect();
        eprintln!("canary #{issue}, DFG on (Verilator {version}): wrong {fields:?}; {line}");
        wrong.push(fields);
        let _ = std::fs::remove_dir_all(&dir);
    }
    match DFG_ON_MEASURED.iter().find(|(v, ..)| *v == version) {
        Some(&(_, w145, w148)) => assert_eq!(
            (wrong[0].as_slice(), wrong[1].as_slice()),
            (w145, w148),
            "Verilator {version} with DFG on no longer gets the measured \
             fields of the #145 and #148 designs wrong (Verilator #9060): \
             measure again and update DFG_ON_MEASURED and ADR-0121"
        ),
        None => eprintln!(
            "canary #9060: Verilator {version} has no measured row; wrong with \
             DFG on: #145 {:?}, #148 {:?}",
            wrong[0], wrong[1]
        ),
    }
}

/// Sabitlenen sürümün (`scripts/ci/verilator.env`) her ölçüm tablosunda
/// satırı var: Verilator'ı güncelleyen PR hataları yeni sürümde ölçer ve
/// satırları ekler.
#[test]
fn the_pinned_verilator_release_has_a_measured_row_in_every_table() {
    let env = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/ci/verilator.env"),
    )
    .expect("scripts/ci/verilator.env");
    let pinned = env
        .lines()
        .find_map(|l| l.strip_prefix("VERILATOR_VERSION="))
        .expect("VERILATOR_VERSION satırı")
        .trim();
    assert!(
        ISSUE_302_MEASURED.iter().any(|(v, ..)| *v == pinned),
        "the pinned Verilator {pinned} has no #302 row in ISSUE_302_MEASURED"
    );
    assert!(
        ISSUE_9256_MEASURED.iter().any(|(v, _)| *v == pinned),
        "the pinned Verilator {pinned} has no row in ISSUE_9256_MEASURED"
    );
    assert!(
        DFG_ON_MEASURED.iter().any(|(v, ..)| *v == pinned),
        "the pinned Verilator {pinned} has no row in DFG_ON_MEASURED"
    );
}
