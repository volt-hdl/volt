//! SVA üretimi testleri (F4a ADIM 2-3).
//!
//! Kontratlar SystemVerilog assertion'larına çevrilir: invariant/ensures
//! /assert → assert property, requires/assume → assume property,
//! cover → cover property. Varsayılan çıktı ayrı .sva dosyası + bind;
//! --sva=inline modunda SV modül gövdesine gömülür.

use volt_span::FileId;
use volt_sv_emit::{emit, emit_full, EmitOutput, SvaMode};

fn full(src: &str, mode: SvaMode) -> EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = emit_full(&parsed.ast, "test.volt", src, mode);
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| d.severity == volt_diagnostics::Severity::Error),
        "emit hatasız olmalı: {:?}",
        out.diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    out
}

/// Tek kontratlı tipik modül: tek saat, örtük domain (posedge + sync rst).
fn uart(contracts: &str) -> String {
    format!(
        "module Uart {{\n    in  clk   : clock\n    in  speed : u8\n    \
         in  start : bool\n    out busy  : bool\n\n{contracts}\n    \
         reg busy_r : bool = false\n\n    on clk {{\n        \
         busy_r <= start\n    }}\n\n    busy = busy_r\n}}\n"
    )
}

fn single_sva(src: &str) -> String {
    let out = full(src, SvaMode::Separate);
    assert_eq!(out.sva_files.len(), 1, "tek modül tek .sva üretmeli");
    out.sva_files[0].content.clone()
}

// ═══ Kontrat türü → SVA yapısı ════════════════════════════════════

#[test]
fn invariant_becomes_assert_property() {
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("property inv_0;"), "{sva}");
    assert!(sva.contains("assert property (inv_0);"), "{sva}");
    assert!(sva.contains("!(start && busy)"), "{sva}");
}

#[test]
fn requires_becomes_assume_property() {
    let sva = single_sva(&uart("    requires: speed <= 2\n"));
    assert!(sva.contains("property req_0;"), "{sva}");
    assert!(sva.contains("assume property (req_0);"), "{sva}");
}

#[test]
fn cover_becomes_cover_property() {
    let sva = single_sva(&uart("    cover: speed == 2 && start\n"));
    assert!(sva.contains("property cov_0;"), "{sva}");
    assert!(sva.contains("cover property (cov_0);"), "{sva}");
}

#[test]
fn ensures_becomes_assert_with_implication_sugar() {
    // Volt'ta '->' yok; üst düzey '!a || b' deseni 'a |-> b' olur.
    let sva = single_sva(&uart("    ensures: !start || busy\n"));
    assert!(sva.contains("property ens_0;"), "{sva}");
    assert!(sva.contains("assert property (ens_0);"), "{sva}");
    assert!(sva.contains("start |-> busy"), "{sva}");
}

// ═══ İmplikasyon operatörü → |-> (ADR-0034) ═══════════════════════

#[test]
fn invariant_implication_becomes_overlapped_implication() {
    let sva = single_sva(&uart("    invariant: start -> busy\n"));
    assert!(sva.contains("property inv_0;"), "{sva}");
    assert!(sva.contains("start |-> busy"), "{sva}");
}

#[test]
fn ensures_implication_operator_becomes_overlapped_implication() {
    let sva = single_sva(&uart("    ensures: start -> busy\n"));
    assert!(sva.contains("property ens_0;"), "{sva}");
    assert!(sva.contains("start |-> busy"), "{sva}");
}

#[test]
fn requires_implication_becomes_assume_with_overlapped_implication() {
    let sva = single_sva(&uart("    requires: start -> speed == 2\n"));
    assert!(sva.contains("assume property (req_0);"), "{sva}");
    assert!(sva.contains("start |-> speed == 8'd2"), "{sva}");
}

#[test]
fn nested_implication_rhs_expands_to_boolean_form() {
    // Sağ birleşme: a -> (b -> c); yalnız üst düzey |-> olur,
    // iç implikasyon boolean açılımıyla (!b || c) yazılır.
    let sva = single_sva(&uart("    invariant: start -> busy -> !start\n"));
    assert!(sva.contains("start |-> !busy || !start"), "{sva}");
    assert_eq!(sva.matches("|->").count(), 1, "{sva}");
}

#[test]
fn implication_lhs_with_negation() {
    let sva = single_sva(&uart("    invariant: !busy -> !start\n"));
    assert!(sva.contains("!busy |-> !start"), "{sva}");
}

#[test]
fn immediate_mode_implication_expands_to_boolean_form() {
    // Yosys '|->' bilmez; tek döngüde '!a || b' eşdeğerdir.
    let out = full(&uart("    invariant: start -> busy\n"), SvaMode::Immediate);
    assert!(out.sv.contains("!start || busy"), "{}", out.sv);
    assert!(!out.sv.contains("|->"), "{}", out.sv);
}

#[test]
fn assume_contract_becomes_assume_property() {
    let sva = single_sva(&uart("    assume: speed == 0\n"));
    assert!(sva.contains("property asm_0;"), "{sva}");
    assert!(sva.contains("assume property (asm_0);"), "{sva}");
}

#[test]
fn assert_contract_becomes_assert_property() {
    let sva = single_sva(&uart("    assert: start || !busy\n"));
    assert!(sva.contains("property ast_0;"), "{sva}");
    assert!(sva.contains("assert property (ast_0);"), "{sva}");
}

#[test]
fn same_kind_contracts_get_sequential_names() {
    let sva = single_sva(&uart(
        "    invariant: !(start && busy)\n    invariant: !(busy && speed == 0)\n",
    ));
    assert!(sva.contains("property inv_0;"), "{sva}");
    assert!(sva.contains("property inv_1;"), "{sva}");
}

// ═══ Saat, reset ve kaynak satırı ═════════════════════════════════

#[test]
fn clock_and_reset_come_from_implicit_domain() {
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("@(posedge clk) disable iff (rst)"), "{sva}");
}

#[test]
fn source_line_comment_is_mandatory() {
    // uart() şablonunda kontrat satırı 7'dedir.
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("// invariant from test.volt:7"), "{sva}");
}

#[test]
fn active_low_async_reset_becomes_disable_iff_not_rst_n() {
    let src = "domain Sys { clock = posedge, reset = async active_low }\n\n\
               module M {\n    in  clk : clock @Sys\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(sva.contains("disable iff (!rst_n)"), "{sva}");
}

#[test]
fn no_reset_domain_omits_disable_iff() {
    let src = "domain Free { clock = posedge, reset = none }\n\n\
               module M {\n    in  clk : clock @Free\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(!sva.contains("disable iff"), "{sva}");
    assert!(sva.contains("@(posedge clk)"), "{sva}");
}

#[test]
fn negedge_domain_clocks_properties_on_negedge() {
    let src = "domain Neg { clock = negedge, reset = sync active_high }\n\n\
               module M {\n    in  clk : clock @Neg\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(sva.contains("@(negedge clk)"), "{sva}");
}

// ═══ Ayrı dosya biçimi (bind) ═════════════════════════════════════

#[test]
fn separate_file_wraps_checker_module_with_bind() {
    let out = full(
        &uart("    invariant: !(start && busy)\n"),
        SvaMode::Separate,
    );
    let file = &out.sva_files[0];
    assert_eq!(file.module_name, "Uart");
    assert_eq!(file.checker_name, "uart_sva");
    assert!(
        file.content.contains("module uart_sva ("),
        "{}",
        file.content
    );
    assert!(
        file.content.contains("bind Uart uart_sva sva_inst (.*);"),
        "{}",
        file.content
    );
}

#[test]
fn checker_ports_carry_referenced_signal_widths() {
    let sva = single_sva(&uart("    requires: speed <= 2\n"));
    assert!(sva.contains("input logic [7:0] speed"), "{sva}");
    assert!(sva.contains("input logic"), "{sva}");
}

#[test]
fn checker_ports_include_registers_used_by_invariant() {
    let sva = single_sva(&uart("    invariant: !(busy_r && start)\n"));
    assert!(sva.contains("busy_r"), "{sva}");
    assert!(sva.contains("bind Uart"), "{sva}");
}

#[test]
fn module_without_contracts_produces_no_sva_file() {
    let out = full(
        "module Plain {\n    in  clk : clock\n    in  a : bool\n    out q : bool\n\n    q = a\n}\n",
        SvaMode::Separate,
    );
    assert!(out.sva_files.is_empty());
}

#[test]
fn clockless_module_contracts_are_skipped() {
    // Formel araçlar saat ister; saatsiz modülde SVA üretilmez.
    let out = full(
        "module Comb {\n    in  a : bool\n    out q : bool\n\n    \
         invariant: !(a && q)\n\n    q = a\n}\n",
        SvaMode::Separate,
    );
    assert!(out.sva_files.is_empty());
}

// ═══ Inline mod ═══════════════════════════════════════════════════

#[test]
fn inline_mode_embeds_properties_in_module_body() {
    let out = full(&uart("    invariant: !(start && busy)\n"), SvaMode::Inline);
    assert!(out.sva_files.is_empty(), "inline modda ayrı dosya yok");
    assert!(out.sv.contains("property inv_0;"), "{}", out.sv);
    assert!(out.sv.contains("assert property (inv_0);"), "{}", out.sv);
    assert!(
        !out.sv.contains("bind "),
        "inline modda bind yok: {}",
        out.sv
    );
    // Özellik bloğu modülün İÇİNDE olmalı.
    let prop = out.sv.find("property inv_0;").unwrap();
    let end = out.sv.find("endmodule").unwrap();
    assert!(prop < end, "SVA bloğu endmodule'den önce olmalı");
}

// ═══ F4b: formal başlangıç varsayımı ve property kimlikleri ═══════

#[test]
fn reset_domain_gets_initial_reset_assumption() {
    // BMC başlangıç durumu kısıtsızdır; ilk döngüde reset varsayılmazsa
    // çözücü sıfırlanmamış register'lı sahte karşı örnek üretir.
    let sva = single_sva(&uart("    invariant: !(start && busy)\n"));
    assert!(sva.contains("initial assume (rst);"), "{sva}");
}

#[test]
fn no_reset_domain_omits_initial_assumption() {
    let src = "domain Free { clock = posedge, reset = none }\n\n\
               module M {\n    in  clk : clock @Free\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let sva = single_sva(src);
    assert!(!sva.contains("initial assume"), "{sva}");
}

#[test]
fn inline_mode_also_assumes_reset_initially() {
    let out = full(&uart("    invariant: !(start && busy)\n"), SvaMode::Inline);
    assert!(out.sv.contains("initial assume (rst);"), "{}", out.sv);
}

#[test]
fn immediate_mode_uses_yosys_compatible_assertions() {
    // Yosys read_verilog 'property/endproperty' bloklarını ayrıştıramaz
    // (TOK_PROPERTY) — verify akışı immediate assertion üretir.
    let out = full(
        &uart("    invariant: !(start && busy)\n"),
        SvaMode::Immediate,
    );
    assert!(out.sva_files.is_empty(), "immediate modda ayrı dosya yok");
    assert!(out.sv.contains("always @(posedge clk)"), "{}", out.sv);
    assert!(out.sv.contains("// volt:inv_0"), "{}", out.sv);
    assert!(out.sv.contains("if (!(rst)) assert ("), "{}", out.sv);
    assert!(out.sv.contains("initial assume (rst);"), "{}", out.sv);
    assert!(
        !out.sv.contains("property"),
        "immediate modda property bloğu olmamalı: {}",
        out.sv
    );
}

#[test]
fn immediate_mode_ensures_stays_boolean_implication() {
    // Tek döngülük 'a |-> b' ile '!a || b' eşdeğer; Yosys '|->' bilmez.
    let out = full(&uart("    ensures: !start || busy\n"), SvaMode::Immediate);
    assert!(out.sv.contains("// volt:ens_0"), "{}", out.sv);
    assert!(out.sv.contains("!start || busy"), "{}", out.sv);
    assert!(!out.sv.contains("|->"), "{}", out.sv);
}

#[test]
fn immediate_mode_no_reset_domain_asserts_unconditionally() {
    let src = "domain Free { clock = posedge, reset = none }\n\n\
               module M {\n    in  clk : clock @Free\n    in  a : bool\n    out q : bool\n\n    \
               invariant: !(a && q)\n\n    q = a\n}\n";
    let out = full(src, SvaMode::Immediate);
    assert!(!out.sv.contains("initial assume"), "{}", out.sv);
    assert!(!out.sv.contains("if (!("), "{}", out.sv);
    assert!(out.sv.contains("assert ("), "{}", out.sv);
}

#[test]
fn immediate_mode_records_sva_props() {
    let out = full(
        &uart("    invariant: !(start && busy)\n"),
        SvaMode::Immediate,
    );
    assert_eq!(out.sva_props.len(), 1);
    assert_eq!(out.sva_props[0].name, "inv_0");
}

#[test]
fn sva_props_map_property_names_to_contracts() {
    let out = full(
        &uart("    requires: speed <= 2\n    invariant: !(start && busy)\n"),
        SvaMode::Inline,
    );
    assert_eq!(out.sva_props.len(), 2, "{:?}", out.sva_props);
    let req = &out.sva_props[0];
    assert_eq!(req.module_name, "Uart");
    assert_eq!(req.name, "req_0");
    assert_eq!(req.keyword, "requires");
    let inv = &out.sva_props[1];
    assert_eq!(inv.name, "inv_0");
    assert_eq!(inv.keyword, "invariant");
    // Span kontrat İFADESİNİ göstermeli (E5001 tanısının konumu).
    assert!(inv.span.end > inv.span.start);
}

#[test]
fn sva_props_empty_when_mode_is_none() {
    let out = full(&uart("    invariant: !(start && busy)\n"), SvaMode::None);
    assert!(out.sva_props.is_empty());
}

#[test]
fn plain_emit_output_is_unchanged() {
    let parsed = volt_syntax::parser::parse(FileId(0), &uart("    invariant: !(start && busy)\n"));
    let result = emit(&parsed.ast, "test.volt");
    assert!(!result.sv.contains("property"), "emit() SVA içermemeli");
    assert!(!result.sv.contains("bind "), "emit() bind içermemeli");
}

// ═══ Alt örnek yükümlülükleri (ADR-0097) ═════════════════════════

/// `requires` modül kendi görevinde doğrulanırken varsayımdır; modül
/// başka bir modülün örneğiyken (görevin tepesi değilken) onu süren üst
/// modülün yükümlülüğüdür. Ayrım `.sby`'nin görev başına tanımladığı
/// `VOLT_SUB_<modül>` makrosuyla yapılır; makro yoksa (elle sby, eski
/// akış) varsayım kalır.
#[test]
fn immediate_requires_is_asserted_when_the_module_is_a_sub_instance() {
    let out = full(&uart("    requires: speed <= 2\n"), SvaMode::Immediate);
    let expected = "    always @(posedge clk)\n\
                    `ifdef VOLT_SUB_Uart\n        \
                    if (!(rst)) assert (speed <= 8'd2); // volt:req_0\n\
                    `else\n        \
                    if (!(rst)) assume (speed <= 8'd2); // volt:req_0\n\
                    `endif\n";
    assert!(out.sv.contains(expected), "{}", out.sv);
}

#[test]
fn immediate_assume_contract_is_also_the_parents_obligation() {
    let out = full(&uart("    assume: speed == 0\n"), SvaMode::Immediate);
    assert!(out.sv.contains("`ifdef VOLT_SUB_Uart\n"), "{}", out.sv);
    assert!(
        out.sv
            .contains("if (!(rst)) assert (speed == 8'd0); // volt:asm_0\n`else"),
        "{}",
        out.sv
    );
}

#[test]
fn immediate_assertions_do_not_switch_by_context() {
    let out = full(
        &uart("    invariant: !(start && busy)\n    ensures: !start || busy\n"),
        SvaMode::Immediate,
    );
    assert!(!out.sv.contains("`ifdef"), "{}", out.sv);
}

#[test]
fn sub_instance_macro_is_shared_with_the_driver() {
    assert_eq!(volt_sv_emit::sub_instance_macro("Uart"), "VOLT_SUB_Uart");
}

/// `--emit=sva` (ticari araçlar) `volt verify` ile aynı makroyu kullanır:
/// modül tek başına doğrulanırken (makro tanımsız) `requires` varsayım,
/// üst bağlamda (makro tanımlı) üst modülün yükümlülüğü.
#[test]
fn separate_mode_requires_switches_on_the_sub_instance_macro() {
    let sva = single_sva(&uart("    requires: speed <= 2\n"));
    let expected = "    endproperty\n\
                    `ifdef VOLT_SUB_Uart\n    \
                    assert property (req_0);\n\
                    `else\n    \
                    assume property (req_0);\n\
                    `endif\n";
    assert!(sva.contains(expected), "{sva}");
}

#[test]
fn inline_mode_assume_switches_on_the_sub_instance_macro() {
    let out = full(&uart("    assume: speed == 0\n"), SvaMode::Inline);
    let expected = "`ifdef VOLT_SUB_Uart\n    \
                    assert property (asm_0);\n\
                    `else\n    \
                    assume property (asm_0);\n\
                    `endif\n";
    assert!(out.sv.contains(expected), "{}", out.sv);
}

#[test]
fn concurrent_assertions_do_not_switch_by_context() {
    let sva = single_sva(&uart("    invariant: !(start && busy)\n    cover: busy\n"));
    assert!(!sva.contains("`ifdef"), "{sva}");
    assert!(
        !sva.contains("VOLT_SUB_"),
        "kural notu yalnız yükümlülükte: {sva}"
    );
}

/// Dosya başı notu: makronun adı ve iki doğrulama biçimi (ticari araçta
/// `+define+`, Yosys'te `read -define`).
#[test]
fn separate_sva_file_header_explains_the_macro() {
    let sva = single_sva(&uart("    requires: speed <= 2\n"));
    let expected = "// Contract obligations (ADR-0097) of module Uart\n\
                    //   Its requires/assume properties switch on the macro VOLT_SUB_Uart\n\
                    //   Verified on its own (the module is the formal top): leave the\n\
                    //     macro undefined; requires/assume are assumptions on its inputs.\n\
                    //   Verified inside a parent (the parent is the formal top): define\n\
                    //     the macro; requires/assume become assertions the parent must meet.\n\
                    //     vlog/vcs/xrun: +define+VOLT_SUB_Uart\n\
                    //     Yosys:         read -define VOLT_SUB_Uart\n";
    let note = sva.find(expected).expect("not");
    let checker = sva.find("module uart_sva (").expect("kontrol modülü");
    assert!(note < checker, "not dosyanın başında: {sva}");
}

/// Uzun modül adı metin satırlarını kırmaz: adlar satır sonundadır.
#[test]
fn obligation_notes_keep_text_lines_short_for_long_module_names() {
    let src = uart("    requires: speed <= 2\n").replace("Uart", "AVeryLongPeripheralName");
    let sva = single_sva(&src);
    for line in sva.lines().filter(|l| l.starts_with("// ")) {
        let has_name = line.contains("AVeryLongPeripheralName");
        assert!(has_name || line.len() <= 80, "uzun satır: {line:?}");
    }
}

const HIERARCHY: &str = "module Leaf {\n    in clk : clock\n    in x : u8\n    \
                         out y : u8\n    requires: x < 10\n    y = x\n}\n\n\
                         module Mid {\n    in clk : clock\n    in a : u8\n    \
                         out b : u8\n    assume: a < 5\n    \
                         let l = Leaf { clk: clk, x: a }\n    b = l.y\n}\n\n\
                         module Top {\n    in clk : clock\n    in p : u8\n    \
                         out q : u8\n    let m = Mid { clk: clk, a: p }\n    \
                         q = m.b\n}\n";

fn module_sv<'a>(out: &'a EmitOutput, name: &str) -> &'a str {
    &out.modules
        .iter()
        .find(|m| m.name == name)
        .expect("modül")
        .sv
}

/// Formal tepe notu: altındaki yükümlülüklü örneklerin makroları —
/// `volt verify`'ın o görevde tanımladığı küme. Kontratsız tepe modülde
/// de yazılır (ayrı kipte `.sva` dosyası yoktur, not RTL dosyasındadır).
#[test]
fn rtl_file_of_a_formal_top_lists_the_macros_to_define() {
    let out = full(HIERARCHY, SvaMode::Separate);
    let top = module_sv(&out, "Top");
    let expected = "// Formal top (ADR-0097): module Top\n\
                    //   When this module is the formal top, define the macros of the\n\
                    //   instances below; their requires/assume are then checked as its\n\
                    //   obligations.\n\
                    //     vlog/vcs/xrun: +define+VOLT_SUB_Leaf+VOLT_SUB_Mid\n\
                    //     Yosys:         read -define VOLT_SUB_Leaf VOLT_SUB_Mid\n";
    let note = top.find(expected).expect("tepe notu");
    assert!(note < top.find("module Top (").expect("modül"), "{top}");
    let mid = module_sv(&out, "Mid");
    assert!(
        mid.contains("//     vlog/vcs/xrun: +define+VOLT_SUB_Leaf\n"),
        "{mid}"
    );
    // Ayrı kipte modülün kendi kuralı .sva'dadır, RTL'de değil.
    assert!(!mid.contains("Contract obligations"), "{mid}");
    let leaf = module_sv(&out, "Leaf");
    assert!(!leaf.contains("ADR-0097"), "altında örnek yok: {leaf}");
}

#[test]
fn inline_mode_puts_both_notes_in_the_module_file() {
    let out = full(HIERARCHY, SvaMode::Inline);
    let mid = module_sv(&out, "Mid");
    let rule = mid
        .find("switch on the macro VOLT_SUB_Mid\n")
        .expect("kural");
    let top = mid
        .find("// Formal top (ADR-0097): module Mid\n")
        .expect("tepe notu");
    assert!(
        rule < top && top < mid.find("module Mid (").unwrap(),
        "{mid}"
    );
    // Birleşik metin (`--single-file`) notları birim başlığında toplar.
    let unit_note = out
        .sv
        .find("switch on the macro VOLT_SUB_Leaf\n")
        .expect("birleşik");
    assert!(
        unit_note < out.sv.find("module Leaf (").unwrap(),
        "{}",
        out.sv
    );
    assert!(
        out.sv
            .contains("//     Yosys:         read -define VOLT_SUB_Leaf VOLT_SUB_Mid\n"),
        "{}",
        out.sv
    );
}

#[test]
fn rtl_only_and_verify_outputs_have_no_obligation_notes() {
    for mode in [SvaMode::None, SvaMode::Immediate] {
        let out = full(HIERARCHY, mode);
        assert!(!out.sv.contains("Formal top"), "{mode:?}: {}", out.sv);
        assert!(
            !out.sv.contains("Contract obligations"),
            "{mode:?}: {}",
            out.sv
        );
    }
}

/// Saatsiz modülde kontrat sessizce düşmez: formal akış onu kaydeder,
/// sürücü erişilebilir modül için E5005 üretir (ADR-0097).
#[test]
fn clockless_module_contracts_are_recorded_in_formal_mode() {
    let src = "module Comb {\n    in  a : u8\n    out b : u8\n\n    \
               requires: a < 10\n    ensures: b == 77\n\n    b = a + 1\n}\n";
    let out = full(src, SvaMode::Immediate);
    assert_eq!(
        out.unclocked_contracts.len(),
        1,
        "{:?}",
        out.unclocked_contracts
    );
    let u = &out.unclocked_contracts[0];
    assert_eq!(u.module, "Comb");
    assert_eq!(u.count, 2);
    // Birincil konum ilk kontratın ifadesidir.
    assert_eq!(&src[u.span.start as usize..u.span.end as usize], "a < 10");
    assert!(out.sva_props.is_empty());
}

#[test]
fn clockless_module_without_contracts_is_not_recorded() {
    let src = "module Comb {\n    in  a : u8\n    out b : u8\n\n    b = a + 1\n}\n";
    let out = full(src, SvaMode::Immediate);
    assert!(out.unclocked_contracts.is_empty());
}

#[test]
fn clockless_module_contracts_are_not_recorded_outside_formal_mode() {
    // `volt build` (SVA yok) ve simülasyon bu ADR'nin kapsamı dışında.
    let src = "module Comb {\n    in  a : u8\n    out b : u8\n\n    \
               requires: a < 10\n\n    b = a + 1\n}\n";
    for mode in [SvaMode::None, SvaMode::Simulation] {
        let out = full(src, mode);
        assert!(out.unclocked_contracts.is_empty(), "{mode:?}");
    }
}
