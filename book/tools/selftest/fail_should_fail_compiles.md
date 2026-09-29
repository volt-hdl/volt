<!-- expect: fail -->
# should_fail on a block that compiles

```volt,should_fail=E2001
pub module Wide {
    in  a   : u8
    out sum : u9

    sum = a + a
}
```
