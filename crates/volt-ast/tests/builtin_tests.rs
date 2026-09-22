//! Yerleşik primitif tablosu birim testleri (ADR-0065 R5').

use volt_ast::builtin::{BuiltinPrim, PortKind};

const ALL: [&str; 12] = [
    "AsyncFifo",
    "HandshakeSync",
    "PulseSync",
    "SyncFifo",
    "Ram",
    "DualPortRam",
    "Counter",
    "ShiftRegister",
    "RoundRobinArbiter",
    "PriorityArbiter",
    "EdgeDetect",
    "AsyncDualPortRam",
];

#[test]
fn only_the_ram_write_side_and_the_priority_arbiter_drive_no_reset() {
    // sv-emit: `emit_async_dual_port_ram` yazma tarafı
    // `builtin_always_ff_no_reset`, `emit_priority_arbiter` yalnız `assign`.
    let mut reset_free = Vec::new();
    for name in ALL {
        let prim = BuiltinPrim::from_name(name).expect("bilinen primitif");
        for port in prim.ports().iter().filter(|p| p.kind == PortKind::Clock) {
            if !prim.clock_resets_flops(port.name) {
                reset_free.push(format!("{name}.{}", port.name));
            }
        }
    }
    assert_eq!(
        reset_free,
        ["PriorityArbiter.clk", "AsyncDualPortRam.wr_clk"]
    );
}

#[test]
fn unknown_port_name_is_treated_as_resetting() {
    // Tutucu varsayılan: tabloda olmayan ad reset örnekler sayılır.
    assert!(BuiltinPrim::AsyncDualPortRam.clock_resets_flops("rd_clk"));
    assert!(BuiltinPrim::AsyncDualPortRam.clock_resets_flops("nope"));
}
