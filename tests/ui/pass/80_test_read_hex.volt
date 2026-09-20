// read_hex + load (ADR-0058): the test reads a $readmemh file that
// sits next to it and writes it into a memory of the design. Loaded
// memories model initialised storage: they survive reset().

module Rom {
    in  clk   : clock
    in  addr  : u3
    in  we    : bool
    in  wdata : u32
    out data  : u32

    reg mem    : [u32; 8] = [0; 8]
    reg data_r : u32 = 0

    on clk {
        if we {
            mem[addr] <= wdata
        }
        data_r <= mem[addr]
    }

    data = data_r
}

test "rom holds the hex file" {
    let dut = Rom { };
    let image = read_hex("80_test_read_hex.hex");
    load(dut.mem, image);
    reset();
    for i in 0..len(image) {
        dut.addr = i;
        step(1);
        assert_eq(dut.data, image[i]);
    }
    // The file holds six words; the rest of the memory keeps its reset value.
    assert_eq(len(image), 6);
    dut.addr = 7;
    step(1);
    assert_eq(dut.data, 0);
}
