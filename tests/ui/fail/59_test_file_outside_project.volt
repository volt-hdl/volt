//~ E8507
// Test data is read relative to the test file and may not leave the
// project (ADR-0058): '..\..' climbs above the project root, so the
// result would depend on the machine the test runs on.

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

test "reads a file outside the project" {
    let dut = Rom { };
    let image = read_hex("..\\..\\..\\..\\secrets.hex");
    //~^ ERROR test data path leaves the project
    load(dut.mem, image);
}
