#!/usr/bin/env python3
"""Print what ccache did for the Verilator builds of a CI job.

Verilator's generated makefile (verilated.mk) compiles every C++ file as
`$(OBJCACHE) $(CXX) ...`; the CI action .github/actions/ccache sets
OBJCACHE=ccache, zeroes the statistics before the job's tests and calls
this script after them with the output of `ccache --print-stats`. It
prints one line with the hits and the misses (and every other non-zero
counter, such as an uncacheable call or an error), puts it in a notice
and in the job summary.

With --require-use it fails when ccache saw no compile at all: the job
ran its tests but no Verilator build went through ccache (OBJCACHE did
not reach make, or no Verilator build ran), so the job would have been
silently slow.

    ccache_report.py STATS --job NAME [--require-use] [--quiet]
    ccache_report.py --self-test
"""

import argparse
import contextlib
import io
import os
import sys
import tempfile
from pathlib import Path

HIT_KEYS = ("direct_cache_hit", "preprocessed_cache_hit")
MISS_KEY = "cache_miss"
# Counters that are not a call outcome: sizes, times, and the split of the
# hits and misses (direct_cache_miss and preprocessed_cache_miss refine
# cache_miss, they are not more calls).
NOT_CALLS = {
    "cache_size_kibibyte",
    "max_cache_size_kibibyte",
    "files_in_cache",
    "cleanups_performed",
    "direct_cache_miss",
    "preprocessed_cache_miss",
    "local_storage_hit",
    "local_storage_miss",
    "local_storage_read_hit",
    "local_storage_read_miss",
    "local_storage_write",
    "remote_storage_hit",
    "remote_storage_miss",
    "remote_storage_read_hit",
    "remote_storage_read_miss",
    "remote_storage_write",
    "remote_storage_error",
    "remote_storage_timeout",
    "recache",
}


class StatsError(Exception):
    pass


def parse(text: str) -> dict:
    """The `ccache --print-stats` lines: `name<TAB>integer`."""
    stats = {}
    for n, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) != 2 or not parts[1].strip().lstrip("-").isdigit():
            raise StatsError(f"line {n} is not 'name<TAB>integer': {line!r}")
        stats[parts[0].strip()] = int(parts[1])
    missing = [k for k in (*HIT_KEYS, MISS_KEY) if k not in stats]
    if missing:
        raise StatsError(f"no {', '.join(missing)} in the statistics (is this `ccache --print-stats`?)")
    return stats


def describe(stats: dict) -> tuple:
    direct, pre = (stats[k] for k in HIT_KEYS)
    hits, misses = direct + pre, stats[MISS_KEY]
    calls = hits + misses
    rate = f", hit rate {100.0 * hits / calls:.1f} %" if calls else ""
    other = sorted(
        f"{k}={v}"
        for k, v in stats.items()
        if v and k not in NOT_CALLS and k not in HIT_KEYS and k != MISS_KEY and not k.endswith("timestamp")
    )
    line = (
        f"{hits} hits (direct {direct}, preprocessed {pre}), {misses} misses{rate}; "
        f"other non-zero counters: {', '.join(other) if other else 'none'}; "
        f"cache {stats.get('cache_size_kibibyte', 0) / 1024:.1f} MiB in {stats.get('files_in_cache', 0)} files"
    )
    return hits, misses, line


def report(path: Path, job: str, require_use: bool, quiet: bool) -> int:
    if not path.is_file():
        print(f"::error title=ccache [{job}]::no statistics at {path}")
        return 1
    try:
        stats = parse(path.read_text(encoding="utf-8"))
    except StatsError as e:
        print(f"::error title=ccache [{job}]::{path}: {e}")
        return 1
    hits, misses, line = describe(stats)
    if not quiet:
        print(f"ccache [{job}]: {line}")
        print(f"::notice title=ccache [{job}]::{line}")
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary:
            with open(summary, "a", encoding="utf-8") as f:
                f.write(f"**ccache [{job}]**: {line}\n\n")
    if require_use and hits + misses == 0:
        print(
            f"::error title=ccache [{job}]::ccache saw no compile in this job: no Verilator build "
            "went through it (OBJCACHE=ccache did not reach make, or no Verilator build ran)"
        )
        return 1
    return 0


SAMPLE = """\
autoconf_test\t0
bad_compiler_arguments\t0
cache_miss\t154
cache_size_kibibyte\t5632
cleanups_performed\t0
could_not_use_precompiled_header\t0
direct_cache_hit\t924
direct_cache_miss\t230
files_in_cache\t174
preprocessed_cache_hit\t72
preprocessed_cache_miss\t154
stats_updated_timestamp\t1763400000
stats_zeroed_timestamp\t1763399900
"""


def self_test() -> int:
    # Each case prints `::error`/`::notice`; the output is swallowed so that
    # no real annotation reaches the CI log.
    with tempfile.TemporaryDirectory() as d, contextlib.redirect_stdout(io.StringIO()) as out:
        os.environ.pop("GITHUB_STEP_SUMMARY", None)
        p = Path(d) / "stats.tsv"
        assert report(p, "t", False, False) == 1, "a missing file must fail"
        p.write_text(SAMPLE, encoding="utf-8")
        assert report(p, "t", True, False) == 0
        hits, misses, line = describe(parse(SAMPLE))
        assert (hits, misses) == (996, 154), (hits, misses)
        assert "hit rate 86.6 %" in line and "other non-zero counters: none" in line, line
        assert "cache 5.5 MiB in 174 files" in line, line
        odd = SAMPLE.replace("could_not_use_precompiled_header\t0", "could_not_use_precompiled_header\t3")
        assert "could_not_use_precompiled_header=3" in describe(parse(odd))[2]
        unused = "".join(
            l.split("\t")[0] + "\t0\n" if l.split("\t")[0] in (*HIT_KEYS, MISS_KEY) else l + "\n"
            for l in SAMPLE.splitlines()
        )
        assert report(p.with_name("u.tsv"), "t", True, False) == 1
        p.with_name("u.tsv").write_text(unused, encoding="utf-8")
        assert report(p.with_name("u.tsv"), "t", False, False) == 0, "zero calls without --require-use passes"
        assert report(p.with_name("u.tsv"), "t", True, False) == 1, "zero calls with --require-use fails"
        p.write_text("cache_miss\t1\n", encoding="utf-8")
        assert report(p, "t", False, False) == 1, "missing hit counters must fail"
        p.write_text("Cacheable calls: 1 / 1\n", encoding="utf-8")
        assert report(p, "t", False, False) == 1, "the human-readable form must fail"
        summary = Path(d) / "summary.md"
        os.environ["GITHUB_STEP_SUMMARY"] = str(summary)
        p.write_text(SAMPLE, encoding="utf-8")
        assert report(p, "t", False, False) == 0
        assert summary.read_text(encoding="utf-8").startswith("**ccache [t]**: 996 hits")
        del os.environ["GITHUB_STEP_SUMMARY"]
        assert report(p, "t", True, True) == 0
    assert "::error" in out.getvalue()
    print("ccache_report self-test: ok")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("stats", nargs="?", help="output of `ccache --print-stats`")
    ap.add_argument("--job", default="ci")
    ap.add_argument("--require-use", action="store_true", help="fail when ccache saw no compile")
    ap.add_argument("--quiet", action="store_true", help="print only an error")
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return self_test()
    if not a.stats:
        ap.error("STATS is required")
    return report(Path(a.stats), a.job, a.require_use, a.quiet)


if __name__ == "__main__":
    sys.exit(main())
