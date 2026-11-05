#!/usr/bin/env python3
"""Targeted mutation run (ADR-0109, section "Hedefli koşu").

cargo-mutants 27.1.0 does not filter field-deletion mutants ("delete field
... from struct ... expression") with --re or --exclude-re: they appear in a
--re list whatever the regex. A targeted run therefore narrows the list a
second time with --in-diff, by line: this script writes a unified diff
whose hunks touch exactly the lines of the mutants that match the regex
(each line "changed" into itself, so the diff matches the tree), and checks
the list that cargo-mutants makes from both filters.

  mutants_target.py diff  --re RE --list re-list.txt --out target.diff
  mutants_target.py check --re RE --list final-list.txt
  mutants_target.py shards --mutants M [--minutes-per-mutant T]
  mutants_target.py --self-test

`diff`: reads the --re list, keeps the names that the regex matches
(Python `re.search`, as cargo-mutants matches), writes the diff, prints how
many it dropped. `check`: exits 1 when a mutant in the list does not match
the regex or the list is empty; this is the proof that the run tries only
the requested mutants. `shards`: the shard count of ADR-0109 section 3 for
M mutants.
"""

import argparse
import math
import re
import sys
from pathlib import Path

# ADR-0109 section 3: a shard stays under 240 minutes, setup and baseline
# take S = 15 minutes, a mutant takes t = 210 s, at most 20 shards.
SHARD_MINUTES = 240
SETUP_MINUTES = 15
SECONDS_PER_MUTANT = 210
MAX_SHARDS = 20

NAME = re.compile(r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): ")


def read_list(path):
    return [l.rstrip("\n") for l in Path(path).read_text(encoding="utf-8").splitlines() if l.strip()]


def wanted(names, regex):
    rx = re.compile(regex)
    return [n for n in names if rx.search(n)]


def lines_by_file(names):
    out = {}
    for n in names:
        m = NAME.match(n)
        if not m:
            raise SystemExit(f"not a mutant name: {n}")
        out.setdefault(m["file"], set()).add(int(m["line"]))
    return out


def make_diff(names, root="."):
    parts = []
    for file, lines in sorted(lines_by_file(names).items()):
        text = Path(root, file).read_text(encoding="utf-8").split("\n")
        parts.append(f"--- a/{file}\n+++ b/{file}\n")
        for n in sorted(lines):
            src = text[n - 1]
            parts.append(f"@@ -{n},1 +{n},1 @@\n-{src}\n+{src}\n")
    return "".join(parts)


def shard_count(mutants, seconds=SECONDS_PER_MUTANT):
    for n in range(1, MAX_SHARDS + 1):
        if math.ceil(mutants / n) * seconds / 60 + SETUP_MINUTES <= SHARD_MINUTES:
            return n
    return MAX_SHARDS


def self_test():
    names = [
        "crates/a/src/x.rs:3:5: replace == with != in f",
        "crates/a/src/x.rs:9:1: delete field hi from struct R expression in g",
        "crates/b/src/y.rs:2:7: replace f -> bool with true",
    ]
    assert wanted(names, r"x\.rs:\d+:\d+: replace") == names[:1]
    assert lines_by_file(names) == {"crates/a/src/x.rs": {3, 9}, "crates/b/src/y.rs": {2}}
    # ADR-0109 section 3: 940 mutants need 15 shards; one mutant needs one.
    assert shard_count(940) == 15
    assert shard_count(1) == 1
    assert shard_count(63) == 1
    assert shard_count(64) == 1
    assert shard_count(65) == 2
    assert shard_count(10**6) == MAX_SHARDS
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        f = Path(d, "crates/a/src/x.rs")
        f.parent.mkdir(parents=True)
        f.write_text("l1\nl2\nl3\n", encoding="utf-8")
        diff = make_diff(names[:1], root=d)
        assert diff == "--- a/crates/a/src/x.rs\n+++ b/crates/a/src/x.rs\n@@ -3,1 +3,1 @@\n-l3\n+l3\n", diff
    print("self-test ok")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return 0
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("diff")
    d.add_argument("--re", required=True)
    d.add_argument("--list", required=True)
    d.add_argument("--out", required=True)
    c = sub.add_parser("check")
    c.add_argument("--re", required=True)
    c.add_argument("--list", required=True)
    s = sub.add_parser("shards")
    s.add_argument("--mutants", type=int, required=True)
    args = ap.parse_args()

    if args.cmd == "diff":
        names = read_list(args.list)
        keep = wanted(names, args.re)
        if not keep:
            print(f"::error title=Targeted mutation run::no mutant matches {args.re!r}")
            return 1
        Path(args.out).write_text(make_diff(keep), encoding="utf-8")
        for n in names:
            if n not in keep:
                print(f"dropped (does not match the regex): {n}")
        print(f"{len(keep)} of {len(names)} listed mutants match the regex")
        return 0
    if args.cmd == "check":
        names = read_list(args.list)
        rx = re.compile(args.re)
        extra = [n for n in names if not rx.search(n)]
        for n in extra:
            print(f"::error title=Targeted mutation run::not requested: {n}")
        if not names:
            print("::error title=Targeted mutation run::the final list is empty")
        if extra or not names:
            return 1
        print(f"all {len(names)} mutants in the list match {args.re!r}")
        return 0
    print(shard_count(args.mutants))
    return 0


if __name__ == "__main__":
    sys.exit(main())
