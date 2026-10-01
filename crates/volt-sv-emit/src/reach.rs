//! Çıktıya yazılan modüller (ADR-0042 ek, ADR-0081 Aşama 3 bulgusu).
//!
//! Birim, `use` ile ulaşılan dosyaların TÜM öğelerini yükler; ama `use
//! riscv_core::imm_i_of` bir fn ister, `RiscvCore` modülünü değil. Çıktı
//! kümesi bu yüzden birimin modülleri değil, KÖK dosyaların modülleri ve
//! onların örnekleme kapanışıdır. Kök dosya = ana dosya (ve ondan üretilen
//! sentetik kaynaklar); `use` ile yüklenen dosyalar kütüphanedir — oradaki
//! modül ancak örneklenirse çıktıya girer.
//!
//! Kapanış emitter'ın örnek hedefiyle aynı kuralı izler
//! ([`user_instance_target`]): modül gövdesinin üst düzey `Instance`
//! deyimleri, yerleşik primitifler hariç. Monomorfize örnek somut adı
//! (`Delay_4_8`) taşır; generic şablon örneklenmediği için dışarıda kalır.

use std::collections::{HashMap, HashSet};

use volt_ast::{ItemKind, ModuleDecl, SourceFile, StmtKind};
use volt_span::FileId;

use crate::instance::user_instance_target;

/// Kök dosyalardaki modüller + örnekleme kapanışı (adlar).
pub fn reachable_modules(
    ast: &SourceFile,
    is_root_file: impl Fn(FileId) -> bool,
) -> HashSet<String> {
    let mut by_name: HashMap<&str, &ModuleDecl> = HashMap::new();
    let mut work: Vec<&str> = Vec::new();
    for &i in &ast.items {
        let item = &ast.items_arena[i];
        let ItemKind::Module(m) = &item.kind else {
            continue;
        };
        by_name.insert(m.name.text.as_str(), m);
        if is_root_file(item.span.file) {
            work.push(m.name.text.as_str());
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    while let Some(name) = work.pop() {
        if !seen.insert(name.to_string()) {
            continue;
        }
        let Some(m) = by_name.get(name) else {
            continue;
        };
        for &s in &m.body {
            if let StmtKind::Instance(inst) = &ast.stmts[s].kind {
                if let Some(target) = user_instance_target(inst) {
                    work.push(target);
                }
            }
        }
    }
    seen
}

/// Modül → doğrudan kullanıcı örneklerinin hedefleri (ADR-0097: üst
/// modülün görevinde denetlenen yükümlülükler). Modüller ve örnekler
/// kaynak sırasındadır; aynı hedef birden çok kez örneklenirse bir kez
/// yazılır.
pub fn instance_children(ast: &SourceFile) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for &i in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[i].kind else {
            continue;
        };
        let mut children: Vec<String> = Vec::new();
        for &s in &m.body {
            if let StmtKind::Instance(inst) = &ast.stmts[s].kind {
                if let Some(target) = user_instance_target(inst) {
                    if !children.iter().any(|c| c == target) {
                        children.push(target.to_string());
                    }
                }
            }
        }
        out.push((m.name.text.clone(), children));
    }
    out
}

/// Modülün altındaki (kendisi hariç, her derinlikte) örneklenen modüller;
/// `children` [`instance_children`] çıktısıdır. `volt verify` görev planı
/// ile `--emit=sva` formal tepe notu aynı kümeyi kullanır (ADR-0097).
pub fn instance_subtree(module: &str, children: &[(String, Vec<String>)]) -> Vec<String> {
    let kids = |m: &str| {
        children
            .iter()
            .find(|(name, _)| name == m)
            .map_or(&[][..], |(_, c)| c.as_slice())
    };
    let mut seen: Vec<String> = Vec::new();
    let mut work: Vec<&str> = kids(module).iter().map(String::as_str).collect();
    while let Some(m) = work.pop() {
        if m == module || seen.iter().any(|s| s == m) {
            continue;
        }
        seen.push(m.to_string());
        work.extend(kids(m).iter().map(String::as_str));
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(files: &[&str]) -> SourceFile {
        let sources: Vec<(FileId, &str)> = files
            .iter()
            .enumerate()
            .map(|(i, t)| (FileId(i as u32), *t))
            .collect();
        let parsed = volt_syntax::parse_unit(&sources);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        parsed.ast
    }

    fn names(set: &HashSet<String>) -> Vec<&str> {
        let mut v: Vec<&str> = set.iter().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    const LIB: &str = "pub module Leaf {\n    in a : bool\n    out b : bool\n    b = a\n}\n\
                       pub module Mid {\n    in a : bool\n    out b : bool\n    \
                       let l = Leaf { a: a }\n    b = l.b\n}\n\
                       pub module Unused {\n    in a : bool\n    out b : bool\n    b = a\n}\n\
                       pub fn inv(a: bool) -> bool {\n    !a\n}\n";

    #[test]
    fn root_file_modules_and_their_instances_are_reached_transitively() {
        let main = "module Top {\n    in a : bool\n    out b : bool\n    \
                    let m = Mid { a: a }\n    b = m.b\n}\n";
        let ast = unit(&[main, LIB]);
        let got = reachable_modules(&ast, |f| f == FileId(0));
        assert_eq!(names(&got), ["Leaf", "Mid", "Top"]);
    }

    #[test]
    fn a_file_that_only_calls_library_fns_reaches_no_library_module() {
        let main = "module Top {\n    in a : bool\n    out b : bool\n    b = inv(a)\n}\n";
        let ast = unit(&[main, LIB]);
        let got = reachable_modules(&ast, |f| f == FileId(0));
        assert_eq!(names(&got), ["Top"]);
    }

    #[test]
    fn a_fn_only_root_file_reaches_nothing() {
        let ast = unit(&["pub fn inc(a: u8) -> u8 {\n    a + 1\n}\n"]);
        assert!(reachable_modules(&ast, |f| f == FileId(0)).is_empty());
    }

    #[test]
    fn every_module_of_the_root_file_is_a_root_even_when_not_instantiated() {
        let main = "module A {\n    in a : bool\n    out b : bool\n    b = a\n}\n\
                    module B {\n    in a : bool\n    out b : bool\n    b = a\n}\n";
        let ast = unit(&[main]);
        assert_eq!(
            names(&reachable_modules(&ast, |f| f == FileId(0))),
            ["A", "B"]
        );
    }

    /// ADR-0097: doğrudan örnek çocukları, modüller kaynak sırasında.
    #[test]
    fn instance_children_lists_direct_user_instances_in_source_order() {
        let main = "module Top {\n    in a : bool\n    out b : bool\n    out c : bool\n    \
                    let m = Mid { a: a }\n    let l = Leaf { a: a }\n    \
                    let l2 = Leaf { a: a }\n    b = m.b\n    c = l.b && l2.b\n}\n";
        let ast = unit(&[main, LIB]);
        let got = instance_children(&ast);
        let top = got.iter().find(|(m, _)| m == "Top").expect("Top");
        assert_eq!(top.1, ["Mid", "Leaf"]);
        let mid = got.iter().find(|(m, _)| m == "Mid").expect("Mid");
        assert_eq!(mid.1, ["Leaf"]);
        let leaf = got.iter().find(|(m, _)| m == "Leaf").expect("Leaf");
        assert!(leaf.1.is_empty());
    }
}
