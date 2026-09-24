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
    Expr, ExternDecl, Idx, InstanceDecl, ItemKind, ModuleDecl, Port, PortDir, StmtKind, TypeRefKind,
};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use crate::alias;
use crate::{reset_port_set, ClockPort, Emitter, Sig};

/// Doğrulanmış kullanıcı modülü örneği.
#[derive(Debug, Clone)]
pub(crate) struct UserInst {
    /// Hedef (somut) modül adı.
    pub(crate) module: String,
    /// Çıkış portları: (port adı, imza) — `f_<port>` telleri.
    pub(crate) outputs: Vec<(String, Sig)>,
}

/// Örneklenebilir kullanıcı hedefi: Volt modülü ya da `extern module`.
#[derive(Clone, Copy)]
pub(crate) enum InstTarget<'a> {
    Module(&'a ModuleDecl),
    Extern(&'a ExternDecl),
}

impl<'a> InstTarget<'a> {
    pub(crate) fn ports(self) -> &'a [Port] {
        match self {
            InstTarget::Module(m) => &m.ports,
            InstTarget::Extern(e) => &e.ports,
        }
    }
}

impl<'a> Emitter<'a> {
    /// Örnekleme hedefi: aynı adlı `module` ya da generic'siz `extern
    /// module`.
    pub(crate) fn inst_target_named(&self, name: &str) -> Option<InstTarget<'a>> {
        self.ast
            .items
            .iter()
            .find_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) if m.name.text == name => Some(InstTarget::Module(m)),
                // Extern generic'leri monomorfize edilmez (ADR-0047):
                // hedefsiz kalır, deyim sırasında E0003.
                ItemKind::Extern(e) if e.name.text == name && e.generics.is_empty() => {
                    Some(InstTarget::Extern(e))
                }
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
            let Some(target) = self.inst_target_named(target_name) else {
                continue;
            };
            // Çift yönlü portlar (ADR-0051): üst modülün teli bağlanır,
            // `<örnek>_<port>` teli üretilmez; bağlanan tel net olur.
            for p in target
                .ports()
                .iter()
                .filter(|p| p.direction.is_bidirectional())
            {
                let bound = inst
                    .bindings
                    .iter()
                    .find(|b| b.port_name.text == p.name.text)
                    .and_then(|b| match b.value {
                        Some(e) => crate::path_single(ast, e).map(str::to_owned),
                        None => Some(p.name.text.clone()),
                    });
                if let Some(wire) = bound {
                    let entry = self.bus_wires.entry(wire).or_insert(p.direction);
                    if p.direction == PortDir::OpenDrain {
                        *entry = PortDir::OpenDrain;
                    }
                }
            }
            let mut outputs = Vec::new();
            for p in target
                .ports()
                .iter()
                .filter(|p| p.direction == PortDir::Out)
            {
                if let Some(sig) = self.port_sig(p) {
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
            let target = user_instance_target(inst).unwrap_or_default();
            let what = alias::describe_missing_instance_target(self.ast, &name, target);
            self.future(span, &what);
            return None;
        };
        let bindings: HashMap<&str, Option<Idx<Expr>>> = inst
            .bindings
            .iter()
            .map(|b| (b.port_name.text.as_str(), b.value))
            .collect();
        let target = match self.inst_target_named(&info.module)? {
            InstTarget::Module(m) => m,
            InstTarget::Extern(e) => {
                let conns = self.extern_conns(&name, e, &bindings, span);
                return Some(format_instance(&info.module, &name, &conns));
            }
        };
        let ast = self.ast;
        let is_clock = |p: &Port| {
            matches!(
                ast.types[crate::alias::resolve(ast, p.ty)].kind,
                TypeRefKind::Clock
            )
        };

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
        let target_clocks = self.collect_clock_ports(target);
        for cfg in reset_port_set(&target_clocks) {
            let rst = cfg.port_name();
            // ADR-0065 §1: çocuğun saatine bağlanan ebeveyn saatinin alanı
            // ham portla besleniyorsa zincir çıkışı bağlanır.
            if let Some(synced) =
                parent_synced_reset(target, &target_clocks, &cfg, &bindings, parent_clocks, ast)
            {
                conns.push((rst.to_string(), synced));
                continue;
            }
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
        // Çocuğun ham reset portları, SV port sırasıyla (otomatiklerden sonra).
        let is_raw = |p: &Port| crate::reset_sync::is_raw_reset(ast, p);
        for p in target.ports.iter().filter(|p| is_raw(p)) {
            let value = self.input_binding(&name, p, &bindings, Sig::BIT, span);
            conns.push((p.name.text.clone(), value));
        }
        for p in target.ports.iter().filter(|p| !is_clock(p) && !is_raw(p)) {
            let Some(sig) = self.port_sig(p) else {
                continue;
            };
            let value = match p.direction {
                PortDir::In => self.input_binding(&name, p, &bindings, sig, span),
                PortDir::Out => self.output_binding(&name, p, &bindings, span),
                PortDir::InOut | PortDir::OpenDrain => {
                    self.bidir_binding(&name, p, &bindings, span)
                }
            };
            conns.push((p.name.text.clone(), value));
        }
        Some(format_instance(&info.module, &name, &conns))
    }

    /// `extern module` bağlantıları (ADR-0071): bildirim sırasıyla, her
    /// port adıyla; dış SV modülünün arayüzü bildirilen portlardır, Volt
    /// modüllerindeki gibi alan reset'i için örtük port eklenmez.
    fn extern_conns(
        &mut self,
        name: &str,
        target: &'a ExternDecl,
        bindings: &HashMap<&str, Option<Idx<Expr>>>,
        span: Span,
    ) -> Vec<(String, String)> {
        let mut conns = Vec::new();
        for p in &target.ports {
            let Some(sig) = self.port_sig(p) else {
                continue;
            };
            let value = match p.direction {
                PortDir::In => self.input_binding(name, p, bindings, sig, span),
                PortDir::Out => self.output_binding(name, p, bindings, span),
                PortDir::InOut | PortDir::OpenDrain => self.bidir_binding(name, p, bindings, span),
            };
            conns.push((p.name.text.clone(), value));
        }
        conns
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

    /// Çift yönlü port (ADR-0051) üst modülün bir `wire`ına ya da kendi
    /// çift yönlü portuna ADIYLA bağlanır (net paylaşımı); ifade ya da
    /// bağlanmamış port E2005.
    fn bidir_binding(
        &mut self,
        inst: &str,
        port: &Port,
        bindings: &HashMap<&str, Option<Idx<Expr>>>,
        span: Span,
    ) -> String {
        let simple = match bindings.get(port.name.text.as_str()) {
            Some(Some(e)) => crate::path_single(self.ast, *e).map(str::to_owned),
            Some(None) => Some(port.name.text.clone()),
            None => None,
        };
        match simple {
            Some(wire) => wire,
            None => {
                self.error(
                    ErrorCode::E2005,
                    lstr!(
                        en: "{} port '{}' of instance '{inst}' must be bound to a wire or a bidirectional port of this module", port.direction.keyword(), port.name.text;
                        tr: "'{inst}' örneğinin {} portu '{}' bu modülün bir wire'ına ya da çift yönlü portuna bağlanmalı", port.direction.keyword(), port.name.text
                    ),
                    span,
                    &lstr!(
                        en: "declare wire {}_bus : <type> and bind {}: {}_bus — the line is shared, not computed", port.name.text, port.name.text, port.name.text;
                        tr: "wire {}_bus : <tip> bildirip {}: {}_bus bağlayın — hat paylaşılır, hesaplanmaz", port.name.text, port.name.text, port.name.text
                    ),
                );
                "1'bz".to_string()
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

/// Çocuğun otomatik reset portuna bağlanacak ebeveyn zincir çıkışı
/// (ADR-0065 §1): o porta bağlı çocuk saatlerinden biri, reset'i ham
/// portla beslenen ve aynı polariteli bir ebeveyn saatine bağlıysa.
/// Çocukta reset örneklemeyen saat (`AsyncDualPortRam.wr_clk`'e
/// giden) önce elenir: port, reset'li flop'ların saatinin zincirini
/// almalı. Hiçbiri kalmazsa bütün saatlere bakılır.
fn parent_synced_reset(
    target: &ModuleDecl,
    target_clocks: &[ClockPort],
    cfg: &crate::ResetCfg,
    bindings: &HashMap<&str, Option<Idx<Expr>>>,
    parent_clocks: &[ClockPort],
    ast: &volt_ast::SourceFile,
) -> Option<String> {
    let candidates: Vec<&ClockPort> = target_clocks
        .iter()
        .filter(|c| !c.info.reset.is_none() && c.info.reset.synced.is_none())
        .filter(|c| c.info.reset.port_name() == cfg.port_name())
        .collect();
    let sampling: Vec<&ClockPort> = candidates
        .iter()
        .copied()
        .filter(|c| !crate::reset_sync::reset_free_clock(ast, target, &c.name))
        .collect();
    let chosen = if sampling.is_empty() {
        candidates
    } else {
        sampling
    };
    chosen.into_iter().find_map(|c| {
        let parent_name = match bindings.get(c.name.as_str())? {
            Some(e) => crate::path_single(ast, *e)?,
            None => c.name.as_str(),
        };
        let parent = parent_clocks.iter().find(|p| p.name == parent_name)?;
        let reset = &parent.info.reset;
        (reset.polarity == cfg.polarity)
            .then(|| reset.synced.clone())
            .flatten()
    })
}
