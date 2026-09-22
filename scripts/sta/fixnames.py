"""Rename the flip-flop cells of a Yosys netlist after the wire they drive.

Yosys (`write_verilog`) names mapped flip-flops `_NNN_`, so the `get_cells
{<name>_reg*}` patterns of a Volt-generated .sdc match nothing. This
post-processor gives every DFF/DFFR instance the Vivado/DC style name
`<wire>_reg`, with bit and word indices kept in brackets:

    .Q(count)              -> count_reg
    .Q(acc_r[3])           -> \acc_r_reg[3]
    .Q(\fifo_mem[5] [2])   -> \fifo_mem_reg[5][2]

Names that contain brackets are written as escaped Verilog identifiers, which
OpenSTA reads back verbatim (`acc_r_reg[3]`), so the glob `acc_r_reg*`
matches every bit. Hierarchy is left untouched: a cell renamed inside module
`Relay` is `u/level_r_reg` once `u` is linked in the top module (ADR-0054 §7
and ADR-0065 stage-3 note 2: keep the hierarchy instead of flattening, so
the `/` separator comes from the timing tool, not from a text substitution).

Usage: python fixnames.py <in.v> <out.v>
Prints the number of renamed cells and fails if a name would repeat inside
one module (the same wire name in two modules is fine: hierarchy keeps them apart).
"""

from __future__ import annotations

import re
import sys

FLOP_CELLS = ("DFFR", "DFF")

# "  DFFR _11_ (\n    .CK(clk_b),\n    .D(...),\n    .Q(a_data),\n    .RN(...)\n  );"
CELL_RE = re.compile(
    r"(\b(?:" + "|".join(FLOP_CELLS) + r")\s+)(\S+)(\s*\(\s*(?:[^;]*?))\.Q\(([^)]*)\)",
    re.S,
)

# `\name[5] [2]`, `\name[5] `, `name[2]`, `name`
Q_RE = re.compile(
    r"^\\?(?P<base>[A-Za-z_][A-Za-z0-9_$.]*)(?P<idx>(?:\[[0-9]+\])*)\s*(?P<bit>\[[0-9]+\])?$"
)


def reg_name(q_expr: str) -> str | None:
    """Map a Q-pin expression to `<base>_reg<indices>`; None if not a plain wire."""
    m = Q_RE.match(q_expr.strip())
    if m is None:
        return None
    return f"{m.group('base')}_reg{m.group('idx')}{m.group('bit') or ''}"


MODULE_RE = re.compile(r"^module\b", re.M)


SIGNED_RE = re.compile(r"\bsigned\s+")


def rename(text: str) -> tuple[str, int]:
    """Rename per module: the same wire name may live in several modules.

    Also drops `signed` from declarations: Yosys 0.69 keeps the signedness of
    the source (`wire signed [7:0] x`), OpenSTA's Verilog reader rejects the
    keyword (`syntax error`), and timing analysis does not depend on it.
    """
    text = SIGNED_RE.sub("", text)
    parts = MODULE_RE.split(text)
    total = 0
    out = [parts[0]]
    for part in parts[1:]:
        renamed, n = rename_module("module" + part)
        out.append(renamed[len("module"):])
        total += n
    return "module".join(out), total


def rename_module(text: str) -> tuple[str, int]:
    seen: set[str] = set()
    count = 0

    def repl(m: re.Match[str]) -> str:
        nonlocal count
        name = reg_name(m.group(4))
        if name is None:
            return m.group(0)
        if name in seen:
            raise SystemExit(f"fixnames: duplicate register name {name} in one module")
        seen.add(name)
        count += 1
        ident = f"\\{name} " if "[" in name else name
        return f"{m.group(1)}{ident}{m.group(3)}.Q({m.group(4)})"

    return CELL_RE.subn(repl, text)[0], count


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__)
        return 2
    src, dst = argv[1], argv[2]
    with open(src, encoding="utf-8") as f:
        text = f.read()
    out, n = rename(text)
    with open(dst, "w", encoding="utf-8", newline="\n") as f:
        f.write(out)
    print(f"fixnames: renamed {n} flops -> {dst}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
