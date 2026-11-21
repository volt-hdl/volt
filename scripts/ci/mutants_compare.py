#!/usr/bin/env python3
"""Compare two runs of the mutation gate, mutant by mutant (ADR-0128, #366).

A change that only makes the gate faster (the test order, the baseline,
the timeout, the shard count, a build setting) must leave the result
alone: the same mutants, each with the same outcome. This script reads
two runs of `.github/workflows/mutants.yml`, each downloaded whole with

    gh run download <run-id> --repo volt-hdl/volt --dir <DIR>

(`<DIR>/mutants-list/`, `<DIR>/mutants-out-<k>/`), and fails when:

- the mutant lists differ (`mutants-list/all.txt`, or the shards' lists);
- a mutant has no outcome in either run (a shard not completed);
- a mutant's outcome differs: caught, missed, timeout, unviable, or
  memory limit (the watchdog's class, read as mutants_summary.py reads it).

It prints every difference by name ("missed -> timeout: <mutant>"), and,
for both runs, the outcome counts, the build and test seconds from
`outcomes.json` (per mutant, per shard, total), each shard's baseline
seconds and test timeout (`stamp.json`), the baseline job's seconds and
the timeout its rule gives when the run has one (`mutants-baseline/`,
ADR-0128 §2; a shard then has no baseline of its own) and the `key.json`
hashes of the mutants and of the tree. A different tree hash is not a difference: the
run under test carries the change. A different mutant hash is one.

    mutants_compare.py A_DIR B_DIR [--markdown FILE] [--times FILE.tsv]
    mutants_compare.py --self-test

Exit codes: 0 the same mutants with the same outcomes; 1 a difference;
2 a run that cannot be read (no `mutants-list`).
"""

import argparse
import contextlib
import io
import json
import sys
import tempfile
from collections import Counter
from pathlib import Path

import mutants_summary  # scripts/ci, next to this script

KINDS = mutants_summary.KINDS  # caught, missed, timeout, unviable, memory
LABEL = {"memory": "memory limit"}


def label(kind: str) -> str:
    return LABEL.get(kind, kind)


def shard_count(lists: Path) -> int:
    return len(list(lists.glob("shard-*.txt")))


def outcome_times(d: Path | None) -> dict[str, tuple[float, float]]:
    """Mutant name -> (build seconds, test seconds) from cargo-mutants' outcomes.json."""
    if d is None:
        return {}
    path = d / "mutants.out" / "outcomes.json"
    if not path.is_file():
        return {}
    out: dict[str, tuple[float, float]] = {}
    for o in json.loads(path.read_text(encoding="utf-8")).get("outcomes", []):
        scenario = o.get("scenario")
        if not isinstance(scenario, dict) or "Mutant" not in scenario:
            continue
        build = test = 0.0
        for p in o.get("phase_results", []):
            if p.get("phase") == "Build":
                build = float(p.get("duration", 0))
            elif p.get("phase") == "Test":
                test = float(p.get("duration", 0))
        out[scenario["Mutant"]["name"]] = (build, test)
    return out


def read_run(root: Path) -> dict:
    lists = root / "mutants-list"
    if not lists.is_dir():
        raise FileNotFoundError(f"{root}: no mutants-list/ (download the whole run with gh run download)")
    shards = shard_count(lists)
    listed = mutants_summary.read_lines(lists / "all.txt")
    if not listed:
        listed = [n for k in range(shards) for n in mutants_summary.read_lines(lists / f"shard-{k}.txt")]
    outcome: dict[str, str] = {}
    untested: list[str] = []
    times: dict[str, tuple[float, float]] = {}
    shard_rows = []
    for k in range(shards):
        res = mutants_summary.shard_result(root, lists, k, shards)
        d = mutants_summary.shard_dir(root, k, shards)
        for kind in KINDS:
            for n in res[kind]:
                outcome[n] = kind
        untested += res["untested"]
        t = outcome_times(d)
        times.update(t)
        stamp = {}
        if d is not None and (d / "stamp.json").is_file():
            stamp = json.loads((d / "stamp.json").read_text(encoding="utf-8"))
        shard_rows.append(
            {
                "shard": k,
                "mutants": len(res["expected"]),
                "status": mutants_summary.status(res),
                "build": sum(b for b, _ in t.values()),
                "test": sum(s for _, s in t.values()),
                "baseline": stamp.get("baseline_seconds"),
                "timeout": stamp.get("timeout"),
            }
        )
    # ADR-0128 §2: one baseline job per run, beside the shards.
    bst = mutants_summary.read_stamp(root / mutants_summary.BASELINE_ARTIFACT)
    baseline = {"seconds": bst.get("baseline_seconds"), "timeout": bst.get("timeout")} if bst else None
    key = {}
    if (lists / "key.json").is_file():
        key = json.loads((lists / "key.json").read_text(encoding="utf-8")).get("hashes", {})
    return {
        "root": root,
        "listed": listed,
        "outcome": outcome,
        "untested": untested,
        "times": times,
        "shards": shard_rows,
        "key": key,
        "baseline": baseline,
    }


def compare(a: dict, b: dict) -> list[str]:
    diffs: list[str] = []
    ca, cb = Counter(a["listed"]), Counter(b["listed"])
    for n in sorted((ca - cb).elements()):
        diffs.append(f"only in A's list: {n}")
    for n in sorted((cb - ca).elements()):
        diffs.append(f"only in B's list: {n}")
    ka, kb = a["key"].get("mutants"), b["key"].get("mutants")
    if ka and kb and ka != kb:
        diffs.append(f"key.json mutant hash differs: {ka} vs {kb}")
    for side, run in (("A", a), ("B", b)):
        for n in sorted(set(run["untested"])):
            diffs.append(f"no outcome in {side} (shard not completed): {n}")
    for n in sorted(set(a["listed"]) & set(b["listed"])):
        oa, ob = a["outcome"].get(n), b["outcome"].get(n)
        if oa and ob and oa != ob:
            diffs.append(f"{label(oa)} -> {label(ob)}: {n}")
    return diffs


def counts(run: dict) -> str:
    c = Counter(run["outcome"].values())
    return ", ".join(f"{label(k)} {c.get(k, 0)}" for k in KINDS)


def report(a: dict, b: dict, diffs: list[str]) -> str:
    out = ["## Mutation runs compared (mutants_compare.py)", ""]
    out.append("| | A | B |")
    out.append("|---|---|---|")
    out.append(f"| Run directory | `{a['root'].name}` | `{b['root'].name}` |")
    out.append(f"| Mutants | {len(a['listed'])} | {len(b['listed'])} |")
    out.append(f"| Outcomes | {counts(a)} | {counts(b)} |")
    for what in ("mutants", "tree"):
        out.append(f"| key.json {what} hash | `{a['key'].get(what, '-')[:12]}` | `{b['key'].get(what, '-')[:12]}` |")
    cell = [
        "- (a baseline in each shard)" if r["baseline"] is None
        else f"{r['baseline']['seconds']} s, timeout {r['baseline']['timeout']} s"
        for r in (a, b)
    ]
    out.append(f"| Baseline job (ADR-0128 section 2): tests, timeout by the rule | {cell[0]} | {cell[1]} |")
    for side, run in (("A", a), ("B", b)):
        tb = sum(x[0] for x in run["times"].values())
        tt = sum(x[1] for x in run["times"].values())
        out.append(f"| {side}: build / test total | {tb:.0f} s / {tt:.0f} s | |")
    out.append("")
    for side, run in (("A", a), ("B", b)):
        out.append(f"**{side}, per shard**")
        out.append("")
        out.append("| Shard | Mutants | build total (s) | test total (s) | Baseline (s) | Timeout (s) | Status |")
        out.append("|---:|---:|---:|---:|---:|---:|---|")
        for r in run["shards"]:
            out.append(
                f"| {r['shard']} | {r['mutants']} | {r['build']:.0f} | {r['test']:.0f} | "
                f"{r['baseline'] if r['baseline'] is not None else '-'} | "
                f"{r['timeout'] if r['timeout'] is not None else '-'} | {r['status']} |"
            )
        out.append("")
    if diffs:
        out.append(f"**{len(diffs)} difference(s): the result is not the same.**")
        out.append("")
        out += [f"- {d}" for d in diffs]
    else:
        out.append(f"**No difference**: the same {len(a['listed'])} mutants, each with the same outcome.")
    out.append("")
    return "\n".join(out)


def times_tsv(a: dict, b: dict) -> str:
    lines = ["mutant\toutcome_a\toutcome_b\tbuild_a\ttest_a\tbuild_b\ttest_b"]
    for n in sorted(set(a["listed"]) | set(b["listed"])):
        ba, ta = a["times"].get(n, (0.0, 0.0))
        bb, tb = b["times"].get(n, (0.0, 0.0))
        lines.append(
            f"{n}\t{a['outcome'].get(n, '-')}\t{b['outcome'].get(n, '-')}\t{ba:.1f}\t{ta:.1f}\t{bb:.1f}\t{tb:.1f}"
        )
    return "\n".join(lines) + "\n"


def run(a_dir: Path, b_dir: Path, markdown: Path | None, times: Path | None) -> int:
    try:
        a, b = read_run(a_dir), read_run(b_dir)
    except FileNotFoundError as e:
        print(f"::error title=mutants_compare::{e}")
        return 2
    diffs = compare(a, b)
    text = report(a, b, diffs)
    print(text)
    if markdown:
        markdown.write_text(text, encoding="utf-8")
    if times:
        times.write_text(times_tsv(a, b), encoding="utf-8")
    return 1 if diffs else 0


# ---------------------------------------------------------------- self-test


def make_run(root: Path, shards: dict[int, dict[str, str]], key: str = "k", memory: dict[int, list[str]] | None = None,
             missing: tuple[int, ...] = (), flat: bool = False, baseline_job: bool = False) -> Path:
    """A downloaded run: shards = {k: {mutant: outcome}}; outcome in OUTCOMES."""
    lists = root / "mutants-list"
    lists.mkdir(parents=True)
    names = [n for k in sorted(shards) for n in shards[k]]
    (lists / "all.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")
    (lists / "key.json").write_text(json.dumps({"hashes": {"mutants": key, "tree": "t-" + root.name}}), encoding="utf-8")
    for k, ms in shards.items():
        (lists / f"shard-{k}.txt").write_text("".join(n + "\n" for n in ms), encoding="utf-8")
        if k in missing:
            continue
        d = root if flat else root / f"mutants-out-{k}"
        out = d / "mutants.out"
        out.mkdir(parents=True)
        for kind in mutants_summary.OUTCOMES:
            (out / f"{kind}.txt").write_text("".join(n + "\n" for n, o in ms.items() if o == kind), encoding="utf-8")
        outcomes = [
            {"scenario": "Baseline", "phase_results": []},
        ] + [
            {
                "scenario": {"Mutant": {"name": n}},
                "phase_results": [{"phase": "Build", "duration": 10.0}, {"phase": "Test", "duration": 100.0}],
            }
            for n in ms
        ]
        (out / "outcomes.json").write_text(json.dumps({"outcomes": outcomes}), encoding="utf-8")
        st = {"role": "shard", "baseline_seconds": None, "timeout": 3610} if baseline_job else \
            {"baseline_seconds": 1000, "timeout": 2000}
        (d / "stamp.json").write_text(json.dumps(st), encoding="utf-8")
        for n in (memory or {}).get(k, []):
            with (d / "memory-kills.tsv").open("a", encoding="utf-8") as f:
                f.write(f"t\t1\t2\t3\t{n}\tcmd\n")
    if baseline_job:
        bd = root / mutants_summary.BASELINE_ARTIFACT
        bd.mkdir(parents=True)
        (bd / "stamp.json").write_text(
            json.dumps({"role": "baseline", "baseline_seconds": 1200, "timeout": 3610}), encoding="utf-8")
    return root


def self_test() -> int:
    failures: list[str] = []
    m1 = "crates/a/src/x.rs:1:1: replace f -> bool with true"
    m2 = "crates/a/src/x.rs:2:5: replace + with -"
    m3 = "crates/a/src/y.rs:3:9: delete !"
    base = {0: {m1: "caught", m2: "missed"}, 1: {m3: "unviable"}}

    def case(name: str, a_spec: dict, b_spec: dict, want_code: int, want_text: str = "", **kw) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            a = make_run(Path(tmp) / "A", a_spec)
            b = make_run(Path(tmp) / "B", b_spec, **kw)
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                code = run(a, b, None, Path(tmp) / "t.tsv")
            text = buf.getvalue()
            if code != want_code:
                failures.append(f"{name}: exit {code}, want {want_code}\n{text}")
            if want_text and want_text not in text:
                failures.append(f"{name}: '{want_text}' not in the report\n{text}")

    case("same runs", base, base, 0, "No difference")
    case("same mutants in another shard layout", base, {0: {m1: "caught", m3: "unviable"}, 1: {m2: "missed"}}, 0)
    case("missed -> timeout", base, {0: {m1: "caught", m2: "timeout"}, 1: {m3: "unviable"}}, 1, f"missed -> timeout: {m2}")
    case("caught -> missed", base, {0: {m1: "missed", m2: "missed"}, 1: {m3: "unviable"}}, 1, f"caught -> missed: {m1}")
    case("missed -> memory limit", base, base, 1, f"missed -> memory limit: {m2}", memory={0: [m2]})
    case("a mutant only in B", base, {0: {m1: "caught", m2: "missed"}, 1: {m3: "unviable", m3 + "x": "caught"}}, 1,
         "only in B's list")
    case("a shard not completed", base, base, 1, "no outcome in B", missing=(1,))
    case("another mutant hash", base, base, 1, "mutant hash differs", key="other")
    case("one shard, flat layout (#179)", {0: base[0]}, {0: base[0]}, 0, flat=True)
    # ADR-0128 §2: B has one baseline job; its shards have none.
    case("a baseline job in B", base, base, 0, "| Baseline job (ADR-0128 section 2): tests, timeout by the rule"
         " | - (a baseline in each shard) | 1200 s, timeout 3610 s |", baseline_job=True)
    case("a baseline job in B, its shard rows", base, base, 0, "| 0 | 2 | 20 | 200 | - | 3610 | completed |",
         baseline_job=True)
    case("a baseline job in B, one shard (#179)", {0: base[0]}, {0: base[0]}, 0, "1200 s, timeout 3610 s",
         flat=True, baseline_job=True)
    with tempfile.TemporaryDirectory() as tmp:
        a = make_run(Path(tmp) / "A", base)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = run(a, Path(tmp) / "nothing", None, None)
        if code != 2:
            failures.append(f"a run with no mutants-list: exit {code}, want 2")
    for f in failures:
        print(f"FAIL: {f}")
    print("mutants_compare self-test: " + ("ok" if not failures else f"{len(failures)} failure(s)"))
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("a", type=Path, nargs="?")
    ap.add_argument("b", type=Path, nargs="?")
    ap.add_argument("--markdown", type=Path, help="also write the report here")
    ap.add_argument("--times", type=Path, help="per-mutant outcomes and times of both runs (TSV)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.a is None or args.b is None:
        ap.error("A_DIR and B_DIR are required")
    return run(args.a, args.b, args.markdown, args.times)


if __name__ == "__main__":
    sys.exit(main())
