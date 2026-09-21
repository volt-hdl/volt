//! Örnekleme: hedef çözümü (kullanıcı modülü ya da yerleşik CDC
//! primitifi, ADR-0027) ve port adı denetimleri (E1009).

use volt_ast::{ItemKind, Name, Path, PortDir};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use super::def::{DefId, DefKind};
use super::scope::ScopeId;
use super::suggest::{closest_match, did_you_mean, with_rename};
use super::Resolver;
use crate::builtin::BuiltinPrim;

/// Örnekleme hedefi: kullanıcı modülü, yerleşik CDC primitifi
/// (ADR-0027) ya da çözülemeyen isim.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum InstanceTarget {
    Module(DefId),
    Builtin(BuiltinPrim),
    Unknown,
}

impl Resolver<'_> {
    pub(super) fn resolve_instance_target(
        &mut self,
        path: &Path,
        scope: ScopeId,
    ) -> InstanceTarget {
        let Some(first) = path.segments.first().cloned() else {
            return InstanceTarget::Unknown;
        };
        // Kapsamda çözülemeyen tek segmentli isim yerleşik bir CDC
        // primitifi olabilir (ADR-0027) — E1001 üretilmeden önce
        // denenir. Kullanıcı aynı adla modül tanımlarsa o kazanır.
        if path.segments.len() == 1 && self.lookup_visible(&first.text, scope).is_none() {
            if let Some(prim) = BuiltinPrim::from_name(&first.text) {
                return InstanceTarget::Builtin(prim);
            }
        }
        let def = self.resolve_simple(&first, scope, true);
        // soc::uart::Uart gibi çok segmentli yollar F2 (paketler) işi.
        let kind = self.def(def).kind;
        match kind {
            DefKind::Module | DefKind::ExternModule => InstanceTarget::Module(def),
            DefKind::Error | DefKind::Import => InstanceTarget::Unknown,
            DefKind::Struct => InstanceTarget::Unknown, // struct literal — tip kontrolü işi
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E1001,
                    lstr!(en: "'{}' is not a module", first.text;
                          tr: "'{}' bir modül değil", first.text),
                    LabeledSpan::primary(
                        first.span,
                        lstr!(en: "expected a module"; tr: "modül bekleniyor"),
                    ),
                    lstr!(en: "the name being instantiated must be a module or an extern module";
                          tr: "örneklenecek isim bir module ya da extern module olmalı"),
                ));
                InstanceTarget::Unknown
            }
        }
    }

    /// Yerleşik primitif örneklemesinde port BAĞLAMA adı denetimi:
    /// bilinmeyen port ya da çıkış portuna değer bağlama E1009.
    pub(super) fn check_builtin_binding(&mut self, prim: BuiltinPrim, port_name: &Name) {
        match prim.port(&port_name.text) {
            None => self.err_builtin_unknown_port(prim, port_name),
            Some(port) if port.dir == PortDir::Out => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E1009,
                    lstr!(en: "cannot bind a value to output port '{}' of '{}'",
                              port_name.text, prim.name();
                          tr: "'{}' çıkış portuna değer bağlanamaz ('{}')",
                              port_name.text, prim.name()),
                    LabeledSpan::primary(
                        port_name.span,
                        lstr!(en: "this is an output port"; tr: "bu bir çıkış portu"),
                    ),
                    lstr!(en: "outputs are read with field access after the instance: x = inst.{}",
                              port_name.text;
                          tr: "çıkışlar örneklemeden sonra alan erişimiyle okunur: x = ornek.{}",
                              port_name.text),
                ));
            }
            Some(_) => {}
        }
    }

    /// Yerleşik primitif alan OKUMASI denetimi (`inst.port`): bilinmeyen
    /// port veya giriş portu okuma E1009.
    pub(super) fn check_builtin_field(&mut self, prim: BuiltinPrim, field: &Name) {
        match prim.port(&field.text) {
            None => self.err_builtin_unknown_port(prim, field),
            Some(port) if port.dir == PortDir::In => {
                self.diagnostics.push(Diagnostic::error(
                    ErrorCode::E1009,
                    lstr!(en: "input port '{}' of '{}' cannot be read via field access",
                              field.text, prim.name();
                          tr: "'{}' giriş portu alan erişimiyle okunamaz ('{}')",
                              field.text, prim.name()),
                    LabeledSpan::primary(
                        field.span,
                        lstr!(en: "this is an input port"; tr: "bu bir giriş portu"),
                    ),
                    lstr!(en: "only output ports are readable; inputs are bound inside the instance";
                          tr: "yalnız çıkış portları okunabilir; girişler örnekleme içinde bağlanır"),
                ));
            }
            Some(_) => {}
        }
    }

    /// E1009 — yerleşik primitifte bilinmeyen port adı; gerçek port
    /// listesi yardım metninde verilir.
    fn err_builtin_unknown_port(&mut self, prim: BuiltinPrim, port_name: &Name) {
        let candidates: Vec<String> = prim.ports().iter().map(|p| p.name.to_string()).collect();
        let message = lstr!(en: "'{}' has no port '{}'", prim.name(), port_name.text;
                            tr: "'{}' primitifinde '{}' portu yok", prim.name(), port_name.text);
        self.err_unknown_port(message, port_name, &candidates);
    }

    /// E1009 gövdesi: en yakın port adı önerilir (fix-it), yoksa gerçek
    /// port listesi yardım metninde verilir.
    fn err_unknown_port(&mut self, message: String, port_name: &Name, candidates: &[String]) {
        let suggestion = closest_match(&port_name.text, candidates);
        let diag = Diagnostic::error(
            ErrorCode::E1009,
            message,
            LabeledSpan::primary(
                port_name.span,
                lstr!(en: "unknown port"; tr: "bilinmeyen port"),
            ),
            did_you_mean(
                suggestion.as_ref(),
                lstr!(en: "available ports: {}", candidates.join(", ");
                      tr: "mevcut portlar: {}", candidates.join(", ")),
            ),
        );
        self.diagnostics
            .push(with_rename(diag, port_name.span, suggestion));
    }

    pub(super) fn check_port_exists(&mut self, module_def: DefId, port_name: &Name) {
        let Some(&item_idx) = self.item_of_def.get(&module_def) else {
            return;
        };
        let ports = match &self.ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => &m.ports,
            ItemKind::Extern(x) => &x.ports,
            _ => return,
        };
        if ports.iter().any(|p| p.name.text == port_name.text) {
            return;
        }
        let module_name = &self.def(module_def).name;
        let candidates: Vec<String> = ports.iter().map(|p| p.name.text.clone()).collect();
        let message = lstr!(en: "module '{}' has no port '{}'", module_name, port_name.text;
                            tr: "'{}' modülünde '{}' portu yok", module_name, port_name.text);
        self.err_unknown_port(message, port_name, &candidates);
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};

    #[test]
    fn unknown_module_port_is_e1009_with_a_rename_fix_it() {
        let r = resolved(
            "module Alt { in data : u8 out s : u8 s = data }
             module Top { in x : u8 out y : u8 let u = Alt { dta: x } y = u.s }",
        );
        let diag = r
            .diagnostics
            .iter()
            .find(|d| d.code.as_str() == "E1009")
            .expect("E1009");
        assert_eq!(diag.suggestions[0].replacement, "data");
    }

    #[test]
    fn instantiating_a_non_module_is_e1001() {
        let c = codes(
            "const K : u8 = 1;\nmodule Top { in x : u8 out y : u8 let u = K { a: x } y = x }",
        );
        assert!(c.contains(&"E1001"), "{c:?}");
    }

    #[test]
    fn instance_edges_are_recorded_for_module_targets() {
        let r = resolved(
            "module Alt { in a : u8 out s : u8 s = a }
             module Top { in x : u8 out y : u8 let u = Alt { a: x } y = u.s }",
        );
        let (u, _) = r.def_by_name("u").expect("u");
        let (alt, _) = r.def_by_name("Alt").expect("Alt");
        assert_eq!(r.instance_module[&u], alt);
    }
}
