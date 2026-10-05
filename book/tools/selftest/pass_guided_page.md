<!-- expect: pass; guided page -->
# A guided page the checker accepts

<div class="before-you-start">

**Before you start**

- **Folder:** your project folder. **Type this:**

  ```console
  cd ~/volt-projects
  ```

</div>

## Steps

**Type this (Windows, PowerShell):**

```powershell
Get-Clipboard -Raw | Set-Content adder.volt -NoNewline
```

**Create this file:** `adder.volt` ([how](setup.md#create-a-file))

```volt,file=adder.volt
pub module Adder {
    in  a   : u8
    in  b   : u8
    out sum : u9

    sum = a + b
}
```

**Type this:**

```console
volt check adder.volt
```

**You should see:**

```text,output
    Checking adder.volt
```

<div class="box from-sv">

**Coming from SystemVerilog?** Code in a side box needs no label

```systemverilog
assign sum = a + b;
```

</div>
