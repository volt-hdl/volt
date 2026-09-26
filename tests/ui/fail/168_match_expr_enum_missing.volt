//~ E0014
// ADR-0083 Karar 3: enum sınananda her varyant ya da `_` (ADR-0074).

enum Op { Add, Sub, And }

module EnumMissing {
    in  op : Op
    in  a  : u8
    in  b  : u8
    out y  : u8

    y = match op { Op::Add => a + b, Op::Sub => a - b }
    //~^ ERROR 'match' on enum 'Op' does not cover every variant: missing Op::And
}
