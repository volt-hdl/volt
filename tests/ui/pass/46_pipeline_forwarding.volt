// Pipeline sözdizimi (ADR-0038): forwarding + bypass + göreli stage
// referansları. stage(...) içeren let'ler kendi aşamalarının
// gecikmesine sabitlenir (otomatik yeniden zamanlama iddiası) —
// elle Delayed anotasyonu gerekmez.
pipeline(4) Fwd {
    in  clk : clock
    in  instr : u32
    out q : u32

    invariant: !(stage(Exec).we && stage(Exec).dst == 0)
    invariant: !(stage(Wb).we && stage(Wb).dst == 0)

    reg file : [u32; 8] = [0; 8]

    stage Fetch {
        let ir : u32 = instr
    }
    stage Dec {
        let dst : u3 = ir[2:0] as u3
        let src : u3 = ir[5:3] as u3
        let we  : bool = ir[6] && dst != 0
        // WB→Dec bypass: yazan bu çevrim Wb'de.
        let v : u32 = if stage(Wb).we && stage(Wb).dst == src {
            stage(Wb).res } else { file[src] }
    }
    stage Exec {
        // Exec→Exec forwarding: bir önceki komutun sonucu.
        let res : u32 = (if stage(+1).we && stage(+1).dst == src {
            stage(+1).res } else { v }) + 1
    }
    stage Wb {
        if we {
            file[dst] <= res
        }
    }

    q = stage(Wb).res
}
