// Pipeline sözdizimi (ADR-0038): temel 3 aşamalı boru hattı.
// Aşama register'ları (fetch_a_r, decode_b_r), stall/flush muhafızları
// ve always_ff gövdesi derleyici tarafından üretilir.
pipeline(3) Basic {
    in  clk : clock
    in  x : u32
    out y : u32
    out stalled : bool

    cover: stalled

    stage Fetch {
        let a : u32 = x + 1
    }
    stage Decode {
        let b : u32 = a + 2
        stall when stage(Execute).c == 0
    }
    stage Execute {
        let c : u32 = b + stage(Fetch).a
    }

    flush Fetch when x == 99

    y = stage(Execute).c
    stalled = stall_decode
}
