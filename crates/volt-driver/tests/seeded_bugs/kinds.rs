//! Mutation kinds of the seeded-bug corpus (ADR-0113).
//!
//! A kind is a single-point change that creates a real hazard, applied
//! to every corpus design where it fits; a near-miss kind is a change
//! that looks like one but is safe. Each kind names the code the
//! specification gives for its hazard (`expect`). A kind whose answer
//! depends on the design (`expect: None`) has every one of its mutants
//! classified by hand in `HAND` (seeded_bug_tests.rs); a mutant the test
//! cannot classify fails the test. The compiler's output is never the
//! expected value.

use super::scan::{code_part, is_ident, is_ident_char, replace_ident, split_top_level, Source};

/// What a mutant is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    /// An unsafe design: one of these codes must newly appear.
    Hazard(Vec<&'static str>),
    /// A safe change: no new error and no new safety code, except the
    /// listed ones (a `declassify` near-miss adds its W3008, for example).
    NearMiss(Vec<&'static str>),
}

pub struct Kind {
    pub name: &'static str,
    /// The specification rule the expected code comes from.
    pub spec: &'static str,
    /// Expected codes for every mutant of the kind; `None`: classified per
    /// mutant in `HAND`.
    pub expect: Option<&'static [&'static str]>,
    pub near_miss: bool,
    pub sites: fn(&Source) -> Vec<(String, String)>,
}

/// `(site label, mutated text)` list of one kind on one design.
type Sites = Vec<(String, String)>;

pub const CDC_READ: &[&str] = &["E3001", "E3012"];

pub fn kinds() -> Vec<Kind> {
    vec![
        // ── Clock domains ─────────────────────────────────────────────
        Kind {
            name: "cdc.drop-sync",
            spec: "domain-inference.md K6/K5 (assignment or join of two domains: E3001); K7 inside an `on` block (E3001/E3012)",
            expect: Some(&["E3001"]),
            near_miss: false,
            sites: drop_sync,
        },
        Kind {
            name: "cdc.sync-to-other-clock",
            spec: "domain-inference.md K9 (sync output is in the destination clock's domain) + K6: the result lands in the wrong domain, E3001",
            expect: Some(&["E3001"]),
            near_miss: false,
            sites: sync_to_other_clock,
        },
        Kind {
            name: "cdc.on-clock-swap",
            spec: "domain-inference.md K4 (registers take the `on` block's domain) + K7/K6: reads of the old domain, E3001/E3012",
            expect: Some(CDC_READ),
            near_miss: false,
            sites: on_clock_swap,
        },
        Kind {
            name: "cdc.instance-clock-swap",
            spec: "domain-inference.md K8/K8a (the clock binding maps the child's domain; ports bound from the other domain are E3001)",
            expect: Some(CDC_READ),
            near_miss: false,
            sites: instance_clock_swap,
        },
        Kind {
            name: "cdc.port-domain-swap",
            spec: "domain-inference.md K1 (an annotation fixes the port's domain) + K6/K7: E3001/E3012",
            expect: Some(CDC_READ),
            near_miss: false,
            sites: port_domain_swap,
        },
        Kind {
            name: "cdc.drop-port-domain",
            spec: "domain-inference.md K3 and §3 step 2 (multi-clock module, port without annotation: E3010)",
            expect: Some(&["E3010"]),
            near_miss: false,
            sites: drop_port_domain,
        },
        Kind {
            name: "cdc.double-writer",
            spec: "domain-inference.md K4 (register written from two domains: E3011); ADR-0073 table d03b (the same case: E4001)",
            expect: Some(&["E3011", "E4001"]),
            near_miss: false,
            sites: double_writer,
        },
        Kind {
            name: "cdc.extern-domain-merge",
            spec: "domain-inference.md K8a (one symbolic domain bound to two clocks: E3014)",
            expect: None,
            near_miss: false,
            sites: extern_domain_merge,
        },
        // ── Reset domains ────────────────────────────────────────────
        Kind {
            name: "rdc.drop-raw-reset",
            spec: "ADR-0065 R5 (async reset shared by two clocks: E3003), R5' (sync: W3010), R1 (async at the unit root: W3009)",
            expect: None,
            near_miss: false,
            sites: drop_raw_reset,
        },
        Kind {
            name: "rdc.reset-polarity-flip",
            spec: "ADR-0065 §1 (raw port spec differs from the domain it feeds: E3003)",
            expect: Some(&["E3003"]),
            near_miss: false,
            sites: reset_polarity_flip,
        },
        Kind {
            name: "rdc.reset-kind-flip",
            spec: "ADR-0065 §1 (raw port spec differs from the domain it feeds: E3003)",
            expect: Some(&["E3003"]),
            near_miss: false,
            sites: reset_kind_flip,
        },
        Kind {
            name: "domain.edge-removed",
            spec: "ADR-0098 (a flop clocked by a domain without a clock edge: E3016); applied where an `on` block of the domain's clock exists",
            expect: Some(&["E3016"]),
            near_miss: false,
            sites: edge_removed,
        },
        // ── Trust levels ─────────────────────────────────────────────
        Kind {
            name: "trust.drop-declassify",
            spec: "domain-inference.md K11 / ADR-0052 §1-2 (a higher level flows to a lower one: E3009)",
            expect: Some(&["E3009"]),
            near_miss: false,
            sites: drop_declassify,
        },
        Kind {
            name: "trust.port-downgrade",
            spec: "domain-inference.md K11 / ADR-0052 §1-2 (output port moved to a lower trust level than its data: E3009)",
            expect: None,
            near_miss: false,
            sites: port_downgrade,
        },
        // ── Other safety checks ──────────────────────────────────────
        Kind {
            name: "bidir.drop-sync",
            spec: "domain-inference.md K2a (bidirectional read outside sync(): W3007)",
            expect: Some(&["W3007"]),
            near_miss: false,
            sites: bidir_drop_sync,
        },
        Kind {
            name: "handshake.valid-on-ready",
            spec: "ADR-0050 §4 (producer valid combinationally from ready: E4007)",
            expect: Some(&["E4007"]),
            near_miss: false,
            sites: valid_on_ready,
        },
        Kind {
            name: "mmio.bus-owned-write",
            spec: "ADR-0044 §2-3 (RTL writes a bus-owned register field: E4006)",
            expect: Some(&["E4006"]),
            near_miss: false,
            sites: mmio_bus_owned,
        },
        Kind {
            name: "timing.drop-delay",
            spec: "ADR-0037 (values of different stages combined under @strict_timing: E5010)",
            expect: Some(&["E5010"]),
            near_miss: false,
            sites: drop_delay,
        },
        Kind {
            name: "timing.drop-retiming",
            spec: "ADR-0037 (if/match arms of different stages without the retiming assertion: E5010)",
            expect: None,
            near_miss: false,
            sites: drop_retiming,
        },
        Kind {
            name: "drivers.duplicate-assign",
            spec: "type-inference.md §11.2 / ADR-0073 (a second driver of an output: E4001)",
            expect: Some(&["E4001"]),
            near_miss: false,
            sites: duplicate_assign,
        },
        // ── Near misses ──────────────────────────────────────────────
        Kind {
            name: "near.sync3-for-sync",
            spec: "domain-inference.md K9 (sync3 is a three-stage synchronizer: still a legal bridge)",
            expect: Some(&[]),
            near_miss: true,
            sites: sync3_for_sync,
        },
        Kind {
            name: "near.on-block-split",
            spec: "domain-inference.md K4 (writers in one domain: the register keeps it); ADR-0073 (one writer per register)",
            expect: Some(&[]),
            near_miss: true,
            sites: on_block_split,
        },
        Kind {
            name: "near.raw-reset-spec-dropped",
            spec: "ADR-0065 §1 (a raw port without a spec takes the spec of the domain it feeds)",
            expect: Some(&[]),
            near_miss: true,
            sites: raw_reset_spec_dropped,
        },
        Kind {
            name: "near.edge-flip",
            spec: "domain-inference.md K1/K2 (negedge is a clock edge like posedge)",
            expect: Some(&[]),
            near_miss: true,
            sites: edge_flip,
        },
        Kind {
            name: "near.ready-on-valid",
            spec: "ADR-0050 §4 (the consumer may derive ready from valid)",
            expect: Some(&[]),
            near_miss: true,
            sites: ready_on_valid,
        },
        Kind {
            name: "near.extern-domain-rename",
            spec: "domain-inference.md K8a (a symbolic domain is a parameter: its name does not matter)",
            expect: Some(&[]),
            near_miss: true,
            sites: extern_domain_rename,
        },
    ]
}

// ── helpers ──────────────────────────────────────────────────────────

/// Brace depth at the start of `line`.
fn depth_before(src: &Source, line: usize) -> i32 {
    let mut d = 0;
    for l in &src.lines[..line] {
        for c in code_part(l).chars() {
            match c {
                '{' => d += 1,
                '}' => d -= 1,
                _ => {}
            }
        }
    }
    d
}

fn splice(line: &str, start: usize, end: usize, new: &str) -> String {
    format!("{}{}{}", &line[..start], new, &line[end..])
}

fn paren(e: &str) -> String {
    if is_ident(e) {
        e.to_string()
    } else {
        format!("({e})")
    }
}

/// Numbers repeated labels: the second `x` becomes `x #2`.
fn number(sites: Sites) -> Sites {
    let mut seen: Vec<String> = Vec::new();
    sites
        .into_iter()
        .map(|(label, text)| {
            let n = seen.iter().filter(|s| **s == label).count();
            seen.push(label.clone());
            if n == 0 {
                (label, text)
            } else {
                (format!("{label} #{}", n + 1), text)
            }
        })
        .collect()
}

fn sync_calls(src: &Source) -> Vec<(super::scan::Call, &'static str)> {
    let mut v: Vec<_> = src.calls("sync").into_iter().map(|c| (c, "sync")).collect();
    v.extend(src.calls("sync3").into_iter().map(|c| (c, "sync3")));
    v.retain(|(c, _)| c.args.len() == 2);
    v.sort_by_key(|(c, _)| (c.line, c.start));
    v
}

fn is_bidir_read(arg: &str) -> bool {
    arg.ends_with(".read()")
}

fn call_text(src: &Source, c: &super::scan::Call) -> String {
    src.code(c.line)[c.start..c.end].to_string()
}

// ── clock domains ────────────────────────────────────────────────────

fn drop_sync(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (c, _) in sync_calls(src) {
        if is_bidir_read(&c.args[0]) {
            continue;
        }
        let line = &src.lines[c.line];
        let new = splice(line, c.start, c.end, &paren(&c.args[0]));
        out.push((call_text(src, &c), src.with_line(c.line, &new)));
    }
    number(out)
}

fn sync_to_other_clock(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (c, name) in sync_calls(src) {
        if is_bidir_read(&c.args[0]) {
            continue;
        }
        let Some(m) = src.module_at(c.line) else {
            continue;
        };
        let clock = c.args[1].trim();
        if m.port(clock).is_none_or(|p| !p.is_clock()) {
            continue;
        }
        let Some(other) = m.other_clock(clock) else {
            continue;
        };
        let line = &src.lines[c.line];
        let new = splice(
            line,
            c.start,
            c.end,
            &format!("{name}({}, {other})", c.args[0]),
        );
        out.push((
            format!("{} -> {other}", call_text(src, &c)),
            src.with_line(c.line, &new),
        ));
    }
    number(out)
}

fn on_clock_swap(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        for b in &m.on_blocks {
            let Some(other) = m.other_clock(&b.clock) else {
                continue;
            };
            let line = &src.lines[b.line];
            let new = line.replacen(&format!("on {}", b.clock), &format!("on {other}"), 1);
            out.push((
                format!("{}: on {} -> {other}", m.name, b.clock),
                src.with_line(b.line, &new),
            ));
        }
    }
    number(out)
}

fn instance_clock_swap(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        let clocks: Vec<String> = m.clocks().iter().map(|p| p.name.clone()).collect();
        if clocks.len() < 2 {
            continue;
        }
        let mut inst = String::new();
        for i in m.start + 1..m.end {
            let code = src.code(i);
            let t = code.trim();
            if t.starts_with("let ") && t.contains('{') {
                inst = t[4..].split('=').next().unwrap_or("").trim().to_string();
            }
            if t.starts_with("in ") || t.starts_with("out ") || t.starts_with("reg") {
                continue;
            }
            // `field: value` pairs whose value is a clock port.
            let mut from = 0;
            while let Some(pos) = code[from..].find(':') {
                let at = from + pos;
                from = at + 1;
                let field: String = code[..at]
                    .trim_end()
                    .chars()
                    .rev()
                    .take_while(|c| is_ident_char(*c))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let rest = code[at + 1..].trim_start();
                let value: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
                let after = rest[value.len()..].trim_start();
                if field.is_empty()
                    || !clocks.contains(&value)
                    || !(after.is_empty() || after.starts_with(',') || after.starts_with('}'))
                {
                    continue;
                }
                let other = m.other_clock(&value).expect("two clocks");
                let vstart = at + 1 + (code[at + 1..].len() - rest.len());
                let line = &src.lines[i];
                let new = splice(line, vstart, vstart + value.len(), &other);
                out.push((
                    format!("{}: {inst}.{field}: {value} -> {other}", m.name),
                    src.with_line(i, &new),
                ));
            }
        }
    }
    number(out)
}

/// Non-clock, non-reset ports annotated with a clock domain of a module
/// whose clock ports carry at least two domains, with the other domain.
fn domain_ports(src: &Source) -> Vec<(usize, String, String, String, String)> {
    let mut v = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        let mut doms: Vec<String> = m.clocks().iter().filter_map(|p| p.annot.clone()).collect();
        doms.dedup();
        if doms.len() < 2 {
            continue;
        }
        for p in &m.ports {
            if p.is_clock() || p.is_reset() {
                continue;
            }
            let Some(a) = &p.annot else { continue };
            if !doms.contains(a) {
                continue;
            }
            let other = doms.iter().find(|d| *d != a).unwrap().clone();
            v.push((p.line, m.name.clone(), p.name.clone(), a.clone(), other));
        }
    }
    v
}

fn port_domain_swap(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (line, m, p, a, other) in domain_ports(src) {
        let new = replace_ident(&src.lines[line], &format!("@{a}"), &format!("@{other}"))
            .or_else(|| Some(src.lines[line].replacen(&format!("@{a}"), &format!("@{other}"), 1)))
            .unwrap();
        out.push((
            format!("{m}.{p}: @{a} -> @{other}"),
            src.with_line(line, &new),
        ));
    }
    number(out)
}

fn drop_port_domain(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (line, m, p, a, _) in domain_ports(src) {
        let l = &src.lines[line];
        let at = l.find(&format!("@{a}")).expect("annotation");
        let new = format!("{}{}", l[..at].trim_end(), &l[at + a.len() + 1..]);
        out.push((format!("{m}.{p}: @{a} removed"), src.with_line(line, &new)));
    }
    number(out)
}

/// `lvalue` of a `lvalue <= expr` statement on this line, if any.
fn nb_target(code: &str) -> Option<String> {
    let at = code.find("<=")?;
    let mut left = code[..at].trim();
    if let Some(p) = left.rfind('{') {
        left = left[p + 1..].trim();
    }
    let ok = !left.is_empty()
        && left
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && left
            .chars()
            .all(|c| is_ident_char(c) || matches!(c, '.' | '[' | ']' | ':'));
    ok.then(|| left.to_string())
}

fn double_writer(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        for b in &m.on_blocks {
            let Some(other) = m.other_clock(&b.clock) else {
                continue;
            };
            let Some(target) = (b.line..=b.end).find_map(|i| nb_target(src.code(i))) else {
                continue;
            };
            let indent: String = src.lines[b.line]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            let new = format!("{indent}on {other} {{ {target} <= {target} }}");
            out.push((
                format!("{}: {target} also written on {other}", m.name),
                src.with_inserted(b.end, &new),
            ));
        }
    }
    number(out)
}

fn extern_domain_merge(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| m.is_extern) {
        let mut doms: Vec<String> = m.clocks().iter().filter_map(|p| p.annot.clone()).collect();
        doms.dedup();
        if doms.len() < 2 {
            continue;
        }
        let (keep, gone) = (&doms[0], &doms[1]);
        let mut lines = src.lines.clone();
        for line in lines.iter_mut().take(m.end).skip(m.start + 1) {
            if let Some(n) = replace_ident(line, &format!("@{gone}"), &format!("@{keep}")) {
                *line = n;
            } else if line.contains(&format!("@{gone}")) {
                *line = line.replace(&format!("@{gone}"), &format!("@{keep}"));
            }
        }
        out.push((
            format!("{}: @{gone} -> @{keep}", m.name),
            Source::join(&lines, src.trailing_newline),
        ));
    }
    number(out)
}

fn extern_domain_rename(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| m.is_extern) {
        let mut doms: Vec<String> = m.clocks().iter().filter_map(|p| p.annot.clone()).collect();
        doms.dedup();
        if doms.len() < 2 {
            continue;
        }
        let gone = &doms[1];
        let new_name = format!("{gone}Side");
        let mut lines = src.lines.clone();
        for line in lines.iter_mut().take(m.end).skip(m.start + 1) {
            *line = line.replace(&format!("@{gone}"), &format!("@{new_name}"));
        }
        out.push((
            format!("{}: @{gone} -> @{new_name}", m.name),
            Source::join(&lines, src.trailing_newline),
        ));
    }
    number(out)
}

// ── reset domains ────────────────────────────────────────────────────

fn raw_resets(src: &Source) -> Vec<(String, super::scan::Port)> {
    src.modules()
        .into_iter()
        .filter(|m| !m.is_extern)
        .flat_map(|m| {
            let name = m.name.clone();
            m.ports
                .into_iter()
                .filter(|p| p.is_reset() && p.dir == "in")
                .map(move |p| (name.clone(), p))
        })
        .collect()
}

fn drop_raw_reset(src: &Source) -> Sites {
    number(
        raw_resets(src)
            .into_iter()
            .map(|(m, p)| (format!("{m}.{}", p.name), src.without_line(p.line)))
            .collect(),
    )
}

fn reset_spec_edit(src: &Source, f: fn(&str, &str) -> Option<(String, String)>) -> Sites {
    let mut out = Vec::new();
    for (m, p) in raw_resets(src) {
        let line = &src.lines[p.line];
        let Some(open) = line.find("reset(") else {
            continue;
        };
        let close = open + line[open..].find(')').expect("closing parenthesis");
        let args = split_top_level(&line[open + 6..close]);
        if args.len() != 2 {
            continue;
        }
        let Some((new_args, what)) = f(&args[0], &args[1]) else {
            continue;
        };
        let new = splice(line, open, close + 1, &new_args);
        out.push((
            format!("{m}.{}: {what}", p.name),
            src.with_line(p.line, &new),
        ));
    }
    number(out)
}

fn reset_polarity_flip(src: &Source) -> Sites {
    reset_spec_edit(src, |s, p| {
        let q = if p == "active_low" {
            "active_high"
        } else {
            "active_low"
        };
        Some((format!("reset({s}, {q})"), format!("{p} -> {q}")))
    })
}

fn reset_kind_flip(src: &Source) -> Sites {
    reset_spec_edit(src, |s, p| {
        let t = if s == "async" { "sync" } else { "async" };
        Some((format!("reset({t}, {p})"), format!("{s} -> {t}")))
    })
}

fn raw_reset_spec_dropped(src: &Source) -> Sites {
    reset_spec_edit(src, |s, p| {
        Some(("reset".to_string(), format!("reset({s}, {p}) -> reset")))
    })
}

/// `(line, domain name, edge)` of every `clock = posedge|negedge` field.
fn domain_edges(src: &Source) -> Vec<(usize, String, String)> {
    let mut v = Vec::new();
    let mut current: Option<String> = None;
    for (i, raw) in src.lines.iter().enumerate() {
        let t = code_part(raw).trim();
        let words: Vec<&str> = t.split_whitespace().collect();
        if let Some(at) = words.iter().position(|w| *w == "domain") {
            if words[..at].iter().all(|w| *w == "pub") && at + 1 < words.len() {
                current = Some(words[at + 1].trim_end_matches('{').to_string());
            }
        }
        if let Some(d) = &current {
            for edge in ["posedge", "negedge"] {
                if t.contains("clock") && t.contains(edge) {
                    v.push((i, d.clone(), edge.to_string()));
                }
            }
        }
        if t.contains('}') {
            current = None;
        }
    }
    v
}

/// Domains whose clock needs an edge in this file (ADR-0098 Ek 2): a
/// clock port annotated with the domain drives an `on` block, is a
/// `sync()` destination, or is bound to a builtin or Volt child module.
/// A binding to an `extern` module does not count: ADR-0098 leaves
/// externs out.
fn clocked_domains(src: &Source) -> Vec<String> {
    let modules = src.modules();
    let externs: Vec<&str> = modules
        .iter()
        .filter(|m| m.is_extern)
        .map(|m| m.name.as_str())
        .collect();
    let syncs = sync_calls(src);
    let mut v = Vec::new();
    for m in modules.iter().filter(|m| !m.is_extern) {
        for p in m.clocks() {
            let Some(d) = &p.annot else { continue };
            let on = m.on_blocks.iter().any(|b| b.clock == p.name);
            let sync = syncs
                .iter()
                .any(|(c, _)| m.contains(c.line) && c.args[1].trim() == p.name);
            let mut bound = false;
            let mut ty = String::new();
            for i in m.start + 1..m.end {
                let t = src.code(i).trim();
                if let Some(rest) = t.strip_prefix("let ") {
                    if let Some((_, rhs)) = rest.split_once('=') {
                        ty = rhs
                            .trim()
                            .chars()
                            .take_while(|c| is_ident_char(*c))
                            .collect();
                    }
                }
                let binds = t.split([',', '{', '}']).any(|b| {
                    b.split_once(':')
                        .is_some_and(|(f, v)| is_ident(f.trim()) && v.trim() == p.name)
                });
                if binds && !externs.contains(&ty.as_str()) {
                    bound = true;
                }
            }
            if on || sync || bound {
                v.push(d.clone());
            }
        }
    }
    v
}

fn edge_removed(src: &Source) -> Sites {
    let clocked = clocked_domains(src);
    number(
        domain_edges(src)
            .into_iter()
            .filter(|(_, d, _)| clocked.contains(d))
            .map(|(i, d, e)| {
                let new = src.lines[i].replacen(&e, "none", 1);
                (format!("domain {d}: {e} -> none"), src.with_line(i, &new))
            })
            .collect(),
    )
}

fn edge_flip(src: &Source) -> Sites {
    number(
        domain_edges(src)
            .into_iter()
            .map(|(i, d, e)| {
                let f = if e == "posedge" { "negedge" } else { "posedge" };
                let new = src.lines[i].replacen(&e, f, 1);
                (format!("domain {d}: {e} -> {f}"), src.with_line(i, &new))
            })
            .collect(),
    )
}

// ── trust levels ─────────────────────────────────────────────────────

fn drop_declassify(src: &Source) -> Sites {
    let mut out = Vec::new();
    for c in src.calls("declassify") {
        if c.args.len() != 2 {
            continue;
        }
        let line = &src.lines[c.line];
        let new = splice(line, c.start, c.end, &paren(&c.args[0]));
        out.push((
            format!("declassify({}, ..)", c.args[0]),
            src.with_line(c.line, &new),
        ));
    }
    number(out)
}

fn trust_levels(src: &Source) -> Vec<(String, u8)> {
    let mut v = Vec::new();
    let mut current: Option<String> = None;
    for raw in &src.lines {
        let t = code_part(raw).trim();
        let words: Vec<&str> = t.split_whitespace().collect();
        if let Some(at) = words.iter().position(|w| *w == "domain") {
            if words[..at].iter().all(|w| *w == "pub") && at + 1 < words.len() {
                current = Some(words[at + 1].trim_end_matches('{').to_string());
            }
        }
        if let (Some(d), true) = (&current, t.contains("trust_level")) {
            let level = if t.contains("secret") {
                2
            } else if t.contains("confidential") {
                1
            } else {
                0
            };
            v.push((d.clone(), level));
        }
        if t.contains('}') {
            current = None;
        }
    }
    v
}

fn port_downgrade(src: &Source) -> Sites {
    let levels = trust_levels(src);
    let Some((low, low_level)) = levels.iter().min_by_key(|(_, l)| *l).cloned() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        for p in m.ports.iter().filter(|p| p.dir == "out") {
            let Some(a) = &p.annot else { continue };
            let Some((_, l)) = levels.iter().find(|(d, _)| d == a) else {
                continue;
            };
            if *l <= low_level {
                continue;
            }
            let new = src.lines[p.line].replacen(&format!("@{a}"), &format!("@{low}"), 1);
            out.push((
                format!("{}.{}: @{a} -> @{low}", m.name, p.name),
                src.with_line(p.line, &new),
            ));
        }
    }
    number(out)
}

// ── other safety checks ──────────────────────────────────────────────

fn bidir_drop_sync(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (c, _) in sync_calls(src) {
        if !is_bidir_read(&c.args[0]) {
            continue;
        }
        let line = &src.lines[c.line];
        let new = splice(line, c.start, c.end, &c.args[0]);
        out.push((call_text(src, &c), src.with_line(c.line, &new)));
    }
    number(out)
}

/// Handshake ports of a module that the protocol checks see: `(module,
/// port, dir)`; a port or module with `@no_protocol_check` is left out.
fn handshake_ports(src: &Source) -> Vec<(super::scan::Module, String, String)> {
    let mut v = Vec::new();
    for m in src.modules().into_iter().filter(|m| !m.is_extern) {
        let module_off = m.start > 0 && src.code(m.start - 1).contains("@no_protocol_check");
        if module_off {
            continue;
        }
        let ports: Vec<(String, String)> = m
            .ports
            .iter()
            .filter(|p| p.ty.starts_with("Handshake<"))
            .filter(|p| !(p.line > 0 && src.code(p.line - 1).contains("@no_protocol_check")))
            .map(|p| (p.name.clone(), p.dir.clone()))
            .collect();
        for (n, d) in ports {
            v.push((m.clone(), n, d));
        }
    }
    v
}

fn field_assign(src: &Source, m: &super::scan::Module, port: &str, field: &str) -> Option<usize> {
    (m.start + 1..m.end).find(|&i| {
        let t = src.code(i).trim();
        depth_before(src, i) == depth_before(src, m.start) + 1
            && t.strip_prefix(&format!("{port}.{field}")).is_some_and(|r| {
                r.trim_start().starts_with('=') && !r.trim_start().starts_with("==")
            })
    })
}

fn guard_field(src: &Source, dir: &str, field: &str, other: &str) -> Sites {
    let mut out = Vec::new();
    for (m, port, d) in handshake_ports(src) {
        if d != dir {
            continue;
        }
        let Some(i) = field_assign(src, &m, &port, field) else {
            continue;
        };
        let line = &src.lines[i];
        let eq = line.find('=').expect("assignment");
        let code_end = eq + code_part(&line[eq..]).trim_end().len();
        let rhs = line[eq + 1..code_end].trim();
        let new = format!(
            "{}= ({rhs}) && {port}.{other}{}",
            &line[..eq],
            &line[code_end..]
        );
        out.push((
            format!("{}: {port}.{field} uses {port}.{other}", m.name),
            src.with_line(i, &new),
        ));
    }
    number(out)
}

fn valid_on_ready(src: &Source) -> Sites {
    guard_field(src, "out", "valid", "ready")
}

fn ready_on_valid(src: &Source) -> Sites {
    guard_field(src, "in", "ready", "valid")
}

fn mmio_bus_owned(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (i, raw) in src.lines.iter().enumerate() {
        let code = code_part(raw);
        if !(code.contains("@reg(") && code.contains("access = ReadOnly")) {
            continue;
        }
        let Some(name_line) = (i + 1..src.lines.len()).find(|&j| !src.code(j).trim().is_empty())
        else {
            continue;
        };
        let reg: String = src
            .code(name_line)
            .trim()
            .chars()
            .take_while(|c| is_ident_char(*c))
            .collect();
        // Only a register that RTL writes: `regs.<reg>.<field> <=`.
        let written = src.lines.iter().any(|l| {
            let c = code_part(l);
            c.contains(&format!("regs.{reg}."))
                && nb_target(c).is_some_and(|t| t.starts_with(&format!("regs.{reg}.")))
        });
        if !written {
            continue;
        }
        let mut lines = src.lines.clone();
        lines[i] = lines[i]
            .replace("access = ReadOnly, volatile", "access = ReadWrite")
            .replace("access = ReadOnly", "access = ReadWrite");
        // `@w1c` belongs to volatile registers only (ADR-0044 §2).
        for line in lines.iter_mut().skip(name_line + 1) {
            if code_part(line).contains('}') {
                break;
            }
            *line = line.replace(" @w1c", "");
        }
        out.push((
            format!("reg {reg}: ReadOnly -> ReadWrite"),
            Source::join(&lines, src.trailing_newline),
        ));
    }
    number(out)
}

fn drop_delay(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (i, raw) in src.lines.iter().enumerate() {
        let code = code_part(raw);
        let Some(at) = code.find("delay<") else {
            continue;
        };
        if code[..at].chars().next_back().is_some_and(is_ident_char) {
            continue;
        }
        let Some(gt) = code[at..].find(">(").map(|g| at + g + 1) else {
            continue;
        };
        let Some(close) = super::scan::matching_paren(code, gt) else {
            continue;
        };
        let arg = &code[gt + 1..close];
        let new = splice(raw, at, close + 1, &paren(arg));
        out.push((code[at..close + 1].to_string(), src.with_line(i, &new)));
    }
    number(out)
}

fn drop_retiming(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (i, raw) in src.lines.iter().enumerate() {
        let t = code_part(raw).trim();
        let Some(rest) = t.strip_prefix("let ") else {
            continue;
        };
        let Some((name, ty)) = rest.split_once(':') else {
            continue;
        };
        if !ty.trim_start().starts_with("Delayed<") {
            continue;
        }
        let colon = raw.find(':').expect("colon");
        let eq = raw[colon..]
            .find('=')
            .map(|e| colon + e)
            .expect("initializer");
        let new = format!("{} {}", raw[..colon].trim_end(), &raw[eq..]);
        out.push((
            format!("let {}: Delayed annotation removed", name.trim()),
            src.with_line(i, &new),
        ));
    }
    number(out)
}

fn duplicate_assign(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        let base = depth_before(src, m.start) + 1;
        let found = (m.start + 1..m.end).find(|&i| {
            let code = src.code(i);
            let t = code.trim();
            let name: String = t.chars().take_while(|c| is_ident_char(*c)).collect();
            let rest = t[name.len()..].trim_start();
            depth_before(src, i) == base
                && m.port(&name).is_some_and(|p| p.dir == "out")
                && rest.starts_with('=')
                && !rest.starts_with("==")
                && !code.contains('{')
                && !code.contains('}')
                && code.matches('(').count() == code.matches(')').count()
        });
        if let Some(i) = found {
            let name: String = src
                .code(i)
                .trim()
                .chars()
                .take_while(|c| is_ident_char(*c))
                .collect();
            out.push((
                format!("{}: second driver of {name}", m.name),
                src.with_inserted(i, &src.lines[i].clone()),
            ));
        }
    }
    number(out)
}

// ── near misses ──────────────────────────────────────────────────────

fn sync3_for_sync(src: &Source) -> Sites {
    let mut out = Vec::new();
    for (c, name) in sync_calls(src) {
        if name != "sync" {
            continue;
        }
        let line = &src.lines[c.line];
        let new = splice(line, c.start, c.start + 4, "sync3");
        out.push((call_text(src, &c), src.with_line(c.line, &new)));
    }
    number(out)
}

fn on_block_split(src: &Source) -> Sites {
    let mut out = Vec::new();
    for m in src.modules().iter().filter(|m| !m.is_extern) {
        if m.clocks().len() < 2 {
            continue;
        }
        for b in &m.on_blocks {
            if b.end == b.line {
                continue;
            }
            let inner = depth_before(src, b.line) + 1;
            let stmts: Vec<usize> = (b.line + 1..b.end)
                .filter(|&i| depth_before(src, i) == inner)
                .filter(|&i| {
                    let c = src.code(i);
                    nb_target(c).is_some() && !c.contains('{') && !c.contains('}')
                })
                .collect();
            if stmts.len() < 2 {
                continue;
            }
            let moved = *stmts.last().unwrap();
            let stmt = src.code(moved).trim().to_string();
            let indent: String = src.lines[b.line]
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            let mut lines = src.lines.clone();
            lines.insert(b.end + 1, format!("{indent}on {} {{ {stmt} }}", b.clock));
            lines.remove(moved);
            let target = nb_target(&stmt).unwrap();
            out.push((
                format!(
                    "{}: {target} moved to a second `on {}` block",
                    m.name, b.clock
                ),
                Source::join(&lines, src.trailing_newline),
            ));
        }
    }
    number(out)
}
