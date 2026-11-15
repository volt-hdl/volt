#!/usr/bin/env python3
"""Memory limit for a mutation run (ADR-0109 appendix "PR kapısı", #160).

A mutant can make the code under test allocate without bound: in #157's
runs `parse_block`'s `&&` -> `||` (stmt.rs:598:32) made the parser loop at
end of file, memory grew by ~250 MB/s and the GitHub runner was shut down
with the whole shard (no outcomes after that mutant, no summary line, no
tool ledger line). cargo-mutants 27.1.0 limits the time of a mutant
(--timeout, --build-timeout) but not its memory.

This script runs a command (cargo mutants, or the baseline) and watches the
memory of every process under it. When their resident memory together
exceeds the limit, it kills the largest one (never the command itself) with
SIGKILL: the test that ran it fails, cargo-mutants records the mutant as
caught and goes on with the next one, so the shard completes. Each kill is
one line in the log (tab-separated: time, pid, MiB of the process, MiB of
the tree, the mutant, the command line); the mutant is the newest log of
cargo-mutants (`mutants.out/log/*.log`, which opens with `*** <name>`).
The summary (mutants_summary.py) reports those mutants as their own class,
"memory limit", caught by ADR-0109 section 4 as a timeout is.

    mutants_watchdog.py run --limit-mib N --log FILE [--mutants-out DIR]
                            [--interval SECONDS] -- COMMAND [ARGS...]
    mutants_watchdog.py --self-test

The exit code is the command's (128 + n when a signal n ended it). SIGINT
and SIGTERM sent to this script (GNU timeout signals the process group)
reach the command too, and the script waits for it. Linux only: it reads
/proc.
"""

import argparse
import os
import signal
import subprocess
import sys
import time
from pathlib import Path


PAGE = os.sysconf("SC_PAGE_SIZE") if hasattr(os, "sysconf") else 4096
MIB = 1024 * 1024


def processes():
    """pid -> (ppid, resident bytes) for every process in /proc."""
    out = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            stat = Path("/proc", entry, "stat").read_text()
        except OSError:
            continue  # the process ended meanwhile
        # The command name (field 2) may hold spaces and parentheses: the
        # fields after it start at the last ')'.
        rest = stat[stat.rfind(")") + 2 :].split()
        out[int(entry)] = (int(rest[1]), int(rest[21]) * PAGE)
    return out


def descendants(root, procs):
    children = {}
    for pid, (ppid, _) in procs.items():
        children.setdefault(ppid, []).append(pid)
    found, todo = [], [root]
    while todo:
        for c in children.get(todo.pop(), []):
            found.append(c)
            todo.append(c)
    return found


def current_mutant(mutants_out):
    """The mutant cargo-mutants is testing: the `*** <name>` line of its newest log."""
    if mutants_out is None:
        return "(no mutant: outside cargo mutants)"
    try:
        logs = sorted(Path(mutants_out, "log").glob("*.log"), key=lambda p: p.stat().st_mtime)
        if logs:
            # The `*** <name>` line opens the log; 27.1.0 writes an empty
            # line before it.
            head = logs[-1].read_text(encoding="utf-8", errors="replace")[:4096].split("\n")
            for line in head:
                if line.startswith("*** "):
                    return line[4:].strip()
            return f"(no mutant name in {logs[-1].name})"
    except OSError:
        pass
    return "(no mutant yet: baseline)"


def cmdline(pid):
    try:
        raw = Path("/proc", str(pid), "cmdline").read_bytes()
    except OSError:
        return "?"
    text = raw.replace(b"\0", b" ").decode("utf-8", "replace")
    # One log line per kill: no tab or newline from the arguments.
    return " ".join(text.split())[:300]


def run(cmd, limit_mib, log, mutants_out=None, interval=0.2):
    child = subprocess.Popen(cmd)

    def forward(signum, _frame):
        if child.poll() is None:
            child.send_signal(signum)

    signal.signal(signal.SIGINT, forward)
    signal.signal(signal.SIGTERM, forward)
    limit = limit_mib * MIB
    kills = 0
    while child.poll() is None:
        procs = processes()
        tree = [p for p in descendants(child.pid, procs) if p in procs]
        total = sum(procs[p][1] for p in tree) + procs.get(child.pid, (0, 0))[1]
        if total > limit and tree:
            victim = max(tree, key=lambda p: procs[p][1])
            line = "\t".join(
                [
                    time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                    str(victim),
                    str(procs[victim][1] // MIB),
                    str(total // MIB),
                    current_mutant(mutants_out),
                    cmdline(victim),
                ]
            )
            try:
                os.kill(victim, signal.SIGKILL)
            except ProcessLookupError:
                continue
            kills += 1
            with open(log, "a", encoding="utf-8") as f:
                f.write(line + "\n")
            print(
                f"::warning title=Mutation memory limit::killed pid {victim} at {total // MIB} MiB "
                f"(limit {limit_mib} MiB): {line.split(chr(9))[4]}",
                flush=True,
            )
            time.sleep(interval)  # let the kernel free the memory before the next look
            continue
        time.sleep(interval)
    code = wait(child)
    print(f"watchdog: {kills} process(es) killed at the memory limit of {limit_mib} MiB", flush=True)
    return code


def wait(child):
    while True:
        try:
            code = child.wait()
            break
        except KeyboardInterrupt:
            continue
    return 128 - code if code < 0 else code


def self_test():
    import tempfile

    hog = (
        "import time\n"
        "chunks = []\n"
        "for _ in range(40):\n"
        "    chunks.append(bytearray(20 * 1024 * 1024))\n"
        "    time.sleep(0.05)\n"
        "time.sleep(1)\n"
    )
    # The direct child waits for the hog: it must survive the kill and see
    # its own child die by SIGKILL.
    parent = (
        "import subprocess, sys\n"
        f"r = subprocess.run([sys.executable, '-c', {hog!r}])\n"
        "sys.exit(3 if r.returncode == -9 else r.returncode)\n"
    )
    me = Path(__file__).resolve()
    with tempfile.TemporaryDirectory() as d:
        out = Path(d, "mutants.out")
        (out / "log").mkdir(parents=True)
        # cargo-mutants 27.1.0 starts a mutant's log with an empty line
        # (run 43731283723: the kill line had no mutant).
        (out / "log" / "a.log").write_text("\n*** crates/a/src/x.rs:3:5: replace == with != in f\n\nmore\n", encoding="utf-8")
        log = Path(d, "kills.tsv")

        def watch(cmd, limit):
            return subprocess.run(
                [sys.executable, str(me), "run", "--limit-mib", str(limit), "--log", str(log),
                 "--mutants-out", str(out), "--interval", "0.05", "--", *cmd]
            ).returncode

        # 1. A process that grows past the limit is killed; its parent goes on.
        start = time.monotonic()
        code = watch([sys.executable, "-c", parent], 300)
        assert code == 3, f"expected the hog to be killed (parent exit 3), got {code}"
        assert time.monotonic() - start < 20, "the hog was not stopped early"
        lines = log.read_text(encoding="utf-8").splitlines()
        assert len(lines) == 1, lines
        fields = lines[0].split("\t")
        assert fields[4] == "crates/a/src/x.rs:3:5: replace == with != in f", fields
        # The tree is over 300 MiB; the log rounds MiB down.
        assert int(fields[3]) >= 300 and "-c" in fields[5], fields
        # 2. Under the limit nothing is killed.
        log.unlink()
        small = "x = bytearray(30 * 1024 * 1024); import time; time.sleep(0.5)"
        assert watch([sys.executable, "-c", small], 300) == 0
        assert not log.exists() or log.read_text(encoding="utf-8") == "", "a process under the limit was killed"
        # 3. The exit code of the command passes through.
        assert watch([sys.executable, "-c", "import sys; sys.exit(7)"], 300) == 7
        # 4. SIGINT to the watchdog reaches the command, which ends with it.
        p = subprocess.Popen(
            [sys.executable, str(me), "run", "--limit-mib", "300", "--log", str(log), "--interval", "0.05",
             "--", sys.executable, "-c", "import time; time.sleep(30)"]
        )
        time.sleep(1.0)
        p.send_signal(signal.SIGINT)
        try:
            code = p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill()
            raise AssertionError("SIGINT did not stop the command")
        assert code == 128 + signal.SIGINT or code == 1, code
    print("mutants_watchdog self-test: ok")
    return 0


def main():
    if "--self-test" in sys.argv:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--limit-mib", type=int, required=True)
    r.add_argument("--log", type=Path, required=True)
    r.add_argument("--mutants-out", type=Path)
    r.add_argument("--interval", type=float, default=0.2)
    r.add_argument("command", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    cmd = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not cmd:
        ap.error("no command after --")
    return run(cmd, args.limit_mib, args.log, args.mutants_out, args.interval)


if __name__ == "__main__":
    sys.exit(main())
