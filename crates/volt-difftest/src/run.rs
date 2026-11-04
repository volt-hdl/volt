//! Koşucu: üret → `volt check` → `volt build` → Verilator → karşılaştır.
//!
//! Derleyici yalnız `volt` ikilisi olarak, Verilator bir süreç olarak
//! çağrılır; araçların yolunu çağıran verir (testte `tools::require`,
//! ADR-0107).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ast::{Expr, PortTy};
use crate::gen::{Gen, Pool};
use crate::harness::{
    build, ensures_order, parse_sim, shape, sv_owners, testbench, vec_hex, vectors, Built, Case,
    Kind, Variant,
};
use crate::model::{tag_ids, Model};
use crate::rng::Rng;
use crate::value::mask;

pub struct Tools {
    pub volt: PathBuf,
    pub verilator: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub seed: u64,
    /// Hedef: bu kadar kabul edilmiş ifade.
    pub count: usize,
    /// Toplu koşu başına ifade.
    pub batch: usize,
    pub vectors: usize,
    /// Küçültülecek en çok fark grubu.
    pub minimize: usize,
}

impl Config {
    pub const DEFAULT_SEED: u64 = 20_261_104;
    pub const DEFAULT_COUNT: usize = 2000;

    /// `VOLT_DIFFTEST_SEED`, `VOLT_DIFFTEST_COUNT`, `VOLT_DIFFTEST_BATCH`,
    /// `VOLT_DIFFTEST_MINIMIZE`.
    pub fn from_env() -> Config {
        fn var<T: std::str::FromStr>(name: &str, default: T) -> T {
            match std::env::var(name) {
                Ok(v) => v
                    .parse()
                    .unwrap_or_else(|_| panic!("{name}={v}: sayı değil")),
                Err(_) => default,
            }
        }
        Config {
            seed: var("VOLT_DIFFTEST_SEED", Self::DEFAULT_SEED),
            count: var("VOLT_DIFFTEST_COUNT", Self::DEFAULT_COUNT),
            batch: var("VOLT_DIFFTEST_BATCH", 250),
            vectors: 16,
            minimize: var("VOLT_DIFFTEST_MINIMIZE", 6),
        }
    }
}

/// Bulgu sınıfları. `soft` olanlar raporlanır ama koşuyu düşürmez.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// Verilator'daki değer modelden farklı.
    ValueDiff,
    /// Sözleşme (`ensures: o == e`) simülasyonda düştü.
    ContractFired,
    /// Model geçerli der, derleyici reddeder.
    ModelValidRejected,
    /// Model hatalı der, derleyici kabul eder.
    ModelInvalidAccepted,
    /// Aynı ifadenin biçimleri farklı tanı verir.
    VariantCodeMismatch,
    /// Derleyicinin ürettiği SV Verilator'da derlenmez.
    InvalidSv,
    /// Model ve derleyici aynı ifadeyi reddeder, kodlar farklı (soft).
    CodeMismatch,
    /// Üretilen SV'de Verilator -Wall uyarısı (soft).
    Lint,
    /// Sınamanın kendisi bozuk (testbench girişleri DUT'a ulaşmadı).
    Harness,
    /// Üretilen SV'de Verilator WIDTH uyarısı: varsayılan olarak ölümcül,
    /// `volt test` durur (#123).
    WidthLint,
}

impl Class {
    pub fn soft(self) -> bool {
        matches!(self, Class::CodeMismatch | Class::Lint)
    }

    pub fn name(self) -> &'static str {
        match self {
            Class::ValueDiff => "value differs from the model",
            Class::ContractFired => "contract fired in simulation",
            Class::ModelValidRejected => "model: valid, compiler: rejected",
            Class::ModelInvalidAccepted => "model: invalid, compiler: accepted",
            Class::VariantCodeMismatch => "variants of one expression get different diagnostics",
            Class::InvalidSv => "generated SV does not compile in Verilator",
            Class::CodeMismatch => "both reject, different code (soft)",
            Class::Lint => "Verilator -Wall warning in the generated SV (soft)",
            Class::Harness => "harness error: testbench inputs did not reach the design",
            Class::WidthLint => {
                "Verilator WIDTH warning in the generated SV (fatal by default, volt test stops)"
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub class: Class,
    /// Gruplama anahtarı: biçimler + şekil.
    pub key: String,
    pub text: String,
    pub expr: Expr,
    pub case_id: usize,
    pub variant: Option<Variant>,
    pub ports: Vec<PortTy>,
    pub vec: Vec<u128>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    /// Regresyon kümesi: (örnek sayısı, düşen örnek sayısı).
    pub regressions: (usize, usize),
    pub seed: u64,
    pub target: usize,
    pub generated: usize,
    pub accepted: usize,
    pub batches: usize,
    pub variants_compared: usize,
    pub values_compared: usize,
    pub x_skipped: usize,
    pub contracts_checked: usize,
    pub rejected_as_predicted: usize,
    /// Reddedilen biçimlerin kodları.
    pub rejected_codes: BTreeMap<String, usize>,
    /// E0003: (görülen, E0003 ile reddedilen).
    pub unsupported: BTreeMap<String, (usize, usize)>,
    /// Spesifikasyonun tanımlamadığı: (görülen, kabul edilen, reddedilen).
    pub unspecified: BTreeMap<String, (usize, usize, usize)>,
    pub tag_counts: BTreeMap<String, usize>,
    pub findings: Vec<Finding>,
    pub minimized: Vec<String>,
}

impl Report {
    pub fn hard_findings(&self) -> usize {
        self.findings.iter().filter(|f| !f.class.soft()).count()
    }

    pub fn ok(&self) -> bool {
        self.hard_findings() == 0 && self.accepted >= self.target
    }

    pub fn summary_line(&self) -> String {
        format!(
            "difftest: seed={} target={} generated={} accepted={} rejected-as-predicted={} unsupported={} unspecified={} values-compared={} diffs={}",
            self.seed,
            self.target,
            self.generated,
            self.accepted,
            self.rejected_as_predicted,
            self.unsupported.values().map(|v| v.0).sum::<usize>(),
            self.unspecified.values().map(|v| v.0).sum::<usize>(),
            self.values_compared,
            self.hard_findings()
        )
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "{}", self.summary_line());
        let _ = writeln!(
            s,
            "difftest: regressions={} failed={}",
            self.regressions.0, self.regressions.1
        );
        let _ = writeln!(
            s,
            "difftest: batches={} variants-compared={} x-skipped={} contracts={}",
            self.batches, self.variants_compared, self.x_skipped, self.contracts_checked
        );
        let mut by_class: BTreeMap<Class, Vec<&Finding>> = BTreeMap::new();
        for f in &self.findings {
            by_class.entry(f.class).or_default().push(f);
        }
        for (class, fs) in &by_class {
            let _ = writeln!(s, "difftest: {} — {}", class.name(), fs.len());
            // Uyarılar kodlarına göre: her koddan en küçük dört ifade (ad
            // sırasıyla ilk 25 grup bir kodu hiç göstermeyebiliyordu).
            if matches!(class, Class::Lint | Class::WidthLint) {
                let mut by_code: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
                for f in fs {
                    by_code
                        .entry(f.key.split(' ').next().unwrap_or("?"))
                        .or_default()
                        .push(f);
                }
                for (code, mut g) in by_code {
                    g.sort_by_key(|f| f.expr.size());
                    let _ = writeln!(s, "  {code}: {}", g.len());
                    for f in g.iter().take(4) {
                        for line in f.text.lines().take(6) {
                            let _ = writeln!(s, "      {line}");
                        }
                    }
                }
                continue;
            }
            let mut groups: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
            for f in fs {
                groups.entry(f.key.as_str()).or_default().push(f);
            }
            for (key, g) in groups.iter().take(25) {
                let _ = writeln!(s, "  [{}x] {key}", g.len());
                for line in g[0].text.lines().take(12) {
                    let _ = writeln!(s, "      {line}");
                }
            }
            if groups.len() > 25 {
                let _ = writeln!(s, "  ... {} more groups", groups.len() - 25);
            }
        }
        // Verilator -Wall uyarılarının tam sayımı (yukarıdaki liste ilk 25
        // grubu gösterir).
        let mut lint: BTreeMap<&str, usize> = BTreeMap::new();
        for f in self
            .findings
            .iter()
            .filter(|f| matches!(f.class, Class::Lint | Class::WidthLint))
        {
            *lint
                .entry(f.key.split(' ').next().unwrap_or("?"))
                .or_default() += 1;
        }
        if !lint.is_empty() {
            let list: Vec<String> = lint.iter().map(|(c, n)| format!("{c}={n}")).collect();
            let _ = writeln!(s, "difftest: lint codes: {}", list.join(" "));
        }
        if !self.rejected_codes.is_empty() {
            let list: Vec<String> = self
                .rejected_codes
                .iter()
                .map(|(c, n)| format!("{c}={n}"))
                .collect();
            let _ = writeln!(s, "difftest: rejected variant codes: {}", list.join(" "));
        }
        for (r, (seen, e0003)) in &self.unsupported {
            let _ = writeln!(s, "difftest: unsupported (E0003 expected): {r}: {seen} seen, {e0003} rejected with E0003");
        }
        for (r, (seen, acc, rej)) in &self.unspecified {
            let _ = writeln!(s, "difftest: unspecified (not compared): {r}: {seen} seen, compiler accepted {acc}, rejected {rej}");
        }
        if !self.tag_counts.is_empty() {
            let list: Vec<String> = self
                .tag_counts
                .iter()
                .map(|(c, n)| format!("{c}={n}"))
                .collect();
            let _ = writeln!(
                s,
                "difftest: assumptions used by compared variants: {}",
                list.join(" ")
            );
        }
        for m in &self.minimized {
            let _ = writeln!(s, "difftest: minimized:\n{m}");
        }
        s
    }
}

fn run_cmd(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("{cmd:?} çalıştırılamadı: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn volt(tools: &Tools, dir: &Path, args: &[&str]) -> (bool, String) {
    let (ok, out, err) = run_cmd(
        Command::new(&tools.volt)
            .env("VOLT_TOOL_BACKEND", "local")
            .env("VOLT_LANG", "en")
            .env("NO_COLOR", "1")
            .current_dir(dir)
            .args(args),
    );
    (ok, format!("{out}{err}"))
}

/// `dt.volt:L:C: error[CODE]: ...` → (satır, kod).
fn volt_errors(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("dt.volt:") else {
            continue;
        };
        let mut it = rest.splitn(3, ':');
        let (Some(l), Some(_), Some(msg)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let msg = msg.trim_start();
        let Some(code) = msg.strip_prefix("error[") else {
            continue;
        };
        let code: String = code.chars().take_while(|c| *c != ']').collect();
        if let Ok(l) = l.parse::<usize>() {
            out.push((l, code));
        }
    }
    out
}

/// Verilator iletisi: (düzey, kod, dosya, satır, metin).
fn verilator_msgs(text: &str) -> Vec<(String, String, String, usize, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let line = match line.find("%Error").or_else(|| line.find("%Warning")) {
            Some(i) => &line[i..],
            None => continue,
        };
        let (head, rest) = match line.split_once(": ") {
            Some(x) => x,
            None => continue,
        };
        let (level, code) = match head.split_once('-') {
            Some((l, c)) => (l.to_string(), c.to_string()),
            None => (head.to_string(), String::new()),
        };
        let mut parts = rest.splitn(3, ':');
        let (Some(file), Some(l)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(l) = l.trim().parse::<usize>() else {
            continue;
        };
        let msg = parts.next().unwrap_or("").to_string();
        out.push((level, code, file.to_string(), l, msg));
    }
    out
}

struct Batch {
    cases: Vec<Case>,
    ports: Vec<PortTy>,
    vecs: Vec<Vec<u128>>,
}

/// Bir toplu koşunun sonucu (rapora eklenir).
#[derive(Default)]
struct Outcome {
    accepted: Vec<usize>,
    findings: Vec<Finding>,
}

pub struct Runner<'a> {
    pub cfg: &'a Config,
    pub tools: &'a Tools,
    pub work: PathBuf,
    pub report: Report,
    dir_seq: usize,
}

impl<'a> Runner<'a> {
    pub fn new(cfg: &'a Config, tools: &'a Tools, work: &Path) -> Runner<'a> {
        Runner {
            cfg,
            tools,
            work: work.to_path_buf(),
            report: Report {
                seed: cfg.seed,
                target: cfg.count,
                ..Report::default()
            },
            dir_seq: 0,
        }
    }

    fn fresh_dir(&mut self) -> PathBuf {
        self.dir_seq += 1;
        let d = self.work.join(format!("b{}", self.dir_seq));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("toplu koşu dizini");
        d
    }

    fn finding(
        &self,
        class: Class,
        case: &Case,
        b: &Batch,
        v: Option<Variant>,
        detail: String,
        vec: Vec<u128>,
    ) -> Finding {
        let model = Model { ports: &b.ports };
        let vs = v.map_or("-".to_string(), |v| v.letter().to_string());
        let tags = v.map_or(0, |v| case.tags(v, &model));
        let key = format!("{vs} {}", shape(&case.expr, &b.ports));
        let mut text = String::new();
        let _ = writeln!(
            text,
            "variant {vs} ({}), assumptions {}",
            v.map_or("-", Variant::what),
            tag_ids(tags)
        );
        let mut ps = Vec::new();
        case.expr.ports(&mut ps);
        ps.sort_unstable();
        ps.dedup();
        let decl: Vec<String> = ps
            .iter()
            .map(|k| format!("p{k} : {}", b.ports[*k].volt()))
            .collect();
        let _ = writeln!(text, "ports: {}", decl.join(", "));
        if let Some(v) = v {
            let _ = writeln!(text, "{}", case.variant_text(v));
        } else {
            let _ = writeln!(
                text,
                "{}",
                crate::print::print(&case.expr, crate::print::Style::Min)
            );
        }
        let _ = write!(text, "{detail}");
        Finding {
            class,
            key,
            text,
            expr: case.expr.clone(),
            case_id: case.id,
            variant: v,
            ports: b.ports.clone(),
            vec,
        }
    }

    /// Ana döngü: hedef sayıda ifade kabul edilene kadar toplu koşular.
    pub fn run(mut self) -> Report {
        self.run_regressions();
        let mut rng = Rng::new(self.cfg.seed);
        let max_generated = self.cfg.count * 3 + 1000;
        while self.report.accepted < self.cfg.count && self.report.generated < max_generated {
            let batch = self.generate(&mut rng);
            self.report.generated += batch.cases.len();
            self.report.batches += 1;
            let out = self.run_batch(&batch, true);
            self.report.accepted += out.accepted.len();
            self.report.findings.extend(out.findings);
        }
        self.minimize_findings();
        self.report
    }

    /// Sabit regresyon kümesi (`regress.rs`), tek toplu koşuda.
    fn run_regressions(&mut self) {
        let regs = crate::regress::all();
        let mut ports = Vec::new();
        let mut exprs = Vec::new();
        for r in &regs {
            exprs.push(r.expr.shift_ports(ports.len()));
            ports.extend(r.ports.iter().copied());
        }
        let mut rng = Rng::new(self.cfg.seed ^ 0x000E_6E55);
        let cases: Vec<Case> = exprs
            .into_iter()
            .enumerate()
            .map(|(i, e)| Case::new(i, e, &ports, &mut rng))
            .collect();
        for (c, r) in cases.iter().zip(&regs) {
            assert!(
                c.is_valid(),
                "regresyon örneği modelde geçersiz: {} ({:?})",
                r.what,
                c.kind
            );
        }
        let vecs = vectors(&ports, self.cfg.vectors, &mut rng);
        let mut cases = cases;
        for c in &mut cases {
            c.drop_x_contract(&ports, &vecs);
        }
        let batch = Batch { cases, ports, vecs };
        let out = self.run_batch(&batch, false);
        let mut failed = BTreeSet::new();
        for mut f in out.findings {
            if f.class.soft() {
                continue;
            }
            let r = &regs[f.case_id];
            failed.insert(f.case_id);
            f.key = format!("regression {}: {} — {}", r.issue, r.what, f.key);
            self.report.findings.push(f);
        }
        self.report.regressions = (regs.len(), failed.len());
    }

    fn generate(&self, rng: &mut Rng) -> Batch {
        let mut pool = Pool::default();
        let mut exprs = Vec::new();
        for _ in 0..self.cfg.batch {
            let mut g = Gen {
                rng: &mut *rng,
                pool: &mut pool,
            };
            let roll = g.rng.below(1000);
            let e = if roll < 50 {
                g.invalid()
            } else if roll < 55 {
                g.concat()
            } else {
                g.expr()
            };
            exprs.push(e);
        }
        let cases = exprs
            .into_iter()
            .enumerate()
            .map(|(i, e)| Case::new(i, e, &pool.ports, rng))
            .collect();
        let vecs = vectors(&pool.ports, self.cfg.vectors, rng);
        let mut cases: Vec<Case> = cases;
        for c in &mut cases {
            c.drop_x_contract(&pool.ports, &vecs);
        }
        Batch {
            cases,
            ports: pool.ports,
            vecs,
        }
    }

    /// `account`: rapordaki sayaçlar güncellenir mi (küçültmede hayır).
    /// Toplu koşu; dizini (Verilator `obj` derlemesi dahil, toplu koşu
    /// başına onlarca MB) iş bitince silinir — 50.000 ifadelik koşu
    /// diskte birikmez. `VOLT_DIFFTEST_KEEP=1` dizinleri bırakır.
    fn run_batch(&mut self, b: &Batch, account: bool) -> Outcome {
        let dir = self.fresh_dir();
        let out = self.run_batch_in(b, account, &dir);
        if std::env::var("VOLT_DIFFTEST_KEEP").map_or(true, |v| v != "1") {
            let _ = std::fs::remove_dir_all(&dir);
        }
        out
    }

    fn run_batch_in(&mut self, b: &Batch, account: bool, dir: &Path) -> Outcome {
        let mut out = Outcome::default();
        let dir = dir.to_path_buf();
        let model = Model { ports: &b.ports };
        let mut active: Vec<usize> = (0..b.cases.len()).collect();
        // Tanılar: (ifade kimliği → biçim → kodlar)
        loop {
            let cases: Vec<Case> = active.iter().map(|i| b.cases[*i].clone()).collect();
            let built = build(&cases, &b.ports);
            std::fs::write(dir.join("dt.volt"), &built.text).expect("dt.volt");
            let (_, text) = volt(self.tools, &dir, &["check", "--format=short", "dt.volt"]);
            let errs = volt_errors(&text);
            let mut by_case: BTreeMap<usize, BTreeMap<Variant, Vec<String>>> = BTreeMap::new();
            for (l, code) in &errs {
                match built.line_owner.get(l - 1).copied().flatten() {
                    Some((id, v)) => by_case
                        .entry(id)
                        .or_default()
                        .entry(v)
                        .or_default()
                        .push(code.clone()),
                    None => panic!(
                        "sahipsiz tanı (satır {l}):\n{text}\n--- dt.volt\n{}",
                        built.text
                    ),
                }
            }
            // Tanısız ifade, turda başka hata varken bekletilir: önceki
            // aşamanın hataları (tip denetimi) sonraki aşamanınkileri
            // (emitter, E0003) gizler. Temiz turda tanısız kalan geçersiz
            // ifade "kabul edildi" sayılır.
            let clean = by_case.is_empty();
            let mut removed = BTreeSet::new();
            for &i in &active {
                let c = &b.cases[i];
                let codes = by_case.remove(&c.id).unwrap_or_default();
                if codes.is_empty() && (c.is_valid() || !clean) {
                    continue;
                }
                removed.insert(i);
                self.classify_rejection(c, b, &codes, account, &mut out);
            }
            active.retain(|i| !removed.contains(i));
            if !clean {
                continue;
            }
            if active.is_empty() {
                return out;
            }
            // Derle
            let (ok, text) = volt(
                self.tools,
                &dir,
                &[
                    "build",
                    "--format=short",
                    "--emit=sva",
                    "--sva",
                    "inline",
                    "--target-dir",
                    "build",
                    "dt.volt",
                ],
            );
            if !ok {
                let errs = volt_errors(&text);
                let mut hit = BTreeSet::new();
                for (l, code) in &errs {
                    if let Some((id, v)) = built.line_owner.get(l - 1).copied().flatten() {
                        let pos = active
                            .iter()
                            .position(|i| b.cases[*i].id == id)
                            .expect("aktif");
                        if hit.insert(pos) {
                            let c = &b.cases[active[pos]];
                            out.findings.push(self.finding(
                                Class::ModelValidRejected,
                                c,
                                b,
                                Some(v),
                                format!("volt build: {code} (volt check accepted)"),
                                vec![],
                            ));
                        }
                    }
                }
                assert!(
                    !hit.is_empty(),
                    "volt build düştü, tanı eşlenemedi:\n{text}"
                );
                let drop: BTreeSet<usize> = hit.iter().map(|p| active[*p]).collect();
                active.retain(|i| !drop.contains(i));
                continue;
            }
            match self.simulate(b, &dir, &built, &active, &model, account) {
                Ok(sim) => {
                    out.findings.extend(sim.findings);
                    out.accepted.extend(sim.accepted);
                    return out;
                }
                Err(bad) => {
                    // Derlenmeyen SV: sorumlu ifadeleri çıkar, yeniden dene.
                    for (i, f) in bad {
                        out.findings.push(f);
                        active.retain(|x| *x != i);
                    }
                }
            }
        }
    }

    fn classify_rejection(
        &mut self,
        c: &Case,
        b: &Batch,
        codes: &BTreeMap<Variant, Vec<String>>,
        account: bool,
        out: &mut Outcome,
    ) {
        let first: BTreeMap<Variant, String> =
            codes.iter().map(|(v, cs)| (*v, cs[0].clone())).collect();
        let emitted: Vec<Variant> = c.variants.clone();
        if account {
            for code in first.values() {
                *self.report.rejected_codes.entry(code.clone()).or_default() += 1;
            }
        }
        // Biçimler arası tutarlılık: her biçim reddedilmeli ve aynı kodla.
        // (Geçerli ifadede sözleşme `o == e`'yi okur: o biçimin reddi yalnız
        // A/B'den kaynaklanabilir — G'yi tutarlılıktan ayırmayız.)
        let distinct: BTreeSet<&String> = first.values().collect();
        let missing: Vec<Variant> = emitted
            .iter()
            .filter(|v| !first.contains_key(v))
            .copied()
            .collect();
        match &c.kind {
            Kind::Valid(_) => {
                let v = *first.keys().next().expect("en az bir tanı");
                let list: Vec<String> = first
                    .iter()
                    .map(|(v, c)| format!("{}:{c}", v.letter()))
                    .collect();
                let all_e0003 = first.values().all(|c| c == "E0003");
                let class = Class::ModelValidRejected;
                let mut f = self.finding(
                    class,
                    c,
                    b,
                    Some(v),
                    format!(
                        "compiler: {}{}",
                        list.join(" "),
                        if all_e0003 {
                            " (E0003: not supported yet)"
                        } else {
                            ""
                        }
                    ),
                    vec![],
                );
                if all_e0003 {
                    f.key = format!("E0003 {}", f.key);
                }
                out.findings.push(f);
                if distinct.len() > 1 {
                    out.findings.push(self.finding(
                        Class::VariantCodeMismatch,
                        c,
                        b,
                        Some(v),
                        format!("codes per variant: {}", list.join(" ")),
                        vec![],
                    ));
                }
            }
            Kind::Invalid(expected) => {
                let list: Vec<String> = first
                    .iter()
                    .map(|(v, c)| format!("{}:{c}", v.letter()))
                    .collect();
                if first.is_empty() {
                    out.findings.push(self.finding(
                        Class::ModelInvalidAccepted,
                        c,
                        b,
                        None,
                        format!("model: {expected}, compiler: no error"),
                        vec![],
                    ));
                    return;
                }
                if !missing.is_empty() || distinct.len() > 1 {
                    let miss: Vec<String> =
                        missing.iter().map(|v| v.letter().to_string()).collect();
                    out.findings.push(self.finding(
                        Class::VariantCodeMismatch,
                        c,
                        b,
                        None,
                        format!(
                            "codes per variant: {}; accepted: {}",
                            list.join(" "),
                            miss.join(",")
                        ),
                        vec![],
                    ));
                }
                if account {
                    self.report.rejected_as_predicted += 1;
                }
                if *expected != "?" && !first.values().any(|c| c == expected) {
                    let mut f = self.finding(
                        Class::CodeMismatch,
                        c,
                        b,
                        None,
                        format!("model: {expected}, compiler: {}", list.join(" ")),
                        vec![],
                    );
                    f.key = format!("{expected}->{} {}", first.values().next().unwrap(), f.key);
                    out.findings.push(f);
                }
            }
            Kind::Unsupported(r) => {
                if account {
                    let e = self.report.unsupported.entry((*r).to_string()).or_default();
                    e.0 += 1;
                    if first.values().any(|c| c == "E0003") {
                        e.1 += 1;
                    }
                }
                if first.is_empty() {
                    out.findings.push(self.finding(
                        Class::ModelInvalidAccepted,
                        c,
                        b,
                        None,
                        format!("model: unsupported ({r}), compiler: no error"),
                        vec![],
                    ));
                }
            }
            Kind::Unspecified(r) => {
                if account {
                    let e = self.report.unspecified.entry((*r).to_string()).or_default();
                    e.0 += 1;
                    if first.is_empty() {
                        e.1 += 1;
                    } else {
                        e.2 += 1;
                    }
                }
            }
        }
    }

    /// Verilator ile derle, koştur, karşılaştır. SV derlenmezse sorumlu
    /// ifadeler (`Err`).
    fn simulate(
        &mut self,
        b: &Batch,
        dir: &Path,
        built: &Built,
        active: &[usize],
        model: &Model,
        account: bool,
    ) -> Result<Outcome, Vec<(usize, Finding)>> {
        let sv_path = dir
            .join("build")
            .join("rtl")
            .join(format!("{}.sv", crate::harness::MODULE));
        let sv = std::fs::read_to_string(&sv_path)
            .unwrap_or_else(|e| panic!("{}: {e}", sv_path.display()));
        let ens = ensures_order(built);
        let owners = sv_owners(&sv, &ens);
        std::fs::write(
            dir.join("tb.sv"),
            testbench(built, &b.ports, self.cfg.vectors),
        )
        .expect("tb.sv");
        for (ci, text) in vec_hex(built, &b.ports, &b.vecs).into_iter().enumerate() {
            std::fs::write(dir.join(format!("vec{ci}.hex")), text).expect("vec.hex");
        }
        let rel_sv = format!("build/rtl/{}.sv", crate::harness::MODULE);
        let (ok, vout, verr) =
            run_cmd(Command::new(&self.tools.verilator).current_dir(dir).args([
                "--binary",
                "--timing",
                "--assert",
                "-Wall",
                "-Wno-fatal",
                "-Wno-DECLFILENAME",
                "--top-module",
                "tb",
                "-Mdir",
                "obj",
                "-o",
                "vtb",
                "-j",
                "2",
                "-CFLAGS",
                "-O0",
                "tb.sv",
                &rel_sv,
            ]));
        let vtext = format!("{vout}{verr}");
        let msgs = verilator_msgs(&vtext);
        let by_id = |id: usize| active.iter().copied().find(|i| b.cases[*i].id == id);
        let mut findings = Vec::new();
        if !ok {
            let mut bad: BTreeMap<usize, Finding> = BTreeMap::new();
            for (level, code, file, line, msg) in &msgs {
                if level != "%Error" || !file.ends_with(&format!("{}.sv", crate::harness::MODULE)) {
                    continue;
                }
                if let Some((id, v)) = owners.get(line - 1).copied().flatten() {
                    if let Some(i) = by_id(id) {
                        let c = &b.cases[i];
                        let sv_line = sv.lines().nth(line - 1).unwrap_or("").trim().to_string();
                        bad.entry(i).or_insert_with(|| {
                            self.finding(
                                Class::InvalidSv,
                                c,
                                b,
                                Some(v),
                                format!("verilator {code}:{msg}\nSV: {sv_line}"),
                                vec![],
                            )
                        });
                    }
                }
            }
            assert!(
                !bad.is_empty(),
                "Verilator düştü, satır eşlenemedi:\n{vtext}"
            );
            return Err(bad.into_iter().collect());
        }
        // Testbench'in bağlantı uyarısı (PINMISSING, PINNOTFOUND, ...):
        // bir giriş ya da çıkış DUT'a bağlanmadı, değerler anlamsız.
        let pin: Vec<String> = msgs
            .iter()
            .filter(|(_, code, file, ..)| file.ends_with("tb.sv") && code.starts_with("PIN"))
            .map(|(_, code, _, line, msg)| format!("tb.sv:{line}: {code}:{msg}"))
            .collect();
        if !pin.is_empty() {
            let c = &b.cases[active[0]];
            let mut f = self.finding(Class::Harness, c, b, None, pin.join("\n"), vec![]);
            f.key = "testbench port connection".into();
            return Ok(Outcome {
                accepted: vec![],
                findings: vec![f],
            });
        }
        // Uyarılar: yalnız üretilen SV'de ve bir ifadeye ait satırda. WIDTH
        // uyarıları Verilator'da varsayılan olarak ölümcüldür (`volt test`
        // durur, #123): sert bulgu; gerisi (CMPCONST, UNSIGNED,
        // UNUSEDSIGNAL: ifadenin kendisi) yumuşak.
        let mut linted = BTreeSet::new();
        for (level, code, file, line, msg) in &msgs {
            if level != "%Warning" || !file.ends_with(&format!("{}.sv", crate::harness::MODULE)) {
                continue;
            }
            if let Some((id, v)) = owners.get(line - 1).copied().flatten() {
                if let Some(i) = by_id(id) {
                    if linted.insert((i, code.clone())) {
                        let sv_line = sv.lines().nth(line - 1).unwrap_or("").trim().to_string();
                        let class = if code.starts_with("WIDTH") {
                            Class::WidthLint
                        } else {
                            Class::Lint
                        };
                        let mut f = self.finding(
                            class,
                            &b.cases[i],
                            b,
                            Some(v),
                            format!("{code}:{msg}\nSV: {sv_line}"),
                            vec![],
                        );
                        f.key = format!("{code} {}", f.key);
                        findings.push(f);
                    }
                }
            }
        }
        let (_, sout, serr) = run_cmd(
            Command::new(dir.join("obj").join("vtb"))
                .current_dir(dir)
                .arg("+verilator+error+limit+1000000"),
        );
        let vals = parse_sim(built, &sout, self.cfg.vectors)
            .unwrap_or_else(|e| panic!("simülasyon çıktısı: {e}\n{sout}\n{serr}"));
        // Girişler DUT'a ulaşmadıysa karşılaştırma anlamsızdır: sert bulgu.
        if let Err(e) = crate::harness::check_echo(built, &b.ports, &b.vecs, &sout) {
            let c = &b.cases[active[0]];
            let mut f = self.finding(Class::Harness, c, b, None, e, vec![]);
            f.key = "testbench input echo".into();
            return Ok(Outcome {
                accepted: vec![],
                findings: vec![f],
            });
        }
        let mut accepted = BTreeSet::new();
        let mut diffed = BTreeSet::new();
        for (j, o) in built.outputs.iter().enumerate() {
            let i = by_id(o.case).expect("aktif ifade");
            let c = &b.cases[i];
            accepted.insert(i);
            if account {
                self.report.variants_compared += 1;
                for (t, id, _) in crate::model::ASSUMPTIONS {
                    if c.tags(o.variant, model) & t != 0 {
                        *self.report.tag_counts.entry((*id).to_string()).or_default() += 1;
                    }
                }
            }
            for (vi, row) in vals.iter().enumerate() {
                let got = row[j] & mask(o.ty.width());
                match c.expected(o.variant, model, &b.vecs[vi]) {
                    None => {
                        if account {
                            self.report.x_skipped += 1;
                        }
                    }
                    Some(want) => {
                        let want = want & mask(o.ty.width());
                        if account {
                            self.report.values_compared += 1;
                        }
                        if want != got && diffed.insert((i, o.variant)) {
                            let mut ps = Vec::new();
                            c.expr.ports(&mut ps);
                            ps.sort_unstable();
                            ps.dedup();
                            let inputs: Vec<String> = ps
                                .iter()
                                .map(|k| format!("p{k}={:#x}", b.vecs[vi][*k]))
                                .collect();
                            let sv_text: Vec<&str> = sv
                                .lines()
                                .zip(&owners)
                                .filter(|(_, ow)| {
                                    matches!(ow, Some((id, v)) if *id == c.id
                                        && (*v == o.variant
                                            || (o.variant == Variant::F && *v == Variant::B)))
                                })
                                .map(|(l, _)| l.trim())
                                .collect();
                            findings.push(self.finding(
                                Class::ValueDiff,
                                c,
                                b,
                                Some(o.variant),
                                format!(
                                    "inputs: {}\nmodel: {want:#x}, Verilator: {got:#x} ({})\nSV: {}",
                                    inputs.join(" "),
                                    o.ty.volt(),
                                    sv_text.join("\n    ")
                                ),
                                b.vecs[vi].clone(),
                            ));
                        }
                    }
                }
            }
        }
        // Sözleşmeler: her ensures her vektörde bir kez örneklenir.
        if account {
            self.report.contracts_checked += ens.len() * self.cfg.vectors;
        }
        for line in format!("{sout}{serr}").lines() {
            if !(line.contains("Assertion failed") || line.contains("'assert' failed")) {
                continue;
            }
            let Some(l) = line
                .split(&format!("{}.sv:", crate::harness::MODULE))
                .nth(1)
                .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|n| n.parse::<usize>().ok())
            else {
                continue;
            };
            if let Some((id, _)) = owners.get(l - 1).copied().flatten() {
                if let Some(i) = by_id(id) {
                    if diffed.insert((i, Variant::G)) {
                        findings.push(self.finding(
                            Class::ContractFired,
                            &b.cases[i],
                            b,
                            Some(Variant::G),
                            format!("Verilator: {}", line.trim()),
                            vec![],
                        ));
                    }
                }
            }
        }
        Ok(Outcome {
            accepted: accepted.into_iter().collect(),
            findings,
        })
    }

    /// Fark gruplarını küçültür: bir düğümü çocuğuyla ya da aynı tipte bir
    /// portla değiştiren adaylar tek toplu koşuda denenir; farkı koruyan en
    /// küçük aday seçilir.
    fn minimize_findings(&mut self) {
        let mut seen = BTreeSet::new();
        let mut targets: Vec<Finding> = self
            .report
            .findings
            .iter()
            .filter(|f| {
                matches!(
                    f.class,
                    Class::ValueDiff
                        | Class::ContractFired
                        | Class::InvalidSv
                        | Class::ModelValidRejected
                        | Class::WidthLint
                )
            })
            .filter(|f| seen.insert(format!("{:?}", f.expr)))
            .cloned()
            .collect();
        targets.sort_by_key(|f| f.expr.size());
        targets.truncate(self.cfg.minimize);
        for f in targets {
            let text = self.minimize(&f);
            self.report.minimized.push(text);
        }
    }

    fn minimize(&mut self, f: &Finding) -> String {
        let mut rng = Rng::new(self.cfg.seed ^ 0x5EED);
        let mut ports = f.ports.clone();
        let mut cur = f.expr.clone();
        let mut last = f.text.clone();
        for _round in 0..12 {
            let mut cands: Vec<Expr> = Vec::new();
            for p in cur.paths() {
                let n = cur.at(&p);
                for ch in n.children() {
                    cands.push(cur.replaced(&p, ch.clone()));
                }
                if !p.is_empty() && !matches!(n, Expr::Port(_) | Expr::Lit(..)) {
                    let nat = (Model { ports: &ports })
                        .synth(n)
                        .ok()
                        .and_then(|t| t.ty.natural());
                    if let Some(nat) = nat {
                        let k = match ports.iter().position(|x| *x == nat) {
                            Some(k) => k,
                            None => {
                                ports.push(nat);
                                ports.len() - 1
                            }
                        };
                        cands.push(cur.replaced(&p, Expr::Port(k)));
                    }
                }
            }
            let model = Model { ports: &ports };
            cands.retain(|c| c.size() < cur.size() && model.synth(c).is_ok());
            cands.sort_by_key(Expr::size);
            cands.dedup();
            cands.truncate(80);
            if cands.is_empty() {
                break;
            }
            let mut vr = Rng::new(rng.next_u64());
            let mut vecs = vectors(&ports, self.cfg.vectors, &mut vr);
            if !f.vec.is_empty() {
                for (k, v) in f.vec.iter().enumerate() {
                    vecs[5][k] = *v;
                }
            }
            let mut cases: Vec<Case> = cands
                .iter()
                .enumerate()
                .map(|(i, e)| Case::new(i, e.clone(), &ports, &mut rng))
                .collect();
            for c in &mut cases {
                c.drop_x_contract(&ports, &vecs);
            }
            let batch = Batch {
                cases,
                ports: ports.clone(),
                vecs,
            };
            let out = self.run_batch(&batch, false);
            let hit = out
                .findings
                .iter()
                .filter(|g| g.class == f.class && g.variant == f.variant)
                .min_by_key(|g| g.expr.size());
            match hit {
                Some(g) => {
                    cur = g.expr.clone();
                    last = g.text.clone();
                }
                None => break,
            }
        }
        format!("  [{}] {}\n{}", f.class.name(), f.key, indent(&last))
    }
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("      {l}\n")).collect()
}

/// Kolaylık: çalışma dizininde koşu.
pub fn run(cfg: &Config, tools: &Tools, work: &Path) -> Report {
    Runner::new(cfg, tools, work).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_short_diagnostics() {
        let t = "dt.volt:17:10: error[E2001]: bit width mismatch\ndt.volt:3:1: warning[W4001]: unused\n";
        assert_eq!(volt_errors(t), vec![(17, "E2001".to_string())]);
    }

    #[test]
    fn parses_verilator_messages() {
        let t = "%Warning-WIDTHEXPAND: build/rtl/Dt.sv:12:20: Operator ADD expects 9 bits\n%Error: build/rtl/Dt.sv:30:5: syntax error, unexpected '['\n";
        let m = verilator_msgs(t);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].1, "WIDTHEXPAND");
        assert_eq!(m[0].3, 12);
        assert_eq!(m[1].0, "%Error");
        assert_eq!(m[1].3, 30);
    }
}
