//! `volt test` / `volt run` ürettiği dosyaların golden karşılaştırması.
//!
//! Sürücü, testbench'i Verilator'u ÇAĞIRMADAN önce diske yazar. Burada
//! `VOLT_VERILATOR` çalıştırılamayan bir dosyaya çevrilir: sürücü
//! dosyaları yazar, araç başlatılamaz (çıkış kodu 3), üretilen metin
//! `tests/golden/` altındaki kayıtla BAYT BAYT karşılaştırılır. Gerçek
//! Verilator gerekmez; her ortamda koşar.
//!
//! Çıktı bilerek değiştiyse: `VOLT_BLESS=1 cargo test -p volt-driver
//! --test sim_golden_tests` kayıtları yeniden yazar.

use std::path::{Path, PathBuf};
use std::process::Command;

const DESIGN: &str = "\
module Table {
    in  clk  : clock
    in  rst  : bool
    in  addr : u3
    in  sv   : i8
    in  we   : bool
    out data  : u8
    out secho : i8
    out wrote : bool
    out empty : bool

    reg mem : [u8; 8] = [0; 8]
    reg hits : u8 = 0

    on clk {
        if rst {
            hits <= 0
        } else if we {
            hits <= hits + 1
        }
    }

    data = mem[addr]
    secho = sv
    wrote = hits > 0
    empty = hits == 0
}
";

/// Her deyim türünden en az bir tane: port yazımı (sabit + denetimli),
/// step, assert ailesi, dizi, `for`, `len`, `load`.
const TESTS: &str = "\
test \"reads back loaded table\" {
    let dut = Table { };
    let image = [3, 1, 4, 1, 5, 9, 2, 6];
    load(dut.mem, image);
    dut.we = false;
    for i in 0..len(image) {
        dut.addr = i;
        step(1);
        assert_eq(dut.data, image[i]);
    }
}

test \"signed echo and flags\" {
    let dut = Table { };
    dut.sv = 0 - 2;
    dut.we = true;
    step(2);
    assert_ne(dut.secho, 0);
    assert_true(dut.wrote);
    assert_false(dut.empty);
}
";

fn golden(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden")).join(name)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-golden-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// Sürücüyü, başlatılamayan bir "Verilator" ile koşturur (çıkış kodu 3).
fn volt_without_tool(dir: &Path, fake_tool: &Path, args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_volt"))
        .env("VOLT_LANG", "en")
        .env("VOLT_VERILATOR", fake_tool)
        .args(args)
        .arg("--target-dir")
        .arg(dir.join("build"))
        .current_dir(dir)
        .output()
        .expect("volt");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(3), "stderr: {stderr}");
    assert!(stderr.contains("error: cannot run"), "stderr: {stderr}");
}

fn assert_matches_golden(generated: &Path, name: &str) {
    let actual = std::fs::read(generated).expect("üretilen dosya");
    if std::env::var_os("VOLT_BLESS").is_some() {
        std::fs::create_dir_all(golden("")).expect("golden dizini");
        std::fs::write(golden(name), &actual).expect("golden yaz");
        return;
    }
    let expected = std::fs::read(golden(name)).expect("golden kaydı");
    assert!(
        actual == expected,
        "{name}: üretilen dosya golden kaydından farklı\n\
         (bilerek değiştiyse VOLT_BLESS=1 ile yeniden yaz)\n--- üretilen ---\n{}",
        String::from_utf8_lossy(&actual)
    );
}

#[test]
fn test_testbench_and_load_config_match_golden() {
    // Arrange
    let dir = temp_dir("test");
    let file = dir.join("table_test.volt");
    std::fs::write(&file, format!("{DESIGN}\n{TESTS}")).expect("yaz");

    // Act
    volt_without_tool(&dir, &file, &["test", "table_test.volt"]);

    // Assert
    let sim = dir.join("build/sim/table_test");
    assert_matches_golden(&sim.join("tb_Table.cpp"), "tb_Table.cpp");
    assert_matches_golden(&sim.join("load_Table.vlt"), "load_Table.vlt");
    assert_matches_golden(&sim.join("Table.sv"), "Table.sv");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn run_testbench_matches_golden() {
    // Arrange
    let dir = temp_dir("run");
    let file = dir.join("table.volt");
    std::fs::write(&file, DESIGN).expect("yaz");

    // Act
    volt_without_tool(&dir, &file, &["run", "table.volt", "--cycles", "12"]);

    // Assert
    let sim = dir.join("build/sim/table");
    assert_matches_golden(&sim.join("tb.cpp"), "run_tb.cpp");
    let _ = std::fs::remove_dir_all(&dir);
}
