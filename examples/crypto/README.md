# Key store — `trust_level`, E3009 and `declassify` (ADR-0052)

A small AES key register with a load state machine. It is deliberately
not an AES core: the point is the trust boundary in front of one. The
key arrives on a secret input, sits in a secret register and leaves on a
secret output; the host side only ever sees status bits, and every one
of them goes through `declassify` with a written reason.

```
KeyStore  (key_store.volt)   in  key_in : bits<128> @SecureCore   secret
                             out key    : bits<128> @SecureCore   secret
                             in  load / clear       @Debug        public
                             out busy / key_valid   @Debug        public, declassified status
                             out debug_out : u8     @Debug        public, declassified counter
```

| File | Lines | Content |
|---|---|---|
| `key_store.volt` | 93 | two domains (`SecureCore` secret, `Debug` public), one clock, 3-state FSM (IDLE / LOADING / READY), three `declassify` calls, 3 invariants + 3 covers |

## The two domains, one clock

```volt
domain SecureCore { clock = posedge, reset = sync active_high, trust_level = secret }
domain Debug      { clock = posedge, reset = sync active_high, trust_level = public }

pub module KeyStore {
    in  clk       : clock     @SecureCore
    in  key_in    : bits<128> @SecureCore
    out debug_out : u8        @Debug
    ...
```

`Debug` carries a trust level but no clock port of this module. Before
ADR-0052 that would have made every `@Debug` signal a separate clock
domain and every assignment into it a CDC error (E3001). Now such an
annotation does not open a clock domain (K11): the signal stays in the
`SecureCore` clock, the annotation only classifies it. `key_in` and
`clk` are `@SecureCore`, so every unannotated register in the module
(`state_r`, `key_r`, `cnt_r`) is secret as well -- the same K2 rule
that assigns them their clock.

## The deliberate leak

Replace the last commented line of the module with a real assignment:

```volt
    debug_out = key_r[7:0] as u8
```

```
error[E3009]: secret data flows to a public output
   ┌─ examples/crypto/key_store.volt:92:5
   │
20 │     trust_level = secret
   │     -------------------- source trust level here
   ·
26 │     trust_level = public
   │     -------------------- destination trust level here
   ·
92 │     debug_out = key_r[7:0] as u8
   │     ^^^^^^^^^   ----- @SecureCore (secret)
   │     │
   │     @Debug (public)
   │
   = reason: information from a higher trust level cannot reach a lower one; this could leak key material (ADR-0052)
   = help: if intentional, use declassify(expr, "reason")
   = for more: volt explain E3009
```

The same happens for anything computed from the key -- a slice, a
parity bit, `if key_r[0] { 1 } else { 0 }`, or a register that was
loaded from it -- and for writes inside an `if` whose condition depends
on the key (implicit flow).

## The sanctioned downgrades

```volt
    busy      = declassify(state_r == 1, "state visibility only")
    key_valid = declassify(state_r == 2, "readiness flag only")
    debug_out = declassify(cnt_r as u8, "load progress counter, not key bits")
```

Each call is a W3008 warning naming the source level and repeating the
reason; the three warnings are the complete list of places where
classified information leaves the secure side:

```
warning[W3008]: deliberate trust downgrade: "state visibility only"
   ┌─ examples/crypto/key_store.volt:88:17
   │
88 │     busy      = declassify(state_r == 1, "state visibility only")
   │                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   │                 │                        │
   │                 │                        reason recorded for review
   │                 @SecureCore (secret) → public here
```

Drop the reason (`declassify(state_r == 1)`) and the compiler stops at
E0016: the justification is part of the syntax, not a comment.

## Building, linting, verifying

```
volt check examples/crypto/key_store.volt                 # 0 errors, 3 × W3008
volt build examples/crypto/key_store.volt                 # build/rtl/KeyStore.sv (65 lines) -- no trace of trust_level or declassify

docker run --rm -v "$PWD/build/rtl:/work" -w /work verilator/verilator:latest \
    --lint-only -Wall --top-module KeyStore KeyStore.sv    # clean

VOLT_SBY=build/sby-docker.cmd volt verify --mode bmc   --depth 12 --engine boolector examples/crypto/key_store.volt   # 6 properties, 1.7 s
VOLT_SBY=build/sby-docker.cmd volt verify --mode prove --depth 3  --engine boolector examples/crypto/key_store.volt   # k-induction, 1.2 s
VOLT_SBY=build/sby-docker.cmd volt verify --mode cover --depth 12 --engine boolector examples/crypto/key_store.volt   # 3/3 reached, 1.2 s
```

Trust is checked at the type level only: the generated SystemVerilog is
byte-for-byte what the same module without `trust_level` and
`declassify` would produce.

## What the compiler does not do

- No automatic "no secret in `debug_out`" contract is generated: that
  property is a two-trace (non-interference) property that an SVA
  assertion on one trace cannot express, and the static check above
  already covers every flow (ADR-0052 §5).
- A submodule without trust annotations is summarised conservatively:
  every output carries the highest level of the instance's classified
  inputs.
- Timing and power side channels (`@constant_time`, `@no_power_leak`)
  are out of scope; they parse and report W0021 (ADR-0048).
