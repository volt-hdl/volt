//! `volt verify -j` — sby görev koşucusu (ADR-0055).
//!
//! Tek `sby -j N -f <iş>.sby` süreci birimdeki tüm modül görevlerini
//! paralel koşturur; bu modül sürecin stdout/stderr akışını satır satır
//! okur, her satırı `[<çalışma dizini>]` etiketinden görevine ayırır ve
//! görevin `DONE (...)` satırı geldiğinde sonucu yorumlar. Sonuç dizisi
//! HER ZAMAN girdi (kaynak) sırasındadır — tamamlanma sırası yalnız
//! ilerleme geri çağrısına yansır (cli-contract.md §8a determinizm).
//!
//! `--fail-fast`: ilk karşı örnekte sby süreci öldürülür, henüz
//! bitmemiş görevler `Skipped` olur. Varsayılan davranış tüm görevlerin
//! tamamlanmasıdır (bir modülün hatası diğerlerini durdurmaz).

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::verify::{interpret_sby_output, SbyOutcome};

/// `-j <N|auto>` değeri (cli-contract.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Jobs {
    /// CPU sayısı (varsayılan).
    Auto,
    /// Sabit iş sayısı, en az 1.
    Count(usize),
}

impl Jobs {
    /// Fiili iş sayısı; `Auto` mantıksal çekirdek sayısıdır (en az 1).
    pub(crate) fn resolve(self) -> usize {
        match self {
            Jobs::Auto => std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            Jobs::Count(n) => n,
        }
    }
}

/// clap değer ayrıştırıcısı: `auto` ya da pozitif tamsayı; aksi kullanım
/// hatasıdır (çıkış kodu 2).
pub(crate) fn parse_jobs(text: &str) -> Result<Jobs, String> {
    if text.eq_ignore_ascii_case("auto") {
        return Ok(Jobs::Auto);
    }
    match text.parse::<usize>() {
        Ok(n) if n >= 1 => Ok(Jobs::Count(n)),
        _ => Err(format!(
            "expected a positive integer or 'auto', got '{text}'"
        )),
    }
}

/// Koşturulacak bir sby görevi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskSpec {
    /// `[tasks]` içindeki ad (`sby -f iş.sby <ad>` ile tek başına koşar).
    pub(crate) name: String,
    /// sby'nin kurduğu çalışma dizini: `<iş>_<ad>` — log etiketi budur.
    pub(crate) workdir: String,
}

/// Bir görevin sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskStatus {
    /// sby `DONE (...)` bastı; süre görevin ilk sürecinden DONE'a kadar.
    Done {
        outcome: SbyOutcome,
        elapsed: Duration,
    },
    /// `--fail-fast` ile sby sonlandırıldı, görev bitmedi.
    Skipped,
    /// sby görev için DONE satırı basmadan çıktı (araç hatası).
    Missing,
}

/// Görev sonucu + görevin kendi log satırları (hata yardımı için).
#[derive(Debug, Clone)]
pub(crate) struct TaskResult {
    pub(crate) status: TaskStatus,
    pub(crate) log: String,
}

/// Bir koşunun toplu sonucu.
#[derive(Debug)]
pub(crate) struct RunReport {
    /// Girdi sırasında görev sonuçları.
    pub(crate) tasks: Vec<TaskResult>,
    /// Hiçbir göreve ait olmayan sby satırları (yapılandırma hatası vb.).
    pub(crate) global_log: String,
    /// sby çıkış kodu (`--fail-fast` ile öldürüldüyse `None` olabilir).
    pub(crate) exit_code: Option<i32>,
}

/// Koşu ayarları.
pub(crate) struct RunConfig<'a> {
    pub(crate) sby: &'a Path,
    /// `cwd`'ye göreli `.sby` dosya adı.
    pub(crate) sby_file: &'a str,
    pub(crate) cwd: &'a Path,
    pub(crate) jobs: usize,
    pub(crate) fail_fast: bool,
}

/// sby'yi koşturur; her görev bittiğinde `on_done(bitmiş sayısı, görev
/// indeksi, durum)` çağrılır. Hata yalnız süreç başlatılamazsa döner.
pub(crate) fn run_sby_tasks(
    cfg: &RunConfig<'_>,
    tasks: &[TaskSpec],
    mut on_done: impl FnMut(usize, usize, &TaskStatus),
) -> std::io::Result<RunReport> {
    let mut child = Command::new(cfg.sby)
        .arg("-j")
        .arg(cfg.jobs.to_string())
        .arg("-f")
        .arg(cfg.sby_file)
        .current_dir(cfg.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (tx, rx) = mpsc::channel::<String>();
    if let Some(out) = child.stdout.take() {
        spawn_line_reader(out, tx.clone());
    }
    if let Some(err) = child.stderr.take() {
        spawn_line_reader(err, tx.clone());
    }
    drop(tx);

    let started = Instant::now();
    let mut results: Vec<TaskResult> = tasks
        .iter()
        .map(|_| TaskResult {
            status: TaskStatus::Missing,
            log: String::new(),
        })
        .collect();
    let mut starts: Vec<Option<Instant>> = vec![None; tasks.len()];
    let mut global_log = String::new();
    let mut finished = 0usize;
    let mut killed = false;

    for line in rx {
        let Some(idx) = task_index_of(&line, tasks) else {
            global_log.push_str(&line);
            global_log.push('\n');
            continue;
        };
        results[idx].log.push_str(&line);
        results[idx].log.push('\n');
        if starts[idx].is_none() && line.contains("starting process") {
            starts[idx] = Some(Instant::now());
        }
        if !line.contains("] DONE (") || results[idx].status != TaskStatus::Missing {
            continue;
        }
        let elapsed = starts[idx].unwrap_or(started).elapsed();
        let outcome = interpret_sby_output(&results[idx].log);
        let is_fail = matches!(outcome, SbyOutcome::Fail(_));
        results[idx].status = TaskStatus::Done { outcome, elapsed };
        finished += 1;
        on_done(finished, idx, &results[idx].status);
        if cfg.fail_fast && is_fail {
            // Kalan görevler bitirilmez; süreç öldürülür, okuma bırakılır.
            let _ = child.kill();
            killed = true;
            break;
        }
    }

    if killed {
        for r in results.iter_mut() {
            if r.status == TaskStatus::Missing {
                r.status = TaskStatus::Skipped;
            }
        }
    }
    let exit_code = child.wait().ok().and_then(|s| s.code());
    Ok(RunReport {
        tasks: results,
        global_log,
        exit_code,
    })
}

/// Akışı satır satır kanala kopyalayan ayrık iş parçacığı. UTF-8
/// bozukluğu satırı düşürmez (`from_utf8_lossy`).
fn spawn_line_reader<R: Read + Send + 'static>(reader: R, tx: mpsc::Sender<String>) {
    std::thread::spawn(move || {
        let mut buf = BufReader::new(reader);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            match buf.read_until(b'\n', &mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let text = String::from_utf8_lossy(&bytes)
                .trim_end_matches(['\r', '\n'])
                .to_string();
            if tx.send(text).is_err() {
                break;
            }
        }
    });
}

/// `SBY hh:mm:ss [<workdir>] ...` satırının hangi göreve ait olduğu —
/// ilk köşeli parantez çiftindeki ad `workdir` ile birebir eşleşmeli.
pub(crate) fn task_index_of(line: &str, tasks: &[TaskSpec]) -> Option<usize> {
    let open = line.find('[')?;
    let close = line[open + 1..].find(']')? + open + 1;
    let tag = &line[open + 1..close];
    tasks.iter().position(|t| t.workdir == tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_jobs_accepts_auto_and_positive_integers() {
        assert_eq!(parse_jobs("auto"), Ok(Jobs::Auto));
        assert_eq!(parse_jobs("AUTO"), Ok(Jobs::Auto));
        assert_eq!(parse_jobs("1"), Ok(Jobs::Count(1)));
        assert_eq!(parse_jobs("16"), Ok(Jobs::Count(16)));
    }

    #[test]
    fn parse_jobs_rejects_zero_negative_and_garbage() {
        assert!(parse_jobs("0").is_err());
        assert!(parse_jobs("-4").is_err());
        assert!(parse_jobs("eight").is_err());
        assert!(parse_jobs("").is_err());
    }

    #[test]
    fn jobs_resolve_count_is_identity_and_auto_is_at_least_one() {
        assert_eq!(Jobs::Count(7).resolve(), 7);
        assert!(Jobs::Auto.resolve() >= 1);
    }

    fn specs() -> Vec<TaskSpec> {
        ["alpha", "beta"]
            .iter()
            .map(|n| TaskSpec {
                name: n.to_string(),
                workdir: format!("three_{n}"),
            })
            .collect()
    }

    #[test]
    fn task_index_uses_first_bracket_tag_only() {
        let tasks = specs();
        assert_eq!(
            task_index_of("SBY 12:00:01 [three_beta] DONE (PASS, rc=0)", &tasks),
            Some(1)
        );
        // Sonraki parantezler (Elapsed [H:MM:SS]) etiketi değiştirmez.
        assert_eq!(
            task_index_of(
                "SBY 12:00:01 [three_alpha] summary: Elapsed clock time [H:MM:SS (secs)]: 0:00:01 (1)",
                &tasks
            ),
            Some(0)
        );
    }

    #[test]
    fn task_index_rejects_unknown_or_untagged_lines() {
        let tasks = specs();
        assert_eq!(
            task_index_of("SBY 12:00:01 [other] DONE (PASS)", &tasks),
            None
        );
        assert_eq!(task_index_of("ERROR: cannot parse config", &tasks), None);
        // Öneksiz `alpha` çalışma dizini değildir (three_alpha olmalı).
        assert_eq!(task_index_of("SBY [alpha] DONE (PASS)", &tasks), None);
    }
}
