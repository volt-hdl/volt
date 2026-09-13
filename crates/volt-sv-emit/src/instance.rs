//! Kullanıcı modülü örnekleme — sv-mapping.md §9, ADR-0041.
//!
//! `let f = Mod { clk, sample: x }` → adlandırılmış port bağlantılı SV
//! örneklemesi. Çıkış portları modül gövdesinin başında `logic`
//! telleri olarak ön bildirilir (`f_result`), böylece `f.result`
//! okumaları deyim sırasından bağımsız çalışır. Generic argümanlar bu
//! aşamaya gelmez: monomorfizasyon (volt-syntax) somut modül adını
//! (`FirFilter_8_16`) yazmıştır.

use std::collections::HashMap;

use volt_ast::builtin::BuiltinPrim;
use volt_ast::{
    Expr, Idx, InstanceDecl, ItemKind, ModuleDecl, Port, PortDir, StmtKind, TypeRefKind,
};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::{reset_port_set, ClockPort, Emitter, Sig};

/// Doğrulanmış kullanıcı modülü örneği.
#[derive(Debug, Clone)]
pub(crate) struct UserInst {
    /// Hedef (somut) modül adı.
    pub(crate) module: String,
    /// Çıkış portları: (port adı, imza) — `f_<port>` telleri.
    pub(crate) outputs: Vec<(String, Sig)>,
}

impl<'a> Emitter<'a> {
    /// Dosyadaki `module <name>` bildirimi.
    pub(crate) fn module_decl_named(&self, name: &str) -> Option<&'a ModuleDecl> {
        self.ast
            .items
            .iter()
            .find_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) if m.name.text == name => Some(m),
                _ => None,
            })
    }

    /// Ön geçiş: kullanıcı modülü örnekleri + çıkış tellerinin ön
    /// bildirimi. Hedefi bulunmayan örnek burada atlanır; deyim
    /// sırasında mevcut E0003 tanısı verilir.
    pub(crate) fn collect_user_insts(&mut self, module: &'a ModuleDecl) {
        self.user_insts.clear();
        let ast = self.ast;
        for &stmt_idx in &module.body {
            let StmtKind::Instance(inst) = &ast.stmts[stmt_idx].kind else {
                continue;
            };
            let Some(target_name) = user_instance_target(inst) else {
                continue;
            };
            let Some(target) = self.module_decl_named(target_name) else {
                continue;
            };
            let mut outputs = Vec::new();
            for p in target.ports.iter().filter(|p| p.direction != PortDir::In) {
                if let Some(sig) = self.sig_of_typeref(p.ty, p.span) {
                    self.pre_decls.push(format!(
                        "    {} {}_{};",
                        sig.decl_type(),
                        inst.name.text,
                        p.name.text
                    ));
                    outputs.push((p.name.text.clone(), sig));
                }
            }
            let info = UserInst {
                module: target_name.to_string(),
                outputs,
            };
            self.user_insts.insert(inst.name.text.clone(), info);
        }
    }

    /// `f.result` okumasının genişliği (kullanıcı modülü çıkışı).
    pub(crate) fn user_field_sig(&self, base: Idx<Expr>, field: &str) -> Option<Sig> {
        let inst = crate::path_single(self.ast, base)?;
        let info = self.user_insts.get(inst)?;
        info.outputs
            .iter()
            .find(|(n, _)| n == field)
            .map(|&(_, sig)| sig)
    }

    /// Örnekleme metni. Port sırası hedef modülün SV port sırasıdır:
    /// saatler → resetler → giriş/inout → çıkış.
    pub(crate) fn emit_user_instance(
        &mut self,
        parent: &'a ModuleDecl,
        parent_clocks: &[ClockPort],
        inst: &'a InstanceDecl,
        span: Span,
    ) -> Option<String> {
        let name = inst.name.text.clone();
        let Some(info) = self.user_insts.get(&name).cloned() else {
            self.future(
                span,
                &lstr!(
                    en: "SV generation of module instance '{name}' (target module is not in this file)";
                    tr: "'{name}' modül örneklemesinin SV üretimi (hedef modül bu dosyada değil)"
                ),
            );
            return None;
        };
        let target = self.module_decl_named(&info.module)?;
        let bindings: HashMap<&str, Option<Idx<Expr>>> = inst
            .bindings
            .iter()
            .map(|b| (b.port_name.text.as_str(), b.value))
            .collect();
        let ast = self.ast;
        let is_clock = |p: &Port| matches!(ast.types[p.ty].kind, TypeRefKind::Clock);

        let mut conns: Vec<(String, String)> = Vec::new();
        for p in target.ports.iter().filter(|p| is_clock(p)) {
            let one = Sig {
                width: 1,
                signed: false,
            };
            conns.push((
                p.name.text.clone(),
                self.input_binding(&name, p, &bindings, one, span),
            ));
        }
        let parent_resets = reset_port_set(parent_clocks);
        for cfg in reset_port_set(&self.collect_clock_ports(target)) {
            let rst = cfg.port_name();
            if !parent_resets.iter().any(|c| c.port_name() == rst) {
                self.error(
                    ErrorCode::E2005,
                    lstr!(
                        en: "instance '{name}' needs the reset port '{rst}' but module '{}' has no clock domain with that reset", parent.name.text;
                        tr: "'{name}' örneği '{rst}' reset portunu ister ama '{}' modülünün o reset'e sahip bir saat alanı yok", parent.name.text
                    ),
                    span,
                    &lstr!(
                        en: "give the parent module a clock port in the same domain";
                        tr: "üst modüle aynı alanda bir saat portu verin"
                    ),
                );
            }
            conns.push((rst.to_string(), rst.to_string()));
        }
        for p in target.ports.iter().filter(|p| !is_clock(p)) {
            let Some(sig) = self.sig_of_typeref(p.ty, p.span) else {
                continue;
            };
            let value = if p.direction == PortDir::In {
                self.input_binding(&name, p, &bindings, sig, span)
            } else {
                self.output_binding(&name, p, &bindings, span)
            };
            conns.push((p.name.text.clone(), value));
        }
        Some(format_instance(&info.module, &name, &conns))
    }

    /// Giriş bağlaması: `port: expr` → hedef imzasıyla; `port` kısayolu
    /// → aynı adlı yerel sinyal; bağlanmamış → E2005.
    fn input_binding(
        &mut self,
        inst: &str,
        port: &Port,
        bindings: &HashMap<&str, Option<Idx<Expr>>>,
        sig: Sig,
        span: Span,
    ) -> String {
        match bindings.get(port.name.text.as_str()) {
            Some(Some(e)) => self.emit_assigned(*e, Some(sig)),
            Some(None) => port.name.text.clone(),
            None => {
                self.error(
                    ErrorCode::E2005,
                    lstr!(
                        en: "input port '{}' of instance '{inst}' is not bound", port.name.text;
                        tr: "'{inst}' örneğinin '{}' giriş portu bağlanmamış", port.name.text
                    ),
                    span,
                    &lstr!(
                        en: "bind every input: {inst} = Module {{ {}: value, ... }}", port.name.text;
                        tr: "her girişi bağlayın: {inst} = Modul {{ {}: değer, ... }}", port.name.text
                    ),
                );
                "1'b0".to_string()
            }
        }
    }

    /// Çıkış portu ön bildirilen tele bağlanır; literalde çıkışa değer
    /// yazmak E2005 (çıkış `f.port` ile okunur).
    fn output_binding(
        &mut self,
        inst: &str,
        port: &Port,
        bindings: &HashMap<&str, Option<Idx<Expr>>>,
        span: Span,
    ) -> String {
        if bindings.contains_key(port.name.text.as_str()) {
            self.error(
                ErrorCode::E2005,
                lstr!(
                    en: "output port '{}' of instance '{inst}' cannot be bound in the instance literal", port.name.text;
                    tr: "'{inst}' örneğinin '{}' çıkış portu örnekleme literalinde bağlanamaz", port.name.text
                ),
                span,
                &lstr!(
                    en: "read it as {inst}.{} instead", port.name.text;
                    tr: "bunun yerine {inst}.{} olarak okuyun", port.name.text
                ),
            );
        }
        format!("{inst}_{}", port.name.text)
    }
}

/// Yerleşik primitif olmayan, tek segmentli hedef modül adı.
pub(crate) fn user_instance_target(inst: &InstanceDecl) -> Option<&str> {
    let [seg] = inst.module_path.segments.as_slice() else {
        return None;
    };
    (BuiltinPrim::from_name(&seg.text).is_none()).then_some(seg.text.as_str())
}

/// `Mod f (\n        .clk   (clk),\n        ...\n    );` — port adları hizalı.
fn format_instance(module: &str, name: &str, conns: &[(String, String)]) -> String {
    let w = conns.iter().map(|(p, _)| p.len()).max().unwrap_or(0);
    let lines: Vec<String> = conns
        .iter()
        .map(|(p, v)| format!("        .{p:<w$}({v})"))
        .collect();
    format!("    {module} {name} (\n{}\n    );", lines.join(",\n"))
}
