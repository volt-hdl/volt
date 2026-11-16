#!/usr/bin/env python3
"""Every known failure waits for an open issue (ADR-0117 section 4, #254).

`crates/volt-driver/tests/metamorphic/known_failures.json` lists the
failures of the real-bug test and of the metamorphic test that an open
issue reproduces. The tests themselves fail on a failure that is not listed
and on an entry that no longer fails. This check adds what the tests cannot
see, with the rules of the mutation exclusions' C entries (#283) and the
same GitHub client (`mutants_exclusions.GitHub`):

(a) the issue of every entry exists, is an issue (not a pull request) and
    is open: a closed issue means the fix is in, so the entry must go and
    the test must pass without it;
(b) with `--pr N`, pull request N does not close an entry's issue (closing
    keywords such as `Closes #216`, or linked by hand) while the entry
    still stands: the pull request that fixes the bug removes the entry.

The issue states are read with the workflow's token. Without a token or a
network the check fails; it never passes without having read them.

    python3 scripts/ci/known_failures.py check [--pr N] [--markdown FILE]
    python3 scripts/ci/known_failures.py --self-test
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ in the checkout
sys.path.insert(0, str(Path(__file__).resolve().parent))
from mutants_exclusions import GitHub, GitHubError  # noqa: E402  (the #283 client)

LIST = Path("crates/volt-driver/tests/metamorphic/known_failures.json")
RELATIONS = {"expect", "test", "codes", "safety", "location", "hardware"}


class ListError(Exception):
    pass


def parse(text: str) -> list[dict]:
    """The entries; the same validation as the Rust test (`known::parse`)."""
    try:
        data = json.loads(text)
    except ValueError as err:
        raise ListError(f"not JSON: {err}")
    entries = data.get("known_failures") if isinstance(data, dict) else None
    if not isinstance(entries, list):
        raise ListError("no `known_failures` array")
    seen = set()
    for i, e in enumerate(entries):
        for k in ("check", "design", "relation", "reason"):
            if not isinstance(e.get(k), str) or not e[k].strip():
                raise ListError(f"entry {i}: `{k}` is missing or empty")
        if e["relation"] not in RELATIONS:
            raise ListError(f"entry {i}: unknown relation `{e['relation']}`")
        if not isinstance(e.get("issue"), int) or isinstance(e["issue"], bool) or e["issue"] <= 0:
            raise ListError(f"entry {i}: `issue` must be an issue number")
        key = (e["check"], e["design"], e["relation"])
        if key in seen:
            raise ListError(f"entry {i}: {key} is listed twice")
        seen.add(key)
    return entries


def where(e: dict) -> str:
    return f"known failure `{e['check']} {e['design']} [{e['relation']}]`"


def issue_errors(entries: list[dict], github, pr: int | None) -> list[str]:
    """(a) every entry's issue is open; (b) the pull request does not close
    it while the entry stands. A state that cannot be read is an error."""
    errors = []
    states: dict[int, object] = {}
    for e in entries:
        n = e["issue"]
        if n not in states:
            try:
                states[n] = github.issue(n)
            except GitHubError as err:
                states[n] = err
        info = states[n]
        if isinstance(info, GitHubError):
            errors.append(f"{where(e)}: cannot read the state of issue #{n}: {info}")
        elif info is None:
            errors.append(f"{where(e)}: issue #{n} does not exist")
        elif info["pull"]:
            errors.append(f"{where(e)}: #{n} is a pull request, not an issue")
        elif info["state"] != "open":
            errors.append(
                f"{where(e)}: issue #{n} is closed: remove the entry (the fix should make the check pass),"
                " or open an issue for what still fails"
            )
    for n, info in sorted(states.items()):
        if isinstance(info, dict) and info["state"] == "open" and not info["pull"]:
            print(f"issue #{n} is open ({sum(e['issue'] == n for e in entries)} known failure(s))")
    if pr is not None:
        try:
            closing = github.closing_issues(pr)
        except GitHubError as err:
            return errors + [f"cannot read the issues pull request #{pr} closes: {err}"]
        print(f"pull request #{pr} closes: {', '.join(f'#{n}' for n in sorted(closing)) or 'no issue'}")
        for e in entries:
            if e["issue"] in closing:
                errors.append(
                    f"{where(e)}: pull request #{pr} closes issue #{e['issue']} and the entry still stands:"
                    " remove it in this pull request"
                )
    return errors


def run_check(path: Path, md: Path | None, pr: int | None = None, github=None) -> int:
    try:
        entries = parse(path.read_text(encoding="utf-8"))
    except (ListError, OSError) as err:
        print(f"::error title=Known failures::{path}: {err}")
        return 1
    if github is None:
        github = GitHub.from_env()
    errors = issue_errors(entries, github, pr)
    for err in errors:
        print(f"::error title=Known failures::{err}")
    issues = sorted({e["issue"] for e in entries})
    verdict = (
        f"{len(entries)} known failure(s)"
        + (f", issues {', '.join(f'#{n}' for n in issues)}" if issues else "")
        + ": "
        + ("every issue is open and no pull request closes one" if not errors else f"{len(errors)} error(s)")
    )
    print(verdict)
    if md:
        with md.open("a", encoding="utf-8") as f:
            f.write("## Known failures (ADR-0117)\n\n" + verdict + "\n\n")
            f.writelines(f"- {e}\n" for e in errors)
            f.write("\n")
    return 1 if errors else 0


def self_test() -> int:
    class Fake:
        def __init__(self, issues=None, closing=(), fail=None):
            self.issues, self.closing, self.fail, self.asked = issues or {}, set(closing), fail, []

        def issue(self, n):
            self.asked.append(n)
            if self.fail:
                raise GitHubError(self.fail)
            return self.issues.get(n)

        def closing_issues(self, pr):
            if self.fail:
                raise GitHubError(self.fail)
            return self.closing

    one = {"check": "real-bug", "design": "tests/real_bugs/216_x.volt", "relation": "test", "issue": 216, "reason": "r"}
    two = dict(one, relation="expect")
    text = json.dumps({"known_failures": [one, two]})
    entries = parse(text)
    w = where(one)
    opened = {216: {"state": "open", "pull": False}}
    with contextlib.redirect_stdout(io.StringIO()):
        gh = Fake(opened)
        assert issue_errors(entries, gh, None) == [] and gh.asked == [216], gh.asked  # asked once per issue
        closed = issue_errors(entries, Fake({216: {"state": "closed", "pull": False}}), None)
        assert len(closed) == 2 and closed[0].startswith(f"{w}: issue #216 is closed"), closed
        assert issue_errors(entries[:1], Fake({}), None) == [f"{w}: issue #216 does not exist"]
        assert issue_errors(entries[:1], Fake({216: {"state": "open", "pull": True}}), None) == [
            f"{w}: #216 is a pull request, not an issue"
        ]
        assert issue_errors(entries[:1], Fake(opened, closing={216}), 400) == [
            f"{w}: pull request #400 closes issue #216 and the entry still stands: remove it in this pull request"
        ]
        assert issue_errors(entries[:1], Fake(opened, closing={7}), 400) == []
        no_token = GitHub(token=None, repo="o/r")
        assert issue_errors(entries[:1], no_token, None) == [
            f"{w}: cannot read the state of issue #216: GITHUB_TOKEN is not set"
        ]
        assert issue_errors([], no_token, 400) == ["cannot read the issues pull request #400 closes: GITHUB_TOKEN is not set"]
    # Validation, the same rules as the Rust test.
    for bad, msg in [
        (dict(one, issue="#216"), "`issue` must be an issue number"),
        (dict(one, issue=True), "`issue` must be an issue number"),
        (dict(one, relation="other"), "unknown relation"),
        (dict(one, reason=" "), "`reason` is missing or empty"),
    ]:
        try:
            parse(json.dumps({"known_failures": [bad]}))
            raise AssertionError(f"accepted {bad}")
        except ListError as err:
            assert msg in str(err), err
    try:
        parse(json.dumps({"known_failures": [one, one]}))
        raise AssertionError("accepted a repeated entry")
    except ListError as err:
        assert "twice" in str(err), err
    # End to end.
    with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()):
        p = Path(d, "k.json")
        p.write_text(text, encoding="utf-8")
        assert run_check(p, None, None, Fake(opened)) == 0
        assert run_check(p, None, None, Fake({216: {"state": "closed", "pull": False}})) == 1
        assert run_check(p, None, 400, Fake(opened, closing={216})) == 1
        assert run_check(p, None, None, GitHub(token=None, repo="o/r")) == 1
        p.write_text("{}", encoding="utf-8")
        assert run_check(p, None, None, Fake(opened)) == 1
        p.write_text(json.dumps({"known_failures": []}), encoding="utf-8")
        assert run_check(p, None, 400, Fake(opened)) == 0
    print("known_failures self-test: ok")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("--list", type=Path, default=LIST)
    c.add_argument("--markdown", type=Path)
    c.add_argument("--pr", default="", help="the pull request whose closing issues to check (empty: none)")
    args = ap.parse_args()
    if args.pr and not args.pr.isdigit():
        ap.error(f"--pr must be a pull request number, got {args.pr!r}")
    return run_check(args.list, args.markdown, int(args.pr) if args.pr else None)


if __name__ == "__main__":
    sys.exit(main())
