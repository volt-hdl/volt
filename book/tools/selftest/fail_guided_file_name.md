<!-- expect: fail; guided page -->
# A file block whose label names no file

<div class="before-you-start">

**Before you start**

Nothing to open.

</div>

**Create this file:** with this content

```volt,file=adder.volt
pub module Adder {
    in  a   : u8
    out b   : u8

    b = a
}
```
