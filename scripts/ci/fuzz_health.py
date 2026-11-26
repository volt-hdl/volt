#!/usr/bin/env python3
"""Did the fuzzer fuzz? A fail-closed check of a libFuzzer run (ADR-0143, #305).

libFuzzer starts by running every input of its corpus once (the replay); the line
`#N INITED ...` ends it. `-max_total_time` counts from the start of the
process, replay included, so a run whose replay eats the budget stops right
after INITED and still exits 0: the PR job of run 45873059789 replayed for
923 s and fuzzed for 0 s, green. This script reads a time-stamped libFuzzer
log and fails the job when

  - libFuzzer did not start (no `INFO: Seed:` line),
  - the replay did not end (no INITED line),
  - the run did not end (no `Done N runs in T second(s)` line: a crash, a
    timeout or a killed step), or
  - the real fuzz time, INITED to Done, is below half the budget.

It writes the run's figures to the job summary: corpus size, replay time,
real fuzz time, exec/s after INITED (the run counter's growth from INITED
to Done over that time), slow units. With --merge-log (the nightly job) it
also reads the corpus merge and decides whether the corpus may be saved
(`corpus_ok` in $GITHUB_OUTPUT): the merge ended in one attempt, and the
replay of the merged corpus reached the coverage (cov) and features (ft)
the merge measured on the restored corpus with the same binary, within
--tolerance.

    cargo +nightly fuzz run T DIR -- ARGS 2>&1 | python3 scripts/ci/fuzz_health.py stamp --out fuzz.log
    python3 scripts/ci/fuzz_health.py check --log fuzz.log --budget 1800 [--merge-log merge.log ...]
    python3 scripts/ci/fuzz_health.py check --log run.log --budget 60      # a `gh run view --log` file
    python3 scripts/ci/fuzz_health.py --self-test

`stamp` echoes its input and writes each line to --out after the Unix time
it arrived (`1764122296.681 #116192 INITED cov: ...`). `check` also reads
the logs GitHub keeps (`gh run view <id> --log`), whose lines carry the
runner's ISO time.
"""

from __future__ import annotations

import argparse
import io
import os
import re
import sys
import tempfile
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import BinaryIO, Callable, TextIO

sys.dont_write_bytecode = True  # no __pycache__ in the checkout

# The real fuzz time must be at least this share of the budget (ADR-0143).
MIN_FUZZ_SHARE = 0.5
# How far the merged corpus' replay may fall below the merge's own cov/ft
# before the corpus is not saved (ADR-0143): the merge and the replay run in
# two processes, and the hash seeds of std's HashMap differ between them.
DEFAULT_TOLERANCE = 0.01
ARTIFACT_KINDS = ("crash", "timeout", "oom", "leak", "slow-unit")

STAMP_RE = re.compile(r"^(\d+(?:\.\d+)?) (.*)$")
GITHUB_RE = re.compile(r"(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)(?:\.(\d+))?Z ?(.*)$")
SEED_RE = re.compile(r"^INFO: Seed: \d+")
CORPUS_RE = re.compile(r"^INFO: seed corpus: files: (\d+) min: \d+b max: \d+b total: (\d+)b")
STATUS_RE = re.compile(r"^#(\d+)\s+(INITED|NEW|REDUCE|pulse|DONE|RELOAD)\b(.*)$")
COV_RE = re.compile(r"\bcov: (\d+)")
FT_RE = re.compile(r"\bft: (\d+)")
CORP_RE = re.compile(r"\bcorp: (\d+)/")
DONE_RE = re.compile(r"^Done (\d+) runs in (\d+) second\(s\)")
SLOW_RE = re.compile(r"^Slowest unit: (\d+) s:")
TIMEOUT_RE = re.compile(r"ERROR: libFuzzer: timeout after (\d+) seconds")
MERGE_START_RE = re.compile(r"^MERGE-OUTER: (\d+) files, (\d+) in the initial corpus")
MERGE_ATTEMPT_RE = re.compile(r"^MERGE-OUTER: attempt (\d+)")
MERGE_END_RE = re.compile(
    r"^MERGE-OUTER: (\d+) new files with (\d+) new features added; (\d+) new coverage edges"
)


def parse_lines(text: str) -> list[tuple[float, str]]:
    """(Unix time, text) of every time-stamped line; other lines are dropped."""
    out = []
    for raw in text.splitlines():
        m = STAMP_RE.match(raw)
        if m:
            out.append((float(m[1]), m[2]))
            continue
        m = GITHUB_RE.search(raw)
        if m:
            base = datetime.strptime(m[1], "%Y-%m-%dT%H:%M:%S").replace(tzinfo=timezone.utc)
            frac = float("0." + m[2]) if m[2] else 0.0
            out.append((base.timestamp() + frac, m[3]))
    return out


@dataclass
class Status:
    t: float
    runs: int
    cov: int | None
    ft: int | None
    corp: int | None


def status(t: float, runs: str, rest: str) -> Status:
    def num(rx: re.Pattern[str]) -> int | None:
        m = rx.search(rest)
        return int(m[1]) if m else None

    return Status(t, int(runs), num(COV_RE), num(FT_RE), num(CORP_RE))


@dataclass
class FuzzLog:
    start: float | None = None
    seed_files: int | None = None
    seed_bytes: int | None = None
    inited: Status | None = None
    last: Status | None = None  # the last status line (cov/ft at the end)
    done_t: float | None = None
    done_runs: int | None = None
    slow: list[int] = field(default_factory=list)
    timeouts: int = 0

    @property
    def replay(self) -> float | None:
        if self.start is None or self.inited is None:
            return None
        return self.inited.t - self.start

    @property
    def fuzz(self) -> float | None:
        if self.inited is None or self.done_t is None:
            return None
        return self.done_t - self.inited.t

    @property
    def execs_per_s(self) -> float | None:
        # Below one second the rate says nothing (INITED and Done in the same
        # second: the run did not fuzz).
        if self.fuzz is None or self.done_runs is None or self.inited is None or self.fuzz < 1:
            return None
        return (self.done_runs - self.inited.runs) / self.fuzz


def read_fuzz(lines: list[tuple[float, str]]) -> FuzzLog:
    r = FuzzLog()
    for t, text in lines:
        if r.start is None and SEED_RE.match(text):
            r.start = t
        elif m := CORPUS_RE.match(text):
            r.seed_files, r.seed_bytes = int(m[1]), int(m[2])
        elif m := STATUS_RE.match(text):
            s = status(t, m[1], m[3])
            r.last = s
            if m[2] == "INITED" and r.inited is None:
                r.inited = s
        elif m := DONE_RE.match(text):
            r.done_t, r.done_runs = t, int(m[1])
        elif m := SLOW_RE.match(text):
            r.slow.append(int(m[1]))
        if TIMEOUT_RE.search(text):
            r.timeouts += 1
    return r


@dataclass
class MergeLog:
    start: float | None = None
    end: float | None = None
    inputs: int | None = None
    attempts: int = 0
    files: int | None = None
    ft: int | None = None
    cov: int | None = None

    @property
    def seconds(self) -> float | None:
        if self.start is None or self.end is None:
            return None
        return self.end - self.start


def read_merge(lines: list[tuple[float, str]]) -> MergeLog:
    r = MergeLog()
    for t, text in lines:
        if r.start is None and (SEED_RE.match(text) or text.startswith("MERGE-OUTER:")):
            r.start = t
        if m := MERGE_START_RE.match(text):
            r.inputs = int(m[1])
        elif m := MERGE_ATTEMPT_RE.match(text):
            r.attempts = max(r.attempts, int(m[1]))
        elif m := MERGE_END_RE.match(text):
            r.end, r.files, r.ft, r.cov = t, int(m[1]), int(m[2]), int(m[3])
    return r


@dataclass
class DirStats:
    files: int
    bytes: int
    over: int  # files longer than --max-len


def dir_stats(path: Path, max_len: int) -> DirStats:
    files = size = over = 0
    for p in path.rglob("*"):
        if p.is_file():
            n = p.stat().st_size
            files, size, over = files + 1, size + n, over + (n > max_len)
    return DirStats(files, size, over)


def artifact_counts(path: Path | None) -> dict[str, int]:
    counts = dict.fromkeys(ARTIFACT_KINDS, 0)
    if path is not None and path.is_dir():
        for p in path.iterdir():
            for kind in ARTIFACT_KINDS:
                if p.name.startswith(kind + "-"):
                    counts[kind] += 1
    return counts


@dataclass
class Verdict:
    failures: list[str]
    corpus_ok: bool | None  # None: no merge given (the PR job saves nothing)
    rows: list[tuple[str, str]]


def mb(n: int | None) -> str:
    return "?" if n is None else f"{n / 1e6:.1f} MB"


def secs(x: float | None) -> str:
    return "—" if x is None else f"{x:.0f} s"


def judge(
    fuzz: FuzzLog | None,
    budget: int,
    merge: MergeLog | None = None,
    merge_expected: bool = False,
    restored: tuple[int, int, int] | None = None,
    corpus: DirStats | None = None,
    artifacts: dict[str, int] | None = None,
    tolerance: float = DEFAULT_TOLERANCE,
) -> Verdict:
    """Fail closed: every missing piece is a failure, never a pass."""
    failures: list[str] = []
    corpus_problems: list[str] = []
    rows: list[tuple[str, str]] = []

    if restored is not None:
        files, size, over = restored
        rows.append(("Restored corpus", f"{files} files, {mb(size)}; {over} longer than -max_len, cut to it"))

    if merge_expected:
        if merge is None:
            failures.append("merge: no log (the merge step did not run)")
            corpus_problems.append("no merge")
        else:
            rows.append(
                (
                    "Merge (-merge=1)",
                    f"{merge.inputs if merge.inputs is not None else '?'} → "
                    f"{merge.files if merge.files is not None else '?'} files in {secs(merge.seconds)}; "
                    f"cov {merge.cov}, ft {merge.ft}",
                )
            )
            if merge.end is None:
                failures.append("merge: no 'MERGE-OUTER: N new files' line (the merge did not end)")
                corpus_problems.append("merge did not end")
            if merge.attempts > 1:
                failures.append(
                    f"merge: {merge.attempts} attempts (an input crashed, timed out or ran out of memory; "
                    "see the crash-/timeout-/oom- files in the artifact)"
                )
                corpus_problems.append("merge needed more than one attempt")

    if fuzz is None:
        failures.append("fuzz: no log (the fuzz step did not run)")
        corpus_problems.append("no fuzz log")
    else:
        if fuzz.seed_files is not None:
            rows.append(("Corpus at start (seeds included)", f"{fuzz.seed_files} files, {mb(fuzz.seed_bytes)}"))
        rows.append(("Replay (start → INITED)", secs(fuzz.replay)))
        share = "" if fuzz.fuzz is None else f" ({100 * fuzz.fuzz / budget:.0f} % of the budget)"
        rows.append(("Real fuzz (INITED → Done)", f"{secs(fuzz.fuzz)} of {budget} s{share}"))
        eps = fuzz.execs_per_s
        rows.append(("exec/s after INITED", "—" if eps is None else f"{eps:.0f}"))
        slow = f"{len(fuzz.slow)}" + (f", slowest {max(fuzz.slow)} s" if fuzz.slow else "")
        rows.append(("Slow units (Slowest unit lines)", slow))
        if fuzz.inited is not None:
            rows.append(
                ("INITED", f"cov {fuzz.inited.cov}, ft {fuzz.inited.ft}, corp {fuzz.inited.corp}, run {fuzz.inited.runs}")
            )
        if fuzz.last is not None and fuzz.done_t is not None:
            rows.append(("At the end", f"cov {fuzz.last.cov}, ft {fuzz.last.ft}, corp {fuzz.last.corp}, runs {fuzz.done_runs}"))

        if fuzz.start is None:
            failures.append("fuzz: no 'INFO: Seed:' line (libFuzzer did not start)")
        if fuzz.inited is None:
            failures.append("fuzz: no INITED line (the replay of the corpus did not end)")
            corpus_problems.append("no INITED")
        if fuzz.done_t is None:
            why = f"; {fuzz.timeouts} timeout(s)" if fuzz.timeouts else ""
            failures.append(f"fuzz: no 'Done N runs' line (crash, timeout or killed step{why})")
        if fuzz.fuzz is not None and fuzz.fuzz < MIN_FUZZ_SHARE * budget:
            failures.append(
                f"fuzz: real fuzz time {fuzz.fuzz:.0f} s is below half the budget ({budget} s); "
                f"the replay took {secs(fuzz.replay)}"
            )

        if merge is not None and merge.end is not None and fuzz.inited is not None:
            for name, before, after in (("cov", merge.cov, fuzz.inited.cov), ("ft", merge.ft, fuzz.inited.ft)):
                if before is None or after is None:
                    corpus_problems.append(f"{name} unknown")
                elif after < before * (1 - tolerance):
                    corpus_problems.append(
                        f"{name} fell from {before} (merge of the restored corpus) to {after} (replay of the merged corpus)"
                    )
            rows.append(
                ("Coverage kept", f"cov {merge.cov} → {fuzz.inited.cov}, ft {merge.ft} → {fuzz.inited.ft} (tolerance {tolerance:.1%})")
            )

    if corpus is not None:
        rows.append(("Corpus saved", f"{corpus.files} files, {mb(corpus.bytes)}; {corpus.over} longer than -max_len"))
        if merge is not None and merge.files is not None and corpus.files < merge.files:
            corpus_problems.append(f"corpus holds {corpus.files} files, fewer than the {merge.files} the merge wrote")
        if corpus.over:
            corpus_problems.append(f"{corpus.over} corpus files longer than -max_len")

    if artifacts is not None:
        rows.append(("Artifacts", ", ".join(f"{k} {v}" for k, v in artifacts.items())))

    corpus_ok = None
    if merge_expected:
        corpus_ok = not corpus_problems
        if corpus_problems:
            failures.append("corpus not saved: " + "; ".join(corpus_problems))
    rows.append(("Result", "pass" if not failures else "FAIL"))
    return Verdict(failures, corpus_ok, rows)


def markdown(title: str, v: Verdict) -> str:
    out = [f"### {title}", "", "| | |", "|---|---|"]
    out += [f"| {k} | {val} |" for k, val in v.rows]
    if v.failures:
        out += ["", "**Why it is red** (ADR-0143):", ""]
        out += [f"- {f}" for f in v.failures]
        out += [
            "",
            "What to do: open the step's log; for a crash or timeout, download the artifact and run "
            "`cargo +nightly fuzz run parse_never_panics <file>` (Linux or WSL, nightly).",
        ]
    return "\n".join(out) + "\n"


def stamp(inp: BinaryIO, log: TextIO, echo: BinaryIO, clock: Callable[[], float] = time.time) -> int:
    """Echo every line and write it to `log` after the time it arrived."""
    n = 0
    for raw in iter(inp.readline, b""):
        echo.write(raw)
        echo.flush()
        line = raw.rstrip(b"\r\n").decode("utf-8", "replace")
        log.write(f"{clock():.3f} {line}\n")
        log.flush()
        n += 1
    return n


def read_log(path: Path | None) -> list[tuple[float, str]] | None:
    if path is None or not path.is_file():
        return None
    return parse_lines(path.read_text(encoding="utf-8", errors="replace"))


def check(a: argparse.Namespace) -> int:
    fuzz_lines = read_log(a.log)
    fuzz = read_fuzz(fuzz_lines) if fuzz_lines is not None else None
    merge = None
    merge_expected = a.merge_log is not None
    if merge_expected:
        merge_lines = read_log(a.merge_log)
        if merge_lines is not None:
            merge = read_merge(merge_lines)
        elif a.restored_files == 0:
            merge_expected = False  # nothing restored, nothing to merge: a fresh corpus
    restored = None
    if a.restored_files is not None:
        restored = (a.restored_files, a.restored_bytes or 0, a.restored_over or 0)
    corpus = dir_stats(a.corpus, a.max_len) if a.corpus is not None and a.corpus.is_dir() else None
    artifacts = artifact_counts(a.artifacts) if a.artifacts is not None else None
    v = judge(fuzz, a.budget, merge, merge_expected, restored, corpus, artifacts, a.tolerance)
    if a.merge_log is not None and not merge_expected:
        v.corpus_ok = fuzz is not None and fuzz.inited is not None
    text = markdown(a.title, v)
    print(text)
    summary = a.summary or os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(text + "\n")
    output = os.environ.get("GITHUB_OUTPUT")
    if output and v.corpus_ok is not None:
        with open(output, "a", encoding="utf-8") as f:
            f.write(f"corpus_ok={'true' if v.corpus_ok else 'false'}\n")
    for f in v.failures:
        print(f"::error title=Fuzz health::{f}")
    return 1 if v.failures else 0


# ── self-test ────────────────────────────────────────────────────────────────

# Lines as libFuzzer prints them (the ones of run 45739110046, with the run
# counters and figures of that night).
START = ["INFO: Running with entropic power schedule (0xFF, 100).", "INFO: Seed: 252858782"]
SEEDS = "INFO: seed corpus: files: 116191 min: 1b max: 60143b total: 192309714b rss: 152Mb"
INITED = "#116192\tINITED cov: 22655 ft: 134473 corp: 14158/16Mb exec/s: 75 rss: 1443Mb"
NEW = "#116548\tNEW    cov: 22655 ft: 134475 corp: 14159/16Mb lim: 4096 exec/s: 76 rss: 1443Mb L: 3647/4096 MS: 2 InsertByte-InsertByte-"
DONE = "Done 127994 runs in 1900 second(s)"
SLOW = ["Slowest unit: 25 s:", "artifact_prefix='fuzz/artifacts/parse_never_panics/'; Test unit written to slow-unit-1"]


def stamped(*pairs: tuple[float, str]) -> list[tuple[float, str]]:
    return parse_lines("\n".join(f"{t:.3f} {s}" for t, s in pairs))


def merge_log(inputs: int, files: int, ft: int, cov: int, attempts: int = 1, end: bool = True) -> MergeLog:
    pairs = [(0.0, "INFO: Seed: 1"), (0.1, f"MERGE-OUTER: {inputs} files, 0 in the initial corpus, 0 processed earlier")]
    pairs += [(1.0 + k, f"MERGE-OUTER: attempt {k}") for k in range(1, attempts + 1)]
    if end:
        pairs.append((912.0, f"MERGE-OUTER: {files} new files with {ft} new features added; {cov} new coverage edges"))
    return read_merge(stamped(*pairs))


def self_test() -> int:
    failures: list[str] = []

    def expect(name: str, cond: bool, detail: object = "") -> None:
        if not cond:
            failures.append(f"{name}: {detail}")

    def red(name: str, v: Verdict, needle: str) -> None:
        expect(name, any(needle in f for f in v.failures), f"want a failure with {needle!r}, got {v.failures}")

    # 1. The night of 45739110046: 1530 s of replay, 370 s of fuzzing out
    #    of 1800 s. Red: below half the budget.
    night = read_fuzz(stamped((0, START[0]), (0, START[1]), (0.5, SEEDS), (367, SLOW[0]), (367, SLOW[1]),
                              (1529.7, INITED), (1530.3, NEW), (1900.2, DONE)))
    expect("night: replay", round(night.replay or 0) == 1530, night.replay)
    expect("night: fuzz", round(night.fuzz or 0) == 370, night.fuzz)
    expect("night: exec/s", round(night.execs_per_s or 0) == 32, night.execs_per_s)  # 11802 runs / 370.5 s
    expect("night: slow units", night.slow == [25], night.slow)
    expect("night: seed corpus", (night.seed_files, night.seed_bytes) == (116191, 192309714))
    expect("night: end cov/ft", night.last is not None and night.last.ft == 134475)
    red("night", judge(night, 1800), "below half the budget")

    # 2. The PR job of 45873059789, read from GitHub's own log format:
    #    923 s of replay, INITED and DONE in the same second. Red.
    gh = "\n".join(
        [
            "Fuzz (60 s)\tparse_never_panics (60 s)\t2026-11-26T18:06:28.9490178Z INFO: Seed: 2828469600",
            "Fuzz (60 s)\tparse_never_panics (60 s)\t2026-11-26T18:06:29.4391551Z INFO: seed corpus: files: 116290 min: 1b max: 60143b total: 192450490b rss: 152Mb",
            "Fuzz (60 s)\tparse_never_panics (60 s)\t2026-11-26T18:21:52.7577659Z #116291\tINITED cov: 22646 ft: 134424 corp: 14163/16355Kb exec/s: 125 rss: 1273Mb",
            "Fuzz (60 s)\tparse_never_panics (60 s)\t2026-11-26T18:21:52.7578406Z #116291\tDONE   cov: 22646 ft: 134424 corp: 14163/16355Kb lim: 4096 exec/s: 125 rss: 1273Mb",
            "Fuzz (60 s)\tparse_never_panics (60 s)\t2026-11-26T18:21:52.7578983Z Done 116291 runs in 923 second(s)",
        ]
    )
    pr = read_fuzz(parse_lines(gh))
    expect("PR: replay", round(pr.replay or 0) == 924, pr.replay)
    expect("PR: fuzz", (pr.fuzz or 1) < 0.01, pr.fuzz)
    expect("PR: no exec/s without fuzz time", pr.execs_per_s is None, pr.execs_per_s)
    red("PR", judge(pr, 60), "below half the budget")

    # 3. A healthy night: 200 s of replay, 1610 s of fuzzing. Green, and the
    #    figures land in the summary.
    good_pairs = [(0, START[1]), (1, SEEDS), (200, INITED), (201, NEW), (1810, DONE)]
    good = read_fuzz(stamped(*good_pairs))
    v = judge(good, 1800)
    expect("good: no failure", v.failures == [], v.failures)
    md = markdown("t", v)
    for needle in ("| Replay (start → INITED) | 200 s |", "| Real fuzz (INITED → Done) | 1610 s of 1800 s (89 % of the budget) |",
                   "| exec/s after INITED | 7 |", "| Result | pass |"):
        expect(f"good: summary has {needle!r}", needle in md, md)

    # 4. Exactly half the budget passes; one second less fails.
    half = read_fuzz(stamped((0, START[1]), (100, INITED), (1000, DONE)))
    expect("half: passes", judge(half, 1800).failures == [], judge(half, 1800).failures)
    red("half - 1 s", judge(read_fuzz(stamped((0, START[1]), (101, INITED), (1000, DONE))), 1800), "below half")

    # 5. Broken logs, each red for its own reason.
    no_inited = read_fuzz(stamped((0, START[1]), (1, SEEDS), (400, "#65536\tpulse  cov: 22107 ft: 107992 corp: 8945/3792Kb exec/s: 199 rss: 1038Mb")))
    red("log cut during the replay", judge(no_inited, 60), "no INITED line")
    red("log cut during the replay: no Done either", judge(no_inited, 60), "no 'Done N runs' line")
    cut = read_fuzz(stamped((0, START[1]), (5, INITED), (6, NEW)))
    red("log cut after INITED", judge(cut, 60), "no 'Done N runs' line")
    timeout = read_fuzz(stamped((0, START[1]), (5, INITED), (16, "==2134== ERROR: libFuzzer: timeout after 10 seconds")))
    red("timeout", judge(timeout, 60), "1 timeout(s)")
    red("empty log", judge(read_fuzz([]), 60), "libFuzzer did not start")
    red("no log", judge(None, 60), "the fuzz step did not run")
    red("untimed lines are not evidence", judge(read_fuzz(parse_lines(f"{START[1]}\n{INITED}\n{DONE}")), 60), "no INITED line")

    # 6. The nightly merge: 115692 restored files kept to 14158; the merged
    #    corpus' replay reaches the same cov/ft. corpus_ok, green.
    m = merge_log(115692, 14158, 134473, 22655)
    expect("merge: figures", (m.inputs, m.files, m.ft, m.cov, round(m.seconds or 0)) == (115692, 14158, 134473, 22655, 912), m)
    corpus = DirStats(14170, 16_000_000, 0)
    v = judge(good, 1800, m, True, (115692, 191_000_000, 2255), corpus, dict.fromkeys(ARTIFACT_KINDS, 0))
    expect("merge: green", v.failures == [] and v.corpus_ok is True, v.failures)
    expect("merge: summary", "| Merge (-merge=1) | 115692 → 14158 files in 912 s; cov 22655, ft 134473 |" in markdown("t", v), markdown("t", v))

    # 7. Broken merges: not saved, and red.
    for name, broken, needle in (
        ("merge without its last line", merge_log(115692, 0, 0, 0, end=False), "did not end"),
        ("merge in two attempts", merge_log(115692, 14158, 134473, 22655, attempts=2), "2 attempts"),
        ("merge lost coverage", merge_log(115692, 14158, 140000, 22655), "ft fell from 140000"),
    ):
        v = judge(good, 1800, broken, True, None, corpus)
        expect(f"{name}: not saved", v.corpus_ok is False, v.corpus_ok)
        red(name, v, needle)
    v = judge(good, 1800, None, True)
    expect("merge log missing: not saved", v.corpus_ok is False, v.corpus_ok)
    red("merge log missing", v, "the merge step did not run")
    v = judge(good, 1800, m, True, None, DirStats(100, 1, 0))
    red("corpus smaller than the merge", v, "fewer than the 14158")
    v = judge(good, 1800, m, True, None, DirStats(14170, 1, 3))
    red("corpus file over -max_len", v, "3 corpus files longer than -max_len")
    # Within the tolerance: 1 % below the merge is kept.
    m2 = merge_log(115692, 14158, 135800, 22655)  # 134473 >= 135800 * 0.99
    expect("tolerance: kept", judge(good, 1800, m2, True, None, corpus).corpus_ok is True)
    # A crash during fuzzing (no Done) still saves a sound corpus: the next
    # night keeps the coverage reached, the run is red.
    v = judge(cut, 60, m, True, None, corpus)
    expect("crash: corpus still saved", v.corpus_ok is True and v.failures, (v.corpus_ok, v.failures))

    # 8. stamp: echoes bytes unchanged, stamps every line, survives bad UTF-8.
    src = io.BytesIO(b"INFO: Seed: 1\n#5\tINITED cov: 1 ft: 2 corp: 3/1b\n\xff\xfe\nDone 9 runs in 1 second(s)")
    echo, log = io.BytesIO(), io.StringIO()
    clock = iter([10.0, 12.0, 13.0, 75.5]).__next__
    n = stamp(src, log, echo, clock)
    expect("stamp: echo", echo.getvalue() == src.getvalue(), echo.getvalue())
    expect("stamp: line count", n == 4, n)
    back = read_fuzz(parse_lines(log.getvalue()))
    expect("stamp: round trip", (back.replay, back.fuzz, back.done_runs) == (2.0, 63.5, 9), (back.replay, back.fuzz, back.done_runs))

    # 9. The directory figures and the artifact counts, on disk.
    with tempfile.TemporaryDirectory() as d:
        root = Path(d)
        (root / "a").write_bytes(b"x" * 4096)
        (root / "b").write_bytes(b"x" * 4097)
        expect("dir_stats", dir_stats(root, 4096) == DirStats(2, 8193, 1), dir_stats(root, 4096))
        for name in ("slow-unit-1", "slow-unit-2", "timeout-3", "crash-4", "other"):
            (root / name).write_bytes(b"")
        counts = artifact_counts(root)
        expect("artifacts", counts == {"crash": 1, "timeout": 1, "oom": 0, "leak": 0, "slow-unit": 2}, counts)
        expect("artifacts: missing dir", artifact_counts(root / "none") == dict.fromkeys(ARTIFACT_KINDS, 0))

    if failures:
        for f in failures:
            print(f"fuzz_health self-test FAIL {f}")
        return 1
    print(
        "fuzz_health self-test: ok (night 45739110046 and PR 45873059789 red; healthy run green; "
        "half the budget; log cut before INITED and before Done, timeout, empty log red; "
        "merge kept, broken merges not saved; stamp round trip)"
    )
    return 0


def main() -> int:
    # The summary has arrows; a Windows console's code page lacks them.
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # type: ignore[union-attr]
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("stamp", help="echo stdin and write it, time-stamped, to --out")
    s.add_argument("--out", type=Path, required=True)
    c = sub.add_parser("check", help="judge a run and write its summary")
    c.add_argument("--log", type=Path, required=True, help="time-stamped libFuzzer log of the fuzz run")
    c.add_argument("--budget", type=int, required=True, help="-max_total_time in seconds")
    c.add_argument("--merge-log", type=Path, help="time-stamped log of the corpus merge (nightly)")
    c.add_argument("--restored-files", type=int)
    c.add_argument("--restored-bytes", type=int)
    c.add_argument("--restored-over", type=int, help="restored files longer than --max-len")
    c.add_argument("--corpus", type=Path, help="the corpus directory after the run")
    c.add_argument("--artifacts", type=Path, help="libFuzzer's artifact directory")
    c.add_argument("--max-len", type=int, default=4096)
    c.add_argument("--tolerance", type=float, default=DEFAULT_TOLERANCE)
    c.add_argument("--title", default="Fuzz")
    c.add_argument("--summary", help="markdown file to append to (default: $GITHUB_STEP_SUMMARY)")
    a = ap.parse_args()
    if a.cmd == "stamp":
        with open(a.out, "w", encoding="utf-8") as log:
            stamp(sys.stdin.buffer, log, sys.stdout.buffer)
        return 0
    if a.budget <= 0:
        ap.error(f"--budget must be positive: {a.budget}")
    return check(a)


if __name__ == "__main__":
    sys.exit(main())
