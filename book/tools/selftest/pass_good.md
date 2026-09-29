<!-- expect: pass -->
# A chapter the checker accepts

A block that compiles:

```volt,file=adder.volt
pub module Adder {
    in  a   : u8
    in  b   : u8
    out sum : u9

    sum = a + b
}
```

A block that fails with the code it declares:

```volt,should_fail=E2001
pub module Narrow {
    in  a   : u8
    out low : u4

    low = a
}
```

A block that warns with the code it declares:

```volt,should_warn=W2010
pub module Cast {
    in  a   : u8
    out low : u4

    low = a as u4
}
```

Other languages are not checked:

```text
this is not Volt
```
