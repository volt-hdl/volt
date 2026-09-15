// AES key register with a load state machine -- the example that gives
// `trust_level` its first real use (ADR-0052). This is NOT an AES core:
// it is the small, security-relevant piece in front of one. The key
// arrives on a secret input, is held in a secret register and leaves on
// a secret output towards the (external) cipher. The only things allowed
// to reach the public debug side are status bits that say nothing about
// the key -- and every one of them has to go through `declassify` with a
// written reason, which the compiler records as a W3008 warning.
//
// Two domains, one clock: `Debug` carries a trust level but no clock
// port of this module, so it does not open a second clock domain -- the
// signals stay in the `SecureCore` clock (K11), the annotation only
// classifies them. Delete the `declassify` around `busy` and the flow
// becomes an E3009 error; assign `key_r[7:0]` to `debug_out` and the
// compiler names the leak (see README).

domain SecureCore {
    clock       = posedge
    reset       = sync active_high
    trust_level = secret
}

domain Debug {
    clock       = posedge
    reset       = sync active_high
    trust_level = public
}

// Number of cycles the key is held in LOADING before it becomes valid
// (models a key-schedule warm-up; small so the covers stay shallow).
const LOAD_CYCLES : u4 = 3

pub module KeyStore {
    in  clk       : clock     @SecureCore
    in  load      : bool      @Debug      // host request: public
    in  clear     : bool      @Debug      // host request: public
    in  key_in    : bits<128> @SecureCore // key material: secret
    out key       : bits<128> @SecureCore // to the cipher: secret
    out key_valid : bool      @Debug      // status only
    out busy      : bool      @Debug      // status only
    out debug_out : u8        @Debug      // load progress, never key bits

    // State encoding: 0 IDLE, 1 LOADING, 2 READY.
    reg state_r : u2        = 0
    reg key_r   : bits<128> = 0 as bits<128>
    reg cnt_r   : u4        = 0

    // The key output is only claimed valid in READY.
    invariant: key_valid -> state_r == 2
    // LOADING and READY never overlap on the status side.
    invariant: !(busy && key_valid)
    // The warm-up counter never runs past its bound.
    invariant: cnt_r <= LOAD_CYCLES
    // Every state is reachable.
    cover: state_r == 1
    cover: state_r == 2
    cover: state_r == 2 && clear

    on clk {
        match state_r {
            0 => {
                if load {
                    key_r   <= key_in
                    cnt_r   <= 0
                    state_r <= 1
                }
            }
            1 => {
                if cnt_r == LOAD_CYCLES {
                    state_r <= 2
                } else {
                    cnt_r <= cnt_r + 1
                }
            }
            _ => {
                if clear {
                    key_r   <= 0 as bits<128>
                    cnt_r   <= 0
                    state_r <= 0
                }
            }
        }
    }

    key = key_r

    // Deliberate downgrades: each one is a W3008 with its reason.
    busy      = declassify(state_r == 1, "state visibility only")
    key_valid = declassify(state_r == 2, "readiness flag only")
    debug_out = declassify(cnt_r as u8, "load progress counter, not key bits")

    // debug_out = key_r[7:0] as u8  // E3009: secret data flows to a public output
}
