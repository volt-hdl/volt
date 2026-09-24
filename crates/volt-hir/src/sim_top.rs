//! Verilator üst modülü olamayan port adları (ADR-0078, E8513).
//!
//! Verilator üst modülü `V<Üst>` C++ sınıfına çevirir; portlar sınıfın
//! veri üyeleridir ve modelin kendi arayüzüyle (`eval()`, `name()`,
//! `rootp`, ...) aynı ad alanındadır. Çakışan port modeli derlenemez
//! yapar; Verilator bunu yeniden adlandırmaz (yalnız C++ anahtar
//! sözcüklerini `__SYM__` yapar). SV geçerli olduğundan `check`/`build`
//! değil, modülü üst modül olarak simüle eden `volt test`/`volt run`
//! denetler.

use volt_ast::reserved::is_verilator_model_member;
use volt_ast::{ModuleDecl, SourceFile};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use crate::sim_struct::struct_port_layout;

/// `module` Verilator üst modülü olarak simüle edilirse model sınıfının
/// üyeleriyle çakışan SV portları (struct yaprakları açılmış) — E8513.
pub fn verilator_top_clashes(src: &SourceFile, module: &ModuleDecl) -> Vec<Diagnostic> {
    let top = &module.name.text;
    let mut out = Vec::new();
    for p in &module.ports {
        let names: Vec<String> = match struct_port_layout(src, module, &p.name.text) {
            Some(layout) => layout
                .leaves
                .iter()
                .map(|l| format!("{}_{}", p.name.text, l.suffix()))
                .collect(),
            None => vec![p.name.text.clone()],
        };
        for name in names.iter().filter(|n| is_verilator_model_member(top, n)) {
            out.push(Diagnostic::error(
                ErrorCode::E8513,
                lstr!(
                    en: "port '{name}' collides with the C++ class Verilator generates for '{top}' (V{top}::{name})";
                    tr: "'{name}' portu Verilator'ın '{top}' için ürettiği C++ sınıfıyla çakışıyor (V{top}::{name})"
                ),
                LabeledSpan::primary(
                    p.name.span,
                    lstr!(en: "simulated as the top-level module"; tr: "üst modül olarak simüle ediliyor"),
                ),
                lstr!(
                    en: "rename the port, or simulate a wrapper that instantiates '{top}' (the SystemVerilog is valid; see volt explain E8513)";
                    tr: "portu yeniden adlandırın ya da '{top}' modülünü örnekleyen bir sarmalayıcıyı simüle edin (SystemVerilog geçerli; bkz. volt explain E8513)"
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ast::ItemKind;

    fn clashes(src: &str) -> Vec<String> {
        let ast = volt_syntax::parser::parse(volt_span::FileId(0), src).ast;
        let module = ast
            .items
            .iter()
            .find_map(|&i| match &ast.items_arena[i].kind {
                ItemKind::Module(m) => Some(m),
                _ => None,
            })
            .expect("modül");
        verilator_top_clashes(&ast, module)
            .iter()
            .map(|d| d.message.clone())
            .collect()
    }

    #[test]
    fn model_members_and_class_name_clash() {
        let got = clashes("module Probe {\n in clk : clock\n out name : u8\n out eval : u8\n out VProbe : u8\n out data : u8\n name = 0\n eval = 0\n VProbe = 0\n data = 0\n}\n");
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(got[0].contains("VProbe::name"), "{got:?}");
    }

    #[test]
    fn struct_leaf_names_are_checked() {
        let got = clashes("struct S {\n end_step : u8\n}\nmodule M {\n in eval : S\n out y : u8\n y = eval.end_step\n}\n");
        assert_eq!(got.len(), 1, "{got:?}");
        assert!(got[0].contains("'eval_end_step'"), "{got:?}");
    }

    #[test]
    fn ordinary_and_cpp_keyword_ports_pass() {
        assert!(
            clashes("module M {\n in interrupt : u8\n out y : u8\n y = interrupt\n}\n").is_empty()
        );
    }
}
