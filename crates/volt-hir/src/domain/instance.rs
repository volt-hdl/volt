//! K8 — modül örnekleme: saat bağlamalarından alan haritası, diğer
//! portların bu haritaya göre denetimi; extern sınırı dahil (ADR-0047).

use std::collections::{HashMap, HashSet};

use volt_ast::{InstanceDecl, ItemKind, Port, PortBinding};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::{DomainId, Inferencer};
use crate::resolve::{DefId, DefKind};

impl<'a> Inferencer<'a> {
    pub(super) fn check_instance(&mut self, inst: &InstanceDecl) {
        let Some(inst_def) = self.decl_def(&inst.name) else {
            return;
        };
        if let Some(&prim) = self.res.instance_builtin.get(&inst_def) {
            self.check_builtin_instance(inst, inst_def, prim);
            return;
        }
        let Some(&target) = self.res.instance_module.get(&inst_def) else {
            // Struct literal veya çözülmemiş hedef — bağlama ifadeleri
            // yine de yayılıma girer.
            for b in &inst.bindings {
                self.propagate_binding(b);
            }
            return;
        };
        let Some(target_ports) = self.target_ports(target) else {
            return;
        };

        // Hedef modülün saat portları (K8 adım 1 hazırlığı).
        let target_clocks: Vec<&Port> = target_ports
            .iter()
            .filter(|p| self.is_clock_port(p))
            .collect();

        // 1. Saat bağlantılarından modülün domain haritasını çıkar.
        let mapping = self.build_domain_mapping(inst, target_ports, &target_clocks);

        // 2. Diğer portları bu haritaya göre kontrol et.
        let mut port_domains: HashMap<String, DomainId> = HashMap::new();
        for port in target_ports {
            let expected = self.expected_port_domain(port, &target_clocks, &mapping);
            port_domains.insert(port.name.text.clone(), expected);
        }
        self.check_port_bindings(inst, target_ports, &port_domains);

        self.instance_ports.insert(inst_def, port_domains);
    }

    /// Örnekleme hedefinin (modül ya da extern, ADR-0047) port listesi.
    fn target_ports(&self, target: DefId) -> Option<&'a [Port]> {
        let &item_idx = self.res.item_of_def.get(&target)?;
        match &self.ast.items_arena[item_idx].kind {
            ItemKind::Module(m) => Some(&m.ports),
            ItemKind::Extern(x) => Some(&x.ports),
            _ => None,
        }
    }

    /// K8 adım 2: saat dışı her bağlama hedef portun beklenen alanında olmalı.
    fn check_port_bindings(
        &mut self,
        inst: &InstanceDecl,
        target_ports: &[Port],
        port_domains: &HashMap<String, DomainId>,
    ) {
        for b in &inst.bindings {
            let Some(port) = target_ports
                .iter()
                .find(|p| p.name.text == b.port_name.text)
            else {
                self.propagate_binding(b);
                continue;
            };
            if self.is_clock_port(port) {
                continue;
            }
            let expected = port_domains
                .get(&port.name.text)
                .copied()
                .unwrap_or(DomainId::Timeless);
            self.check_binding(expected, b);
        }
    }

    /// K8 adım 1: saat bağlamalarından anahtar → gerçek alan haritası.
    /// Aynı anahtara (sembolik ya da açık alan) ikinci bir saat farklı
    /// alandan gelirse E3014; anahtar Error'a düşer ki port denetimleri
    /// E3001 kaskadı üretmesin (ADR-0047).
    fn build_domain_mapping(
        &mut self,
        inst: &InstanceDecl,
        target_ports: &[Port],
        target_clocks: &[&Port],
    ) -> HashMap<DefId, DomainId> {
        let mut mapping: HashMap<DefId, DomainId> = HashMap::new();
        let mut first_bind: HashMap<DefId, Span> = HashMap::new();
        for b in &inst.bindings {
            let Some(port) = target_ports
                .iter()
                .find(|p| p.name.text == b.port_name.text)
            else {
                continue;
            };
            if !self.is_clock_port(port) {
                continue;
            }
            let Some(key) = self.port_domain_key(port, target_clocks) else {
                continue;
            };
            let actual = self.binding_domain(b);
            match mapping.get(&key).copied() {
                Some(prev) if self.clocks_conflict(prev, actual) => {
                    let prev_span = first_bind.get(&key).copied().unwrap_or(b.span);
                    self.err_clock_conflict(key, prev, actual, prev_span, b.span);
                    mapping.insert(key, DomainId::Error);
                }
                Some(_) => {}
                None => {
                    mapping.insert(key, actual);
                    first_bind.insert(key, b.span);
                }
            }
        }
        mapping
    }

    /// İki saat bağlaması çelişir mi: ikisi de belirli ve farklı alan.
    /// Timeless/Error/Unresolved taraflar çelişki sayılmaz (kaskad yok).
    fn clocks_conflict(&self, a: DomainId, b: DomainId) -> bool {
        matches!(
            (self.resolve_dom(a), self.resolve_dom(b)),
            (DomainId::Explicit(x), DomainId::Explicit(y)) if x != y
        )
    }

    /// E3014 (ADR-0047) — aynı alan anahtarına iki farklı saat bağlandı.
    /// Beş parça: kod, konum (ikinci bağlama), açıklama, öneri, ADR
    /// referansı (not); ilk bağlama ikincil etiket taşır.
    fn err_clock_conflict(
        &mut self,
        key: DefId,
        prev: DomainId,
        actual: DomainId,
        prev_span: Span,
        span: Span,
    ) {
        let key_name = format!("@{}", self.res.defs[key.0 as usize].name);
        let what = if self.res.def_kind(key) == DefKind::DomainParam {
            lstr!(en: "symbolic domain"; tr: "sembolik saat alanı")
        } else {
            lstr!(en: "clock domain"; tr: "saat alanı")
        };
        let (d_prev, d_now) = (self.display(prev), self.display(actual));
        let diag = Diagnostic::error(
            ErrorCode::E3014,
            lstr!(en: "{what} '{key_name}' is bound to two different clocks: {d_prev} and {d_now}";
                  tr: "'{key_name}' {what} iki farklı saate bağlandı: {d_prev} ve {d_now}"),
            LabeledSpan::primary(
                span,
                lstr!(en: "this clock is {d_now}"; tr: "bu saat {d_now}"),
            ),
            lstr!(en: "drive both clock ports of '{key_name}' from one clock, or give the second \
                       port its own domain in the declaration (e.g. @Dst)";
                  tr: "'{key_name}' alanının iki saat portunu da tek saatten sürün ya da \
                       bildirimde ikinci porta kendi alanını verin (ör. @Dst)"),
        )
        .with_secondary(
            prev_span,
            lstr!(en: "'{key_name}' was already bound to {d_prev} here";
                  tr: "'{key_name}' burada zaten {d_prev} olarak bağlanmıştı"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "one domain annotation stands for exactly one clock domain per \
                       instantiation; two clocks would open a CDC path inside the module \
                       (ADR-0047)";
                  tr: "bir alan anotasyonu her örneklemede tam olarak bir saat alanını temsil \
                       eder; iki saat modülün içinde bir CDC yolu açar (ADR-0047)"),
        );
        self.diagnostics.push(diag);
    }

    /// Hedef portun domain anahtarı: açık @Domain tanımı ya da hedefin
    /// clock portu (tek saat kuralı hedef modülde de geçerli).
    fn port_domain_key(&self, port: &Port, target_clocks: &[&Port]) -> Option<DefId> {
        if let Some(ann) = &port.domain {
            let key = self.use_def(ann.span)?;
            // K11 (ADR-0052): hedefte clock portu taşımayan trust_level'lı
            // anotasyon saat anahtarı değildir — tek saat kuralına düşer.
            let target_anchored: HashSet<DefId> = target_clocks
                .iter()
                .filter_map(|c| c.domain.as_ref())
                .filter_map(|a| self.use_def(a.span))
                .collect();
            if !self.is_trust_only_alias(key, &target_anchored) {
                return Some(key);
            }
        }
        let def = self.decl_def(&port.name)?;
        if self.is_clock_def(def) {
            return Some(def);
        }
        match target_clocks {
            [single] => {
                if let Some(ann) = &single.domain {
                    self.use_def(ann.span)
                } else {
                    self.decl_def(&single.name)
                }
            }
            _ => None,
        }
    }

    /// K8 adım 2: haritada eşleşme yoksa global domain tanımı geçerli;
    /// bağlanmamış sembolik alan (ADR-0047) Error (denetlenemez, kaskad
    /// yok); o da yoksa Timeless (saatsiz hedef modül).
    fn expected_port_domain(
        &mut self,
        port: &Port,
        target_clocks: &[&Port],
        mapping: &HashMap<DefId, DomainId>,
    ) -> DomainId {
        let Some(key) = self.port_domain_key(port, target_clocks) else {
            return DomainId::Timeless;
        };
        if let Some(&dom) = mapping.get(&key) {
            return dom;
        }
        match self.by_decl.get(&key) {
            Some(&id) => DomainId::Explicit(id),
            None if self.res.def_kind(key) == DefKind::DomainParam => DomainId::Error,
            None => DomainId::Timeless,
        }
    }

    /// Hedef portu bulunamayan bağlama: ifadesi yine de yayılıma girer.
    pub(super) fn propagate_binding(&mut self, b: &PortBinding) {
        if let Some(e) = b.value {
            self.expr_domain(e);
        }
    }

    /// Bağlanan değer portun beklenen alanıyla uyumlu mu (K6 üzerinden).
    pub(super) fn check_binding(&mut self, expected: DomainId, b: &PortBinding) {
        let actual = self.binding_domain(b);
        let value_span = match b.value {
            Some(e) => self.ast.exprs[e].span,
            None => b.span,
        };
        self.check_compat(expected, actual, b.span, value_span);
    }

    pub(super) fn binding_domain(&mut self, b: &PortBinding) -> DomainId {
        match b.value {
            Some(e) => self.expr_domain(e),
            // `clk:` kısayolu — yerel isim port adıyla aynı.
            None => self
                .use_def(b.port_name.span)
                .map(|def| self.def_domain(def))
                .unwrap_or(DomainId::Timeless),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{inferred, two_clock};

    const INNER: &str =
        "module Inner {\n    in clk : clock\n    in d : u8\n    out q : u8\n    q = d\n}\n";
    const EXT: &str =
        "extern module Ext {\n    in wr_clk : clock @Core\n    in rd_clk : clock @Core\n    \
                       in d : u8 @Core\n    out q : u8 @Core\n}\n";

    #[test]
    fn target_ports_follow_the_bound_clock() {
        let r = inferred(&format!(
            "{INNER}{}",
            two_clock("    in a : u8 @Slow\n    out y : u8 @Slow\n    let i = Inner { clk: slow_clk, d: a }\n    y = i.q")
        ));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn binding_from_another_domain_is_e3001() {
        let r = inferred(&format!(
            "{INNER}{}",
            two_clock("    in a : u8 @Fast\n    let i = Inner { clk: slow_clk, d: a }")
        ));
        assert_eq!(r.codes(), ["E3001"]);
    }

    #[test]
    fn shorthand_binding_uses_the_local_signal_of_the_same_name() {
        let r = inferred(&format!(
            "{INNER}module Top {{\n    in clk : clock\n    in d : u8\n    let i = Inner {{ clk, d }}\n}}\n"
        ));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }

    #[test]
    fn one_symbolic_domain_bound_to_two_clocks_is_e3014() {
        let r = inferred(&format!(
            "{EXT}{}",
            two_clock(
                "    in a : u8 @Fast\n    let e = Ext { wr_clk: fast_clk, rd_clk: slow_clk, d: a }"
            )
        ));
        // Anahtar Error'a düşer: port denetimleri E3001 kaskadı üretmez.
        assert_eq!(r.codes(), ["E3014"]);
        let d = &r.dom.diagnostics[0];
        assert!(
            d.message.contains("@Core")
                && d.message.contains("@Fast")
                && d.message.contains("@Slow")
        );
        assert_eq!(d.spans.len(), 2);
    }

    #[test]
    fn same_clock_on_both_ports_of_one_symbolic_domain_is_clean() {
        let r = inferred(&format!(
            "{EXT}{}",
            two_clock(
                "    in a : u8 @Fast\n    let e = Ext { wr_clk: fast_clk, rd_clk: fast_clk, d: a }"
            )
        ));
        assert!(r.codes().is_empty(), "{:?}", r.codes());
    }
}
