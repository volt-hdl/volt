<!-- expect: fail -->
# A warning without should_warn

```volt
pub module Cast {
    in  a   : u8
    out low : u4

    low = a as u4
}
```
