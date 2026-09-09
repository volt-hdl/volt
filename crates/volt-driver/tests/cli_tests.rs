//! volt CLI entegrasyon testleri — cli-contract.md §2 çıkış kodları,
//! §5 build/format, F2c aşamalı anlamsal boru hattı (CDC dahil).

use std::path::PathBuf;
use std::process::Command;

fn volt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_volt"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures"))
}

fn ui(rel: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ui")).join(rel)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-cli-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

#[test]
fn build_counter_succeeds_and_matches_expected() {
    let target = temp_dir("build-ok");
    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(fixtures().join("counter.volt"))
        .output()
        .expect("volt çalışmalı");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sv_path = target.join("rtl").join("counter.sv");
    let produced = std::fs::read_to_string(&sv_path).expect("counter.sv üretilmeli");
    let expected =
        std::fs::read_to_string(fixtures().join("counter.expected.sv")).expect("beklenen");
    assert_eq!(produced, expected, "CLI çıktısı da birebir eşleşmeli");

    // cli-contract.md §5 ilerleme mesajları (stderr'de, §11) — varsayılan dil EN
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Compiling"), "stderr: {stderr}");
    assert!(stderr.contains("Finished"));
    assert!(stderr.contains("Output"));
    assert!(stderr.contains("counter.sv"));
    assert!(stderr.contains("lines)"));

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn build_missing_file_is_io_error_exit_3() {
    let output = volt()
        .args(["build", "boyle-bir-dosya-yok.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn build_compile_error_exit_1() {
    let target = temp_dir("build-err");
    let bad = target.join("bozuk.volt");
    std::fs::write(&bad, "module M { in a : }").expect("yazılmalı");

    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(&bad)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("build failed"), "stderr: {stderr}");
    assert!(
        !target.join("rtl").join("bozuk.sv").exists(),
        "hatalı build çıktı üretmemeli"
    );

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn usage_error_exit_2() {
    let output = volt()
        .args(["build", "--boyle-bayrak-yok"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn check_counter_exit_0() {
    let output = volt()
        .arg("check")
        .arg(fixtures().join("counter.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Result 0 error"), "stderr: {stderr}");
}

// ═══ F2c: CDC kontrolü CLI'da (Volt'un vaadi) ═════════════════════

#[test]
fn check_cdc_violation_exit_1_with_e3001() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E3001"), "stderr: {stderr}");
    // 5 parça: çözüm (help) satırı sync() önermeli — varsayılan dil EN.
    assert!(stderr.contains("= help"), "stderr: {stderr}");
    assert!(stderr.contains("sync("), "stderr: {stderr}");
}

#[test]
fn check_single_clock_pass_exit_0_no_diagnostics() {
    let output = volt()
        .arg("check")
        .arg(ui("pass/14_single_clock_no_domain.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Result 0 error(s), 0 warning(s)"),
        "stderr: {stderr}"
    );
    // UX Anayasası: hiçbir tanı yok — kullanıcı 'domain' kavramını
    // görmez (dosya YOLU 'no_domain' içerdiğinden tanı satırı sayılır).
    assert!(!stderr.contains("error["), "stderr: {stderr}");
    assert!(!stderr.contains("warning["), "stderr: {stderr}");
}

#[test]
fn check_cdc_bridge_with_sync_exit_0() {
    let output = volt()
        .arg("check")
        .arg(ui("pass/13_cdc_correct_bridge.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Result 0 error"), "stderr: {stderr}");
}

#[test]
fn build_cdc_violation_exit_1_no_sv_output() {
    let target = temp_dir("build-cdc");
    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E3001"), "stderr: {stderr}");
    assert!(stderr.contains("build failed"), "stderr: {stderr}");
    assert!(
        !target.join("rtl").join("01_cdc_violation.sv").exists(),
        "CDC ihlali SV üretmemeli — Volt'un vaadi"
    );

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn check_typeck_error_via_cli_e2002() {
    // Aşama 3 (tip kontrolü) CLI'dan da çalışıyor.
    let output = volt()
        .arg("check")
        .arg(ui("fail/08_signedness_mismatch.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E2002"), "stderr: {stderr}");
}

#[test]
fn stage_gating_resolve_error_stops_pipeline() {
    // E1001 (aşama 2) varken sonraki aşamaların kodları görünmemeli.
    let output = volt()
        .arg("check")
        .arg(ui("fail/19_undefined_name.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(
        !stderr.contains("error[E2") && !stderr.contains("error[E3"),
        "kaskad tanı olmamalı: {stderr}"
    );
}

// ═══ --format=json / --format=short (cli-contract.md §5) ══════════

#[test]
fn check_json_format_cdc_violation() {
    let output = volt()
        .args(["check", "--format", "json"])
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout geçerli JSON olmalı");
    assert_eq!(envelope["version"], "1");
    assert_eq!(envelope["command"], "check");
    assert_eq!(envelope["success"], false);
    // Hatalar önce sıralanır: CI ilk kayıtta engelleyiciyi görür.
    assert_eq!(envelope["diagnostics"][0]["code"], "E3001");
    assert_eq!(envelope["diagnostics"][0]["severity"], "error");
    assert_eq!(envelope["summary"]["errors"], 1);
    assert!(envelope["diagnostics"][0]["explain_url"]
        .as_str()
        .unwrap()
        .contains("E3001"));
}

#[test]
fn check_json_format_clean_file() {
    let output = volt()
        .args(["check", "--format", "json"])
        .arg(ui("pass/14_single_clock_no_domain.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout geçerli JSON olmalı");
    assert_eq!(envelope["success"], true);
    assert_eq!(envelope["summary"]["errors"], 0);
    assert_eq!(envelope["diagnostics"].as_array().unwrap().len(), 0);
}

#[test]
fn build_json_format_lists_artifact() {
    let target = temp_dir("build-json");
    let output = volt()
        .args(["build", "--format", "json", "--target-dir"])
        .arg(&target)
        .arg(fixtures().join("counter.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout geçerli JSON olmalı");
    assert_eq!(envelope["command"], "build");
    assert_eq!(envelope["success"], true);
    let artifacts = envelope["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].as_str().unwrap().contains("counter.sv"));

    let _ = std::fs::remove_dir_all(&target);
}

// ═══ --lang / VOLT_LANG / Volt.toml [ui] lang (cli-contract.md §3) ═

#[test]
fn default_lang_is_english() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/01_cdc_violation.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E3001]"), "stderr: {stderr}");
    assert!(stderr.contains("= reason:"), "stderr: {stderr}");
    assert!(stderr.contains("= help:"), "stderr: {stderr}");
    assert!(
        stderr.contains("= for more: volt explain E3001"),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains("= çözüm"),
        "EN çıktıda Türkçe anahtar olmamalı: {stderr}"
    );
}

#[test]
fn lang_flag_tr_switches_output_to_turkish() {
    let output = volt()
        .args(["build", "--lang=tr", "--target-dir"])
        .arg(temp_dir("lang-flag"))
        .arg(ui("fail/01_cdc_violation.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E3001]"), "stderr: {stderr}");
    assert!(stderr.contains("= çözüm:"), "stderr: {stderr}");
    assert!(stderr.contains("= neden:"), "stderr: {stderr}");
    assert!(stderr.contains("sync("), "stderr: {stderr}");
    assert!(stderr.contains("derleme başarısız"), "stderr: {stderr}");
}

#[test]
fn volt_lang_env_tr_switches_output_to_turkish() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/01_cdc_violation.volt"))
        .env("VOLT_LANG", "tr")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E3001]"), "stderr: {stderr}");
    assert!(stderr.contains("= çözüm:"), "stderr: {stderr}");
}

#[test]
fn lang_flag_overrides_volt_lang_env() {
    let output = volt()
        .args(["check", "--lang=en"])
        .arg(ui("fail/01_cdc_violation.volt"))
        .env("VOLT_LANG", "tr")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("= help:"), "stderr: {stderr}");
    assert!(!stderr.contains("= çözüm"), "stderr: {stderr}");
}

#[test]
fn volt_toml_ui_lang_tr_used_when_no_flag_or_env() {
    let dir = temp_dir("toml-lang");
    std::fs::write(dir.join("Volt.toml"), "[ui]\nlang = \"tr\"\n").expect("Volt.toml");
    let output = volt()
        .arg("check")
        .arg(ui("fail/01_cdc_violation.volt"))
        .current_dir(&dir)
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("= çözüm:"), "stderr: {stderr}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn same_error_code_in_both_languages() {
    // İki dilde de AYNI E-kodu üretilmeli; yalnız metin dili değişir.
    let run = |lang: &str| {
        let output = volt()
            .args(["check", "--lang", lang])
            .arg(ui("fail/01_cdc_violation.volt"))
            .env_remove("VOLT_LANG")
            .output()
            .expect("volt çalışmalı");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let (en_code, en_err) = run("en");
    let (tr_code, tr_err) = run("tr");
    assert_eq!(en_code, Some(1));
    assert_eq!(tr_code, Some(1));
    assert!(en_err.contains("error[E3001]"), "en stderr: {en_err}");
    assert!(tr_err.contains("error[E3001]"), "tr stderr: {tr_err}");
    assert!(en_err.contains("= help:"), "en stderr: {en_err}");
    assert!(tr_err.contains("= çözüm:"), "tr stderr: {tr_err}");
}

// ═══ volt explain (cli-contract.md §9) ════════════════════════════

#[test]
fn explain_e3001_exit_0_with_spec_structure() {
    let output = volt()
        .args(["explain", "E3001"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // §9 yapısı: başlık, neden, örnek, çözüm, docs linki (stdout, §11).
    assert!(stdout.starts_with("E3001: "), "stdout: {stdout}");
    assert!(stdout.contains("WHY THIS IS A PROBLEM"), "stdout: {stdout}");
    assert!(stdout.contains("EXAMPLE"), "stdout: {stdout}");
    assert!(stdout.contains("SOLUTION"), "stdout: {stdout}");
    assert!(stdout.contains("FOR MORE"), "stdout: {stdout}");
    assert!(stdout.contains("https://volthdl.org/errors/E3001"));
    assert!(stdout.contains("sync("), "stdout: {stdout}");
}

#[test]
fn explain_unknown_code_exit_2() {
    let output = volt()
        .args(["explain", "E9999"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown code 'E9999'"), "stderr: {stderr}");
    assert!(output.stdout.is_empty(), "bilinmeyen kod stdout üretmemeli");
}

#[test]
fn explain_unknown_code_turkish_message() {
    let output = volt()
        .args(["explain", "--lang=tr", "E9999"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bilinmeyen kod 'E9999'"),
        "stderr: {stderr}"
    );
}

#[test]
fn explain_typo_gets_suggestion() {
    let output = volt()
        .args(["explain", "E1000"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("did you mean 'E1001'?"), "stderr: {stderr}");
}

#[test]
fn explain_lang_tr_renders_turkish_sections() {
    let output = volt()
        .args(["explain", "--lang=tr", "E3001"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NEDEN SORUN"), "stdout: {stdout}");
    assert!(stdout.contains("ÖRNEK"), "stdout: {stdout}");
    assert!(stdout.contains("ÇÖZÜM"), "stdout: {stdout}");
    assert!(stdout.contains("DAHA FAZLA"), "stdout: {stdout}");
}

#[test]
fn explain_code_is_case_insensitive() {
    let output = volt()
        .args(["explain", "e3001"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("E3001: "));
}

#[test]
fn explain_list_shows_all_codes_by_category() {
    let output = volt()
        .args(["explain", "--list"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for cat in ["Syntax", "Name resolution", "Warnings"] {
        assert!(stdout.contains(cat), "kategori yok: {cat}\n{stdout}");
    }
    // Uçlardan örneklem: ilk kod, son kod ve aradaki kategoriler.
    for code in ["E0001", "E3001", "E9002", "W0010", "W4002"] {
        assert!(stdout.contains(code), "kod yok: {code}");
    }
}

#[test]
fn explain_without_args_is_usage_error_exit_2() {
    let output = volt().arg("explain").output().expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn explain_wraps_prose_to_columns_env() {
    let output = volt()
        .args(["explain", "E3001"])
        .env("COLUMNS", "50")
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        if line.starts_with("  ") {
            continue; // kod blokları ve linkler sarılmaz
        }
        assert!(line.chars().count() <= 50, "satır 50'yi aşıyor: {line:?}");
    }
}

#[test]
fn explain_color_always_emits_ansi_piped_default_does_not() {
    let colored = volt()
        .args(["explain", "--color=always", "E3001"])
        .output()
        .expect("volt çalışmalı");
    assert!(String::from_utf8_lossy(&colored.stdout).contains('\x1b'));

    // Boruya bağlı stdout'ta auto renk kapalı olmalı (§10).
    let piped = volt()
        .args(["explain", "E3001"])
        .output()
        .expect("volt çalışmalı");
    assert!(!String::from_utf8_lossy(&piped.stdout).contains('\x1b'));
}

#[test]
fn check_short_format_single_line_diagnostics() {
    let output = volt()
        .args(["check", "--format", "short"])
        .arg(ui("fail/01_cdc_violation.volt"))
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // dosya:satır:sütun: error[E3001]: mesaj — tek satır.
    assert!(
        stderr.lines().any(|l| l.contains(":21:5: error[E3001]:")),
        "stderr: {stderr}"
    );
}

// ═══ F4a — volt build --emit=sva ══════════════════════════════════

fn contract_source() -> &'static str {
    "module Uart {\n    in  clk   : clock\n    in  speed : u8\n    in  start : bool\n    out busy  : bool\n\n    requires: speed <= 2\n    invariant: !(busy_r && start)\n\n    reg busy_r : bool = false\n\n    on clk {\n        busy_r <= start\n    }\n\n    busy = busy_r\n}\n"
}

#[test]
fn build_emit_sva_writes_formal_file() {
    let target = temp_dir("emit-sva");
    let src = target.join("uart.volt");
    std::fs::write(&src, contract_source()).expect("yazılmalı");

    let output = volt()
        .args(["build", "--emit", "sva", "--target-dir"])
        .arg(&target)
        .arg(&src)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sva_path = target.join("formal").join("uart.sva");
    let sva = std::fs::read_to_string(&sva_path).expect("uart.sva üretilmeli");
    assert!(sva.contains("assume property (req_0);"), "{sva}");
    assert!(sva.contains("assert property (inv_0);"), "{sva}");
    assert!(sva.contains("bind Uart uart_sva sva_inst (.*);"), "{sva}");

    // Üretilen dosyalar listesinde SVA görünmeli (stderr, human format).
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("uart.sva"), "stderr: {stderr}");

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn build_emit_sva_json_lists_artifacts() {
    let target = temp_dir("emit-sva-json");
    let src = target.join("uart.volt");
    std::fs::write(&src, contract_source()).expect("yazılmalı");

    let output = volt()
        .args(["build", "--emit", "sva", "--format", "json", "--target-dir"])
        .arg(&target)
        .arg(&src)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("JSON zarfı");
    let artifacts: Vec<String> = json["artifacts"]
        .as_array()
        .expect("artifacts dizisi")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(
        artifacts.iter().any(|a| a.ends_with("uart.sv")),
        "{artifacts:?}"
    );
    assert!(
        artifacts.iter().any(|a| a.ends_with("uart.sva")),
        "{artifacts:?}"
    );

    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn build_sva_inline_embeds_properties_in_sv() {
    let target = temp_dir("sva-inline");
    let src = target.join("uart.volt");
    std::fs::write(&src, contract_source()).expect("yazılmalı");

    let output = volt()
        .args(["build", "--emit", "sva", "--sva", "inline", "--target-dir"])
        .arg(&target)
        .arg(&src)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sv = std::fs::read_to_string(target.join("rtl").join("uart.sv")).expect("uart.sv");
    assert!(sv.contains("property inv_0;"), "{sv}");
    assert!(
        !target.join("formal").exists(),
        "inline modda ayrı dosya yok"
    );

    let _ = std::fs::remove_dir_all(&target);
}

// ═══ Regresyon — --emit=sva yol biçimleri ═════════════════════════
// Dört yol biçimi de aynı sonucu vermeli: göreli, ./göreli, mutlak ve
// çalışma dizininden çıplak dosya adı. Varsayılan --target-dir (build/)
// sürecin çalışma dizinine göre çözülür.

#[test]
fn build_emit_sva_relative_path_writes_formal_file() {
    let root = temp_dir("sva-rel");
    std::fs::create_dir_all(root.join("src")).expect("src dizini");
    std::fs::write(root.join("src").join("uart.volt"), contract_source()).expect("yazılmalı");

    let output = volt()
        .current_dir(&root)
        .args(["build", "--emit", "sva", "src/uart.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let sva = std::fs::read_to_string(root.join("build").join("formal").join("uart.sva"))
        .expect("uart.sva üretilmeli");
    assert!(sva.contains("assert property (inv_0);"), "{sva}");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn build_emit_sva_dot_relative_path_writes_formal_file() {
    let root = temp_dir("sva-dot-rel");
    std::fs::create_dir_all(root.join("src")).expect("src dizini");
    std::fs::write(root.join("src").join("uart.volt"), contract_source()).expect("yazılmalı");

    let output = volt()
        .current_dir(&root)
        .args(["build", "--emit", "sva", "./src/uart.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.join("build").join("formal").join("uart.sva").exists(),
        "./ önekli yol .sva üretmeli"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn build_emit_sva_absolute_path_writes_formal_file() {
    let root = temp_dir("sva-abs");
    std::fs::create_dir_all(root.join("src")).expect("src dizini");
    let src = root.join("src").join("uart.volt");
    std::fs::write(&src, contract_source()).expect("yazılmalı");
    assert!(src.is_absolute(), "temp yolu mutlak olmalı");

    let output = volt()
        .current_dir(&root)
        .args(["build", "--emit", "sva"])
        .arg(&src)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        root.join("build").join("formal").join("uart.sva").exists(),
        "mutlak yol .sva üretmeli"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn build_emit_sva_bare_filename_in_source_dir() {
    let root = temp_dir("sva-bare");
    std::fs::create_dir_all(root.join("src")).expect("src dizini");
    std::fs::write(root.join("src").join("uart.volt"), contract_source()).expect("yazılmalı");

    let output = volt()
        .current_dir(root.join("src"))
        .args(["build", "--emit", "sva", "uart.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // build/ bu kez kaynak dizininin içinde açılır (çalışma dizini orası).
    assert!(
        root.join("src")
            .join("build")
            .join("formal")
            .join("uart.sva")
            .exists(),
        "çıplak dosya adı .sva üretmeli"
    );

    let _ = std::fs::remove_dir_all(&root);
}

// '--target-dir' değer beklerken yanına başka bayrak gelirse bu bir
// kullanım hatasıdır (cli-contract.md §2: çıkış 2) — yol çözümleme
// hatası değil. Mesaj eksik değeri açıkça söylemeli.
#[test]
fn build_target_dir_without_value_is_usage_error_exit_2() {
    let output = volt()
        .args(["build", "--target-dir", "--emit=sva", "uart.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("a value is required for '--target-dir"),
        "stderr: {stderr}"
    );
}

// ═══ F4b — volt verify (SymbiYosys entegrasyonu) ══════════════════

/// PATH'te gerçek sby var mı? (Gerçek-araç testleri yoksa SKIP eder.)
fn sby_on_path() -> bool {
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .any(|d| d.join("sby").is_file() || d.join("sby.exe").is_file())
        })
        .unwrap_or(false)
}

/// Sahte sby: verilen satırları basıp verilen kodla çıkan betik.
/// `volt verify` VOLT_SBY üzerinden bunu çağırır — sby kurulu olmayan
/// ortamda FAIL/PASS yorumlama yolları uçtan uca test edilir.
#[cfg(windows)]
fn write_fake_sby(dir: &std::path::Path, body: &[&str], exit: i32) -> PathBuf {
    let path = dir.join("sby.bat");
    let mut script = String::from("@echo off\r\n");
    for line in body {
        script.push_str(line);
        script.push_str("\r\n");
    }
    script.push_str(&format!("exit /b {exit}\r\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    path
}

#[cfg(unix)]
fn write_fake_sby(dir: &std::path::Path, body: &[&str], exit: i32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join("sby");
    let mut script = String::from("#!/bin/sh\n");
    for line in body {
        script.push_str(line);
        script.push('\n');
    }
    script.push_str(&format!("exit {exit}\n"));
    std::fs::write(&path, script).expect("sahte sby yazılmalı");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// FAIL basan sahte sby; `leakycounter` çalışma dizinine iz de yazar.
fn fake_fail_sby(dir: &std::path::Path) -> PathBuf {
    #[cfg(windows)]
    let body = [
        "mkdir leakycounter\\engine_0 2>nul",
        "echo dummy> leakycounter\\engine_0\\trace.vcd",
        "echo SBY [leakycounter] engine_0: ## 0:00:00 Checking assertions in step 7..",
        "echo SBY [leakycounter] engine_0: ## 0:00:00 Assert failed in LeakyCounter: leakycounter.sv:9999.1-9999.5",
        "echo SBY [leakycounter] DONE (FAIL, rc=2)",
    ];
    #[cfg(unix)]
    let body = [
        "mkdir -p leakycounter/engine_0",
        "echo dummy > leakycounter/engine_0/trace.vcd",
        "echo 'SBY [leakycounter] engine_0: ## 0:00:00 Checking assertions in step 7..'",
        "echo 'SBY [leakycounter] engine_0: ## 0:00:00 Assert failed in LeakyCounter: leakycounter.sv:9999.1-9999.5'",
        "echo 'SBY [leakycounter] DONE (FAIL, rc=2)'",
    ];
    write_fake_sby(dir, &body, 2)
}

/// PASS basan sahte sby.
fn fake_pass_sby(dir: &std::path::Path) -> PathBuf {
    #[cfg(windows)]
    let body = ["echo SBY [boundedcounter] DONE (PASS, rc=0)"];
    #[cfg(unix)]
    let body = ["echo 'SBY [boundedcounter] DONE (PASS, rc=0)'"];
    write_fake_sby(dir, &body, 0)
}

#[test]
fn verify_missing_file_exit_3() {
    let output = volt()
        .args(["verify", "boyle-bir-dosya-yok.volt"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn verify_compile_error_exit_1() {
    let target = temp_dir("verify-compile-err");
    let bad = target.join("bozuk.volt");
    std::fs::write(&bad, "module M { in a : }").expect("yazılmalı");
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(&bad)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_without_sby_prints_install_help_exit_3() {
    let target = temp_dir("verify-no-sby");
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("pass/23_provable_invariant.volt"))
        .env("PATH", "")
        .env_remove("VOLT_SBY")
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SymbiYosys not found"), "stderr: {stderr}");
    assert!(stderr.contains("= reason:"), "stderr: {stderr}");
    assert!(
        stderr.contains("apt install yosys z3, then pip install symbiyosys"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("docker pull hdlc/formal"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("use WSL or Docker"), "stderr: {stderr}");
    assert!(
        stderr.contains("'volt build' and 'volt check' do not need SymbiYosys"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("= for more: volt explain verify-setup"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_without_sby_turkish_install_help() {
    let target = temp_dir("verify-no-sby-tr");
    let output = volt()
        .args(["verify", "--lang=tr", "--target-dir"])
        .arg(&target)
        .arg(ui("pass/23_provable_invariant.volt"))
        .env("PATH", "")
        .env_remove("VOLT_SBY")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SymbiYosys bulunamadı"), "stderr: {stderr}");
    assert!(
        stderr.contains("= çözüm: kurulum seçenekleri:"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("= daha fazla: volt explain verify-setup"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_writes_sby_and_formal_sv_before_tool_lookup() {
    // Yapıtlar sby aranmadan ÖNCE üretilir — sby'siz ortam da .sby görür.
    let target = temp_dir("verify-artifacts");
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("pass/23_provable_invariant.volt"))
        .env("PATH", "")
        .env_remove("VOLT_SBY")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(3), "sby yok → 3");

    let sby = std::fs::read_to_string(target.join("formal").join("boundedcounter.sby"))
        .expect("boundedcounter.sby üretilmeli");
    assert!(sby.starts_with("[options]\nmode bmc\ndepth 20\n"), "{sby}");
    assert!(sby.contains("[engines]\nsmtbmc z3\n"), "{sby}");
    assert!(sby.contains("read -formal boundedcounter.sv"), "{sby}");
    assert!(sby.contains("prep -top BoundedCounter"), "{sby}");
    assert!(sby.contains("[files]\nboundedcounter.sv"), "{sby}");

    let sv = std::fs::read_to_string(target.join("formal").join("boundedcounter.sv"))
        .expect("boundedcounter.sv üretilmeli");
    // Yosys uyumu: immediate assertion + '// volt:' işareti; property
    // blokları Yosys'te ayrıştırılamıyor (bkz. volt-sv-emit/src/sby.rs).
    assert!(sv.contains("// volt:inv_0"), "{sv}");
    assert!(sv.contains("assert ("), "{sv}");
    assert!(!sv.contains("property inv_0;"), "{sv}");
    assert!(sv.contains("initial assume (rst);"), "{sv}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_depth_engine_mode_flags_change_sby() {
    let target = temp_dir("verify-flags");
    let _ = volt()
        .args([
            "verify",
            "--depth",
            "33",
            "--engine",
            "boolector",
            "--mode",
            "prove",
            "--target-dir",
        ])
        .arg(&target)
        .arg(ui("pass/23_provable_invariant.volt"))
        .env("PATH", "")
        .env_remove("VOLT_SBY")
        .output()
        .expect("volt çalışmalı");
    let sby = std::fs::read_to_string(target.join("formal").join("boundedcounter.sby"))
        .expect("boundedcounter.sby üretilmeli");
    assert!(sby.contains("mode prove\n"), "{sby}");
    assert!(sby.contains("depth 33\n"), "{sby}");
    assert!(sby.contains("smtbmc boolector\n"), "{sby}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_no_contracts_is_note_exit_0() {
    let target = temp_dir("verify-no-contracts");
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(fixtures().join("counter.volt"))
        .env("PATH", "")
        .env_remove("VOLT_SBY")
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no contracts found"), "stderr: {stderr}");
    assert!(
        !target.join("formal").exists(),
        "kontratsız tasarım formal çıktı üretmemeli"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_fake_sby_pass_exit_0() {
    let target = temp_dir("verify-fake-pass");
    let sby = fake_pass_sby(&target);
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("pass/23_provable_invariant.volt"))
        .env("VOLT_SBY", &sby)
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("1 property verified"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_fake_sby_fail_exit_6_with_counterexample() {
    let target = temp_dir("verify-fake-fail");
    let sby = fake_fail_sby(&target);
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/24_violated_invariant.volt"))
        .env("VOLT_SBY", &sby)
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(6),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E5001]"), "stderr: {stderr}");
    assert!(stderr.contains("contract violated"), "stderr: {stderr}");
    // Karşı örnek gösterimi: döngü etiketi, vcd yolu, açma yardımı.
    assert!(stderr.contains("violated at cycle 7"), "stderr: {stderr}");
    assert!(stderr.contains("= counterexample:"), "stderr: {stderr}");
    assert!(stderr.contains("leakycounter_cex.vcd"), "stderr: {stderr}");
    assert!(stderr.contains("gtkwave"), "stderr: {stderr}");
    assert!(
        stderr.contains("= for more: volt explain E5001"),
        "stderr: {stderr}"
    );
    assert!(
        target.join("formal").join("leakycounter_cex.vcd").is_file(),
        "karşı örnek vcd kopyalanmalı"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_fake_sby_fail_turkish_counterexample() {
    let target = temp_dir("verify-fake-fail-tr");
    let sby = fake_fail_sby(&target);
    let output = volt()
        .args(["verify", "--lang=tr", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/24_violated_invariant.volt"))
        .env("VOLT_SBY", &sby)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(6));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E5001]"), "stderr: {stderr}");
    assert!(stderr.contains("kontrat ihlal edildi"), "stderr: {stderr}");
    assert!(stderr.contains("= karşı örnek:"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_fake_sby_fail_json_reports_e5001() {
    let target = temp_dir("verify-fake-fail-json");
    let sby = fake_fail_sby(&target);
    let output = volt()
        .args(["verify", "--format", "json", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/24_violated_invariant.volt"))
        .env("VOLT_SBY", &sby)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(6));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("JSON zarfı");
    assert_eq!(envelope["command"], "verify");
    assert_eq!(envelope["success"], false);
    assert_eq!(envelope["diagnostics"][0]["code"], "E5001");
    let artifacts = envelope["artifacts"].as_array().expect("artifacts");
    assert!(
        artifacts
            .iter()
            .any(|a| a.as_str().unwrap_or("").ends_with("leakycounter.sby")),
        "{artifacts:?}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_provable_invariant_with_real_sby() {
    // Gerçek araç testi: sby kurulu değilse SKIP (CI'da opsiyonel job koşar).
    if !sby_on_path() {
        eprintln!("SKIP: sby PATH'te yok — kurulum için 'volt explain verify-setup'");
        return;
    }
    let target = temp_dir("verify-real-pass");
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("pass/23_provable_invariant.volt"))
        .env_remove("VOLT_SBY")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn verify_violated_invariant_with_real_sby_exit_6() {
    if !sby_on_path() {
        eprintln!("SKIP: sby PATH'te yok — kurulum için 'volt explain verify-setup'");
        return;
    }
    let target = temp_dir("verify-real-fail");
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/24_violated_invariant.volt"))
        .env_remove("VOLT_SBY")
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(6),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E5001]"), "stderr: {stderr}");
    assert!(stderr.contains("= counterexample:"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&target);
}

// ═══ F4b — volt explain: E5001 ve verify-setup konusu ═════════════

#[test]
fn explain_e5001_exit_0_with_spec_structure() {
    let output = volt()
        .args(["explain", "E5001"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("E5001: "), "stdout: {stdout}");
    assert!(stdout.contains("WHY THIS IS A PROBLEM"), "stdout: {stdout}");
    assert!(stdout.contains("counterexample"), "stdout: {stdout}");
    assert!(stdout.contains("gtkwave"), "stdout: {stdout}");
    assert!(stdout.contains("https://volthdl.org/errors/E5001"));
}

#[test]
fn explain_verify_setup_topic_exit_0() {
    let output = volt()
        .args(["explain", "verify-setup"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("verify-setup: "), "stdout: {stdout}");
    assert!(stdout.contains("INSTALL"), "stdout: {stdout}");
    assert!(
        stdout.contains("pip install symbiyosys"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("docker pull hdlc/formal"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("VOLT_SBY"), "stdout: {stdout}");
}

#[test]
fn explain_verify_setup_turkish() {
    let output = volt()
        .args(["explain", "--lang=tr", "verify-setup"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("KURULUM"), "stdout: {stdout}");
    assert!(stdout.contains("WSL ya da Docker"), "stdout: {stdout}");
}

#[test]
fn explain_unknown_topic_still_exit_2() {
    let output = volt()
        .args(["explain", "boyle-konu-yok"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "bilinmeyen konu stdout üretmemeli"
    );
}

// ═══ Kendi kendini belgeleme: komutsuz yardım, EXAMPLES, Next, konular ═

#[test]
fn no_command_prints_common_tasks_exit_0() {
    // Komutsuz çağrı hata değil, yol göstermedir (çıkış 0, stdout).
    let output = volt()
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Volt HDL"), "stdout: {stdout}");
    assert!(stdout.contains("No command given"), "stdout: {stdout}");
    for cmd in [
        "volt build",
        "volt run",
        "volt test",
        "volt verify",
        "volt explain E3001",
    ] {
        assert!(stdout.contains(cmd), "eksik görev: {cmd}\n{stdout}");
    }
    assert!(stdout.contains("volt --help"), "stdout: {stdout}");
}

#[test]
fn no_command_turkish_help() {
    let output = volt()
        .env("VOLT_LANG", "tr")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Komut verilmedi"), "stdout: {stdout}");
    assert!(stdout.contains("Kontratları kanıtla"), "stdout: {stdout}");
}

#[test]
fn top_level_help_has_examples_section() {
    let output = volt().arg("--help").output().expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("EXAMPLES:"), "stdout: {stdout}");
    assert!(
        stdout.contains("volt build counter.volt"),
        "stdout: {stdout}"
    );
}

#[test]
fn subcommand_helps_have_examples() {
    for (cmd, sample) in [
        ("build", "volt build --emit=sva design.volt"),
        ("check", "volt check design.volt"),
        ("verify", "volt verify --mode prove design.volt"),
        ("run", "volt run --vcd waves.vcd design.volt"),
        ("test", "volt test uart --nocapture"),
        ("explain", "volt explain --topics"),
    ] {
        let output = volt()
            .args([cmd, "--help"])
            .output()
            .expect("volt çalışmalı");
        assert_eq!(output.status.code(), Some(0), "{cmd} --help");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("EXAMPLES:"), "{cmd}: {stdout}");
        assert!(stdout.contains(sample), "{cmd}: {stdout}");
    }
}

#[test]
fn build_success_suggests_next_steps() {
    let target = temp_dir("build-next");
    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(fixtures().join("counter.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Next: volt run"), "stderr: {stderr}");
    assert!(stderr.contains("volt verify"), "stderr: {stderr}");
    assert!(stderr.contains("(simulate)"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn check_success_suggests_build() {
    let output = volt()
        .arg("check")
        .arg(fixtures().join("counter.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Next: volt build"), "stderr: {stderr}");
}

#[test]
fn check_failure_has_no_next_suggestion() {
    let output = volt()
        .arg("check")
        .arg(ui("fail/01_cdc_violation.volt"))
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("Next: volt build"), "stderr: {stderr}");
}

#[test]
fn verify_failure_suggests_explain_e5001() {
    let target = temp_dir("verify-next");
    let sby = fake_fail_sby(&target);
    let output = volt()
        .args(["verify", "--target-dir"])
        .arg(&target)
        .arg(ui("fail/24_violated_invariant.volt"))
        .env("VOLT_SBY", &sby)
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(6));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Next: volt explain E5001"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&target);
}

#[test]
fn explain_topics_lists_all_topics_exit_0() {
    let output = volt()
        .args(["explain", "--topics"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for topic in [
        "getting-started",
        "domains",
        "contracts",
        "stdlib",
        "verify-setup",
        "simulation-setup",
    ] {
        assert!(stdout.contains(topic), "eksik konu: {topic}\n{stdout}");
    }
}

#[test]
fn explain_topics_turkish() {
    let output = volt()
        .args(["explain", "--topics", "--lang=tr"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Konular"), "stdout: {stdout}");
    assert!(stdout.contains("saat alanları"), "stdout: {stdout}");
}

#[test]
fn explain_domains_topic_exit_0() {
    let output = volt()
        .args(["explain", "domains"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("domains: "), "stdout: {stdout}");
    assert!(stdout.contains("E3001"), "stdout: {stdout}");
    assert!(stdout.contains("domain Fast"), "stdout: {stdout}");
}

#[test]
fn explain_contracts_topic_mentions_implication() {
    let output = volt()
        .args(["explain", "contracts"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("requires"), "stdout: {stdout}");
    assert!(stdout.contains("!busy -> tx"), "stdout: {stdout}");
    assert!(stdout.contains("--mode prove"), "stdout: {stdout}");
}

#[test]
fn explain_stdlib_topic_lists_components() {
    let output = volt()
        .args(["explain", "stdlib"])
        .env_remove("VOLT_LANG")
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    for prim in ["AsyncFifo", "SyncFifo", "Ram", "EdgeDetect", "Counter"] {
        assert!(stdout.contains(prim), "eksik bileşen: {prim}\n{stdout}");
    }
}

#[test]
fn explain_getting_started_topic_turkish() {
    let output = volt()
        .args(["explain", "--lang=tr", "getting-started"])
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("İlk Volt tasarımınız"), "stdout: {stdout}");
    assert!(stdout.contains("volt build blink.volt"), "stdout: {stdout}");
}

#[test]
fn build_without_emit_sva_stays_rtl_only() {
    let target = temp_dir("no-sva");
    let src = target.join("uart.volt");
    std::fs::write(&src, contract_source()).expect("yazılmalı");

    let output = volt()
        .args(["build", "--target-dir"])
        .arg(&target)
        .arg(&src)
        .output()
        .expect("volt çalışmalı");
    assert_eq!(output.status.code(), Some(0));

    let sv = std::fs::read_to_string(target.join("rtl").join("uart.sv")).expect("uart.sv");
    assert!(!sv.contains("property"), "varsayılan build SVA içermemeli");
    assert!(!target.join("formal").exists());

    let _ = std::fs::remove_dir_all(&target);
}
