#!/usr/bin/env python3
"""Merge the shards of a cargo-mutants run (ADR-0109) into one summary.

The tools job of .github/workflows/mutants.yml uploads `mutants-list`:
`shard-<k>.txt` holds the mutants of shard k, as `cargo mutants --list
--shard k/N` prints them. Each shard uploads `mutants-out-<k>` with
cargo-mutants' own `mutants.out/` (`caught.txt`, `missed.txt`,
`timeout.txt`, `unviable.txt`: one mutant name per line); the upload runs
even when the shard was stopped, so a stopped shard still brings the
outcomes it reached.

This script counts them, lists the missed mutants by file (file:line,
change), writes the merged lists and a Markdown summary, and fails when a
shard is not completed: no artifact, or fewer outcomes than the mutants it
was given. A shard that is not completed is named at the top of the
summary with the mutants it did not test, never left out. A missed mutant
does not fail the script: the run measures the tests.

    mutants_summary.py SHARDS_DIR --lists LISTS_DIR --shards N [--out DIR] [--markdown FILE]
    mutants_summary.py --self-test
"""

import argparse
import contextlib
import io
import re
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

KINDS = ("caught", "missed", "timeout", "unviable")
NAME = re.compile(r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): (?P<change>.+)$")
# `cargo mutants --list` colors its output when CARGO_TERM_COLOR=always.
ANSI = re.compile(r"\x1b\[[0-9;]*m")


def read_lines(path: Path) -> list[str]:
    if not path.is_file():
        return []
    text = ANSI.sub("", path.read_text(encoding="utf-8"))
    return [l.strip() for l in text.splitlines() if l.strip()]


def shard_result(root: Path, lists: Path, k: int) -> dict:
    d = root / f"mutants-out-{k}"
    res = {"shard": k, "present": d.is_dir(), "expected": read_lines(lists / f"shard-{k}.txt")}
    out = d / "mutants.out"
    for kind in KINDS:
        res[kind] = read_lines(out / f"{kind}.txt") if res["present"] else []
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


def markdown(results: list[dict]) -> str:
    tot = {k: sum(len(r[k]) for r in results) for k in KINDS}
    listed = sum(len(r["expected"]) for r in results)
    done = sum(tested(r) for r in results)
    untested = sum(len(r["untested"]) for r in results)
    incomplete = [r for r in results if not complete(r)]
    out = ["## Mutation test (ADR-0109)", ""]
    if incomplete:
        out += [
            f"> **NOT COMPLETED: {len(incomplete)} of {len(results)} shard(s)** "
            f"({', '.join(str(r['shard']) for r in incomplete)}); {untested} mutant(s) not tested. "
            "The counts below cover the tested mutants alone.",
            "",
        ]
    out += ["| Mutants | Tested | Not tested | Caught | Missed | Timeout | Unviable |", "|---:|---:|---:|---:|---:|---:|---:|"]
    out += [f"| {listed} | {done} | {untested} | {tot['caught']} | {tot['missed']} | {tot['timeout']} | {tot['unviable']} |", ""]
    out += ["| Shard | Mutants | Tested | Caught | Missed | Timeout | Unviable | Status |", "|---:|---:|---:|---:|---:|---:|---:|---|"]
    for r in results:
        out.append(
            f"| {r['shard']} | {len(r['expected'])} | {tested(r)} | {len(r['caught'])} | {len(r['missed'])} "
            f"| {len(r['timeout'])} | {len(r['unviable'])} | {status(r)} |"
        )
    out.append("")
    for kind, title in (("missed", "Missed"), ("timeout", "Timeout")):
        names = sorted(n for r in results for n in r[kind])
        out += [f"### {title} ({len(names)})", ""]
        for f, items in by_file(names).items():
            out += [f"**`{f}`** ({len(items)})", ""]
            out += [f"- `{f}:{line}`: {change}" for line, change in items]
            out.append("")
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


def summarize(root: Path, lists: Path, shards: int, out_dir: Path | None, md_path: Path | None) -> int:
    results = [shard_result(root, lists, k) for k in range(shards)]
    md = markdown(results)
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
    tot = {k: sum(len(r[k]) for r in results) for k in KINDS}
    print(
        "::notice title=Mutation test::"
        + " ".join(f"{k}={tot[k]}" for k in KINDS)
        + f" untested={sum(len(r['untested']) for r in results)}"
        + f" mutants={sum(len(r['expected']) for r in results)}"
    )
    return code


def self_test() -> int:
    a = "crates/a.rs:10:5: replace f -> bool with true"
    b = "crates/a.rs:3:9: replace + with - in g"
    c = "crates/b.rs:7:1: delete ! in h"
    d = "crates/b.rs:9:1: replace h -> u32 with 0"
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
        results = [shard_result(root, lists, k) for k in range(3)]
        assert results[1]["untested"] == [d], results[1]
        assert results[2]["untested"] == [a] and not results[2]["present"], results[2]
        md = markdown(results)
        assert "> **NOT COMPLETED: 2 of 3 shard(s)** (1, 2); 2 mutant(s) not tested." in md, md
        assert "| 5 | 3 | 2 | 1 | 1 | 0 | 1 |" in md, md
        assert "| 1 | 2 | 1 | 0 | 0 | 0 | 1 | **NOT COMPLETED**: 1 not tested |" in md, md
        assert "| 2 | 1 | 0 | 0 | 0 | 0 | 0 | **NOT COMPLETED**: no artifact, 1 not tested |" in md, md
        assert "<details><summary>Shard 1: not tested (1)</summary>" in md and f"- {d}" in md, md
        assert "- `crates/a.rs:3`: replace + with - in g" in md, md
        out = Path(tmp) / "merged"
        summarize(root, lists, 3, out, None)
        assert (out / "missed.txt").read_text(encoding="utf-8") == b + "\n"
        assert (out / "untested.txt").read_text(encoding="utf-8") == a + "\n" + d + "\n"
        listed(3, [])
        assert status(shard_result(root, lists, 3)) == "**NOT COMPLETED**: no mutant list for this shard"
        assert by_file([a, b]) == {"crates/a.rs": [(3, "replace + with - in g"), (10, "replace f -> bool with true")]}
    print("mutants_summary self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir", type=Path, nargs="?")
    ap.add_argument("--lists", type=Path)
    ap.add_argument("--shards", type=int)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.dir is None or args.lists is None or args.shards is None:
        ap.error("DIR, --lists and --shards are required")
    return summarize(args.dir, args.lists, args.shards, args.out, args.markdown)


if __name__ == "__main__":
    sys.exit(main())
