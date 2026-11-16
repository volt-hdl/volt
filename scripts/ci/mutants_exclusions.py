#!/usr/bin/env python3
"""Every exclude_re entry matches exactly the mutants it was written for (#239).

An `exclude_re` entry of `.cargo/mutants.toml` (ADR-0109 section 4) is a
regex over mutant names. When the code moves, an entry can drift both ways:
a pinned entry (`575:43`) stops matching its mutant, which comes back as
missed; or an entry starts to match a mutant nobody classified (a new `<`
in the same function, another statement that lands on the pinned
line:col), which then silently leaves the measurement.

So each entry names the mutants it was written for, one comment line each,
between its reason and the entry, exactly as `cargo mutants --list` prints
them:

    # A1. ...reason...
    # expect: crates/volt-sv-emit/src/let_width.rs:262:14: replace < with <= in demand
    "volt-sv-emit/src/let_width\\.rs:\\d+:\\d+: replace < with <= in demand$",

`check` takes the list of every mutant in scope before the exclusions
(`cargo mutants --list` with a copy of the configuration whose exclude_re
is empty; `--no-config` would drop examine_globs too) and applies each
entry as cargo-mutants does (Python `re.search` over the name). It fails
when an entry has no `expect:` line, matches no mutant, misses an expected
mutant, or matches one that is not expected; for a name that moved it
shows the mutants with the same file, change and function. cargo-mutants
27.1.0 lists field-deletion mutants whatever the filters (ADR-0109
"Hedefli koşu"); the check filters names in Python, so they are covered too.

Names cannot tell two mutants with the same name apart: if the classified
statement moves away and another one with the same change, in the same
function, lands on exactly its line:col, the check still passes.

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

    mutants_exclusions.py check [--config .cargo/mutants.toml] [--list FILE] [--markdown FILE] [--pr N]
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
from pathlib import Path

EXPECT = re.compile(r"^\s*#\s*expect:\s*(?P<name>\S.*?)\s*$")
ISSUE = re.compile(r"^\s*#\s*issue:\s*(?P<value>.*?)\s*$")
ISSUE_VALUE = re.compile(r"^#(?P<n>[1-9][0-9]*)$")
# The class label that opens a C entry's reason: `C.`, `C,` or `C (...)`.
C_LABEL = re.compile(r"^\s*#\s*C(\s*\([^)]*\))?[.,]\s")
ENTRY = re.compile(r'^\s*(?P<str>"(?:[^"\\]|\\.)*")\s*,?\s*(#.*)?$')
START = re.compile(r"^\s*exclude_re\s*=\s*\[\s*$")
LINECOL = re.compile(r"^(?P<file>[^:]+):\d+:\d+: (?P<rest>.+)$")


class ConfigError(Exception):
    pass


def parse(text: str) -> list[dict]:
    """The exclude_re entries with their expect: names, in file order."""
    lines = text.splitlines()
    try:
        start = next(i for i, l in enumerate(lines) if START.match(l))
    except StopIteration:
        raise ConfigError("no `exclude_re = [` line (one entry per line)")
    entries, pending, issues, is_c = [], [], [], False
    for no, line in enumerate(lines[start + 1 :], start + 2):
        s = line.strip()
        if s == "]":
            break
        if not s:
            continue
        m = EXPECT.match(line)
        if m:
            pending.append(m["name"])
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
        entries.append({"regex": regex, "line": no, "expect": pending, "issues": issues, "c": is_c})
        pending, issues, is_c = [], [], False
    else:
        raise ConfigError("exclude_re has no closing `]` line")
    if pending or issues:
        raise ConfigError(f"expect: or issue: lines after the last entry: {pending + issues}")
    parsed = tomllib.loads(text).get("exclude_re", [])
    if [e["regex"] for e in entries] != parsed:
        raise ConfigError("the line-by-line reading of exclude_re differs from the TOML value")
    return entries


def without_linecol(name: str) -> str:
    m = LINECOL.match(name)
    return f"{m['file']}: {m['rest']}" if m else name


def check(entries: list[dict], names: list[str]) -> list[str]:
    """The errors: an empty list when every entry matches exactly its expect: names."""
    errors = []
    loose = {}
    for n in names:
        loose.setdefault(without_linecol(n), []).append(n)
    for e in entries:
        where = f"exclude_re entry at line {e['line']} ({e['regex']})"
        rx = re.compile(e["regex"])
        matched = [n for n in names if rx.search(n)]
        expected = e["expect"]
        if not expected:
            errors.append(f"{where}: no `# expect:` line names the mutants it was written for")
        if not matched:
            errors.append(f"{where}: matches no mutant")
        for n in expected:
            if n not in matched:
                moved = [c for c in loose.get(without_linecol(n), []) if c != n]
                hint = f"; same file, change and function now: {', '.join(moved)}" if moved else ""
                errors.append(f"{where}: does not match the expected mutant {n}{hint}")
        for n in matched:
            if n not in expected:
                errors.append(f"{where}: matches a mutant it was not written for: {n}")
    return errors


def where(e: dict) -> str:
    return f"exclude_re entry at line {e['line']} ({e['regex']})"


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


def unfiltered_list(config: Path, extra: list[str] = ()) -> list[str]:
    """`cargo mutants --list [EXTRA...]` with the configuration minus its exclude_re."""
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


def run_check(config: Path, list_file: Path | None, md: Path | None, pr: int | None = None, github=None) -> int:
    try:
        entries = parse(config.read_text(encoding="utf-8"))
        names = (
            [l.strip() for l in list_file.read_text(encoding="utf-8").splitlines() if l.strip()]
            if list_file
            else unfiltered_list(config)
        )
    except (ConfigError, subprocess.CalledProcessError) as err:
        print(f"::error title=Mutation exclusions::{err}")
        return 1
    if github is None:
        github = GitHub.from_env()
    errors = check(entries, names) + class_errors(entries) + issue_errors(entries, github, pr)
    for err in errors:
        print(f"::error title=Mutation exclusions::{err}")
    expected = sum(len(e["expect"]) for e in entries)
    c_issues = [f"#{n}" for n in (issue_number(e) for e in entries) if n is not None]
    verdict = (
        f"{len(entries)} exclude_re entries ({sum(e['c'] for e in entries)} C"
        + (f": {', '.join(c_issues)}" if c_issues else "")
        + f"), {expected} expected mutants, {len(names)} mutants in scope before the exclusions: "
        + ("every entry matches exactly its expected mutants, every C entry's issue is open" if not errors else f"{len(errors)} error(s)")
    )
    print(verdict)
    if md:
        with md.open("a", encoding="utf-8") as f:
            f.write("## Mutation exclusions (#239)\n\n" + verdict + "\n\n")
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
        "    # A2. pinned\n"
        "    # expect: crates/a/src/x.rs:9:3: replace + with - in g\n"
        "    # expect: crates/a/src/x.rs:9:3: replace + with * in g\n"
        '    "a/src/x\\\\.rs:9:3: replace \\\\+ with [-*] in g$",\n'
        "]\n"
        "test_workspace = true\n"
    )
    entries = parse(cfg)
    assert [e["regex"] for e in entries] == [
        r"a/src/x\.rs:\d+:\d+: replace < with <= in f$",
        r"a/src/x\.rs:9:3: replace \+ with [-*] in g$",
    ], entries
    assert entries[1]["expect"] == ["crates/a/src/x.rs:9:3: replace + with - in g", "crates/a/src/x.rs:9:3: replace + with * in g"]
    names = [
        "crates/a/src/x.rs:5:9: replace < with <= in f",
        "crates/a/src/x.rs:9:3: replace + with - in g",
        "crates/a/src/x.rs:9:3: replace + with * in g",
        "crates/a/src/x.rs:12:3: replace + with - in g",
    ]
    assert check(entries, names) == []
    # The pinned statement moved two lines down (#238: 590:43 -> 575:43).
    moved = [n.replace(":9:3:", ":11:3:") for n in names]
    errs = check(entries, moved)
    assert any("matches no mutant" in e for e in errs), errs
    assert any(
        "does not match the expected mutant crates/a/src/x.rs:9:3: replace + with - in g; same file, change and"
        " function now: crates/a/src/x.rs:11:3: replace + with - in g, crates/a/src/x.rs:12:3: replace + with - in g" in e
        for e in errs
    ), errs
    # A new `<` in f: the unpinned entry matches a mutant nobody classified.
    errs = check(entries, names + ["crates/a/src/x.rs:7:2: replace < with <= in f"])
    assert errs == [
        "exclude_re entry at line 5 (a/src/x\\.rs:\\d+:\\d+: replace < with <= in f$): matches a mutant it was"
        " not written for: crates/a/src/x.rs:7:2: replace < with <= in f"
    ], errs
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
    with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
        c, l = Path(d, "m.toml"), Path(d, "list.txt")
        c.write_text(cfg, encoding="utf-8")
        l.write_text("\n".join(names) + "\n", encoding="utf-8")
        assert run_check(c, l, None) == 0
        l.write_text("\n".join(moved) + "\n", encoding="utf-8")
        assert run_check(c, l, None) == 1

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
            "    # expect: crates/a/src/x.rs:12:3: replace + with - in g\n"
            '    "a/src/x\\\\.rs:12:3: replace \\\\+ with - in g$",\n'
        )
        cfg_c = cfg.replace("]\ntest_workspace", c_entry + "]\ntest_workspace")
        entries_c = parse(cfg_c)
        assert [e["c"] for e in entries_c] == [False, False, True], entries_c
        assert entries_c[2]["issues"] == ["#301"] and entries_c[2]["expect"] == [names[3]], entries_c[2]
        assert check(entries_c, names) == [] and class_errors(entries_c) == []
        opened = {301: {"state": "open", "pull": False}}
        closed = {301: {"state": "closed", "pull": False}}
        where = "exclude_re entry at line 13 (a/src/x\\.rs:12:3: replace \\+ with - in g$)"
        # Open: the entry stands; no pull request, nothing else is asked.
        gh = Fake(opened)
        assert issue_errors(entries_c, gh, None) == [] and gh.asked == [("issue", 301)], gh.asked
        # (a) Closed: the check fails.
        assert issue_errors(entries_c, Fake(closed), None) == [
            f"{where}: issue #301 is closed: remove the entry, so that the gate tries the mutant again"
            " (the fix should kill it), or classify the mutant again"
        ], issue_errors(entries_c, Fake(closed), None)
        # (a) An issue that does not exist, or is a pull request.
        assert issue_errors(entries_c, Fake({}), None) == [f"{where}: issue #301 does not exist"]
        assert issue_errors(entries_c, Fake({301: {"state": "open", "pull": True}}), None) == [
            f"{where}: #301 is a pull request, not an issue"
        ]
        # (b) The pull request closes the issue and the entry still stands.
        assert issue_errors(entries_c, Fake(opened, closing={301, 7}), 400) == [
            f"{where}: pull request #400 closes issue #301 and the entry still stands: remove it in this pull"
            " request (the fix should kill the mutant), or classify the mutant again"
        ], issue_errors(entries_c, Fake(opened, closing={301}), 400)
        assert issue_errors(entries_c, Fake(opened, closing={7}), 400) == []
        # No token, no network: the check fails, never passes silently.
        no_token = GitHub(token=None, repo="o/r")
        assert issue_errors(entries_c, no_token, None) == [
            f"{where}: cannot read the state of issue #301: GITHUB_TOKEN is not set"
        ], issue_errors(entries_c, no_token, None)
        assert issue_errors(entries_c, Fake(fail="<urlopen error [Errno -3] name resolution>"), 400) == [
            f"{where}: cannot read the state of issue #301: <urlopen error [Errno -3] name resolution>",
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
            "exclude_re entry at line 12 (a/src/x\\.rs:12:3: replace \\+ with - in g$): a C entry names its open"
            " issue on one `# issue: #N` line; it has none"
        ], class_errors(no_issue)
        two = parse(cfg_c.replace("    # issue: #301\n", "    # issue: #301\n    # issue: #302\n"))
        assert class_errors(two) == [
            "exclude_re entry at line 14 (a/src/x\\.rs:12:3: replace \\+ with - in g$): a C entry names its open"
            " issue on one `# issue: #N` line; it has 2"
        ], class_errors(two)
        bare_number = parse(cfg_c.replace("# issue: #301", "# issue: 301"))
        assert class_errors(bare_number) == [f"{where}: `# issue: 301` is not `# issue: #N`"], class_errors(bare_number)
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
        # End to end: a closed issue fails run_check, an open one passes.
        with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
            c, l = Path(d, "m.toml"), Path(d, "list.txt")
            c.write_text(cfg_c, encoding="utf-8")
            l.write_text("\n".join(names) + "\n", encoding="utf-8")
            assert run_check(c, l, None, pr=None, github=Fake(opened)) == 0
            assert run_check(c, l, None, pr=None, github=Fake(closed)) == 1
            assert run_check(c, l, None, pr=400, github=Fake(opened, closing={301})) == 1
            assert run_check(c, l, None, pr=None, github=GitHub(token=None, repo="o/r")) == 1
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
    return run_check(args.config, args.list, args.markdown, int(args.pr) if args.pr else None)


if __name__ == "__main__":
    sys.exit(main())
