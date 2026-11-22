#!/usr/bin/env python3
"""Every exclude_re entry matches exactly the mutants it was written for (#239).

An `exclude_re` entry of `.cargo/mutants.toml` (ADR-0109 section 4) is a
regex over mutant names. When the code moves, an entry can drift both ways:
it stops matching its mutant, which comes back as missed; or it starts to
match a mutant nobody classified (a new `<` in the same function), which
then silently leaves the measurement.

So each entry names the mutants it was written for, one comment line each,
between its reason and the entry, as `cargo mutants --list` prints them:

    # A1. ...reason...
    # expect: crates/volt-sv-emit/src/let_width.rs:262:14: replace < with <= in demand
    "volt-sv-emit/src/let_width\\.rs:\\d+:\\d+: replace < with <= in demand$",

An entry is tied to the function and the change, never to a line
(ADR-0133). Its regex has the form `<file>\\.rs:\\d+:\\d+: <change> in
<function>$`: the line is always `\\d+`. Where the same change occurs more
than once in the function and the entry is written for fewer of them, the
column tells them apart: `<file>\\.rs:\\d+:93: ...`. The column does not
move when lines are added or removed above the statement; it moves when
the statement's own line is edited, and then the entry is looked at again.
An `expect:` line is compared by file, column, change and function; its
line number is information only. When the code moves, the check still
passes and prints the new line as a notice; `check --fix-lines` writes the
new line numbers into the configuration.

`check` takes the list of every mutant in scope before the exclusions
(`cargo mutants --list` with a copy of the configuration whose exclude_re
is empty; `--no-config` would drop examine_globs too) and applies each
entry as cargo-mutants does (Python `re.search` over the name). It fails
when an entry's regex pins a line or has another form, when it has no
`expect:` line, matches no mutant, misses an expected mutant, or matches
one that is not expected (an ambiguity: the same change in the same
function, which the entry was not written for); for a mutant it misses it
shows the mutants with the same file, change and function. cargo-mutants
27.1.0 lists field-deletion mutants whatever the filters (ADR-0109
"Hedefli koşu"); the check filters names in Python, so they are covered
too. It runs only with the cargo-mutants version that mutants.yml pins
(CARGO_MUTANTS_VERSION): the names and the list depend on the version.

The limit: the check knows a mutant by file, column, change and function.
If the classified statement goes away and another one with the same change,
in the same function, lands on exactly its column, the check still passes.

C entries (#283). ADR-0109 section 4's group C (suspect: the mutant
suggests that today's behavior contradicts the specification) is not fixed
in the pull request that finds it; it gets a "silent-wrong?" issue, and the
mutant waits for that issue in an entry of its own. The reason opens with
the class label `C.` (or `C,`, `C (...)`), and one `# issue: #N` line,
next to the `# expect:` lines, names the issue:

    # C. ...why the mutant suggests a spec violation...
    # issue: #301
    # expect: crates/volt-hir/src/x.rs:12:3: replace + with - in g
    "volt-hir/src/x\\.rs:\\d+:\\d+: replace \\+ with - in g$",

`check` fails when a C entry has no `# issue:` line or more than one, when
an A entry has one, when the issue is closed, does not exist or is a pull
request (a), and, with `--pr N`, when pull request N closes the issue
(closing keywords such as `Closes #301`, or linked by hand) while the entry
still stands (b): the pull request that fixes the bug removes the entry and
the gate tries the mutant again. It reads GitHub with GITHUB_TOKEN and
GITHUB_REPOSITORY; without them, or without a network, it fails instead of
passing. With `--pr` it always asks GitHub, whether or not a C entry exists.
`--offline` (`just check`) does not read GitHub: the issue states are not
checked, and the output says so; the workflow always reads them.
`--if-installed` (`just check`): when the pinned cargo-mutants is not
installed (none, or another version), the check is skipped, the output
says so with the install command, and the exit code is 0; the workflow
runs without it and fails instead.

    mutants_exclusions.py check [--config .cargo/mutants.toml] [--list FILE] [--markdown FILE]
                                [--pr N | --offline] [--fix-lines] [--if-installed]
    mutants_exclusions.py list --out FILE [--config .cargo/mutants.toml] -- [CARGO MUTANTS ARGS...]
    mutants_exclusions.py --self-test

`list` writes the `cargo mutants --list` of the run before the exclusions;
the gate's summary lists the mutants on the changed lines that a C entry
excludes, with their issue.
"""

import argparse
import contextlib
import io
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path

EXPECT = re.compile(r"^\s*#\s*expect:\s*(?P<name>\S.*?)\s*$")
ISSUE = re.compile(r"^\s*#\s*issue:\s*(?P<value>.*?)\s*$")
ISSUE_VALUE = re.compile(r"^#(?P<n>[1-9][0-9]*)$")
# The class label that opens a C entry's reason: `C.`, `C,` or `C (...)`.
C_LABEL = re.compile(r"^\s*#\s*C(\s*\([^)]*\))?[.,]\s")
ENTRY = re.compile(r'^\s*(?P<str>"(?:[^"\\]|\\.)*")\s*,?\s*(#.*)?$')
START = re.compile(r"^\s*exclude_re\s*=\s*\[\s*$")
LINECOL = re.compile(r"^(?P<file>[^:]+):(?P<line>\d+):(?P<col>\d+): (?P<rest>.+)$")
# The form of an entry's regex (ADR-0133): the line is `\d+`, never a
# number; the column is `\d+` or, to tell the same change in the same
# function apart, a number; the regex ends with `$`.
SHAPE = re.compile(r"^[^:]+\\\.rs:\\d\+:(\\d\+|[1-9][0-9]*): .+\$$")
SHAPE_HELP = (
    "the regex has the form `<file>\\.rs:\\d+:\\d+: <change> in <function>$`: it never pins a line"
    " (ADR-0133); where the same change occurs more than once in the function, the column"
    " (`\\d+:<col>`) tells them apart"
)
PINNED_VERSION = re.compile(r"^\s*CARGO_MUTANTS_VERSION:\s*(?P<v>\S+)\s*$", re.M)


class ConfigError(Exception):
    pass


def parse(text: str) -> list[dict]:
    """The exclude_re entries with their expect: names, in file order."""
    lines = text.splitlines()
    try:
        start = next(i for i, l in enumerate(lines) if START.match(l))
    except StopIteration:
        raise ConfigError("no `exclude_re = [` line (one entry per line)")
    entries, pending, at, issues, is_c = [], [], [], [], False
    for no, line in enumerate(lines[start + 1 :], start + 2):
        s = line.strip()
        if s == "]":
            break
        if not s:
            continue
        m = EXPECT.match(line)
        if m:
            pending.append(m["name"])
            at.append(no)
            continue
        m = ISSUE.match(line)
        if m:
            issues.append(m["value"])
            continue
        if s.startswith("#"):
            is_c = is_c or bool(C_LABEL.match(line))
            continue
        m = ENTRY.match(line)
        if not m:
            raise ConfigError(f"line {no}: not one string per line: {s}")
        regex = tomllib.loads(f"v = {m['str']}")["v"]
        entries.append(
            {"regex": regex, "line": no, "expect": pending, "expect_at": at, "issues": issues, "c": is_c}
        )
        pending, at, issues, is_c = [], [], [], False
    else:
        raise ConfigError("exclude_re has no closing `]` line")
    if pending or issues:
        raise ConfigError(f"expect: or issue: lines after the last entry: {pending + issues}")
    parsed = tomllib.loads(text).get("exclude_re", [])
    if [e["regex"] for e in entries] != parsed:
        raise ConfigError("the line-by-line reading of exclude_re differs from the TOML value")
    return entries


def without_linecol(name: str) -> str:
    """File, change and function: the mutants a moved one may have become."""
    m = LINECOL.match(name)
    return f"{m['file']}: {m['rest']}" if m else name


def key(name: str) -> tuple:
    """What tells a mutant apart (ADR-0133): file, column, change and
    function. The line is information only."""
    m = LINECOL.match(name)
    return (m["file"], int(m["col"]), m["rest"]) if m else (name, 0, "")


def line_of(name: str) -> int:
    m = LINECOL.match(name)
    return int(m["line"]) if m else 0


def where(e: dict) -> str:
    return f"exclude_re entry at line {e['line']} ({e['regex']})"


def check(entries: list[dict], names: list[str]) -> list[str]:
    """The errors: an empty list when every entry has the form of ADR-0133
    and matches exactly its expect: mutants (by file, column, change and
    function)."""
    errors = []
    loose = {}
    for n in names:
        loose.setdefault(without_linecol(n), []).append(n)
    for e in entries:
        w = where(e)
        if not SHAPE.match(e["regex"]):
            errors.append(f"{w}: {SHAPE_HELP}")
        rx = re.compile(e["regex"])
        matched = [n for n in names if rx.search(n)]
        expected = e["expect"]
        if not expected:
            errors.append(f"{w}: no `# expect:` line names the mutants it was written for")
        if not matched:
            errors.append(f"{w}: matches no mutant")
        want, got = Counter(map(key, expected)), Counter(map(key, matched))
        missing = want - got
        for n in expected:
            if missing[key(n)] > 0:
                missing[key(n)] -= 1
                now = loose.get(without_linecol(n), [])
                hint = f"; same file, change and function now: {', '.join(now)}" if now else ""
                errors.append(f"{w}: does not match the expected mutant {n}{hint}")
        extra = got - want
        for k in sorted(extra, key=lambda k: min(line_of(n) for n in matched if key(n) == k)):
            same = [n for n in matched if key(n) == k]
            if want[k] == 0:
                for n in same:
                    count = len(loose.get(without_linecol(n), []))
                    hint = (
                        f"; the same change occurs {count} times in the function: pin the column of the"
                        " one the entry was written for (`\\d+:<col>`)"
                        if count > 1
                        else ""
                    )
                    errors.append(f"{w}: matches a mutant it was not written for: {n}{hint}")
            else:
                errors.append(
                    f"{w}: matches {got[k]} mutants with the same file, column, change and function and was"
                    f" written for {want[k]}: the entry cannot tell them apart: {', '.join(same)}"
                )
    return errors


def moved_lines(entries: list[dict], names: list[str]) -> list[tuple[dict, int, str, str]]:
    """(entry, index of the expect: line, its name, today's name) for every
    expected mutant of a matching entry whose line number has changed."""
    moves = []
    for e in entries:
        rx = re.compile(e["regex"])
        matched = [n for n in names if rx.search(n)]
        if not e["expect"] or Counter(map(key, e["expect"])) != Counter(map(key, matched)):
            continue
        today = {}
        for n in sorted(matched, key=line_of):
            today.setdefault(key(n), []).append(n)
        for i in sorted(range(len(e["expect"])), key=lambda i: line_of(e["expect"][i])):
            old = e["expect"][i]
            new = today[key(old)].pop(0)
            if new != old:
                moves.append((e, i, old, new))
    return moves


def fix_lines(text: str, moves: list[tuple[dict, int, str, str]]) -> str:
    """The configuration with today's line numbers in the moved expect: lines."""
    lines = text.splitlines(keepends=True)
    for e, i, old, new in moves:
        no = e["expect_at"][i] - 1
        if old not in lines[no]:
            raise ConfigError(f"line {no + 1}: expected `{old}` there")
        lines[no] = lines[no].replace(old, new, 1)
    return "".join(lines)


def issue_number(e: dict) -> int | None:
    """The issue of a well-formed C entry; None otherwise (class_errors says why)."""
    if not e["c"] or len(e["issues"]) != 1:
        return None
    m = ISSUE_VALUE.match(e["issues"][0])
    return int(m["n"]) if m else None


def class_errors(entries: list[dict]) -> list[str]:
    """#283: a C entry names exactly one issue, `# issue: #N`; an A entry names none."""
    errors = []
    for e in entries:
        if e["c"] and len(e["issues"]) != 1:
            count = "none" if not e["issues"] else str(len(e["issues"]))
            errors.append(f"{where(e)}: a C entry names its open issue on one `# issue: #N` line; it has {count}")
        elif not e["c"] and e["issues"]:
            errors.append(
                f"{where(e)}: has an `# issue:` line but no C label (`# C. <reason>`): only a C entry waits for an issue"
            )
        for v in e["issues"]:
            if not ISSUE_VALUE.match(v):
                errors.append(f"{where(e)}: `# issue: {v}` is not `# issue: #N`")
    return errors


class GitHubError(Exception):
    pass


class GitHub:
    """The issue states and the issues a pull request closes, read with the
    workflow's token (permissions: issues: read, pull-requests: read).
    Without a token or a network every call raises GitHubError: the check
    then fails, it never passes without having read the state."""

    def __init__(self, token, repo, api="https://api.github.com", graphql="https://api.github.com/graphql"):
        self.token, self.repo, self.api, self.graphql = token, repo, api.rstrip("/"), graphql

    @classmethod
    def from_env(cls):
        return cls(
            os.environ.get("GITHUB_TOKEN") or None,
            os.environ.get("GITHUB_REPOSITORY") or None,
            os.environ.get("GITHUB_API_URL") or "https://api.github.com",
            os.environ.get("GITHUB_GRAPHQL_URL") or "https://api.github.com/graphql",
        )

    def _call(self, url, body=None):
        if not self.token:
            raise GitHubError("GITHUB_TOKEN is not set")
        if not self.repo:
            raise GitHubError("GITHUB_REPOSITORY is not set")
        req = urllib.request.Request(
            url,
            data=None if body is None else json.dumps(body).encode(),
            headers={
                "Authorization": f"Bearer {self.token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
                "User-Agent": "volt-mutants-exclusions",
            },
        )
        last = None
        for attempt in range(3):
            try:
                with urllib.request.urlopen(req, timeout=30) as resp:
                    return json.load(resp)
            except urllib.error.HTTPError as err:
                if err.code == 404:
                    return None
                last = f"HTTP {err.code} {err.reason} for {url}"
                if err.code < 500:
                    break
            except (urllib.error.URLError, OSError, ValueError) as err:
                last = f"{err} for {url}"
            # A 5xx answer or a network error: two more tries, a moment apart.
            if attempt < 2:
                time.sleep(5 * (attempt + 1))
        raise GitHubError(last)

    def issue(self, n):
        """{"state": "open" | "closed", "pull": bool}, or None when #n does not exist."""
        data = self._call(f"{self.api}/repos/{self.repo}/issues/{n}")
        if data is None:
            return None
        return {"state": data["state"], "pull": "pull_request" in data}

    def closing_issues(self, pr):
        """The numbers of the issues pull request #pr closes when it is merged
        (closing keywords in its description, or linked by hand)."""
        owner, name = self.repo.split("/", 1) if self.repo else ("", "")
        query = (
            "query($owner: String!, $name: String!, $pr: Int!) { repository(owner: $owner, name: $name) {"
            " pullRequest(number: $pr) { closingIssuesReferences(first: 100) { nodes { number } } } } }"
        )
        data = self._call(self.graphql, {"query": query, "variables": {"owner": owner, "name": name, "pr": pr}})
        try:
            if data.get("errors"):
                raise GitHubError(f"GraphQL: {data['errors']}")
            nodes = data["data"]["repository"]["pullRequest"]["closingIssuesReferences"]["nodes"]
        except (AttributeError, KeyError, TypeError):
            raise GitHubError(f"unexpected answer for pull request #{pr}: {data}")
        return {n["number"] for n in nodes}


def issue_errors(entries: list[dict], github, pr: int | None) -> list[str]:
    """#283: (a) the issue of each C entry is open; (b) the pull request does
    not close it while the entry still stands. A state that cannot be read is
    an error, never a pass."""
    errors = []
    for e in entries:
        n = issue_number(e)
        if n is None:
            continue
        try:
            info = github.issue(n)
        except GitHubError as err:
            errors.append(f"{where(e)}: cannot read the state of issue #{n}: {err}")
            continue
        if info is None:
            errors.append(f"{where(e)}: issue #{n} does not exist")
        elif info["pull"]:
            errors.append(f"{where(e)}: #{n} is a pull request, not an issue")
        elif info["state"] != "open":
            errors.append(
                f"{where(e)}: issue #{n} is closed: remove the entry, so that the gate tries the mutant again"
                " (the fix should kill it), or classify the mutant again"
            )
        else:
            print(f"C entry at line {e['line']}: issue #{n} is open")
    if pr is not None:
        try:
            closing = github.closing_issues(pr)
        except GitHubError as err:
            return errors + [f"cannot read the issues pull request #{pr} closes: {err}"]
        print(f"pull request #{pr} closes: {', '.join(f'#{n}' for n in sorted(closing)) or 'no issue'}")
        for e in entries:
            n = issue_number(e)
            if n is not None and n in closing:
                errors.append(
                    f"{where(e)}: pull request #{pr} closes issue #{n} and the entry still stands: remove it in this"
                    " pull request (the fix should kill the mutant), or classify the mutant again"
                )
    return errors


def pinned_version(config: Path) -> str:
    """The cargo-mutants version mutants.yml pins (CARGO_MUTANTS_VERSION)."""
    wf = config.resolve().parent.parent / ".github" / "workflows" / "mutants.yml"
    try:
        m = PINNED_VERSION.search(wf.read_text(encoding="utf-8"))
    except OSError as err:
        raise ConfigError(f"cannot read the pinned cargo-mutants version: {err}")
    if not m:
        raise ConfigError(f"no CARGO_MUTANTS_VERSION in {wf}")
    return m["v"]


class ToolMissing(ConfigError):
    """The pinned cargo-mutants is not installed (none, or another version)."""


def installed_version() -> str | None:
    """The version of the installed cargo-mutants, None when there is none."""
    try:
        out = subprocess.run(["cargo", "mutants", "--version"], check=True, capture_output=True, text=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    return out.split()[-1] if out.split() else "?"


def require_version(config: Path) -> None:
    """Fail unless the installed cargo-mutants is the pinned one: the mutant
    names and the list depend on the version."""
    want = pinned_version(config)
    install = f"cargo install --locked cargo-mutants --version {want}"
    have = installed_version()
    if have is None:
        raise ToolMissing(f"cargo-mutants is not installed; mutants.yml pins {want}: {install}")
    if have != want:
        raise ToolMissing(f"cargo-mutants {have} is installed and mutants.yml pins {want}: {install}")


def unfiltered_list(config: Path, extra: list[str] = ()) -> list[str]:
    """`cargo mutants --list [EXTRA...]` with the configuration minus its exclude_re."""
    require_version(config)
    text = config.read_text(encoding="utf-8")
    copy = re.sub(r"(?ms)^exclude_re\s*=\s*\[.*?^\]", "exclude_re = []", text, count=1)
    if tomllib.loads(copy).get("exclude_re") != []:
        raise ConfigError("could not empty exclude_re in the copy of the configuration")
    fd, tmp = tempfile.mkstemp(suffix=".toml")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            f.write(copy)
        env = dict(os.environ, CARGO_TERM_COLOR="never")
        out = subprocess.run(
            ["cargo", "mutants", "--list", "--config", tmp, *extra], check=True, capture_output=True, text=True, env=env
        ).stdout
    finally:
        os.unlink(tmp)
    return [l.strip() for l in out.splitlines() if l.strip()]


def run_check(
    config: Path,
    list_file: Path | None,
    md: Path | None,
    pr: int | None = None,
    github=None,
    offline: bool = False,
    fix: bool = False,
    if_installed: bool = False,
) -> int:
    try:
        text = config.read_text(encoding="utf-8")
        entries = parse(text)
        names = (
            [l.strip() for l in list_file.read_text(encoding="utf-8").splitlines() if l.strip()]
            if list_file
            else unfiltered_list(config)
        )
    except ToolMissing as err:
        if if_installed:
            # `just check` (ADR-0133): without the pinned cargo-mutants the
            # check is skipped, said plainly; the workflow always runs it.
            print(
                f"Mutation exclusions: SKIPPED, not checked here: {err}."
                " The \"Mutation exclusions\" workflow checks them on every pull request."
            )
            return 0
        print(f"::error title=Mutation exclusions::{err}")
        return 1
    except (ConfigError, subprocess.CalledProcessError) as err:
        print(f"::error title=Mutation exclusions::{err}")
        return 1
    if github is None:
        github = GitHub.from_env()
    errors = check(entries, names) + class_errors(entries)
    if offline:
        print("--offline: the issue states of C entries are not read (the workflow reads them)")
    else:
        errors += issue_errors(entries, github, pr)
    for err in errors:
        print(f"::error title=Mutation exclusions::{err}")
    moves = moved_lines(entries, names)
    for e, _, old, new in moves:
        print(
            f"::notice title=Mutation exclusions::exclude_re entry at line {e['line']}: the expected mutant {old}"
            f" is now {new} (the line is information only; `check --fix-lines` writes it)"
        )
    if fix and moves:
        with config.open("w", encoding="utf-8", newline="") as f:
            f.write(fix_lines(text, moves))
        print(f"--fix-lines: wrote today's line into {len(moves)} expect: line(s) of {config}")
    expected = sum(len(e["expect"]) for e in entries)
    c_issues = [f"#{n}" for n in (issue_number(e) for e in entries) if n is not None]
    verdict = (
        f"{len(entries)} exclude_re entries ({sum(e['c'] for e in entries)} C"
        + (f": {', '.join(c_issues)}" if c_issues else "")
        + f"), {expected} expected mutants, {len(names)} mutants in scope before the exclusions: "
        + (
            "every entry matches exactly its expected mutants"
            + ("" if offline else ", every C entry's issue is open")
            if not errors
            else f"{len(errors)} error(s)"
        )
        + (f"; {len(moves)} expect: line(s) name an old line number" if moves and not fix else "")
    )
    print(verdict)
    if md:
        with md.open("a", encoding="utf-8") as f:
            f.write("## Mutation exclusions (#239, ADR-0133)\n\n" + verdict + "\n\n")
            f.writelines(f"- {e}\n" for e in errors)
            f.write("\n")
    return 1 if errors else 0


def self_test() -> int:
    cfg = (
        'examine_globs = ["crates/*/src/**/*.rs"]\n'
        "exclude_re = [\n"
        "    # A1. reason\n"
        "    # expect: crates/a/src/x.rs:5:9: replace < with <= in f\n"
        '    "a/src/x\\\\.rs:\\\\d+:\\\\d+: replace < with <= in f$",\n'
        "    # A2. two of the three `+` of g: the column tells them apart\n"
        "    # expect: crates/a/src/x.rs:9:3: replace + with - in g\n"
        "    # expect: crates/a/src/x.rs:9:3: replace + with * in g\n"
        '    "a/src/x\\\\.rs:\\\\d+:3: replace \\\\+ with [-*] in g$",\n'
        "]\n"
        "test_workspace = true\n"
    )
    entries = parse(cfg)
    assert [e["regex"] for e in entries] == [
        r"a/src/x\.rs:\d+:\d+: replace < with <= in f$",
        r"a/src/x\.rs:\d+:3: replace \+ with [-*] in g$",
    ], entries
    assert entries[1]["expect"] == ["crates/a/src/x.rs:9:3: replace + with - in g", "crates/a/src/x.rs:9:3: replace + with * in g"]
    assert entries[1]["expect_at"] == [7, 8], entries[1]
    names = [
        "crates/a/src/x.rs:5:9: replace < with <= in f",
        "crates/a/src/x.rs:9:3: replace + with - in g",
        "crates/a/src/x.rs:9:3: replace + with * in g",
        "crates/a/src/x.rs:12:7: replace + with - in g",
    ]
    assert check(entries, names) == [] and moved_lines(entries, names) == []

    def shifted(ns, by):
        return [f"{m['file']}:{int(m['line']) + by}:{m['col']}: {m['rest']}" for m in map(LINECOL.match, ns)]

    # Two lines are added above every statement (#238: 590:43 -> 575:43;
    # #385, #389). The entries are tied to function, change and column: the
    # check passes, and names the new lines.
    moved = shifted(names, 2)
    assert check(entries, moved) == [], check(entries, moved)
    assert [(old, new) for _, _, old, new in moved_lines(entries, moved)] == [
        ("crates/a/src/x.rs:5:9: replace < with <= in f", "crates/a/src/x.rs:7:9: replace < with <= in f"),
        ("crates/a/src/x.rs:9:3: replace + with - in g", "crates/a/src/x.rs:11:3: replace + with - in g"),
        ("crates/a/src/x.rs:9:3: replace + with * in g", "crates/a/src/x.rs:11:3: replace + with * in g"),
    ], moved_lines(entries, moved)
    # The same entry in the old form, pinned to its line (`9:3`), fails on
    # the same move: it matches nothing, its mutants come back as missed.
    old_form = parse(cfg.replace('\\\\d+:3: replace', "9:3: replace"))
    assert old_form[1]["regex"] == r"a/src/x\.rs:9:3: replace \+ with [-*] in g$", old_form[1]
    assert check(old_form, names) == [f"{where(old_form[1])}: {SHAPE_HELP}"], check(old_form, names)
    errs = check(old_form, moved)
    assert f"{where(old_form[1])}: matches no mutant" in errs, errs
    assert any("does not match the expected mutant crates/a/src/x.rs:9:3: replace + with - in g" in e for e in errs), errs
    # The statement's own line is edited and its column moves: the entry is
    # looked at again; the error shows the candidates.
    edited = [n.replace("x.rs:9:3:", "x.rs:9:5:") for n in names]
    errs = check(entries, edited)
    assert f"{where(entries[1])}: matches no mutant" in errs, errs
    assert (
        f"{where(entries[1])}: does not match the expected mutant crates/a/src/x.rs:9:3: replace + with - in g;"
        " same file, change and function now: crates/a/src/x.rs:9:5: replace + with - in g,"
        " crates/a/src/x.rs:12:7: replace + with - in g"
    ) in errs, errs
    # A new `<` in f: the entry matches a mutant nobody classified.
    errs = check(entries, names + ["crates/a/src/x.rs:7:2: replace < with <= in f"])
    assert errs == [
        "exclude_re entry at line 5 (a/src/x\\.rs:\\d+:\\d+: replace < with <= in f$): matches a mutant it was"
        " not written for: crates/a/src/x.rs:7:2: replace < with <= in f; the same change occurs 2 times in the"
        " function: pin the column of the one the entry was written for (`\\d+:<col>`)"
    ], errs
    # A new `+` in g at the pinned column: the column cannot tell them
    # apart, the check fails (an ambiguity is never a pass).
    errs = check(entries, names + ["crates/a/src/x.rs:20:3: replace + with - in g"])
    assert errs == [
        f"{where(entries[1])}: matches 2 mutants with the same file, column, change and function and was"
        " written for 1: the entry cannot tell them apart: crates/a/src/x.rs:9:3: replace + with - in g,"
        " crates/a/src/x.rs:20:3: replace + with - in g"
    ], errs
    # The form of the regex.
    for rx, ok in (
        (r"a/x\.rs:\d+:\d+: replace < with <= in f$", True),
        (r"a/x\.rs:\d+:93: replace < with <= in f$", True),
        (r"a/x\.rs:9:3: replace < with <= in f$", False),
        (r"a/x\.rs:9:\d+: replace < with <= in f$", False),
        (r"a/x\.rs:\d+:\d+: replace < with <= in f", False),
        (r"a/x\.rs:\d+:0: replace < with <= in f$", False),
        (r"replace < with <= in f$", False),
    ):
        assert bool(SHAPE.match(rx)) == ok, rx
    # An entry without expect: lines.
    bare = parse(cfg.replace("    # expect: crates/a/src/x.rs:5:9: replace < with <= in f\n", ""))
    assert any("no `# expect:` line" in e for e in check(bare, names)), check(bare, names)
    for bad in ('exclude_re = [\n    "a", "b",\n]\n', 'exclude_re = [\n    "a",\n    # expect: x\n]\n', "x = 1\n"):
        try:
            parse(bad)
        except ConfigError:
            pass
        else:
            raise AssertionError(f"accepted: {bad!r}")
    with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()) as out:
        c, l = Path(d, "m.toml"), Path(d, "list.txt")
        c.write_text(cfg, encoding="utf-8", newline="\n")
        l.write_text("\n".join(names) + "\n", encoding="utf-8")
        assert run_check(c, l, None, offline=True) == 0
        l.write_text("\n".join(moved) + "\n", encoding="utf-8")
        assert run_check(c, l, None, offline=True) == 0
        assert "is now crates/a/src/x.rs:11:3: replace + with * in g" in out.getvalue(), out.getvalue()
        assert "3 expect: line(s) name an old line number" in out.getvalue(), out.getvalue()
        # --fix-lines writes today's lines; nothing else changes.
        assert run_check(c, l, None, offline=True, fix=True) == 0
        fixed = c.read_text(encoding="utf-8")
        assert fixed == cfg.replace("x.rs:5:9:", "x.rs:7:9:").replace("x.rs:9:3:", "x.rs:11:3:"), fixed
        assert moved_lines(parse(fixed), moved) == []
        l.write_text("\n".join(shifted(edited, 2)) + "\n", encoding="utf-8")
        assert run_check(c, l, None, offline=True) == 1

    # #283 (its own output: the issue states it read, kept out of the log).
    with contextlib.redirect_stdout(io.StringIO()):
        # #283: a C entry names its open issue. The issue states and the issues
        # a pull request closes come from GitHub; Fake stands in for it.
        class Fake:
            def __init__(self, issues=None, closing=(), fail=None):
                self.issues, self.closing, self.fail, self.asked = issues or {}, set(closing), fail, []

            def issue(self, n):
                self.asked.append(("issue", n))
                if self.fail:
                    raise GitHubError(self.fail)
                return self.issues.get(n)

            def closing_issues(self, pr):
                self.asked.append(("closing", pr))
                if self.fail:
                    raise GitHubError(self.fail)
                return set(self.closing)

        c_entry = (
            "    # C. the mutant suggests that g adds where the spec subtracts (silent-wrong?)\n"
            "    # issue: #301\n"
            "    # expect: crates/a/src/x.rs:12:7: replace + with - in g\n"
            '    "a/src/x\\\\.rs:\\\\d+:7: replace \\\\+ with - in g$",\n'
        )
        cfg_c = cfg.replace("]\ntest_workspace", c_entry + "]\ntest_workspace")
        entries_c = parse(cfg_c)
        assert [e["c"] for e in entries_c] == [False, False, True], entries_c
        assert entries_c[2]["issues"] == ["#301"] and entries_c[2]["expect"] == [names[3]], entries_c[2]
        assert check(entries_c, names) == [] and class_errors(entries_c) == []
        opened = {301: {"state": "open", "pull": False}}
        closed = {301: {"state": "closed", "pull": False}}
        at = "exclude_re entry at line 13 (a/src/x\\.rs:\\d+:7: replace \\+ with - in g$)"
        # Open: the entry stands; no pull request, nothing else is asked.
        gh = Fake(opened)
        assert issue_errors(entries_c, gh, None) == [] and gh.asked == [("issue", 301)], gh.asked
        # (a) Closed: the check fails.
        assert issue_errors(entries_c, Fake(closed), None) == [
            f"{at}: issue #301 is closed: remove the entry, so that the gate tries the mutant again"
            " (the fix should kill it), or classify the mutant again"
        ], issue_errors(entries_c, Fake(closed), None)
        # (a) An issue that does not exist, or is a pull request.
        assert issue_errors(entries_c, Fake({}), None) == [f"{at}: issue #301 does not exist"]
        assert issue_errors(entries_c, Fake({301: {"state": "open", "pull": True}}), None) == [
            f"{at}: #301 is a pull request, not an issue"
        ]
        # (b) The pull request closes the issue and the entry still stands.
        assert issue_errors(entries_c, Fake(opened, closing={301, 7}), 400) == [
            f"{at}: pull request #400 closes issue #301 and the entry still stands: remove it in this pull"
            " request (the fix should kill the mutant), or classify the mutant again"
        ], issue_errors(entries_c, Fake(opened, closing={301}), 400)
        assert issue_errors(entries_c, Fake(opened, closing={7}), 400) == []
        # No token, no network: the check fails, never passes silently.
        no_token = GitHub(token=None, repo="o/r")
        assert issue_errors(entries_c, no_token, None) == [
            f"{at}: cannot read the state of issue #301: GITHUB_TOKEN is not set"
        ], issue_errors(entries_c, no_token, None)
        assert issue_errors(entries_c, Fake(fail="<urlopen error [Errno -3] name resolution>"), 400) == [
            f"{at}: cannot read the state of issue #301: <urlopen error [Errno -3] name resolution>",
            "cannot read the issues pull request #400 closes: <urlopen error [Errno -3] name resolution>",
        ]
        assert issue_errors(entries, no_token, 400) == [
            "cannot read the issues pull request #400 closes: GITHUB_TOKEN is not set"
        ], issue_errors(entries, no_token, 400)
        # No C entry and no pull request: nothing to read, no token needed.
        gh = Fake(fail="must not be asked")
        assert issue_errors(entries, gh, None) == [] and gh.asked == []
        # The form of a C entry: exactly one `# issue: #N`, and only on a C entry.
        no_issue = parse(cfg_c.replace("    # issue: #301\n", ""))
        assert class_errors(no_issue) == [
            "exclude_re entry at line 12 (a/src/x\\.rs:\\d+:7: replace \\+ with - in g$): a C entry names its open"
            " issue on one `# issue: #N` line; it has none"
        ], class_errors(no_issue)
        two = parse(cfg_c.replace("    # issue: #301\n", "    # issue: #301\n    # issue: #302\n"))
        assert class_errors(two) == [
            "exclude_re entry at line 14 (a/src/x\\.rs:\\d+:7: replace \\+ with - in g$): a C entry names its open"
            " issue on one `# issue: #N` line; it has 2"
        ], class_errors(two)
        bare_number = parse(cfg_c.replace("# issue: #301", "# issue: 301"))
        assert class_errors(bare_number) == [f"{at}: `# issue: 301` is not `# issue: #N`"], class_errors(bare_number)
        on_a = parse(cfg.replace("    # A1. reason\n", "    # A1. reason\n    # issue: #301\n"))
        assert class_errors(on_a) == [
            "exclude_re entry at line 6 (a/src/x\\.rs:\\d+:\\d+: replace < with <= in f$): has an `# issue:` line"
            " but no C label (`# C. <reason>`): only a C entry waits for an issue"
        ], class_errors(on_a)
        # The C label: `C.`, `C,` or `C (...)` opens a comment line; prose that
        # starts with a C word is not one.
        for label, is_c in (("# C. x", True), ("# C, x", True), ("# C (#301). x", True), ("# Concat x", False),
                            ("# C x", False), ("# A1. C. x", False)):
            assert bool(C_LABEL.match("    " + label)) == is_c, label
        # End to end: a closed issue fails run_check, an open one passes;
        # --offline reads no issue state.
        with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
            c, l = Path(d, "m.toml"), Path(d, "list.txt")
            c.write_text(cfg_c, encoding="utf-8")
            l.write_text("\n".join(names) + "\n", encoding="utf-8")
            assert run_check(c, l, None, pr=None, github=Fake(opened)) == 0
            assert run_check(c, l, None, pr=None, github=Fake(closed)) == 1
            assert run_check(c, l, None, pr=400, github=Fake(opened, closing={301})) == 1
            assert run_check(c, l, None, pr=None, github=GitHub(token=None, repo="o/r")) == 1
            gh = Fake(fail="must not be asked")
            assert run_check(c, l, None, github=gh, offline=True) == 0 and gh.asked == []

    # The pinned version: read from mutants.yml next to the configuration.
    with tempfile.TemporaryDirectory() as d:
        wf = Path(d, ".github", "workflows")
        wf.mkdir(parents=True)
        Path(d, ".cargo").mkdir()
        (wf / "mutants.yml").write_text("env:\n  CARGO_MUTANTS_VERSION: 27.1.0\n", encoding="utf-8")
        assert pinned_version(Path(d, ".cargo", "mutants.toml")) == "27.1.0"
        (wf / "mutants.yml").write_text("env:\n  OTHER: 1\n", encoding="utf-8")
        try:
            pinned_version(Path(d, ".cargo", "mutants.toml"))
        except ConfigError:
            pass
        else:
            raise AssertionError("a workflow without CARGO_MUTANTS_VERSION was accepted")
        # --if-installed (`just check`): without the pinned cargo-mutants (none,
        # or another version) the check is skipped with a message and exit 0;
        # without the flag (the workflow) it fails. With it, the check runs.
        (wf / "mutants.yml").write_text("env:\n  CARGO_MUTANTS_VERSION: 27.1.0\n", encoding="utf-8")
        c = Path(d, ".cargo", "mutants.toml")
        c.write_text(cfg, encoding="utf-8")
        global installed_version
        real = installed_version
        try:
            for have in (None, "26.2.0"):
                installed_version = lambda have=have: have  # noqa: E731
                with contextlib.redirect_stdout(io.StringIO()) as out:
                    assert run_check(c, None, None, offline=True, if_installed=True) == 0
                    assert "Mutation exclusions: SKIPPED, not checked here" in out.getvalue(), out.getvalue()
                    assert "cargo install --locked cargo-mutants --version 27.1.0" in out.getvalue(), out.getvalue()
                    assert run_check(c, None, None, offline=True) == 1
                    assert "::error title=Mutation exclusions::cargo-mutants" in out.getvalue(), out.getvalue()
            # The pinned version is installed: the check runs (here on a list
            # file, so that cargo-mutants itself is not started).
            installed_version = lambda: "27.1.0"  # noqa: E731
            require_version(c)
            with contextlib.redirect_stdout(io.StringIO()) as out:
                l = Path(d, "list.txt")
                l.write_text("\n".join(names) + "\n", encoding="utf-8")
                assert run_check(c, l, None, offline=True, if_installed=True) == 0
                assert "SKIPPED" not in out.getvalue() and "every entry matches" in out.getvalue(), out.getvalue()
        finally:
            installed_version = real
    print("mutants_exclusions self-test: ok")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("--config", type=Path, default=Path(".cargo/mutants.toml"))
    c.add_argument("--list", type=Path, help="the unfiltered list (default: run cargo mutants --list)")
    c.add_argument("--markdown", type=Path)
    c.add_argument("--pr", default="", help="the pull request whose closing issues to check (empty: none)")
    c.add_argument("--offline", action="store_true", help="do not read GitHub: the issue states are not checked")
    c.add_argument("--fix-lines", action="store_true", help="write today's line numbers into the expect: lines")
    c.add_argument(
        "--if-installed",
        action="store_true",
        help="without the pinned cargo-mutants, skip the check with a message and exit 0 (just check)",
    )
    lst = sub.add_parser("list", help="cargo mutants --list without the exclusions (the gate's summary, #283)")
    lst.add_argument("--config", type=Path, default=Path(".cargo/mutants.toml"))
    lst.add_argument("--out", type=Path, required=True)
    lst.add_argument("cargo_args", nargs=argparse.REMAINDER, help="-- and the filter of the run (--in-diff FILE)")
    args = ap.parse_args()
    if args.cmd == "list":
        extra = args.cargo_args[1:] if args.cargo_args[:1] == ["--"] else args.cargo_args
        args.out.write_text(
            "".join(n + "\n" for n in unfiltered_list(args.config, extra)), encoding="utf-8", newline="\n"
        )
        return 0
    if args.pr and not args.pr.isdigit():
        ap.error(f"--pr must be a pull request number, got {args.pr!r}")
    if args.pr and args.offline:
        ap.error("--pr reads the pull request on GitHub; --offline reads nothing")
    return run_check(
        args.config,
        args.list,
        args.markdown,
        int(args.pr) if args.pr else None,
        offline=args.offline,
        fix=args.fix_lines,
        if_installed=args.if_installed,
    )


if __name__ == "__main__":
    sys.exit(main())
