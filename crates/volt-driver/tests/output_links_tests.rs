//! Çıktı yazımı ve bağlantılar (#221): Volt'un yazdığı her dosya çıktı
//! kökünün altında kalır; kökün altındaki yolda ya da hedefte bir
//! sembolik bağlantı (Windows'ta junction ya da başka bir reparse point)
//! varsa yazım reddedilir, çıkış kodu 3 (G/Ç hatası).
//!
//! Kök: varsayılan `build/` için çalışma dizini (proje kökü),
//! `--target-dir` ile verilen dizin, `volt new`'in proje dizini, `volt init`
//! için çalışma dizini. Kökün kendisi ve üstündeki yol kullanıcının
//! seçimidir: oradaki bağlantı (ör. bağlantı olan bir ana dizin) meşrudur.
//!
//! Sembolik bağlantı Linux'ta her zaman kurulur. Windows'ta yetki ister
//! (Geliştirici Modu ya da yönetici); kurulamazsa test bunu söyleyerek
//! atlanır (ADR-0079'un kuralı: atlama sessiz değildir). Junction yetki
//! istemez; Windows'a özgü test onu kullanır.
//!
//! `run`/`test`/`verify` için araç gerekmez: geçersiz `VOLT_VERILATOR` /
//! `VOLT_SBY` ile dosyalar araç başlatılmadan önce yazılır (proje kipi
//! testlerinin deseni).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-links-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("üst dizin")).expect("dizin");
    std::fs::write(path, text).expect("yaz");
}

/// `link` → `target` dosya bağlantısı; kurulamazsa atlama notu ve `false`.
fn link_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(target, link);
    skip_unless_made(made, link)
}

/// `link` → `target` dizin bağlantısı; kurulamazsa atlama notu ve `false`.
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(target, link);
    skip_unless_made(made, link)
}

fn skip_unless_made(made: std::io::Result<()>, link: &Path) -> bool {
    match made {
        Ok(()) => true,
        // Linux'ta bağlantı her zaman kurulur: kurulamıyorsa test ortamı
        // bozuktur, atlanmaz.
        Err(e) if cfg!(unix) => panic!("bağlantı kurulamadı ({}): {e}", link.display()),
        Err(e) => {
            eprintln!(
                "SKIP: sembolik bağlantı kurulamadı ({e}); Windows'ta Geliştirici Modu \
                 ya da yönetici yetkisi gerekir"
            );
            false
        }
    }
}

const TOP: &str = "module Top {\n    in  clk : clock\n    in  d   : u4\n    out q   : u4\n    \
                   reg r : u4 = 0\n    on clk { r <= d }\n    q = r\n}\n";

const TOP_TEST: &str = "test \"holds\" {\n    let dut = Top { };\n    dut.d = 5;\n    \
                        step(1);\n    assert_eq(dut.q, 5);\n}\n";

const CONTRACT: &str =
    "module Top {\n    in  clk : clock\n    in  d   : u4\n    out q   : u4\n\n    \
                        requires: d <= 9\n    ensures:  q <= 9\n\n    \
                        reg r : u4 = 0\n    on clk { r <= d }\n    q = r\n}\n";

const VICTIM: &str = "ssh-ed25519 AAAA victim-key\n";

fn volt(dir: &Path, args: &[&str]) -> Output {
    // Var olan ama çalıştırılamayan "araçlar" (proje kipi testlerinin deseni).
    for fake in ["no-verilator", "no-sby"] {
        let _ = std::fs::write(dir.join(fake), "not a program");
    }
    Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(args)
        .current_dir(dir)
        .env("VOLT_LANG", "en")
        .env_remove("VOLT_MANIFEST_DIR")
        .env("VOLT_VERILATOR", dir.join("no-verilator"))
        .env("VOLT_SBY", dir.join("no-sby"))
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı")
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Ret iletisi ADR-0125'in üç parçasını taşır: hangi yol (ne oldu), neden
/// (bağlantı ya da kök dışı), ne yapılabilir.
fn assert_refused(out: &Output, culprit: &str) {
    let err = stderr(out);
    assert_eq!(out.status.code(), Some(3), "çıkış kodu 3 olmalı:\n{err}");
    let culprit = culprit.replace('/', std::path::MAIN_SEPARATOR_STR);
    for part in [
        format!("'{culprit}' is a symbolic link"),
        "= reason:".to_string(),
        format!("= help: remove the link '{culprit}'"),
    ] {
        assert!(err.contains(&part), "`{part}` yok:\n{err}");
    }
}

/// #221'in örneği: klonlanan depoda `build/rtl/Top.sv`, depo dışındaki bir
/// dosyaya bağlantı. Yazım reddedilir, kurban dosyası değişmez.
#[test]
fn build_refuses_an_output_file_that_is_a_link_and_leaves_the_target_unchanged() {
    // Arrange
    let base = temp_dir("file");
    let repo = base.join("repo");
    write(&base.join("home").join("authorized_keys"), VICTIM);
    write(&repo.join("m.volt"), TOP);
    std::fs::create_dir_all(repo.join("build").join("rtl")).expect("dizin");
    let link = repo.join("build").join("rtl").join("Top.sv");
    // Göreli hedef, #221'deki gibi (`../../../home/authorized_keys`);
    // Windows `/` ayraçlı göreli hedefi çözemez, ayraç platformunki.
    let target: PathBuf = ["..", "..", "..", "home", "authorized_keys"]
        .iter()
        .collect();
    if !link_file(&target, &link) {
        return;
    }

    // Act
    let out = volt(&repo, &["build", "m.volt"]);

    // Assert
    assert_refused(&out, "build/rtl/Top.sv");
    let victim = std::fs::read_to_string(base.join("home").join("authorized_keys")).expect("oku");
    assert_eq!(victim, VICTIM, "bağlantının hedefi değişmemeli");
    let _ = std::fs::remove_dir_all(&base);
}

/// `build` dizininin kendisi bir bağlantı: bütün çıktılar onu izlerdi.
#[test]
fn build_refuses_a_build_directory_that_is_a_link() {
    // Arrange
    let base = temp_dir("dir");
    let repo = base.join("repo");
    let elsewhere = base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("dizin");
    write(&repo.join("m.volt"), TOP);
    if !link_dir(&elsewhere, &repo.join("build")) {
        return;
    }

    // Act
    let out = volt(&repo, &["build", "m.volt"]);

    // Assert
    assert_refused(&out, "build");
    let written: Vec<_> = std::fs::read_dir(&elsewhere).expect("oku").collect();
    assert!(
        written.is_empty(),
        "bağlantının hedefine yazılmamalı: {written:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// `--target-dir` ile verilen dizin kullanıcının seçimidir: kendisi bir
/// bağlantı olabilir; altındaki bağlantı yine reddedilir.
#[test]
fn an_explicit_target_dir_may_be_a_link_but_links_below_it_are_refused() {
    // Arrange
    let base = temp_dir("explicit");
    let repo = base.join("repo");
    let real = base.join("real-out");
    std::fs::create_dir_all(&real).expect("dizin");
    write(&repo.join("m.volt"), TOP);
    if !link_dir(&real, &repo.join("out")) {
        return;
    }

    // Act: kök bir bağlantı — yazım onun hedefine gider.
    let ok = volt(&repo, &["build", "--target-dir", "out", "m.volt"]);

    // Assert
    assert_eq!(ok.status.code(), Some(0), "{}", stderr(&ok));
    assert!(real.join("rtl").join("Top.sv").is_file());

    // Arrange: kökün altında bir bağlantı.
    std::fs::remove_file(real.join("rtl").join("Top.sv")).expect("sil");
    write(&base.join("victim.txt"), VICTIM);
    if !link_file(&base.join("victim.txt"), &real.join("rtl").join("Top.sv")) {
        return;
    }

    // Act
    let refused = volt(&repo, &["build", "--target-dir", "out", "m.volt"]);

    // Assert
    assert_refused(&refused, "out/rtl/Top.sv");
    let victim = std::fs::read_to_string(base.join("victim.txt")).expect("oku");
    assert_eq!(victim, VICTIM);
    let _ = std::fs::remove_dir_all(&base);
}

/// `volt init` çalışma dizinine yazar: orada bekleyen (hedefi olmayan)
/// bir bağlantı dosyayı başka bir yerde oluştururdu.
#[test]
fn init_refuses_a_dangling_link_in_the_current_directory() {
    // Arrange
    let base = temp_dir("init");
    let repo = base.join("repo");
    std::fs::create_dir_all(&repo).expect("dizin");
    if !link_file(Path::new("../planted.md"), &repo.join("README.md")) {
        return;
    }

    // Act
    let out = volt(&repo, &["init", "--name", "p"]);

    // Assert
    // `volt init`'in dizini `.`: yol eski G/Ç iletisindeki biçimde basılır.
    assert_refused(&out, "./README.md");
    assert!(
        !base.join("planted.md").exists(),
        "bağlantının hedefi oluşturulmamalı"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// `volt test`: sim dizini bir bağlantı.
#[test]
fn test_refuses_a_sim_directory_that_is_a_link() {
    // Arrange
    let base = temp_dir("test");
    let repo = base.join("repo");
    let elsewhere = base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("dizin");
    write(&repo.join("top.volt"), TOP);
    write(&repo.join("top_test.volt"), TOP_TEST);
    std::fs::create_dir_all(repo.join("build")).expect("dizin");
    if !link_dir(&elsewhere, &repo.join("build").join("sim")) {
        return;
    }

    // Act
    let out = volt(&repo, &["test", "top_test.volt"]);

    // Assert
    assert_refused(&out, "build/sim");
    let written: Vec<_> = std::fs::read_dir(&elsewhere).expect("oku").collect();
    assert!(
        written.is_empty(),
        "bağlantının hedefine yazılmamalı: {written:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// `volt test`: depoyla gelen eski bir dalga formu bağlantısı
/// (`waves/<Modül>-<test>.vcd`) ve Verilator obj dizini bağlantısı silinir:
/// testbench ve Verilator onların üzerinden yazardı. Silinen bağlantının
/// kendisidir; hedefleri değişmez.
#[test]
fn test_removes_planted_waveform_and_obj_dir_links() {
    // Arrange
    let base = temp_dir("stale");
    let repo = base.join("repo");
    let elsewhere = base.join("elsewhere");
    write(&elsewhere.join("keep.txt"), VICTIM);
    write(&base.join("victim.txt"), VICTIM);
    write(&repo.join("top.volt"), TOP);
    write(&repo.join("top_test.volt"), TOP_TEST);
    let sim = repo.join("build").join("sim").join("top_test");
    std::fs::create_dir_all(sim.join("waves")).expect("dizin");
    let vcd = sim.join("waves").join("Top-holds.vcd");
    if !link_file(&base.join("victim.txt"), &vcd) || !link_dir(&elsewhere, &sim.join("obj_top")) {
        return;
    }

    // Act: araç sahte — dosyalar yazılır, Verilator başlatılamaz (çıkış 3).
    let out = volt(&repo, &["test", "top_test.volt"]);

    // Assert
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));
    assert!(
        std::fs::symlink_metadata(&vcd).is_err(),
        "dalga formu bağlantısı silinmeli"
    );
    assert!(
        std::fs::symlink_metadata(sim.join("obj_top")).is_err(),
        "obj dizini bağlantısı silinmeli"
    );
    assert_eq!(
        std::fs::read_to_string(base.join("victim.txt")).expect("oku"),
        VICTIM
    );
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("keep.txt")).expect("hedef dizin kalmalı"),
        VICTIM
    );
    assert!(sim.join("Top.sv").is_file(), "sim dosyaları yazılmalı");
    let _ = std::fs::remove_dir_all(&base);
}

/// `volt verify`: üretilen `.sby` dosyası bir bağlantı.
#[test]
fn verify_refuses_an_sby_file_that_is_a_link() {
    // Arrange
    let base = temp_dir("verify");
    let repo = base.join("repo");
    write(&base.join("victim.txt"), VICTIM);
    write(&repo.join("top.volt"), CONTRACT);
    std::fs::create_dir_all(repo.join("build").join("formal")).expect("dizin");
    if !link_file(
        &base.join("victim.txt"),
        &repo.join("build").join("formal").join("top.sby"),
    ) {
        return;
    }

    // Act
    let out = volt(&repo, &["verify", "top.volt"]);

    // Assert
    assert_refused(&out, "build/formal/top.sby");
    let victim = std::fs::read_to_string(base.join("victim.txt")).expect("oku");
    assert_eq!(victim, VICTIM);
    let _ = std::fs::remove_dir_all(&base);
}

/// `volt run --vcd`: dalga formu dosyası bir bağlantı. VCD'yi simülasyon
/// yazar; Volt hedefi simülasyondan önce denetler.
#[test]
fn run_refuses_a_vcd_path_that_is_a_link() {
    // Arrange
    let base = temp_dir("vcd");
    let repo = base.join("repo");
    write(&base.join("victim.txt"), VICTIM);
    write(&repo.join("top.volt"), TOP);
    if !link_file(&base.join("victim.txt"), &repo.join("waves.vcd")) {
        return;
    }

    // Act
    let out = volt(&repo, &["run", "--vcd", "waves.vcd", "top.volt"]);

    // Assert
    assert_refused(&out, "waves.vcd");
    let err = stderr(&out);
    assert!(err.contains("another --vcd"), "--vcd önerisi yok:\n{err}");
    let _ = std::fs::remove_dir_all(&base);
}

/// Windows: junction yetki istemeden kurulur ve bir dizin bağlantısı gibi
/// davranır; `build` bir junction ise yazım reddedilir.
#[cfg(windows)]
#[test]
fn build_refuses_a_build_directory_that_is_a_junction() {
    // Arrange
    let base = temp_dir("junction");
    let repo = base.join("repo");
    let elsewhere = base.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("dizin");
    write(&repo.join("m.volt"), TOP);
    let made = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(repo.join("build"))
        .arg(&elsewhere)
        .output()
        .expect("cmd çalışmalı");
    assert!(made.status.success(), "junction kurulamadı: {made:?}");

    // Act
    let out = volt(&repo, &["build", "m.volt"]);

    // Assert
    assert_refused(&out, "build");
    let written: Vec<_> = std::fs::read_dir(&elsewhere).expect("oku").collect();
    assert!(
        written.is_empty(),
        "junction'ın hedefine yazılmamalı: {written:?}"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// Bağlantısız yollar eskisi gibi yazılır: `build`, `test` ve `verify`
/// çıktıları varsayılan kökte ve `--target-dir` kökünde bayt bayt aynıdır;
/// `volt new` şablon dosyalarını depodaki şablonla bayt bayt aynı yazar.
#[test]
fn outputs_without_links_are_written_as_before() {
    // Arrange
    let base = temp_dir("plain");
    write(&base.join("top.volt"), CONTRACT);
    write(&base.join("top_test.volt"), TOP_TEST);

    // Act
    let mut runs = Vec::new();
    for target in [None, Some("other")] {
        let extra: Vec<&str> = target.map(|t| vec!["--target-dir", t]).unwrap_or_default();
        let build = volt(&base, &[&["build"][..], &extra, &["top.volt"]].concat());
        assert_eq!(build.status.code(), Some(0), "{}", stderr(&build));
        // Araç sahte: dosyalar yazılır, araç başlatılamaz (çıkış 3).
        let test = volt(&base, &[&["test"][..], &extra, &["top_test.volt"]].concat());
        assert_eq!(test.status.code(), Some(3), "{}", stderr(&test));
        let verify = volt(&base, &[&["verify"][..], &extra, &["top.volt"]].concat());
        assert_eq!(verify.status.code(), Some(3), "{}", stderr(&verify));
        runs.push(base.join(target.unwrap_or("build")));
    }

    // Assert
    let files = [
        "rtl/Top.sv",
        "sim/top_test/Top.sv",
        "sim/top_test/tb_Top.cpp",
        "formal/top.sv",
        "formal/top.sby",
    ];
    for f in files {
        let a = std::fs::read(runs[0].join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        let b = std::fs::read(runs[1].join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        assert!(!a.is_empty(), "{f} boş");
        assert_eq!(a, b, "{f} iki kökte farklı");
    }

    // Act: volt new
    let out = volt(&base, &["new", "proj"]);

    // Assert
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/minimal");
    for f in ["counter.volt", "counter_test.volt", ".gitignore"] {
        let written = std::fs::read(base.join("proj").join(f)).expect("yazılmış");
        let source = std::fs::read(template.join(f)).expect("şablon");
        assert_eq!(written, source, "{f} şablondan farklı");
    }
    let _ = std::fs::remove_dir_all(&base);
}
