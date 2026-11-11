//! `@mmio` (ADR-0044) ve pipeline (ADR-0038) desugar geçitlerinin kör
//! noktaları: #206'nın hedefli mutasyon koşusunda (ADR-0109 §4, B) hiçbir
//! testin yakalamadığı mutantlar. Her test, mutantın değiştirdiği davranışı
//! adlandırır; mutantlar atılabilir bir worktree'de elle uygulandı, test
//! düştü, mutant geri alındı.
//!
//! Beklenen tanılar şartnameden, elle: ADR-0044 §2 ve §6 (`@reg`
//! argümanları, 32 bitlik adres alanı, birimde tanımlı olmayan AXI
//! bundle'larının üretilmesi, `@w1c` temizleyicisinin yeri) ve ADR-0038
//! (aşama referansı `0..=N-1` içinde, E5015 çevrimi adlandırır). Değerler
//! (simülasyon) Verilator'da; Verilator yoksa simülasyon testi atlanır,
//! `VOLT_REQUIRE_TOOLS=verilator` ile düşer (ADR-0079 §3).

mod tools;

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-blind-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Bir tanı: önem, kod, birincil satır, ileti.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Diag {
    severity: String,
    code: String,
    line: u64,
    message: String,
}

/// `volt check --format json`; tanılar kaynak sırasıyla.
fn check(tag: &str, src: &str) -> Vec<Diag> {
    let dir = temp_dir(tag);
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    let _ = std::fs::remove_dir_all(&dir);
    let env: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)));
    let mut v: Vec<Diag> = env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| Diag {
            severity: d["severity"].as_str().unwrap_or_default().to_string(),
            code: d["code"].as_str().unwrap_or_default().to_string(),
            line: d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0),
            message: d["message"].as_str().unwrap_or_default().to_string(),
        })
        .collect();
    v.sort();
    v
}

/// Hataların (kod, satır) listesi.
fn errors(diags: &[Diag]) -> Vec<(String, u64)> {
    diags
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| (d.code.clone(), d.line))
        .collect()
}

/// `marker` içeren ilk satırın numarası (1'den).
fn line_with(src: &str, marker: &str) -> u64 {
    src.lines()
        .position(|l| l.contains(marker))
        .map(|i| i as u64 + 1)
        .unwrap_or_else(|| panic!("işaret yok: {marker}"))
}

/// Tek register'lı bir `@mmio` modülü; `{BASE}` ve `{REG}` yerine konur.
const ONE_REG: &str = "@mmio(base = {BASE}, bus = AXI4Lite)
module G {
    in  clk : clock
    out en  : bool
    @reg({REG}) // reg
    control : {
        enable : bool,
        @reserved : bits<31>,
    }
    en = regs.control.enable
}
";

fn one_reg(base: &str, reg: &str) -> String {
    ONE_REG.replace("{BASE}", base).replace("{REG}", reg)
}

// ═══ @reg argümanları (ADR-0044 §2, §6: E0009) ═════════════════════════

/// `access` bir adla yazılır: `ReadWrite`, `ReadOnly`, `WriteOnly`.
/// `ReadWrite::X` çok parçalı bir yoldur, erişim adı değildir: E0009.
/// Mutasyon koşusunun kör noktası: `path_ident`'in tek parça koruması
/// `true` olunca yolun ilk parçası okunuyor, `ReadWrite::X` sessizce
/// `ReadWrite` sayılıyordu.
#[test]
fn an_access_value_with_a_path_prefix_is_e0009() {
    let src = one_reg("0x0", "offset = 0x00, access = ReadWrite::X");
    let got = check("access-path", &src);
    assert_eq!(
        errors(&got),
        vec![("E0009".to_string(), line_with(&src, "// reg"))],
        "{got:?}"
    );
}

/// Konumsal `@reg` argümanı yalnız `volatile` bayrağıdır; başka konumsal
/// değer E0009. Kör nokta: koruma `true` olunca `3` gibi her konumsal
/// argüman `volatile` sayılıyor, register donanımın sayılıp kabul
/// ediliyordu.
#[test]
fn a_positional_reg_argument_other_than_volatile_is_e0009() {
    let src = one_reg("0x0", "offset = 0x00, access = ReadOnly, 3");
    let got = check("positional", &src);
    assert_eq!(
        errors(&got),
        vec![("E0009".to_string(), line_with(&src, "// reg"))],
        "{got:?}"
    );
}

// ═══ 32 bitlik adres alanı (ADR-0044 §2, §6: E0015) ════════════════════

/// `offset` 4 bayta hizalıdır; register `base + offset` 32 bite sığdığı
/// sürece yerleşir (E0015 yalnız hizasız ya da aralık dışı offset için).
/// 0xFFFF_FFFF son adrestir, aralık içindedir. Kör nokta: sınır `>=`
/// olunca bu adres "aralık dışı" sayılıp E0015 alıyordu.
///
/// ADR-0044 bugün `base`'in hizalanmasını istemiyor; #220 hizasız tabanı
/// reddetmeyi öneriyor. Bu yüzden test yalnız E0015'in olmadığını sınar,
/// tanısız derlemeyi değil: hizalı tabanla `base + offset` 4'ün katıdır ve
/// 0xFFFF_FFFF'e hiç eşit olmaz. #220 hizasız tabanı reddedince `>=`
/// mutantı ulaşılamaz olur ve yeniden sınıflandırılır (ADR-0109 §4, A2).
#[test]
fn a_register_at_the_last_32_bit_address_is_not_out_of_range() {
    let src = one_reg("0xFFFF_FFFF", "offset = 0x00, access = ReadWrite");
    let got = check("last-address", &src);
    assert!(got.iter().all(|d| d.code != "E0015"), "{got:?}");
}

/// `base` 32 bitlik bir tam sayıdır (ADR-0044 §2): 0x1_0000_0000 E0009,
/// `@mmio` niteliğinin satırında. Kör nokta: koruma `true` olunca taban
/// kabul ediliyor, hata register'ın offset'inde E0015'e dönüyordu.
#[test]
fn a_base_wider_than_32_bits_is_e0009() {
    let src = one_reg("0x1_0000_0000", "offset = 0x00, access = ReadWrite");
    let got = check("wide-base", &src);
    assert_eq!(errors(&got), vec![("E0009".to_string(), 1)], "{got:?}");
}

/// 32 bitin ötesindeki adres aralık dışıdır: E0015, `@reg`'in satırında.
/// Kör nokta: sınır `==` olunca 0x1_0000_0000 yerleşiyor, hata üretilen
/// metinde E2010 olarak çıkıyordu.
#[test]
fn an_offset_past_the_32_bit_address_space_is_e0015() {
    let src = one_reg("0x0", "offset = 0x1_0000_0000, access = ReadWrite");
    let got = check("past-space", &src);
    assert_eq!(
        errors(&got),
        vec![("E0015".to_string(), line_with(&src, "// reg"))],
        "{got:?}"
    );
}

// ═══ AXI bundle'ları (ADR-0044 §2) ═════════════════════════════════════

/// Birimde tanımlı olmayan AXI bundle'ları üretilir. Başka adlı bir tip
/// takma adı onları tanımlamaz: tasarım tanısız derlenir. Kör nokta:
/// `unit_has_item`'ın takma ad karşılaştırması `!=` olunca her takma ad
/// bütün bundle'ları "tanımlı" yapıyor, üretilen modül 42 E1001 alıyordu.
#[test]
fn a_type_alias_in_the_unit_does_not_hide_the_axi_bundles() {
    let src = format!(
        "type Word = u32\n\n{}",
        one_reg("0x0", "offset = 0x00, access = ReadWrite")
    );
    let got = check("alias", &src);
    assert_eq!(got, vec![], "{got:?}");
}

// ═══ Üretilen kaynaktaki tanıların konumu (ADR-0044 §2; ADR-0109 §4 A2) ═

/// İnsan biçimindeki `volt check` çıktısından, konumu `file` önekiyle
/// başlayan her tanı: (başlık satırı, konum, gösterilen kaynak satırının
/// numarası ve metni, altı çizili parça). Altı çizili parça, kaynak
/// satırının `│`'den sonraki metninde, bir sonraki satırın `^`'lerinin
/// sütunlarıdır.
fn rendered_spans(src: &str, file: &str) -> Vec<(String, String, u64, String, String)> {
    rendered_unit_spans(&[("case.volt", src)], file)
}

/// `rendered_spans`'in çok dosyalı biçimi: dosyalar aynı dizine yazılır,
/// `volt check` ilk dosyayı (giriş) denetler; ötekiler `use` ile yüklenir.
fn rendered_unit_spans(
    files: &[(&str, &str)],
    file: &str,
) -> Vec<(String, String, u64, String, String)> {
    // Paralel testler aynı dizini paylaşmasın.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = temp_dir(&format!("rendered-{n}"));
    for (name, src) in files {
        std::fs::write(dir.join(name), src).expect("yaz");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("NO_COLOR", "1")
        .args(["--lang", "en", "check", files[0].0])
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    let _ = std::fs::remove_dir_all(&dir);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let after_bar = |l: &str| l.split_once("│ ").map(|(_, r)| r.to_string());
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    let mut header = String::new();
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("error[") || l.starts_with("warning[") {
            header = l.to_string();
            continue;
        }
        let Some(loc) = l.trim_start().strip_prefix("┌─ ") else {
            continue;
        };
        if !loc.starts_with(file) {
            continue;
        }
        // Konumdan sonra: (boş gutter), `N │ kaynak`, `  │ ^^^ etiket`.
        let Some(j) = (i + 1..lines.len()).find(|&j| {
            lines[j]
                .trim_start()
                .split_once(" │")
                .is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty())
        }) else {
            panic!("{loc}: kaynak satırı yok\n{text}");
        };
        let (num, _) = lines[j].trim_start().split_once(" │").unwrap();
        let code = after_bar(lines[j]).unwrap_or_default();
        let carets = after_bar(lines[j + 1]).unwrap_or_default();
        let start = carets
            .find('^')
            .unwrap_or_else(|| panic!("{loc}: ^ yok\n{text}"));
        let len = carets[start..].chars().take_while(|&c| c == '^').count();
        let under: String = code.chars().skip(start).take(len).collect();
        found.push((
            header.clone(),
            loc.to_string(),
            num.parse().unwrap(),
            code,
            under,
        ));
    }
    found
}

/// Üretilen `@mmio` kaynağına (`<mmio:G>`) düşen bir tanının konumu, aynı
/// tanıda gösterilen kaynak parçasında adlandırdığı adı gösterir. Bu,
/// `render_contracts`'ın kontrat bölümündeki yorum satırını koşula bağlayan
/// `!`'in mutantının (A2, `exclude_re`) varsayımını sınar. Mutant o yorumu
/// atar ve sonraki her satırı bir yukarı kaydırır (55 → 54). Tanı yine de
/// doğru adı gösterir, çünkü konum ve gösterilen parça ayrıştırılanla aynı
/// üretilen metinden gelir: sürücü de LSP de (`analysis.rs`)
/// `ParseResult.generated`'ın metnini kaydeder. Metin diske yazılmaz
/// (ADR-0044 §2), `--emit-mmio` gibi bir dökümü yok; satır numarası bu
/// tanının dışında bir şey göstermez. Kaydedilen metin ayrıştırılandan
/// ayrılırsa (ya da metin diske yazılırsa) bu test düşer ve A2 girdisi
/// yeniden incelenir.
///
/// Tetikleyici: birimde tanımlı `AxiReadAddr` bundle'ı. Bugün bu girdi
/// üretilen kodda `ar_*` adları için E1001 alıyor (#244). Bu tanılar hem
/// yorumdan önceki kontratlarda hem sonraki `cover`'da ve mantık
/// satırlarındadır. #244 düzelince bu girdi derlenir ve test düşer: yeni
/// bir tetikleyici gerekir. İkinci tetikleyici yazılabilir register'ı
/// olmayan bir haritadır. Orada `w.data` W1001 alır (ADR-0044 §2) ve
/// gösterilen satır `in w : AxiWriteData` port bildirimidir. Düzleştirilmiş
/// alanın sütunu bugün keyfidir (#246); bu yüzden yalnız satır sınanır.
#[test]
fn a_diagnostic_in_the_generated_mmio_source_shows_the_name_it_reports() {
    let src = "pub struct port AxiReadAddr {
    out addr  : u32
    out prot  : u3
    out valid : bool
    in  ready : bool
}

@mmio(base = 0x0, bus = AXI4Lite)
module G {
    in  clk : clock
    out en  : bool
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable : bool,
        @reserved : bits<31>,
    }
    en = regs.control.enable
}
";
    let spans = rendered_spans(src, "<mmio:G>:");
    let e1001: Vec<_> = spans
        .iter()
        .filter(|(h, ..)| h.starts_with("error[E1001]"))
        .collect();
    assert!(e1001.len() >= 5, "{spans:#?}");
    for (header, loc, _, code, under) in &e1001 {
        let name = header.split('\'').nth(1).unwrap_or_default();
        assert_eq!(under, name, "{loc}: {code}");
    }
    // Yorumdan sonraki satırlar da sınandı: bir `cover` ve bir `let`.
    assert!(
        e1001
            .iter()
            .any(|(.., code, _)| code.trim_start().starts_with("cover:")),
        "{spans:#?}"
    );
    assert!(
        e1001
            .iter()
            .any(|(.., code, _)| code.trim_start().starts_with("let ")),
        "{spans:#?}"
    );

    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    in  tick : bool
    out cnt  : u16
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        count   : u16,
        @reserved : bits<16>,
    }
    on clk {
        if tick {
            regs.status.count <= regs.status.count + 1
        }
    }
    cnt = regs.status.count
}
";
    let spans = rendered_spans(src, "<mmio:T>:");
    let w: Vec<_> = spans
        .iter()
        .filter(|(h, ..)| h.contains("unused input port: 'w."))
        .collect();
    assert_eq!(w.len(), 2, "{spans:#?}");
    for (_, loc, _, code, _) in &w {
        let tokens: Vec<&str> = code.split_whitespace().collect();
        assert_eq!(tokens, ["in", "w", ":", "AxiWriteData"], "{loc}");
    }
}

/// Bir tanının gösterdiği kaynak satırı, iletisinin adlandırdığı adı taşır:
/// E1001'de altı çizili parça o addır, W1001'de (port kullanılmadı)
/// satır o portun bildirimidir (`in <port> : …`; düzleştirilmiş alanın
/// sütunu keyfi, #246).
fn assert_shows_its_name(spans: &[(String, String, u64, String, String)]) {
    for (header, loc, _, code, under) in spans {
        let name = header.split('\'').nth(1).unwrap_or_default();
        if header.starts_with("error[E1001]") {
            assert_eq!(under, name, "{loc}: {code}");
        } else if header.contains("unused input port") {
            let port = name.split('.').next().unwrap_or_default();
            let tokens: Vec<&str> = code.split_whitespace().collect();
            assert_eq!(tokens.get(..2), Some(&["in", port][..]), "{loc}: {code}");
        }
    }
}

/// Bir birimin her `@mmio` modülü kendi üretilen kaynağını alır
/// (`<mmio:TimerRegs>`, `<mmio:GpioRegs>`), her biri ayrı bir `FileId` ile:
/// iki modülün tanıları kendi metnini gösterir. Kör nokta: sentetik dosya
/// sayacının `+=`'si `*=` olunca iki kaynak aynı `FileId`'yi paylaşıyor,
/// GpioRegs'in `w.data` uyarısı `<mmio:TimerRegs>:7:3`'te `out prot : u3`
/// satırını gösteriyordu.
///
/// Birim bugün #220 yüzünden derlenmiyor (ikinci modül AXI bundle'larını
/// bulamıyor, E1001). Test tanıların sayısını değil yalnız gösterdikleri
/// yeri sınar; #220 düzelince iki modülde de `w.data`/`w.strb` W1001'i
/// (yazılabilir register yok, ADR-0044 §2) kalır ve özellik onlarla sınanır.
#[test]
fn each_mmio_module_of_a_unit_has_its_own_generated_source() {
    let timer = "@mmio(base = 0x100, bus = AXI4Lite)
pub module TimerRegs {
    in  clk  : clock
    in  tick : bool
    out cnt  : u16
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        count : u16,
        @reserved : bits<16>,
    }
    on clk {
        if tick {
            regs.status.count <= regs.status.count + 1
        }
    }
    cnt = regs.status.count
}
";
    let main = "use timer::TimerRegs;

@mmio(base = 0x0, bus = AXI4Lite)
module GpioRegs {
    in  clk : clock
    out en  : bool
    @reg(offset = 0x00, access = ReadOnly, volatile)
    control : {
        enable : bool,
        @reserved : bits<31>,
    }
    en = regs.control.enable
}
";
    let spans = rendered_unit_spans(&[("main.volt", main), ("timer.volt", timer)], "<mmio:");
    for module in ["TimerRegs", "GpioRegs"] {
        let prefix = format!("<mmio:{module}>:");
        assert!(
            spans.iter().any(|(_, loc, ..)| loc.starts_with(&prefix)),
            "{module}: {spans:#?}"
        );
    }
    assert_shows_its_name(&spans);
}

// ═══ Yazma etkinliği ve @w1c (ADR-0044 §2, §4) ═════════════════════════

/// Bus'ın yazdığı register da `@w1c` alanı da yoksa üretilen kodda yazma
/// etkinliği (`mmio_we`) yoktur: okuyanı olmayan bir bağlama olurdu. Tek
/// uyarılar yazılabilir register'ı olmayan bloğun kullanmadığı `w.data` ve
/// `w.strb`'dir (ADR-0044 §2, W1001). Kör nokta: `has_w1c` `true` olunca
/// `mmio_we` üretiliyor, kullanıcı kendi yazmadığı bir ad için "unused
/// binding: 'mmio_we'" uyarısı görüyordu.
#[test]
fn a_map_without_bus_writes_or_w1c_has_no_write_enable() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    in  tick : bool
    out cnt  : u16
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        count   : u16,
        @reserved : bits<16>,
    }
    on clk {
        if tick {
            regs.status.count <= regs.status.count + 1
        }
    }
    cnt = regs.status.count
}
";
    let got = check("no-we", src);
    let messages: Vec<&str> = got.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        ["unused input port: 'w.data'", "unused input port: 'w.strb'"],
        "{got:?}"
    );
    assert!(got.iter().all(|d| d.code == "W1001"), "{got:?}");
}

/// `@w1c` temizleyicisi alanı yazan kullanıcı `on` bloğunun başına girer
/// (ADR-0044 §2 adım 4): alanın tek sürücüsü o bloktur. Alanı ikinci `on`
/// bloğu bir `if` içindeki `for`'da yazar; ilk blok bir `if` içinde başka
/// bir alanı yazar. Tasarım hatasız derlenir. Kör noktalar (hepsinde
/// temizleyici yanlış bloğa ya da kendi bloğuna gidiyor, alan iki blokta
/// sürülüyordu, E4001): `block_assigns`'ın ad karşılaştırması `!=`,
/// `block_assigns` ya da `if_assigns` her zaman `true`, `block_assigns`'ın
/// `for` kolunun silinmesi.
#[test]
fn a_w1c_clear_goes_into_the_on_block_that_sets_the_field() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    in  tick : bool
    out irq  : bool
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        count   : u16,
        expired : bool @w1c,
        @reserved : bits<15>,
    }
    on clk {
        if tick {
            regs.status.count <= regs.status.count + 1
        }
    }
    on clk {
        if tick {
            for i in 0..1 {
                regs.status.expired <= true
            }
        }
    }
    irq = regs.status.expired
}
";
    let got = check("w1c-block", src);
    assert_eq!(errors(&got), vec![], "{got:?}");
}

/// Alanı bir `match` kolunda yazan `on` bloğu da alanı yazan bloktur:
/// temizleyici oraya girer, tasarım hatasız derlenir. Kör nokta:
/// `block_assigns`'ın `match` kolu silinince temizleyici kendi bloğunda
/// kalıyor, alan iki blokta sürülüyordu (E4001).
#[test]
fn a_w1c_clear_goes_into_an_on_block_that_sets_the_field_in_a_match() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    in  sel  : u2
    out irq  : bool
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        count   : u16,
        expired : bool @w1c,
        @reserved : bits<15>,
    }
    on clk {
        regs.status.count <= regs.status.count + 1
    }
    on clk {
        match sel {
            3 => { regs.status.expired <= true }
            _ => { }
        }
    }
    irq = regs.status.expired
}
";
    let got = check("w1c-match", src);
    assert_eq!(errors(&got), vec![], "{got:?}");
}

/// Bus'ın yazdığı register yoksa da `@w1c` alanının temizleyicisi yazma
/// etkinliğini (`mmio_we`) okur, üretilir: tasarım tanısız derlenir. Kör
/// nokta: `has_w1c` `false` olunca `mmio_we` üretilmiyor, temizleyici
/// E1001 alıyordu.
#[test]
fn a_map_with_only_a_w1c_flag_has_a_write_enable() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    in  tick : bool
    out irq  : bool
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        expired : bool @w1c,
        @reserved : bits<31>,
    }
    on clk {
        if tick {
            regs.status.expired <= true
        }
    }
    irq = regs.status.expired
}
";
    let got = check("w1c-only", src);
    assert_eq!(errors(&got), vec![], "{got:?}");
}

// ═══ Alan tipleri ve nitelikleri (ADR-0044 §2, §6) ═════════════════════

/// Alan tipi `bool`, `bits<N>` ya da `uN` (N ≤ 32); `uint<N>`, `uN`'in
/// genişliği ifade olan eşidir (ADR-0041): `uint<8>` alanı kabul edilir.
/// Kör nokta: `field_ty`'nin `UIntN` kolu silinince alan E0015 alıyordu.
#[test]
fn a_uint_n_register_field_is_accepted() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk : clock
    out v   : u8
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        value : uint<8>,
        @reserved : bits<24>,
    }
    v = regs.control.value
}
";
    let got = check("uint-field", src);
    assert_eq!(got, vec![], "{got:?}");
}

/// `u40` desteklenmeyen bir alan tipidir (N ≤ 32): E0015, ileti tip
/// kuralını söyler. Kör nokta: `field_ty`'nin `N <= 32` koruması `true`
/// olunca alan kabul ediliyor, tanı "alanlar toplamı 32 biti aşıyor"
/// iletisine dönüyordu (ADR-0044 §6'da iki ayrı neden).
#[test]
fn a_field_wider_than_32_bits_is_an_unsupported_type() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    out en   : bool
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        value : u40, // field
    }
    en = regs.control.value == 0
}
";
    let got = check("u40-field", src);
    let e: Vec<(&str, u64, &str)> = got
        .iter()
        .filter(|d| d.severity == "error")
        .map(|d| (d.code.as_str(), d.line, d.message.as_str()))
        .collect();
    assert_eq!(
        e,
        [(
            "E0015",
            line_with(src, "// field"),
            "register field type must be bool, bits<N> or uN (N <= 32)"
        )],
        "{got:?}"
    );
}

/// `@w1c` ve `@self_clearing` yalnız adlandırılmış `bool` alana uygulanır;
/// `u16` alanında tek başına `@w1c` E0009. Kör nokta: koşulun `||`'si `&&`
/// olunca yalnız iki nitelik birlikteyken denetleniyor, `@w1c` `u16`'ya
/// geçip tip hatası (E2003) üretilen metinde çıkıyordu.
#[test]
fn w1c_on_a_non_bool_field_is_e0009() {
    let src = "@mmio(base = 0x0, bus = AXI4Lite)
module T {
    in  clk  : clock
    out cnt  : u16
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        count   : u16 @w1c, // field
        @reserved : bits<16>,
    }
    on clk {
        regs.status.count <= regs.status.count + 1
    }
    cnt = regs.status.count
}
";
    let got = check("w1c-u16", src);
    assert_eq!(
        errors(&got),
        vec![("E0009".to_string(), line_with(src, "// field"))],
        "{got:?}"
    );
}

/// `tests/ui/fail/138`–`141`: iki `@mmio` modülü ya da bir modülün
/// register'ları aynı sürücü adlarına inerse E1014, fikstürün `//~^ ERROR`
/// işaretinin bir üstündeki satırda ve tek tanı olarak. Bu dört fikstürü
/// hiçbir test koşmuyordu. Kör nokta: `check_driver_stem` hiç çalışmayınca
/// 141'de E1014 kayboluyor, yerine #220'nin 42 E1001'i çıkıyordu.
#[test]
fn ui_fail_138_to_141_mmio_driver_name_collisions_are_e1014() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ui/fail");
    for name in [
        "138_mmio_raw_field_collision.volt",
        "139_mmio_register_field_path_collision.volt",
        "140_mmio_driver_own_name.volt",
        "141_mmio_module_file_stem_collision.volt",
    ] {
        let src = std::fs::read_to_string(root.join(name)).expect("fikstür");
        assert!(src.starts_with("//~ E1014"), "{name}");
        let marker = line_with(&src, "//~^ ERROR");
        let got = check(&format!("ui-{}", &name[..3]), &src);
        assert_eq!(
            errors(&got),
            vec![("E1014".to_string(), marker - 1)],
            "{name}: {got:?}"
        );
    }
}

// ═══ Pipeline aşama referansları (ADR-0038) ════════════════════════════

/// `stage(+k)` aşama `i`'den `i + k`'ya düşer; `0..=N-1` dışı E5012. İki
/// aşamalı pipeline'ın son aşamasında (1) `stage(+1)` 2'ye düşer: E5012,
/// referansın satırında. Kör nokta: son aşama `N - 1` yerine `N / 1`
/// olunca referans sessizce kabul ediliyordu.
#[test]
fn a_relative_stage_reference_past_the_last_stage_is_e5012() {
    let src = "pipeline(2) P {
    in  clk : clock
    in  x : u32
    out y : u32
    stage F { let a : u32 = x }
    stage D {
        let b : u32 = a + 1
        let c : u32 = stage(+1).b // ref
    }
    y = stage(D).b + stage(D).c
}
";
    let got = check("past-last", src);
    assert_eq!(
        errors(&got),
        vec![("E5012".to_string(), line_with(src, "// ref"))],
        "{got:?}"
    );
}

/// Stall listesi ilk aşamadan başlayan bitişik bir önektir (ADR-0038 §4):
/// `stall F, E` (D atlanmış) E5013. Kör nokta: önek koşulunun `&&`'si `||`
/// olunca ilk aşamayı içeren her liste önek sayılıp kabul ediliyordu.
#[test]
fn a_stall_list_with_a_gap_is_e5013() {
    let src = "pipeline(3) P {
    in  clk : clock
    in  x : u32
    in  c : bool
    out y : u32
    stage F { let a : u32 = x }
    stage D { let b : u32 = a + 1 }
    stage E { let e : u32 = b + 1 }
    stall F, E when c // stall
    y = stage(E).e
}
";
    let got = check("stall-gap", src);
    assert_eq!(
        errors(&got),
        vec![("E5013".to_string(), line_with(src, "// stall"))],
        "{got:?}"
    );
}

/// Modül düzeyindeki `on` bloğunda, bir `if`'in içindeki aşama referansı
/// da yeniden yazılır (`stage(D).b` → D'nin değeri): tasarım tanısız
/// derlenir. Kör noktalar: `rw_block` ya da `rw_if` hiçbir şey yapmayınca
/// referans yazılmadan kalıyor, tasarım yalnız "unused binding: 'b'"
/// uyarısıyla kabul ediliyordu.
#[test]
fn a_stage_reference_inside_an_if_of_an_on_block_is_rewritten() {
    let src = "pipeline(2) P {
    in  clk : clock
    in  x : u32
    in  c : bool
    out y : u32
    stage F { let a : u32 = x }
    stage D { let b : u32 = a + 1 }
    reg r : u32 = 0
    on clk {
        if c {
            r <= stage(D).b
        }
    }
    y = r
}
";
    let got = check("on-if-ref", src);
    assert_eq!(got, vec![], "{got:?}");
}

/// `on` bloğundaki bir `match` kolunun aşama referansı da yeniden yazılır:
/// tasarım tanısız derlenir. Kör nokta: `collect_arm_idxs` hiçbir şey
/// toplamayınca kollar gezilmiyor, referans yazılmadan kalıyor, tasarım
/// yalnız "unused binding: 'b'" uyarısıyla kabul ediliyordu.
#[test]
fn a_stage_reference_inside_a_match_arm_of_an_on_block_is_rewritten() {
    let src = "pipeline(2) P {
    in  clk : clock
    in  x : u32
    in  sel : u2
    out y : u32
    stage F { let a : u32 = x }
    stage D { let b : u32 = a + 1 }
    reg r : u32 = 0
    on clk {
        match sel {
            0 => { r <= stage(D).b }
            _ => { r <= 0 }
        }
    }
    y = r
}
";
    let got = check("on-match-ref", src);
    assert_eq!(got, vec![], "{got:?}");
}

/// Muhafız sinyalleri (`stall_<aşama>`) yalnız kullanılıyorsa üretilir;
/// kullanıcının modül düzeyi `on` bloğunda okuduğu `stall_f` de
/// kullanımdır (D'deki stall F'yi de durdurur, `stall_f` iç mantıkta
/// okunmaz). Tasarım tanısız derlenir. Kör nokta: `collect_block_names`
/// hiçbir şey toplamayınca `on` gövdesindeki okuma görülmüyor, `stall_f`
/// üretilmiyor, kullanım E1001 alıyordu.
#[test]
fn a_stall_guard_read_only_in_an_on_block_is_generated() {
    let src = "pipeline(2) P {
    in  clk : clock
    in  x : u32
    out y : u32
    out s : bool
    stage F { let a : u32 = x }
    stage D {
        let b : u32 = a + 1
        stall when b == 0
    }
    reg r : bool = false
    on clk {
        r <= stall_f
    }
    y = stage(D).b
    s = r
}
";
    let got = check("guard-on", src);
    assert_eq!(got, vec![], "{got:?}");
}

/// E5015 çevrimdeki değerleri adlandırır: `a → b`. Çevrime girmeyen `c`
/// iletide yoktur. Kör nokta: süzgecin `!`'i silinince ileti çevrimdekileri
/// değil sıralanabilenleri sayıyordu ("…references: " boş).
///
/// Tanılar yalnız bu E5015'tir; `z = nope` E1001 almaz: ayrıştırma hatası
/// derlemeyi ad çözümlemesinden önce durdurur. `order_lets`'in E5015'ten
/// sonraki `!done[i]` döngüsünün `!` mutantı (ADR-0109 §4, A2) bu
/// varsayıma dayanır: mutantta çevrimdeki `a`, `b` gövdeden düşer ama bunu
/// gösterecek çözümleme koşmaz. Çözümleme ayrıştırma hatasından sonra da
/// koşmaya başlarsa bu test düşer ve A2 girdisi yeniden incelenir.
#[test]
fn e5015_names_the_values_on_the_cycle() {
    let src = "pipeline(2) P {
    in  clk : clock
    in  x : u32
    out y : u32
    out z : u32
    stage F {
        let a : u32 = stage(D).b
        let c : u32 = x
    }
    stage D { let b : u32 = stage(F).a + c }
    y = stage(D).b
    z = nope
}
";
    let got = check("cycle", src);
    let all: Vec<(&str, &str)> = got
        .iter()
        .map(|d| (d.code.as_str(), d.message.as_str()))
        .collect();
    assert_eq!(
        all,
        [(
            "E5015",
            "combinational cycle through stage references: a → b"
        )],
        "{got:?}"
    );
}

/// `uint<N>` (ADR-0041) bir aşama sınırını geçer: sınır register'ı değerin
/// tipini taşır, tasarım tanısız derlenir. Kör nokta: `copy_ty`'nin
/// `UIntN` kolu silinince register'ın tipi kayboluyor, sıfır başlangıç
/// değeri E2005 alıyordu.
#[test]
fn a_uint_n_value_crosses_a_stage_boundary() {
    let src = "const W : u32 = 8
pipeline(2) P {
    in  clk : clock
    in  x : uint<W>
    out y : uint<W>
    stage F { let a : uint<W> = x }
    stage D { let b : uint<W> = a }
    y = stage(D).b
}
";
    let got = check("uint-n", src);
    assert_eq!(got, vec![], "{got:?}");
}

/// `sint<N>` de bir aşama sınırını geçer (ADR-0041, ADR-0038). Kör nokta:
/// `copy_ty`'nin `SIntN` kolu silinince sıfır başlangıç değeri E2005
/// alıyordu. (`bits<K>` aynı yolda bugün E2003 alıyor: #245.)
#[test]
fn a_sint_n_value_crosses_a_stage_boundary() {
    let src = "const W : u32 = 8
pipeline(2) P {
    in  clk : clock
    in  x : sint<W>
    out y : sint<W>
    stage F { let a : sint<W> = x }
    stage D { let b : sint<W> = a }
    y = stage(D).b
}
";
    let got = check("sint-n", src);
    assert_eq!(got, vec![], "{got:?}");
}

// ═══ Üretilen @mmio metninin iki değişmezi (ADR-0044 §2; ADR-0109 §4 A2) ═

/// `@mmio` tasarımları: depodaki bütün geçerli örnekler.
fn mmio_designs() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    [
        "tests/ui/pass/57_mmio_basic.volt",
        "tests/ui/pass/58_mmio_access_control.volt",
        "tests/ui/pass/72_mmio_driver_generation.volt",
        "examples/soc/gpio.volt",
    ]
    .iter()
    .map(|f| {
        let src = std::fs::read_to_string(root.join(f)).expect("tasarım");
        (f.to_string(), src)
    })
    .collect()
}

/// Bir register'ın alanları (rezerve dolgu dahil) en düşük bitten başlayıp
/// sırayla yerleşir (ADR-0044 §2): bit aralıkları ayrıktır ve 32 biti
/// aşmaz. `RegInfo::read_mask`'ın `|` → `^` mutantı (A2, `exclude_re`) bu
/// varsayıma dayanır: ayrık maskelerde OR ile XOR aynıdır.
#[test]
fn mmio_fields_have_disjoint_bit_ranges() {
    for (name, src) in mmio_designs() {
        let parsed = volt_syntax::parser::parse(volt_span::FileId(0), &src);
        assert!(
            parsed.diagnostics.is_empty(),
            "{name}: {:?}",
            parsed.error_codes()
        );
        assert!(!parsed.regmaps.is_empty(), "{name}");
        for map in &parsed.regmaps {
            for reg in &map.registers {
                let mut used = 0u64;
                for f in &reg.fields {
                    assert!(f.lsb + f.width <= 32, "{name}: {}.{}", reg.name, f.name);
                    let mask = ((1u64 << f.width) - 1) << f.lsb;
                    assert_eq!(used & mask, 0, "{name}: {}.{} örtüşüyor", reg.name, f.name);
                    used |= mask;
                }
            }
        }
    }
}

/// Üretilen modülün gövdesinde bütün `reg`/`let` bildirimleri ilk başka
/// deyimden (`on`, atama) öncedir: ADR-0044 §2 bildirimleri kullanıcı
/// gövdesinin önüne, `on` bloğunu ve atamaları arkasına koyar.
/// `parse_generated`'ın `tail.is_empty()` korumasının `true` mutantı (A2,
/// `exclude_re`) bu varsayıma dayanır: başka deyimden sonra gelen bir
/// bildirim olmadıkça koruma hiçbir şeyi değiştirmez.
#[test]
fn generated_mmio_declarations_come_before_every_other_statement() {
    use volt_ast::{ItemKind, StmtKind};
    for (name, src) in mmio_designs() {
        let parsed = volt_syntax::parser::parse(volt_span::FileId(0), &src);
        assert!(!parsed.generated.is_empty(), "{name}");
        for g in &parsed.generated {
            let gen = volt_syntax::parser::parse(volt_span::FileId(1), &g.text);
            assert!(
                gen.diagnostics.is_empty(),
                "{name}: {:?}",
                gen.error_codes()
            );
            let module = gen
                .ast
                .items
                .iter()
                .find_map(|&i| match &gen.ast.items_arena[i].kind {
                    ItemKind::Module(m) => Some(m),
                    _ => None,
                })
                .expect("üretilen modül");
            let mut seen_other = false;
            for &si in &module.body {
                let decl = matches!(gen.ast.stmts[si].kind, StmtKind::Reg(_) | StmtKind::Let(_));
                assert!(
                    !(decl && seen_other),
                    "{name}: başka deyimden sonra bildirim"
                );
                seen_other |= !decl;
            }
            assert!(seen_other, "{name}: gövdede `on`/atama yok");
        }
    }
}

// ═══ Stall, bubble ve flush değerleri (ADR-0038; ADR-0109 §4 A2) ══════

/// İki pipeline. `P`: D'de `stall when hold` (önek F, D), E'de
/// `stall when hold2` (önek F, D, E), `flush F when fl`. `Q`: yalnız D'de
/// `stall when hold`. Değerler: `a` = x, `b` = a + 1, `c` = b + 1, y = c.
const STALL_BUBBLE_FLUSH: &str = r#"pipeline(3) P {
    in  clk   : clock
    in  x     : u8
    in  hold  : bool
    in  hold2 : bool
    in  fl    : bool
    out y     : u8
    stage F { let a : u8 = x }
    stage D {
        let b : u8 = a + 1
        stall when hold
    }
    stage E {
        let c : u8 = b + 1
        stall when hold2
    }
    flush F when fl
    y = stage(E).c
}
pipeline(3) Q {
    in  clk  : clock
    in  x    : u8
    in  hold : bool
    out y    : u8
    stage F { let a : u8 = x }
    stage D {
        let b : u8 = a + 1
        stall when hold
    }
    stage E { let c : u8 = b + 1 }
    y = stage(E).c
}
test "two stall prefixes, bubble and flush" {
    let dut = P { };
    dut.hold = false;
    dut.hold2 = false;
    dut.fl = false;
    dut.x = 10;
    step(1);
    assert_eq(dut.y, 2);
    dut.x = 20;
    step(1);
    assert_eq(dut.y, 12);
    dut.x = 30;
    dut.hold = true;
    step(1);
    assert_eq(dut.y, 1);
    dut.x = 40;
    dut.hold = false;
    step(1);
    assert_eq(dut.y, 22);
    dut.x = 50;
    dut.hold2 = true;
    step(1);
    assert_eq(dut.y, 22);
    dut.x = 60;
    dut.hold = true;
    step(1);
    assert_eq(dut.y, 22);
    dut.x = 70;
    dut.hold = false;
    dut.hold2 = false;
    step(1);
    assert_eq(dut.y, 42);
    dut.x = 80;
    dut.fl = true;
    step(1);
    assert_eq(dut.y, 72);
    dut.x = 90;
    dut.fl = false;
    step(1);
    assert_eq(dut.y, 2);
}
test "one stall prefix, bubble" {
    let dut = Q { };
    dut.hold = false;
    dut.x = 10;
    step(1);
    assert_eq(dut.y, 2);
    dut.x = 20;
    step(1);
    assert_eq(dut.y, 12);
    dut.x = 30;
    dut.hold = true;
    step(1);
    assert_eq(dut.y, 1);
    dut.x = 40;
    dut.hold = false;
    step(1);
    assert_eq(dut.y, 22);
}
"#;

/// Pipeline'ın sınır register'ları ADR-0038'in önceliğiyle yazılır: flush >
/// hold > bubble > normal. Durdurulan önek 0..=k'da k'dan önceki sınırlar
/// tutar, k sınırına bubble (0) girer. Bu test `on clk` gövdesini kuran
/// fonksiyonların iki A2 mutantının (ADR-0109 §4, `exclude_re`) kanıtıdır.
/// İkisi de SV'yi değiştirir, değeri ve lint sonucunu değiştirmez:
///
/// - `bubble_cond`'un `ends_at[b] == stall_conds[b]` karşılaştırması `!=`:
///   eşit kümede `stall_<b>` adı yerine aynı koşulların OR'u yazılır;
///   eşit olmayanda (P'nin D→E sınırı: hold biter, hold2 daha geç biter)
///   `stall_d` = hold || hold2 yazılır. Fark yalnız hold2 doğruyken çıkar,
///   o zaman da önceki dal (`!stall_e` tutması) kazanır. Varsayım: bubble
///   koşulu yalnız bir sonraki aşamanın tutmasından sonra sınanır
///   (`build_on_block`'un iki dalı).
/// - `if_else`'in tek `if`'lik `else`'i `else if`'e düzleştiren koruması
///   `false`: flush'ın `else`'i `else begin if … end end` olur.
///
/// Beklenen (elle; register'lar 0'dan başlar, her `step(1)` bir kenar):
/// P: x=10 → a_r=10, b_r=0+1, y=2. x=20 → y=12. hold: a_r tutar, b_r'ye
/// bubble, y=0+1=1. Bırak, x=40 → a_r=40, b_r=20+1, y=22. hold2: a_r ve
/// b_r tutar, y=22; hold ve hold2 birlikte: tutma bubble'ı yener, y=22.
/// Bırak, x=70 → a_r=70, b_r=41, y=42. fl, x=80 → a_r'ye 0 (flush), b_r=71,
/// y=72. x=90 → b_r=0+1, y=2. Q: 2, 12, 1, 22.
///
/// Ayrıca `volt check` tanısızdır ve üretilen `P.sv`, `Q.sv`
/// `verilator --lint-only -Wall`'dan bulgusuz geçer.
#[test]
fn pipeline_stall_bubble_and_flush_have_their_values() {
    let got = check("pipe-check", STALL_BUBBLE_FLUSH);
    assert_eq!(got, vec![], "{got:?}");
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    sim_and_lint("pipe", STALL_BUBBLE_FLUSH, 2, &verilator, &["P", "Q"]);
}

/// `src`'yi `volt test` ile koşar (`passed` test geçmeli, hiçbiri
/// düşmemeli), sonra `volt build`'in her `tops` modülünün SV'sini
/// `verilator --lint-only -Wall` ile denetler: uyarı ya da hata satırı yok.
fn sim_and_lint(tag: &str, src: &str, passed: usize, verilator: &std::path::Path, tops: &[&str]) {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("case.volt"), src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "case.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // CI günlüğünde testin Verilator'da koştuğu görünsün (atlanan test de
    // "ok" yazar).
    eprintln!("--- volt test (desugar_blind_spot_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );

    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .args([
            "--lang",
            "en",
            "build",
            "case.volt",
            "--target-dir",
            "rtl-build",
        ])
        .current_dir(&dir)
        .output()
        .expect("volt build");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rtl = dir.join("rtl-build").join("rtl");
    for top in tops {
        let out = Command::new(verilator)
            .args(["--lint-only", "-Wall", "--top-module", top])
            .arg(format!("{top}.sv"))
            .current_dir(&rtl)
            .output()
            .expect("verilator çalışmalı");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        eprintln!("--- verilator --lint-only -Wall {top}.sv\n{text}");
        // 5.05x `--lint-only`'de de "Verilation Report" özeti basar;
        // ölçüt uyarı/hata satırı olmaması.
        let findings = text.lines().filter(|l| l.starts_with('%')).count();
        assert!(out.status.success() && findings == 0, "{top}.sv:\n{text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ═══ Donanım alanının sıfırlama değeri (ADR-0044 §2; ADR-0109 §4 A2) ═══

/// `@mmio` donanım alanı ve sıfır biçimleri: iki modül, iki test bloğu.
///
/// `Z`: donanımın yazdığı `bits<16>`, `bits<8>`, `bits<4>` alanları. Bus'ın
/// yazdığı `scratch` register'ı `w.data` ve `w.strb`'yi kullanır, yoksa
/// `-Wall` onları UNUSED sayardı (ADR-0044 §2).
///
/// `Forms`: üreticinin `FieldTy::zero`'da yazdığı sıfır biçimleri, elle,
/// `N > 8` genişliklerde: `(0u8 as uN) as bits<N>` (bugün `N > 8` için) ve
/// `((false as u1) as uN) as bits<N>` (bugün `N < 8` için; `w > 8`
/// korumasının `==` mutantı bunu `N > 8`'e de yazar).
const ZERO_FORMS: &str = r#"@mmio(base = 0x0, bus = AXI4Lite)
module Z {
    in  clk  : clock
    in  load : bool
    in  d16  : u16
    in  d8   : u8
    in  d4   : u4
    out q16  : u16
    out q8   : u8
    out q4   : u4
    @reg(offset = 0x00, access = ReadOnly, volatile)
    status : {
        wide   : bits<16>,
        mid    : bits<8>,
        narrow : bits<4>,
        @reserved : bits<4>,
    }
    @reg(offset = 0x04, access = ReadWrite)
    scratch : {
        value : u32,
    }
    on clk {
        if load {
            regs.status.wide <= d16 as bits<16>
            regs.status.mid <= d8 as bits<8>
            regs.status.narrow <= d4 as bits<4>
        }
    }
    q16 = regs.status.wide as u16
    q8  = regs.status.mid as u8
    q4  = regs.status.narrow as u4
}
module Forms {
    in  clk  : clock
    in  load : bool
    in  d16  : u16
    in  d32  : u32
    out a16  : u16
    out b16  : u16
    out b32  : u32
    reg ra16 : bits<16> = (0u8 as u16) as bits<16>
    reg rb16 : bits<16> = ((false as u1) as u16) as bits<16>
    reg rb32 : bits<32> = ((false as u1) as u32) as bits<32>
    on clk {
        if load {
            ra16 <= d16 as bits<16>
            rb16 <= d16 as bits<16>
            rb32 <= d32 as bits<32>
        }
    }
    a16 = ra16 as u16
    b16 = rb16 as u16
    b32 = rb32 as u32
}
test "hardware fields reset to zero" {
    let dut = Z { };
    dut.load = false;
    dut.d16 = 0x1237;
    dut.d8 = 0x37;
    dut.d4 = 0x7;
    step(1);
    assert_eq(dut.q16, 0);
    assert_eq(dut.q8, 0);
    assert_eq(dut.q4, 0);
    dut.load = true;
    dut.d16 = 0x1256;
    dut.d8 = 0x56;
    dut.d4 = 0x6;
    step(1);
    assert_eq(dut.q16, 0x1256);
    assert_eq(dut.q8, 0x56);
    assert_eq(dut.q4, 0x6);
}
test "both zero forms are zero above 8 bits" {
    let dut = Forms { };
    dut.load = false;
    dut.d16 = 0xFFFF;
    dut.d32 = 0xFFFFFFFF;
    step(1);
    assert_eq(dut.a16, 0);
    assert_eq(dut.b16, 0);
    assert_eq(dut.b32, 0);
    dut.load = true;
    dut.d16 = 0x8001;
    dut.d32 = 0x80000001;
    step(1);
    assert_eq(dut.a16, 0x8001);
    assert_eq(dut.b16, 0x8001);
    assert_eq(dut.b32, 0x80000001);
}
"#;

/// `FieldTy::zero`'nun `w > 8` korumasının `==` mutantı A2'dir (ADR-0109
/// §4, `exclude_re`). Varsayımı: üretilen metindeki iki sıfır biçimi de
/// `N > 8`'de uyarısız genişlemedir ve değeri N bitlik 0'dır. Bu test o
/// varsayımı doğrudan sınar; varsayım bozulursa (bir biçim uyarı alır,
/// lint bulgusu verir ya da 0 olmaz) düşer ve `exclude_re` girdisi yeniden
/// incelenir.
///
/// 1. `volt check`: iki modül de tanısızdır (Volt tarafında uyarı yok).
/// 2. `volt test`, Verilator: iki test bloğu da geçer. Beklenen (elle):
///    `load` düşükken her çıkış sıfırlama değerindedir, 0; `load` bir kenar
///    yüksekken girişler kendi genişliğinde yazılır. `volt test`
///    Verilator'ın WIDTH uyarılarında durur.
/// 3. Üretilen `Z.sv` ve `Forms.sv` `verilator --lint-only -Wall`'dan
///    bulgusuz geçer (ADR-0109 §4 eki (b)).
#[test]
fn hardware_bits_fields_reset_to_zero_at_every_width() {
    let got = check("zero-check", ZERO_FORMS);
    assert_eq!(got, vec![], "{got:?}");
    let Some(verilator) = tools::require(tools::Tool::Verilator) else {
        return;
    };
    sim_and_lint("zero", ZERO_FORMS, 2, &verilator, &["Z", "Forms"]);
}
