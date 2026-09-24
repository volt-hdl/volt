// ADR-0074: explicit codes with a base type carry an external encoding
// (here RISC-V major opcodes) into the design. 'as u7' yields the code
// itself; decoding a raw field back into the enum is an explicit match
// (the opcode field of an instruction word).
enum Opcode : u7 { Load = 0b0000011, Store = 0b0100011, Alu = 0b0110011, Illegal = 0b1111111 }

const RESET_OP : Opcode = Opcode::Illegal

module OpcodeReg {
    in  clk   : clock
    in  field : bits<7>
    out code  : u7
    out is_ls : bool

    reg op_r : Opcode = RESET_OP

    on clk {
        match field as u7 {
            0b0000011 => { op_r <= Opcode::Load }
            0b0100011 => { op_r <= Opcode::Store }
            0b0110011 => { op_r <= Opcode::Alu }
            _ => { op_r <= Opcode::Illegal }
        }
    }

    code  = op_r as u7
    is_ls = op_r == Opcode::Load || op_r == Opcode::Store
}
