// ADR-0077: `p as uN` packs the fields (first field most significant,
// zero-extended when N > W); `raw as P` slices a W-bit value back into
// the fields — allowed only when no field is an enum or Trit.
struct RType {
    funct7 : u7
    rs2    : u5
    rs1    : u5
    funct3 : u3
    rd     : u5
    opcode : u7
}

module Decode {
    in  instr  : u32
    out rd     : u5
    out opcode : u7
    out word   : u32
    out wide   : u40

    let r : RType = instr as RType
    rd = r.rd
    opcode = r.opcode
    word = r as u32
    wide = r as u40
}
