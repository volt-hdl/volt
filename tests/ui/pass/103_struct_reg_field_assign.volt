// ADR-0077: a struct register holds a whole value; single fields are
// updated with field assignments, the whole value with a literal. Every
// field of the reset value is given (E2014 otherwise); a const struct is
// a valid reset value.
struct Cfg {
    enable : bool
    div    : u4
    mode   : u2
}

const DEFAULT_CFG : Cfg = Cfg { enable: false, div: 9, mode: 1 }

module Timer {
    in  clk   : clock
    in  load  : bool
    in  div   : u4
    in  kick  : bool
    out tick  : bool
    out mode  : u2

    reg cfg : Cfg = DEFAULT_CFG
    reg cnt : u4 = 0

    on clk {
        if load {
            cfg <= Cfg { enable: true, div: div, mode: cfg.mode }
        } else if kick {
            cfg.enable <= !cfg.enable
        }
        if cfg.enable {
            if cnt == cfg.div { cnt <= 0 } else { cnt <= cnt + 1 }
        }
    }

    tick = cfg.enable && cnt == cfg.div
    mode = cfg.mode
}
