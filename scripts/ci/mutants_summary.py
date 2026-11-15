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
summary with the mutants it did not test, never left out. A mutant whose
test process the watchdog killed at the memory limit is its own class,
"memory limit": caught, as a timeout is (ADR-0109 section 4).

A full or targeted run measures the tests: a missed mutant does not fail
it. The pull-request gate (`--gate`, ADR-0109 appendix "PR kapısı") fails
on every missed mutant that no `exclude_re` entry of the configuration
covers; the summary lists them with the classification rules. `--shards 0`
is a gate run with no mutant to test (the reason is `--note`): it passes.

    mutants_summary.py SHARDS_DIR --lists LISTS_DIR --shards N [--out DIR] [--markdown FILE]
                       [--gate --config .cargo/mutants.toml] [--note TEXT]
    mutants_summary.py --self-test
"""

import argparse
import contextlib
import io
import re
import sys
import tempfile
import tomllib
from collections import defaultdict
from pathlib import Path

OUTCOMES = ("caught", "missed", "timeout", "unviable")
# `memory` is not a cargo-mutants outcome: the watchdog's kills, taken out
# of `caught` (or `timeout`).
KINDS = OUTCOMES + ("memory",)
NAME = re.compile(r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): (?P<change>.+)$")
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
    " with \"silent-wrong?\" in its title; the compiler is not fixed in this pull request, and the mutant"
    " stays missed here until that issue is decided.",
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


def shard_result(root: Path, lists: Path, k: int, shards: int = 0) -> dict:
    d = shard_dir(root, k, shards)
    res = {"shard": k, "present": d is not None, "expected": read_lines(lists / f"shard-{k}.txt")}
    for kind in OUTCOMES:
        res[kind] = read_lines(d / "mutants.out" / f"{kind}.txt") if d else []
    killed = set(memory_kills(d)) if d else set()
    res["memory"] = [n for n in res["caught"] + res["timeout"] if n in killed]
    res["caught"] = [n for n in res["caught"] if n not in killed]
    res["timeout"] = [n for n in res["timeout"] if n not in killed]
    seen = {n for kind in KINDS for n in res[kind]}
    res["untested"] = [n for n in res["expected"] if n not in seen]
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


def markdown(results: list[dict], gate: dict | None = None, note: str | None = None) -> str:
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
        return "\n".join(out)
    if note:
        out += [note, ""]
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
    out += [
        "| Mutants | Tested | Not tested | Caught | Missed | Timeout | Memory limit | Unviable |",
        "|---:|---:|---:|---:|---:|---:|---:|---:|",
        f"| {listed} | {done} | {untested} | {tot['caught']} | {tot['missed']} | {tot['timeout']} "
        f"| {tot['memory']} | {tot['unviable']} |",
        "",
        "| Shard | Mutants | Tested | Caught | Missed | Timeout | Memory limit | Unviable | Status |",
        "|---:|---:|---:|---:|---:|---:|---:|---:|---|",
    ]
    for r in results:
        out.append(
            f"| {r['shard']} | {len(r['expected'])} | {tested(r)} | {len(r['caught'])} | {len(r['missed'])} "
            f"| {len(r['timeout'])} | {len(r['memory'])} | {len(r['unviable'])} | {status(r)} |"
        )
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
) -> int:
    results = [shard_result(root, lists, k, shards) for k in range(shards)]
    gate = None
    if config is not None:
        missed = sorted(n for r in results for n in r["missed"])
        open_, covered = gate_split(missed, exclusions(config))
        gate = {"open": open_, "covered": covered}
    md = markdown(results, gate, note)
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
    for r in results:
        if not complete(r):
            print(f"::error title=Mutation test::shard {r['shard']}: {status(r).replace('**', '')}")
            code = 1
    if gate is not None:
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
        # No mutant to test: the gate passes and says why.
        md_file = Path(tmp) / "summary.md"
        assert summarize(Path(tmp) / "none", lists, 0, None, md_file, config=cfg, note="No Rust source in scope changed.") == 0
        text = md_file.read_text(encoding="utf-8")
        assert "**No mutant to test.** No Rust source in scope changed." in text and "gate: **passed** (nothing to test)" in text, text
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
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.dir is None or args.lists is None or args.shards is None:
        ap.error("DIR, --lists and --shards are required")
    return summarize(
        args.dir, args.lists, args.shards, args.out, args.markdown, args.config if args.gate else None, args.note
    )


if __name__ == "__main__":
    sys.exit(main())
