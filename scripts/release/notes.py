#!/usr/bin/env python3
"""Release notes of a Volt release (ADR-0105).

Usage:
    python3 scripts/release/notes.py --check
    python3 scripts/release/notes.py --build X.Y.Z OUT
    python3 scripts/release/notes.py --self-test

The notes of release X.Y.Z are docs/release-notes/vX.Y.Z.md, in English.
--check checks every such file, --build checks one and writes the body of
the GitHub Release to OUT (release.yml, in the dry run too). The rules:

    1. the file is named vX.Y.Z.md and its top line is `# Volt X.Y.Z`
       (the Release is titled "Volt X.Y.Z", so the line is left out of
       the body);
    2. the earliest `##` heading is `## Behavior changes` and the section
       is not empty: "None." when nothing changed (ADR-0100);
    3. prose outside code uses none of the words the style guide bans
       (the list of book/tools/check_book.py);
    4. the body fits in a GitHub Release: at most 125000 characters.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
NOTES_DIR = ROOT / "docs" / "release-notes"
sys.dont_write_bytecode = True  # no __pycache__ in book/tools: the checkout stays clean
sys.path.insert(0, str(ROOT / "book" / "tools"))
import check_book  # noqa: E402  (the banned word list and the prose splitter)

NAME_RE = re.compile(r"^v(\d+\.\d+\.\d+)\.md$")
MAX_BODY = 125000
BEHAVIOR = "## Behavior changes"


def check(path: Path, report: check_book.Report) -> str:
    """Check one notes file; return the Release body (title line removed)."""
    m = NAME_RE.match(path.name)
    if not m:
        report.fail(path, 1, "release notes are named vX.Y.Z.md")
        return ""
    version = m.group(1)
    lines = path.read_text(encoding="utf-8").splitlines()
    title = f"# Volt {version}"
    if not lines or lines[0].rstrip() != title:
        report.fail(path, 1, f"the top line must be '{title}'")

    _, prose = check_book.parse_markdown(path, report)
    headings = [(n, t.rstrip()) for n, t in prose if t.startswith("## ")]
    if not headings or headings[0][1] != BEHAVIOR:
        report.fail(path, headings[0][0] if headings else 1, f"the earliest '##' section must be '{BEHAVIOR}' (ADR-0100)")
    else:
        start = headings[0][0]
        end = headings[1][0] if len(headings) > 1 else len(lines) + 1
        if not any(lines[i - 1].strip() for i in range(start + 1, end)):
            report.fail(path, start, "the Behavior changes section is empty: write 'None.' when nothing changed (ADR-0100)")
    check_book.lint_prose(path, prose, report)

    body = "\n".join(lines[1:]).lstrip("\n") + "\n"
    if len(body) > MAX_BODY:
        report.fail(path, 1, f"the body is {len(body)} characters; a GitHub Release takes at most {MAX_BODY}")
    return body


def run_check(files: list[Path]) -> int:
    report = check_book.Report()
    for path in files:
        check(path, report)
    print(f"{len(files)} release notes file(s) checked, {len(report.failures)} failure(s)")
    return 1 if report.failures else 0


def run_build(version: str, out: Path) -> int:
    path = NOTES_DIR / f"v{version}.md"
    if not path.is_file():
        print(f"::error::{path.relative_to(ROOT).as_posix()} is missing: every release has its notes there (RELEASING.md)")
        return 1
    report = check_book.Report()
    body = check(path, report)
    if report.failures:
        return 1
    out.write_text(body, encoding="utf-8", newline="\n")
    print(f"{out}: {len(body)} characters from {path.relative_to(ROOT).as_posix()}")
    return 0


GOOD = "# Volt 1.2.3\n\nIntro.\n\n## Behavior changes\n\nNone.\n\n## Highlights\n\nText with `only` in code.\n"
SELF_TESTS = [
    # (file name, content, expected to pass)
    ("v1.2.3.md", GOOD, True),
    ("v1.2.3.md", GOOD.replace("## Highlights", "```text\n## Fenced\n```\n\n## Highlights"), True),
    ("1.2.3.md", GOOD, False),
    ("v1.2.3.md", GOOD.replace("# Volt 1.2.3", "# Volt 1.2.4"), False),
    ("v1.2.3.md", GOOD.replace("# Volt 1.2.3\n\n", ""), False),
    ("v1.2.3.md", GOOD.replace("Intro.", "## Intro\n\nText."), False),
    ("v1.2.3.md", GOOD.replace("## Behavior changes\n\nNone.\n\n", ""), False),
    ("v1.2.3.md", GOOD.replace("None.", ""), False),
    ("v1.2.3.md", GOOD.replace("Intro.", "The first release."), False),
    ("v1.2.3.md", GOOD + "x" * MAX_BODY + "\n", False),
]


def self_test() -> int:
    # The expected failures must not become GitHub annotations.
    os.environ.pop("GITHUB_ACTIONS", None)
    wrong = 0
    with tempfile.TemporaryDirectory() as tmp:
        for i, (name, content, should_pass) in enumerate(SELF_TESTS):
            case = Path(tmp) / str(i)
            case.mkdir()
            path = case / name
            path.write_text(content, encoding="utf-8")
            report = check_book.Report()
            check(path, report)
            if (not report.failures) != should_pass:
                print(f"SELF-TEST case {i} ({name}): expected {'pass' if should_pass else 'failure'}")
                wrong += 1
    print(f"self-test: {len(SELF_TESTS)} cases, {wrong} wrong")
    return 1 if wrong else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="check every docs/release-notes/v*.md")
    mode.add_argument("--build", nargs=2, metavar=("VERSION", "OUT"), help="check vVERSION.md and write the Release body to OUT")
    mode.add_argument("--self-test", action="store_true", help="check the checker on built-in cases")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.build:
        return run_build(args.build[0], Path(args.build[1]))
    return run_check(sorted(NOTES_DIR.glob("*.md")))


if __name__ == "__main__":
    sys.exit(main())
