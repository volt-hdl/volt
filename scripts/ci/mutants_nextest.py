#!/usr/bin/env python3
"""The test order of the mutation gate: nextest profile `mutants` (ADR-0128, #366).

`.config/nextest.toml` holds the profile: the slow volt-driver binaries
last (priority -100), every other crate's tests before volt-driver's (50).
A shard of `.github/workflows/mutants.yml` adds, before its baseline, the
test binaries the pull request touches (priority 100), at the line
`# @pr-overrides`:

- `crates/<pkg>/tests/<name>.rs` that is an integration test target of
  `<pkg>` (`cargo metadata`): `binary_id(=<pkg>::<target>)`;
- a file under a package's `src/`: that package's unit tests,
  `binary_id(=<pkg>)` (when the package has a library target);
- anything else (a module under `tests/<dir>/`, a non-Rust file) changes
  no priority.

The order decides when a test starts, never whether it runs: every
mutant still runs every test until the first failure
(`--max-fail=1:immediate`), and a missed mutant runs them all.

    mutants_nextest.py pr --changed mutants-list/changed.txt [--config .config/nextest.toml]
    mutants_nextest.py check [--config .config/nextest.toml]
    mutants_nextest.py --self-test

`pr` rewrites the config in the shard's checkout and prints the block it
added; a missing `--changed` file (a full or targeted run) adds nothing.
`check` fails when the marker is not there exactly once, when an override
has no priority, or when a `binary_id(=…)` of the config names no test
binary of the workspace (a renamed or deleted test file).
"""

import argparse
import json
import re
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

MARKER = "# @pr-overrides"
PR_PRIORITY = 100
BINARY_ID = re.compile(r"binary_id\(=([^)\s]+)\)")


def workspace_binaries(metadata: dict) -> tuple[dict[str, str], dict[str, str]]:
    """(integration test source → binary id, package src/ directory → lib binary id)."""
    tests: dict[str, str] = {}
    libs: dict[str, str] = {}
    root = Path(metadata["workspace_root"])
    for pkg in metadata["packages"]:
        if pkg["id"] not in metadata["workspace_members"]:
            continue
        pkg_dir = Path(pkg["manifest_path"]).parent
        for t in pkg["targets"]:
            src = Path(t["src_path"])
            if "test" in t["kind"]:
                tests[rel(src, root)] = f"{pkg['name']}::{t['name']}"
            if "lib" in t["kind"]:
                libs[rel(pkg_dir / "src", root)] = pkg["name"]
    return tests, libs


def rel(path: Path, root: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def all_binary_ids(metadata: dict) -> set[str]:
    tests, libs = workspace_binaries(metadata)
    return set(tests.values()) | set(libs.values())


def pr_binaries(changed: list[str], metadata: dict) -> list[str]:
    tests, libs = workspace_binaries(metadata)
    out: list[str] = []
    for path in changed:
        path = path.strip().replace("\\", "/")
        if not path:
            continue
        hit = tests.get(path)
        if hit is None:
            for src_dir, lib in libs.items():
                if path.startswith(src_dir + "/"):
                    hit = lib
                    break
        if hit is not None and hit not in out:
            out.append(hit)
    return out


def pr_block(binaries: list[str]) -> str:
    if not binaries:
        return ""
    ors = " | ".join(f"binary_id(={b})" for b in binaries)
    return (
        "[[profile.mutants.overrides]]\n"
        "# Written by mutants_nextest.py pr: the test binaries this pull request touches.\n"
        f'filter = "{ors}"\n'
        f"priority = {PR_PRIORITY}\n"
    )


def insert(config_text: str, block: str) -> str:
    lines = config_text.splitlines(keepends=True)
    idx = [i for i, l in enumerate(lines) if l.strip() == MARKER]
    if len(idx) != 1:
        raise ValueError(f"the config must hold the line '{MARKER}' exactly once, found {len(idx)}")
    if not block:
        return config_text
    i = idx[0]
    return "".join(lines[: i + 1]) + block + "".join(lines[i + 1 :])


def check(config_text: str, binaries: set[str]) -> list[str]:
    errors: list[str] = []
    n = sum(1 for l in config_text.splitlines() if l.strip() == MARKER)
    if n != 1:
        errors.append(f"the line '{MARKER}' must appear exactly once, found {n}")
    try:
        cfg = tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as e:
        return errors + [f"not TOML: {e}"]
    overrides = cfg.get("profile", {}).get("mutants", {}).get("overrides")
    if not overrides:
        errors.append("profile.mutants has no overrides")
        return errors
    for k, o in enumerate(overrides):
        if "priority" not in o:
            errors.append(f"override {k + 1} has no priority")
        for name in BINARY_ID.findall(o.get("filter", "")):
            if name not in binaries:
                errors.append(f"override {k + 1}: binary_id(={name}) is no test binary of the workspace")
    return errors


def cargo_metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], check=True, capture_output=True, text=True
    )
    return json.loads(out.stdout)


def self_test() -> int:
    failures: list[str] = []

    def expect(cond: bool, what: str) -> None:
        if not cond:
            failures.append(what)

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for p in ("crates/a/src/lib.rs", "crates/a/tests/t1.rs", "crates/a/tests/util/mod.rs", "crates/b/src/main.rs"):
            (root / p).parent.mkdir(parents=True, exist_ok=True)
            (root / p).write_text("", encoding="utf-8")
        meta = {
            "workspace_root": str(root),
            "workspace_members": ["a-id", "b-id"],
            "packages": [
                {
                    "id": "a-id",
                    "name": "a",
                    "manifest_path": str(root / "crates/a/Cargo.toml"),
                    "targets": [
                        {"name": "a", "kind": ["lib"], "src_path": str(root / "crates/a/src/lib.rs")},
                        {"name": "t1", "kind": ["test"], "src_path": str(root / "crates/a/tests/t1.rs")},
                    ],
                },
                {
                    "id": "b-id",
                    "name": "b",
                    "manifest_path": str(root / "crates/b/Cargo.toml"),
                    "targets": [{"name": "b", "kind": ["bin"], "src_path": str(root / "crates/b/src/main.rs")}],
                },
                {"id": "dep", "name": "dep", "manifest_path": "/x/Cargo.toml", "targets": []},
            ],
        }
        got = pr_binaries(
            [
                "crates/a/tests/t1.rs",
                "crates/a/src/deep/x.rs",
                "crates/a/tests/util/mod.rs",
                "crates/b/src/main.rs",
                "docs/x.md",
                "crates/a/src/lib.rs",
            ],
            meta,
        )
        expect(got == ["a::t1", "a"], f"pr_binaries: {got}")
        expect(all_binary_ids(meta) == {"a", "a::t1"}, f"all_binary_ids: {all_binary_ids(meta)}")

    base = (
        "[profile.mutants]\n# @pr-overrides\n\n[[profile.mutants.overrides]]\n"
        'filter = "binary_id(=a::t1)"\npriority = -100\n'
    )
    added = insert(base, pr_block(["a"]))
    cfg = tomllib.loads(added)["profile"]["mutants"]["overrides"]
    expect([o["priority"] for o in cfg] == [100, -100], f"the PR block comes first: {cfg}")
    expect(cfg[0]["filter"] == "binary_id(=a)", f"PR filter: {cfg[0]}")
    expect(insert(base, "") == base, "no PR binary: the config is unchanged")
    expect(check(added, {"a", "a::t1"}) == [], f"check of a good config: {check(added, {'a', 'a::t1'})}")
    expect(any("is no test binary" in e for e in check(base, {"a"})), "check: a binary that does not exist")
    expect(any("exactly once" in e for e in check(base.replace(MARKER, ""), {"a::t1"})), "check: no marker")
    expect(any("no priority" in e for e in check(base.replace("priority = -100\n", ""), {"a::t1"})), "check: priority")
    try:
        insert(base + MARKER + "\n", pr_block(["a"]))
        failures.append("insert: two markers accepted")
    except ValueError:
        pass
    # The repository's own config parses and has the marker once.
    repo_cfg = Path(__file__).resolve().parents[2] / ".config" / "nextest.toml"
    text = repo_cfg.read_text(encoding="utf-8")
    errs = [e for e in check(text, set(BINARY_ID.findall(text)) | {"x"}) if "is no test binary" not in e]
    expect(errs == [], f"the repository's .config/nextest.toml: {errs}")

    for f in failures:
        print(f"FAIL: {f}")
    print("mutants_nextest self-test: " + ("ok" if not failures else f"{len(failures)} failure(s)"))
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", nargs="?", choices=["pr", "check"])
    ap.add_argument("--changed", type=Path, help="pr: the files changed since the gate's base, one per line")
    ap.add_argument("--config", type=Path, default=Path(".config/nextest.toml"))
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if args.command is None:
        ap.error("pr, check or --self-test")
    text = args.config.read_text(encoding="utf-8")
    meta = cargo_metadata()
    if args.command == "check":
        errors = check(text, all_binary_ids(meta))
        for e in errors:
            print(f"::error title=nextest profile mutants::{args.config}: {e}")
        if not errors:
            print(f"{args.config}: profile mutants checked; every binary_id names a test binary of the workspace")
        return 1 if errors else 0
    changed = []
    if args.changed is not None and args.changed.is_file():
        changed = args.changed.read_text(encoding="utf-8").splitlines()
    else:
        print("No list of changed files (a full or targeted run): no pull-request binary goes first.")
    binaries = pr_binaries(changed, meta)
    block = pr_block(binaries)
    args.config.write_text(insert(text, block), encoding="utf-8", newline="\n")
    if block:
        print(f"Test binaries of this pull request first (priority {PR_PRIORITY}):")
        for b in binaries:
            print(f"  {b}")
    else:
        print("No changed file is a test binary or a package's src/: no pull-request binary goes first.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
