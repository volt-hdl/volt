// Open-drain port (ADR-0051): `opendrain sda : bool` is a pad that the
// module only ever pulls LOW; the pull-up is external and several
// devices share the line (wired-AND). The drive state is a register the
// compiler synthesises (sda_drive_low); the module changes it only with
// sda.drive_low() / sda.release() inside an 'on' block, reads the line
// with sda.read() and names the state in contracts as sda.released /
// sda.driving. The generated SV is `assign sda = sda_drive_low ? 1'b0 : 1'bz`.
// The line is read through sync(): its other end is an external device.
module OpenDrainPad {
    in  clk    : clock
    in  pull   : bool
    opendrain sda : bool
    out level  : bool
    out held   : bool

    // Driver intent, not the z value: released == !sda_drive_low.
    invariant: !pull_r -> sda.released
    invariant: sda.driving -> pull_r
    cover: sda.driving

    wire sda_s : bool
    sda_s = sync(sda.read(), clk)

    reg pull_r  : bool = false
    reg level_r : bool = true

    on clk {
        pull_r  <= pull
        level_r <= sda_s
        if pull {
            sda.drive_low()
        } else {
            sda.release()
        }
    }

    level = level_r
    held  = sda.driving
}

// Two open-drain devices on one bus: the parent declares a plain wire,
// binds it to both pads and reads the resolved level. The wire becomes a
// `tri1` net in SV (pull-up + wired-AND).
module OpenDrainBus {
    in  clk  : clock
    in  a    : bool
    in  b    : bool
    out line : bool
    out lvl_a : bool

    wire sda_bus : bool

    let pa = OpenDrainPad { clk, pull: a, sda: sda_bus }
    let pb = OpenDrainPad { clk, pull: b, sda: sda_bus }

    line  = sda_bus
    lvl_a = pa.level
}
