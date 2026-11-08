//! Dosyalar arası görünürlük (#139): ayrıştırıcının birim sonu adımları
//! (const değerlendirmesi, bundle düzleştirmesi, kapsam zinciri) HIR'ın
//! çözümüyle aynı kuralı izler.
//!
//! Şartname: name-resolution.md §3.3 ve ADR-0042 §3 — kök kapsam birim
//! genelinde tektir; bir dosya başka dosyanın öğesini yalnız `use` ile
//! getirdiyse (adıyla, `as` takma adıyla ya da glob ile) yalın adla
//! kullanır, aksi E1001. Kendi dosyasının öğeleri her zaman görünür; bir
//! const'un değeri kendi dosyasının kapsamında çözülür (§4, kök kapsam).
//! Glob yalnız `pub` öğeleri getirir. İki dosya aynı adla öğe tanımlayamaz
//! (ADR-0042 §4 "Bilinen sınır", E1003); aynı yerel ad iki `use` ile
//! gelirse E1010 (§10).
//!
//! Ayrıştırıcı bu adları HIR'dan önce yok eder (bundle dizisi indeksi
//! `rx[K]` → `rx_1_data`, `for` sınırı, bundle tipi): düzeltmeden önce
//! başka dosyanın private const'u ya da struct'u burada sessizce değer
//! veriyordu, takma adlı import ise hiç bulunmuyordu. "Denetim" diye
//! işaretli testler düzeltmeden önce de geçiyordu; kuralın öteki
//! uçlarını sabitlerler.
//!
//! Davranış testleri tasarımı `volt test` ile Verilator'da simüle eder;
//! beklenen değerler kaynaktan elle hesaplandı. Verilator yoksa atlanır;
//! `VOLT_REQUIRE_TOOLS=verilator` ile yokluk testi düşürür (ADR-0079 §3).

mod tools;

use std::path::{Path, PathBuf};
use std::process::Command;

/// Dosyaları yeni bir geçici dizine yazar (ana dosya `main.volt`).
fn unit(tag: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-xfile-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    for (name, text) in files {
        std::fs::write(dir.join(name), text).expect("yaz");
    }
    dir
}

/// `volt build main.volt`: (başarılı mı, `dosya:satır:sütun: kod` satırları).
fn build(dir: &Path) -> (bool, Vec<String>) {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(dir)
        .args([
            "build",
            "main.volt",
            "--format",
            "short",
            "--target-dir",
            "build",
        ])
        .output()
        .expect("volt build");
    let text = String::from_utf8_lossy(&out.stderr).to_string();
    let diags = text
        .lines()
        .filter_map(|l| {
            let (loc, rest) = l.split_once(": ")?;
            let code = rest
                .strip_prefix("error[")
                .or_else(|| rest.strip_prefix("warning["))?
                .split(']')
                .next()?;
            let file = loc.rsplit(['/', '\\']).next()?;
            Some(format!("{file}: {code}"))
        })
        .collect();
    (out.status.success(), diags)
}

/// Birimin tanıları tam olarak `expected` (sırasız, `dosya:satır:sütun:
/// kod`); `ok` derlemenin sonucu.
fn assert_diags(tag: &str, files: &[(&str, &str)], ok: bool, expected: &[&str]) {
    let dir = unit(tag, files);
    let (success, mut got) = build(&dir);
    got.sort();
    let mut want: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!((success, &got), (ok, &want), "{tag}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Birimi `volt test main.volt` ile koşar; `passed` test geçmeli.
fn sim(tag: &str, files: &[(&str, &str)], passed: usize) {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = unit(tag, files);
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .env("NO_COLOR", "1")
        .current_dir(&dir)
        .args(["test", "main.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let text = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // CI günlüğünde testin Verilator'da koştuğu görünsün.
    eprintln!("--- volt test (cross_file_visibility_tests::{tag})\n{text}");
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains(&format!("{passed} passed; 0 failed")),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// İki elemanlı Handshake dizisi; `y` indeksle seçilen elemanın verisi.
fn handshake_pick(index: &str) -> String {
    format!(
        "module M {{
    in  rx : [Handshake<u8>; 2]
    out y  : u8
    rx[0].ready = true
    rx[1].ready = true
    y = rx[{index}].data
}}
"
    )
}

/// `rx_0_data = 10`, `rx_1_data = 20`: indeks 1 ise `y` 20.
const PICK_ONE_TEST: &str = r#"test "index 1 selects rx[1]" {
    let dut = M { };
    dut.rx_0_data = 10;
    dut.rx_1_data = 20;
    step(1);
    assert_eq(dut.y, 20);
}
"#;

/// `for i in 0..<bound> { y[i] = a[i] }`, kalan bitler 0.
fn low_bits(bound: &str) -> String {
    format!(
        "module M {{
    in  a : u4
    out y : u4
    for i in 0..{bound} {{
        y[i] = a[i]
    }}
    y[2] = false
    y[3] = false
}}
"
    )
}

/// `a = 15` ve sınır 2: `y` = 0b0011 = 3.
const LOW_BITS_TEST: &str = r#"test "two low bits pass" {
    let dut = M { };
    dut.a = 15;
    step(1);
    assert_eq(dut.y, 3);
}
"#;

// ═══ (a) Başka dosyanın private const'u: E1001 (§3.3) ═══════════════════

/// Bundle dizisi indeksi (#139'un örneği). Düzeltmeden önce tanı yoktu,
/// `y = rx_1_data` üretiliyordu.
#[test]
fn private_const_of_another_file_in_a_bundle_index_is_e1001() {
    let main = format!("use other::X;\n{}", handshake_pick("K"));
    assert_diags(
        "idx",
        &[
            ("other.volt", "pub const X : u32 = 7\nconst K : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:7:12: E1001"],
    );
}

/// Bundle dizisi uzunluğu. Düzeltmeden önce iki elemanlı dizi açılıyordu.
#[test]
fn private_const_of_another_file_as_a_bundle_array_length_is_e1001() {
    assert_diags(
        "len",
        &[
            ("other.volt", "pub const X : u32 = 7\nconst K : u32 = 2\n"),
            (
                "main.volt",
                "use other::X;
module M {
    in  rx : [Handshake<u8>; K]
    out y  : u8
    y = 0
}
",
            ),
        ],
        false,
        &["main.volt:3:30: E1001"],
    );
}

/// Modül düzeyi `for` sınırı. Düzeltmeden önce döngü iki kez açılıyordu.
#[test]
fn private_const_of_another_file_as_a_for_bound_is_e1001() {
    let main = format!("use other::X;\n{}", low_bits("K"));
    assert_diags(
        "for",
        &[
            ("other.volt", "pub const X : u32 = 7\nconst K : u32 = 2\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:5:17: E1001"],
    );
}

/// Glob yalnız `pub` öğeleri getirir (§3.3): private `K` görünmez.
/// Düzeltmeden önce `y = rx_0_data` üretiliyordu.
#[test]
fn glob_import_does_not_bring_a_private_const() {
    let main = format!("use other::*;\n{}", handshake_pick("K"));
    assert_diags(
        "glob-priv",
        &[
            ("other.volt", "pub const X : u32 = 1\nconst K : u32 = 0\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:7:12: E1001"],
    );
}

/// Generic modülün gövdesi tanımlandığı dosyada çözülür: örnekleyen
/// dosyanın private `K`'si orada görünmez (generic argüman yolu, ADR-0041).
/// Düzeltmeden önce `S_4` gövdesi `main.volt`'un `K`'siyle açılıyordu.
#[test]
fn generic_body_does_not_see_a_private_const_of_the_instantiating_file() {
    assert_diags(
        "mono",
        &[
            (
                "other.volt",
                "pub module S<const N: u32> {
    in  a : u4
    out y : u4
    for i in 0..K {
        y[i] = a[i]
    }
    y[2] = false
    y[3] = false
}
",
            ),
            (
                "main.volt",
                "use other::S;
const K : u32 = 2
module M {
    in  a : u4
    out y : u4
    let s = S<4> { a: a }
    y = s.y
}
",
            ),
        ],
        false,
        &["other.volt:4:17: E1001"],
    );
}

/// Bundle tipi (`struct port`) de bir öğedir: içe aktarılmamış private
/// tip E1001. Düzeltmeden önce port açılıyor, tip adı kayboluyordu.
#[test]
fn private_bundle_type_of_another_file_is_e1001() {
    assert_diags(
        "bundle-ty",
        &[
            (
                "other.volt",
                "pub const X : u32 = 1\nstruct port Bus {\n    out d : u8\n}\n",
            ),
            (
                "main.volt",
                "use other::X;
module M {
    in  p : Bus
    out y : u8
    y = p.d
}
",
            ),
        ],
        false,
        &["main.volt:3:13: E1001"],
    );
}

/// Handshake payload'ı olan sade struct da öyle. Düzeltmeden önce payload
/// alanlara açılıyor (`rx_data_a`), tip adı kayboluyordu.
#[test]
fn private_payload_struct_of_another_file_is_e1001() {
    assert_diags(
        "payload",
        &[
            (
                "other.volt",
                "pub const X : u32 = 1\nstruct Hdr {\n    a : u8\n}\n",
            ),
            (
                "main.volt",
                "use other::X;
module M {
    in  rx : Handshake<Hdr>
    out y  : u8
    rx.ready = true
    y = rx.data.a
}
",
            ),
        ],
        false,
        &["main.volt:3:24: E1001"],
    );
}

/// Görünmeyen bir ad gölgelenmez (§6): döngü değişkeni `i` başka dosyanın
/// private `i`'sini örtmez, W1002 yok. Düzeltmeden önce W1002 veriliyordu.
#[test]
fn a_loop_variable_does_not_shadow_an_invisible_name_of_another_file() {
    assert_diags(
        "shadow",
        &[
            ("other.volt", "pub const X : u32 = 2\nconst i : u32 = 0\n"),
            (
                "main.volt",
                "use other::X;
module M {
    in  a : [u8; 2]
    out y : [u8; 2]
    for i in 0..X {
        y[i] = a[i]
    }
}
",
            ),
        ],
        true,
        &[],
    );
}

/// Denetim: hiç tanımlanmamış ad bu konumda ADR-0072'nin E2021'ini korur
/// (#139 yalnız başka dosyada bildirilmiş, görünmeyen adı E1001 yaptı).
/// Şartnamenin §3.1'i tanımsız ada E1001 der; hangisinin geçerli olduğu
/// #122'de karar bekliyor.
#[test]
fn an_undefined_name_as_a_for_bound_keeps_e2021() {
    assert_diags(
        "undef",
        &[("main.volt", &low_bits("Q"))],
        false,
        &["main.volt:4:5: E2021"],
    );
}

/// Denetim: birimde başka dosya ve başka adlı öğeler olsa da tanımsız ad
/// E2021'de kalır; yalnız aynı adla başka dosyada bildirilmiş ad E1001
/// olur. Hedefli mutasyon koşusunun kör noktası: `declared_elsewhere`'in
/// ad karşılaştırması (`==` → `!=`) başka adlı her öğeyi eşliyor, tanımsız
/// `Q`'yu E1001 yapıyordu; tek dosyalı test bunu göremez.
#[test]
fn an_undefined_name_beside_another_file_keeps_e2021() {
    let main = format!("use other::X;\n{}", low_bits("Q"));
    assert_diags(
        "undef2",
        &[
            ("other.volt", "pub const X : u32 = 2\nconst K : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:5:5: E2021"],
    );
}

/// Görünmeyen ad tekli eksinin altında da bulunur: `0..-K`'da `K` başka
/// dosyanın private const'u, E1001 (`K`'nin konumu). Hedefli mutasyon
/// koşusunun kör noktası: `invisible_name`'in `-` kolu silinince ad
/// bulunmuyor, E2021 veriliyordu.
#[test]
fn private_const_of_another_file_under_a_negation_is_e1001() {
    let main = format!("use other::X;\n{}", low_bits("-K"));
    assert_diags(
        "neg",
        &[
            ("other.volt", "pub const X : u32 = 2\nconst K : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:5:18: E1001"],
    );
}

/// Denetim: görünmezlik tanısı yalnız yalın adlar içindir (§3.3, yalın
/// adla kullanım). Nitelikli yol (`Mode::B`, başka dosyanın private
/// enum'unun varyantı) tamsayı sabiti değildir: sınır E2021 kalır
/// (ADR-0072, `for`'un konumu). Hedefli mutasyon koşusunun kör noktası:
/// `invisible_name`'in yol koşulu (`len() == 1` → `true`) yolun ilk
/// parçasını (`Mode`) yalın ad sayıp E1001 veriyordu.
#[test]
fn a_qualified_path_bound_keeps_e2021() {
    let main = format!("use other::X;\n{}", low_bits("Mode::B"));
    assert_diags(
        "qualified",
        &[
            ("other.volt", "pub const X : u32 = 2\nenum Mode { A, B }\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:5:5: E2021"],
    );
}

/// İkili ifadenin içinde de: `0..1 + K`'da `K` E1001 (`K`'nin konumu).
/// Hedefli mutasyon koşusunun kör noktası: `invisible_name`'in ikili kolu
/// silinince ad bulunmuyor, E2021 veriliyordu.
#[test]
fn private_const_of_another_file_inside_a_binary_bound_is_e1001() {
    let main = format!("use other::X;\n{}", low_bits("1 + K"));
    assert_diags(
        "binary",
        &[
            ("other.volt", "pub const X : u32 = 2\nconst K : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:5:21: E1001"],
    );
}

/// E1001'in yardım satırı öğenin görünürlüğüne göre değişir (tanı metni
/// davranıştır, ADR-0109 §4): `pub` öğe için yalnız içe aktarma, private
/// öğe için önce `pub`. Hedefli mutasyon koşusunun kör noktası:
/// `invisible_name_error`'ın `pub` kolu silinince `pub` `K` için de
/// "private" yardımı veriliyordu.
#[test]
fn the_e1001_help_says_whether_the_name_is_pub() {
    let main = format!("use other::X;\n{}", low_bits("K"));
    for (other, help) in [
        (
            "pub const X : u32 = 7\npub const K : u32 = 2\n",
            "'K' is declared in another file of this design: import it with 'use <package>::K;'",
        ),
        (
            "pub const X : u32 = 7\nconst K : u32 = 2\n",
            "'K' is private to another file of this design: mark it 'pub' there and import it with 'use <package>::K;'",
        ),
    ] {
        let dir = unit("help", &[("other.volt", other), ("main.volt", &main)]);
        let out = Command::new(env!("CARGO_BIN_EXE_volt"))
            .env("VOLT_LANG", "en")
            .env("NO_COLOR", "1")
            .current_dir(&dir)
            .args(["build", "main.volt", "--target-dir", "build"])
            .output()
            .expect("volt build");
        let text = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(!out.status.success(), "{text}");
        assert!(text.contains("error[E1001]"), "{text}");
        assert!(text.contains(help), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Takma adla getirilen ad (`use other::K as J`) üçüncü bir dosyanın aynı
/// adlı private `J`'siyle karışmaz: sınır `K` = 2 olur (E1001 yok). `a =
/// 15`, iki düşük bit: `y` = 3.
#[test]
fn an_alias_is_not_hidden_by_a_private_name_of_a_third_file() {
    let main = format!(
        "use other::K as J;\nuse third::Z;\n{}{LOW_BITS_TEST}",
        low_bits("J").replace("y[3] = false", "y[3] = Z")
    );
    let files = [
        ("other.volt", "pub const K : u32 = 2\n"),
        (
            "third.volt",
            "pub const Z : bool = false\nconst J : u32 = 0\n",
        ),
        ("main.volt", main.as_str()),
    ];
    assert_diags("alias-hide", &files, true, &[]);
    sim("alias-hide-sim", &files, 1);
}

/// Takma ad kök çerçevede yerel adıyla bağlıdır (§3.3): sabit olmayan
/// sınırda (`J + n`, `n` port) görünmez ad aranırken `J` bulunur, tanı
/// E2021 kalır (ADR-0072, `for`'un konumu). Hedefli mutasyon koşusunun kör
/// noktası (#139): `Scopes::module`'ün takma ad koşulu (`local != item` →
/// `==`) `J`'yi zincire koymuyor, `invisible_name` onu `third.volt`'un
/// private `J`'si sanıp `J`'nin konumunda E1001 veriyordu. Private `J`
/// takma adın kaynağı `other.volt`'ta olsaydı mutant görünmezdi: kök
/// çerçeve o dosyadan getirilen yerel adla aynı adlı öğesini de bildirir
/// (`RootFilter::sees` tanım dosyasına bakar).
#[test]
fn an_alias_in_a_non_constant_bound_keeps_e2021() {
    assert_diags(
        "alias-e2021",
        &[
            ("other.volt", "pub const K : u32 = 2\n"),
            (
                "third.volt",
                "pub const Z : bool = false\nconst J : u32 = 0\n",
            ),
            (
                "main.volt",
                "use other::K as J;
use third::Z;
module M {
    in  n : u2
    out y : bool
    for i in 0..J + n {
        let t = i
    }
    y = Z
}
",
            ),
        ],
        false,
        &["main.volt:6:5: E2021"],
    );
}

// ═══ (a), (d) Denetim: HIR'ın gördüğü konumlar ═══════════════════════════

/// Ad ayrıştırıcıda kaybolmuyorsa çözücü E1001 verir; düzeltmeden önce de
/// böyleydi. Genişlik (`uint<K>`), sade dizi uzunluğu, değer konumu,
/// sayaç kontratının sınırı ve öğe türleri (fn, enum, struct, modül,
/// generic modül, tip takma adı); alan için E3002 (domain-inference.md).
/// Generic argüman literal olmalıdır (ADR-0041, E2008): const orada
/// görünürlükten bağımsız reddedilir.
#[test]
fn names_the_resolver_sees_keep_their_diagnostics() {
    let other = "pub const X : u32 = 1
const K : u32 = 4
fn inc(a: u8) -> u8 { a + 1 }
enum St { A, B }
struct Pt {
    a : u8
}
module Sub {
    in  a : u8
    out y : u8
    y = a
}
module G<const W: u32> {
    in  a : u8
    out y : u8
    y = a ^ W
}
type Word = u8
";
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "width",
            "module M {\n    in a : uint<K>\n    out y : uint<K>\n    y = a\n}\n",
            &["main.volt:3:17: E1001", "main.volt:4:18: E1001"],
        ),
        (
            "array",
            "module M {\n    in a : [u8; K]\n    out y : u8\n    y = a[0]\n}\n",
            &["main.volt:3:17: E1001"],
        ),
        (
            "value",
            "module M {\n    out y : u32\n    y = K\n}\n",
            &["main.volt:4:9: E1001"],
        ),
        (
            "fn",
            "module M {\n    in a : u8\n    out y : u8\n    y = inc(a)\n}\n",
            &["main.volt:5:9: E1001"],
        ),
        (
            "struct",
            "module M {\n    in p : Pt\n    out y : u8\n    y = p.a\n}\n",
            &["main.volt:3:12: E1001"],
        ),
        (
            "module",
            "module M {\n    in a : u8\n    out y : u8\n    let s = Sub { a: a }\n    y = s.y\n}\n",
            &["main.volt:5:13: E1001"],
        ),
        (
            "generic",
            "module M {\n    in a : u8\n    out y : u8\n    let s = G<3> { a: a }\n    y = s.y\n}\n",
            &["main.volt:5:13: E1001"],
        ),
        (
            "alias",
            "module M {\n    in a : Word\n    out y : u8\n    y = a\n}\n",
            &["main.volt:3:12: E1001"],
        ),
        (
            "generic-arg",
            "use other::G;\nmodule M {\n    in a : u8\n    out y : u8\n    let s = G<X> { a: a }\n    y = s.y\n}\n",
            &["main.volt:5:15: E2008"],
        ),
    ];
    for (tag, body, expected) in cases {
        let main = if body.starts_with("use ") {
            body.replacen("use other::G;\n", "use other::{G, X};\n", 1)
        } else {
            format!("use other::X;\n{body}")
        };
        assert_diags(
            tag,
            &[("other.volt", other), ("main.volt", &main)],
            false,
            expected,
        );
    }
}

/// Denetim: sayaç kontratının sınırı modül gövdesinde kalır, çözücü
/// E1001 verir (kontrat üretimi sessizce durur, ADR-0066).
#[test]
fn private_const_as_a_counter_bound_is_e1001() {
    let (ok, diags) = build(&unit(
        "counter",
        &[
            ("other.volt", "pub const X : u32 = 7\nconst K : u8 = 9\n"),
            (
                "main.volt",
                "use other::X;
module M {
    in  clk : clock
    out y   : u8
    reg c : u8 = 0
    on clk {
        if c == K { c <= 0 } else { c <= c + 1 }
    }
    y = c
}
",
            ),
        ],
    ));
    assert!(!ok);
    assert!(
        diags.contains(&"main.volt:7:17: E1001".to_string()),
        "{diags:?}"
    );
}

/// Denetim: alan adı başka dosyadan görünmez (E3002, domain-inference.md).
#[test]
fn private_domain_of_another_file_is_e3002() {
    let (ok, diags) = build(&unit(
        "domain",
        &[
            (
                "other.volt",
                "pub const X : u32 = 1
domain Fast {
    clock = posedge
    reset = sync active_high
}
",
            ),
            (
                "main.volt",
                "use other::X;
module M {
    in  clk : clock @Fast
    out y   : bool @Fast
    y = true
}
",
            ),
        ],
    ));
    assert!(!ok);
    assert!(diags.iter().any(|d| d.ends_with("E3002")), "{diags:?}");
}

// ═══ (b), (c) Aynı ad iki dosyada: E1003 / E1010 (ADR-0042) ══════════════

/// (b) İçe aktarılan `pub X` ile üçüncü dosyanın private `X`'i: kök kapsam
/// birim genelinde tektir, ikinci tanım E1003 (ADR-0042 §4). Denetim.
#[test]
fn imported_const_and_a_private_namesake_in_a_third_file_is_e1003() {
    let main = format!("use other::X;\nuse third::T;\n{}", handshake_pick("X"));
    assert_diags(
        "dup-third",
        &[
            ("other.volt", "pub const X : u32 = 1\n"),
            ("third.volt", "pub const T : u32 = 5\nconst X : u32 = 0\n"),
            ("main.volt", &main),
        ],
        false,
        &["third.volt:2:7: E1003"],
    );
}

/// (b) Kendi dosyasındaki `K` ile başka dosyanın private `K`'si: E1003.
/// Denetim.
#[test]
fn own_const_and_a_private_namesake_in_another_file_is_e1003() {
    let main = format!("use other::P;\nconst K : u32 = 0\n{}", handshake_pick("K"));
    assert_diags(
        "dup-own",
        &[
            ("other.volt", "pub const P : u32 = 5\nconst K : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:2:7: E1003"],
    );
}

/// (c) Aynı ad iki dosyada `pub` ve ikisi de içe aktarılıyor: E1010.
/// Denetim.
#[test]
fn two_public_namesakes_imported_together_are_e1010() {
    let main = format!("use a::X;\nuse b::X;\n{}", handshake_pick("X"));
    assert_diags(
        "dup-pub",
        &[
            ("a.volt", "pub const X : u32 = 0\n"),
            ("b.volt", "pub const X : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["main.volt:2:8: E1010"],
    );
}

/// (c) Aynı ad iki dosyada `pub`, biri içe aktarılıyor: E1003. Denetim.
#[test]
fn two_public_namesakes_one_imported_are_e1003() {
    let main = format!("use a::A;\nuse b::X;\n{}", handshake_pick("X"));
    assert_diags(
        "dup-pub-one",
        &[
            ("a.volt", "pub const X : u32 = 0\npub const A : u32 = 0\n"),
            ("b.volt", "pub const X : u32 = 1\n"),
            ("main.volt", &main),
        ],
        false,
        &["b.volt:1:11: E1003"],
    );
}

// ═══ Görünen adlar: davranış (Verilator) ════════════════════════════════

/// `use other::X as J`: `rx[J]` = `rx[X]` = `rx[1]`, `y` = 20. Düzeltmeden
/// önce takma ad bulunamıyordu (E2008).
#[test]
fn aliased_const_selects_the_bundle_element() {
    let main = format!("use other::X as J;\n{}{PICK_ONE_TEST}", handshake_pick("J"));
    sim(
        "alias-idx",
        &[
            ("other.volt", "pub const X : u32 = 1\n"),
            ("main.volt", &main),
        ],
        1,
    );
}

/// `use other::X as J`, `X` = 2: döngü iki kez açılır, `y` = 3.
/// Düzeltmeden önce E2021.
#[test]
fn aliased_const_bounds_the_module_level_for() {
    let main = format!("use other::X as J;\n{}{LOW_BITS_TEST}", low_bits("J"));
    sim(
        "alias-for",
        &[
            ("other.volt", "pub const X : u32 = 2\n"),
            ("main.volt", &main),
        ],
        1,
    );
}

/// `use other::Bus as B`: takma adlı bundle tipi açılır, `y` = `p.d` = 5.
/// Düzeltmeden önce port açılmıyordu (E4001).
#[test]
fn aliased_bundle_type_is_flattened() {
    sim(
        "alias-bundle",
        &[
            ("other.volt", "pub struct port Bus {\n    out d : u8\n}\n"),
            (
                "main.volt",
                r#"use other::Bus as B;
module M {
    in  p : B
    out y : u8
    y = p.d
}
test "p.d reaches y" {
    let dut = M { };
    dut.p_d = 5;
    step(1);
    assert_eq(dut.y, 5);
}
"#,
            ),
        ],
        1,
    );
}

/// Denetim: `pub X = K`, `K` `other.volt`'un private'ı. `X`'in değeri
/// kendi dosyasında çözülür (`K` orada görünür): `rx[X]` = `rx[1]`, 20.
#[test]
fn public_const_defined_through_a_private_one_keeps_its_value() {
    let main = format!("use other::X;\n{}{PICK_ONE_TEST}", handshake_pick("X"));
    sim(
        "chain",
        &[
            ("other.volt", "const K : u32 = 1\npub const X : u32 = K\n"),
            ("main.volt", &main),
        ],
        1,
    );
}

/// Denetim: glob `pub X`'i getirir: `rx[X]` = `rx[1]`, 20.
#[test]
fn glob_imported_const_selects_the_bundle_element() {
    let main = format!("use other::*;\n{}{PICK_ONE_TEST}", handshake_pick("X"));
    sim(
        "glob",
        &[
            ("other.volt", "pub const X : u32 = 1\nconst K : u32 = 0\n"),
            ("main.volt", &main),
        ],
        1,
    );
}

/// Denetim: generic modülün gövdesi kendi dosyasının private `K`'sini
/// görür (K = 2): `y` = 3.
#[test]
fn generic_body_sees_a_private_const_of_its_own_file() {
    sim(
        "mono-own",
        &[
            (
                "other.volt",
                "const K : u32 = 2
pub module S<const N: u32> {
    in  a : u4
    out y : u4
    for i in 0..K {
        y[i] = a[i]
    }
    y[2] = false
    y[3] = false
}
",
            ),
            (
                "main.volt",
                &format!(
                    "use other::S;
module M {{
    in  a : u4
    out y : u4
    let s = S<4> {{ a: a }}
    y = s.y
}}
{LOW_BITS_TEST}"
                ),
            ),
        ],
        1,
    );
}
