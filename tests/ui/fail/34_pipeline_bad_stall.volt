//~ E5013
// Önek olmayan stall kümesi: Decode'u Fetch durmadan durdurmak
// mümkün değildir — tutulan aşama arkadan ezilir (ADR-0038 §4).
pipeline(3) P {
    in  clk : clock
    in  x : u32
    out y : u32

    stage Fetch {
        let a : u32 = x
    }
    stage Decode {
        let b : u32 = a + 1
    }
    stage Execute {
        let c : u32 = b + 1
    }

    stall Decode when x == 0

    y = stage(Execute).c
}
