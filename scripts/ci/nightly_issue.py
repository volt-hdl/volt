#!/usr/bin/env python3
"""Open or update the issue of a red metamorphic night (ADR-0117 section 5, #254).

`metamorphic-nightly.yml` runs this when the nightly metamorphic test is
red. It looks for the open issue with the fixed title (an exact title
match, not GitHub's fuzzy search): when there is one it adds the report as
a comment, otherwise it opens the issue. The body names the run, the
night's seed with the command that repeats the run, and the two reports
(a missing report is said, not skipped). A `gh` failure fails the step: a
red night never ends without its issue silently.

    python3 scripts/ci/nightly_issue.py report --run-url URL --seed SEED \\
        --report reports/volt-real-bugs.txt --report reports/volt-metamorphic.txt
    python3 scripts/ci/nightly_issue.py --self-test

The self-test runs the `report` path against a fake `gh` (a Python script
that records its arguments and answers `issue list` from a file).
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

TITLE = "Nightly metamorphic test is red (ADR-0117)"
REPORT_LIMIT = 30000  # bytes of each report in the body


class GhError(Exception):
    pass


def gh(cmd: list[str], *args: str) -> str:
    """Runs `gh` (`cmd`, so the self-test can give a fake) and returns stdout."""
    p = subprocess.run([*cmd, *args], capture_output=True, text=True)
    if p.returncode != 0:
        raise GhError(f"gh {' '.join(args[:2])} failed (exit {p.returncode}): {p.stderr.strip()}")
    return p.stdout


def body(run_url: str, seed: str, reports: list[Path]) -> str:
    seed = seed.strip() or "unknown"
    out = [
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


def report(run_url: str, seed: str, reports: list[Path], cmd: list[str], title: str = TITLE) -> str:
    """Comments on the open issue or opens one; returns what it did."""
    with tempfile.TemporaryDirectory() as tmp:
        f = Path(tmp) / "body.md"
        f.write_bytes(body(run_url, seed, reports).encode("utf-8"))
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

        def run(listed, fail=None, reports=(rb, mm), seed="2026-11-16"):
            log = tmp / "log.jsonl"
            log.unlink(missing_ok=True)
            (tmp / "list.json").write_text(json.dumps(listed), encoding="utf-8")
            os.environ.update(FAKE_GH_LOG=str(log), FAKE_GH_LIST=str(tmp / "list.json"))
            os.environ.pop("FAKE_GH_FAIL", None)
            if fail:
                os.environ["FAKE_GH_FAIL"] = fail
            try:
                did = report("https://run/1", seed, list(reports), cmd)
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

    for k in ("FAKE_GH_LOG", "FAKE_GH_LIST", "FAKE_GH_FAIL"):
        os.environ.pop(k, None)
    if failures:
        for f in failures:
            print(f"nightly_issue self-test FAIL {f}")
        return 1
    print("nightly_issue self-test: ok (5 cases against a fake gh)")
    return 0


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("report")
    r.add_argument("--run-url", required=True)
    r.add_argument("--seed", default="")
    r.add_argument("--report", type=Path, action="append", default=[])
    a = ap.parse_args()
    try:
        did = report(a.run_url, a.seed, a.report, ["gh"])
    except GhError as err:
        print(f"::error title=Metamorphic nightly::{err}")
        return 1
    print(f"::notice title=Metamorphic nightly::{did}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
