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
   runner image (ImageOS, ImageVersion).

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
- every shard's tool ledger passes `--require-zero-skips --min-ran 60`
  (its `Tool ledger [mutants-k]: ... SKIP: 0` line is printed again);
- every shard's stamp (written after its baseline passed) names the same
  rustc, cargo-mutants, cargo-nextest and runner image as the key, and
  shows that the inert paths were absent while it tested;
- every mutant of this run's list (after today's exclude_re) was tested
  then and caught or unviable.

    mutants_reuse.py key --lists DIR [--out FILE]
    mutants_reuse.py find --lists DIR --pr N --head-repo OWNER/NAME --head-ref BRANCH [--github-output FILE]
    mutants_reuse.py mask
    mutants_reuse.py stamp --out FILE --baseline-seconds S --timeout T
    mutants_reuse.py --self-test

`key` writes DIR/key.json from DIR/unexcluded.txt and the checkout. `find`
looks for the earlier run and, when it reuses one, writes DIR/reuse/
(reuse.json, the earlier lists and the shards' outcomes) for the summary
job; GitHub Actions outputs `run`, `sha` and `key`. `mask` deletes the
inert paths from the checkout (each shard, before its baseline). `stamp`
writes a shard's tool versions, runner image and inert paths.
"""

import argparse
import contextlib
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
import urllib.parse
import zipfile
from pathlib import Path, PurePosixPath

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mutants_summary  # noqa: E402  scripts/ci, next to this script
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
)
RUNNER_VARS = ("ImageOS", "ImageVersion", "RUNNER_OS", "RUNNER_ARCH")
TOOLS = ("rustc", "cargo_mutants", "cargo_nextest")
# The shard's own `Tool ledger` step: tool_ledger.py report --require-zero-skips --min-ran 60.
MIN_RAN = 60
# How many earlier runs of the pull request are looked at, newest first.
MAX_RUNS = 50
# What a shard's artifact brings to a later run (no per-mutant logs).
SHARD_FILES = (
    "mutants-shard.txt",
    "memory-kills.tsv",
    "baseline-memory-kills.tsv",
    "baseline-build-peak.txt",
    "baseline-test-peak.txt",
    "stamp.json",
    "tool-ledger.tsv",
) + tuple(f"mutants.out/{k}.txt" for k in mutants_summary.OUTCOMES)


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
            "runner": {v: env.get(v, "") for v in RUNNER_VARS},
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


def tool_versions(run) -> dict:
    return {
        "rustc": run(["rustc", "-vV"]),
        "cargo_mutants": run(["cargo", "mutants", "--version"]),
        "cargo_nextest": run(["cargo", "nextest", "--version"]),
    }


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


def stamp(repo: Path, env: dict, run, baseline_seconds: int, timeout: int) -> dict:
    entries = parse_inert((repo / INERT_FILE).read_text(encoding="utf-8"))
    st = tool_versions(run)
    st["verilator"] = run(["verilator", "--version"])
    st["yosys"] = run(["yosys", "-V"])
    st.update({v: env.get(v, "") for v in RUNNER_VARS})
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


def check_outcomes(lists: Path, shards: Path, key: dict, current: list[str]) -> tuple[list[str], list[str], dict]:
    """(problems, log lines, facts) of an earlier run's shards."""
    problems, log = [], []
    n = len(list(lists.glob("shard-*.txt")))
    if n == 0:
        return ["it has no shard list"], log, {}
    env = key["parts"]["environment"]
    want = {**env["tools"], **env["runner"]}
    results, tools_seen = [], {}
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
                code, out = tool_ledger.report(tool_ledger.load(ledger), f"mutants-{k}", True, MIN_RAN)
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
        for field, value in want.items():
            if st.get(field) != value:
                problems.append(f"shard {k}: {field} {short(str(st.get(field)))}, the key has {short(str(value))}")
        if st.get("inert") != env["inert"] or st.get("inert_present"):
            problems.append(f"shard {k}: the inert paths were not masked: {st.get('inert_present')}")
        for t in ("verilator", "yosys"):
            tools_seen.setdefault(t, set()).add(st.get(t))
    for t, seen in tools_seen.items():
        if len(seen) != 1:
            problems.append(f"the shards ran different {t} versions: {sorted(map(str, seen))}")
    tot = {kind: sorted(x for r in results for x in r[kind]) for kind in mutants_summary.KINDS}
    for kind, word in (("missed", "missed"), ("timeout", "timeout"), ("memory", "memory limit")):
        if tot[kind]:
            problems.append(f"{len(tot[kind])} {word}: {listed(tot[kind], 3)}")
    tested = set(tot["caught"]) | set(tot["unviable"])
    new = sorted(set(current) - tested)
    if new:
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
        except ReuseError as err:
            problems.append(f"its shard outcomes could not be read: {err}")
            return {"decision": "rerun", "run": rid,
                    "reason": f"{head} has the same key but is not reusable: " + "; ".join(problems)}
        more, log, facts = check_outcomes(tmp / "lists", tmp / "shards", key, current)
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
        info = {"run": rid, "sha": sha, "key": key["key"], "created_at": r.get("created_at"),
                "url": r.get("html_url", ""), **facts}
        (out / "reuse.json").write_text(json.dumps(info, indent=2) + "\n", encoding="utf-8")
        c = facts["counts"]
        print(f"{head}: same key; {sum(c.values())} mutant(s) in {facts['shards']} shard(s): "
              f"caught={c['caught']} unviable={c['unviable']} missed=0 timeout=0 memory=0; "
              f"every shard completed, SKIP: 0, the same tools and runner image")
        if facts["excluded_now"]:
            print(f"excluded by today's exclude_re, tested then: {listed(facts['excluded_now'], 20)}")
        print(f"reused: run {rid} (SHA {sha}), key {key['key']}")
        return {"decision": "reuse", "run": rid, "sha": sha, "excluded_now": facts["excluded_now"]}
    return {"decision": "rerun", "reason": f"no earlier run of pull request #{scope['pr']} with key {key['key']}"}


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
    tools = {"rustc": "rustc 1.91.0", "cargo_mutants": "cargo-mutants 27.1.0", "cargo_nextest": "cargo-nextest 0.9.151"}
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
            ("ImageVersion", {"env": dict(env, ImageVersion="20261118.1.0")}),
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
        st = stamp(work, env, lambda args: {"rustc": tools["rustc"], "cargo": "x"}.get(args[0], f"{args[0]} 1"), 600, 1200)
        assert st["inert_present"] == [] and st["inert"] == entries, st
        assert stamp(repo, env, lambda args: "v", 1, 300)["inert_present"] == entries

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

        def run(rid, *, key=cur, shards=((names[0],), (names[1],)), outcome="caught", conclusion="success",
                status="completed", event="pull_request", pr=7, head_repo="o/r", branch="feat/x", reused=None,
                stamp_over=None, ledger=ledger_ok, kills=None, drop=None, tamper=False, no_key=False):
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
            for k, s in enumerate(shards if not reused else ()):
                st = {**tools, **{v: env[v] for v in ("ImageOS", "ImageVersion", "RUNNER_OS", "RUNNER_ARCH")},
                      "verilator": "Verilator 5.054", "yosys": "Yosys 0.57", "inert": entries, "inert_present": [],
                      "baseline_seconds": 600, "timeout": 1200}
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
            return res, log.getvalue(), gh

        res, log, _ = decide(run(101))
        assert res["decision"] == "reuse" and res["run"] == 101, (res, log)
        assert f"reused: run 101 (SHA {101:040d}), key {cur['key']}" in log, log
        assert "Tool ledger [mutants-0]: 61 test(s) asked for a tool; 61 lookup(s) found the tool (61 required), SKIP: 0" in log, log
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
            ("SKIP: 1", run(102, ledger=ledger_ok + "SKIP\tb\tt99\tsby\t0\n")),
            ("only 59 tool lookup(s)", run(102, ledger=ledger_ok[: ledger_ok.index("t59") - 7])),
            ("no tool ledger", run(102, drop=["tool-ledger.tsv"])),
            ("no stamp", run(102, drop=["stamp.json"])),
            ("rustc", run(102, stamp_over={"rustc": "rustc 1.90.0"})),
            ("ImageVersion", run(102, stamp_over={"ImageVersion": "20261104.1.1"})),
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
    s = sub.add_parser("stamp", help="a shard's tool versions, runner image and inert paths")
    s.add_argument("--out", type=Path, required=True)
    s.add_argument("--baseline-seconds", type=int, required=True)
    s.add_argument("--timeout", type=int, required=True)
    args = ap.parse_args()
    repo = Path.cwd()
    if args.cmd == "mask":
        return mask(repo)
    if args.cmd == "stamp":
        st = stamp(repo, dict(os.environ), run_text, args.baseline_seconds, args.timeout)
        args.out.write_text(json.dumps(st, indent=2) + "\n", encoding="utf-8")
        print(f"stamp: {st['rustc'].splitlines()[0]}; {st['cargo_mutants']}; {st['cargo_nextest'].splitlines()[0]};"
              f" {st['verilator']}; {st['yosys']}; {st['ImageOS']} {st['ImageVersion']};"
              f" inert paths present: {st['inert_present'] or 'none'}")
        if st["inert_present"]:
            print(f"::error title=Mutation gate::inert path(s) present while the shard tested: {st['inert_present']}")
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
