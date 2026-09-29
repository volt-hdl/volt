<!-- expect: fail -->
# A plain block with a compile error

```volt
pub module Narrow {
    in  a   : u8
    out low : u4

    low = a
}
```
