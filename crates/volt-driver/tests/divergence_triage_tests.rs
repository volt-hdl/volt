//! #407 envanterinin "Bilinen ayrışmalar" tablosunun ayıklaması (#412,
//! ADR-0137): bugün kullanıcının ulaşamadığı (reddedilen) ve görebileceği
//! bir farkı olmayan (gizli) ayrışmaların bugünkü davranışı.
//!
//! Her test tablonun bir satırını sabitler; satır numarası
//! `docs/design/tek-yerde-bilgi-envanteri.md`'deki sıradır. Beklenen
//! kodlar elle, spec'ten ya da ilgili ADR'den yazılmıştır; bir test
//! bugünkü yanlış bir tanıyı sabitliyorsa yorumu bunu ve açık bulguyu
//! söyler. Ayrışma giderilince (bilgi ADR-0136 Aşama 2'de tek yere
//! geçince) test yeni davranışa göre güncellenir.
//!
//! Sessiz yanlış satırların (1, 5, 8, 9) testi burada yoktur: onlar
//! düzeltmeleriyle `tests/real_bugs`'a girer (ADR-0117).

mod tools;

use std::path::PathBuf;
use std::process::Command;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-triage-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// İnsan biçimindeki rapordaki hata ve uyarı kodları, sıralı.
fn codes(report: &str) -> Vec<String> {
    let mut out = Vec::new();
    for head in ["error[", "warning["] {
        let mut rest = report;
        while let Some(i) = rest.find(head) {
            let code = &rest[i + head.len()..];
            if let Some(end) = code.find(']') {
                out.push(code[..end].to_string());
            }
            rest = &rest[i + head.len()..];
        }
    }
    out.sort();
    out
}

struct Run {
    exit: Option<i32>,
    codes: Vec<String>,
    report: String,
    /// `out/` altındaki dosyalar (yol, metin).
    files: Vec<(String, String)>,
}

/// `volt <args> t.volt` kaynağı geçici bir dizinde çalıştırır.
fn volt(tag: &str, args: &[&str], src: &str) -> Run {
    let dir = temp_dir(tag);
    std::fs::write(dir.join("t.volt"), src).expect("kaynak yazılmalı");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .current_dir(&dir)
        .args(args)
        .arg("t.volt")
        .args(["--color=never"])
        .env("VOLT_LANG", "en")
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    let report =
        String::from_utf8_lossy(&out.stderr).into_owned() + &String::from_utf8_lossy(&out.stdout);
    let mut files = Vec::new();
    collect(&dir.join("out"), &dir.join("out"), &mut files);
    files.sort();
    let _ = std::fs::remove_dir_all(&dir);
    Run {
        exit: out.status.code(),
        codes: codes(&report),
        report,
        files,
    }
}

fn collect(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(base, &p, out);
        } else {
            let rel = p
                .strip_prefix(base)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((rel, std::fs::read_to_string(&p).unwrap_or_default()));
        }
    }
}

fn check(tag: &str, src: &str) -> Run {
    volt(tag, &["check"], src)
}

fn build(tag: &str, src: &str) -> Run {
    volt(
        tag,
        &[
            "build",
            "--single-file",
            "--emit=sva,sdc",
            "--target-dir",
            "out",
        ],
        src,
    )
}

fn assert_codes(run: &Run, exit: i32, want: &[&str]) {
    let mut want: Vec<String> = want.iter().map(|c| c.to_string()).collect();
    want.sort();
    assert_eq!(
        (run.exit, &run.codes),
        (Some(exit), &want),
        "{}",
        run.report
    );
}

// ── Satır 2: sabitte `as` (reddedilen) ────────────────────────────────
// Tip denetiminin değerlendiricisi `as`'ı yok sayar (`consteval.rs`,
// "Sabit değer cast'te değişmez"); `volt_ast::const_eval`'da `as` kolu
// yok. type-inference.md §6 (cast): `5 as u2` düşük iki biti tutar, 1;
// W2010 bunu söyler.

/// Üreticinin katlayamadığı `as`'lı sabit: E0003, çıktı yok.
#[test]
fn row2_a_cast_constant_the_emitter_cannot_fold_is_rejected() {
    let run = build(
        "r2-fold",
        "const K : u32 = 5 as u2\n\nmodule S {\n    in  x : u8\n    out y : u8\n    \
         y = x << K\n}\n",
    );
    assert_codes(&run, 1, &["E0003", "W2010"]);
    assert!(run.files.is_empty(), "{:?}", run.files);
}

/// Dizi indeksi `as`'lı sabitle: tip denetimi kesilmemiş değeri (5)
/// sınırla karşılaştırır ve E2006 verir; spec'e göre değer 1'dir ve
/// indeks geçerlidir. Bugünkü yanlış red sabitlenir (açık bulgu: #412
/// raporu, satır 2).
#[test]
fn row2_a_cast_constant_index_is_bounded_with_the_uncut_value() {
    let run = check(
        "r2-index",
        "const K : u32 = 5 as u2\n\nmodule S {\n    in  x : [u8; 4]\n    out y : u8\n    \
         y = x[K]\n}\n",
    );
    assert_codes(&run, 1, &["E2006", "W2010"]);
    assert!(
        run.report.contains("index 5 out of bounds"),
        "{}",
        run.report
    );
}

/// Satır içi `as`'lı kaydırma miktarı derlenir: W2010 kesmeyi söyler,
/// W2013 ise kesilmemiş değerle (9) "hep 0" der; üretici 1 bit kaydırır
/// (`shift_amount`). Bugünkü yanlış W2013 sabitlenir (açık bulgu).
#[test]
fn row2_an_inline_cast_shift_amount_warns_with_the_uncut_value() {
    let run = build(
        "r2-shift",
        "module S {\n    in  x : u8\n    out y : u8\n    y = x << (9 as u3)\n}\n",
    );
    assert_codes(&run, 0, &["W2010", "W2013"]);
    assert!(
        run.report
            .contains("shift amount 9 exceeds the width of 8 bits"),
        "{}",
        run.report
    );
}

// ── Satır 3: `clog2` ve `~` (reddedilen) ──────────────────────────────
// Tip denetimi ikisini de değerlendirir; `volt_ast::const_eval`'da kolları
// yok: üretici genişliği ya da değeri bulamaz ve derlemeyi durdurur.

#[test]
fn row3_a_clog2_constant_as_a_width_is_rejected() {
    let run = check(
        "r3-clog2-width",
        "const W : u32 = clog2(16)\n\nmodule C {\n    in  x : bits<W>\n    out y : bits<W>\n    \
         y = x\n}\n",
    );
    assert_codes(&run, 1, &["E2005", "E2005"]);
}

#[test]
fn row3_a_clog2_constant_as_a_value_is_rejected() {
    let run = check(
        "r3-clog2-value",
        "const W : u32 = clog2(16)\n\nmodule C {\n    in  x : u8\n    out y : u8\n    \
         y = x << W\n}\n",
    );
    assert_codes(&run, 1, &["E0003"]);
}

#[test]
fn row3_a_bitnot_constant_as_a_width_is_rejected() {
    let run = check(
        "r3-not",
        "const M : u32 = ~0 & 15\n\nmodule C {\n    in  x : bits<M>\n    out y : bits<M>\n    \
         y = x\n}\n",
    );
    assert_codes(&run, 1, &["E2005", "E2005"]);
}

// ── Satır 4: yerleşik primitifin DEPTH'i (reddedilen) ─────────────────
// Tip denetimi DEPTH/N için yalnız literal kabul eder (E2008); üretici
// herhangi bir sabiti okuyabilirdi. Generic parametre monomorflamada
// literale dönüşür: iki taraf da 4 görür.

const FIFO: &str = "    in  clk : clock\n    in  d   : u8\n    in  go  : bool\n    out q   : u8\n    \
                    let buf = SyncFifo<u8, DEPTH> { clk: clk, wr_data: d, wr_en: go, rd_en: go }\n    \
                    q = buf.rd_data\n";

#[test]
fn row4_a_named_constant_depth_is_rejected() {
    let src = format!(
        "const D : u32 = 4\n\nmodule F {{\n{}}}\n",
        FIFO.replace("DEPTH", "D")
    );
    assert_codes(&check("r4-const", &src), 1, &["E2008"]);
}

#[test]
fn row4_a_generic_parameter_depth_compiles() {
    let src = format!(
        "module F<const D: u32> {{\n{}}}\n\nmodule Top {{\n    in  clk : clock\n    \
         in  d   : u8\n    in  go  : bool\n    out q   : u8\n    \
         let f = F<4> {{ clk: clk, d: d, go: go }}\n    q = f.q\n}}\n",
        FIFO.replace("DEPTH", "D")
    );
    assert_codes(&check("r4-generic", &src), 0, &[]);
}

// ── Satır 6: test dilinde tasarım sabiti (reddedilen) ────────────────
// Test dili yalnız düz literal const'u görür (ADR-0060); hesaplanan const
// E8506 alır, değeri sessizce yanlış okunmaz.

const ECHO: &str =
    "module Echo {\n    in  _clk : clock\n    in  addr : u3\n    out echo : u3\n    \
                    echo = addr\n}\n";

#[test]
fn row6_a_computed_constant_is_not_visible_in_a_test() {
    let src = format!(
        "const LAST : u8 = 3 + 4\n\n{ECHO}\ntest \"computed\" {{\n    let dut = Echo {{ }};\n    \
         dut.addr = LAST;\n    step(1);\n    assert_eq(dut.echo, LAST);\n}}\n"
    );
    assert_codes(&check("r6-computed", &src), 1, &["E8506", "E8506"]);
}

/// Düz literal const görülür ve sığma denetimine girer (ADR-0059).
#[test]
fn row6_a_literal_constant_is_checked_against_the_port() {
    let src = format!(
        "const BIG : u8 = 8\n\n{ECHO}\ntest \"literal\" {{\n    let dut = Echo {{ }};\n    \
         dut.addr = BIG;\n    step(1);\n}}\n"
    );
    let run = check("r6-literal", &src);
    assert_codes(&run, 1, &["E8512"]);
    assert!(run.report.contains("BIG = 8"), "{}", run.report);
}

// ── Satır 7: takma adlı saat/reset (#234 ile kapandı; kalan #415) ─────
// #234'ün yerleri `aliased_clock_reset_tests.rs`'te üç biçimle sınanır.
// Kalan sınır #415 (açık hata): ayrıştırma sırasında koşan iki geçit
// kendisinden sonra bildirilen takma adı görmez. #415 düzeltilince bu iki
// test, takma adın yerinden bağımsız olarak temiz derlemeyi ister.

const PIPELINE: &str = "pipeline(2) P {\n    in  clk : Clk\n    in  x   : u32\n    \
                        out y   : u32\n    reg ry : u32 = 0\n    stage A {\n        \
                        let a : u32 = x + 1\n    }\n    stage B {\n        ry <= a\n    }\n    \
                        y = ry\n}\n";

#[test]
fn row7_issue_415_an_alias_after_a_pipeline_is_not_its_clock() {
    let before = check("r7-before", &format!("type Clk = clock\n\n{PIPELINE}"));
    assert_codes(&before, 0, &[]);
    let after = check("r7-after", &format!("{PIPELINE}\ntype Clk = clock\n"));
    assert_codes(&after, 1, &["E5011"]);
}

#[test]
fn row7_issue_415_the_e0019_hint_misses_an_alias_after_the_module() {
    let module = "module M {\n    in  clk : Clk\n    in  d   : u8\n    out q   : u8\n    \
                  reg r : u8 = 0\n    r <= d\n    q = r\n}\n";
    let before = check("r7-hint-before", &format!("type Clk = clock\n\n{module}"));
    assert!(
        before.report.contains("on clk { r <= d }"),
        "{}",
        before.report
    );
    let after = check("r7-hint-after", &format!("{module}\ntype Clk = clock\n"));
    assert_eq!(before.codes, after.codes, "{}", after.report);
    assert!(
        after.report.contains("on <clock> { r <= d }"),
        "{}",
        after.report
    );
}

// ── Satır 10: struct yaprak adı (gizli) ───────────────────────────────
// `Leaf::suffix`, üreticinin `leaf_name`'i, açılımın `path.join("_")`'i
// ve test dilinin `sv_port_name`'i aynı adı yazar: `p.i.x` → `p_i_x`
// (ADR-0039 §14; ad biçimini o ADR tanımlar, bu yüzden metin
// karşılaştırılır). Çakışan ad E1003 alır (ADR-0090).

const LEAVES: &str =
    "struct Inner {\n    x : u4\n}\n\nstruct Pair {\n    i : Inner\n    b : bool\n}\n\n\
domain A {\n    clock = posedge\n    frequency = 100.mhz\n}\n\n\
domain B {\n    clock = posedge\n    frequency = 50.mhz\n}\n\n\
module Cross {\n    in  _a_clk : clock @A\n    in  b_clk : clock @B\n    in  p : Pair @A\n    \
out q : Pair @B\n    q = sync(p, b_clk)\n}\n\n\
test \"struct leaves keep one name\" {\n    let dut = Cross { };\n    dut.p.i.x = 9;\n    \
dut.p.b = true;\n    step(8);\n    assert_eq(dut.q.i.x, 9);\n    assert_eq(dut.q.b, true);\n}\n";

#[test]
fn row10_struct_leaves_have_one_name_in_sv_sdc_and_tests() {
    let run = build("r10", LEAVES);
    // Beş bitlik yapının iki bayraklı senkronu W3003 (domain-inference.md).
    assert_codes(&run, 0, &["W3003"]);
    let text = |suffix: &str| {
        run.files
            .iter()
            .find(|(p, _)| p.ends_with(suffix))
            .map(|(_, t)| t.clone())
            .unwrap_or_else(|| panic!("{suffix} yok: {:?}", run.files))
    };
    let sv = text(".sv");
    for port in [
        "input  logic [3:0] p_i_x",
        "input  logic       p_b",
        "output logic [3:0] q_i_x",
        "output logic       q_b",
    ] {
        assert!(sv.contains(port), "`{port}` yok:\n{sv}");
    }
    let sdc = text("Cross.sdc");
    for leaf in ["sync_p_i_x", "sync_p_b"] {
        assert!(sv.contains(&format!("{leaf}_src")), "SV'de {leaf}_src yok");
        assert!(
            sdc.contains(&format!("{{{leaf}_src_reg*}}")),
            "SDC'de {leaf} yok:\n{sdc}"
        );
    }
}

/// Test dili aynı yaprağı sürer ve okur: Verilator'da uçtan uca.
#[test]
fn row10_a_test_drives_and_reads_struct_leaves_by_their_one_name() {
    if tools::require(tools::Tool::Verilator).is_none() {
        return;
    }
    let dir = temp_dir("r10-sim");
    std::fs::write(dir.join("leaf_test.volt"), LEAVES).expect("yaz");
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_TOOL_BACKEND", "local")
        .env("VOLT_LANG", "en")
        .current_dir(&dir)
        .args(["test", "leaf_test.volt", "--target-dir", "build"])
        .output()
        .expect("volt test");
    let out = format!(
        "{}\n--- stderr\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(output.status.code(), Some(0), "{out}");
    assert!(out.contains("test result: ok. 1 passed; 0 failed"), "{out}");
}

// ── Satır 11: değer aralığı (gizli) ───────────────────────────────────
// "Sığar mı" tip denetiminde yedi kez yazılmış; uN/iN 64 bitle sınırlı
// (E0003) ve yedisi 1..=64 bitte aynı sınırı verir. Literal bağlamları
// E2010, kesen `as` W2010 (type-inference.md §4, §6). Test dili yazılan
// değeri bit deseni olarak da kabul eder (ADR-0059 §2: `i8`'e 255): bu
// tasarlanmış farktır, ayrışma değil.

fn fits_module(ty: &str, value: &str) -> String {
    format!(
        "module F {{\n    in  a : {ty}\n    out y : {ty}\n    out e : bool\n    out m : bool\n    \
         out c : {ty}\n    y = {value}\n    e = a == {value}\n    \
         m = match a {{ {value} => true, _ => false }}\n    c = ({value} as {ty})\n}}\n"
    )
}

#[test]
fn row11_every_literal_context_has_the_same_range() {
    for (ty, max, over) in [
        ("u8", "255", "256"),
        ("i8", "127", "128"),
        ("u64", "18446744073709551615", "18446744073709551616"),
        ("i64", "9223372036854775807", "9223372036854775808"),
    ] {
        let inside = check(&format!("r11-{ty}-in"), &fits_module(ty, max));
        assert_codes(&inside, 0, &[]);
        let outside = check(&format!("r11-{ty}-out"), &fits_module(ty, over));
        // atama, karşılaştırma, desen: E2010; `as`: W2010.
        assert_codes(&outside, 1, &["E2010", "E2010", "E2010", "W2010"]);
    }
}

#[test]
fn row11_a_test_writes_values_in_the_port_range_or_bit_pattern() {
    let module = "module P {\n    in  _clk : clock\n    in  u : u8\n    in  s : i8\n    \
                  out y : u8\n    y = u ^ (s as u8)\n}\n";
    let test = |stmt: &str| {
        format!(
            "{module}\ntest \"range\" {{\n    let dut = P {{ }};\n    {stmt};\n    step(1);\n}}\n"
        )
    };
    for ok in [
        "dut.u = 255",
        "dut.s = 127",
        "dut.s = 255",
        "dut.s = 0 - 128",
    ] {
        assert_codes(&check("r11-test-ok", &test(ok)), 0, &[]);
    }
    for bad in ["dut.u = 256", "dut.s = 256", "dut.s = 0 - 129"] {
        assert_codes(&check("r11-test-bad", &test(bad)), 1, &["E8512"]);
    }
}
