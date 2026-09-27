//! Struct / enum öğeli yerleşik primitifler (ADR-0087): `T` bellekte TEK
//! paketlenmiş vektördür (ADR-0077 Karar 3 düzeni, AoS — BRAM korunur);
//! veri girişi paketlenir, veri çıkışı yaprak dilimlerine açılır; veri
//! register'ları `T`'nin varsayılan kodlamasıyla reset'lenir. RAM
//! ailesinde enum yapraklı `T` E2009.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-ps-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Kaynağı derler; `<module>.sv` satır başı boşlukları atılmış döner.
fn build(tag: &str, src: &Path, module: &str) -> String {
    let target = temp_dir(tag);
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "--target-dir"])
        .arg(&target)
        .arg(src)
        .output()
        .expect("volt çalışmalı");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("warning["), "{stderr}");
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
fn struct_elements_are_one_packed_word_per_entry() {
    let sv = build(
        "pass",
        &root().join("tests/ui/pass/128_prim_struct_elements.volt"),
        "PrimStructs",
    );
    assert_contains(
        &sv,
        &[
            // Tek bellek dizisi, eleman başına 12 (Packet) / 8 (Cmd) bit.
            "logic [11:0] fifo_mem [fifo_DEPTH];",
            "logic [11:0] ram_mem [ram_DEPTH];",
            // Paketleme: ilk alan MSB (ADR-0077 Karar 3).
            "fifo_mem[fifo_wptr] <= ({pkt_tag, pkt_data});",
            // Açma: yaprak başına dilim.
            "assign fifo_out_tag = fifo_rd_data[11:8];",
            "assign fifo_out_data = fifo_rd_data[7:0];",
            "assign ram_out_tag = ram_rd_data[11:8];",
            "assign cmd_out_mode = line_data_out[7:6];",
            "assign hs_out_arg = hs_data_out[5:0];",
            "assign xfer_out_data = xfer_rd_data[7:0];",
            // ShiftRegister 3 aşama × 8 bit.
            "logic [23:0] line_shift;",
        ],
    );
    // Alan başına bellek (SoA) yok.
    assert!(
        !sv.contains("fifo_mem_tag") && !sv.contains("mem_data ["),
        "{sv}"
    );
}

#[test]
fn ram_of_an_enum_bearing_struct_is_e2009_on_the_marked_line() {
    let file = root().join("tests/ui/fail/184_ram_enum_field_element.volt");
    let text = std::fs::read_to_string(&file).expect("okunmalı");
    let marked = text
        .lines()
        .position(|l| l.trim_start().starts_with("//~^ ERROR"))
        .expect("işaret") as u64;
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .output()
        .expect("volt çalışmalı");
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let errs: Vec<(String, u64)> = json["diagnostics"]
        .as_array()
        .expect("dizi")
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
        .collect();
    assert_eq!(errs, [("E2009".to_string(), marked)]);
}

#[test]
fn data_registers_reset_to_the_default_encoding_of_t() {
    // İlk kodu 0 olmayan enum: reset değeri geçerli bir varyant olmalı.
    let dir = temp_dir("dflt");
    let src = dir.join("dflt.volt");
    std::fs::write(
        &src,
        "enum Lvl : u4 { Lo = 2, Hi = 8 }\nstruct Cmd {\n    lvl : Lvl\n    arg : u4\n}\n\
         pub module Top {\n    in  clk : clock\n    in  d   : Cmd\n    in  e   : Lvl\n    in  we  : bool\n    \
         in  re  : bool\n    out q   : Cmd\n    out r   : Lvl\n    out s   : Cmd\n    \
         let m = SyncFifo<Cmd, 8> { clk: clk, wr_data: d, wr_en: we, rd_en: re }\n    \
         let n = SyncFifo<Lvl, 8> { clk: clk, wr_data: e, wr_en: we, rd_en: re }\n    \
         let sr = ShiftRegister<Cmd, 3> { clk: clk, data_in: d, shift_en: we }\n    \
         q = m.rd_data\n    r = n.rd_data\n    s = sr.data_out\n}\n",
    )
    .expect("kaynak");
    let sv = build("dflt-out", &src, "Top");
    assert_contains(
        &sv,
        &[
            "m_rd_data <= 8'h20;", // lvl = Lo (2), arg = 0
            "n_rd_data <= 4'h2;",
            "sr_shift <= {3{8'h20}};",
        ],
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn zero_default_keeps_the_old_reset_literal() {
    // Kodu 0 olan ilk varyant (varsayılan): çıktı eskisiyle aynı.
    let sv = build(
        "zero",
        &root().join("tests/ui/pass/128_prim_struct_elements.volt"),
        "PrimStructs",
    );
    assert_contains(&sv, &["fifo_rd_data <= 12'd0;", "line_shift <= 24'd0;"]);
}
