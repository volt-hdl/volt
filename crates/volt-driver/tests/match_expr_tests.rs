//! match ifadesi ve blok içi `let` uçtan uca (ADR-0083):
//! `tests/ui/fail/*_{implicit_flow,match_expr,block_let}_*` fixture'ları
//! `volt check`'te beklenen kodu beklenen satırda verir (ADR-0070 ortak
//! boru hattı); `tests/ui/pass/*_{match_expr,block_let}_*` SV'si Karar 10
//! (kökte `case`, içte üçlü) ve Karar 11 (süreç içi yerel) biçimindedir.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const TAGS: [&str; 3] = ["_implicit_flow_", "_match_expr_", "_block_let_"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .expect("ui dizini okunmalı")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().is_some_and(|e| e == "volt")
                && p.file_name().is_some_and(|n| {
                    let n = n.to_string_lossy();
                    TAGS.iter().any(|t| n.contains(t))
                })
        })
        .collect();
    files.sort();
    files
}

fn check_json(file: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(file)
        .output()
        .expect("volt çalışmalı");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{}: JSON değil ({e}): {}",
            file.display(),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

/// Hata tanıları: (kod, birincil satır).
fn errors(env: &Value) -> Vec<(String, u64)> {
    env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            let line = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0);
            (d["code"].as_str().unwrap_or_default().to_string(), line)
        })
        .collect()
}

/// Satır 1 `//~ KOD`; `//~^ ERROR` bir üst satırı işaretler.
fn expectation(text: &str) -> (String, u64) {
    let code = text
        .lines()
        .next()
        .and_then(|l| l.trim().strip_prefix("//~ "))
        .map(|c| c.trim().to_string())
        .expect("ilk satır '//~ KOD'");
    let line = text
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("'//~^ ERROR' anotasyonu") as u64;
    (code, line)
}

// ═══ ui/fail ═════════════════════════════════════════════════════════

#[test]
fn every_fail_fixture_reports_its_code_on_the_marked_line() {
    let files = fixtures("tests/ui/fail");
    assert_eq!(files.len(), 12, "ADR-0083 ui/fail fixture sayısı");
    let mut bad = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("okunmalı");
        let (code, line) = expectation(&text);
        let errs = errors(&check_json(file));
        if !errs.iter().any(|(c, l)| *c == code && *l == line) {
            bad.push(format!(
                "{}: {code}@{line} bekleniyor, bulunan {errs:?}",
                file.display()
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

// ═══ ui/pass SV biçimi ══════════════════════════════════════════════

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-mx-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `tests/ui/pass/<name>` derlenir; `<module>.sv` satır başı girintisi
/// ve boş satırlar atılmış döner (karşılaştırma biçimden bağımsız).
fn sv_of(name: &str, module: &str) -> String {
    let target = temp_dir(module);
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(&target)
        .arg(root().join("tests/ui/pass").join(name))
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sv = std::fs::read_to_string(target.join("rtl").join(format!("{module}.sv")))
        .unwrap_or_else(|e| panic!("{module}.sv: {e}"));
    let _ = std::fs::remove_dir_all(&target);
    sv.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_contains(sv: &str, parts: &[&str]) {
    for p in parts {
        assert!(sv.contains(p), "SV'de yok: {p}\n---\n{sv}");
    }
}

#[test]
fn whole_right_hand_side_is_a_case_and_an_operand_is_a_ternary() {
    let sv = sv_of("119_match_expr_module.volt", "MatchExprModule");
    assert_contains(
        &sv,
        &[
            // Kök: modül let'i → bildirim + always_comb + case (Karar 10.1).
            "logic [7:0] pick;\nalways_comb begin\ncase (op)\n2'd0: pick = a;\n2'd1, 2'd2: pick = b;\ndefault: pick = a ^ b;\nendcase\nend",
            // Enum, `_`'sız: son kol default (Karar 4).
            "default: r2 = a ^ b; // AluOp_Xor (and invalid codes)",
            // İç: üçlü zincir, iç içe match parantezli.
            "assign r3 = ((op == 2'd0) ? ((eop == AluOp_Add) ? a : b) : 8'd20) + 8'd1;",
            // Sabit bağlam: derleme zamanında seçilir.
            "assign r4 = 8'd20;",
        ],
    );
}

#[test]
fn match_in_blocks_repeats_the_target_per_arm() {
    let sv = sv_of("120_match_expr_blocks.volt", "MatchExprBlocks");
    assert_contains(
        &sv,
        &[
            "case (mode)\nMode_Idle: acc <= acc;\nMode_Load: acc <= d;\ndefault: acc <= acc << 1; // Mode_Shift (and invalid codes)\nendcase",
            "if ((sel == 2'd0 || sel == 2'd1) ? 1'b1 : 1'b0) begin",
            "case (sel)\n2'd2: y = d ^ 8'hFF;\ndefault: y = 8'd0;\nendcase",
            ".x((sel == 2'd0) ? d : acc),",
        ],
    );
    // for içinde match her iterasyonda açılır: 1 (y) + 4 (w[i]).
    assert_eq!(sv.matches("case (sel)").count(), 5, "{sv}");
}

#[test]
fn match_in_a_function_body_follows_the_call_mode() {
    let sv = sv_of("121_match_expr_fn.volt", "MatchExprFn");
    assert_contains(
        &sv,
        &[
            // Tel kipi: sonuç teli kök konumda → case.
            "case (op)\n3'd0: r = a + b;",
            "default: r = 8'd0;",
            // İkame kipi (comb): üçlü zincir, dönüş genişliğine sabit.
            "z = 8'((op == 3'd0) ? (b + a) : (op == 3'd1) ? (b - a)",
            "3'd5: t = !taken_0_lt;",
        ],
    );
}

#[test]
fn expected_type_is_pushed_into_every_arm_and_structs_split_per_field() {
    let sv = sv_of("122_match_expr_types.volt", "MatchExprTypes");
    assert_contains(
        &sv,
        &[
            "2'd0: wide = 9'(a) + 9'(b);",
            "default: wide = 9'(b);",
            "2'd0: p_v = a;\ndefault: p_v = b;",
            "2'd0: p_ok = 1'b1;\ndefault: p_ok = 1'b0;",
        ],
    );
}

#[test]
fn contract_match_lowers_to_a_ternary_in_sva() {
    let target = temp_dir("sva");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--emit", "sva", "--target-dir"])
        .arg(&target)
        .arg(root().join("tests/ui/pass/123_match_expr_contract.volt"))
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let sva = std::fs::read_dir(target.join("formal"))
        .expect("formal dizini")
        .filter_map(|e| e.ok())
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect::<String>();
    let _ = std::fs::remove_dir_all(&target);
    assert!(
        sva.contains("((op == 2'd0) ? a : (op == 2'd1) ? b : 8'd0) == ((op == 2'd0) ? a : ((op == 2'd1) ? b : 8'd0))"),
        "{sva}"
    );
    // Bağlanan SVA modülü match içindeki adları port olarak alır.
    for port in ["op,", "a,", "b\n"] {
        assert!(sva.contains(port), "{port}\n{sva}");
    }
}

#[test]
fn block_let_is_a_process_local_in_a_named_block() {
    let sv = sv_of("124_block_let_on_comb.volt", "BlockLetOnComb");
    assert_contains(
        &sv,
        &[
            // Karar 11: adlı blok, süreç başında ayrı bildirim, ilk değer yok.
            "always_ff @(posedge clk) begin : on_0\nlogic [7:0] s;\nlogic [7:0] t;\nlogic [7:0] prior;\nlogic [7:0] k;\nif (rst) begin",
            // Bildirim noktasında blocking atama; gövde sırası korunur.
            "s = a + b;\nif (en) begin\nt = s ^ a;\nr <= t;\nend\nprior = r;\nold <= prior;",
            // comb: dal/for içi yerel mandal olmasın diye süreç başında sıfır.
            "always_comb begin : comb_0\nlogic [7:0] acc;\nlogic [8:0] typed;\nacc = 8'd0;\ntyped = 9'd0;",
            "acc = y + a;\ny = acc;\nacc = y + a;",
            "typed = 9'(a) + 9'(b);",
        ],
    );
    assert!(
        !sv.contains("logic [7:0] s ="),
        "bildirimde ilk değer olmamalı: {sv}"
    );
}

#[test]
fn shadowing_block_let_is_renamed_in_sv() {
    let sv = sv_of("125_block_let_shadowing.volt", "BlockLetShadowing");
    assert_contains(
        &sv,
        &[
            "logic [7:0] t;\nlogic [7:0] t_2;\nlogic [7:0] a_2;",
            "t = a;\nif (c) begin\nt_2 = b;\na_2 = t_2 + 8'd1;\nr <= a_2;\nend else begin\nr <= t;",
        ],
    );
}

#[test]
fn struct_block_let_splits_into_one_local_per_leaf() {
    let sv = sv_of("125_block_let_shadowing.volt", "BlockLetStruct");
    assert_contains(
        &sv,
        &[
            "logic [7:0] p_x;
logic p_f;",
            "p_x = a;
p_f = c;
if (p_f) begin
r <= p_x;",
        ],
    );
}

#[test]
fn comb_block_let_keeps_the_value_at_its_declaration() {
    // §5.5: modül teline taşınsaydı z son y'yi okurdu.
    let sv = sv_of("126_block_let_comb_order.volt", "BlockLetCombOrder");
    assert_contains(
        &sv,
        &["y = 8'd0;\nt = y;\nif (c) begin\ny = a;\nend\nz = t;"],
    );
    assert!(!sv.contains("assign"), "{sv}");
}
