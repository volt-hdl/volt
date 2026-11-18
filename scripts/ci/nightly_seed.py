#!/usr/bin/env python3
"""The seed of a nightly run: the night's UTC date (ADR-0123, #255).

GitHub starts scheduled runs late (6.5 to 8.6 hours in November 2026). A
run set for 17:03 UTC can start after midnight, so the UTC date of its
start is not always the date of its night. The night's date is the UTC
date 12 hours before the run starts: a run set for the evening of day D
gets D whether it starts on time or late until 12:00 UTC the next day
(18 h 57 min for a 17:03 cron, 17 h 39 min for 18:21), every night has a
single seed, and two nights never share one.

    python3 scripts/ci/nightly_seed.py --kind metamorphic   # 2026-11-18
    python3 scripts/ci/nightly_seed.py --kind difftest      # 2026111800 (shard k: + k)
    python3 scripts/ci/nightly_seed.py --kind difftest --now 2026-11-19T01:03:00Z
    python3 scripts/ci/nightly_seed.py --self-test

The self-test reads the cron lines of the two workflows that use it and
checks every delay from 0 to 16 hours (about twice the largest delay
measured) against them.
"""

from __future__ import annotations

import argparse
import re
import sys
from datetime import date, datetime, timedelta, timezone
from pathlib import Path

sys.dont_write_bytecode = True  # no __pycache__ in the checkout

SHIFT = timedelta(hours=12)
ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = {"difftest": "difftest.yml", "metamorphic": "metamorphic-nightly.yml"}


def night(now: datetime) -> date:
    """The night's UTC date: the UTC date 12 hours before `now`."""
    if now.tzinfo is None:
        raise ValueError(f"{now.isoformat()} has no time zone")
    return (now.astimezone(timezone.utc) - SHIFT).date()


def seed(kind: str, now: datetime) -> str:
    d = night(now)
    if kind == "metamorphic":
        return d.isoformat()  # VOLT_METAMORPHIC_SEED, ADR-0117 section 5
    if kind == "difftest":
        return d.strftime("%Y%m%d") + "00"  # VOLT_DIFFTEST_SEED of shard 0
    raise ValueError(f"unknown kind {kind!r}")


def cron(kind: str) -> tuple[int, int]:
    """(hour, minute) of the workflow's daily cron line."""
    text = (ROOT / ".github" / "workflows" / WORKFLOWS[kind]).read_text(encoding="utf-8")
    found = re.findall(r'^\s*- cron: "(\d+) (\d+) \* \* \*"', text, re.M)
    if len(found) != 1:
        raise ValueError(f"{WORKFLOWS[kind]}: expected one daily cron line, found {found}")
    minute, hour = (int(x) for x in found[0])
    return hour, minute


def self_test() -> int:
    failures = []

    def check(name: str, got: str, want: str) -> None:
        if got != want:
            failures.append(f"{name}: got {got}, want {want}")

    utc = timezone.utc
    t = lambda s: datetime.fromisoformat(s.replace("Z", "+00:00"))  # noqa: E731

    # 1. No delay: the run starts at its cron time on the evening of the
    #    18th and runs the 18th.
    check("no delay, difftest", seed("difftest", t("2026-11-18T17:03:00Z")), "2026111800")
    check("no delay, metamorphic", seed("metamorphic", t("2026-11-18T18:21:00Z")), "2026-11-18")
    # 2. Eight hours late: the run starts on the 19th and still runs the 18th.
    check("8 h late, difftest", seed("difftest", t("2026-11-19T01:03:00Z")), "2026111800")
    check("8 h late, metamorphic", seed("metamorphic", t("2026-11-19T02:21:00Z")), "2026-11-18")
    # 3. Across midnight: one second before and after 00:00 UTC, the same
    #    night; across a year too.
    check("23:59:59", seed("metamorphic", t("2026-11-18T23:59:59Z")), "2026-11-18")
    check("00:00:01", seed("metamorphic", t("2026-11-19T00:00:01Z")), "2026-11-18")
    check("new year", seed("difftest", t("2027-01-01T01:03:00Z")), "2026123100")
    # The boundary itself: 12:00 UTC is the next night's first second.
    check("11:59:59", seed("metamorphic", t("2026-11-19T11:59:59Z")), "2026-11-18")
    check("12:00:00", seed("metamorphic", t("2026-11-19T12:00:00Z")), "2026-11-19")
    # A time zone other than UTC is converted: 14:30 +03:00 on the 19th is
    # 11:30 UTC, still the night of the 18th (without the conversion, the
    # 19th).
    check("+03:00", seed("difftest", t("2026-11-19T14:30:00+03:00")), "2026111800")
    try:
        night(datetime(2026, 11, 18, 17, 3))
        failures.append("naive datetime: accepted")
    except ValueError:
        pass

    # 4. Against the workflows' cron lines: every delay from 0 to 16 hours
    #    (about twice the 8.6 h measured), in 20-minute steps, gives the
    #    cron's own day; a late night and an on-time next night (and the
    #    reverse) never share a seed.
    for kind in WORKFLOWS:
        hour, minute = cron(kind)
        day = date(2026, 11, 18)
        start = datetime(day.year, day.month, day.day, hour, minute, tzinfo=utc)
        want = seed(kind, datetime(2026, 11, 18, 12, 0, tzinfo=utc))
        for step in range(0, 16 * 3 + 1):
            delay = timedelta(minutes=20 * step)
            check(f"{kind} cron {hour:02}:{minute:02} + {delay}", seed(kind, start + delay), want)
        late, next_on_time = start + timedelta(hours=8, minutes=38), start + timedelta(days=1)
        if seed(kind, late) == seed(kind, next_on_time):
            failures.append(f"{kind}: a late night and the next on-time night share {seed(kind, late)}")
        on_time, next_late = start, start + timedelta(days=1, hours=8, minutes=38)
        if seed(kind, on_time) == seed(kind, next_late):
            failures.append(f"{kind}: an on-time night and the next late night share {seed(kind, on_time)}")

    if failures:
        for f in failures:
            print(f"nightly_seed self-test FAIL {f}")
        return 1
    crons = ", ".join(f"{k} {h:02}:{m:02}" for k, (h, m) in ((k, cron(k)) for k in WORKFLOWS))
    print(f"nightly_seed self-test: ok (no delay, 8 h late, across midnight; delays 0-16 h against the crons: {crons} UTC)")
    return 0


def main() -> int:
    if sys.argv[1:] == ["--self-test"]:
        return self_test()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--kind", required=True, choices=sorted(WORKFLOWS))
    ap.add_argument("--now", help="ISO time with a time zone (default: now)")
    a = ap.parse_args()
    now = datetime.fromisoformat(a.now.replace("Z", "+00:00")) if a.now else datetime.now(timezone.utc)
    print(seed(a.kind, now))
    return 0


if __name__ == "__main__":
    sys.exit(main())
