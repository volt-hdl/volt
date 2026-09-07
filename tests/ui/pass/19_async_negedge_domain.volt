// Async reset, negedge clock, active-low
// sv-mapping.md sec. 7 -- all reset variants
domain UsbDomain {
    clock = negedge
    reset = async active_low
}

module AsyncResetModule {
    in  usb_clk : clock @UsbDomain
    in  data    : u8    @UsbDomain
    out result  : u8    @UsbDomain

    reg buffer : u8 = 0

    on usb_clk {
        buffer <= data
    }

    result = buffer
}
