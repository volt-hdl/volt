#!/usr/bin/env python3
"""Print the summary of the differential test (ADR-0108) in a CI log.

The test (crates/volt-driver/tests/difftest_tests.rs) appends its report
to the file named by VOLT_DIFFTEST_SUMMARY. This step prints it, puts the
seed/count line in a notice and fails when the file is missing (the test
never ran) or the report counts a difference.

    difftest_summary.py FILE --job NAME
    difftest_summary.py --self-test
"""

import argparse
import re
import sys
import tempfile
from pathlib import Path

LINE = re.compile(r"^difftest: seed=(\d+) target=(\d+) generated=(\d+) accepted=(\d+) .* diffs=(\d+)$")


def summarize(path: Path, job: str) -> int:
    if not path.is_file() or path.stat().st_size == 0:
        print(f"::error title=Differential test [{job}]::no summary at {path}: the test did not run")
        return 1
    text = path.read_text(encoding="utf-8")
    print(text, end="" if text.endswith("\n") else "\n")
    lines = [m for m in (LINE.match(l) for l in text.splitlines()) if m]
    if not lines:
        print(f"::error title=Differential test [{job}]::no 'difftest: seed=...' line in {path}")
        return 1
    status = 0
    for m in lines:
        seed, target, generated, accepted, diffs = (int(g) for g in m.groups())
        msg = f"seed={seed} target={target} generated={generated} accepted={accepted} diffs={diffs}"
        if diffs or accepted < target:
            print(f"::error title=Differential test [{job}]::{msg}")
            status = 1
        else:
            print(f"::notice title=Differential test [{job}]::{msg}")
    return status


def self_test() -> int:
    ok = "difftest: seed=7 target=10 generated=12 accepted=10 rejected-as-predicted=1 unsupported=0 unspecified=1 values-compared=99 diffs=0\n"
    bad = ok.replace("diffs=0", "diffs=2")
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "s.txt"
        assert summarize(p, "t") == 1, "missing file must fail"
        p.write_text(ok, encoding="utf-8")
        assert summarize(p, "t") == 0
        p.write_text(bad, encoding="utf-8")
        assert summarize(p, "t") == 1
        p.write_text("nothing\n", encoding="utf-8")
        assert summarize(p, "t") == 1
    print("difftest_summary self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("file", nargs="?")
    ap.add_argument("--job", default="ci")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if not a.file:
        ap.error("FILE is required")
    return summarize(Path(a.file), a.job)


if __name__ == "__main__":
    sys.exit(main())
