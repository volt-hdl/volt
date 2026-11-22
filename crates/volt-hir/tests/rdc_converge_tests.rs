//! R6 yakınsama tanısının (E3003) iki yeri (ADR-0065; #371): ikinci
//! senkronizör birincil, ilki ikincil etiket. Modülün kendi zinciri
//! ilkse ikincil etiket o saatin portundadır; ilk yer bir örnekse onun
//! bağlamasındadır. Saat bir örneğin çıkışı olabilir (#371).

use volt_diagnostics::Diagnostic;
use volt_syntax::{parse, FileId};

const CHILD: &str = "\
domain Core { clock = posedge, reset = async active_low }

extern module Pll {
    in  clk_in : clock @Core
    out clk_o  : clock @Core
}

module Child {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q     : u8
    reg r : u8 = 0
    on clk { r <= d }
    q = r
}
";

fn e3003(src: &str) -> Diagnostic {
    let parsed = parse(FileId(0), src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let result = volt_hir::analyze(&parsed.ast);
    let found: Vec<&Diagnostic> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E3003")
        .collect();
    assert_eq!(found.len(), 1, "{:?}", result.error_codes());
    found[0].clone()
}

/// İkincil etiketin kaynak metni.
fn secondary_text<'a>(src: &'a str, d: &Diagnostic) -> &'a str {
    let s = d
        .spans
        .iter()
        .find(|s| !s.primary)
        .unwrap_or_else(|| panic!("ikincil etiket yok: {d:?}"));
    &src[s.span.start as usize..s.span.end as usize]
}

#[test]
fn a_local_chain_first_labels_the_clock_port_of_the_module() {
    // Top `clk`'te `rst_n`'yi kendisi senkronlar (reg r), çocuk da aynı
    // saatte: ikincil etiket Top'un `clk` portu (ad metni `clk`), Top'un
    // ikinci saat portu `clk2` değil.
    let src = format!(
        "{CHILD}
module Top {{
    in  clk2  : clock @Core
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q     : u8
    out q2    : u8
    out q3    : u8
    reg r : u8 = 0
    on clk {{ r <= d }}
    reg r2 : u8 = 0
    on clk2 {{ r2 <= d }}
    q = r
    q3 = r2
    let u = Child {{ clk: clk, rst_n: rst_n, d: d }}
    q2 = u.q
}}
"
    );
    let d = e3003(&src);
    assert!(d.message.contains("'clk'"), "{}", d.message);
    assert_eq!(secondary_text(&src, &d), "clk");
    let at = d
        .spans
        .iter()
        .find(|s| !s.primary)
        .expect("ikincil")
        .span
        .start as usize;
    let top = src.find("module Top").expect("Top");
    let clk_port = top
        + src[top..]
            .find("    in  clk   : clock @Core")
            .expect("port")
        + 8;
    assert_eq!(at, clk_port, "ikincil etiket Top'un clk portunda olmalı");
}

#[test]
fn two_instances_on_an_instance_clock_name_the_output_and_both_bindings() {
    let src = format!(
        "{CHILD}
module Top {{
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q1    : u8
    out q2    : u8
    let p = Pll {{ clk_in: clk }}
    let a = Child {{ clk: p.clk_o, rst_n: rst_n, d: d }}
    let b = Child {{ clk: p.clk_o, rst_n: rst_n, d: d }}
    q1 = a.q
    q2 = b.q
}}
"
    );
    let d = e3003(&src);
    assert!(d.message.contains("'p.clk_o'"), "{}", d.message);
    assert!(
        d.message.contains("'a'") && d.message.contains("'b'"),
        "{}",
        d.message
    );
    assert_eq!(secondary_text(&src, &d), "rst_n: rst_n");
}

#[test]
fn two_instances_on_different_instance_outputs_do_not_converge() {
    // Aynı PLL'in iki ayrı çıkışı iki ayrı saattir; iki örnek de olsa iki
    // ayrı (saat, ham reset) çifti: E3003 yok.
    let src = CHILD.replace(
        "    out clk_o  : clock @Core\n",
        "    out clk_o  : clock @Core\n    out clk_p  : clock @Core\n",
    ) + "
module Top {
    in  clk   : clock @Core
    in  rst_n : reset(async, active_low)
    in  d     : u8
    out q1    : u8
    out q2    : u8
    let p = Pll { clk_in: clk }
    let a = Child { clk: p.clk_o, rst_n: rst_n, d: d }
    let b = Child { clk: p.clk_p, rst_n: rst_n, d: d }
    q1 = a.q
    q2 = b.q
}
";
    let parsed = parse(FileId(0), &src);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.error_codes());
    let result = volt_hir::analyze(&parsed.ast);
    assert!(
        !result.error_codes().contains(&"E3003"),
        "{:?}",
        result.error_codes()
    );
}
