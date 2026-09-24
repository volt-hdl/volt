// ADR-0077: a struct port connects modules; each field becomes its own SV
// port (`cmd_op`, `cmd_a`, ...) and the instance binds them field by field.
enum Op { Add, Sub, Pass }

struct Cmd {
    op : Op
    a  : u8
    b  : u8
}

struct Sum {
    value : u9
    zero  : bool
}

module Alu {
    in  cmd : Cmd
    out res : Sum

    let wide_a : u9 = cmd.a as u9
    let wide_b : u9 = cmd.b as u9
    let value : u9 = if cmd.op == Op::Sub { wide_a - wide_b } else { wide_a + wide_b }
    res = Sum { value: value, zero: value == 0 }
}

module Top {
    in  op : Op
    in  a  : u8
    in  b  : u8
    out y  : u9
    out z  : bool

    let alu = Alu { cmd: Cmd { op: op, a: a, b: b } }
    y = alu.res.value
    z = alu.res.zero
}
