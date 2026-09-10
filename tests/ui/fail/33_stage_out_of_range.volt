//~ E5012
// stage(+9) son aşamayı aşar: 2 aşamalı boru hattında aşama 1
// içinden +9, 0..=1 aralığının dışına düşer.
pipeline(2) P {
    in  clk : clock
    in  x : u32
    out y : u32

    stage Fetch {
        let a : u32 = x
    }
    stage Decode {
        let b : u32 = stage(+9).a
    }

    y = stage(Decode).b
}
