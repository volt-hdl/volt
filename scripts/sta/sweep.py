#!/usr/bin/env python3
"""ADR-0079 output net: every generated .sdc goes through OpenSTA `read_sdc`.

run.py proves the targeted .sdc of ONE design is a second net (ADR-0065);
this sweep checks that EVERY constraint file Volt writes for the corpus
(tests/ui/pass, examples/ without *_test.volt, tests/fixtures without the
parity negatives) is read by the real consumer:

  volt build --emit=sdc (targeted and clock-groups styles)
  for each .sdc with at least one command (comment-only files have nothing
  to read -- no clock frequency, W0022):
    Yosys (run.synth: cells from min.lib, hierarchy kept, flops renamed)
    OpenSTA: read_liberty, read_verilog, link_design <module>, read_sdc
      -> no "Error"/"Warning" line (an unknown object is a warning there)
      -> every get_cells pattern names >= 1 cell

Fixtures marked `//~ NET-SKIP` or `//~ SYNTH-SKIP` (see
crates/volt-driver/tests/output_net_tests.rs) are left out: they cannot be
synthesized by Yosys on purpose.

Tools as in run.py (VOLT_STA_YOSYS, VOLT_STA_STA, VOLT_BIN). A missing tool
is an error, never a skip (ADR-0079 section 3). Exit 0 only when every file
reads cleanly; the summary lands in <work>/summary.md.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import shutil
import subprocess
import sys

import run as sta_run

ROOT = sta_run.ROOT
SKIP_MARKERS = ("//~ NET-SKIP", "//~ SYNTH-SKIP")
STYLES = {"targeted": "targeted", "groups": "clock-groups"}


def corpus() -> list[pathlib.Path]:
    files = sorted((ROOT / "tests" / "ui" / "pass").glob("*.volt"))
    for base in ("examples", "tests/fixtures"):
        files += sorted(
            p
            for p in (ROOT / base).rglob("*.volt")
            if not p.name.endswith("_test.volt") and "parity" not in p.parts and "build" not in p.parts
        )
    return files


def skipped(design: pathlib.Path) -> bool:
    text = design.read_text(encoding="utf-8")
    return any(line.strip().startswith(SKIP_MARKERS) for line in text.splitlines())


def has_commands(sdc: pathlib.Path) -> bool:
    return any(l.strip() and not l.lstrip().startswith("#") for l in sdc.read_text(encoding="utf-8").splitlines())


def build(design: pathlib.Path, style: str, out: pathlib.Path) -> None:
    cmd = [str(sta_run.volt_bin()), "build", "--target-dir", str(out), "--emit=sdc", f"--sdc-style={STYLES[style]}", str(design)]
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if proc.returncode != 0:
        raise SystemExit(f"volt build failed for {design}:\n{proc.stdout}{proc.stderr}")


def stage_rtl(design: pathlib.Path, rtl: pathlib.Path, dst: pathlib.Path) -> None:
    """Generated RTL plus the extern bodies named by @source (ADR-0076),
    under <work>/rtl where run.synth reads every *.sv."""
    if dst.exists():
        return
    shutil.copytree(rtl, dst)
    for rel in re.findall(r'@source\("([^"]+)"\)', design.read_text(encoding="utf-8")):
        src = design.parent / rel
        shutil.copyfile(src, dst / f"extern_{src.stem}.sv")


def read_sdc(netlist: pathlib.Path, top: str, sdc: pathlib.Path, work: pathlib.Path) -> list[str]:
    """Problems OpenSTA reports while reading one .sdc; empty = clean."""
    lines = ["read_liberty min.lib", f"read_verilog {netlist.name}", f"link_design {top}", f"read_sdc {sdc.name}"]
    for p in sta_run.sdc_patterns(sdc):
        lines.append(f'puts "PATTERN {p} => [llength [get_cells {{{p}}}]]"')
    lines += ['puts "## end"', "exit", ""]
    name = f"read_{sdc.stem}"
    (work / f"{name}.tcl").write_text("\n".join(lines), encoding="utf-8", newline="\n")
    out = sta_run.run(sta_run.tool("VOLT_STA_STA", "sta", work) + ["-no_splash", "-exit", f"{name}.tcl"], work, work / f"{name}.sta.log")
    problems = [l for l in out.splitlines() if l.startswith(("Error", "Warning"))]
    if "## end" not in out:
        problems.append("OpenSTA stopped before the end of the script")
    for m in re.finditer(r"^PATTERN (.*) => (\d+)$", out, re.M):
        if int(m.group(2)) == 0:
            problems.append(f"get_cells {{{m.group(1)}}} names no cell")
    return problems


def sweep_design(design: pathlib.Path, work_root: pathlib.Path) -> tuple[int, list[str]]:
    rel = design.relative_to(ROOT).as_posix()
    work = work_root / rel.removesuffix(".volt").replace("/", "__")
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    shutil.copyfile(sta_run.HERE / "min.lib", work / "min.lib")
    checked, problems = 0, []
    netlists: dict[str, pathlib.Path] = {}
    for style in STYLES:
        out = work / f"volt_{style}"
        build(design, style, out)
        for sdc in sorted((out / "constraints").glob("*.sdc")):
            if not has_commands(sdc):
                continue
            top = sdc.stem
            if top not in netlists:
                stage_rtl(design, out / "rtl", work / "rtl")
                netlists[top], _ = sta_run.synth(f"{top}_net", work / "rtl", top, work)
            local = work / f"{top}.{style}.sdc"
            shutil.copyfile(sdc, local)
            checked += 1
            problems += [f"{rel} {local.name}: {p}" for p in read_sdc(netlists[top], top, local, work)]
    return checked, problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--work", type=pathlib.Path, default=ROOT / "build" / "sta" / "sweep", help="work dir")
    ap.add_argument("--min-files", type=int, default=20, help="fail if fewer .sdc files were read (empty corpus guard)")
    args = ap.parse_args()
    work_root = args.work.resolve()
    work_root.mkdir(parents=True, exist_ok=True)
    total, problems, skipped_designs = 0, [], []
    for design in corpus():
        if skipped(design):
            skipped_designs.append(design.relative_to(ROOT).as_posix())
            continue
        checked, found = sweep_design(design, work_root)
        if checked:
            sta_run.log(f"  {design.relative_to(ROOT).as_posix()}: {checked} .sdc, {len(found)} problem(s)")
        total += checked
        problems += found
    lines = [
        "# ADR-0079 SDC sweep (OpenSTA read_sdc)",
        "",
        f"- .sdc files read: {total} (both styles)",
        f"- skipped (NET-SKIP / SYNTH-SKIP): {', '.join(skipped_designs) or 'none'}",
        f"- problems: {len(problems)}",
        *[f"  - {p}" for p in problems],
    ]
    if total < args.min_files:
        problems.append(f"only {total} .sdc files read (< {args.min_files})")
        lines.append(f"- FAIL: only {total} .sdc files read (< {args.min_files})")
    (work_root / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("\n".join(lines))
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main())
