<!-- expect: fail -->
# A verify attribute with a mode volt verify does not have

```volt,verify=induct
pub module Hold {
    in  clk : clock
    in  d   : bool
    out q   : bool

    reg q_r : bool = false
    on clk {
        q_r <= d
    }
    q = q_r
}
```
