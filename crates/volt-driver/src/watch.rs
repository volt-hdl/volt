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

use std::ops::ControlFlow;
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
    let mut live = Live {
        scope: WatchScope::new(&cwd, filter, opts.target_dir),
        filter,
        opts,
    };
    // Döngüden yalnız Ctrl-C çıkar (`interrupt`, çıkış kodu 130).
    watch_loop(&mut live);
    ExitCode::SUCCESS
}

/// İzleme döngüsünün dış dünyası: anlık görüntü, bekleme ve test koşusu.
/// Gerçeği `Live`'dır; birim testleri sanal saatle sürer.
trait WatchEnv {
    /// İzlenen kümenin anlık görüntüsü.
    fn snapshot(&mut self) -> Vec<Stamp>;
    fn sleep(&mut self, d: Duration);
    /// `n`'inci koşu; `files` izlenen dosya sayısı (özette). `Break`
    /// döngüyü bitirir — yalnız testlerde; gerçek döngüden Ctrl-C çıkar.
    fn run(&mut self, n: u64, files: usize) -> ControlFlow<()>;
}

/// Koşu, değişiklik bekleme, koşu… Bir koşunun tabanı koşudan ÖNCE
/// alınır: koşu sürerken gelen kayıt tabandan ayrılır ve bir sonraki
/// koşuyu tetikler. (Taban koşudan sonra alınınca o kayıt tabana karışıyor,
/// izleyici eski içeriğin sonucunu gösterip bekliyordu — #176.)
fn watch_loop(env: &mut impl WatchEnv) {
    let mut base = env.snapshot();
    for n in 1u64.. {
        if env.run(n, base.len()).is_break() {
            return;
        }
        base = wait_for_change(env, &base);
    }
}

/// Gerçek ortam: dosya sistemi, duvar saati, `sim::test`.
struct Live<'a> {
    scope: WatchScope,
    filter: Option<&'a str>,
    opts: TestOptions<'a>,
}

impl WatchEnv for Live<'_> {
    fn snapshot(&mut self) -> Vec<Stamp> {
        self.scope.snapshot()
    }

    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }

    fn run(&mut self, n: u64, files: usize) -> ControlFlow<()> {
        print_rule(n);
        let started = Instant::now();
        let code = sim::test(self.filter, self.opts);
        // İstemcisi ölmüş bir konteyner koşudan sonra kalmasın.
        crate::interrupt::sweep();
        print_summary(code, started.elapsed(), files);
        ControlFlow::Continue(())
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

/// Küme `base`'den ayrılana, sonra `SETTLE` boyunca sabit kalana kadar
/// bekler; sabitlenen kümeyi döner.
fn wait_for_change(env: &mut impl WatchEnv, base: &[Stamp]) -> Vec<Stamp> {
    let mut last = loop {
        env.sleep(POLL);
        let now = env.snapshot();
        if now != base {
            break now;
        }
    };
    loop {
        env.sleep(SETTLE);
        let now = env.snapshot();
        if now == last {
            return now;
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

    /// Sanal saatli ortam: tek dosya, zamanı belli yazımlar. Bekleme saati
    /// ilerletir; koşu, başladığı andaki dosya boyutunu okur ve `run_takes`
    /// sürer. Duvar saati yok: sonuç her makinede aynı (#176).
    struct Script {
        now: Duration,
        /// (an, boyut), zaman sırasıyla; `fs::write` = kısalt (0) + yaz.
        writes: Vec<(Duration, u64)>,
        initial: u64,
        run_takes: Duration,
        /// Her koşunun okuduğu boyut.
        runs: Vec<u64>,
        max_runs: usize,
    }

    /// Bu sanal sürede koşu gelmezse test düşer (asılı kalmaz).
    const DEADLINE: Duration = Duration::from_secs(60);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    impl Script {
        fn new(initial: u64, writes: &[(u64, u64)]) -> Script {
            Script {
                now: Duration::ZERO,
                writes: writes.iter().map(|&(t, len)| (ms(t), len)).collect(),
                initial,
                run_takes: ms(10),
                runs: Vec::new(),
                max_runs: 2,
            }
        }

        fn len_now(&self) -> u64 {
            self.writes
                .iter()
                .rev()
                .find(|(t, _)| *t <= self.now)
                .map_or(self.initial, |&(_, len)| len)
        }

        fn drive(mut self) -> Vec<u64> {
            watch_loop(&mut self);
            self.runs
        }
    }

    impl WatchEnv for Script {
        fn snapshot(&mut self) -> Vec<Stamp> {
            let changed = self.writes.iter().rev().find(|(t, _)| *t <= self.now);
            let mtime = UNIX_EPOCH + changed.map_or(Duration::ZERO, |(t, _)| *t);
            vec![(PathBuf::from("a.volt"), Some(mtime), self.len_now())]
        }

        fn sleep(&mut self, d: Duration) {
            self.now += d;
            assert!(
                self.now < DEADLINE,
                "no run for the last change within {DEADLINE:?} (virtual); runs read {:?}",
                self.runs
            );
        }

        fn run(&mut self, _n: u64, _files: usize) -> ControlFlow<()> {
            self.runs.push(self.len_now());
            self.now += self.run_takes;
            if self.runs.len() == self.max_runs {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    #[test]
    fn a_burst_of_saves_inside_the_settle_window_is_one_run() {
        // Arrange: iki kayıt (her biri kısalt + yaz) 200 ms arayla; bir
        // yoklama aralarına düşer (1210 ms ilkini görür).
        let script = Script::new(10, &[(1100, 0), (1100, 11), (1300, 0), (1300, 12)]);

        // Act
        let runs = script.drive();

        // Assert: ilk koşu + son içerikle tek yeniden koşu.
        assert_eq!(runs, [10, 12]);
    }

    #[test]
    fn a_writer_stalled_longer_than_the_settle_window_is_run_then_rerun() {
        // Arrange (#176'nın düzeneği): yazıcı dosyayı kısaltıp 600 ms
        // durur (yavaş disk; Linux'ta kısaltma süren geri yazımı bekler).
        // Boş dosya 250 ms'den uzun sabit: izleyici bunu kullanıcının boş
        // kaydından ayıramaz.
        let mut script = Script::new(10, &[(1000, 0), (1600, 12)]);
        script.max_runs = 3;

        // Act
        let runs = script.drive();

        // Assert: ara durum koşar, son içerik yeniden koşar — son söz son
        // kaydındır; koşu sayısı yazıcının hızına bağlıdır.
        assert_eq!(runs, [10, 0, 12]);
    }

    #[test]
    fn a_save_during_a_run_triggers_the_next_run() {
        // Arrange: koşu 2 s sürer; kayıt koşunun ortasında gelir — koşu
        // dosyayı eski haliyle okudu.
        let mut script = Script::new(10, &[(500, 0), (500, 11)]);
        script.run_takes = ms(2000);

        // Act
        let runs = script.drive();

        // Assert: kayıt kaybolmaz; yeniden koşu yeni içeriği okur.
        assert_eq!(runs, [10, 11]);
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
