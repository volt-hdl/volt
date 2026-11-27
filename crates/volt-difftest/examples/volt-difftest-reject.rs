//! Beklenen ret ölçütü (#458) verilen bir `volt` ikilisiyle: her tipin
//! tanımsız işlemleri (`volt_difftest::reject::UNDEFINED`) beklenen tanı
//! koduyla reddedilmeli. Verilator gerekmez. Yayımlanmış eski bir sürümü
//! sınamak için (v0.1.3'te #434: `~t` kabul ediliyordu):
//!
//! ```text
//! cargo run -p volt-difftest --example volt-difftest-reject -- --volt <volt.exe> [--seed N] [--per-row N] [--work DIR]
//! ```
//!
//! Çıkış kodu: fark yoksa 0, fark varsa 1, kullanım hatasında 2.

use std::path::PathBuf;
use std::process::ExitCode;

use volt_difftest::reject::run_reject;

const USAGE: &str =
    "usage: volt-difftest-reject --volt <path> [--seed N] [--per-row N] [--work DIR]";

/// Varsayılan tohum: #458.
const DEFAULT_SEED: u64 = 458;
const DEFAULT_PER_ROW: usize = 8;

struct Args {
    volt: PathBuf,
    seed: u64,
    per_row: usize,
    work: PathBuf,
}

fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut volt = None;
    let mut seed = DEFAULT_SEED;
    let mut per_row = DEFAULT_PER_ROW;
    let mut work = None;
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match a.as_str() {
            "--volt" => volt = Some(PathBuf::from(value("--volt")?)),
            "--seed" => {
                let v = value("--seed")?;
                seed = v.parse().map_err(|_| format!("--seed {v}: not a number"))?;
            }
            "--per-row" => {
                let v = value("--per-row")?;
                per_row = v
                    .parse()
                    .map_err(|_| format!("--per-row {v}: not a number"))?;
            }
            "--work" => work = Some(PathBuf::from(value("--work")?)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let volt = volt.ok_or("--volt is required")?;
    let work = work.unwrap_or_else(|| {
        std::env::temp_dir().join(format!(
            "volt-difftest-reject-{}-{seed}",
            std::process::id()
        ))
    });
    Ok(Args {
        volt,
        seed,
        per_row,
        work,
    })
}

fn main() -> ExitCode {
    let args = match parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    println!("difftest-reject: volt={}", args.volt.display());
    let report = run_reject(&args.volt, args.seed, args.per_row, &args.work);
    print!("{}", report.render());
    if report.ok() {
        let _ = std::fs::remove_dir_all(&args.work);
        ExitCode::SUCCESS
    } else {
        println!(
            "difftest-reject: work directory kept: {}",
            args.work.display()
        );
        ExitCode::from(1)
    }
}
