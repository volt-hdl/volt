<!-- expect: fail -->
# A misspelt attribute

```volt,shuold_fail=E2001
pub module Wide {
    in  a   : u8
    out sum : u9

    sum = a + a
}
```
