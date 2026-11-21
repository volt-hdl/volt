#!/usr/bin/env python3
"""Merge the shards of a cargo-mutants run (ADR-0109) into one summary.

The tools job of .github/workflows/mutants.yml uploads `mutants-list`:
`shard-<k>.txt` holds the mutants of shard k, as `cargo mutants --list
--shard k/N` prints them. Each shard uploads `mutants-out-<k>` with
cargo-mutants' own `mutants.out/` (`caught.txt`, `missed.txt`,
`timeout.txt`, `unviable.txt`: one mutant name per line) and the memory
watchdog's `memory-kills.tsv` (mutants_watchdog.py, #160); the upload runs
even when the shard was stopped, so a stopped shard still brings the
outcomes it reached. With one shard, download-artifact extracts the one
artifact straight into the download directory (#179); both layouts are read.

This script counts them, lists the missed mutants by file (file:line,
change), writes the merged lists and a Markdown summary, and fails when a
shard is not completed: no artifact, or fewer outcomes than the mutants it
was given. A shard that is not completed is named at the top of the
summary with the mutants it did not test, never left out.

The baseline is one job of the run, beside the shards (ADR-0128 §2): its
artifact `mutants-baseline` (`--baseline DIR`) brings the peak memory of
its build and tests (the watchdog's `--peak` files,
`baseline-build-peak.txt` and `baseline-test-peak.txt`, #253) and its
stamp: the seconds of its tests and the test timeout the rule gives,
max(2 x baseline, floor) (ADR-0128 §3). The summary shows both; a peak
above half of the watchdog's limit (`--memory-limit-mib`) is a warning.
`--baseline-result` is that job's result: anything but `success` fails
the run and gives no mutant verdict. Each shard's stamp names the test
timeout it used (the floor: a shard does not wait for the baseline); a
shard that used less than the rule gives fails on each of its timeout
mutants (the outcome is not decided) and warns otherwise (no other
outcome depends on the timeout: a caught mutant failed before it, a
missed one passed within it). A mutant whose
test process the watchdog killed at the memory limit is its own class,
"memory limit": caught, as a timeout is (ADR-0109 section 4).

A full or targeted run measures the tests: a missed mutant does not fail
it. The pull-request gate (`--gate`, ADR-0109 appendix "PR kapısı") fails
on every missed mutant that no `exclude_re` entry of the configuration
covers; the summary lists them with the classification rules. `--shards 0`
is a gate run with no mutant to test (the reason is `--note`): it passes.

    mutants_summary.py SHARDS_DIR --lists LISTS_DIR --shards N [--out DIR] [--markdown FILE]
                       [--gate --config .cargo/mutants.toml] [--note TEXT] [--memory-limit-mib N]
    mutants_summary.py --self-test
"""

import argparse
import contextlib
import io
import json
import re
import sys
import tempfile
import tomllib
from collections import defaultdict
from pathlib import Path

import mutants_exclusions  # scripts/ci, next to this script

OUTCOMES = ("caught", "missed", "timeout", "unviable")
# `memory` is not a cargo-mutants outcome: the watchdog's kills, taken out
# of `caught` (or `timeout`).
KINDS = OUTCOMES + ("memory",)
NAME = re.compile(r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): (?P<change>.+)$")
# The two watchdog runs of the baseline job (#253): file part -> wording.
PEAK_PARTS = {"build": "baseline build", "test": "baseline tests"}
# The baseline job's artifact (ADR-0128 §2).
BASELINE_ARTIFACT = "mutants-baseline"
# `cargo mutants --list` colors its output when CARGO_TERM_COLOR=always.
ANSI = re.compile(r"\x1b\[[0-9;]*m")

RULES = [
    "Classify each one (ADR-0109 section 4) and fix it in this pull request:",
    "",
    "- **B, blind spot**: the change alters behavior and no test notices. Write the test that kills it:"
    " apply the mutant by hand in a throwaway worktree, see the test fail, restore the file.",
    "- **A1, equivalent within the function** whatever calls it, or **A2, equivalent given an assumption**"
    " (today's callers, the text the emitter writes): an `exclude_re` entry in `.cargo/mutants.toml` with"
    " its reason, its proof (unreachable, or the same lint and simulated value; for A2 a permanent test of"
    " the assumption) and an `# expect:` line per mutant it is written for.",
    "- **C, suspect**: the mutant suggests that today's behavior contradicts the specification. Open an issue"
    " with \"silent-wrong?\" in its title; the compiler is not fixed in this pull request. Add an `exclude_re`"
    " entry whose reason opens with `C.`, with an `# issue: #N` line naming that issue and its `# expect:`"
    " lines (#283): the gate passes the mutant while the issue is open; the exclusion check fails when the"
    " issue closes, or when a pull request closes it and the entry still stands.",
]


def read_lines(path: Path) -> list[str]:
    if not path.is_file():
        return []
    text = ANSI.sub("", path.read_text(encoding="utf-8"))
    return [l.strip() for l in text.splitlines() if l.strip()]


def shard_dir(root: Path, k: int, shards: int) -> Path | None:
    d = root / f"mutants-out-{k}"
    if d.is_dir():
        return d
    # #179: one artifact is extracted without its own directory.
    if shards == 1 and ((root / "mutants.out").is_dir() or (root / "mutants-shard.txt").is_file()):
        return root
    return None


def memory_kills(d: Path) -> list[str]:
    """The mutants whose test process the watchdog killed (field 5 of its log)."""
    path = d / "memory-kills.tsv"
    if not path.is_file():
        return []
    names = []
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t")
        if len(fields) >= 5 and fields[4] not in names:
            names.append(fields[4])
    return names


def baseline_peak(d: Path, part: str) -> int | None:
    """The watchdog's peak (MiB) of the baseline's build or tests; None if not recorded."""
    path = d / f"baseline-{part}-peak.txt"
    try:
        return int(path.read_text(encoding="utf-8").strip())
    except (OSError, ValueError):
        return None


def read_stamp(d: Path | None) -> dict:
    path = d / "stamp.json" if d is not None else None
    if path is None or not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def baseline_facts(d: Path | None) -> dict:
    """The baseline job's artifact (ADR-0128 §2): its seconds, the rule's
    test timeout (§3) and the peak memory of its build and tests (#253)."""
    present = d is not None and d.is_dir()
    st = read_stamp(d) if present else {}
    return {
        "present": present,
        "seconds": st.get("baseline_seconds"),
        "timeout": st.get("timeout"),
        "peak": {part: baseline_peak(d, part) if present else None for part in PEAK_PARTS},
    }


def memory_warnings(base: dict, limit_mib: int) -> list[str]:
    """#253: the baseline peaked above half of the watchdog's limit.

    A mutant over the limit counts as caught (memory limit, #160); a baseline
    near the limit leaves a mutant little room, so a small increase of
    memory makes it "caught" and can hide a blind spot.
    """
    out = []
    if not base["present"]:
        return out
    for part in PEAK_PARTS:
        mib = base["peak"][part]
        what = PEAK_PARTS[part]
        if mib is None:
            out.append(f"the {what}'s peak was not recorded")
        elif mib * 2 > limit_mib:
            out.append(
                f"the {what} peaked at {mib} MiB, above half of the watchdog's limit"
                f" ({limit_mib} MiB); a mutant that adds a little memory is reported as memory limit (caught)"
                " and can hide a blind spot"
            )
    return out


def timeout_check(results: list[dict], base: dict) -> tuple[list[str], list[str]]:
    """(errors, warnings) of ADR-0128 §3: each shard's --timeout against the rule.

    The rule's timeout is max(2 x baseline, floor), from the baseline job's
    stamp; a shard used the floor, before the baseline was measured. A
    shard below the rule decided nothing about its timeout mutants (error);
    with none, no outcome of it depends on the timeout (warning).
    """
    errors, warnings = [], []
    rule = base.get("timeout")
    if not base.get("present"):
        return errors, warnings
    if rule is None:
        warnings.append("the baseline job's stamp holds no test timeout: the shards' --timeout is not checked")
        return errors, warnings
    for r in results:
        used = r.get("used_timeout")
        if used is None:
            if r["present"]:
                warnings.append(f"shard {r['shard']}: its stamp holds no --timeout: not checked against the rule's {rule} s")
            continue
        if used >= rule:
            continue
        why = (f"shard {r['shard']}: --timeout {used} s, below the rule's {rule} s"
               f" (max(2 x baseline {base.get('seconds')} s, floor), ADR-0128 §3)")
        if r["timeout"]:
            errors.append(f"{why}; its {len(r['timeout'])} timeout mutant(s) are not decided: "
                          + ", ".join(r["timeout"]))
        else:
            warnings.append(f"{why}; no mutant of it timed out, so no outcome depends on it")
    return errors, warnings


def baseline_cell(v) -> str:
    return "not recorded" if v is None else str(v)


def shard_result(root: Path, lists: Path, k: int, shards: int = 0) -> dict:
    d = shard_dir(root, k, shards)
    res = {"shard": k, "present": d is not None, "expected": read_lines(lists / f"shard-{k}.txt")}
    for kind in OUTCOMES:
        res[kind] = read_lines(d / "mutants.out" / f"{kind}.txt") if d else []
    # A mutant during which the watchdog killed a process is "memory limit"
    # whatever cargo-mutants made of it: caught or timeout (the test died),
    # missed (the killed process was not the test's verdict) or unviable
    # (the build died). Like a timeout it counts as caught and never fails
    # the gate; the summary lists it.
    killed = set(memory_kills(d)) if d else set()
    res["memory"] = [n for kind in OUTCOMES for n in res[kind] if n in killed]
    for kind in OUTCOMES:
        res[kind] = [n for n in res[kind] if n not in killed]
    seen = {n for kind in KINDS for n in res[kind]}
    res["untested"] = [n for n in res["expected"] if n not in seen]
    # ADR-0128 §3: the test timeout the shard used (its stamp).
    res["used_timeout"] = read_stamp(d).get("timeout")
    return res


def tested(res: dict) -> int:
    return sum(len(res[k]) for k in KINDS)


def complete(res: dict) -> bool:
    return res["present"] and len(res["expected"]) > 0 and not res["untested"]


def status(res: dict) -> str:
    if complete(res):
        return "completed"
    if not res["expected"]:
        return "**NOT COMPLETED**: no mutant list for this shard"
    if not res["present"]:
        return f"**NOT COMPLETED**: no artifact, {len(res['expected'])} not tested"
    return f"**NOT COMPLETED**: {len(res['untested'])} not tested"


def by_file(names: list[str]) -> dict[str, list[tuple[int, str]]]:
    groups: dict[str, list[tuple[int, str]]] = defaultdict(list)
    for n in names:
        m = NAME.match(n)
        if m:
            groups[m["file"]].append((int(m["line"]), m["change"]))
        else:
            groups["(unparsed)"].append((0, n))
    return {f: sorted(v) for f, v in sorted(groups.items())}


def exclusions(config: Path) -> list[str]:
    """The `exclude_re` entries of the cargo-mutants configuration."""
    with config.open("rb") as f:
        return list(tomllib.load(f).get("exclude_re", []))


def c_waiting(config: Path, names: list[str]) -> list[tuple[str, int]]:
    """#283: (mutant, issue) for each of `names` that a C entry excludes."""
    out = []
    for e in mutants_exclusions.parse(config.read_text(encoding="utf-8")):
        n = mutants_exclusions.issue_number(e)
        if n is None:
            continue
        rx = re.compile(e["regex"])
        out += [(name, n) for name in names if rx.search(name)]
    return sorted(out)


def gate_split(missed: list[str], patterns: list[str]) -> tuple[list[str], list[str]]:
    """(missed mutants no entry covers, missed mutants an entry covers).

    cargo-mutants 27.1.0 does not apply exclude_re to field-deletion mutants
    (ADR-0109 "Hedefli koşu"), so the gate applies it again, as cargo-mutants
    matches: Python re.search over the name.
    """
    rx = [re.compile(p) for p in patterns]
    open_, covered = [], []
    for n in missed:
        (covered if any(r.search(n) for r in rx) else open_).append(n)
    return open_, covered


def file_list(out: list[str], names: list[str]) -> None:
    for f, items in by_file(names).items():
        out += [f"**`{f}`** ({len(items)})", ""]
        out += [f"- `{f}:{line}`: {change}" for line, change in items]
        out.append("")


def c_list(out: list[str], gate: dict) -> None:
    """#283: the mutants on the changed lines that a C entry excludes, with its issue."""
    if gate.get("c"):
        out += [
            f"Excluded as C, waiting for their issue ({len(gate['c'])}): the gate tries them again when the issue"
            " closes and the entry is removed",
            "",
        ]
        out += [f"- {name}: #{n}" for name, n in gate["c"]] + [""]


def markdown(
    results: list[dict],
    gate: dict | None = None,
    note: str | None = None,
    memory_limit: int | None = None,
    base: dict | None = None,
    baseline_status: str | None = None,
) -> str:
    tot = {k: sum(len(r[k]) for r in results) for k in KINDS}
    listed = sum(len(r["expected"]) for r in results)
    done = sum(tested(r) for r in results)
    untested = sum(len(r["untested"]) for r in results)
    incomplete = [r for r in results if not complete(r)]
    out = ["## Mutation test (ADR-0109)", ""]
    if not results:
        out += [f"**No mutant to test.** {note or ''}".rstrip(), ""]
        if gate is not None:
            out += ["Pull-request gate: **passed** (nothing to test).", ""]
            c_list(out, gate)
        return "\n".join(out)
    if note:
        out += [note, ""]
    no_verdict = baseline_status not in (None, "success")
    if no_verdict:
        out += [
            f"### Mutation test: FAILED, the baseline job did not pass (`{baseline_status}`): no mutant verdict",
            "",
            "Every mutant is tested against the baseline's tree, tools and tests (ADR-0128 §2): with no passed"
            " baseline, an outcome below may come from the tree itself (a failing test, a missing tool), not from"
            " its mutant. The tables are for diagnosis only.",
            "",
        ]
        gate = None
    if incomplete:
        out += [
            f"> **NOT COMPLETED: {len(incomplete)} of {len(results)} shard(s)** "
            f"({', '.join(str(r['shard']) for r in incomplete)}); {untested} mutant(s) not tested. "
            "The counts below cover the tested mutants alone.",
            "",
        ]
    if gate is not None:
        if gate["open"]:
            out += [
                f"### Pull-request gate: FAILED, {len(gate['open'])} missed mutant(s) not covered by `exclude_re`",
                "",
            ]
            file_list(out, gate["open"])
            out += RULES + [""]
        elif incomplete:
            out += ["### Pull-request gate: FAILED, a shard did not complete", ""]
        else:
            out += ["### Pull-request gate: passed, every mutant on the changed lines is caught or excluded", ""]
        if gate["covered"]:
            out += [f"Missed and covered by `exclude_re` (cargo-mutants does not filter field deletions): {len(gate['covered'])}", ""]
            out += [f"- {n}" for n in gate["covered"]] + [""]
        c_list(out, gate)
    out += [
        "| Mutants | Tested | Not tested | Caught | Missed | Timeout | Memory limit | Unviable |",
        "|---:|---:|---:|---:|---:|---:|---:|---:|",
        f"| {listed} | {done} | {untested} | {tot['caught']} | {tot['missed']} | {tot['timeout']} "
        f"| {tot['memory']} | {tot['unviable']} |",
        "",
        "| Shard | Mutants | Tested | Caught | Missed | Timeout | Memory limit | Unviable | Status | Test timeout, s |",
        "|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|",
    ]
    for r in results:
        out.append(
            f"| {r['shard']} | {len(r['expected'])} | {tested(r)} | {len(r['caught'])} | {len(r['missed'])} "
            f"| {len(r['timeout'])} | {len(r['memory'])} | {len(r['unviable'])} | {status(r)} "
            f"| {baseline_cell(r.get('used_timeout'))} |"
        )
    out.append("")
    if base is not None:
        out += [
            "| Baseline (one job, ADR-0128 §2) | Tests, s | Test timeout by the rule, s | Peak, MiB (build / tests) |",
            "|---|---:|---:|---|",
            f"| {'passed' if baseline_status in (None, 'success') else baseline_status}"
            f"{'' if base['present'] else ', no artifact'} | {baseline_cell(base['seconds'])} "
            f"| {baseline_cell(base['timeout'])} "
            f"| {' / '.join(baseline_cell(base['peak'][p]) for p in PEAK_PARTS)} |",
            "",
        ]
        errors, warnings = timeout_check(results, base)
        out += [f"> **Test timeout.** {w}" for w in errors + warnings]
        if errors or warnings:
            out.append("")
        if memory_limit is not None:
            out += [f"Watchdog limit {memory_limit} MiB; a warning above half of it ({memory_limit // 2} MiB).", ""]
            out += [f"> **Baseline memory.** {w}" for w in memory_warnings(base, memory_limit)]
            out.append("")
    for kind, title in (
        ("missed", "Missed"),
        ("timeout", "Timeout"),
        ("memory", "Memory limit (a test process killed by the watchdog; caught, as a timeout is)"),
    ):
        names = sorted(n for r in results for n in r[kind])
        out += [f"### {title} ({len(names)})", ""]
        file_list(out, names)
    for r in incomplete:
        if r["untested"]:
            out += [f"<details><summary>Shard {r['shard']}: not tested ({len(r['untested'])})</summary>", ""]
            out += [f"- {n}" for n in r["untested"]]
            out += ["", "</details>", ""]
    unviable = sorted(n for r in results for n in r["unviable"])
    out += [f"<details><summary>Unviable ({len(unviable)}): the mutation does not build</summary>", ""]
    out += [f"- {n}" for n in unviable]
    out += ["", "</details>", ""]
    return "\n".join(out)


def summarize(
    root: Path,
    lists: Path,
    shards: int,
    out_dir: Path | None,
    md_path: Path | None,
    config: Path | None = None,
    note: str | None = None,
    memory_limit: int | None = None,
    baseline: Path | None = None,
    baseline_status: str | None = None,
) -> int:
    results = [shard_result(root, lists, k, shards) for k in range(shards)]
    base = baseline_facts(baseline) if (baseline is not None or baseline_status is not None) and results else None
    gate = None
    if config is not None:
        missed = sorted(n for r in results for n in r["missed"])
        open_, covered = gate_split(missed, exclusions(config))
        unexcluded = read_lines(lists / "unexcluded.txt")
        gate = {"open": open_, "covered": covered, "c": c_waiting(config, unexcluded)}
    md = markdown(results, gate, note, memory_limit, base, baseline_status if results else None)
    print(md)
    if md_path:
        with md_path.open("a", encoding="utf-8") as f:
            f.write(md + "\n")
    if out_dir:
        out_dir.mkdir(parents=True, exist_ok=True)
        for kind in KINDS + ("untested",):
            names = sorted(n for r in results for n in r[kind])
            (out_dir / f"{kind}.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")
        (out_dir / "summary.md").write_text(md + "\n", encoding="utf-8")
    code = 0
    if base is not None:
        if baseline_status not in (None, "success"):
            print(f"::error title=Mutation test::the baseline job did not pass ({baseline_status}): no mutant verdict")
            code = 1
        elif not base["present"]:
            print("::error title=Mutation test::no artifact of the baseline job (mutants-baseline)")
            code = 1
        errors, warnings = timeout_check(results, base)
        for e in errors:
            print(f"::error title=Mutation test timeout::{e}")
            code = 1
        for w in warnings:
            print(f"::warning title=Mutation test timeout::{w}")
        if memory_limit is not None:
            for w in memory_warnings(base, memory_limit):
                print(f"::warning title=Mutation baseline memory::{w}")
    for r in results:
        if not complete(r):
            print(f"::error title=Mutation test::shard {r['shard']}: {status(r).replace('**', '')}")
            code = 1
    if gate is not None and baseline_status in (None, "success"):
        for n in gate["open"]:
            print(f"::error title=Mutation gate::missed, not covered by exclude_re: {n}")
        if gate["open"]:
            code = 1
    tot = {k: sum(len(r[k]) for r in results) for k in KINDS}
    print(
        "::notice title=Mutation test::"
        + " ".join(f"{k}={tot[k]}" for k in KINDS)
        + f" untested={sum(len(r['untested']) for r in results)}"
        + f" mutants={sum(len(r['expected']) for r in results)}"
        + ("" if gate is None else f" gate_open={len(gate['open'])} gate_excluded={len(gate['covered'])}")
    )
    return code


def summarize_reused(
    reuse: Path,
    lists: Path,
    config: Path | None,
    md_path: Path | None,
    out_dir: Path | None,
    memory_limit: int | None = None,
) -> int:
    """#318: the gate's verdict from an earlier run of the same pull request.

    `reuse` is what mutants_reuse.py find wrote: reuse.json (the earlier
    run, its SHA, the key), the earlier lists and its shards' outcomes. The
    checks of find are made again here, so the verdict never rests on the
    tools job alone: every shard completed, no missed, timeout or
    memory-limit mutant, and every mutant of this run's list (after
    today's exclude_re) was tested then.
    """
    info = json.loads((reuse / "reuse.json").read_text(encoding="utf-8"))
    rlists, rshards = reuse / "lists", reuse / "shards"
    n = len(list(rlists.glob("shard-*.txt")))
    results = [shard_result(rshards, rlists, k, n) for k in range(n)]
    current = read_lines(lists / "all.txt")
    tested = {x for r in results for kind in ("caught", "unviable") for x in r[kind]}
    errors = [f"shard {r['shard']}: {status(r).replace('**', '')}" for r in results if not complete(r)]
    if n == 0:
        errors.append("the reused run has no shard")
    for kind in ("missed", "timeout", "memory"):
        names = sorted(x for r in results for x in r[kind])
        if names:
            errors.append(f"{len(names)} {kind} in the reused run: {', '.join(names)}")
    errors += [f"not tested in the reused run: {x}" for x in sorted(set(current) - tested)]
    excluded_now = sorted(tested - set(current))
    gate = {"open": [], "covered": [], "c": []}
    if config is not None:
        gate["c"] = c_waiting(config, read_lines(lists / "unexcluded.txt"))
    line = f"reused: run {info['run']} (SHA {info['sha']}), key {info['key']}"
    head = ["## Mutation test (ADR-0109)", "", f"**{line}**", ""]
    if errors:
        head += ["### Pull-request gate: FAILED, the reused run is not clean", ""] + [f"- {e}" for e in errors] + [""]
    else:
        head += [
            "### Pull-request gate: passed (reused)",
            "",
            f"No shard ran in this run. The mutants on the changed lines (before the exclusions), the merge commit's"
            f" tree without the inert paths and the environment are those of run [{info['run']}]({info.get('url', '')})"
            f" (SHA `{info['sha']}`): every shard of it completed with no missed, timeout or memory-limit mutant and"
            " `SKIP: 0` (ADR-0109 appendix \"Kapının önceki sonucu\", #318). Its outcomes are the verdict, with"
            f" today's `exclude_re` re-applied: {len(current)} mutant(s) on the changed lines, every one caught or"
            " unviable then.",
            "",
        ]
    if excluded_now:
        head += [f"Excluded by today's `exclude_re`, tested then ({len(excluded_now)}):", ""]
        head += [f"- {x}" for x in excluded_now] + [""]
    c_list(head, gate)
    body = markdown(results, None, None, memory_limit, baseline_facts(reuse / "baseline")).split("\n")[2:]
    md = "\n".join(head + [f"The tables of run {info['run']}:", ""] + body)
    print(md)
    if md_path:
        with md_path.open("a", encoding="utf-8") as f:
            f.write(md + "\n")
    if out_dir:
        out_dir.mkdir(parents=True, exist_ok=True)
        for kind in KINDS + ("untested",):
            names = sorted(x for r in results for x in r[kind])
            (out_dir / f"{kind}.txt").write_text("".join(x + "\n" for x in names), encoding="utf-8")
        (out_dir / "summary.md").write_text(md + "\n", encoding="utf-8")
        (out_dir / "reused.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    for e in errors:
        print(f"::error title=Mutation gate::{e}")
    print(f"::notice title=Mutation gate::{line}")
    tot = {k: sum(len(r[k]) for r in results) for k in KINDS}
    print(
        "::notice title=Mutation test::"
        + " ".join(f"{k}={tot[k]}" for k in KINDS)
        + f" untested={sum(len(r['untested']) for r in results)} mutants={len(current)}"
        + f" gate_open=0 reused_run={info['run']} excluded_now={len(excluded_now)}"
    )
    return 1 if errors else 0


def self_test() -> int:
    a = "crates/a.rs:10:5: replace f -> bool with true"
    b = "crates/a.rs:3:9: replace + with - in g"
    c = "crates/b.rs:7:1: delete ! in h"
    d = "crates/b.rs:9:1: replace h -> u32 with 0"
    e = "crates/b.rs:12:5: delete field x from struct S expression in k"
    with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stdout(io.StringIO()):
        root = Path(tmp) / "shards"
        lists = Path(tmp) / "lists"
        lists.mkdir()

        def listed(k: int, names: list[str]) -> None:
            (lists / f"shard-{k}.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")

        def shard(k: int, **kinds: list[str]) -> None:
            s = root / f"mutants-out-{k}" / "mutants.out"
            s.mkdir(parents=True)
            for kind, names in kinds.items():
                (s / f"{kind}.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")

        listed(0, [a, "\x1b[33m" + b + "\x1b[0m"])
        listed(1, [c, d])
        listed(2, [a])
        shard(0, caught=[a], missed=[b])
        assert summarize(root, lists, 1, None, None) == 0, "completed shard"
        assert summarize(root, lists, 2, None, None) == 1, "shard without artifact must fail"
        shard(1, unviable=[c])
        assert summarize(root, lists, 2, None, None) == 1, "stopped shard must fail"
        results = [shard_result(root, lists, k, 3) for k in range(3)]
        assert results[1]["untested"] == [d], results[1]
        assert results[2]["untested"] == [a] and not results[2]["present"], results[2]
        md = markdown(results)
        assert "> **NOT COMPLETED: 2 of 3 shard(s)** (1, 2); 2 mutant(s) not tested." in md, md
        assert "| 5 | 3 | 2 | 1 | 1 | 0 | 0 | 1 |" in md, md
        assert "| 1 | 2 | 1 | 0 | 0 | 0 | 0 | 1 | **NOT COMPLETED**: 1 not tested |" in md, md
        assert "| 2 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | **NOT COMPLETED**: no artifact, 1 not tested |" in md, md
        assert "<details><summary>Shard 1: not tested (1)</summary>" in md and f"- {d}" in md, md
        assert "- `crates/a.rs:3`: replace + with - in g" in md, md
        out = Path(tmp) / "merged"
        summarize(root, lists, 3, out, None)
        assert (out / "missed.txt").read_text(encoding="utf-8") == b + "\n"
        assert (out / "untested.txt").read_text(encoding="utf-8") == a + "\n" + d + "\n"
        listed(3, [])
        assert status(shard_result(root, lists, 3)) == "**NOT COMPLETED**: no mutant list for this shard"
        assert by_file([a, b]) == {"crates/a.rs": [(3, "replace + with - in g"), (10, "replace f -> bool with true")]}
        # #179: with one shard, download-artifact (pattern mutants-out-*)
        # extracts the one artifact straight into the download directory,
        # with no mutants-out-0/ in between.
        single = Path(tmp) / "single"
        (single / "mutants.out").mkdir(parents=True)
        (single / "mutants.out" / "caught.txt").write_text(a + "\n", encoding="utf-8")
        (single / "mutants.out" / "missed.txt").write_text(b + "\n", encoding="utf-8")
        assert summarize(single, lists, 1, None, None) == 0, "#179: a one-shard run is completed"
        # Two or more shards: one directory per artifact (mutants-out-<k>/).
        multi = Path(tmp) / "multi"
        for k, kind, name in ((0, "caught", a), (1, "unviable", c)):
            (multi / f"mutants-out-{k}" / "mutants.out").mkdir(parents=True)
            (multi / f"mutants-out-{k}" / "mutants.out" / f"{kind}.txt").write_text(name + "\n", encoding="utf-8")
        listed(0, [a])
        listed(1, [c])
        assert summarize(multi, lists, 2, None, None) == 0, "a two-shard run is completed"
        # The flattened layout is read for one shard alone: with two shards
        # a bare mutants.out/ belongs to no shard.
        assert shard_dir(single, 0, 2) is None

        # #160: the watchdog's kills are their own class, taken out of caught.
        mem = Path(tmp) / "mem"
        (mem / "mutants-out-0" / "mutants.out").mkdir(parents=True)
        (mem / "mutants-out-0" / "mutants.out" / "caught.txt").write_text(a + "\n" + c + "\n", encoding="utf-8")
        (mem / "mutants-out-0" / "memory-kills.tsv").write_text(
            f"2026-11-15T00:00:00Z\t42\t9000\t12500\t{c}\t/target/debug/deps/parser_tests-0 --exact t\n", encoding="utf-8"
        )
        listed(0, [a, c])
        r = shard_result(mem, lists, 0, 1)
        assert r["caught"] == [a] and r["memory"] == [c] and complete(r), r
        md = markdown([r])
        assert "### Memory limit (a test process killed by the watchdog; caught, as a timeout is) (1)" in md, md
        assert summarize(mem, lists, 1, None, None) == 0, "a memory-limit mutant is caught"

        # The gate: a missed mutant fails it unless an exclude_re entry covers
        # it; field deletions, which cargo-mutants does not exclude, too.
        cfg = Path(tmp) / "mutants.toml"
        cfg.write_text('exclude_re = [\n    "b\\\\.rs:\\\\d+:\\\\d+: delete field x from struct S expression in k$",\n]\n', encoding="utf-8")
        gated = Path(tmp) / "gated"
        (gated / "mutants-out-0" / "mutants.out").mkdir(parents=True)
        (gated / "mutants-out-0" / "mutants.out" / "missed.txt").write_text(b + "\n" + e + "\n", encoding="utf-8")
        listed(0, [b, e])
        assert summarize(gated, lists, 1, None, None) == 0, "a run that is not the gate does not fail on missed"
        assert summarize(gated, lists, 1, None, None, config=cfg) == 1, "the gate fails on an uncovered missed mutant"
        res = [shard_result(gated, lists, 0, 1)]
        open_, covered = gate_split(res[0]["missed"], exclusions(cfg))
        assert open_ == [b] and covered == [e], (open_, covered)
        md = markdown(res, {"open": open_, "covered": covered})
        assert "### Pull-request gate: FAILED, 1 missed mutant(s) not covered by `exclude_re`" in md, md
        assert "- `crates/a.rs:3`: replace + with - in g" in md and "**B, blind spot**" in md and "**C, suspect**" in md, md
        (gated / "mutants-out-0" / "mutants.out" / "missed.txt").write_text(e + "\n", encoding="utf-8")
        (gated / "mutants-out-0" / "mutants.out" / "caught.txt").write_text(b + "\n", encoding="utf-8")
        assert summarize(gated, lists, 1, None, None, config=cfg) == 0, "covered missed mutants pass the gate"
        # The gate and the limits: a mutant over the memory limit, whatever
        # outcome cargo-mutants gave it (caught; missed when the killed
        # process was not the test's verdict; unviable when it was the
        # build), and a timed-out mutant pass the gate as caught and are
        # listed in the summary (ADR-0109 section 4).
        f, g = "crates/c.rs:4:2: replace * with + in m", "crates/c.rs:8:2: replace m -> u8 with 1"
        limits = Path(tmp) / "limits"
        s = limits / "mutants-out-0" / "mutants.out"
        s.mkdir(parents=True)
        (s / "caught.txt").write_text(a + "\n", encoding="utf-8")
        (s / "missed.txt").write_text(c + "\n", encoding="utf-8")
        (s / "unviable.txt").write_text(f + "\n", encoding="utf-8")
        (s / "timeout.txt").write_text(d + "\n" + g + "\n", encoding="utf-8")
        (limits / "mutants-out-0" / "memory-kills.tsv").write_text(
            "".join(f"2026-11-15T00:00:00Z\t42\t9000\t12500\t{n}\t/target/debug/deps/t-0\n" for n in (a, c, f, d)),
            encoding="utf-8",
        )
        listed(0, [a, c, d, f, g])
        r = shard_result(limits, lists, 0, 1)
        assert sorted(r["memory"]) == sorted([a, c, d, f]) and r["timeout"] == [g], r
        assert not r["missed"] and not r["caught"] and not r["unviable"] and complete(r), r
        md_file = Path(tmp) / "limits.md"
        assert summarize(limits, lists, 1, None, md_file, config=cfg) == 0, "memory limit and timeout pass the gate"
        text = md_file.read_text(encoding="utf-8")
        assert "### Pull-request gate: passed" in text, text
        assert "### Memory limit (a test process killed by the watchdog; caught, as a timeout is) (4)" in text, text
        assert "### Timeout (1)" in text and "- `crates/c.rs:8`: replace m -> u8 with 1" in text, text
        # #283: the mutants on the changed lines that a C entry excludes are
        # listed with their issue (lists/unexcluded.txt: the gate's list
        # before the exclusions); A entries are not listed.
        ccfg = Path(tmp) / "c.toml"
        ccfg.write_text(
            "exclude_re = [\n"
            "    # A1. reason\n"
            f"    # expect: {b}\n"
            '    "a\\\\.rs:3:9: replace \\\\+ with - in g$",\n'
            "    # C. the spec says h is never 0 (silent-wrong?)\n"
            "    # issue: #301\n"
            f"    # expect: {d}\n"
            '    "b\\\\.rs:9:1: replace h -> u32 with 0$",\n'
            "]\n",
            encoding="utf-8",
        )
        assert c_waiting(ccfg, [a, b, d]) == [(d, 301)], c_waiting(ccfg, [a, b, d])
        assert c_waiting(ccfg, [a, b]) == []
        cdir = Path(tmp) / "cgate"
        (cdir / "mutants-out-0" / "mutants.out").mkdir(parents=True)
        (cdir / "mutants-out-0" / "mutants.out" / "caught.txt").write_text(a + "\n", encoding="utf-8")
        clists = Path(tmp) / "clists"
        clists.mkdir()
        (clists / "shard-0.txt").write_text(a + "\n", encoding="utf-8")
        (clists / "unexcluded.txt").write_text(f"{a}\n{b}\n{d}\n", encoding="utf-8")
        md_file = Path(tmp) / "c.md"
        assert summarize(cdir, clists, 1, None, md_file, config=ccfg) == 0
        text = md_file.read_text(encoding="utf-8")
        assert "Excluded as C, waiting for their issue (1): the gate tries them again when the issue closes" in text, text
        assert f"- {d}: #301" in text and f"- {b}" not in text, text
        # ADR-0128 §2, #253: the baseline is one job; its artifact brings the
        # peak memory of its build and tests and a warning above half of the
        # watchdog's limit.
        peaks = Path(tmp) / "peaks"
        for k in range(3):
            s = peaks / f"mutants-out-{k}"
            (s / "mutants.out").mkdir(parents=True)
            (s / "mutants.out" / "caught.txt").write_text(a + "\n", encoding="utf-8")
            (s / "stamp.json").write_text(json.dumps({"role": "shard", "timeout": 3610}), encoding="utf-8")
            listed(k, [a])
        bdir = Path(tmp) / "baseline"
        bdir.mkdir()
        (bdir / "stamp.json").write_text(
            json.dumps({"role": "baseline", "baseline_seconds": 1300, "timeout": 3610}), encoding="utf-8")
        (bdir / "baseline-build-peak.txt").write_text("7000\n", encoding="utf-8")
        (bdir / "baseline-test-peak.txt").write_text("6144\n", encoding="utf-8")
        res = [shard_result(peaks, lists, k, 3) for k in range(3)]
        assert [r["used_timeout"] for r in res] == [3610] * 3, res
        base = baseline_facts(bdir)
        assert base == {"present": True, "seconds": 1300, "timeout": 3610, "peak": {"build": 7000, "test": 6144}}, base
        warns = memory_warnings(base, 12288)
        assert warns == [
            "the baseline build peaked at 7000 MiB, above half of the watchdog's limit (12288 MiB);"
            " a mutant that adds a little memory is reported as memory limit (caught) and can hide a blind spot",
        ], warns
        assert memory_warnings(dict(base, peak={"build": None, "test": 6145}), 12288)[0] == "the baseline build's peak was not recorded"
        assert memory_warnings(baseline_facts(None), 12288) == []
        md = markdown(res, memory_limit=12288, base=base)
        assert "| Shard | Mutants | Tested | Caught | Missed | Timeout | Memory limit | Unviable | Status | Test timeout, s |" in md, md
        assert "| 0 | 1 | 1 | 1 | 0 | 0 | 0 | 0 | completed | 3610 |" in md, md
        assert "| passed | 1300 | 3610 | 7000 / 6144 |" in md, md
        assert "Watchdog limit 12288 MiB; a warning above half of it (6144 MiB)." in md, md
        assert "> **Baseline memory.** the baseline build peaked at 7000 MiB" in md, md
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            assert summarize(peaks, lists, 3, None, None, memory_limit=12288, baseline=bdir,
                             baseline_status="success") == 0, "a baseline peak warns, never fails"
        assert out.getvalue().count("::warning title=Mutation baseline memory::") == 1, out.getvalue()
        # The baseline job did not pass: no mutant verdict, the run fails, even
        # with every mutant caught; and a gate's missed mutant is not judged.
        for st in ("failure", "cancelled", "skipped"):
            out = io.StringIO()
            md_file = Path(tmp) / f"b-{st}.md"
            with contextlib.redirect_stdout(out):
                assert summarize(peaks, lists, 3, None, md_file, config=cfg, baseline=bdir, baseline_status=st) == 1, st
            text = md_file.read_text(encoding="utf-8")
            assert f"### Mutation test: FAILED, the baseline job did not pass (`{st}`): no mutant verdict" in text, text
            assert "Pull-request gate: passed" not in text and "### Pull-request gate" not in text, text
            assert f"::error title=Mutation test::the baseline job did not pass ({st})" in out.getvalue(), out.getvalue()
        # No baseline artifact although the job passed: fails.
        with contextlib.redirect_stdout(io.StringIO()):
            assert summarize(peaks, lists, 3, None, None, baseline=Path(tmp) / "nobase", baseline_status="success") == 1
        # ADR-0128 §3: a shard that used less than max(2 x baseline, floor).
        slow = dict(base, seconds=2000, timeout=4000)
        errs, warns = timeout_check(res, slow)
        assert errs == [] and len(warns) == 3 and "no mutant of it timed out" in warns[0], (errs, warns)
        assert "shard 0: --timeout 3610 s, below the rule's 4000 s (max(2 x baseline 2000 s, floor), ADR-0128 §3)" in warns[0], warns
        res_t = [dict(res[0], timeout=[b])] + res[1:]
        errs, warns = timeout_check(res_t, slow)
        assert errs == [f"shard 0: --timeout 3610 s, below the rule's 4000 s (max(2 x baseline 2000 s, floor),"
                        f" ADR-0128 §3); its 1 timeout mutant(s) are not decided: {b}"] and len(warns) == 2, (errs, warns)
        assert timeout_check(res_t, base) == ([], []), "at the rule: nothing to say"
        (bdir / "stamp.json").write_text(
            json.dumps({"role": "baseline", "baseline_seconds": 2000, "timeout": 4000}), encoding="utf-8")
        (peaks / "mutants-out-1" / "mutants.out" / "timeout.txt").write_text(b + "\n", encoding="utf-8")
        listed(1, [a, b])
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            assert summarize(peaks, lists, 3, None, None, baseline=bdir, baseline_status="success") == 1
        assert "::error title=Mutation test timeout::shard 1: --timeout 3610 s" in out.getvalue(), out.getvalue()
        listed(1, [a])
        (peaks / "mutants-out-1" / "mutants.out" / "timeout.txt").unlink()
        # No mutant to test: the gate passes and says why.
        md_file = Path(tmp) / "summary.md"
        assert summarize(Path(tmp) / "none", lists, 0, None, md_file, config=cfg, note="No Rust source in scope changed.") == 0
        text = md_file.read_text(encoding="utf-8")
        assert "**No mutant to test.** No Rust source in scope changed." in text and "gate: **passed** (nothing to test)" in text, text

        # #318: a gate run that reused an earlier run's outcomes (mutants_reuse.py find).
        reuse = Path(tmp) / "reuse"
        cur = Path(tmp) / "cur"
        cur.mkdir()
        for k, names in enumerate(([a, b], [c])):
            (reuse / "lists").mkdir(parents=True, exist_ok=True)
            (reuse / "lists" / f"shard-{k}.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")
            s = reuse / "shards" / f"mutants-out-{k}" / "mutants.out"
            s.mkdir(parents=True)
            (s / "caught.txt").write_text("".join(n + "\n" for n in names if n != c), encoding="utf-8")
            (s / "unviable.txt").write_text(c + "\n" if c in names else "", encoding="utf-8")
        sha = "ab" * 20
        (reuse / "reuse.json").write_text(json.dumps({"run": 4242, "sha": sha, "key": "k" * 64, "url": "u"}), encoding="utf-8")
        (cur / "all.txt").write_text(f"{a}\n{c}\n", encoding="utf-8")
        md_file = Path(tmp) / "reused.md"
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            assert summarize_reused(reuse, cur, cfg, md_file, Path(tmp) / "merged") == 0, out.getvalue()
        text = md_file.read_text(encoding="utf-8")
        assert f"reused: run 4242 (SHA {sha}), key {'k' * 64}" in text, text
        assert "### Pull-request gate: passed (reused)" in text and "No shard ran in this run" in text, text
        assert f"Excluded by today's `exclude_re`, tested then (1)" in text and f"- {b}" in text, text
        assert "| 3 | 3 | 0 | 2 | 0 | 0 | 0 | 1 |" in text, text
        assert f"::notice title=Mutation gate::reused: run 4242 (SHA {sha}), key {'k' * 64}" in out.getvalue(), out.getvalue()
        assert (Path(tmp) / "merged" / "caught.txt").read_text(encoding="utf-8") == f"{a}\n{b}\n"
        # Defensive: a reused run that is not clean, or that did not test a mutant of this run, fails.
        (cur / "all.txt").write_text(f"{a}\n{d}\n", encoding="utf-8")
        with contextlib.redirect_stdout(io.StringIO()):
            assert summarize_reused(reuse, cur, cfg, None, None) == 1, "a mutant not tested then"
        (cur / "all.txt").write_text(f"{a}\n", encoding="utf-8")
        (reuse / "shards" / "mutants-out-0" / "mutants.out" / "timeout.txt").write_text(b + "\n", encoding="utf-8")
        (reuse / "shards" / "mutants-out-0" / "mutants.out" / "caught.txt").write_text(a + "\n", encoding="utf-8")
        with contextlib.redirect_stdout(io.StringIO()):
            assert summarize_reused(reuse, cur, cfg, None, None) == 1, "a timeout is never reused"
    print("mutants_summary self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir", type=Path, nargs="?")
    ap.add_argument("--lists", type=Path)
    ap.add_argument("--shards", type=int)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--gate", action="store_true", help="fail on a missed mutant that no exclude_re entry covers")
    ap.add_argument("--config", type=Path, default=Path(".cargo/mutants.toml"))
    ap.add_argument("--note", help="a line for the top of the summary (why there is no mutant to test)")
    ap.add_argument("--memory-limit-mib", type=int, help="the watchdog's limit: warn on a baseline peak above half of it")
    ap.add_argument("--reused", type=Path, help="#318: the gate's verdict from an earlier run (mutants_reuse.py find)")
    ap.add_argument("--baseline", type=Path, help="ADR-0128 §2: the baseline job's artifact (mutants-baseline)")
    ap.add_argument("--baseline-result", help="ADR-0128 §2: the baseline job's result; anything but success fails")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.reused is not None:
        if args.lists is None:
            ap.error("--reused needs --lists")
        return summarize_reused(
            args.reused, args.lists, args.config if args.gate else None, args.markdown, args.out, args.memory_limit_mib
        )
    if args.dir is None or args.lists is None or args.shards is None:
        ap.error("DIR, --lists and --shards are required")
    return summarize(
        args.dir,
        args.lists,
        args.shards,
        args.out,
        args.markdown,
        args.config if args.gate else None,
        args.note,
        args.memory_limit_mib,
        args.baseline,
        args.baseline_result,
    )


if __name__ == "__main__":
    sys.exit(main())
