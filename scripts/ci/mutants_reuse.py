#!/usr/bin/env python3
"""Reuse of an earlier passed gate run (#318, ADR-0109 appendix "Kapının önceki sonucu").

The pull-request gate of .github/workflows/mutants.yml tests the mutants on
the changed Rust lines. A push that changes nothing those outcomes depend
on (a docs-only push) used to pay for the whole run again. Before any
shard starts, the gate now computes a reuse key, and when an earlier gate
run of the same pull request has the same key and a clean, complete
result, the gate gives its verdict from that run's outcomes instead.

The key is the hash of three parts, each kept in key.json in full so that
two keys are compared field by field and the log says what differs:

1. mutants: the gate's mutant list before the exclusions (`unexcluded.txt`,
   `cargo mutants --list --in-diff` with exclude_re emptied), and the blob
   of every file it mutates;
2. tree: the merge commit's tree (path, mode, blob) without the inert paths
   (scripts/ci/mutants_inert.txt) and without .cargo/mutants.toml, which is
   in part 3;
3. environment: .cargo/mutants.toml parsed, without exclude_re (exclude_re
   only filters the list; the current one is re-applied to the earlier
   outcomes); the workflow's blob, the Verilator pin (verilator.env) and
   action, the inert list; the workflow's tool and limit variables;
   `rustc -vV`, `cargo mutants --version`, `cargo nextest --version`; the
   runner image (ImageOS and the weekly release, the date part of
   ImageVersion) and the runner's toolchain (g++, glibc, binutils, make,
   python3, git: the first line of each `--version`).

An earlier run is used only when all of this holds; otherwise the gate
runs in full and the log says why:

- it is a `pull_request` run of mutants.yml for the same pull request
  number, the same head branch and the same head repository as this one,
  which is not a fork; never a hand run, never another pull request;
- it completed (a cancelled run, a run still going, a run without a key
  and a run that itself reused are passed over: the next older one is
  looked at); the newest earlier run with an equal key decides, and it
  must have concluded `success`;
- its key.json equals this run's, part by part;
- its artifacts come from the GitHub API (`gh api`, the workflow's token,
  `actions: read`); each zip's sha256 equals the digest GitHub recorded at
  upload, before anything in it is read;
- every shard of it completed: every listed mutant has an outcome; no
  missed, no timeout, no memory-limit mutant (a timeout and a memory kill
  depend on the runner and its load, they are never reused);
- it has the artifact of its baseline job (`mutants-baseline`, ADR-0128
  §2: one baseline per run, beside the shards): its tool ledger passes
  `--require-zero-skips --min-ran 60` and its stamp (written after the
  baseline passed) names the same rustc, cargo-mutants, cargo-nextest and
  runner image as the key and shows that the inert paths were absent;
- every shard's tool ledger passes `--require-zero-skips` (its
  `Tool ledger [mutants-k]: ... SKIP: 0` line is printed again; a shard
  has no baseline, so how many tool tests its mutants reach depends on
  where they are caught, and `--min-ran` is the baseline's);
- every shard's stamp names the same rustc, cargo-mutants, cargo-nextest
  and runner image as the key, and shows that the inert paths were absent
  while it tested;
- every mutant of this run's list (after today's exclude_re) was tested
  then and caught or unviable.

    mutants_reuse.py key --lists DIR [--out FILE]
    mutants_reuse.py find --lists DIR --pr N --head-repo OWNER/NAME --head-ref BRANCH [--github-output FILE]
    mutants_reuse.py mask
    mutants_reuse.py retest --lists DIR (--pr N --head-repo OWNER/NAME --head-ref BRANCH | --run ID) [--github-output FILE]
    mutants_reuse.py retest-check --lists DIR
    mutants_reuse.py stamp --out FILE --role baseline --baseline-seconds S --timeout T
    mutants_reuse.py stamp --out FILE --role shard --timeout T
    mutants_reuse.py --self-test

`key` writes DIR/key.json from DIR/unexcluded.txt and the checkout. `find`
looks for the earlier run and, when it reuses one, writes DIR/reuse/
(reuse.json, the earlier lists, the shards' outcomes and the baseline's
files) for the summary job; GitHub Actions outputs `run`, `sha` and `key`.
`retest` (#364, ADR-0129), when nothing was reused: the pull request's
last full gate run (or the run `--run` names, a hand gate run's
`retest_from`) is the base when its mutants and environment are this run's
and every path changed since is under a package's `tests/`; it writes
DIR/retest/ (retest.json: why each mutant is tested again, the outcomes
carried over; retest.re: the exact names) and outputs `run` and `sha`.
Tested again: the base's missed, timeout and memory-limit mutants, those
it did not test, and those caught by a test binary the push changed (read
from the nextest failure line of the mutant's log; a log that names none
is tested again). `retest-check` checks the narrowed list against the
gate's (`gate-all.txt`) and fixes what is carried over.
`mask` deletes the inert paths from the checkout (the baseline job and each
shard, before they test). `stamp` writes the tool versions, runner image
and inert paths of the baseline job (with its seconds and the test timeout
the rule gives, ADR-0128 §3) or of a shard (with the timeout it used).
"""

import argparse
import contextlib
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import urllib.parse
import zipfile
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mutants_nextest  # noqa: E402  scripts/ci, next to this script
import mutants_summary  # noqa: E402
import tool_ledger  # noqa: E402

KEY_VERSION = 1
INERT_FILE = "scripts/ci/mutants_inert.txt"
CONFIG = ".cargo/mutants.toml"
WORKFLOW = ".github/workflows/mutants.yml"
WORKFLOW_FILE = "mutants.yml"
VERILATOR_ENV = "scripts/ci/verilator.env"
VERILATOR_ACTION = ".github/actions/verilator"
# The workflow's variables that change what a shard does.
ENV_VARS = (
    "CARGO_MUTANTS_VERSION",
    "CARGO_NEXTEST_VERSION",
    "OSS_CAD_SUITE_DATE",
    "MUTANTS_MEMORY_LIMIT_MIB",
    "MUTANTS_GATE_SECONDS_PER_MUTANT",
    "MUTANTS_TIMEOUT_FLOOR_SECONDS",
)
# The runner image: its OS and its weekly release (the date part of
# ImageVersion). The rest of ImageVersion is a build number that differs
# between jobs of one run (run 44186837945: the first job on 20261111.370.1,
# its shard on 20261111.127.1); what of the image can change an outcome is
# measured instead (SYSTEM).
RUNNER_VARS = ("ImageOS", "RUNNER_OS", "RUNNER_ARCH")
TOOLS = ("rustc", "cargo_mutants", "cargo_nextest")
# The runner's own toolchain: g++ builds every Verilator model, glibc and
# binutils link it, make drives it, python3 runs the CI scripts, git the
# version text of build.rs. First line of each `--version`.
SYSTEM = {
    "gxx": ["g++", "--version"],
    "glibc": ["ldd", "--version"],
    "binutils": ["ld", "--version"],
    "make": ["make", "--version"],
    "python3": ["python3", "--version"],
    "git": ["git", "--version"],
}
# The baseline job's own `Tool ledger` step: tool_ledger.py report
# --require-zero-skips --min-ran 60; a shard's: --require-zero-skips.
MIN_RAN = 60
# How many earlier runs of the pull request are looked at, newest first.
MAX_RUNS = 50
# What a shard's artifact brings to a later run (no per-mutant logs).
SHARD_FILES = (
    "mutants-shard.txt",
    "memory-kills.tsv",
    "stamp.json",
    "tool-ledger.tsv",
) + tuple(f"mutants.out/{k}.txt" for k in mutants_summary.OUTCOMES)
# The baseline job's artifact (ADR-0128 §2) and what it brings.
BASELINE_ARTIFACT = mutants_summary.BASELINE_ARTIFACT
BASELINE_FILES = (
    "baseline-memory-kills.tsv",
    "baseline-build-peak.txt",
    "baseline-test-peak.txt",
    "stamp.json",
    "tool-ledger.tsv",
)


class InertError(Exception):
    pass


class ReuseError(Exception):
    pass


# ── The inert list ────────────────────────────────────────────────────


def parse_inert(text: str) -> list[str]:
    """The paths of mutants_inert.txt; each needs a reason comment right above it."""
    entries: list[str] = []
    reason = False
    for n, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line:
            reason = False
            continue
        if line.startswith("#"):
            reason = True
            continue
        p = PurePosixPath(line.rstrip("/"))
        if line.startswith("/") or "\\" in line or ".." in p.parts or str(p) in ("", "."):
            raise InertError(f"{INERT_FILE}:{n}: not a relative path inside the repository: {line}")
        if not reason:
            raise InertError(f"{INERT_FILE}:{n}: {line} has no reason comment right above it")
        if line in entries:
            raise InertError(f"{INERT_FILE}:{n}: {line} is listed twice")
        entries.append(line)
        reason = False
    return entries


def is_inert(path: str, entries: list[str]) -> bool:
    return any(path.startswith(e) if e.endswith("/") else path == e for e in entries)


# ── The key ───────────────────────────────────────────────────────────


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args], check=True, capture_output=True, text=True, encoding="utf-8"
    ).stdout


def ls_tree(repo: Path, rev: str = "HEAD") -> dict[str, str]:
    """path -> "mode type object" of every file of the commit."""
    out = subprocess.run(
        ["git", "-C", str(repo), "ls-tree", "-r", "-z", "--full-tree", rev], check=True, capture_output=True
    ).stdout.decode("utf-8")
    tree = {}
    for item in out.split("\0"):
        if item:
            meta, path = item.split("\t", 1)
            tree[path] = meta
    return tree


def digest(obj) -> str:
    return hashlib.sha256(json.dumps(obj, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def config_part(text: str) -> dict:
    """The cargo-mutants configuration as parsed, without exclude_re."""
    cfg = tomllib.loads(text)
    cfg.pop("exclude_re", None)
    return cfg


def compute_key(repo: Path, names: list[str], env: dict, tools: dict, rev: str = "HEAD") -> dict:
    full = ls_tree(repo, rev)
    entries = parse_inert(git(repo, "show", f"{rev}:{INERT_FILE}"))
    names = sorted(set(names))
    files = sorted({n.split(":", 1)[0] for n in names})
    missing = [f for f in files if f not in full]
    if missing:
        raise ReuseError(f"mutated file(s) not in the tree: {', '.join(missing)}")
    parts = {
        "mutants": {"names": names, "files": {f: full[f].split()[2] for f in files}},
        "tree": {p: m for p, m in full.items() if p != CONFIG and not is_inert(p, entries)},
        "environment": {
            "config": config_part(git(repo, "show", f"{rev}:{CONFIG}")),
            "workflow": full.get(WORKFLOW, ""),
            "verilator_env": git(repo, "show", f"{rev}:{VERILATOR_ENV}") if VERILATOR_ENV in full else "",
            "verilator_action": git(repo, "rev-parse", f"{rev}:{VERILATOR_ACTION}").strip()
            if any(p.startswith(VERILATOR_ACTION + "/") for p in full)
            else "",
            "inert": entries,
            "env": {v: env.get(v, "") for v in ENV_VARS},
            "tools": {t: tools.get(t, "") for t in TOOLS},
            "system": {s: tools.get(s, "") for s in SYSTEM},
            "runner": runner(env),
        },
    }
    hashes = {k: digest(v) for k, v in parts.items()}
    return {"version": KEY_VERSION, "key": digest(hashes), "hashes": hashes, "parts": parts}


def short(text: str, n: int = 5) -> str:
    return text if len(text) <= 60 else text[:57] + "..."


def listed(items: list[str], n: int = 10) -> str:
    return ", ".join(items[:n]) + (f" and {len(items) - n} more" if len(items) > n else "")


def key_diff(old: dict, new: dict) -> list[str]:
    """What differs between two keys, part by part; [] when they are equal."""
    out = []
    if old.get("version") != new.get("version"):
        return [f"version: {old.get('version')} -> {new.get('version')}"]
    po, pn = old.get("parts", {}), new.get("parts", {})
    mo, mn = po.get("mutants", {}), pn.get("mutants", {})
    gone = sorted(set(mo.get("names", [])) - set(mn.get("names", [])))
    came = sorted(set(mn.get("names", [])) - set(mo.get("names", [])))
    if gone or came:
        out.append(f"mutants: {len(came)} new, {len(gone)} gone: {listed(came + gone, 5)}")
    blobs = sorted(f for f in set(mo.get("files", {})) | set(mn.get("files", {}))
                   if mo.get("files", {}).get(f) != mn.get("files", {}).get(f))
    if blobs:
        out.append(f"mutants: the mutated file(s) differ: {listed(blobs)}")
    to, tn = po.get("tree", {}), pn.get("tree", {})
    paths = sorted(p for p in set(to) | set(tn) if to.get(p) != tn.get(p))
    if paths:
        out.append(f"tree: {len(paths)} path(s) differ: {listed(paths)}")
    eo, en = po.get("environment", {}), pn.get("environment", {})
    for field in sorted(set(eo) | set(en)):
        a, b = eo.get(field), en.get(field)
        if a == b:
            continue
        if isinstance(a, dict) and isinstance(b, dict):
            for sub in sorted(set(a) | set(b)):
                if a.get(sub) != b.get(sub):
                    out.append(f"environment: {field}.{sub}: {short(str(a.get(sub)))} -> {short(str(b.get(sub)))}")
        else:
            out.append(f"environment: {field} differs")
    if not out and old.get("key") != new.get("key"):
        out.append("key: the hash differs from its parts")
    return out


def runner(env: dict) -> dict:
    out = {v: env.get(v, "") for v in RUNNER_VARS}
    out["image_release"] = env.get("ImageVersion", "").split(".")[0]
    return out


def tool_versions(run) -> dict:
    """rustc, cargo-mutants, cargo-nextest and the runner's toolchain (SYSTEM)."""
    out = {
        "rustc": run(["rustc", "-vV"]),
        "cargo_mutants": run(["cargo", "mutants", "--version"]),
        "cargo_nextest": run(["cargo", "nextest", "--version"]),
    }
    for name, args in SYSTEM.items():
        out[name] = (run(args).splitlines() or [""])[0]
    return out


def run_text(args: list[str]) -> str:
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout.strip()


# ── A shard: the mask and the stamp ───────────────────────────────────


def mask(repo: Path) -> int:
    """Delete the inert paths from the checkout; print what went."""
    entries = parse_inert((repo / INERT_FILE).read_text(encoding="utf-8"))
    for e in entries:
        p = repo / e.rstrip("/")
        if p.is_dir() and e.endswith("/"):
            n = sum(1 for f in p.rglob("*") if f.is_file())
            shutil.rmtree(p)
            print(f"masked: {e} ({n} file(s))")
        elif p.is_file() and not e.endswith("/"):
            p.unlink()
            print(f"masked: {e} (1 file(s))")
        else:
            print(f"masked: {e} (absent)")
    left = [e for e in entries if (repo / e.rstrip("/")).exists()]
    if left:
        print(f"::error title=Mutation gate::inert path(s) still present after the mask: {', '.join(left)}")
        return 1
    return 0


def stamp(repo: Path, env: dict, run, timeout: int, baseline_seconds: int | None = None, role: str = "shard") -> dict:
    """The stamp of the baseline job (`role` baseline: its seconds, and the
    test timeout of ADR-0128 §3) or of a shard (the timeout it used)."""
    entries = parse_inert((repo / INERT_FILE).read_text(encoding="utf-8"))
    st = tool_versions(run)
    st["role"] = role
    st["verilator"] = run(["verilator", "--version"])
    st["yosys"] = run(["yosys", "-V"])
    st.update(runner(env))
    # The whole build number, for the log; not compared (RUNNER_VARS).
    st["ImageVersion"] = env.get("ImageVersion", "")
    st["inert"] = entries
    st["inert_present"] = [e for e in entries if (repo / e.rstrip("/")).exists()]
    st["baseline_seconds"] = baseline_seconds
    st["timeout"] = timeout
    return st


# ── Earlier runs ──────────────────────────────────────────────────────


def extract(data: bytes, expected: str | None, dest: Path, keep=None) -> None:
    """Unzip an artifact after checking its sha256 against GitHub's digest."""
    if not expected or not expected.startswith("sha256:"):
        raise ReuseError(f"the artifact has no sha256 digest ({expected})")
    got = "sha256:" + hashlib.sha256(data).hexdigest()
    if got != expected:
        raise ReuseError(f"digest mismatch: GitHub recorded {expected}, the download is {got}")
    dest.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for info in z.infolist():
            p = PurePosixPath(info.filename)
            if p.is_absolute() or ".." in p.parts or "\\" in info.filename:
                raise ReuseError(f"the artifact holds a path outside its directory: {info.filename}")
            if info.is_dir() or (keep is not None and not keep(info.filename)):
                continue
            target = dest.joinpath(*p.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(z.read(info))


class GhCli:
    """The earlier runs and their artifacts, read with `gh api` (GH_TOKEN, actions: read)."""

    def __init__(self, repo: str):
        self.repo = repo

    def _api(self, path: str) -> bytes:
        p = subprocess.run(["gh", "api", "-H", "Accept: application/vnd.github+json", path], capture_output=True)
        if p.returncode != 0:
            raise ReuseError(f"gh api {path}: {p.stderr.decode('utf-8', 'replace').strip()}")
        return p.stdout

    def runs(self, branch: str) -> list[dict]:
        q = urllib.parse.urlencode({"event": "pull_request", "branch": branch, "per_page": 100})
        return json.loads(self._api(f"repos/{self.repo}/actions/workflows/{WORKFLOW_FILE}/runs?{q}"))["workflow_runs"]

    def run(self, run_id) -> dict:
        return json.loads(self._api(f"repos/{self.repo}/actions/runs/{run_id}"))

    def artifacts(self, run_id) -> list[dict]:
        return json.loads(self._api(f"repos/{self.repo}/actions/runs/{run_id}/artifacts?per_page=100"))["artifacts"]

    def download(self, artifact: dict, dest: Path, keep=None) -> None:
        extract(self._api(f"repos/{self.repo}/actions/artifacts/{artifact['id']}/zip"), artifact.get("digest"), dest, keep)


def scope_problem(run: dict, scope: dict) -> str | None:
    """Why `run` is not a gate run of this very pull request, or None."""
    if run.get("id") == scope["run_id"]:
        return "this run"
    if run.get("event") != "pull_request":
        return f"event {run.get('event')}, not pull_request"
    if run.get("path") != WORKFLOW:
        return f"workflow {run.get('path')}"
    if (run.get("repository") or {}).get("full_name") != scope["repo"]:
        return "another repository"
    if (run.get("head_repository") or {}).get("full_name") != scope["head_repo"]:
        return f"head repository {(run.get('head_repository') or {}).get('full_name')}, not {scope['head_repo']}"
    if run.get("head_branch") != scope["head_ref"]:
        return f"head branch {run.get('head_branch')}"
    prs = [p.get("number") for p in run.get("pull_requests") or []]
    if scope["pr"] not in prs:
        return f"pull request(s) {prs or 'none'}, not #{scope['pr']}"
    return None


def read_names(path: Path) -> list[str]:
    return mutants_summary.read_lines(path)


def stamp_problems(where: str, st: dict, want: dict, inert: list[str]) -> list[str]:
    """What of a stamp differs from the key, or shows an inert path present."""
    problems = [f"{where}: {field} {short(str(st.get(field)))}, the key has {short(str(value))}"
                for field, value in want.items() if st.get(field) != value]
    if st.get("inert") != inert or st.get("inert_present"):
        problems.append(f"{where}: the inert paths were not masked: {st.get('inert_present')}")
    return problems


def check_baseline(baseline: Path | None, want: dict, inert: list[str], tools_seen: dict) -> tuple[list[str], list[str]]:
    """(problems, log lines) of an earlier run's baseline job (ADR-0128 §2)."""
    if baseline is None or not baseline.is_dir():
        return [f"no {BASELINE_ARTIFACT} artifact (the baseline job's ledger and stamp)"], []
    problems, log = [], []
    ledger = baseline / "tool-ledger.tsv"
    if not ledger.is_file():
        problems.append("the baseline: no tool ledger")
    else:
        try:
            code, out = tool_ledger.report(tool_ledger.load(ledger), "mutants-baseline", True, MIN_RAN)
        except SystemExit as err:
            code, out = 1, [f"malformed tool ledger: {err}"]
        log.append(out[0])
        if code:
            problems.append("the baseline: " + out[0] + "".join(f"; {l.strip()}" for l in out[1:] if "::error" in l))
    st_path = baseline / "stamp.json"
    if not st_path.is_file():
        problems.append("the baseline: no stamp (tool versions, runner image, inert paths)")
        return problems, log
    st = json.loads(st_path.read_text(encoding="utf-8"))
    if st.get("role") != "baseline":
        problems.append(f"the baseline: its stamp is a {st.get('role')} stamp")
    problems += stamp_problems("the baseline", st, want, inert)
    for t in ("verilator", "yosys"):
        tools_seen.setdefault(t, set()).add(st.get(t))
    return problems, log


def check_outcomes(lists: Path, shards: Path, key: dict, current: list[str],
                   baseline: Path | None = None, clean: bool = True) -> tuple[list[str], list[str], dict]:
    """(problems, log lines, facts) of an earlier run's baseline and shards.

    `clean` (a reuse, #318): no missed, timeout or memory-limit mutant, and
    every mutant of `current` caught or unviable then. A retest base
    (#364) needs the rest only: complete, the baseline passed, SKIP: 0, the
    key's tools and image.
    """
    problems, log = [], []
    n = len(list(lists.glob("shard-*.txt")))
    if n == 0:
        return ["it has no shard list"], log, {}
    env = key["parts"]["environment"]
    want = {**env["tools"], **env.get("system", {}), **env["runner"]}
    results, tools_seen = [], {}
    more, blog = check_baseline(baseline, want, env["inert"], tools_seen)
    problems += more
    log += blog
    for k in range(n):
        r = mutants_summary.shard_result(shards, lists, k, n)
        results.append(r)
        if not mutants_summary.complete(r):
            problems.append(f"shard {k}: {mutants_summary.status(r).replace('**', '')}"
                            + (f" ({len(r['untested'])} not tested)" if r["untested"] and r["present"] else ""))
        d = mutants_summary.shard_dir(shards, k, n)
        ledger = d / "tool-ledger.tsv" if d else None
        if ledger is None or not ledger.is_file():
            problems.append(f"shard {k}: no tool ledger")
        else:
            try:
                code, out = tool_ledger.report(tool_ledger.load(ledger), f"mutants-{k}", True, 0)
            except SystemExit as err:
                code, out = 1, [f"malformed tool ledger: {err}"]
            log.append(out[0])
            if code:
                problems.append(f"shard {k}: {out[0]}" + "".join(f"; {l.strip()}" for l in out[1:] if "::error" in l))
        st_path = d / "stamp.json" if d else None
        if st_path is None or not st_path.is_file():
            problems.append(f"shard {k}: no stamp (tool versions, runner image, inert paths)")
            continue
        st = json.loads(st_path.read_text(encoding="utf-8"))
        problems += stamp_problems(f"shard {k}", st, want, env["inert"])
        for t in ("verilator", "yosys"):
            tools_seen.setdefault(t, set()).add(st.get(t))
    for t, seen in tools_seen.items():
        if len(seen) != 1:
            problems.append(f"the shards ran different {t} versions: {sorted(map(str, seen))}")
    tot = {kind: sorted(x for r in results for x in r[kind]) for kind in mutants_summary.KINDS}
    for kind, word in (("missed", "missed"), ("timeout", "timeout"), ("memory", "memory limit")):
        if tot[kind] and clean:
            problems.append(f"{len(tot[kind])} {word}: {listed(tot[kind], 3)}")
    tested = set(tot["caught"]) | set(tot["unviable"])
    new = sorted(set(current) - tested)
    if new and clean:
        problems.append(f"{len(new)} mutant(s) of this run were not tested then: {listed(new, 3)}")
    facts = {
        "shards": n,
        "counts": {kind: len(v) for kind, v in tot.items()},
        "tools": {t: sorted(map(str, s))[0] for t, s in tools_seen.items()},
        "excluded_now": sorted(tested - set(current)),
    }
    return problems, log, facts


def find(gh, scope: dict, key: dict, current: list[str], work: Path) -> dict:
    """Look for an earlier passed run with this key; on reuse, write work/reuse/."""
    print(f"Reuse key {key['key']} (" + ", ".join(f"{k} {v[:12]}" for k, v in key["hashes"].items()) + ")")
    if scope["event"] != "pull_request":
        return {"decision": "rerun", "reason": f"reuse is off for a {scope['event']} run"}
    if scope["head_repo"] != scope["repo"]:
        return {"decision": "rerun", "reason": f"reuse is off for a pull request from a fork ({scope['head_repo']})"}
    try:
        runs = sorted(gh.runs(scope["head_ref"]), key=lambda r: (r.get("created_at", ""), r.get("id", 0)), reverse=True)
    except ReuseError as err:
        return {"decision": "rerun", "reason": f"the earlier runs could not be read: {err}"}
    looked = 0
    for r in runs:
        if looked >= MAX_RUNS:
            print(f"looked at the {MAX_RUNS} newest earlier runs; older ones are not")
            break
        rid, sha = r.get("id"), r.get("head_sha", "")
        head = f"earlier run {rid} (SHA {sha})"
        why = scope_problem(r, scope)
        if why == "this run":
            continue
        looked += 1
        if why:
            print(f"{head}: {why}: not this pull request's gate, passed over")
            continue
        if r.get("status") != "completed":
            print(f"{head}: status {r.get('status')}, not completed: passed over")
            continue
        if r.get("conclusion") in ("cancelled", "skipped"):
            print(f"{head}: {r.get('conclusion')}: says nothing about the outcomes, passed over")
            continue
        # Downloads go to a scratch directory: only the reused run's files
        # land in work/reuse (and in this run's mutants-list artifact).
        tmp = Path(tempfile.mkdtemp(prefix=f"mutants-run-{rid}-"))
        try:
            arts = {a["name"]: a for a in gh.artifacts(rid) if not a.get("expired")}
            if "mutants-list" not in arts:
                print(f"{head}: no mutants-list artifact (expired or never uploaded), passed over")
                continue
            gh.download(arts["mutants-list"], tmp / "lists")
        except ReuseError as err:
            return {"decision": "rerun", "reason": f"{head}: its mutant lists could not be read: {err}"}
        kp = tmp / "lists" / "key.json"
        if not kp.is_file():
            print(f"{head}: no reuse key (a run before #318, or a gate with nothing to test), passed over")
            continue
        old = json.loads(kp.read_text(encoding="utf-8"))
        diff = key_diff(old, key)
        if diff:
            print(f"{head}: the key differs; " + "; ".join(diff))
            continue
        if (tmp / "lists" / "reuse" / "reuse.json").is_file():
            src = json.loads((tmp / "lists" / "reuse" / "reuse.json").read_text(encoding="utf-8")).get("run")
            print(f"{head}: same key, but it reused run {src} itself: passed over")
            continue
        if (tmp / "lists" / "retest" / "retest.json").is_file():
            src = json.loads((tmp / "lists" / "retest" / "retest.json").read_text(encoding="utf-8")).get("run")
            print(f"{head}: same key, but it retested part of run {src} (#364): passed over")
            continue
        # The newest earlier run with this key decides: a failed one is not
        # passed over for an older passed one (the same inputs gave two
        # results). Its outcomes are still read, so the log says why.
        problems = [] if r.get("conclusion") == "success" else [f"concluded {r.get('conclusion')}"]
        try:
            n = len(list((tmp / "lists").glob("shard-*.txt")))
            for k in range(n):
                name = f"mutants-out-{k}"
                if name not in arts:
                    raise ReuseError(f"no {name} artifact")
                gh.download(arts[name], tmp / "shards" / name, keep=lambda f: f in SHARD_FILES)
            if BASELINE_ARTIFACT in arts:
                gh.download(arts[BASELINE_ARTIFACT], tmp / "baseline", keep=lambda f: f in BASELINE_FILES)
        except ReuseError as err:
            problems.append(f"its shard outcomes could not be read: {err}")
            return {"decision": "rerun", "run": rid,
                    "reason": f"{head} has the same key but is not reusable: " + "; ".join(problems)}
        more, log, facts = check_outcomes(tmp / "lists", tmp / "shards", key, current, tmp / "baseline")
        problems += more
        for line in log:
            print(f"{head}: {line}")
        if problems:
            return {"decision": "rerun", "run": rid,
                    "reason": f"{head} has the same key but is not reusable: " + "; ".join(problems)}
        out = work / "reuse"
        if out.exists():
            shutil.rmtree(out)
        out.mkdir(parents=True)
        shutil.move(str(tmp / "lists"), str(out / "lists"))
        shutil.move(str(tmp / "shards"), str(out / "shards"))
        shutil.move(str(tmp / "baseline"), str(out / "baseline"))
        info = {"run": rid, "sha": sha, "key": key["key"], "created_at": r.get("created_at"),
                "url": r.get("html_url", ""), **facts}
        (out / "reuse.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
        c = facts["counts"]
        print(f"{head}: same key; {sum(c.values())} mutant(s) in {facts['shards']} shard(s): "
              f"caught={c['caught']} unviable={c['unviable']} missed=0 timeout=0 memory=0; "
              f"the baseline passed (--min-ran {MIN_RAN}), every shard completed, SKIP: 0, the same tools and runner image")
        if facts["excluded_now"]:
            print(f"excluded by today's exclude_re, tested then: {listed(facts['excluded_now'], 20)}")
        print(f"reused: run {rid} (SHA {sha}), key {key['key']}")
        return {"decision": "reuse", "run": rid, "sha": sha, "excluded_now": facts["excluded_now"]}
    return {"decision": "rerun", "reason": f"no earlier run of pull request #{scope['pr']} with key {key['key']}"}


# ── Retest after a test-only push (#364, ADR-0129) ─────────────────────

# A test file: anything under a workspace package's `tests/` directory
# (`crates/<dir>/tests/...`). The repository's own `tests/` (the ui and
# real-bug corpora, read by several crates) is not one: a change there runs
# the gate in full.
TEST_PATH = re.compile(r"^(crates/[^/]+)/tests/")
# nextest's failure lines in a mutant's log: FAIL, SIGSEGV, TIMEOUT, ... with
# the binary id and the test name. Lines after "Cancelling due to test
# failure" are the tests nextest terminated, not the one that caught it.
FAIL_LINE = re.compile(
    r"^\s+(FAIL|SIG[A-Z]+|TIMEOUT|ABORT|LEAK-FAIL)\s+\[[^\]]*\]\s+(?:\(\s*\d+/\s*\d+\)\s+)?(\S+)\s+(\S+)"
)
CANCEL = "Cancelling due to test failure"
ANSI = re.compile(r"\x1b\[[0-9;]*m")
# What the base run's shard artifacts bring for a retest: the outcome lists,
# outcomes.json (each mutant's log path) and the logs of the mutants.
RETEST_KEEP = lambda f: f in SHARD_FILES or f == "mutants.out/outcomes.json" or f.startswith("mutants.out/log/")  # noqa: E731
RETESTED_KINDS = ("missed", "timeout", "memory")


def rust_escape(name: str) -> str:
    """An exact-name regex for cargo-mutants' --re (Rust regex) and Python's re."""
    return "".join("\\" + c if c in "\\.+*?()|[]{}^$#&-~" else c for c in name)


def names_regex(names: list[str]) -> str:
    return "^(" + "|".join(rust_escape(n) for n in sorted(names)) + ")$"


def changed_paths(old: dict, new: dict) -> list[str]:
    """Paths of the key's tree part that differ (added, deleted or changed)."""
    to, tn = old["parts"]["tree"], new["parts"]["tree"]
    return sorted(p for p in set(to) | set(tn) if to.get(p) != tn.get(p))


def changed_binaries(paths: list[str], metadata: dict) -> tuple[set[str], set[str], list[str]]:
    """(exact binary ids, packages whose every binary counts as changed, non-test paths).

    `crates/<dir>/tests/<name>.rs` that is an integration test target is that
    binary; any other path under a package's `tests/` (a module, a data
    file, a deleted target) counts for every test binary of the package.
    """
    tests, _ = mutants_nextest.workspace_binaries(metadata)
    root = Path(metadata["workspace_root"])
    packages = {}
    for pkg in metadata["packages"]:
        if pkg["id"] in metadata["workspace_members"]:
            d = Path(pkg["manifest_path"]).parent.resolve().relative_to(root.resolve()).as_posix()
            packages[d] = pkg["name"]
    exact, whole, other = set(), set(), []
    for p in paths:
        m = TEST_PATH.match(p)
        if not m or m[1] not in packages:
            other.append(p)
        elif p in tests:
            exact.add(tests[p])
        else:
            whole.add(packages[m[1]])
    return exact, whole, other


def binary_changed(binary: str, exact: set[str], whole: set[str]) -> bool:
    return binary in exact or any(binary == p or binary.startswith(p + "::") for p in whole)


def catchers(shard: Path) -> dict[str, list[str]]:
    """Mutant name -> the test binaries of its failure lines (before nextest
    cancelled the rest), from cargo-mutants' outcomes.json and the logs."""
    out: dict[str, list[str]] = {}
    oj = shard / "mutants.out" / "outcomes.json"
    if not oj.is_file():
        return out
    for o in json.loads(oj.read_text(encoding="utf-8")).get("outcomes", []):
        sc = o.get("scenario")
        if not isinstance(sc, dict) or "Mutant" not in sc:
            continue
        name = sc["Mutant"].get("name")
        log = shard / "mutants.out" / (o.get("log_path") or "-")
        bins: list[str] = []
        if log.is_file():
            for line in ANSI.sub("", log.read_text(encoding="utf-8", errors="replace")).splitlines():
                if CANCEL in line:
                    break
                m = FAIL_LINE.match(line)
                if m and m[2] not in bins:
                    bins.append(m[2])
        out[name] = bins
    return out


def plan_retest(lists: Path, shards: Path, current: list[str], exact: set[str], whole: set[str]) -> dict:
    """Which mutants of `current` to test again, which outcomes to carry over.

    Tested again: a mutant the base run reported missed, timeout or memory
    limit; one it did not test (excluded then); one caught by a test binary
    this push changed; one caught whose catching binary its log does not
    name. Carried over: caught by an unchanged binary, unviable.
    """
    n = len(list(lists.glob("shard-*.txt")))
    outcome: dict[str, str] = {}
    caught_by: dict[str, list[str]] = {}
    for k in range(n):
        r = mutants_summary.shard_result(shards, lists, k, n)
        for kind in mutants_summary.KINDS:
            for x in r[kind]:
                outcome[x] = kind
        d = mutants_summary.shard_dir(shards, k, n)
        if d is not None:
            caught_by.update(catchers(d))
    retest: dict[str, str] = {}
    carried: dict[str, dict] = {}
    for x in current:
        kind = outcome.get(x)
        if kind is None:
            retest[x] = "not tested then"
        elif kind in RETESTED_KINDS:
            retest[x] = f"{mutants_summary_label(kind)} then"
        elif kind == "unviable":
            carried[x] = {"outcome": "unviable"}
        else:
            bins = caught_by.get(x, [])
            hit = [b for b in bins if binary_changed(b, exact, whole)]
            if not bins:
                retest[x] = "caught then; its log names no failing test binary"
            elif hit:
                retest[x] = f"caught then by {', '.join(hit)}, changed since"
            else:
                carried[x] = {"outcome": "caught", "caught_by": bins}
    return {"retest": retest, "carried": carried}


def mutants_summary_label(kind: str) -> str:
    return "memory limit" if kind == "memory" else kind


def full_run(lists: Path) -> bool:
    """A gate run that tested every mutant itself: not a reuse, not a retest."""
    return not (lists / "reuse" / "reuse.json").is_file() and not (lists / "retest" / "retest.json").is_file()


def retest_from(gh, base: dict, key: dict, current: list[str], work: Path, metadata: dict) -> dict:
    """Check one base run and, when it qualifies, write work/retest/."""
    rid, sha = base.get("id"), base.get("head_sha", "")
    head = f"base run {rid} (SHA {sha})"
    if base.get("status") != "completed":
        return {"decision": "full", "reason": f"{head}: status {base.get('status')}, not completed"}
    if base.get("conclusion") in ("cancelled", "skipped"):
        return {"decision": "full", "reason": f"{head}: {base.get('conclusion')}"}
    tmp = Path(tempfile.mkdtemp(prefix=f"mutants-base-{rid}-"))
    try:
        arts = {a["name"]: a for a in gh.artifacts(rid) if not a.get("expired")}
        if "mutants-list" not in arts:
            return {"decision": "full", "reason": f"{head}: no mutants-list artifact"}
        gh.download(arts["mutants-list"], tmp / "lists")
        kp = tmp / "lists" / "key.json"
        if not kp.is_file():
            return {"decision": "full", "reason": f"{head}: no reuse key"}
        if not full_run(tmp / "lists"):
            return {"decision": "full", "reason": f"{head}: it did not test every mutant itself (a reuse or a retest)"}
        old = json.loads(kp.read_text(encoding="utf-8"))
        for part in ("mutants", "environment"):
            if old.get("hashes", {}).get(part) != key["hashes"][part] or old["parts"][part] != key["parts"][part]:
                diff = [l for l in key_diff(old, key) if l.startswith(part + ":")] or [f"{part}: differs"]
                return {"decision": "full", "reason": f"{head}: " + "; ".join(diff)}
        paths = changed_paths(old, key)
        exact, whole, other = changed_binaries(paths, metadata)
        if other:
            return {"decision": "full", "reason": f"{head}: {len(other)} changed path(s) are not test files: {listed(other, 5)}"}
        n = len(list((tmp / "lists").glob("shard-*.txt")))
        for k in range(n):
            name = f"mutants-out-{k}"
            if name not in arts:
                return {"decision": "full", "reason": f"{head}: no {name} artifact"}
            gh.download(arts[name], tmp / "shards" / name, keep=RETEST_KEEP)
        if BASELINE_ARTIFACT in arts:
            gh.download(arts[BASELINE_ARTIFACT], tmp / "baseline", keep=lambda f: f in BASELINE_FILES)
    except ReuseError as err:
        return {"decision": "full", "reason": f"{head}: its artifacts could not be read: {err}"}
    problems, log, facts = check_outcomes(tmp / "lists", tmp / "shards", key, current, tmp / "baseline", clean=False)
    for line in log:
        print(f"{head}: {line}")
    if problems:
        return {"decision": "full", "reason": f"{head} is not complete: " + "; ".join(problems)}
    plan = plan_retest(tmp / "lists", tmp / "shards", current, exact, whole)
    out = work / "retest"
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    info = {
        "run": rid, "sha": sha, "key": key["key"], "base_key": old.get("key"), "url": base.get("html_url", ""),
        "created_at": base.get("created_at"), "base_counts": facts.get("counts", {}),
        "changed_paths": paths, "changed_binaries": sorted(exact), "changed_packages": sorted(whole),
        **plan,
    }
    (out / "retest.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    (out / "names.txt").write_text("".join(x + "\n" for x in sorted(plan["retest"])), encoding="utf-8")
    (out / "retest.re").write_text(names_regex(list(plan["retest"])) + "\n" if plan["retest"] else "", encoding="utf-8")
    print(f"{head}: the mutants and the environment are the same; {len(paths)} changed path(s), all test files: {listed(paths, 10)}")
    print(f"changed test binaries: {', '.join(sorted(exact)) or 'none'}; every binary of: {', '.join(sorted(whole)) or 'none'}")
    for x, why in sorted(plan["retest"].items()):
        print(f"retest: {x} ({why})")
    print(f"retest: run {rid} (SHA {sha}): {len(plan['retest'])} of {len(current)} retested, {len(plan['carried'])} carried over")
    return {"decision": "retest", "run": rid, "sha": sha, **plan}


def find_retest(gh, scope: dict, key: dict, current: list[str], work: Path, metadata: dict, run_id=None) -> dict:
    """The last full gate run of this pull request (or the run a hand gate run names) and a retest plan."""
    if run_id is not None:
        if scope["event"] == "pull_request":
            return {"decision": "full", "reason": "retest_from is for a hand run only"}
        try:
            base = gh.run(run_id)
        except ReuseError as err:
            return {"decision": "full", "reason": f"run {run_id} could not be read: {err}"}
        if base.get("path") != WORKFLOW or (base.get("repository") or {}).get("full_name") != scope["repo"]:
            return {"decision": "full", "reason": f"run {run_id} is not a {WORKFLOW} run of {scope['repo']}"}
        return retest_from(gh, base, key, current, work, metadata)
    if scope["event"] != "pull_request":
        return {"decision": "full", "reason": f"a {scope['event']} run retests only with retest_from"}
    if scope["head_repo"] != scope["repo"]:
        return {"decision": "full", "reason": f"no retest for a pull request from a fork ({scope['head_repo']})"}
    try:
        runs = sorted(gh.runs(scope["head_ref"]), key=lambda r: (r.get("created_at", ""), r.get("id", 0)), reverse=True)
    except ReuseError as err:
        return {"decision": "full", "reason": f"the earlier runs could not be read: {err}"}
    looked = 0
    for r in runs:
        why = scope_problem(r, scope)
        if why == "this run":
            continue
        if looked >= MAX_RUNS:
            break
        looked += 1
        if why:
            continue
        if r.get("status") != "completed" or r.get("conclusion") in ("cancelled", "skipped"):
            print(f"earlier run {r.get('id')}: {r.get('status')} {r.get('conclusion')}: passed over")
            continue
        try:
            arts = {a["name"]: a for a in gh.artifacts(r["id"]) if not a.get("expired")}
            if "mutants-list" not in arts:
                print(f"earlier run {r.get('id')}: no mutants-list artifact: passed over")
                continue
            tmp = Path(tempfile.mkdtemp(prefix=f"mutants-scan-{r['id']}-"))
            gh.download(arts["mutants-list"], tmp)
        except ReuseError as err:
            return {"decision": "full", "reason": f"earlier run {r.get('id')}: {err}"}
        if not (tmp / "key.json").is_file():
            print(f"earlier run {r.get('id')}: no reuse key (nothing to test, or before #318): passed over")
            continue
        if not full_run(tmp):
            print(f"earlier run {r.get('id')}: a reuse or a retest, not a full run: passed over")
            continue
        # The newest full run decides; an older one is never looked at.
        return retest_from(gh, r, key, current, work, metadata)
    return {"decision": "full", "reason": f"no earlier full gate run of pull request #{scope['pr']}"}


def retest_check(lists: Path) -> int:
    """After the tools job made the retest list: R ⊆ all.txt ⊆ gate-all.txt.

    A mutant the line filter adds beyond R (a field-deletion mutant on a
    retested line, cargo-mutants 27.1.0 does not filter those with --re)
    is tested again too; the rest of the gate's list is carried over."""
    info_path = lists / "retest" / "retest.json"
    info = json.loads(info_path.read_text(encoding="utf-8"))
    gate = read_names(lists / "gate-all.txt")
    tested = read_names(lists / "all.txt")
    want = set(info["retest"])
    errors = [f"to retest, not in this run's list: {x}" for x in sorted(want - set(tested))]
    errors += [f"in this run's list, not on the gate's: {x}" for x in sorted(set(tested) - set(gate))]
    extra = sorted(set(tested) - want - (set(tested) - set(gate)))
    for x in extra:
        info["retest"][x] = "on a retested line (the line filter keeps it); tested again"
        info["carried"].pop(x, None)
    carried = {x: info["carried"].get(x) for x in gate if x not in set(tested)}
    errors += [f"carried over with no outcome from run {info['run']}: {x}" for x, v in sorted(carried.items()) if not v]
    for e in errors:
        print(f"::error title=Mutation gate retest::{e}")
    if errors:
        return 1
    info["carried"] = carried
    info_path.write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
    print(f"retest: run {info['run']} (SHA {info['sha']}): {len(tested)} of {len(gate)} retested"
          f" ({len(extra)} on a retested line), {len(carried)} carried over")
    return 0


def self_test() -> int:
    inert_text = (
        "# Header.\n\n"
        "# The ADRs: read by people only.\n"
        "docs/adr/\n"
        "# The changelog.\n"
        "CHANGELOG.md\n"
    )
    entries = parse_inert(inert_text)
    assert entries == ["docs/adr/", "CHANGELOG.md"], entries
    assert is_inert("docs/adr/ADR-0001.md", entries) and is_inert("CHANGELOG.md", entries)
    assert not is_inert("docs/adrx.md", entries) and not is_inert("docs/CHANGELOG.md", entries)
    for bad in ("docs/adr/\n", "# r\n/abs\n", "# r\n../x\n", "# r\ndocs/adr/\n# r\ndocs/adr/\n", "# r\n\n"):
        try:
            got = parse_inert(bad)
        except InertError:
            continue
        if bad == "# r\n\n" and got == []:
            continue
        raise AssertionError(f"accepted: {bad!r} -> {got}")

    def git(repo: Path, *args: str) -> str:
        return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout

    files = {
        "crates/a/src/lib.rs": "pub fn f() -> u8 { 1 }\n",
        "crates/a/tests/t.rs": "#[test] fn t() { assert_eq!(a::f(), 1); }\n",
        "docs/adr/ADR-0001.md": "# ADR-0001\n",
        "docs/adrx.md": "not inert\n",
        "CHANGELOG.md": "# Changes\n",
        "README.md": "# Read me\n",
        "book/src/p.md": "page\n",
        "tests/ui/pass/a.volt": "mod a {}\n",
        "scripts/ci/verilator.env": "VERILATOR_VERSION=5.054\n",
        ".github/actions/verilator/action.yml": "name: v\n",
        ".github/workflows/mutants.yml": "name: Mutation test\n",
        INERT_FILE: inert_text,
        ".cargo/mutants.toml": (
            'examine_globs = ["crates/*/src/**/*.rs"]\n'
            "exclude_re = [\n    # A1. reason\n    # expect: x\n    \"x$\",\n]\n"
            "timeout_multiplier = 2.0\n"
        ),
    }
    env = {
        "CARGO_MUTANTS_VERSION": "27.1.0",
        "CARGO_NEXTEST_VERSION": "0.9.151",
        "OSS_CAD_SUITE_DATE": "2026-09-21",
        "MUTANTS_MEMORY_LIMIT_MIB": "12288",
        "MUTANTS_GATE_SECONDS_PER_MUTANT": "600",
        "ImageOS": "ubuntu24",
        "ImageVersion": "20261111.82.6",
        "RUNNER_OS": "Linux",
        "RUNNER_ARCH": "X64",
    }
    tools = {"rustc": "rustc 1.91.0", "cargo_mutants": "cargo-mutants 27.1.0", "cargo_nextest": "cargo-nextest 0.9.151",
             "gxx": "g++ (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0", "glibc": "ldd (Ubuntu GLIBC 2.39-0ubuntu8.6) 2.39",
             "binutils": "GNU ld (GNU Binutils for Ubuntu) 2.42", "make": "GNU Make 4.3", "python3": "Python 3.12.3",
             "git": "git version 2.51.0"}
    names = ["crates/a/src/lib.rs:1:20: replace f -> u8 with 0", "crates/a/src/lib.rs:1:20: replace f -> u8 with 1"]

    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp, "repo")
        repo.mkdir()
        git(repo, "init", "-q")
        git(repo, "config", "user.email", "t@example.invalid")
        git(repo, "config", "user.name", "t")

        def commit(changes: dict[str, str | None]) -> None:
            for path, text in changes.items():
                p = repo / path
                if text is None:
                    p.unlink()
                    continue
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text, encoding="utf-8", newline="\n")
            git(repo, "add", "-A")
            git(repo, "commit", "-q", "--allow-empty", "-m", "c")

        commit(files)
        base = compute_key(repo, names, env, tools)
        assert base["key"] == compute_key(repo, list(reversed(names)), env, tools)["key"], "order of the list"
        assert "docs/adr/ADR-0001.md" not in base["parts"]["tree"] and "CHANGELOG.md" not in base["parts"]["tree"]
        assert ".cargo/mutants.toml" not in base["parts"]["tree"], "the configuration is in the environment part"
        assert "exclude_re" not in base["parts"]["environment"]["config"]
        assert base["parts"]["environment"]["config"]["timeout_multiplier"] == 2.0
        assert base["parts"]["mutants"]["files"] == {"crates/a/src/lib.rs": git(repo, "rev-parse", "HEAD:crates/a/src/lib.rs").strip()}

        def key_after(changes: dict[str, str | None], **over) -> dict:
            commit(changes)
            k = compute_key(repo, over.get("names", names), over.get("env", env), over.get("tools", tools))
            git(repo, "reset", "-q", "--hard", "HEAD~1")
            return k

        # (a) these keep the key.
        for what, changes in (
            ("an ADR", {"docs/adr/ADR-0001.md": "# ADR-0001\n\nchanged\n"}),
            ("a new ADR", {"docs/adr/ADR-0002.md": "# ADR-0002\n"}),
            ("the changelog", {"CHANGELOG.md": "# Changes\n- one\n"}),
            ("exclude_re alone", {".cargo/mutants.toml": files[".cargo/mutants.toml"].replace('"x$"', '"y$"')}),
            ("a comment of the configuration", {".cargo/mutants.toml": "# note\n" + files[".cargo/mutants.toml"]}),
        ):
            k = key_after(changes)
            assert k["key"] == base["key"] and key_diff(base, k) == [], (what, key_diff(base, k))
        # Another build of the same weekly runner image (run 44186837945: the
        # first job on 20261111.370.1, its shard on 20261111.127.1).
        for build in ("20261111.370.1", "20261111.127.1"):
            k = key_after({}, env=dict(env, ImageVersion=build))
            assert k["key"] == base["key"], (build, key_diff(base, k))
        # (b), (d) these change it, and the difference names the part.
        for what, changes, part, word in (
            ("a Rust test", {"crates/a/tests/t.rs": "#[test] fn t() {}\n"}, "tree", "crates/a/tests/t.rs"),
            ("a Rust source", {"crates/a/src/lib.rs": "pub fn f() -> u8 { 2 }\n"}, "tree", "crates/a/src/lib.rs"),
            ("a book page", {"book/src/p.md": "page 2\n"}, "tree", "book/src/p.md"),
            ("a test input", {"tests/ui/pass/b.volt": "mod b {}\n"}, "tree", "tests/ui/pass/b.volt"),
            ("the Verilator pin", {"scripts/ci/verilator.env": "VERILATOR_VERSION=5.056\n"}, "environment", "verilator_env"),
            ("the Verilator action", {".github/actions/verilator/action.yml": "name: w\n"}, "environment", "verilator_action"),
            ("the README (not inert)", {"README.md": "# Read me!\n"}, "tree", "README.md"),
            ("a path next to an inert one", {"docs/adrx.md": "x\n"}, "tree", "docs/adrx.md"),
            ("the workflow", {".github/workflows/mutants.yml": "name: Mutation test\n# c\n"}, "environment", "workflow"),
            (
                "timeout_multiplier",
                {".cargo/mutants.toml": files[".cargo/mutants.toml"].replace("2.0", "3.0")},
                "environment",
                "timeout_multiplier",
            ),
            ("the inert list", {INERT_FILE: inert_text + "# r\ndocs/adrx.md\n"}, "environment", "inert"),
        ):
            k = key_after(changes)
            diff = key_diff(base, k)
            assert k["key"] != base["key"], what
            assert any(l.startswith(part + ":") and word in l for l in diff), (what, diff)
        # A moved base: other mutants on the changed lines, or another tree.
        k = key_after({}, names=names[:1])
        assert k["key"] != base["key"] and any(l.startswith("mutants:") for l in key_diff(base, k)), key_diff(base, k)
        k = key_after({"crates/b/src/lib.rs": "pub fn g() {}\n"})
        assert any(l.startswith("tree:") and "crates/b/src/lib.rs" in l for l in key_diff(base, k)), key_diff(base, k)
        # The environment outside the tree.
        for field, over in (
            ("image_release", {"env": dict(env, ImageVersion="20261118.1.0")}),
            ("ImageOS", {"env": dict(env, ImageOS="ubuntu26")}),
            ("gxx", {"tools": dict(tools, gxx="g++ (Ubuntu 15.2.0-4ubuntu4) 15.2.0")}),
            ("glibc", {"tools": dict(tools, glibc="ldd (Ubuntu GLIBC 2.43-2ubuntu1) 2.43")}),
            ("MUTANTS_MEMORY_LIMIT_MIB", {"env": dict(env, MUTANTS_MEMORY_LIMIT_MIB="8192")}),
            ("CARGO_MUTANTS_VERSION", {"env": dict(env, CARGO_MUTANTS_VERSION="27.2.0")}),
            ("rustc", {"tools": dict(tools, rustc="rustc 1.92.0")}),
        ):
            k = key_after({}, **over)
            assert k["key"] != base["key"] and any(field in l for l in key_diff(base, k)), (field, key_diff(base, k))

        # The mask: every inert path is gone, nothing else.
        work = Path(tmp, "work")
        subprocess.run(["git", "clone", "-q", str(repo), str(work)], check=True)
        with contextlib.redirect_stdout(io.StringIO()) as out:
            assert mask(work) == 0
        assert not (work / "docs/adr").exists() and not (work / "CHANGELOG.md").exists()
        assert (work / "docs/adrx.md").is_file() and (work / "README.md").is_file()
        assert "docs/adr/ (1 file(s))" in out.getvalue(), out.getvalue()
        st = stamp(work, env, lambda args: {"rustc": tools["rustc"], "cargo": "x"}.get(args[0], f"{args[0]} 1"), 3610, 1200, "baseline")
        assert st["inert_present"] == [] and st["inert"] == entries, st
        assert st["role"] == "baseline" and st["baseline_seconds"] == 1200 and st["timeout"] == 3610, st
        st = stamp(work, env, lambda args: "v", 3610)
        assert st["role"] == "shard" and st["baseline_seconds"] is None and st["timeout"] == 3610, st
        assert stamp(repo, env, lambda args: "v", 3610)["inert_present"] == entries

        # Artifacts: the digest of the zip is checked before anything is read.
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, "w") as z:
            z.writestr("key.json", "{}")
            z.writestr("mutants.out/log/big.log", "x" * 100)
        data = buf.getvalue()
        good = "sha256:" + hashlib.sha256(data).hexdigest()
        extract(data, good, Path(tmp, "x1"), keep=lambda n: not n.startswith("mutants.out/log/"))
        assert Path(tmp, "x1/key.json").is_file() and not Path(tmp, "x1/mutants.out/log").exists()
        for digest in ("sha256:" + "0" * 64, None, "md5:abc"):
            try:
                extract(data, digest, Path(tmp, "x2"))
            except ReuseError:
                continue
            raise AssertionError(f"extracted with digest {digest}")
        evil = io.BytesIO()
        with zipfile.ZipFile(evil, "w") as z:
            z.writestr("../escape.txt", "x")
        try:
            extract(evil.getvalue(), "sha256:" + hashlib.sha256(evil.getvalue()).hexdigest(), Path(tmp, "x3"))
        except ReuseError:
            pass
        else:
            raise AssertionError("a member outside the directory was extracted")

        # The earlier runs, through a fake GitHub.
        cur = dict(base)
        scope = {"repo": "o/r", "pr": 7, "head_repo": "o/r", "head_ref": "feat/x", "run_id": 900, "event": "pull_request"}
        ledger_ok = "".join(f"RAN\tb\tt{i}\tverilator\t1\n" for i in range(61))
        # A shard has no baseline: its mutants reach few tool tests (ADR-0128 §2).
        ledger_few = "".join(f"RAN\tb\tt{i}\tverilator\t1\n" for i in range(3))

        def run(rid, *, key=cur, shards=((names[0],), (names[1],)), outcome="caught", conclusion="success",
                status="completed", event="pull_request", pr=7, head_repo="o/r", branch="feat/x", reused=None,
                stamp_over=None, ledger=ledger_few, kills=None, drop=None, tamper=False, no_key=False,
                baseline_ledger=ledger_ok, baseline_over=None, baseline_drop=None, no_baseline=False):
            r = {
                "id": rid, "event": event, "head_branch": branch, "head_sha": f"{rid:040d}",
                "path": ".github/workflows/mutants.yml", "status": status, "conclusion": conclusion,
                "head_repository": {"full_name": head_repo}, "repository": {"full_name": "o/r"},
                "pull_requests": [{"number": pr}] if pr else [], "created_at": "2026-11-18T00:00:00Z",
            }
            lists = {f"shard-{k}.txt": "".join(n + "\n" for n in s) for k, s in enumerate(shards)}
            lists["all.txt"] = "".join(n + "\n" for s in shards for n in s)
            if not no_key:
                lists["key.json"] = json.dumps(key)
            if reused:
                lists["reuse/reuse.json"] = json.dumps({"run": reused})
                lists = {k: v for k, v in lists.items() if not k.startswith("shard-")}
            arts = {"mutants-list": lists}
            common = {**tools, **{v: env[v] for v in ("ImageOS", "ImageVersion", "RUNNER_OS", "RUNNER_ARCH")},
                      "image_release": env["ImageVersion"].split(".")[0],
                      "verilator": "Verilator 5.054", "yosys": "Yosys 0.57", "inert": entries, "inert_present": []}
            if not reused and not no_baseline:
                bst = {**common, "role": "baseline", "baseline_seconds": 1200, "timeout": 3610}
                bst.update(baseline_over or {})
                bfiles = {"stamp.json": json.dumps(bst), "tool-ledger.tsv": baseline_ledger,
                          "baseline-build-peak.txt": "900\n", "baseline-test-peak.txt": "3000\n"}
                for d in baseline_drop or ():
                    bfiles.pop(d, None)
                arts["mutants-baseline"] = bfiles
            for k, s in enumerate(shards if not reused else ()):
                st = {**common, "role": "shard", "baseline_seconds": None, "timeout": 3610}
                st.update(stamp_over or {})
                files_ = {"mutants-shard.txt": "".join(n + "\n" for n in s), "stamp.json": json.dumps(st),
                          "tool-ledger.tsv": ledger}
                kinds = {"caught": [], "missed": [], "timeout": [], "unviable": []}
                for i, n in enumerate(s):
                    kinds[outcome if (k, i) == (0, 0) else "caught"].append(n)
                for kind, ns in kinds.items():
                    files_[f"mutants.out/{kind}.txt"] = "".join(n + "\n" for n in ns)
                if kills and k == 0:
                    files_["memory-kills.tsv"] = f"t\t1\t9000\t13000\t{s[0]}\tcargo test\n"
                for d in drop or ():
                    files_.pop(d, None)
                arts[f"mutants-out-{k}"] = files_
            return r, arts, tamper

        class FakeGitHub:
            def __init__(self, *runs_):
                self.list = [r for r, _, _ in runs_]
                self.arts = {r["id"]: (a, t) for r, a, t in runs_}
                self.downloads = []

            def runs(self, branch):
                return [r for r in self.list if r["head_branch"] == branch]

            def run(self, rid):
                return next(r for r in self.list if r["id"] == int(rid))

            def artifacts(self, rid):
                return [{"id": f"{rid}/{n}", "name": n, "expired": False} for n in self.arts[rid][0]]

            def download(self, artifact, dest, keep=None):
                rid, name = artifact["id"].split("/", 1)
                files_, tampered = self.arts[int(rid)]
                self.downloads.append(artifact["id"])
                buf = io.BytesIO()
                with zipfile.ZipFile(buf, "w") as z:
                    for p, t in files_[name].items():
                        z.writestr(p, t)
                digest = "sha256:" + hashlib.sha256(buf.getvalue()).hexdigest()
                if tampered:
                    digest = "sha256:" + "f" * 64
                extract(buf.getvalue(), digest, dest, keep)

        def decide(*runs_, current=names, scope_=scope):
            gh = FakeGitHub(*runs_)
            with tempfile.TemporaryDirectory() as w, contextlib.redirect_stdout(io.StringIO()) as log:
                res = find(gh, scope_, cur, current, Path(w))
                if res["decision"] == "reuse":
                    assert (Path(w) / "reuse/reuse.json").is_file()
                    assert (Path(w) / "reuse/shards/mutants-out-0/stamp.json").is_file()
                    assert (Path(w) / "reuse/baseline/stamp.json").is_file()
                    assert (Path(w) / "reuse/baseline/baseline-test-peak.txt").is_file()
            return res, log.getvalue(), gh

        res, log, _ = decide(run(101))
        assert res["decision"] == "reuse" and res["run"] == 101, (res, log)
        assert f"reused: run 101 (SHA {101:040d}), key {cur['key']}" in log, log
        assert "Tool ledger [mutants-baseline]: 61 test(s) asked for a tool; 61 lookup(s) found the tool (61 required), SKIP: 0" in log, log
        assert "Tool ledger [mutants-0]: 3 test(s) asked for a tool; 3 lookup(s) found the tool (3 required), SKIP: 0" in log, log
        # A shard on another build of the same weekly image, with the same toolchain.
        res, log, _ = decide(run(101, stamp_over={"ImageVersion": "20261111.127.1"}))
        assert res["decision"] == "reuse", (res, log)
        # (a) once more, with the exclusions re-applied: one mutant excluded since.
        res, log, _ = decide(run(101), current=names[:1])
        assert res["decision"] == "reuse" and res["excluded_now"] == [names[1]], (res, log)
        # (c) an earlier run with the same key that is not clean: the gate runs in full.
        for what, r in (
            ("missed", run(102, outcome="missed", conclusion="failure")),
            ("missed", run(102, outcome="missed")),
            ("timeout", run(102, outcome="timeout")),
            ("memory limit", run(102, kills=True)),
            ("not tested", run(102, drop=["mutants.out/caught.txt"])),
            ("SKIP: 1", run(102, ledger=ledger_few + "SKIP\tb\tt99\tsby\t0\n")),
            ("no tool ledger", run(102, drop=["tool-ledger.tsv"])),
            ("no stamp", run(102, drop=["stamp.json"])),
            # The baseline job (ADR-0128 §2): its artifact, ledger (--min-ran 60) and stamp.
            ("no mutants-baseline artifact", run(102, no_baseline=True)),
            ("the baseline: Tool ledger [mutants-baseline]: 62 test(s) asked for a tool; 61 lookup(s) found the tool (61 required), SKIP: 1",
             run(102, baseline_ledger=ledger_ok + "SKIP\tb\tt99\tsby\t0\n")),
            ("only 59 tool lookup(s)", run(102, baseline_ledger=ledger_ok[: ledger_ok.index("t59") - 7])),
            ("the baseline: no tool ledger", run(102, baseline_drop=["tool-ledger.tsv"])),
            ("the baseline: no stamp", run(102, baseline_drop=["stamp.json"])),
            ("a shard stamp", run(102, baseline_over={"role": "shard"})),
            ("the baseline: rustc", run(102, baseline_over={"rustc": "rustc 1.90.0"})),
            ("the baseline: the inert paths", run(102, baseline_over={"inert_present": ["docs/adr/"]})),
            ("different verilator versions", run(102, baseline_over={"verilator": "Verilator 5.050"})),
            ("rustc", run(102, stamp_over={"rustc": "rustc 1.90.0"})),
            ("image_release", run(102, stamp_over={"ImageVersion": "20261104.1.1", "image_release": "20261104"})),
            ("gxx", run(102, stamp_over={"gxx": "g++ 15.2.0"})),
            ("glibc", run(102, stamp_over={"glibc": "ldd 2.43"})),
            ("inert", run(102, stamp_over={"inert_present": ["docs/adr/"]})),
            ("digest", run(102, tamper=True)),
            ("concluded failure", run(102, conclusion="failure")),
        ):
            res, log, _ = decide(r, run(101))
            assert res["decision"] == "rerun" and what in res["reason"], (what, res, log)
        # A mutant of this run that was not tested then (an exclusion removed since).
        res, log, _ = decide(run(101, shards=((names[0],),)))
        assert res["decision"] == "rerun" and "not tested then" in res["reason"], (res, log)
        # (d) the key differs: never reused; an older run with this key is.
        other = json.loads(json.dumps(cur))
        other["parts"]["tree"]["book/src/p.md"] = "100644 blob " + "1" * 40
        other["key"] = "f" * 64
        res, log, _ = decide(run(103, key=other))
        assert res["decision"] == "rerun" and "no earlier run" in res["reason"], (res, log)
        assert "tree: 1 path(s) differ: book/src/p.md" in log, log
        res, log, _ = decide(run(103, key=other), run(101))
        assert res["decision"] == "reuse" and res["run"] == 101, (res, log)
        # Field by field, not only the hash.
        forged = json.loads(json.dumps(other))
        forged["key"] = cur["key"]
        res, log, _ = decide(run(103, key=forged))
        assert res["decision"] == "rerun", (res, log)
        # Never from another pull request, a fork, a hand run, a reused run, a run still going.
        for what, r in (
            ("pull request", run(104, pr=8)),
            ("pull request", run(104, pr=None)),
            ("head repository", run(104, head_repo="fork/r")),
            ("event", run(104, event="workflow_dispatch")),
            ("not completed", run(104, status="in_progress")),
            ("cancelled", run(104, conclusion="cancelled")),
            ("no reuse key", run(104, no_key=True)),
            ("reused", run(104, reused=101)),
        ):
            res, log, gh = decide(r)
            assert res["decision"] == "rerun" and what in log, (what, res, log)
            res, log, _ = decide(r, run(101))
            assert res["decision"] == "reuse" and res["run"] == 101, (what, res, log)
        # The current run itself is not an earlier run.
        res, log, _ = decide(run(900))
        assert res["decision"] == "rerun", (res, log)
        # A fork pull request, or a hand run of the gate: reuse is off.
        for sc in (dict(scope, head_repo="fork/r"), dict(scope, event="workflow_dispatch")):
            res, log, gh = decide(run(101), scope_=sc)
            assert res["decision"] == "rerun" and gh.downloads == [], (res, log)

        # #364 (ADR-0129): a retest after a test-only push.
        meta = {
            "workspace_root": str(repo),
            "workspace_members": ["a 0.1"],
            "packages": [{
                "id": "a 0.1", "name": "a", "manifest_path": str(repo / "crates/a/Cargo.toml"),
                "targets": [
                    {"name": "a", "kind": ["lib"], "src_path": str(repo / "crates/a/src/lib.rs")},
                    {"name": "t", "kind": ["test"], "src_path": str(repo / "crates/a/tests/t.rs")},
                    {"name": "u", "kind": ["test"], "src_path": str(repo / "crates/a/tests/u.rs")},
                ],
            }],
        }
        assert changed_binaries(["crates/a/tests/u.rs"], meta) == ({"a::u"}, set(), [])
        assert changed_binaries(["crates/a/tests/common/mod.rs", "crates/a/tests/data/x.volt"], meta) == (set(), {"a"}, [])
        assert changed_binaries(["tests/ui/x.volt", "crates/a/src/lib.rs", "crates/zz/tests/t.rs"], meta)[2] == [
            "tests/ui/x.volt", "crates/a/src/lib.rs", "crates/zz/tests/t.rs"]
        assert binary_changed("a", set(), {"a"}) and binary_changed("a::t", set(), {"a"})
        assert not binary_changed("ab::t", set(), {"a"}) and not binary_changed("a::t", {"a::u"}, set())
        rx = names_regex([names[1], 'x.rs:1:1: replace f -> Option<(A, B)> with Some(("x".into(), B::new()))'])
        assert re.fullmatch(rx, names[1]) and not re.fullmatch(rx, names[0]), rx
        assert re.fullmatch(rx, 'x.rs:1:1: replace f -> Option<(A, B)> with Some(("x".into(), B::new()))'), rx
        log_t = ("*** " + names[1] + "\n        PASS [   0.010s] (   1/9) a grammar::x\n"
                 "        FAIL [   0.013s] (   2/9) a::t catches_it\n"
                 "  Cancelling due to test failure: immediately terminating 1 tests still running\n"
                 "     SIGKILL [   0.020s] (   3/9) a::u killed_later\n")

        def with_logs(run_, logs):
            """Add outcomes.json and the mutants' logs to each shard of a fake run."""
            r_, arts_, tamper_ = run_
            for k_ in range(2):
                files_ = arts_.get(f"mutants-out-{k_}")
                if files_ is None:
                    continue
                outs = [{"scenario": "Baseline"}]
                for x in files_["mutants-shard.txt"].split("\n"):
                    if not x:
                        continue
                    lp = f"log/{abs(hash(x))}.log"
                    outs.append({"scenario": {"Mutant": {"name": x}}, "log_path": lp})
                    files_[f"mutants.out/{lp}"] = logs.get(x, "*** " + x + "\n")
                files_["mutants.out/outcomes.json"] = json.dumps({"outcomes": outs})
            return r_, arts_, tamper_

        # The base: run 201, names[0] missed, names[1] caught by a::t.
        base201 = lambda **kw: with_logs(run(201, outcome="missed", conclusion="failure", **kw), {names[1]: log_t})  # noqa: E731
        k_u = key_after({"crates/a/tests/u.rs": "#[test] fn more() {}\n"})
        k_t = key_after({"crates/a/tests/t.rs": "#[test] fn weaker() {}\n"})
        k_mod = key_after({"crates/a/tests/common/mod.rs": "pub fn h() {}\n"})

        def plan(*runs_, key=k_u, scope_=scope, run_id=None, current=names):
            gh = FakeGitHub(*runs_)
            with tempfile.TemporaryDirectory() as w, contextlib.redirect_stdout(io.StringIO()) as log:
                res = find_retest(gh, scope_, key, current, Path(w), meta, run_id)
                info = json.loads((Path(w) / "retest/retest.json").read_text(encoding="utf-8")) \
                    if res["decision"] == "retest" else None
                rx_ = (Path(w) / "retest/retest.re").read_text(encoding="utf-8").strip() if info else None
            return res, info, rx_, log.getvalue()

        res, info, rx_, log = plan(base201())
        assert res["decision"] == "retest" and res["run"] == 201, (res, log)
        assert info["retest"] == {names[0]: "missed then"}, info
        assert info["carried"] == {names[1]: {"outcome": "caught", "caught_by": ["a::t"]}}, info
        assert info["changed_paths"] == ["crates/a/tests/u.rs"] and info["changed_binaries"] == ["a::u"], info
        assert rx_ == names_regex([names[0]]), rx_
        assert "retest: run 201 (SHA " in log and "1 of 2 retested, 1 carried over" in log, log
        # The catching binary changed (a::u after nextest's "Cancelling" is not a catcher).
        res, info, _, log = plan(base201(), key=k_t)
        assert info["retest"][names[1]] == "caught then by a::t, changed since", info
        # A module of tests/ counts for every binary of its package.
        res, info, _, log = plan(base201(), key=k_mod)
        assert set(info["retest"]) == set(names) and info["changed_packages"] == ["a"], info
        # A caught mutant whose log names no failing binary is tested again.
        res, info, _, log = plan(with_logs(run(201, outcome="missed"), {}))
        assert info["retest"][names[1]] == "caught then; its log names no failing test binary", info
        # A mutant excluded then, not today, is tested.
        res, info, _, log = plan(with_logs(run(201, shards=((names[1],),)), {names[1]: log_t}))
        assert info["retest"] == {names[0]: "not tested then"}, info
        # Anything else runs in full and says why.
        for what, key_ in (
            ("not test files: README.md", key_after({"README.md": "x\n"})),
            ("mutants:", key_after({"crates/a/src/lib.rs": "pub fn f() -> u8 { 2 }\n"})),
            ("environment:", key_after({"crates/a/tests/u.rs": "x\n"}, env=dict(env, MUTANTS_MEMORY_LIMIT_MIB="8192"))),
        ):
            res, info, _, log = plan(base201(), key=key_)
            assert res["decision"] == "full" and what in res["reason"], (what, res, log)
        res, _, _, log = plan(base201(drop=["mutants.out/caught.txt"]))
        assert res["decision"] == "full" and "is not complete" in res["reason"], (res, log)
        res, _, _, log = plan(base201(no_baseline=True))
        assert res["decision"] == "full" and "mutants-baseline" in res["reason"], (res, log)
        # The newest full run decides: a reuse, a retest and a run still going are passed over.
        newer = [run(301, reused=101), with_logs(run(302), {}), run(303, status="in_progress")]
        newer[1][1]["mutants-list"]["retest/retest.json"] = json.dumps({"run": 9})
        for r_, _, _ in newer:
            r_["created_at"] = "2026-11-19T00:00:00Z"
        res, info, _, log = plan(base201(), *newer)
        assert res["decision"] == "retest" and res["run"] == 201, (res, log)
        assert "a reuse or a retest, not a full run" in log and "in_progress" in log, log
        # A newer full run that does not qualify is not passed over for an older one.
        bad = run(304, key=k_mod, conclusion="failure")
        bad[0]["created_at"] = "2026-11-19T00:00:00Z"
        res, _, _, log = plan(base201(), bad, key=key_after({"crates/a/src/lib.rs": "pub fn f() -> u8 { 3 }\n"}))
        assert res["decision"] == "full" and "base run 304" in res["reason"], (res, log)
        # Never from another pull request, a fork; a hand run only with retest_from.
        res, _, _, _ = plan(run(201, pr=8))
        assert res["decision"] == "full" and "no earlier full gate run" in res["reason"], res
        res, _, _, _ = plan(base201(), scope_=dict(scope, head_repo="fork/r"))
        assert res["decision"] == "full" and "fork" in res["reason"], res
        hand = dict(scope, event="workflow_dispatch")
        res, _, _, _ = plan(base201(), scope_=hand)
        assert res["decision"] == "full" and "retest_from" in res["reason"], res
        res, info, _, _ = plan(base201(), scope_=hand, run_id=201)
        assert res["decision"] == "retest" and info["retest"] == {names[0]: "missed then"}, res
        res, _, _, _ = plan(base201(), run_id=201)
        assert res["decision"] == "full" and "hand run only" in res["reason"], res
        # retest-check: R in the final list, the list on the gate's, the rest carried over.
        with tempfile.TemporaryDirectory() as w:
            lp = Path(w)
            (lp / "retest").mkdir()
            (lp / "gate-all.txt").write_text("".join(x + "\n" for x in names), encoding="utf-8")
            base_info = {"run": 201, "sha": "s", "retest": {names[0]: "missed then"},
                         "carried": {names[1]: {"outcome": "caught", "caught_by": ["a::t"]}}}
            for listed_, code_, want in (
                ([names[0]], 0, {names[1]: {"outcome": "caught", "caught_by": ["a::t"]}}),
                (names, 0, {}),
                ([], 1, None),
                ([names[0], "crates/a/src/lib.rs:9:9: other"], 1, None),
            ):
                (lp / "retest" / "retest.json").write_text(json.dumps(base_info), encoding="utf-8")
                (lp / "all.txt").write_text("".join(x + "\n" for x in listed_), encoding="utf-8")
                with contextlib.redirect_stdout(io.StringIO()):
                    assert retest_check(lp) == code_, (listed_, code_)
                if want is not None:
                    got = json.loads((lp / "retest" / "retest.json").read_text(encoding="utf-8"))
                    assert got["carried"] == want, got
                    assert set(got["retest"]) == set(listed_), got
    print("mutants_reuse self-test: ok")
    return 0


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    k = sub.add_parser("key", help="write the reuse key of this gate run")
    k.add_argument("--lists", type=Path, required=True)
    k.add_argument("--out", type=Path)
    f = sub.add_parser("find", help="an earlier passed run of this pull request with the same key")
    f.add_argument("--lists", type=Path, required=True)
    f.add_argument("--pr", type=int, required=True)
    f.add_argument("--head-repo", required=True)
    f.add_argument("--head-ref", required=True)
    f.add_argument("--github-output", type=Path)
    sub.add_parser("mask", help="delete the inert paths from the checkout")
    s = sub.add_parser("stamp", help="the baseline job's or a shard's tool versions, runner image and inert paths")
    s.add_argument("--out", type=Path, required=True)
    s.add_argument("--role", choices=("baseline", "shard"), required=True)
    s.add_argument("--baseline-seconds", type=int, help="the baseline's tests, seconds (role baseline)")
    s.add_argument("--timeout", type=int, required=True, help="the test timeout per mutant, seconds")
    rt = sub.add_parser("retest", help="#364: after a test-only push, the mutants to test again")
    rt.add_argument("--lists", type=Path, required=True)
    rt.add_argument("--pr", type=int, help="the pull request (a pull_request run)")
    rt.add_argument("--head-repo", default="")
    rt.add_argument("--head-ref", default="")
    rt.add_argument("--run", type=int, help="a hand gate run: the base run (input retest_from)")
    rt.add_argument("--github-output", type=Path)
    rc = sub.add_parser("retest-check", help="#364: the retest list against the gate's, the carried outcomes")
    rc.add_argument("--lists", type=Path, required=True)
    args = ap.parse_args()
    repo = Path.cwd()
    if args.cmd == "mask":
        return mask(repo)
    if args.cmd == "retest-check":
        return retest_check(args.lists)
    if args.cmd == "retest":
        key = json.loads((args.lists / "key.json").read_text(encoding="utf-8"))
        scope = {
            "repo": os.environ.get("GITHUB_REPOSITORY", ""),
            "pr": args.pr,
            "head_repo": args.head_repo,
            "head_ref": args.head_ref,
            "run_id": int(os.environ.get("GITHUB_RUN_ID", "0")),
            "event": os.environ.get("GITHUB_EVENT_NAME", ""),
        }
        res = find_retest(GhCli(scope["repo"]), scope, key, read_names(args.lists / "all.txt"), args.lists,
                          mutants_nextest.cargo_metadata(), args.run)
        if res["decision"] != "retest":
            print(f"::notice title=Mutation gate::no retest, the gate runs in full: {res['reason']}")
            return 0
        print(f"::notice title=Mutation gate::retest: run {res['run']} (SHA {res['sha']}):"
              f" {len(res['retest'])} to test again, {len(res['carried'])} to carry over")
        if args.github_output:
            with args.github_output.open("a", encoding="utf-8") as fh:
                fh.write(f"run={res['run']}\nsha={res['sha']}\n")
        return 0
    if args.cmd == "stamp":
        if (args.role == "baseline") != (args.baseline_seconds is not None):
            print("::error title=Mutation gate::--baseline-seconds goes with --role baseline, and only with it")
            return 1
        st = stamp(repo, dict(os.environ), run_text, args.timeout, args.baseline_seconds, args.role)
        args.out.write_text(json.dumps(st, indent=2) + "\n", encoding="utf-8")
        print(f"stamp: {st['rustc'].splitlines()[0]}; {st['cargo_mutants']}; {st['cargo_nextest'].splitlines()[0]};"
              f" {st['verilator']}; {st['yosys']}; {st['gxx']}; {st['glibc']}; {st['ImageOS']} {st['ImageVersion']};"
              f" inert paths present: {st['inert_present'] or 'none'}")
        if st["inert_present"]:
            print(f"::error title=Mutation gate::inert path(s) present while the {args.role} tested: {st['inert_present']}")
            return 1
        return 0
    if args.cmd == "key":
        names = read_names(args.lists / "unexcluded.txt")
        if not names:
            print(f"::error title=Mutation gate::{args.lists / 'unexcluded.txt'} is empty or missing: no key")
            return 1
        key = compute_key(repo, names, dict(os.environ), tool_versions(run_text))
        out = args.out or args.lists / "key.json"
        out.write_text(json.dumps(key, indent=1, sort_keys=True) + "\n", encoding="utf-8")
        h = key["hashes"]
        print(f"Reuse key {key['key']}: mutants {h['mutants'][:12]} ({len(key['parts']['mutants']['names'])} before"
              f" the exclusions), tree {h['tree'][:12]} ({len(key['parts']['tree'])} paths, inert paths and"
              f" {CONFIG} left out), environment {h['environment'][:12]}")
        return 0
    key = json.loads((args.lists / "key.json").read_text(encoding="utf-8"))
    current = read_names(args.lists / "all.txt")
    scope = {
        "repo": os.environ.get("GITHUB_REPOSITORY", ""),
        "pr": args.pr,
        "head_repo": args.head_repo,
        "head_ref": args.head_ref,
        "run_id": int(os.environ.get("GITHUB_RUN_ID", "0")),
        "event": os.environ.get("GITHUB_EVENT_NAME", ""),
    }
    res = find(GhCli(scope["repo"]), scope, key, current, args.lists)
    if res["decision"] != "reuse":
        print(f"::notice title=Mutation gate::not reused, the gate runs in full: {res['reason']}")
        return 0
    print(f"::notice title=Mutation gate::reused: run {res['run']} (SHA {res['sha']}), key {key['key']}")
    if args.github_output:
        with args.github_output.open("a", encoding="utf-8") as fh:
            fh.write(f"run={res['run']}\nsha={res['sha']}\nkey={key['key']}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
