//! `ParseResult.regmaps` — `@mmio` register haritasının yazılım tarafı
//! kopyası (ADR-0053). Parser RTL'e açtığı `RegInfo` listesinden aynı
//! anda dil bağımsız `RegMap` üretir; sürücü/belge üreticileri bunu okur.

use volt_ast::mmio::{FieldKind, RegAccess, RegMap};
use volt_span::FileId;
use volt_syntax::parser::{parse, parse_unit};

const GPIO: &str = r#"
/// 8-bit GPIO block.
/// Second doc line.
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module GpioRegs {
    in  clk        : clock
    in  pin_values : bits<8>
    out pins_out   : bits<8>

    /// Pin direction, 1 = output.
    @reg(offset = 0x00, access = ReadWrite)
    direction : {
        /// One bit per pin.
        pins : bits<8>,
        @reserved : bits<24>,
    }

    @reg(offset = 0x08, access = ReadOnly, volatile)
    input : { pins : bits<8>, @reserved : bits<24> }

    @reg(offset = 0x0C, access = ReadWrite)
    control : {
        enable : bool,
        /// Soft reset pulse.
        reset  : bool @self_clearing,
        @reserved : bits<30>,
    }

    @reg(offset = 0x10, access = ReadOnly, volatile)
    status : { count : u16, expired : bool @w1c, @reserved : bits<15> }

    @reg(offset = 0x14, access = WriteOnly)
    compare : { value : u16, @reserved : bits<16> }

    @reg(offset = 0x18, access = ReadOnly)
    id : { value : u16, @reserved : bits<16> }

    on clk {
        regs.input.pins <= pin_values
        regs.status.count <= regs.status.count + 1
        if regs.status.count == regs.compare.value { regs.status.expired <= true }
    }
    pins_out = regs.direction.pins
}
"#;

fn gpio() -> RegMap {
    let result = parse(FileId(0), GPIO);
    assert!(
        result.diagnostics.is_empty(),
        "temiz ayrışmalı: {:?}",
        result.error_codes()
    );
    assert_eq!(result.regmaps.len(), 1, "bir @mmio modülü → bir harita");
    result.regmaps.into_iter().next().unwrap()
}

#[test]
fn regmap_carries_module_name_base_bus_and_doc() {
    let map = gpio();
    assert_eq!(map.module, "GpioRegs");
    assert_eq!(map.base, 0x4000_0000);
    assert_eq!(map.bus, "AXI4Lite");
    assert_eq!(
        map.doc.as_deref(),
        Some("8-bit GPIO block.\nSecond doc line.")
    );
}

#[test]
fn regmap_lists_registers_in_declaration_order_with_offsets() {
    let map = gpio();
    let names: Vec<&str> = map.registers.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        ["direction", "input", "control", "status", "compare", "id"]
    );
    let offsets: Vec<u64> = map.registers.iter().map(|r| r.offset).collect();
    assert_eq!(offsets, [0x00, 0x08, 0x0C, 0x10, 0x14, 0x18]);
    assert_eq!(map.registers[1].address(map.base), 0x4000_0008);
}

#[test]
fn regmap_records_access_and_volatile() {
    let map = gpio();
    let r = |n: &str| map.registers.iter().find(|r| r.name == n).unwrap();
    assert_eq!(r("direction").access, RegAccess::ReadWrite);
    assert!(!r("direction").volatile);
    assert_eq!(r("input").access, RegAccess::ReadOnly);
    assert!(r("input").volatile);
    assert_eq!(r("compare").access, RegAccess::WriteOnly);
    assert_eq!(r("id").access, RegAccess::ReadOnly);
    assert!(!r("id").volatile, "sabit register volatile değil");
}

#[test]
fn regmap_fields_have_lsb_width_kind_and_reserved_padding() {
    let map = gpio();
    let status = map.registers.iter().find(|r| r.name == "status").unwrap();
    let f: Vec<(&str, u32, u32, FieldKind, bool)> = status
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.lsb, f.width, f.kind, f.reserved))
        .collect();
    assert_eq!(
        f,
        [
            ("count", 0, 16, FieldKind::UInt, false),
            ("expired", 16, 1, FieldKind::Bool, false),
            ("_reserved", 17, 15, FieldKind::Bits, true),
        ]
    );
    assert_eq!(status.fields[1].msb(), 16);
    assert_eq!(status.fields[1].mask(), 0x1_0000);
    assert_eq!(status.read_mask(), 0x1_FFFF, "rezerve bitler maskede yok");
}

#[test]
fn regmap_records_self_clearing_and_w1c_flags() {
    let map = gpio();
    let control = map.registers.iter().find(|r| r.name == "control").unwrap();
    let reset = control.named().find(|f| f.name == "reset").unwrap();
    assert!(reset.self_clearing && !reset.w1c);
    let enable = control.named().find(|f| f.name == "enable").unwrap();
    assert!(!enable.self_clearing && !enable.w1c);
    let status = map.registers.iter().find(|r| r.name == "status").unwrap();
    let expired = status.named().find(|f| f.name == "expired").unwrap();
    assert!(expired.w1c && !expired.self_clearing);
}

#[test]
fn regmap_carries_register_and_field_doc_comments() {
    let map = gpio();
    let direction = map
        .registers
        .iter()
        .find(|r| r.name == "direction")
        .unwrap();
    assert_eq!(direction.doc.as_deref(), Some("Pin direction, 1 = output."));
    assert_eq!(
        direction.fields[0].doc.as_deref(),
        Some("One bit per pin."),
        "alan doc yorumu (ADR-0053 ile eklendi)"
    );
    assert_eq!(direction.fields[1].doc, None, "@reserved doc taşımaz");
    let control = map.registers.iter().find(|r| r.name == "control").unwrap();
    assert_eq!(control.fields[1].doc.as_deref(), Some("Soft reset pulse."));
    assert_eq!(control.fields[0].doc, None);
}

#[test]
fn regmap_is_empty_without_mmio_module() {
    let result = parse(
        FileId(0),
        "module Plain { in clk : clock in a : bool out b : bool\n b = a }",
    );
    assert!(result.diagnostics.is_empty());
    assert!(result.regmaps.is_empty());
}

#[test]
fn regmap_is_not_produced_for_a_module_with_layout_errors() {
    let src = r#"
@mmio(base = 0, bus = AXI4Lite)
module Bad {
    in clk : clock
    @reg(offset = 0x00, access = ReadWrite) a : { x : bool, @reserved : bits<31> }
    @reg(offset = 0x02, access = ReadWrite) b : { y : bool, @reserved : bits<31> }
}
"#;
    let result = parse(FileId(0), src);
    assert!(result.error_codes().contains(&"E0015"));
    // Hatalı register düşer; kalan harita yine üretilir (RTL de üretilir).
    let map = &result.regmaps[0];
    assert_eq!(map.registers.len(), 1);
    assert_eq!(map.registers[0].name, "a");
}

#[test]
fn regmap_from_multi_file_unit_collects_every_mmio_module() {
    let other = r#"
@mmio(base = 0x100, bus = AXI4Lite)
module TimerRegs {
    in clk : clock
    out en : bool
    @reg(offset = 0x00, access = ReadWrite) ctrl : { enable : bool, @reserved : bits<31> }
    en = regs.ctrl.enable
}
"#;
    let result = parse_unit(&[(FileId(0), other), (FileId(1), GPIO)]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.error_codes());
    let names: Vec<&str> = result.regmaps.iter().map(|m| m.module.as_str()).collect();
    assert_eq!(names, ["TimerRegs", "GpioRegs"]);
    assert_eq!(result.regmaps[0].base, 0x100);
}
