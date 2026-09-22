#!/usr/bin/env python3
"""ADR-0065 stage 4: prove with OpenSTA that the targeted .sdc is a second net.

The compiler is the first net: a crossing without a synchronizer is E3001, a
reset released in the wrong clock is E3003. The generated .sdc is the second:
it must leave every such path VISIBLE to the timing tool, so that a path the
checker misses still shows up as a timing violation. `set_clock_groups
-asynchronous` (ADR-0054, now `--sdc-style=clock-groups`) hid them all.

Flow, for one Volt design (default: tests/ui/pass/87, every bridge kind, two
raw resets, a child instance, 100 MHz / 25.175 MHz):

  volt build --emit=sdc  (both styles)   -> rtl/*.sv, <top>.targeted.sdc, <top>.groups.sdc
  Yosys: proc/memory/techmap/dfflibmap/abc against scripts/sta/min.lib (no
         flatten, no opt_merge: hierarchy stays `u/x`, twin registers stay apart)
  fixnames.py: DFF cells named <wire>_reg[...] so the .sdc patterns match
  OpenSTA:
    clean netlist + targeted .sdc   (4.3 / 4.4)  every get_cells pattern names
        >= 1 cell; no VIOLATED endpoint on any inter-clock path
    probed netlist + groups .sdc    (4.1 / 4.2)  the injected unsynchronized
        crossing and the wrong-clock reset are "No paths found"
    probed netlist + targeted .sdc  (4.1 / 4.2)  the crossing is VIOLATED, the
        wrong-clock reset is a timed recovery endpoint

The probes are two flops appended to the build/ COPY of the top module's SV;
Volt itself refuses both (E3001, E3003), which is the point: they stand for a
path the first net missed.

Tools come from the environment so the same script runs in CI (native yosys
from OSS CAD Suite, OpenSTA in Docker) and on a developer machine (both in
Docker):
  VOLT_STA_YOSYS  command template, `{work}` = absolute work dir (default: yosys)
  VOLT_STA_STA    command template for OpenSTA               (default: sta)
  VOLT_BIN        volt binary (default: target/debug/volt[.exe])

Exit status 0 only when every expectation holds; the summary is written to
<work>/summary.md and printed.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import shlex
import shutil
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEFAULT_DESIGN = ROOT / "tests" / "ui" / "pass" / "87_sdc_targeted_all_bridges.volt"

STYLES = ("targeted", "groups")
STYLE_FLAG = {"targeted": "targeted", "groups": "clock-groups"}

PROBE_CDC = "rdc_probe_cdc"
PROBE_RST = "rdc_probe_rst"


class Check:
    """One expectation with its evidence line."""

    def __init__(self, step: str, what: str, ok: bool, evidence: str) -> None:
        self.step, self.what, self.ok, self.evidence = step, what, ok, evidence


def log(msg: str) -> None:
    print(msg, flush=True)


def tool(env: str, default: str, work: pathlib.Path) -> list[str]:
    tmpl = os.environ.get(env)
    if not tmpl:
        return [default]
    return shlex.split(tmpl.replace("{work}", work.as_posix()))


def run(cmd: list[str], cwd: pathlib.Path, logfile: pathlib.Path) -> str:
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    out = proc.stdout + proc.stderr
    logfile.write_text(out, encoding="utf-8")
    if proc.returncode != 0:
        tail = "\n".join(out.splitlines()[-25:])
        raise SystemExit(f"command failed ({proc.returncode}): {' '.join(cmd)}\n--- {logfile} (tail) ---\n{tail}")
    return out


def volt_bin() -> pathlib.Path:
    env = os.environ.get("VOLT_BIN")
    if env:
        return pathlib.Path(env)
    exe = "volt.exe" if os.name == "nt" else "volt"
    return ROOT / "target" / "debug" / exe


# ── 1. Volt: SV + both .sdc styles ─────────────────────────────────────────


def generate(design: pathlib.Path, top: str, work: pathlib.Path) -> dict[str, pathlib.Path]:
    sdc: dict[str, pathlib.Path] = {}
    for style in STYLES:
        out_dir = work / f"volt_{style}"
        cmd = [
            str(volt_bin()),
            "build",
            "--target-dir",
            str(out_dir),
            "--emit=sdc",
            f"--sdc-style={STYLE_FLAG[style]}",
            str(design),
        ]
        run(cmd, ROOT, work / f"volt_{style}.log")
        src = out_dir / "constraints" / f"{top}.sdc"
        if not src.is_file():
            raise SystemExit(f"{src} not produced (is --top {top} the top module?)")
        dst = work / f"{top}.{style}.sdc"
        shutil.copyfile(src, dst)
        sdc[style] = dst
    rtl = work / "rtl"
    if rtl.exists():
        shutil.rmtree(rtl)
    shutil.copytree(work / "volt_targeted" / "rtl", rtl)
    return sdc


# ── 2. Probes (injected into the build/ copy only) ─────────────────────────


def inject_probes(rtl: pathlib.Path, top: str, args: argparse.Namespace) -> pathlib.Path:
    probed = rtl.parent / "rtl_probed"
    if probed.exists():
        shutil.rmtree(probed)
    shutil.copytree(rtl, probed)
    top_sv = probed / f"{top}.sv"
    text = top_sv.read_text(encoding="utf-8")
    idx = text.rstrip().rfind("endmodule")
    if idx < 0:
        raise SystemExit(f"{top_sv}: no endmodule")
    block = f"""
    // ── ADR-0065 stage 4 probes: injected into this build/ copy by
    // scripts/sta/run.py, NEVER generated by Volt (the compiler rejects the
    // first with E3001 and the second with E3003). They stand for a path the
    // first net (the checker) missed; the second net (the .sdc) must show them.
    // P2: register of the other clock sampled without a synchronizer.
    (* keep *) logic {PROBE_CDC};
    always_ff @(posedge {args.probe_clk}) begin
        {PROBE_CDC} <= {args.probe_src} & {args.probe_src2};
    end
    // P4: flop cleared by a reset that was synchronized to the OTHER clock.
    (* keep *) logic {PROBE_RST};
    always_ff @(posedge {args.probe_clk} or negedge {args.probe_rst}) begin
        if (!{args.probe_rst}) {PROBE_RST} <= 1'b0;
        else {PROBE_RST} <= {PROBE_CDC};
    end
"""
    top_sv.write_text(text[:idx] + block.lstrip("\n") + text[idx:], encoding="utf-8", newline="\n")
    return probed


# ── 3. Yosys + fixnames ────────────────────────────────────────────────────


def synth(name: str, rtl: pathlib.Path, top: str, work: pathlib.Path) -> tuple[pathlib.Path, str]:
    files = sorted(p.name for p in rtl.glob("*.sv"))
    rel = rtl.name
    script = "\n".join(
        [
            *(f"read_verilog -sv {rel}/{f}" for f in files),
            f"hierarchy -check -top {top}",
            "proc",
            "opt_expr",
            "opt_clean",
            "memory",
            "opt_expr",
            "opt_clean",
            "techmap",
            "opt_expr",
            "opt_clean",
            "dfflibmap -liberty min.lib",
            "abc -liberty min.lib",
            "opt_clean",
            "stat -liberty min.lib",
            f"write_verilog -noattr {name}.v",
            "",
        ]
    )
    (work / f"{name}.ys").write_text(script, encoding="utf-8", newline="\n")
    out = run(tool("VOLT_STA_YOSYS", "yosys", work) + ["-s", f"{name}.ys"], work, work / f"{name}.yosys.log")
    named = work / f"{name}_named.v"
    fix = run([sys.executable, str(HERE / "fixnames.py"), f"{name}.v", named.name], work, work / f"{name}.fixnames.log")
    m = re.search(r"renamed (\d+) flops", fix)
    flops = m.group(1) if m else "?"
    # `stat` prints one table per module and a hierarchy total last; the
    # column order changed between Yosys 0.36 (`DFF 314`) and 0.66 (`314 1256 DFF`).
    totals: dict[str, str] = {}
    for cell, n in re.findall(r"^\s+(DFFR?)\s+(\d+)\s*$", out, re.M):
        totals[cell] = n
    for n, cell in re.findall(r"^\s+(\d+)\s+[\d.]+\s+(DFFR?)\s*$", out, re.M):
        totals[cell] = n
    stat = ", ".join(f"{c} {n}" for c, n in sorted(totals.items())) or "see the yosys log"
    return named, f"{flops} flops renamed; Yosys stat (hierarchy total): {stat}"


# ── 4. OpenSTA ─────────────────────────────────────────────────────────────


def sdc_patterns(sdc: pathlib.Path) -> list[str]:
    seen: list[str] = []
    for p in re.findall(r"get_cells \{([^}]*)\}", sdc.read_text(encoding="utf-8")):
        if p not in seen:
            seen.append(p)
    return seen


def sdc_clocks(sdc: pathlib.Path) -> list[str]:
    return re.findall(r"^create_clock -name (\S+)", sdc.read_text(encoding="utf-8"), re.M)


def sta(name: str, tcl: str, work: pathlib.Path) -> str:
    (work / f"{name}.tcl").write_text(tcl, encoding="utf-8", newline="\n")
    out = run(tool("VOLT_STA_STA", "sta", work) + ["-no_splash", "-exit", f"{name}.tcl"], work, work / f"{name}.sta.log")
    errors = [l for l in out.splitlines() if l.startswith("Error")]
    if errors:
        raise SystemExit(f"OpenSTA reported errors in {name}:\n" + "\n".join(errors))
    return out


def section(out: str, header: str) -> str:
    """Text between `## header` and the next `## ` line."""
    m = re.search(rf"^## {re.escape(header)}\n(.*?)(?=^## |\Z)", out, re.S | re.M)
    return m.group(1) if m else ""


def endpoint_lines(text: str) -> list[str]:
    return [l.strip() for l in text.splitlines() if re.search(r"\((MET|VIOLATED)\)\s*$", l)]


def sta_clean(netlist: pathlib.Path, top: str, sdc: pathlib.Path, work: pathlib.Path, require_zero_tns: bool) -> list[Check]:
    patterns = sdc_patterns(sdc)
    clocks = sdc_clocks(sdc)
    lines = [
        "read_liberty min.lib",
        f"read_verilog {netlist.name}",
        f"link_design {top}",
        f"read_sdc {sdc.name}",
        'puts "## patterns"',
    ]
    for p in patterns:
        lines.append(f'puts "PATTERN {p} => [llength [get_cells {{{p}}}]]"')
    lines.append('puts "## crossings"')
    for a in clocks:
        for b in clocks:
            if a != b:
                lines.append(f'puts "PAIR {a} -> {b}"')
                lines.append(
                    f"report_checks -from [get_clocks {a}] -to [get_clocks {b}] -path_delay max -format end -group_path_count 5"
                )
    lines += [
        'puts "## worst"',
        "report_worst_slack -max",
        "report_worst_slack -min",
        "report_tns",
        'puts "## violators"',
        "report_check_types -max_delay -min_delay -recovery -removal -violators",
        'puts "## end"',
        "exit",
        "",
    ]
    out = sta("clean_targeted", "\n".join(lines), work)
    checks: list[Check] = []
    for l in section(out, "patterns").splitlines():
        m = re.match(r"PATTERN (.*) => (\d+)$", l.strip())
        if m:
            n = int(m.group(2))
            checks.append(Check("4.4", f"get_cells {{{m.group(1)}}} names a Yosys cell", n >= 1, f"{n} cell(s)"))
    cross = section(out, "crossings")
    violated = [l for l in endpoint_lines(cross) if "VIOLATED" in l]
    pairs = re.findall(r"^PAIR (.*)$", cross, re.M)
    timed = endpoint_lines(cross)
    checks.append(
        Check(
            "4.3",
            f"no VIOLATED endpoint on inter-clock paths ({', '.join(pairs) or 'single clock'})",
            not violated,
            f"{len(timed)} timed endpoint(s), {len(violated)} violated" + (": " + "; ".join(violated) if violated else ""),
        )
    )
    worst = section(out, "worst")
    tns = re.search(r"^tns(?: max)?\s+(-?[\d.]+)", worst, re.M)
    slack = re.search(r"^worst slack max\s+(-?[\d.]+)", worst, re.M)
    evidence = f"worst slack max {slack.group(1) if slack else '?'}, tns {tns.group(1) if tns else '?'}"
    if require_zero_tns:
        checks.append(Check("4.3", "tns 0.00 with the targeted .sdc (whole design)", bool(tns) and float(tns.group(1)) == 0.0, evidence))
    else:
        log(f"  info: {evidence} (intra-clock paths not part of the claim)")
    return checks


def sta_probed(netlist: pathlib.Path, top: str, sdc: dict[str, pathlib.Path], args: argparse.Namespace, work: pathlib.Path) -> list[Check]:
    results: dict[str, str] = {}
    for style in STYLES:
        tcl = "\n".join(
            [
                "read_liberty min.lib",
                f"read_verilog {netlist.name}",
                f"link_design {top}",
                f"read_sdc {sdc[style].name}",
                'puts "## P2"',
                f"report_checks -from [get_cells {{{args.probe_src}_reg}}] -to [get_cells {{{PROBE_CDC}_reg}}] -path_delay max -format end",
                'puts "## P4"',
                f"report_checks -to [get_pins {{{PROBE_RST}_reg/RN}}] -path_delay max -format end",
                'puts "## P3"',
                f"report_checks -to [get_pins {{{args.probe_src}_reg/RN}}] -path_delay max -format end",
                'puts "## recovery"',
                "report_checks -to [get_pins -hierarchical {*/RN}] -path_delay max -format end -group_path_count 1000",
                'puts "## worst"',
                "report_worst_slack -max",
                "report_tns",
                'puts "## end"',
                "exit",
                "",
            ]
        )
        results[style] = sta(f"probed_{style}", tcl, work)
    checks: list[Check] = []
    g, t = results["groups"], results["targeted"]

    def first_endpoint(text: str) -> str:
        eps = endpoint_lines(text)
        return eps[0] if eps else ("No paths found." if "No paths found" in text else text.strip()[:80])

    p2g, p2t = section(g, "P2"), section(t, "P2")
    checks.append(Check("4.1", f"clock-groups .sdc hides the unsynchronized crossing {args.probe_src} -> {PROBE_CDC}", "No paths found" in p2g, first_endpoint(p2g)))
    checks.append(Check("4.1", "targeted .sdc reports the same crossing as VIOLATED", any("VIOLATED" in l for l in endpoint_lines(p2t)), first_endpoint(p2t)))
    p4g, p4t = section(g, "P4"), section(t, "P4")
    checks.append(Check("4.2", f"clock-groups .sdc hides the wrong-clock reset recovery {args.probe_rst} -> {PROBE_RST}/RN", "No paths found" in p4g, first_endpoint(p4g)))
    checks.append(Check("4.2", "targeted .sdc times that recovery path", bool(endpoint_lines(p4t)), first_endpoint(p4t)))
    p3g, p3t = section(g, "P3"), section(t, "P3")
    checks.append(Check("4.2", f"same-clock recovery {args.probe_src}/RN timed in BOTH styles", bool(endpoint_lines(p3g)) and bool(endpoint_lines(p3t)), f"groups: {first_endpoint(p3g)} | targeted: {first_endpoint(p3t)}"))
    rg, rt = endpoint_lines(section(g, "recovery")), endpoint_lines(section(t, "recovery"))
    checks.append(Check("4.2", "targeted .sdc times more recovery endpoints (flop RN pins) than clock-groups", len(rt) > len(rg), f"groups {len(rg)}, targeted {len(rt)}"))
    wg, wt = section(g, "worst"), section(t, "worst")
    tg = re.search(r"^tns(?: max)?\s+(-?[\d.]+)", wg, re.M)
    tt = re.search(r"^tns(?: max)?\s+(-?[\d.]+)", wt, re.M)
    checks.append(Check("4.1", "probed design: tns 0 under clock-groups, negative under targeted", bool(tg and tt) and float(tg.group(1)) == 0.0 and float(tt.group(1)) < 0.0, f"groups tns {tg.group(1) if tg else '?'}, targeted tns {tt.group(1) if tt else '?'}"))
    return checks


# ── 5. Report ──────────────────────────────────────────────────────────────


def summary(checks: list[Check], notes: list[str], work: pathlib.Path) -> bool:
    rows = ["| Step | Expectation | Result | Evidence |", "|---|---|---|---|"]
    for c in checks:
        rows.append(f"| {c.step} | {c.what} | {'PASS' if c.ok else 'FAIL'} | {c.evidence} |")
    text = "\n".join(["# OpenSTA second-net proof (ADR-0065 stage 4)", "", *notes, "", *rows, ""])
    (work / "summary.md").write_text(text, encoding="utf-8", newline="\n")
    log(text)
    failed = [c for c in checks if not c.ok]
    log(f"{len(checks) - len(failed)}/{len(checks)} expectations hold")
    return not failed


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--design", type=pathlib.Path, default=DEFAULT_DESIGN, help="Volt entry file")
    ap.add_argument("--top", default="AllBridges", help="top module (name of the .sdc)")
    ap.add_argument("--work", type=pathlib.Path, default=None, help="work dir (default build/sta/<top>)")
    ap.add_argument("--probe", action="store_true", help="also run the injected-probe comparison (4.1/4.2)")
    ap.add_argument("--probe-src", default="go_r", help="register of clock A sampled by the probe (has an RN pin)")
    ap.add_argument("--probe-src2", default="sync_start_src", help="second clock-A register (keeps the probe distinct)")
    ap.add_argument("--probe-clk", default="slow_clk", help="clock B port")
    ap.add_argument("--probe-rst", default="rst_sync_fast_clk_stage1", help="clock A reset-synchronizer output")
    ap.add_argument("--require-zero-tns", action="store_true", help="4.3: the clean design must have tns 0.00")
    args = ap.parse_args()

    work = (args.work or ROOT / "build" / "sta" / args.top).resolve()
    work.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(HERE / "min.lib", work / "min.lib")
    design = args.design.resolve()
    notes = [f"- design: `{design.relative_to(ROOT) if design.is_relative_to(ROOT) else design}`, top `{args.top}`", f"- work dir: `{work}`"]

    log(f"[1/4] volt build ({', '.join(STYLES)}) -> {work}")
    sdc = generate(design, args.top, work)
    notes.append(f"- constraints: `{sdc['targeted'].name}` ({len(sdc_patterns(sdc['targeted']))} get_cells patterns, clocks {', '.join(sdc_clocks(sdc['targeted']))}), `{sdc['groups'].name}`")

    log("[2/4] yosys (clean)")
    clean, info = synth("clean", work / "rtl", args.top, work)
    notes.append(f"- clean netlist: {info}")

    log("[3/4] OpenSTA: clean netlist + targeted .sdc (4.3 / 4.4)")
    checks = sta_clean(clean, args.top, sdc["targeted"], work, args.require_zero_tns)

    if args.probe:
        log("[4/4] probes: inject, yosys, OpenSTA with both styles (4.1 / 4.2)")
        probed_rtl = inject_probes(work / "rtl", args.top, args)
        probed, info = synth("probed", probed_rtl, args.top, work)
        notes.append(f"- probed netlist: {info} (+ `{PROBE_CDC}`, `{PROBE_RST}` injected into `rtl_probed/{args.top}.sv`)")
        checks += sta_probed(probed, args.top, sdc, args, work)
    else:
        log("[4/4] probes skipped (no --probe)")

    return 0 if summary(checks, notes, work) else 1


if __name__ == "__main__":
    sys.exit(main())
