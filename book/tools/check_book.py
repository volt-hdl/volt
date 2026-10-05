#!/usr/bin/env python3
"""Check the Volt book: every ```volt block compiles, prose follows the style guide.

Usage:
    python3 book/tools/check_book.py --volt target/debug/volt
    python3 book/tools/check_book.py --volt target/debug/volt --run-tests
    python3 book/tools/check_book.py --volt target/debug/volt --self-test

Code blocks (CONTRIBUTING-BOOK.md, "Code blocks"). The info string is
`volt` followed by comma-separated attributes:

    file=NAME.volt     save the block as NAME.volt in the part's workspace:
                       one directory per book directory (tour/, tutorial/,
                       ...), filled in SUMMARY.md order, like the reader's
                       project folder. Later blocks see it (a test file
                       finds its design); a later block with the same name
                       replaces it
    should_fail=CODE   `volt check` must fail with exactly this error code
    should_warn=CODE   `volt check` must pass with exactly this warning code
    test_fails         with --run-tests: `volt test` must report a failed
                       test (exit 5) instead of passing
    from=PATH          the block must equal this repository file (a template
                       or an example shown verbatim stays in sync with it)
    output             shown for reading ("You should see"), not created by
                       the reader; the theme hides its copy button

Every other ```volt block must pass `volt check` with no error and no
warning. With --run-tests, every block saved as *_test.volt also runs
`volt test` (Verilator, locally or through Volt's Docker bridge).

Prose outside code is checked for the words the style guide bans.

Guided pages (setup.md and every chapter in tour/; CONTRIBUTING-BOOK.md,
"Guided pages") follow three more rules:
    1. a command block ("Type this") has no "$ " prompt;
    2. the page opens with a "Before you start" box;
    3. every block outside a side box carries a label: "Type this", "You
       should see" (info string has `output`) or "Create this file:" /
       "Replace this file:" followed by the file name.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

BOOK_DIR = Path(__file__).resolve().parent.parent
SRC_DIR = BOOK_DIR / "src"
SELFTEST_DIR = Path(__file__).resolve().parent / "selftest"

# Same list as CONTRIBUTING-BOOK.md, "Words we do not use" (and the README).
BANNED_WORDS = [
    "revolutionary",
    "only",
    "first",
    "unique",
    "unprecedented",
    "game-changing",
    "best-in-class",
    "world-class",
    "blazing",
    "effortless",
]
BANNED_RE = re.compile(r"\b(" + "|".join(re.escape(w) for w in BANNED_WORDS) + r")\b", re.IGNORECASE)

FENCE_RE = re.compile(r"^(\s{0,3})(`{3,}|~{3,})(.*)$")
DIAG_RE = re.compile(r"^(error|warning)\[([EW]\d{4})\]", re.MULTILINE)
KNOWN_ATTRS = {"file", "should_fail", "should_warn", "test_fails", "from", "output"}

# Guided pages: the pages a beginner follows step by step.
GUIDED_DIRS = {SRC_DIR / "tour"}
GUIDED_FILES = {SRC_DIR / "setup.md"}
GUIDED_MARK = "guided page"  # a self-test fixture's top line opts in
BEFORE_BOX = '<div class="before-you-start">'
LABEL_RE = re.compile(r"\*\*(Type this|You should see|Create this file|Replace this file)( \([^)]*\))?:\*\*(.*)", re.DOTALL)
FILE_NAME_RE = re.compile(r"^\s*`([^`]+)`")
PROMPT_RE = re.compile(r"^\s*\$ ")

EXIT_TESTS_FAILED = 5


@dataclass
class Block:
    path: Path
    line: int  # 1-based line of the opening fence
    attrs: dict[str, str]
    code: str


@dataclass
class Report:
    failures: list[str] = field(default_factory=list)
    checked: int = 0
    tests_run: int = 0

    def fail(self, path: Path, line: int, msg: str, detail: str = "") -> None:
        rel = display(path)
        self.failures.append(f"{rel}:{line}: {msg}")
        # GitHub Actions annotation: shows on the PR diff.
        if os.environ.get("GITHUB_ACTIONS") == "true":
            print(f"::error file={rel},line={line}::{msg}")
        print(f"FAIL {rel}:{line}: {msg}")
        if detail:
            print("     " + detail.rstrip().replace("\n", "\n     "))


def display(path: Path) -> str:
    try:
        return path.resolve().relative_to(BOOK_DIR.parent).as_posix()
    except ValueError:
        return path.as_posix()


def parse_markdown(path: Path, report: Report) -> tuple[list[Block], list[tuple[int, str]]]:
    """Split a chapter into ```volt blocks and prose lines (outside any fence)."""
    blocks: list[Block] = []
    prose: list[tuple[int, str]] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    i = 0
    while i < len(lines):
        m = FENCE_RE.match(lines[i])
        if not m:
            prose.append((i + 1, lines[i]))
            i += 1
            continue
        indent, fence, info = m.group(1), m.group(2), m.group(3).strip()
        start = i
        body: list[str] = []
        i += 1
        while i < len(lines):
            close = lines[i].strip()
            if close.startswith(fence[0] * len(fence)) and close.strip(fence[0]) == "":
                break
            body.append(lines[i][len(indent):] if lines[i].startswith(indent) else lines[i])
            i += 1
        else:
            report.fail(path, start + 1, "code block is never closed")
        i += 1
        parts = [p.strip() for p in info.split(",") if p.strip()]
        if not parts or parts[0] != "volt":
            continue
        attrs: dict[str, str] = {}
        for p in parts[1:]:
            key, _, value = p.partition("=")
            if key not in KNOWN_ATTRS:
                report.fail(path, start + 1, f"unknown code block attribute '{key}' (known: {', '.join(sorted(KNOWN_ATTRS))})")
            attrs[key] = value
        blocks.append(Block(path, start + 1, attrs, "\n".join(body) + "\n"))
    return blocks, prose


def lint_prose(path: Path, prose: list[tuple[int, str]], report: Report) -> None:
    in_comment = False
    for line_no, text in prose:
        # Skip HTML comments and inline code: `read-only` in a command is not prose.
        if "<!--" in text:
            in_comment = "-->" not in text.split("<!--", 1)[1]
            text = text.split("<!--", 1)[0]
        elif in_comment:
            if "-->" in text:
                in_comment = False
                text = text.split("-->", 1)[1]
            else:
                continue
        text = re.sub(r"`[^`]*`", "", text)
        text = re.sub(r"\]\([^)]*\)", "]", text)  # link targets
        for m in BANNED_RE.finditer(text):
            report.fail(path, line_no, f"banned word '{m.group(1)}' (CONTRIBUTING-BOOK.md, 'Words we do not use')")


def is_guided(path: Path) -> bool:
    path = path.resolve()
    if path in {p.resolve() for p in GUIDED_FILES} or path.parent in {d.resolve() for d in GUIDED_DIRS}:
        return True
    if path.parent == SELFTEST_DIR.resolve():
        head = path.read_text(encoding="utf-8").splitlines()[:1]
        return bool(head) and GUIDED_MARK in head[0]
    return False


def lint_guided_page(path: Path, report: Report) -> None:
    """The three rules of guided pages (see the module docstring)."""
    lines = path.read_text(encoding="utf-8").splitlines()
    seen_box = False
    in_side_box = False
    i = 0
    while i < len(lines):
        text = lines[i]
        stripped = text.strip()
        if stripped == BEFORE_BOX:
            seen_box = True
            if not any(t.strip() == "**Before you start**" for t in lines[i + 1:i + 4]):
                report.fail(path, i + 1, "the 'Before you start' box starts with the line **Before you start**")
        elif stripped.startswith('<div class="box'):
            in_side_box = True
        elif stripped == "</div>":
            in_side_box = False
        m = FENCE_RE.match(text)
        if (text.startswith("## ") or m) and not seen_box:
            report.fail(path, i + 1, f"a guided page opens with a 'Before you start' box ({BEFORE_BOX}) above its first section and code block")
            seen_box = True  # report it once
        if not m:
            i += 1
            continue
        fence, info = m.group(2), m.group(3).strip()
        start = i
        body: list[str] = []
        i += 1
        while i < len(lines) and not (lines[i].strip().startswith(fence) and lines[i].strip().strip(fence[0]) == ""):
            body.append(lines[i])
            i += 1
        i += 1
        # Side boxes are skippable reading; their code is never a step.
        if not in_side_box:
            check_block_label(path, lines, start, info, body, report)


def check_block_label(path: Path, lines: list[str], start: int, info: str, body: list[str], report: Report) -> None:
    paragraph: list[str] = []
    j = start - 1
    while j >= 0 and not lines[j].strip():
        j -= 1
    while j >= 0 and lines[j].strip() and not FENCE_RE.match(lines[j]):
        paragraph.insert(0, lines[j].strip())
        j -= 1
    label = LABEL_RE.search(" ".join(paragraph))
    parts = [p.strip() for p in info.split(",") if p.strip()]
    is_output = "output" in parts[1:]
    line = start + 1
    if not label:
        report.fail(path, line, "block without a label: the paragraph above it says **Type this:**, **You should see:**, "
                                "or **Create this file:** / **Replace this file:** followed by the file name")
        return
    kind, rest = label.group(1), label.group(3)
    if kind == "Type this":
        if is_output:
            report.fail(path, line, "a 'Type this' block is a command: drop `output` from its info string")
        for k, text in enumerate(body):
            if PROMPT_RE.match(text):
                report.fail(path, line + 1 + k, "command block line starts with the prompt '$ ': write the command alone "
                                                "and put its output in a 'You should see' block")
    elif kind == "You should see":
        if not is_output:
            report.fail(path, line, "a 'You should see' block needs `output` in its info string (```text,output) "
                                    "so that the theme hides its copy button")
    else:
        name = FILE_NAME_RE.match(rest)
        if not name:
            report.fail(path, line, f"'{kind}:' is followed by the file name in backticks, e.g. **{kind}:** `crossing.volt`")
        elif parts and parts[0] == "volt":
            attrs = dict(p.partition("=")[::2] for p in parts[1:])
            if attrs.get("file") != name.group(1):
                report.fail(path, line, f"the label names `{name.group(1)}` but the block is file={attrs.get('file')}")
        if is_output:
            report.fail(path, line, f"a '{kind}' block is created by the reader: drop `output` from its info string")


# Where `volt test` runs Verilator: set explicitly (never the automatic
# fallback), so a missing Verilator fails the check instead of silently
# starting Docker containers. `--backend docker` opts in.
TOOL_BACKEND = "local"


def run(cmd: list[str], cwd: Path) -> tuple[int, str]:
    env = dict(os.environ, NO_COLOR="1", VOLT_LANG="en", VOLT_TOOL_BACKEND=TOOL_BACKEND)
    proc = subprocess.run(cmd, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    return proc.returncode, proc.stdout.decode("utf-8", errors="replace")


def check_block(volt: str, block: Block, workdir: Path, index: int, run_tests: bool, report: Report) -> None:
    name = block.attrs.get("file")
    if name is not None:
        if not re.fullmatch(r"[A-Za-z0-9_]+\.volt", name):
            report.fail(block.path, block.line, f"file={name}: expected NAME.volt")
            return
        target_dir = workdir
    else:
        # An unnamed block stands alone: it neither sees nor hides other blocks.
        name = "block.volt"
        target_dir = workdir / f"unnamed-{index}"
        target_dir.mkdir()
    (target_dir / name).write_text(block.code, encoding="utf-8")

    origin = block.attrs.get("from")
    if origin is not None:
        source = BOOK_DIR.parent / origin
        if not source.is_file():
            report.fail(block.path, block.line, f"from={origin}: no such file in the repository")
            return
        if source.read_text(encoding="utf-8").replace("\r\n", "\n") != block.code:
            report.fail(block.path, block.line, f"from={origin}: the block differs from the file; copy it again")
            return

    code, out = run([volt, "check", name], target_dir)
    report.checked += 1
    found = DIAG_RE.findall(out)
    errors = sorted({c for kind, c in found if kind == "error"})
    warnings = sorted({c for kind, c in found if kind == "warning"})
    want_error = block.attrs.get("should_fail")
    want_warning = block.attrs.get("should_warn")
    label = f"{name}" + (f" (should_fail={want_error})" if want_error else "")

    if want_error:
        if code == 0 or errors != [want_error]:
            got = ", ".join(errors) or "no error"
            report.fail(block.path, block.line, f"{label}: expected volt check to fail with {want_error}, got exit {code} and {got}", out)
            return
    else:
        if code != 0 or errors:
            report.fail(block.path, block.line, f"{label}: volt check failed (exit {code})", out)
            return
        expected_warnings = [want_warning] if want_warning else []
        if warnings != expected_warnings:
            got = ", ".join(warnings) or "no warning"
            want = want_warning or "no warning"
            report.fail(block.path, block.line, f"{label}: expected {want}, got {got}", out)
            return
    print(f"ok   {display(block.path)}:{block.line}: {label}")

    if run_tests and name.endswith("_test.volt") and not want_error:
        code, out = run([volt, "test", name], target_dir)
        report.tests_run += 1
        want_fail = "test_fails" in block.attrs
        if want_fail and code != EXIT_TESTS_FAILED:
            report.fail(block.path, block.line, f"{name}: expected a failing test (exit {EXIT_TESTS_FAILED}), got exit {code}", out)
        elif not want_fail and code != 0:
            report.fail(block.path, block.line, f"{name}: volt test failed (exit {code})", out)
        else:
            print(f"ok   {display(block.path)}:{block.line}: volt test {name}" + (" (fails as expected)" if want_fail else ""))


def check_book(volt: str, files: list[Path], run_tests: bool) -> Report:
    report = Report()
    unnamed = 0
    with tempfile.TemporaryDirectory(prefix="volt-book-") as tmp:
        workdirs: dict[Path, Path] = {}
        for path in files:
            blocks, prose = parse_markdown(path, report)
            lint_prose(path, prose, report)
            if is_guided(path):
                lint_guided_page(path, report)
            # One workspace per book directory: the chapters of a part build
            # on the same project, as the reader's own folder does.
            workdir = workdirs.get(path.parent)
            if workdir is None:
                workdir = Path(tmp) / f"part-{len(workdirs)}"
                workdir.mkdir()
                workdirs[path.parent] = workdir
            for block in blocks:
                unnamed += 1
                check_block(volt, block, workdir, unnamed, run_tests, report)
    return report


def book_files(report: Report) -> list[Path]:
    """The chapters in SUMMARY.md order, then the style guide. A chapter file
    missing from SUMMARY.md would never be published: that is an error."""
    summary = SRC_DIR / "SUMMARY.md"
    files = [summary.resolve()]
    for m in re.finditer(r"\]\(([^)]+\.md)\)", summary.read_text(encoding="utf-8")):
        files.append((SRC_DIR / m.group(1)).resolve())
    listed = set(files)
    for path in sorted(SRC_DIR.rglob("*.md")):
        if path.resolve() not in listed:
            report.fail(path, 1, "chapter is not listed in SUMMARY.md")
    files.append(BOOK_DIR / "CONTRIBUTING-BOOK.md")
    return files


def self_test(volt: str) -> int:
    """Each fixture states its expected outcome on its top line: the checker
    must accept `expect: pass` and reject `expect: fail`."""
    bad = 0
    fixtures = sorted(SELFTEST_DIR.glob("*.md"))
    for path in fixtures:
        head = path.read_text(encoding="utf-8").splitlines()[0]
        m = re.search(r"expect: (pass|fail)", head)
        if not m:
            print(f"FAIL {display(path)}: top line must say 'expect: pass' or 'expect: fail'")
            bad += 1
            continue
        print(f"--- self-test {display(path)} (expect {m.group(1)})")
        report = check_book(volt, [path], run_tests=False)
        passed = not report.failures
        if passed != (m.group(1) == "pass"):
            print(f"SELF-TEST FAILED: {display(path)} should {m.group(1)}")
            bad += 1
    print(f"self-test: {len(fixtures) - bad}/{len(fixtures)} fixtures behaved as expected")
    return 1 if bad or not fixtures else 0


def main() -> int:
    # Compiler output holds box-drawing characters; a Windows console code
    # page cannot print them.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    ap =argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--volt", default="volt", help="path to the volt binary")
    ap.add_argument("--run-tests", action="store_true", help="also run `volt test` on *_test.volt blocks")
    ap.add_argument("--self-test", action="store_true", help="check the checker against book/tools/selftest/")
    ap.add_argument("--backend", choices=["local", "docker"], default="local",
                    help="where `volt test` runs Verilator (VOLT_TOOL_BACKEND; default local: a missing Verilator fails)")
    args = ap.parse_args()
    global TOOL_BACKEND
    TOOL_BACKEND = args.backend
    # Blocks run in their own directories: a relative binary path must not
    # depend on the working directory.
    volt = shutil.which(args.volt) if os.sep not in args.volt and "/" not in args.volt else str(Path(args.volt).resolve())
    if not volt or not Path(volt).exists():
        print(f"volt binary not found: {args.volt}")
        return 2
    args.volt = volt

    if args.self_test:
        return self_test(args.volt)

    listing = Report()
    files = book_files(listing)
    report = check_book(args.volt, files, args.run_tests)
    report.failures[:0] = listing.failures
    print()
    print(f"{report.checked} volt block(s) checked, {report.tests_run} test file(s) run, {len(report.failures)} failure(s)")
    if report.checked == 0:
        print("no ```volt block found: the check is not running on the book")
        return 1
    return 1 if report.failures else 0


if __name__ == "__main__":
    sys.exit(main())
