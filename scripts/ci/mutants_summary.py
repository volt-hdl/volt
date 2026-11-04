#!/usr/bin/env python3
"""Merge the shards of a cargo-mutants run (ADR-0109) into one summary.

Each shard of .github/workflows/mutants.yml uploads an artifact
`mutants-out-<k>` with `mutants-shard.txt` (the mutants of the shard, as
`cargo mutants --list --shard k/N` prints them) and cargo-mutants' own
`mutants.out/` (`caught.txt`, `missed.txt`, `timeout.txt`, `unviable.txt`:
one mutant name per line). This script counts them, lists the missed
mutants by file (file:line, change), writes the merged lists and a Markdown
summary, and fails when a shard is missing or did not test all of its
mutants. A missed mutant does not fail it: the run measures the tests.

    mutants_summary.py DIR --shards N [--out DIR] [--markdown FILE]
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


def read_lines(path: Path) -> list[str]:
    if not path.is_file():
        return []
    return [l.strip() for l in path.read_text(encoding="utf-8").splitlines() if l.strip()]


def shard_result(root: Path, k: int) -> dict:
    d = root / f"mutants-out-{k}"
    res = {"shard": k, "present": d.is_dir(), "listed": 0}
    for kind in KINDS:
        res[kind] = []
    if not res["present"]:
        return res
    res["listed"] = len(read_lines(d / "mutants-shard.txt"))
    out = d / "mutants.out"
    for kind in KINDS:
        res[kind] = read_lines(out / f"{kind}.txt")
    return res


def tested(res: dict) -> int:
    return sum(len(res[k]) for k in KINDS)


def complete(res: dict) -> bool:
    return res["present"] and res["listed"] > 0 and tested(res) == res["listed"]


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
    listed = sum(r["listed"] for r in results)
    done = sum(tested(r) for r in results)
    out = ["## Mutation test (ADR-0109)", ""]
    out += ["| Mutants | Tested | Caught | Missed | Timeout | Unviable |", "|---:|---:|---:|---:|---:|---:|"]
    out += [f"| {listed} | {done} | {tot['caught']} | {tot['missed']} | {tot['timeout']} | {tot['unviable']} |", ""]
    out += ["| Shard | Mutants | Tested | Caught | Missed | Timeout | Unviable | Status |", "|---:|---:|---:|---:|---:|---:|---:|---|"]
    for r in results:
        if not r["present"]:
            status = "no artifact"
        elif complete(r):
            status = "complete"
        else:
            status = f"incomplete ({r['listed'] - tested(r)} not tested)"
        out.append(
            f"| {r['shard']} | {r['listed']} | {tested(r)} | {len(r['caught'])} | {len(r['missed'])} "
            f"| {len(r['timeout'])} | {len(r['unviable'])} | {status} |"
        )
    out.append("")
    for kind, title in (("missed", "Missed"), ("timeout", "Timeout")):
        names = sorted(n for r in results for n in r[kind])
        out += [f"### {title} ({len(names)})", ""]
        for f, items in by_file(names).items():
            out += [f"**`{f}`** ({len(items)})", ""]
            out += [f"- `{f}:{line}`: {change}" for line, change in items]
            out.append("")
    unviable = sorted(n for r in results for n in r["unviable"])
    out += [f"<details><summary>Unviable ({len(unviable)}): the mutation does not build</summary>", ""]
    out += [f"- {n}" for n in unviable]
    out += ["", "</details>", ""]
    return "\n".join(out)


def summarize(root: Path, shards: int, out_dir: Path | None, md_path: Path | None) -> int:
    results = [shard_result(root, k) for k in range(shards)]
    md = markdown(results)
    print(md)
    if md_path:
        with md_path.open("a", encoding="utf-8") as f:
            f.write(md + "\n")
    if out_dir:
        out_dir.mkdir(parents=True, exist_ok=True)
        for kind in KINDS:
            names = sorted(n for r in results for n in r[kind])
            (out_dir / f"{kind}.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")
        (out_dir / "summary.md").write_text(md + "\n", encoding="utf-8")
    status = 0
    for r in results:
        if not r["present"]:
            print(f"::error title=Mutation test::shard {r['shard']}: no artifact")
            status = 1
        elif not complete(r):
            print(f"::error title=Mutation test::shard {r['shard']}: {tested(r)} of {r['listed']} mutants tested")
            status = 1
    tot = {k: sum(len(r[k]) for r in results) for k in KINDS}
    print(
        "::notice title=Mutation test::"
        + " ".join(f"{k}={tot[k]}" for k in KINDS)
        + f" mutants={sum(r['listed'] for r in results)}"
    )
    return status


def self_test() -> int:
    a = "crates/a.rs:10:5: replace f -> bool with true"
    b = "crates/a.rs:3:9: replace + with - in g"
    c = "crates/b.rs:7:1: delete ! in h"
    with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
        root = Path(d)

        def shard(k: int, listed: list[str], **kinds: list[str]) -> None:
            s = root / f"mutants-out-{k}"
            (s / "mutants.out").mkdir(parents=True)
            (s / "mutants-shard.txt").write_text("".join(n + "\n" for n in listed), encoding="utf-8")
            for kind, names in kinds.items():
                (s / "mutants.out" / f"{kind}.txt").write_text("".join(n + "\n" for n in names), encoding="utf-8")

        shard(0, [a, b], caught=[a], missed=[b])
        assert summarize(root, 1, None, None) == 0, "complete shard"
        assert summarize(root, 2, None, None) == 1, "missing shard must fail"
        shard(1, [c, a], unviable=[c])
        assert summarize(root, 2, None, None) == 1, "incomplete shard must fail"
        md = markdown([shard_result(root, 0), shard_result(root, 1)])
        assert "| 4 | 3 | 1 | 1 | 0 | 1 |" in md, md
        assert "- `crates/a.rs:3`: replace + with - in g" in md, md
        assert "incomplete (1 not tested)" in md, md
        out = root / "merged"
        summarize(root, 2, out, None)
        assert (out / "missed.txt").read_text(encoding="utf-8") == b + "\n"
        assert by_file([a, b]) == {"crates/a.rs": [(3, "replace + with - in g"), (10, "replace f -> bool with true")]}
    print("mutants_summary self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir", type=Path, nargs="?")
    ap.add_argument("--shards", type=int, default=8)
    ap.add_argument("--out", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.dir is None:
        ap.error("DIR is required")
    return summarize(args.dir, args.shards, args.out, args.markdown)


if __name__ == "__main__":
    sys.exit(main())
