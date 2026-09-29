<!-- expect: fail -->
# should_fail names a different code than the one the compiler reports

```volt,should_fail=E3001
pub module Narrow {
    in  a   : u8
    out low : u4

    low = a
}
```
