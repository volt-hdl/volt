//! Bir `domain` bildiriminde her alan anahtarı en çok bir kez yazılır
//! (#420, ADR-0140). Bakımcının #420'deki kararı: aynı alanın ikinci kez
//! yazılması E1003'tür (aynı kapsamda çift tanım) ve ileti iki yeri de
//! gösterir; kural yalnız `reset` için değil, ayrıştırıcının tanıdığı her
//! anahtar için geçerlidir.
//!
//! Önce bildirimi okuyan geçitler yinelenen alanda ayrışıyordu: SV ve RDC
//! son `reset` alanını alıyor, kısıt modeli "bir alan `none` diyorsa
//! reset'siz" sayıyordu. `reset = none` ardından `reset = sync
//! active_high` tanısız derleniyor, SV reset senkronizörünü üretiyor, XDC
//! onun `ASYNC_REG` satırını yazmıyordu. Beklenen değerler elle, bu
//! karardan ve ADR-0054'ün XDC satırından yazılmıştır.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-domfield-{tag}-{}", std::process::id()));
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
    /// `out/` altındaki dosyalar: `/` ayraçlı göreli ad ve içerik.
    files: Vec<(String, String)>,
}

impl Run {
    fn file(&self, suffix: &str) -> &str {
        self.files
            .iter()
            .find(|(name, _)| name.ends_with(suffix))
            .map(|(_, text)| text.as_str())
            .unwrap_or_else(|| panic!("'{suffix}' üretilmeli:\n{}", self.report))
    }
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
    collect(&dir.join("out"), "", &mut files);
    files.sort();
    let _ = std::fs::remove_dir_all(&dir);
    Run {
        exit: out.status.code(),
        codes: codes(&report),
        report,
        files,
    }
}

fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            collect(&e.path(), &format!("{name}/"), out);
        } else {
            let text = std::fs::read_to_string(e.path()).unwrap_or_default();
            out.push((name, text));
        }
    }
}

fn check(tag: &str, src: &str) -> Run {
    volt(tag, &["check"], src)
}

/// SV, SDC ve XDC (ADR-0054).
fn build(tag: &str, src: &str) -> Run {
    volt(
        tag,
        &[
            "build",
            "--single-file",
            "--emit=sdc,xdc",
            "--target-dir",
            "out",
        ],
        src,
    )
}

/// `volt check --format json`'un tanıları.
fn json_diagnostics(tag: &str, src: &str) -> Vec<Value> {
    let run = volt(tag, &["check", "--format", "json"], src);
    let start = run.report.find('{').expect("JSON rapor");
    let mut stream = serde_json::Deserializer::from_str(&run.report[start..]).into_iter::<Value>();
    let doc = stream.next().expect("JSON belge").expect("geçerli JSON");
    doc["diagnostics"].as_array().cloned().unwrap_or_default()
}

/// Çıkış kodu ve tanı kodları tam olarak bunlar.
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

/// #420'nin örneği, sorundaki biçimiyle.
const ISSUE_420: &str = "\
domain Sys {
    clock = posedge
    frequency = 50.mhz
    reset = none
    reset = sync active_high
}

module M {
    in  clk : clock @Sys
    in  rst : reset(sync, active_high)
    in  d   : u8
    out q   : u8
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}
";

/// `Sys` alanının gövdesi `fields` olan tek saatli tasarım.
fn design(fields: &str) -> String {
    format!(
        "domain Sys {{\n{fields}}}\n\n\
         module M {{\n    in  clk : clock @Sys\n    in  d   : u8\n    out q   : u8\n    \
         reg r : u8 = 0\n    on clk {{ r <= d }}\n    q = r\n}}\n"
    )
}

/// Her anahtar bir kez: tasarım temiz derlenir.
const ONCE: &str = "    clock = posedge\n    frequency = 50.mhz\n    reset = sync active_high\n    trust_level = public\n";

#[test]
fn issue_420_a_reset_written_twice_is_e1003_and_nothing_is_written() {
    let run = build("i420", ISSUE_420);
    assert_codes(&run, 1, &["E1003"]);
    assert!(
        run.files.is_empty(),
        "hatalı birim dosya üretmemeli: {:?}",
        run.files.iter().map(|f| &f.0).collect::<Vec<_>>()
    );
    assert!(
        run.report
            .contains("field 'reset' is written twice in domain 'Sys'"),
        "ileti alanı ve domain'i adlandırmalı:\n{}",
        run.report
    );
}

/// İleti iki yeri gösterir: birincil etiket ikinci alanda (satır 5),
/// ikincil etiket ilk alanda (satır 4).
#[test]
fn issue_420_the_diagnostic_points_at_both_fields() {
    let diags = json_diagnostics("i420json", ISSUE_420);
    let e1003: Vec<&Value> = diags.iter().filter(|d| d["code"] == "E1003").collect();
    assert_eq!(e1003.len(), 1, "{diags:#?}");
    let spans = e1003[0]["spans"].as_array().expect("spans");
    let line = |primary: bool| -> Vec<u64> {
        spans
            .iter()
            .filter(|s| s["primary"] == primary)
            .map(|s| s["start"]["line"].as_u64().expect("satır"))
            .collect()
    };
    assert_eq!(
        line(true),
        vec![5],
        "birincil: ikinci reset alanı\n{spans:#?}"
    );
    assert_eq!(line(false), vec![4], "ikincil: ilk reset alanı\n{spans:#?}");
    assert!(
        e1003[0]["help"].as_str().is_some_and(|h| !h.is_empty()),
        "düzeltme önerisi olmalı: {:#?}",
        e1003[0]
    );
}

/// Sıra fark etmez: önce `sync`, sonra `none` da E1003.
#[test]
fn issue_420_the_other_order_is_e1003_too() {
    let src = ISSUE_420.replace(
        "    reset = none\n    reset = sync active_high\n",
        "    reset = sync active_high\n    reset = none\n",
    );
    assert_codes(&build("i420rev", &src), 1, &["E1003"]);
}

#[test]
fn every_key_once_compiles_clean() {
    assert_codes(&check("once", &design(ONCE)), 0, &[]);
}

/// Ayrıştırıcının tanıdığı her anahtar (`parse_domain_field`): ikinci
/// yazımı E1003'tür. `reset_cycles` ve `reset_sequence` ayrılmıştır: her
/// yazımı ayrıca E0003 alır (ADR-0098).
#[test]
fn every_key_written_twice_is_e1003() {
    let cases: [(&str, &str, &[&str]); 6] = [
        ("clock", "    clock = negedge\n", &["E1003"]),
        ("frequency", "    frequency = 100.mhz\n", &["E1003"]),
        ("reset", "    reset = none\n", &["E1003"]),
        ("trust_level", "    trust_level = public\n", &["E1003"]),
        (
            "reset_cycles",
            "    reset_cycles = 4\n    reset_cycles = 8\n",
            &["E0003", "E0003", "E1003"],
        ),
        (
            "reset_sequence",
            "    reset_sequence = 1\n    reset_sequence = 2\n",
            &["E0003", "E0003", "E1003"],
        ),
    ];
    let mut wrong = Vec::new();
    for (key, extra, want) in cases {
        let run = check(key, &design(&format!("{ONCE}{extra}")));
        let mut want: Vec<String> = want.iter().map(|c| c.to_string()).collect();
        want.sort();
        let message = format!("field '{key}' is written twice in domain 'Sys'");
        if run.exit != Some(1) || run.codes != want || !run.report.contains(&message) {
            wrong.push(format!(
                "{key}: çıkış {:?}, kodlar {:?}; beklenen 1, {want:?} ve '{message}'",
                run.exit, run.codes
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Aynı değerin yinelenmesi de E1003'tür: alan iki kez yazılmıştır.
#[test]
fn the_same_value_twice_is_e1003() {
    let src = design("    clock = posedge, clock = posedge\n");
    assert_codes(&check("samevalue", &src), 1, &["E1003"]);
}

/// Üçüncü yazım da ilkine karşı raporlanır: her fazla alan bir E1003.
#[test]
fn every_repeat_is_reported() {
    let src = design("    reset = none\n    reset = none\n    reset = none\n");
    assert_codes(&check("three", &src), 1, &["E1003", "E1003"]);
}

/// Kapsam bildirimdir: iki ayrı domain aynı anahtarı birer kez yazar.
#[test]
fn the_same_key_in_two_domains_is_no_duplicate() {
    let src = "\
domain A {
    clock = posedge
    reset = none
}

domain B {
    clock = posedge
    reset = none
}

module M {
    in  a_clk : clock @A
    in  b_clk : clock @B
    in  d     : bool @A
    out q     : bool @B
    reg r : bool = false
    on a_clk { r <= d }
    q = sync(r, b_clk)
}
";
    assert_codes(&check("twodomains", src), 0, &[]);
}

/// Bilinmeyen anahtar bir alan değildir: W0020 alır, hiçbir geçit
/// okumaz; ikinci yazımı E1003 almaz (ADR-0140, Sınırlar).
#[test]
fn an_unknown_key_twice_is_only_w0020() {
    let src = design("    clock = posedge\n    jitter = 1\n    jitter = 2\n");
    assert_codes(&check("unknown", &src), 0, &["W0020", "W0020"]);
}

/// Tek `reset` alanında SV ve kısıtlar aynı cevabı verir: senkron reset
/// senkronizör üretir ve XDC onun zincirine `ASYNC_REG` yazar (ADR-0054,
/// XDC satırı; ADR-0065 §2'nin `rst_sync_<saat>_stage<i>` adları);
/// `reset = none` ikisini de üretmez.
#[test]
fn one_reset_field_gives_the_sv_and_the_constraints_one_answer() {
    let raw = "    in  rst : reset(sync, active_high)\n";
    let with_reset = |fields: &str| {
        design(fields).replace("    in  d   : u8\n", &format!("{raw}    in  d   : u8\n"))
    };
    let chain = "set_property ASYNC_REG TRUE [get_cells {rst_sync_clk_stage0_reg* rst_sync_clk_stage1_reg*}]";

    let sync = build(
        "onesync",
        &with_reset("    clock = posedge\n    frequency = 50.mhz\n    reset = sync active_high\n"),
    );
    assert_codes(&sync, 0, &[]);
    assert!(
        sync.file(".sv").contains("rst_sync_clk_stage1"),
        "{}",
        sync.file(".sv")
    );
    assert!(sync.file("M.xdc").contains(chain), "{}", sync.file("M.xdc"));

    let none = build(
        "onenone",
        &with_reset("    clock = posedge\n    frequency = 50.mhz\n    reset = none\n"),
    );
    assert_codes(&none, 0, &[]);
    assert!(
        !none.file(".sv").contains("rst_sync_clk"),
        "{}",
        none.file(".sv")
    );
    assert!(
        !none.file("M.xdc").contains("ASYNC_REG"),
        "{}",
        none.file("M.xdc")
    );
}
