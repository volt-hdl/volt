package lib;

pub const WIDTH : u32 = 8

// Not `pub`: another file cannot read it, also where the parser unrolls
// or flattens the name away before name resolution (#139).
const PICK : u32 = 1
