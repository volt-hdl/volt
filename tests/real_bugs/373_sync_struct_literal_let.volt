//~ W3003
// #373: sync() of an untyped let bound to a struct literal is one
// synchronizer per field in the SV (ADR-0077 Karar 5 rule 12:
// sync_p_a_stage0, sync_p_b_stage0), but the .sdc and .xdc constrained
// sync_p_stage0_reg*, a cell the netlist does not have: Vivado's and
// OpenSTA's get_cells found nothing, so the false path into each first
// stage and the ASYNC_REG property were dropped without a word. The
// constraint walker took a signal's struct type from its declared type
// alone; the SV lowering also takes it from an untyped let's value. ADR-0065
// sec. 4.2 constrains the path into each generated synchronizer's first
// stage. The one diagnostic is W3003, from the specification
// (domain-inference.md: a multi-bit sync() does not keep the bits
// coherent; the struct is two bits). Fixed by the pull request of #373:
// both read the struct type from volt_ast::struct_signals.

domain Fast { clock = posedge, reset = none, frequency = 100.mhz, }
domain Slow { clock = posedge, reset = none, frequency = 25.mhz, }

struct P {
    a : bool,
    b : bool,
}

pub module SyncStruct {
    in  fclk : clock @Fast
    in  sclk : clock @Slow
    in  d    : bool  @Fast
    out q    : P     @Slow

    reg a_r : bool = false
    reg b_r : bool = false
    on fclk {
        a_r <= d
        b_r <= !d
    }
    let p = P { a: a_r, b: b_r }
    q = sync(p, sclk)
}
