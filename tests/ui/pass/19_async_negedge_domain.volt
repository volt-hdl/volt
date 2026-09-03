// Asenkron reset, negedge saat, aktif-düşük
// sv-mapping.md §7 — tüm reset varyantları
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
