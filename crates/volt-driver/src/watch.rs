//! `volt test --watch` (ADR-0095 §4).
//!
//! Proje dosyaları değişince testler yeniden koşar. İzlenen küme: proje
//! kökünden (Volt.toml; yoksa çalışma dizini, özyinelemesiz) gizli
//! olmayan bütün dosyalar — `.volt`, test veri dosyaları (`read_hex`,
//! `load`) ve Volt.toml dahil. Test keşfinin atlama kuralları geçerlidir
//! (gizli dizinler, `build`, `target`, iç içe proje, .gitignore dizinleri);
//! `--target-dir` proje içindeyse o da izlenmez (çıktı kendi kendini
//! tetiklemesin).
//!
//! Değişiklik yoklamayla bulunur (yeni bağımlılık yok, her platformda
//! aynı): dosya listesi + değişiklik zamanı + boyut. İlk değişiklikten
//! sonra küme `SETTLE` boyunca sabit kalana kadar beklenir — art arda
//! kayıtlar (editörün geçici dosyası, birden çok dosyayı kaydetme) tek
//! koşu olur.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use volt_diagnostics::lstr;
use volt_hir::unit_load::Manifest;

use crate::sim::{self, TestOptions};

/// Yoklama aralığı.
const POLL: Duration = Duration::from_millis(300);
/// Değişiklikten sonra kümenin sabit kalması gereken süre.
const SETTLE: Duration = Duration::from_millis(250);
/// Ayırıcı satırın genişliği.
const RULE_WIDTH: usize = 60;
const SECONDS_PER_DAY: u64 = 86_400;

/// Bir dosyanın izlenen durumu.
type Stamp = (PathBuf, Option<SystemTime>, u64);

pub(crate) fn watch(filter: Option<&str>, opts: TestOptions<'_>) -> ExitCode {
    if let Err(e) = crate::interrupt::install(print_stopped) {
        eprintln!(
            "{}",
            lstr!(
                en: "error: cannot catch Ctrl-C for --watch: {e}";
                tr: "hata: --watch için Ctrl-C yakalanamıyor: {e}"
            )
        );
        return ExitCode::from(3);
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let scope = WatchScope::new(&cwd, filter, opts.target_dir);
    // Döngüden yalnız Ctrl-C çıkar (`interrupt`, çıkış kodu 130).
    let mut run: u64 = 0;
    loop {
        run += 1;
        print_rule(run);
        let started = Instant::now();
        let code = sim::test(filter, opts);
        // İstemcisi ölmüş bir konteyner koşudan sonra kalmasın.
        crate::interrupt::sweep();
        let before = scope.snapshot();
        print_summary(code, started.elapsed(), before.len());
        wait_for_change(&scope, before);
    }
}

/// İzlenen dosyalar.
struct WatchScope {
    manifest: Option<Manifest>,
    cwd: PathBuf,
    /// Süzgeç bir dosyaysa (proje dışında da olabilir) o da izlenir.
    extra: Option<PathBuf>,
    /// Çıktı dizini (kanonik): altındaki dosyalar izlenmez.
    target: Option<PathBuf>,
}

impl WatchScope {
    fn new(cwd: &Path, filter: Option<&str>, target_dir: &Path) -> WatchScope {
        WatchScope {
            manifest: Manifest::discover(cwd),
            cwd: cwd.to_path_buf(),
            extra: filter.map(|f| cwd.join(f)).filter(|p| p.is_file()),
            target: target_dir.canonicalize().ok(),
        }
    }

    fn files(&self) -> Vec<PathBuf> {
        // Keşif `cwd`'ye göreli döner; durum okuması sürecin çalışma
        // dizininden bağımsız olsun diye mutlak yapılır.
        let mut files: Vec<PathBuf> = match &self.manifest {
            Some(m) => sim::project_files(m, &self.cwd, &[PathBuf::new()], &is_watched),
            None => flat_files(&self.cwd),
        }
        .into_iter()
        .map(|f| self.cwd.join(f))
        .collect();
        if let Some(target) = &self.target {
            files.retain(|f| !f.canonicalize().is_ok_and(|c| c.starts_with(target)));
        }
        if let Some(extra) = &self.extra {
            if !files.contains(extra) {
                files.push(extra.clone());
            }
        }
        files
    }

    fn snapshot(&self) -> Vec<Stamp> {
        self.files()
            .into_iter()
            .map(|f| {
                let meta = std::fs::metadata(&f).ok();
                let modified = meta.as_ref().and_then(|m| m.modified().ok());
                let len = meta.map_or(0, |m| m.len());
                (f, modified, len)
            })
            .collect()
    }
}

/// Gizli dosyalar (editör takas dosyaları: `.x.swp`) ve editör yedekleri
/// (`x~`) izlenmez.
fn is_watched(p: &Path) -> bool {
    p.file_name().is_some_and(|n| {
        let n = n.to_string_lossy();
        !n.starts_with('.') && !n.ends_with('~')
    })
}

/// Projesiz: çalışma dizinindeki dosyalar (test keşfiyle aynı kapsam).
fn flat_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_watched(p))
        .collect();
    files.sort();
    files
}

/// Küme değişene, sonra `SETTLE` boyunca sabit kalana kadar bekler.
fn wait_for_change(scope: &WatchScope, before: Vec<Stamp>) {
    let mut last = before;
    loop {
        std::thread::sleep(POLL);
        let now = scope.snapshot();
        if now != last {
            last = now;
            break;
        }
    }
    loop {
        std::thread::sleep(SETTLE);
        let now = scope.snapshot();
        if now == last {
            return;
        }
        last = now;
    }
}

/// Koşu ayırıcısı: sıra ve saat (UTC — yerel saat dilimi standart
/// kitaplıkta yok; bağımlılık almamak için).
fn print_rule(run: u64) {
    let clock = utc_clock(SystemTime::now());
    let label = lstr!(
        en: " volt test --watch · run {run} · {clock} UTC ";
        tr: " volt test --watch · koşu {run} · {clock} UTC "
    );
    let fill = RULE_WIDTH.saturating_sub(label.chars().count() + 2);
    eprintln!("\n──{label}{}", "─".repeat(fill));
}

/// `SS:DD:ss` (UTC).
fn utc_clock(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) % SECONDS_PER_DAY;
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// Koşu sonu özeti: sonuç, süre, izlenen dosya sayısı.
fn print_summary(code: ExitCode, took: Duration, files: usize) {
    let secs = took.as_secs_f64();
    let result = if code == ExitCode::SUCCESS {
        lstr!(en: "tests passed"; tr: "testler geçti")
    } else if code == ExitCode::from(5) {
        lstr!(en: "tests FAILED"; tr: "testler DÜŞTÜ")
    } else if code == ExitCode::from(1) {
        lstr!(en: "compile errors"; tr: "derleme hataları")
    } else {
        lstr!(en: "could not run the tests"; tr: "testler koşturulamadı")
    };
    eprintln!(
        "{}",
        lstr!(
            en: "     Watched {result} in {secs:.1}s; waiting for changes in {files} file(s) (Ctrl-C to stop)";
            tr: "     İzleme {result}, {secs:.1}s; {files} dosyada değişiklik bekleniyor (durdurmak için Ctrl-C)"
        )
    );
}

fn print_stopped() {
    eprintln!(
        "{}",
        lstr!(en: "\n     Stopped watching"; tr: "\n     İzleme durduruldu")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_clock_formats_time_of_day() {
        let t = UNIX_EPOCH + Duration::from_secs(3 * SECONDS_PER_DAY + 13 * 3600 + 5 * 60 + 9);
        assert_eq!(utc_clock(t), "13:05:09");
    }

    #[test]
    fn hidden_and_backup_files_are_not_watched() {
        assert!(is_watched(Path::new("a/counter.volt")));
        assert!(is_watched(Path::new("data/image.hex")));
        assert!(!is_watched(Path::new("a/.counter.volt.swp")));
        assert!(!is_watched(Path::new("a/counter.volt~")));
    }

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("volt-watch-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("build/sim")).expect("dizin");
        std::fs::create_dir_all(dir.join("data")).expect("dizin");
        std::fs::write(
            dir.join("Volt.toml"),
            "[package]\nname = \"w\"\nsrc = \".\"\n",
        )
        .expect("yaz");
        std::fs::write(dir.join("a.volt"), "module A {}\n").expect("yaz");
        std::fs::write(dir.join("data/img.hex"), "00\n").expect("yaz");
        std::fs::write(dir.join("build/sim/x.vcd"), "x").expect("yaz");
        dir
    }

    fn names(scope: &WatchScope) -> Vec<String> {
        scope
            .files()
            .iter()
            .map(|f| f.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn project_scope_covers_sources_data_and_manifest_but_not_build() {
        // Arrange
        let dir = temp_project("scope");

        // Act
        let scope = WatchScope::new(&dir, None, &dir.join("build"));
        let got = names(&scope);

        // Assert
        assert!(got.iter().any(|f| f.ends_with("a.volt")), "{got:?}");
        assert!(got.iter().any(|f| f.ends_with("data/img.hex")), "{got:?}");
        assert!(got.iter().any(|f| f.ends_with("Volt.toml")), "{got:?}");
        assert!(!got.iter().any(|f| f.contains("build/")), "{got:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_target_dir_inside_the_project_is_not_watched() {
        let dir = temp_project("target");
        std::fs::create_dir_all(dir.join("out")).expect("dizin");
        std::fs::write(dir.join("out/tb.cpp"), "x").expect("yaz");
        let scope = WatchScope::new(&dir, None, &dir.join("out"));
        assert!(!names(&scope).iter().any(|f| f.contains("out/")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snapshot_changes_when_a_file_is_edited_added_or_removed() {
        // Arrange
        let dir = temp_project("snap");
        let scope = WatchScope::new(&dir, None, &dir.join("build"));
        let first = scope.snapshot();

        // Act + Assert: boyut değişen düzenleme
        std::fs::write(dir.join("a.volt"), "module A { }\n").expect("yaz");
        let edited = scope.snapshot();
        assert_ne!(first, edited);
        // ekleme
        std::fs::write(dir.join("b.volt"), "x").expect("yaz");
        let added = scope.snapshot();
        assert_ne!(edited, added);
        // silme
        std::fs::remove_file(dir.join("b.volt")).expect("sil");
        assert_ne!(added, scope.snapshot());
        // build/ altındaki çıktı tetiklemez
        let stable = scope.snapshot();
        std::fs::write(dir.join("build/sim/y.vcd"), "yy").expect("yaz");
        assert_eq!(stable, scope.snapshot());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
