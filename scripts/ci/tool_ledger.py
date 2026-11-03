#!/usr/bin/env python3
"""Tool ledger: count and cross-check the tests that need a tool (ADR-0107).

Every tool lookup of a test (`crates/volt-driver/tests/tools/mod.rs`:
`require`, `opt_in`) appends one line to the file named by
VOLT_TOOL_LEDGER:

    <SKIP|RAN>\t<test binary>\t<test name>\t<tool or gate>\t<required 0|1>

`report` prints the summary of one job's ledger; with
`--require-zero-skips` (jobs that install the tools) a single SKIP fails
the job, and `--min-ran N` fails it when fewer than N tool lookups found
their tool (the ledger variable did not reach the tests, or the suite
shrank).

`cover` is the inventory check. Every (binary, test, tool) seen in any
ledger -- the tool-less job sees every tool test, since there each of them
skips or finds a compiler -- must have run in a tool job while that tool
was required (VOLT_REQUIRE_TOOLS). A test that only ran where the tool
was optional, or never ran with its tool, fails the check by name.

    tool_ledger.py report LEDGER --job NAME [--require-zero-skips] [--min-ran N]
    tool_ledger.py cover --inventory LEDGER... --tools LEDGER...
    tool_ledger.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import tempfile
from collections import Counter
from typing import NamedTuple


class Entry(NamedTuple):
    event: str
    binary: str
    test: str
    tool: str
    required: bool

    @property
    def key(self) -> tuple[str, str, str]:
        return (self.binary, self.test, self.tool)


def load(path: pathlib.Path) -> list[Entry]:
    """A missing file is an empty ledger (no test asked for a tool)."""
    if not path.exists():
        return []
    entries = []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) != 5 or parts[0] not in ("SKIP", "RAN") or parts[4] not in ("0", "1"):
            raise SystemExit(f"{path}:{n}: malformed ledger line: {line!r}")
        entries.append(Entry(parts[0], parts[1], parts[2], parts[3], parts[4] == "1"))
    return entries


def name(key: tuple[str, str, str]) -> str:
    binary, test, tool = key
    return f"{binary}::{test} ({tool})"


def report(entries: list[Entry], job: str, require_zero: bool, min_ran: int) -> tuple[int, list[str]]:
    skips = sorted({e.key for e in entries if e.event == "SKIP"})
    ran = {e.key for e in entries if e.event == "RAN"}
    required = {e.key for e in entries if e.event == "RAN" and e.required}
    tests = {(e.binary, e.test) for e in entries}
    out = [
        f"Tool ledger [{job}]: {len(tests)} test(s) asked for a tool; "
        f"{len(ran)} lookup(s) found the tool ({len(required)} required), "
        f"SKIP: {len(skips)}",
    ]
    per_tool = Counter(e.tool for e in entries if e.event == "SKIP")
    if per_tool:
        out.append("  skips per tool: " + ", ".join(f"{t} {c}" for t, c in sorted(per_tool.items())))
    out += [f"  SKIP {name(k)}" for k in skips]
    code = 0
    if require_zero and skips:
        out.append(f"::error::{len(skips)} test(s) skipped in a job that installs the tools (ADR-0107)")
        code = 1
    if len(ran) < min_ran:
        out.append(f"::error::only {len(ran)} tool lookup(s) found their tool (< {min_ran}): "
                   "VOLT_TOOL_LEDGER did not reach the tests, or the tool tests shrank")
        code = 1
    return code, out


def cover(inventory: list[Entry], tools: list[Entry]) -> tuple[int, list[str]]:
    seen = {e.key for e in inventory} | {e.key for e in tools}
    required = {e.key for e in tools if e.event == "RAN" and e.required}
    optional = {e.key for e in tools if e.event == "RAN" and not e.required}
    missing = sorted(seen - required)
    out = [f"Tool coverage: {len(seen)} (test, tool) pair(s) in the ledgers, "
           f"{len(seen) - len(missing)} ran with the tool required, {len(missing)} did not"]
    for key in missing:
        why = ("ran in a tool job, but the tool was not in VOLT_REQUIRE_TOOLS there"
               if key in optional else "never ran in a tool job (skipped or not invoked)")
        out.append(f"::error::{name(key)}: {why}")
    return (1 if missing else 0), out


def self_test() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        root = pathlib.Path(tmp)
        less = root / "less.tsv"
        less.write_text(
            "SKIP\ta_tests\tt1\tverilator\t0\n"
            "SKIP\ta_tests\tt2\tsby\t0\n"
            "RAN\tb_tests\tt3\tcc\t0\n"
            "SKIP\tc_tests\tt4\tVOLT_DOCKER_E2E\t0\n",
            encoding="utf-8",
        )
        good = root / "good.tsv"
        good.write_text(
            "RAN\ta_tests\tt1\tverilator\t1\n"
            "RAN\ta_tests\tt2\tsby\t1\n"
            "RAN\tb_tests\tt3\tcc\t1\n",
            encoding="utf-8",
        )
        docker = root / "docker.tsv"
        docker.write_text("RAN\tc_tests\tt4\tVOLT_DOCKER_E2E\t1\n", encoding="utf-8")
        partial = root / "partial.tsv"
        partial.write_text("RAN\ta_tests\tt1\tverilator\t1\nRAN\ta_tests\tt2\tsby\t0\n", encoding="utf-8")

        code, out = report(load(less), "less", False, 0)
        assert code == 0 and "SKIP: 3" in out[0], out
        code, out = report(load(less), "tools", True, 0)
        assert code == 1 and any("a_tests::t1 (verilator)" in l for l in out), out
        code, out = report(load(good), "tools", True, 3)
        assert code == 0, out
        code, _ = report(load(good), "tools", True, 4)
        assert code == 1
        code, _ = report(load(root / "absent.tsv"), "tools", True, 1)
        assert code == 1, "an absent ledger must not pass --min-ran"

        code, out = cover(load(less), load(good) + load(docker))
        assert code == 0, out
        code, out = cover(load(less), load(good))
        assert code == 1 and any("c_tests::t4" in l and "never ran" in l for l in out), out
        code, out = cover(load(less), load(partial) + load(docker))
        assert code == 1, out
        assert any("a_tests::t2 (sby)" in l and "not in VOLT_REQUIRE_TOOLS" in l for l in out), out
        assert any("b_tests::t3 (cc)" in l and "never ran" in l for l in out), out

        bad = root / "bad.tsv"
        bad.write_text("SKIP\tonly-three\tfields\n", encoding="utf-8")
        try:
            load(bad)
        except SystemExit:
            pass
        else:
            raise AssertionError("malformed line accepted")
    print("tool_ledger self-test: ok")
    return 0


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("report", help="summary of one job's ledger")
    r.add_argument("ledger", type=pathlib.Path)
    r.add_argument("--job", required=True)
    r.add_argument("--require-zero-skips", action="store_true")
    r.add_argument("--min-ran", type=int, default=0)
    c = sub.add_parser("cover", help="every tool test ran with its tool required")
    c.add_argument("--inventory", type=pathlib.Path, nargs="+", required=True)
    c.add_argument("--tools", type=pathlib.Path, nargs="+", required=True)
    args = ap.parse_args()
    if args.cmd == "report":
        code, out = report(load(args.ledger), args.job, args.require_zero_skips, args.min_ran)
    else:
        inventory = [e for p in args.inventory for e in load(p)]
        if not inventory:
            print("::error::the inventory ledgers are empty: the tool-less job recorded no tool test")
            return 1
        code, out = cover(inventory, [e for p in args.tools for e in load(p)])
    print("\n".join(out))
    return code


if __name__ == "__main__":
    sys.exit(main())
