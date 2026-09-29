<!-- expect: fail -->
# from= names a file the block does not match

```volt,from=templates/minimal/counter.volt
pub module NotTheTemplate {
    in  a : bool
    out b : bool

    b = a
}
```
