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

    mutants_exclusions.py check [--config .cargo/mutants.toml] [--list FILE] [--markdown FILE]
    mutants_exclusions.py --self-test
"""

import argparse
import contextlib
import io
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

EXPECT = re.compile(r"^\s*#\s*expect:\s*(?P<name>\S.*?)\s*$")
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
    entries, pending = [], []
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
        if s.startswith("#"):
            continue
        m = ENTRY.match(line)
        if not m:
            raise ConfigError(f"line {no}: not one string per line: {s}")
        regex = tomllib.loads(f"v = {m['str']}")["v"]
        entries.append({"regex": regex, "line": no, "expect": pending})
        pending = []
    else:
        raise ConfigError("exclude_re has no closing `]` line")
    if pending:
        raise ConfigError(f"expect: lines after the last entry: {pending}")
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


def unfiltered_list(config: Path) -> list[str]:
    """`cargo mutants --list` with the configuration minus its exclude_re."""
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
            ["cargo", "mutants", "--list", "--config", tmp], check=True, capture_output=True, text=True, env=env
        ).stdout
    finally:
        os.unlink(tmp)
    return [l.strip() for l in out.splitlines() if l.strip()]


def run_check(config: Path, list_file: Path | None, md: Path | None) -> int:
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
    errors = check(entries, names)
    for err in errors:
        print(f"::error title=Mutation exclusions::{err}")
    expected = sum(len(e["expect"]) for e in entries)
    verdict = (
        f"{len(entries)} exclude_re entries, {expected} expected mutants, {len(names)} mutants in scope before"
        f" the exclusions: " + ("every entry matches exactly its expected mutants" if not errors else f"{len(errors)} error(s)")
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
    args = ap.parse_args()
    return run_check(args.config, args.list, args.markdown)


if __name__ == "__main__":
    sys.exit(main())
