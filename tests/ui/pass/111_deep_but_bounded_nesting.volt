// ADR-0080: the parser bounds tree depth (E0018 past 256 levels), but real
// designs stay far below it. A 64-term XOR chain (one tree level per link),
// a 16-link else-if chain and 32 nested parentheses compile cleanly.

module DeepButBounded {
    in  clk : clock
    in  d   : bits<64>
    in  sel : u8
    out p   : bool
    out y   : u8

    reg r : u8 = 0

    on clk {
        if sel == 0 {
            r <= 0
        } else if sel == 1 {
            r <= 1
        } else if sel == 2 {
            r <= 2
        } else if sel == 3 {
            r <= 3
        } else if sel == 4 {
            r <= 4
        } else if sel == 5 {
            r <= 5
        } else if sel == 6 {
            r <= 6
        } else if sel == 7 {
            r <= 7
        } else if sel == 8 {
            r <= 8
        } else if sel == 9 {
            r <= 9
        } else if sel == 10 {
            r <= 10
        } else if sel == 11 {
            r <= 11
        } else if sel == 12 {
            r <= 12
        } else if sel == 13 {
            r <= 13
        } else if sel == 14 {
            r <= 14
        } else if sel == 15 {
            r <= 15
        } else {
            r <= sel
        }
    }

    p = d[0] ^ d[1] ^ d[2] ^ d[3] ^ d[4] ^ d[5] ^ d[6] ^ d[7] ^ d[8] ^ d[9] ^ d[10] ^ d[11] ^ d[12] ^ d[13] ^ d[14] ^ d[15] ^ d[16] ^ d[17] ^ d[18] ^ d[19] ^ d[20] ^ d[21] ^ d[22] ^ d[23] ^ d[24] ^ d[25] ^ d[26] ^ d[27] ^ d[28] ^ d[29] ^ d[30] ^ d[31] ^ d[32] ^ d[33] ^ d[34] ^ d[35] ^ d[36] ^ d[37] ^ d[38] ^ d[39] ^ d[40] ^ d[41] ^ d[42] ^ d[43] ^ d[44] ^ d[45] ^ d[46] ^ d[47] ^ d[48] ^ d[49] ^ d[50] ^ d[51] ^ d[52] ^ d[53] ^ d[54] ^ d[55] ^ d[56] ^ d[57] ^ d[58] ^ d[59] ^ d[60] ^ d[61] ^ d[62] ^ d[63]
    y = ((((((((((((((((((((((((((((((((r))))))))))))))))))))))))))))))))
}
