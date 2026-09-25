//! Ad kuralı ile üreticiler aynı kalır (ADR-0079 §2): parser'ın çarpışma
//! denetimi (E1014) `volt_ast::mmio_names::driver_names` kümesine bakar;
//! üreticiler bu kümeden başka bir kapsam adı yazarsa ya da birini
//! yazmazsa denetim gerçekte çakışan bir adı kaçırır. Bu test üretilen
//! Rust/C metninden tanımlayıcıları çıkarıp kümeyle karşılaştırır.

use std::path::Path;

use volt_ast::mmio::RegMap;
use volt_ast::mmio_names::{driver_names, Lang};
use volt_span::FileId;
use volt_sw_emit::{emit_c, emit_rust, EmitOpts};
use volt_syntax::parser::parse;

fn opts() -> EmitOpts {
    EmitOpts {
        source: "x.volt".to_string(),
        version: "0.1.0".to_string(),
    }
}

/// Her erişim türü, tek/çok alan, @reserved, @self_clearing, @w1c.
const ALL_FORMS: &str = r#"
@mmio(base = 0x0000_0100, bus = AXI4Lite)
module TimerRegs {
    in  clk  : clock
    in  tick : bool
    out irq  : bool

    @reg(offset = 0x00, access = ReadWrite)
    ctrl : { enable : bool, clear : bool @self_clearing, mode : u3, @reserved : bits<27> }
    @reg(offset = 0x04, access = ReadOnly)
    id : { value : u16, @reserved : bits<16> }
    @reg(offset = 0x08, access = WriteOnly)
    compare : { value : u16, @reserved : bits<16> }
    @reg(offset = 0x0C, access = ReadOnly, volatile)
    status : { count : u16, expired : bool @w1c, @reserved : bits<15> }
    @reg(offset = 0x10, access = WriteOnly)
    kick : { go : bool @self_clearing, arm : bool, @reserved : bits<30> }

    on clk {
        if regs.ctrl.clear {
            regs.status.count <= 0
        } else if regs.ctrl.enable && tick {
            regs.status.count <= regs.status.count + 1
        }
        if regs.status.count == regs.compare.value { regs.status.expired <= true }
    }
    irq = regs.status.expired && regs.kick.arm
}
"#;

fn maps() -> Vec<RegMap> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = vec![ALL_FORMS.to_string()];
    for f in [
        "tests/ui/pass/57_mmio_basic.volt",
        "tests/ui/pass/58_mmio_access_control.volt",
        "tests/ui/pass/72_mmio_driver_generation.volt",
    ] {
        sources.push(std::fs::read_to_string(root.join(f)).expect("fixture"));
    }
    sources
        .iter()
        .flat_map(|src| {
            let r = parse(FileId(0), src);
            assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
            r.regmaps
        })
        .collect()
}

/// Rust `impl` öğeleri: `pub const X:`, `fn x(` (her görünürlük).
fn rust_items(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line
            .strip_prefix("pub const ")
            .filter(|r| !r.starts_with("unsafe"))
        {
            out.push(rest.split(':').next().unwrap().to_string());
        } else if let Some(i) = line.find("fn ") {
            let head = &line[..i];
            if ["", "pub ", "pub const unsafe ", "pub unsafe "].contains(&head) {
                out.push(line[i + 3..].split('(').next().unwrap().to_string());
            }
        }
    }
    out
}

/// C dosya kapsamı: `#define X`, `static inline T x(`.
fn c_items(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("#define ") {
            out.push(rest.split_whitespace().next().unwrap().to_string());
        } else if line.starts_with("static inline ") {
            let head = line.split('(').next().unwrap();
            out.push(head.split_whitespace().last().unwrap().to_string());
        }
    }
    out
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn naming_rule_matches_every_generated_identifier() {
    let maps = maps();
    assert!(maps.len() >= 4);
    for map in &maps {
        let names = driver_names(map);
        let of = |lang| {
            sorted(
                names
                    .iter()
                    .filter(|n| n.lang == lang)
                    .map(|n| n.ident.clone())
                    .collect(),
            )
        };
        assert_eq!(
            sorted(rust_items(&emit_rust(map, &opts()))),
            of(Lang::Rust),
            "{} Rust",
            map.module
        );
        assert_eq!(
            sorted(c_items(&emit_c(map, &opts()))),
            of(Lang::C),
            "{} C",
            map.module
        );
    }
}

/// Alan `word` ise okuma-değiştirme-yazma yereli başka ad alır: Rust'ta
/// `let word` parametreyi gölgeleyip eski sözcüğü yazardı (derlenir!),
/// C'de aynı kapsamda yeniden bildirimdi.
#[test]
fn word_field_setter_does_not_shadow_its_parameter() {
    let src = r#"
@mmio(base = 0x0, bus = AXI4Lite)
module W {
    in clk : clock
    out y : bool
    @reg(offset = 0x00, access = ReadWrite)
    r : { word : u8, flag : bool, @reserved : bits<23> }
    y = regs.r.flag
}
"#;
    let r = parse(FileId(0), src);
    assert!(r.diagnostics.is_empty(), "{:?}", r.error_codes());
    let rs = emit_rust(&r.regmaps[0], &opts());
    assert!(
        rs.contains("pub fn set_r_word(&mut self, word: u8) {"),
        "{rs}"
    );
    assert!(
        rs.contains("let current = self.read(Self::R_OFFSET)"),
        "{rs}"
    );
    assert!(
        rs.contains("current | (u32::from(word) & 0x0000_00FF)"),
        "{rs}"
    );
    let h = emit_c(&r.regmaps[0], &opts());
    assert!(
        h.contains("uint32_t current = *(volatile uint32_t *)W_R & "),
        "{h}"
    );
    // Diğer alanlar eski kalıbı korur (çıktı değişmez).
    assert!(rs.contains("let word = self.read(Self::R_OFFSET)"), "{rs}");
}
