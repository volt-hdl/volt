//! Bir tipin bit genişliği tek fonksiyondan gelir (#419, ADR-0137 §1):
//! `TypeArena::bit_width` (skaler tablo `volt_ast::bit_width::Scalar`).
//!
//! #419'dan önce tip denetiminin iki kopyası vardı: `signal_width`
//! `Trit`'e ve `bool`'a genişlik vermiyordu ve `ShiftRegister`'ın `taps`'ı
//! onu `unwrap_or(1)` ile bir bit sayıyordu. `ShiftRegister<Trit, 4>`'ün
//! `taps`'ı `bits<4>` oluyor (SV'de 8 bit, atama kesiliyordu), `Trit`'in
//! `sync()`'i W3003 almıyordu. Beklenen her genişlik spec'ten elle yazıldı:
//! `bool` 1, `Trit` 2 (ADR-0003), `uN`/`iN`/`bits<N>` N, enum kodlama
//! genişliği (ADR-0074: üç varyant 2 bit, iki varyant 1 bit), struct düzen
//! genişliği (ADR-0077), takma ad hedefinin (ADR-0070 §6), dizi eleman ×
//! uzunluk (ADR-0056), `Delayed<T, N>` `T`'nin (ADR-0037). `taps`
//! `bits<LEN * width(T)>`'dir (ADR-0029); W3003 genişliği 1'den büyük
//! `sync()` verisidir (domain-inference.md, W3003).

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-bitw-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// `volt check --format json`: (kod, mesaj) listesi.
fn check(tag: &str, src: &str) -> Vec<(String, String)> {
    let dir = temp_dir(tag);
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    let env: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)));
    let _ = std::fs::remove_dir_all(&dir);
    env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap_or_default().to_string(),
                d["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn codes(diags: &[(String, String)]) -> Vec<&str> {
    diags.iter().map(|(c, _)| c.as_str()).collect()
}

/// Sınanan tipler: (ad, tip, spec'ten elle genişlik, sıfırlama değeri).
/// Enum ve struct dizisinin SV eşlemesi henüz yok (E0003): o tiplerde dizi
/// yerleri sınanmaz ([`has_array`]).
const TYPES: [(&str, &str, u32, &str); 9] = [
    ("bool", "bool", 1, "false"),
    ("trit", "Trit", 2, "0"),
    ("u3", "u3", 3, "0"),
    ("i5", "i5", 5, "0"),
    ("bits6", "bits<6>", 6, "0 as bits<6>"),
    ("enum2", "Two", 1, "Two::Off"),
    ("enum3", "Dir", 2, "Dir::Hold"),
    ("struct", "Pair", 3, "Pair { t: 0, ok: false }"),
    ("alias", "Tr", 2, "0"),
];

/// Tipin dizisi SV'ye inebilir mi (enum ve struct dizisi E0003).
fn has_array(ty: &str) -> bool {
    !matches!(ty, "Two" | "Dir" | "Pair")
}

/// İki saat alanı (`sync()` birimleri).
const DOMAINS: &str = "domain Fast {
    clock = posedge
}

domain Slow {
    clock = posedge
}
";

/// Her birimin başındaki tip bildirimleri.
const PRELUDE: &str = "enum Two {
    Off,
    On,
}

enum Dir {
    Hold,
    Up,
    Down,
}

struct Pair {
    t  : Trit
    ok : bool
}

type Tr = Trit
";

/// `ShiftRegister<T, 4>`'ün `taps`'ı `bits<w>` bir porta atanır.
fn taps_unit(ty: &str, w: u32) -> String {
    format!(
        "{PRELUDE}
module M {{
    in  clk    : clock
    in  d      : {ty}
    in  strobe : bool
    out window : bits<{w}>

    let sr = ShiftRegister<{ty}, 4> {{ clk: clk, data_in: d, shift_en: strobe }}

    window = sr.taps
}}
"
    )
}

/// #419'un örneği, issue'daki biçimde: `taps` `bits<8>`'dir; `bits<4>`
/// porta atama E2001, `bits<8>` temiz. Düzeltmeden önce tersiydi.
#[test]
fn issue_419_trit_taps_are_two_bits_per_stage() {
    let narrow = check("419-narrow", &taps_unit("Trit", 4));
    assert_eq!(codes(&narrow), ["E2001"], "{narrow:?}");
    assert!(
        narrow[0]
            .1
            .contains("a bits<8> value cannot be assigned to a bits<4> target"),
        "{narrow:?}"
    );
    let right = check("419-right", &taps_unit("Trit", 8));
    assert!(right.is_empty(), "{right:?}");
}

/// #419'un ikinci etkisi: `Trit` iki bittir, `sync()`'i W3003 alır;
/// `bool` tek bittir, almaz.
#[test]
fn issue_419_trit_sync_is_w3003() {
    let unit = |ty: &str| {
        format!(
            "{DOMAINS}{PRELUDE}
module S {{
    in  fast : clock @Fast
    in  slow : clock @Slow
    in  t    : {ty} @Fast
    out y    : {ty} @Slow

    y = sync(t, slow)
}}
"
        )
    };
    let trit = check("419-sync-trit", &unit("Trit"));
    let w3003: Vec<_> = trit.iter().filter(|(c, _)| c == "W3003").collect();
    assert_eq!(w3003.len(), 1, "{trit:?}");
    assert!(w3003[0].1.contains("for 2-bit signals"), "{trit:?}");
    let one_bit = check("419-sync-bool", &unit("bool"));
    assert!(!codes(&one_bit).contains(&"W3003"), "{one_bit:?}");
}

/// Her tip için `taps` `bits<4 × w>`'dir: o genişlikte port temiz, bir
/// bit geniş port E2001.
#[test]
fn taps_width_is_len_times_the_width_of_every_element_type() {
    for (name, ty, w, _) in TYPES {
        let right = check(&format!("taps-{name}"), &taps_unit(ty, 4 * w));
        assert!(right.is_empty(), "{ty}: bits<{}>: {right:?}", 4 * w);
        let wide = check(&format!("taps-wide-{name}"), &taps_unit(ty, 4 * w + 1));
        assert_eq!(
            codes(&wide),
            ["E2001"],
            "{ty}: bits<{}>: {wide:?}",
            4 * w + 1
        );
        let want = format!("a bits<{}> value cannot be assigned", 4 * w);
        assert!(wide[0].1.contains(&want), "{ty}: {wide:?}");
    }
}

/// `sync()`'e verilen değerin genişliği her kullanım yerinde aynıdır:
/// port, dizi elemanı, struct alanı ve `Delayed<T, 1>` register'ı T'nin
/// genişliğini taşır (W3003 yalnız `w > 1` iken, mesajında `w`); bütün
/// dizi `3 × w` bittir (hep W3003). `sync()`'in kaynağı bir addır
/// (bileşik kaynak E0003), değer önce bir `let`'e bağlanır.
#[test]
fn sync_sees_the_same_width_at_every_use() {
    let sites: [(&str, &str, u32); 5] = [
        ("port", "d", 1),
        ("elem", "arr[1]", 1),
        ("field", "h.f", 1),
        ("delayed", "r", 1),
        ("array", "arr", 3),
    ];
    for (name, ty, w, init) in TYPES {
        for (site, value, times) in sites {
            let array = has_array(ty);
            if !array && matches!(site, "elem" | "array") {
                continue;
            }
            let out_ty = if site == "array" {
                format!("[{ty}; 3]")
            } else {
                ty.to_string()
            };
            let (arr_in, arr_out, arr_use) = if array {
                (
                    format!(
                        "
    in  arr  : [{ty}; 3] @Fast"
                    ),
                    format!(
                        "
    out a    : [{ty}; 3] @Fast"
                    ),
                    "
    a = arr",
                )
            } else {
                (String::new(), String::new(), "")
            };
            let src = format!(
                "{DOMAINS}{PRELUDE}
struct Hold {{
    f  : {ty}
    ok : bool
}}

module S {{
    in  fast : clock @Fast
    in  slow : clock @Slow
    in  d    : {ty} @Fast{arr_in}
    in  h    : Hold @Fast
    out y    : {out_ty} @Slow
    out k    : bool @Fast
    out e    : {ty} @Fast{arr_out}

    reg r : Delayed<{ty}, 1> = {init}
    on fast {{
        r <= d
    }}

    k = h.ok
    e = d{arr_use}
    let v = {value}
    y = sync(v, slow)
}}
"
            );
            let got = check(&format!("sync-{name}-{site}"), &src);
            let errors: Vec<_> = got.iter().filter(|(c, _)| c.starts_with('E')).collect();
            assert!(errors.is_empty(), "{ty} {site}: {got:?}");
            let bits = w * times;
            let w3003: Vec<_> = got.iter().filter(|(c, _)| c == "W3003").collect();
            if bits > 1 {
                assert_eq!(w3003.len(), 1, "{ty} {site}: {got:?}");
                let want = format!("for {bits}-bit signals");
                assert!(w3003[0].1.contains(&want), "{ty} {site}: {got:?}");
            } else {
                assert!(w3003.is_empty(), "{ty} {site}: {got:?}");
            }
        }
    }
}

/// `taps`'ın genişliği sınırı (65535, const-eval.md E2025) aşarsa E2025.
/// Düzeltmeden önce çarpım `u16`'ya kesiliyordu: `ShiftRegister<bits<2000>,
/// 64>`'ün `taps`'ı 128000 bit yerine `bits<62464>` (128000 - 65536)
/// sayılıyor, o porta atama temiz geçiyordu.
#[test]
fn taps_above_the_width_limit_is_e2025() {
    let unit = |len: u32, port: u32| {
        format!(
            "module M {{
    in  clk    : clock
    in  d      : bits<2000>
    in  strobe : bool
    out window : bits<{port}>

    let sr = ShiftRegister<bits<2000>, {len}> {{ clk: clk, data_in: d, shift_en: strobe }}

    window = sr.taps
}}
"
        )
    };
    let over = check("taps-over", &unit(64, 62464));
    assert_eq!(codes(&over), ["E2025"], "{over:?}");
    assert!(
        over[0].1.contains("'taps' would be 128000 bits wide"),
        "{over:?}"
    );
    // 32 × 2000 = 64000 sınırın altında: hata yok.
    let under = check("taps-under", &unit(32, 64000));
    assert!(under.is_empty(), "{under:?}");
}

/// `taps`'ı olmayan primitifte sınır denetimi yoktur: `SyncFifo<bits<2000>,
/// 64>`'ün öğesi 2000 bittir, çarpılmaz.
#[test]
fn element_width_is_not_multiplied_without_taps() {
    let src = "module M {
    in  clk  : clock
    in  d    : bits<2000>
    in  push : bool
    in  pop  : bool
    out q    : bits<2000>

    let f = SyncFifo<bits<2000>, 64> { clk: clk, wr_data: d, wr_en: push, rd_en: pop }

    q = f.rd_data
}
";
    let got = check("fifo-wide", src);
    assert!(!codes(&got).iter().any(|c| c.starts_with('E')), "{got:?}");
}

/// RAM ailesinin E2009 önerisi ham bitlerin genişliğini tek kaynaktan
/// adlandırır: üç varyantlı enum 2 bittir (`u2`), `Trit` 2 bittir.
/// Düzeltmeden önce `Trit` `unwrap_or(1)` ile `u1` öneriyordu.
#[test]
fn memory_element_advice_names_the_raw_width() {
    for (ty, raw) in [("Dir", "u2"), ("Trit", "u2")] {
        let src = format!(
            "{PRELUDE}
module M {{
    in  clk  : clock
    in  d    : {ty}
    in  we   : bool
    in  addr : bits<2>
    out q    : {ty}

    let m = Ram<{ty}, 4> {{ clk: clk, addr: addr, wr_data: d, wr_en: we }}

    q = m.rd_data
}}
"
        );
        let dir = temp_dir(&format!("ram-{raw}-{ty}"));
        let file = dir.join("case.volt");
        std::fs::write(&file, &src).expect("yaz");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .args(["--lang", "en", "check", "--format", "json"])
            .arg(&file)
            .output()
            .expect("volt");
        let env: Value = serde_json::from_slice(&out.stdout).expect("json");
        let _ = std::fs::remove_dir_all(&dir);
        let e2009 = env["diagnostics"]
            .as_array()
            .expect("tanılar")
            .iter()
            .find(|d| d["code"] == "E2009")
            .unwrap_or_else(|| panic!("{ty}: E2009 yok: {env:#}"));
        let text = e2009.to_string();
        assert!(text.contains(&format!("Ram<{raw}, ...>")), "{ty}: {text}");
    }
}

/// #419'un örneğinin değeri gerçek Verilator'da (`volt test`):
/// `tests/real_bugs/419_shift_register_trit_taps.volt`'un test bloğu,
/// beklenen değerleri spec'ten elle (dosyadaki yorumlar).
#[test]
fn issue_419_trit_taps_value_in_simulation() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("419-sim");
    let src = root().join("tests/real_bugs/419_shift_register_trit_taps.volt");
    std::fs::copy(&src, dir.join("case.volt")).expect("kopyala");
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
    eprintln!("--- volt test (bit_width_tests::issue_419)\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(text.contains("1 passed; 0 failed"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// E3001'in `sync()` önerisi yalnız tek bitlik veriye yazılır (K9):
/// genişlik tek kaynaktan, bu yüzden tek elemanlı `[bool; 1]` de alır
/// (önce almıyordu); saat tek bittir ama veri değildir, almaz; `u2` iki
/// bittir, almaz (W3003'ün alanı).
#[test]
fn sync_advice_is_for_one_bit_data_only() {
    let unit = |ty: &str| {
        format!(
            "{DOMAINS}
module Cross {{
    in  _fclk : clock @Fast
    in  sclk  : clock @Slow
    in  a     : {ty} @Fast
    out y     : {ty} @Slow

    wire w : {ty} @Slow
    w = a
    y = w
}}
"
        )
    };
    for (ty, advice) in [
        ("bool", true),
        ("[bool; 1]", true),
        ("clock", false),
        ("u2", false),
    ] {
        let dir = temp_dir(&format!(
            "advice-{}",
            ty.replace(|c: char| !c.is_ascii_alphanumeric(), "")
        ));
        let file = dir.join("case.volt");
        std::fs::write(&file, unit(ty)).expect("yaz");
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .args(["--lang", "en", "check", "--format", "json"])
            .arg(&file)
            .output()
            .expect("volt");
        let env: Value = serde_json::from_slice(&out.stdout).expect("json");
        let _ = std::fs::remove_dir_all(&dir);
        let e3001 = env["diagnostics"]
            .as_array()
            .expect("tanılar")
            .iter()
            .find(|d| d["code"] == "E3001")
            .unwrap_or_else(|| panic!("{ty}: E3001 yok: {env:#}"))
            .to_string();
        assert_eq!(e3001.contains("sync(a, sclk)"), advice, "{ty}: {e3001}");
    }
}

/// `volt_ast::bit_width::type_ref_bits` modülün `p` portu için.
fn port_bits(src: &str) -> Option<u32> {
    let parsed = volt_syntax::parser::parse(volt_span::FileId(0), src);
    let ast = parsed.ast;
    let port = ast
        .items
        .iter()
        .find_map(|&i| match &ast.items_arena[i].kind {
            volt_ast::ItemKind::Module(m) => m.ports.iter().find(|p| p.name.text == "p"),
            _ => None,
        })
        .expect("p portu");
    volt_ast::bit_width::type_ref_bits(&ast, port.ty)
}

/// AST genişliği iç içe tipte 16 düzeye kadar iner (`bits_at`'in
/// `MAX_TYPE_DEPTH`'i): 16 düzey `bool` dizisi 2^16 bit, 17 düzey çözülmez;
/// birbirini dizi elemanı olarak adlandıran iki takma ad sonsuz döngüye
/// girmez.
#[test]
fn ast_width_stops_at_the_nesting_limit_and_on_alias_cycles() {
    let nested = |levels: usize| {
        let mut ty = "bool".to_string();
        for _ in 0..levels {
            ty = format!("[{ty}; 2]");
        }
        format!("module M {{\n    in  p : {ty}\n    out q : bool\n    q = true\n}}\n")
    };
    assert_eq!(port_bits(&nested(16)), Some(1 << 16));
    assert_eq!(port_bits(&nested(17)), None);
    let cycle = "type A = [B; 2]\ntype B = [A; 2]\n\
                 module M {\n    in  p : A\n    out q : bool\n    q = true\n}\n";
    assert_eq!(port_bits(cycle), None);
    // Skaler tablo, takma ad, enum ve struct da aynı fonksiyondan.
    let src = |ty: &str| {
        format!("{PRELUDE}\nmodule M {{\n    in  p : {ty}\n    out q : bool\n    q = true\n}}\n")
    };
    for (_, ty, w, _) in TYPES {
        assert_eq!(port_bits(&src(ty)), Some(w), "{ty}");
    }
    assert_eq!(port_bits(&src("clock")), Some(1));
    assert_eq!(port_bits(&src("(u8, u8)")), None);
}

/// `taps` tam sınırda (15 × 4369 = 65535 bit) geçerlidir; bir aşama
/// fazlası E2025 (16 × 4369 = 69904).
#[test]
fn taps_exactly_at_the_width_limit_is_valid() {
    let unit = |len: u32, port: u32| {
        format!(
            "module M {{
    in  clk    : clock
    in  d      : bits<4369>
    in  strobe : bool
    out window : bits<{port}>

    let sr = ShiftRegister<bits<4369>, {len}> {{ clk: clk, data_in: d, shift_en: strobe }}

    window = sr.taps
}}
"
        )
    };
    let at = check("taps-at", &unit(15, 65535));
    assert!(at.is_empty(), "{at:?}");
    let over = check("taps-over-one", &unit(16, 65535));
    assert_eq!(codes(&over), ["E2025"], "{over:?}");
    assert!(over[0].1.contains("69904 bits wide"), "{over:?}");
}

/// Test dili değeri sayı olarak yalnız skalerde (ve enum'da) yazar; struct
/// elemanının genişliği orada "bilinmiyor"dur (`scalar_width`, #419):
/// struct dizisine `load` değer aralığı denetimine (E8510) girmez, kullanıcı
/// tasarımın asıl hatasını görür: struct dizisinin SV eşlemesi yok (E0003).
#[test]
fn load_into_a_struct_array_reports_the_design_error_not_a_width() {
    let dir = temp_dir("load-struct");
    std::fs::write(dir.join("img.hex"), "1\n9\n").expect("yaz");
    let src = "struct P {
    t  : u2
    ok : bool
}

module M {
    in  clk : clock
    out y   : u2
    reg mem : [P; 4] = [P { t: 0, ok: false }; 4]
    on clk { mem[0] <= mem[1] }
    y = mem[0].t
}

test \"load into struct memory\" {
    let dut = M { };
    let image = read_hex(\"img.hex\");
    load(dut.mem, image);
    step(1);
    assert_eq(dut.y, 0);
}
";
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt");
    let env: Value = serde_json::from_slice(&out.stdout).expect("json");
    let _ = std::fs::remove_dir_all(&dir);
    let got: Vec<&str> = env["diagnostics"]
        .as_array()
        .expect("tanılar")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert_eq!(got, ["E0003"], "{env:#}");
}
