#!/usr/bin/env python3
"""Open or update the issue of a red nightly run (ADR-0117 section 5, #254;
ADR-0108, #255).

Two nightly workflows run this when they are red, each with its kind:
`metamorphic-nightly.yml` (`--kind metamorphic`) and the nightly run of
`difftest.yml` (`--kind difftest`). It looks for the open issue with the
kind's fixed title (an exact title match, not GitHub's fuzzy search): when
there is one it adds the report as a comment, otherwise it opens the
issue. The body names the run, the night's seed with the commands that
repeat the run, and the reports (a missing report is said, not skipped).
A `gh` failure fails the step: a red night never ends without its issue
silently.

    python3 scripts/ci/nightly_issue.py report --kind metamorphic \\
        --run-url URL --seed SEED \\
        --report reports/volt-real-bugs.txt --report reports/volt-metamorphic.txt
    python3 scripts/ci/nightly_issue.py report --kind difftest \\
        --run-url URL --seed 2026111800 --count 12500 --shards 4 \\
        --report reports/volt-difftest-shard-0.txt ...
    python3 scripts/ci/nightly_issue.py --self-test

The self-test runs the `report` path of both kinds against a fake `gh` (a
Python script that records its arguments and answers `issue list` from a
file).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ in the checkout

TITLES = {
    "metamorphic": "Nightly metamorphic test is red (ADR-0117)",
    "difftest": "Nightly differential test is red (ADR-0108)",
}
LABELS = {"metamorphic": "Metamorphic nightly", "difftest": "Differential nightly"}
TITLE = TITLES["metamorphic"]
REPORT_LIMIT = 30000  # bytes of each report in the body


class GhError(Exception):
    pass


def gh(cmd: list[str], *args: str) -> str:
    """Runs `gh` (`cmd`, so the self-test can give a fake) and returns stdout."""
    p = subprocess.run([*cmd, *args], capture_output=True, text=True)
    if p.returncode != 0:
        raise GhError(f"gh {' '.join(args[:2])} failed (exit {p.returncode}): {p.stderr.strip()}")
    return p.stdout


def metamorphic_head(run_url: str, seed: str) -> list[str]:
    return [
        f"The nightly metamorphic test failed: {run_url}\n",
        f"Seed: `{seed}`. Repeat this run:\n",
        "```text",
        f"VOLT_METAMORPHIC_TIER=nightly VOLT_METAMORPHIC_SEED={seed} "
        "VOLT_REQUIRE_TOOLS=verilator cargo test -p volt-driver --test metamorphic_tests",
        "```\n",
        "Every failure is either a new silent-wrong result (open an issue with the "
        "minimized design and add it to the known-failures list, or fix it) or a "
        "known failure whose issue was fixed (remove the entry).\n",
    ]


def difftest_head(run_url: str, seed: str, count: str, shards: int) -> list[str]:
    """Shard k of `difftest.yml` runs seed + k with `count` expressions."""
    out = [f"The nightly differential test failed: {run_url}\n"]
    count = count.strip() or "unknown"
    if seed.isdigit() and count.isdigit() and shards > 0:
        out += [
            f"Seed: `{seed}` (shard k uses seed + k), {count} accepted expressions per "
            "shard. Repeat a shard with the pinned Verilator (`scripts/ci/verilator.env`):\n",
            "```text",
        ]
        for k in range(shards):
            out += [
                f"# shard {k}",
                f"VOLT_DIFFTEST_SEED={int(seed) + k} VOLT_DIFFTEST_COUNT={count} "
                "VOLT_REQUIRE_TOOLS=verilator cargo test --release -p volt-driver --test difftest_tests",
            ]
        out.append("```\n")
    else:
        out.append(f"Seed: `{seed}`, count: `{count}`: no repeat command (the seed job did not run).\n")
    out.append(
        "Every difference is a finding (ADR-0108): minimize it, open a silent-wrong issue "
        "with the example, and add the example to `crates/volt-difftest/src/regress.rs` in "
        "the pull request that fixes it. A difference that only a model assumption makes "
        "is an issue with the options for the maintainer, not a fix.\n"
    )
    return out


def body(kind: str, run_url: str, seed: str, reports: list[Path], count: str = "", shards: int = 0) -> str:
    seed = seed.strip() or "unknown"
    if kind == "difftest":
        out = difftest_head(run_url, seed, count, shards)
    else:
        out = metamorphic_head(run_url, seed)
    for r in reports:
        data = r.read_bytes() if r.is_file() else b""
        if data:
            out += ["```text", data[:REPORT_LIMIT].decode("utf-8", "replace"), "```\n"]
        else:
            out.append(f"{r.name}: missing (the test did not run to its end)\n")
    return "\n".join(out)


def open_issue(title: str, cmd: list[str]) -> int | None:
    """The number of the open issue whose title is exactly `title`."""
    listed = json.loads(
        gh(cmd, "issue", "list", "--state", "open", "--search", f'"{title}" in:title',
           "--json", "number,title", "--limit", "100")
    )
    numbers = sorted(i["number"] for i in listed if i.get("title") == title)
    return numbers[0] if numbers else None


def report(
    run_url: str,
    seed: str,
    reports: list[Path],
    cmd: list[str],
    kind: str = "metamorphic",
    count: str = "",
    shards: int = 0,
) -> str:
    """Comments on the open issue of `kind` or opens one; returns what it did."""
    title = TITLES[kind]
    with tempfile.TemporaryDirectory() as tmp:
        f = Path(tmp) / "body.md"
        f.write_bytes(body(kind, run_url, seed, reports, count, shards).encode("utf-8"))
        number = open_issue(title, cmd)
        if number is not None:
            gh(cmd, "issue", "comment", str(number), "--body-file", str(f))
            return f"commented on #{number}"
        url = gh(cmd, "issue", "create", "--title", title, "--body-file", str(f)).strip()
        return f"opened {url}"


FAKE_GH = r'''
import json, os, sys
from pathlib import Path
log = Path(os.environ["FAKE_GH_LOG"])
args = sys.argv[1:]
entry = {"args": args}
if "--body-file" in args:
    entry["body"] = Path(args[args.index("--body-file") + 1]).read_text(encoding="utf-8")
with log.open("a", encoding="utf-8") as f:
    f.write(json.dumps(entry) + "\n")
if os.environ.get("FAKE_GH_FAIL") == args[0] + " " + args[1]:
    sys.stderr.write("HTTP 502\n")
    sys.exit(1)
if args[:2] == ["issue", "list"]:
    print(Path(os.environ["FAKE_GH_LIST"]).read_text(encoding="utf-8"))
elif args[:2] == ["issue", "create"]:
    print("https://github.com/o/r/issues/400")
'''


def self_test() -> int:
    failures = []

    def check(name: str, ok: bool, detail: str = "") -> None:
        if not ok:
            failures.append(f"{name}: {detail}")

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        fake = tmp / "fake_gh.py"
        fake.write_text(FAKE_GH, encoding="utf-8")
        cmd = [sys.executable, str(fake)]
        rb = tmp / "volt-real-bugs.txt"
        rb.write_text("22 reproductions; 3 failures, 2 known\n", encoding="utf-8")
        mm = tmp / "volt-metamorphic.txt"
        mm.write_text("tier nightly: 488 designs\n  seed: 2026-11-16\n", encoding="utf-8")

        def run(listed, fail=None, reports=(rb, mm), seed="2026-11-16", kind="metamorphic", count="", shards=0):
            log = tmp / "log.jsonl"
            log.unlink(missing_ok=True)
            (tmp / "list.json").write_text(json.dumps(listed), encoding="utf-8")
            os.environ.update(FAKE_GH_LOG=str(log), FAKE_GH_LIST=str(tmp / "list.json"))
            os.environ.pop("FAKE_GH_FAIL", None)
            if fail:
                os.environ["FAKE_GH_FAIL"] = fail
            try:
                did = report("https://run/1", seed, list(reports), cmd, kind, count, shards)
            except GhError as err:
                did = f"error: {err}"
            calls = [json.loads(l) for l in log.read_text(encoding="utf-8").splitlines()] if log.exists() else []
            return did, calls

        # 1. No open issue with the exact title (a similar title does not
        #    count): the issue is opened, with the seed, the command that
        #    repeats the run and both reports.
        did, calls = run([{"number": 7, "title": TITLE + " (old)"}])
        check("open", did == "opened https://github.com/o/r/issues/400", did)
        create = [c for c in calls if c["args"][:2] == ["issue", "create"]]
        check("open: one create", len(create) == 1, str(calls))
        if create:
            a, b = create[0]["args"], create[0]["body"]
            check("open: title", a[a.index("--title") + 1] == TITLE, str(a))
            check("open: seed", "Seed: `2026-11-16`" in b, b)
            check("open: repeat command", "VOLT_METAMORPHIC_SEED=2026-11-16 " in b, b)
            check("open: run url", "https://run/1" in b, b)
            check("open: reports", "22 reproductions" in b and "488 designs" in b, b)
        check("open: no comment", not any(c["args"][:2] == ["issue", "comment"] for c in calls), str(calls))

        # 2. An open issue with the exact title: a comment on it, no new issue.
        did, calls = run([{"number": 9, "title": TITLE + " (old)"}, {"number": 12, "title": TITLE}])
        check("comment", did == "commented on #12", did)
        check("comment: target", any(c["args"][:3] == ["issue", "comment", "12"] for c in calls), str(calls))
        check("comment: no create", not any(c["args"][:2] == ["issue", "create"] for c in calls), str(calls))

        # 3. A missing report (the test did not run): the body says so.
        did, calls = run([], reports=(rb, tmp / "absent.txt"), seed="")
        bodies = [c.get("body", "") for c in calls if "body" in c]
        check("missing", bool(bodies) and "absent.txt: missing" in bodies[0], str(bodies))
        check("missing: seed unknown", bool(bodies) and "Seed: `unknown`" in bodies[0], str(bodies))

        # 4. gh fails: the step fails, it never passes without the issue.
        did, _ = run([], fail="issue create")
        check("gh fails on create", did.startswith("error: gh issue create failed"), did)
        did, calls = run([], fail="issue list")
        check("gh fails on list", did.startswith("error: gh issue list failed"), did)
        check("gh fails on list: nothing written", len(calls) == 1, str(calls))

        # 5. A long report is cut.
        mm.write_text("x" * (REPORT_LIMIT + 500), encoding="utf-8")
        did, calls = run([])
        bodies = [c.get("body", "") for c in calls if "body" in c]
        check("cut", bool(bodies) and "x" * REPORT_LIMIT in bodies[0] and "x" * (REPORT_LIMIT + 1) not in bodies[0], "")

        # 6. The differential test's kind: its own title (the metamorphic
        #    issue is open and is not touched), the seed, the command of
        #    every shard (seed + k, the night's count) and every report.
        dt = TITLES["difftest"]
        shard = []
        for k in range(4):
            f = tmp / f"volt-difftest-shard-{k}.txt"
            f.write_text(f"difftest: seed={2026111800 + k} target=12500 diffs={k}\n", encoding="utf-8")
            shard.append(f)
        did, calls = run([{"number": 12, "title": TITLE}, {"number": 13, "title": dt + " (old)"}],
                         reports=shard, seed="2026111800", kind="difftest", count="12500", shards=4)
        check("difftest open", did == "opened https://github.com/o/r/issues/400", did)
        check("difftest: no comment", not any(c["args"][:2] == ["issue", "comment"] for c in calls), str(calls))
        create = [c for c in calls if c["args"][:2] == ["issue", "create"]]
        check("difftest: one create", len(create) == 1, str(calls))
        if create:
            a, b = create[0]["args"], create[0]["body"]
            check("difftest: title", a[a.index("--title") + 1] == dt, str(a))
            check("difftest: seed", "Seed: `2026111800` (shard k uses seed + k), 12500 accepted" in b, b)
            for k in range(4):
                check(f"difftest: shard {k} command",
                      f"# shard {k}\nVOLT_DIFFTEST_SEED={2026111800 + k} VOLT_DIFFTEST_COUNT=12500 "
                      "VOLT_REQUIRE_TOOLS=verilator cargo test --release" in b, b)
                check(f"difftest: shard {k} report", f"seed={2026111800 + k} target=12500 diffs={k}" in b, b)
            check("difftest: no fifth shard", "VOLT_DIFFTEST_SEED=2026111804" not in b, b)
            check("difftest: not the metamorphic command", "VOLT_METAMORPHIC" not in b, b)
            check("difftest: run url", "https://run/1" in b, b)

        # 7. The differential test's open issue: a comment on it, never on the
        #    metamorphic one.
        did, calls = run([{"number": 12, "title": TITLE}, {"number": 14, "title": dt}],
                         reports=shard, seed="2026111800", kind="difftest", count="12500", shards=4)
        check("difftest comment", did == "commented on #14", did)
        check("difftest comment: no create", not any(c["args"][:2] == ["issue", "create"] for c in calls), str(calls))

        # 8. The seed job did not run: no seed, no repeat command, the
        #    missing report is named.
        did, calls = run([], reports=(tmp / "volt-difftest-shard-9.txt",), seed="", kind="difftest", count="", shards=4)
        bodies = [c.get("body", "") for c in calls if "body" in c]
        b = bodies[0] if bodies else ""
        check("difftest no seed", "Seed: `unknown`, count: `unknown`: no repeat command" in b, b)
        check("difftest no seed: no command", "VOLT_DIFFTEST_SEED=" not in b, b)
        check("difftest no seed: missing report", "volt-difftest-shard-9.txt: missing" in b, b)
        #    No shard count: no repeat command either (never an empty block).
        did, calls = run([], reports=shard, seed="2026111800", kind="difftest", count="12500", shards=0)
        bodies = [c.get("body", "") for c in calls if "body" in c]
        b = bodies[0] if bodies else ""
        check("difftest no shards", "no repeat command" in b and "# shard" not in b, b)

    for k in ("FAKE_GH_LOG", "FAKE_GH_LIST", "FAKE_GH_FAIL"):
        os.environ.pop(k, None)
    if failures:
        for f in failures:
            print(f"nightly_issue self-test FAIL {f}")
        return 1
    print("nightly_issue self-test: ok (8 cases against a fake gh)")
    return 0


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("report")
    r.add_argument("--kind", required=True, choices=sorted(TITLES))
    r.add_argument("--run-url", required=True)
    r.add_argument("--seed", default="")
    r.add_argument("--count", default="", help="difftest: accepted expressions per shard")
    r.add_argument("--shards", type=int, default=0, help="difftest: shard k runs seed + k")
    r.add_argument("--report", type=Path, action="append", default=[])
    a = ap.parse_args()
    label = LABELS[a.kind]
    try:
        did = report(a.run_url, a.seed, a.report, ["gh"], a.kind, a.count, a.shards)
    except GhError as err:
        print(f"::error title={label}::{err}")
        return 1
    print(f"::notice title={label}::{did}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
