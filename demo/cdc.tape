# The README demo: a clock domain crossing error and its fix, with the
# code of the book's chapter "A clock domain crossing error"
# (demo/crossing.volt -> demo/crossing_fixed.volt).
# Recorded by demo/record.sh; see book/CONTRIBUTING-BOOK.md,
# "The README demo".

Output /out/cdc-demo.gif

Require volt
Require nano

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 820
Set Padding 20
Set Framerate 24
Set TypingSpeed 45ms

Hide
Type "mkdir -p /tmp/cdc && cp /demo/crossing.volt /tmp/cdc/ && cd /tmp/cdc && clear"
Enter
Show

# 1. The design passes a register from the fast clock to the slow one.
Type "volt check crossing.volt"
Sleep 500ms
Enter
Sleep 10s

# 2. Do what the help line says: add the sync() line above the
#    `on slow_clk` block (line 25) and read the synchronized name.
Type "nano +25 crossing.volt"
Enter
Sleep 2s
Type "    let pressed_r_sync = sync(pressed_r, slow_clk)"
Enter
Enter
Sleep 1s
Down 2
End
Type "_sync"
Sleep 3s
Ctrl+O
Sleep 500ms
Enter
Sleep 500ms
Ctrl+X
Sleep 500ms

# 3. Check again.
Hide
Type "clear"
Enter
Show
Type "volt check crossing.volt"
Sleep 500ms
Enter
Sleep 6s

# record.sh compares the edited file with demo/crossing_fixed.volt.
Hide
Type "cp crossing.volt /out/crossing_after.volt"
Enter
Sleep 500ms
