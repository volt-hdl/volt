//! K8 — modül örnekleme: saat bağlamalarından alan haritası, diğer
//! portların bu haritaya göre denetimi; extern sınırı dahil (ADR-0047).

use std::collections::{BTreeSet, HashMap, HashSet};

use volt_ast::{ClockEdge, ExprKind, InstanceDecl, ItemKind, Port, PortBinding, PortDir};
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};
use volt_span::Span;

use super::edgeless::EdgeUse;
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
        // 1b. Kenarsız saat, kenarlı alandaki çocuk saat portunu sürmez
        //     (ADR-0098 eki 2; extern'in içi Volt'un üretimi değildir).
        self.check_child_clock_edges(inst, target, target_ports);

        // 2. Diğer portları bu haritaya göre kontrol et.
        let mut port_domains: HashMap<String, DomainId> = HashMap::new();
        for port in target_ports {
            let expected = self.expected_port_domain(port, &target_clocks, &mapping);
            port_domains.insert(port.name.text.clone(), expected);
        }
        // 3. Saatsiz hedef (K8b, ADR-0114): anotasyonsuz çıkış, modül
        //    özetinde bağlı olduğu girişlerin alanlarının K5 join'idir.
        if target_clocks.is_empty() {
            self.clockless_outputs(inst, inst_def, target, target_ports, &mut port_domains);
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
            // Örneklenemeyen öğeler (hedef hatası isim çözümlemede).
            ItemKind::Domain(_)
            | ItemKind::Fn(_)
            | ItemKind::Struct(_)
            | ItemKind::Enum(_)
            | ItemKind::Const(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Test(_)
            | ItemKind::Error => None,
        }
    }

    /// Çocuk modülün saat portu kenarlı bir alandaysa (açıklama kenarlı bir
    /// `domain`, ya da örtük alan: posedge) ona bağlanan saat kenarsız bir
    /// alandan gelemez (E3016): çocuk flop'larını o kenarda zamanlar.
    fn check_child_clock_edges(&mut self, inst: &InstanceDecl, target: DefId, ports: &[Port]) {
        let Some(&item_idx) = self.res.item_of_def.get(&target) else {
            return;
        };
        let ItemKind::Module(child) = &self.ast.items_arena[item_idx].kind else {
            return;
        };
        for b in &inst.bindings {
            let Some(port) = ports.iter().find(|p| p.name.text == b.port_name.text) else {
                continue;
            };
            if !self.is_clock_port(port) || !self.child_port_has_edge(port) {
                continue;
            }
            let dom = self.binding_domain(b);
            let use_ = EdgeUse::ChildClock {
                module: &child.name.text,
                port: &port.name.text,
            };
            self.check_edgeless_use(dom, b.span, use_);
        }
    }

    /// Çocuk saat portunun alanının kenarı var mı: örtük alan posedge'dir
    /// (K2); açıklama bir `domain` bildirimine çözülürse onun kenarı.
    fn child_port_has_edge(&self, port: &Port) -> bool {
        let Some(ann) = &port.domain else {
            return true;
        };
        self.use_def(ann.span)
            .and_then(|def| self.by_decl.get(&def))
            .is_some_and(|&id| !matches!(self.domains[id as usize].clock.edge, ClockEdge::None))
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
                       instantiation; two clocks would open a CDC path inside the module";
                  tr: "bir alan anotasyonu her örneklemede tam olarak bir saat alanını temsil \
                       eder; iki saat modülün içinde bir CDC yolu açar"),
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

    /// K8b (ADR-0114): saatsiz hedefin anotasyonsuz çıkışı, özette bağlı
    /// olduğu girişlere bağlanan değerlerin K5 join'ini taşır; `@Alan`
    /// anotasyonlu giriş kendi alanıyla katılır, anotasyonlu çıkış kendi
    /// alanında kalır (girişlerini `check_annotated_output` denetler). İki
    /// farklı alan aynı çıkışta birleşirse örnek sitesinde, örnek başına
    /// bir kez E3001; çıkış `Error` olur (kaskad yok). Modülün içinde tanı
    /// yoktur: içeride anotasyonsuz portlar Timeless'tır.
    fn clockless_outputs(
        &mut self,
        inst: &InstanceDecl,
        inst_def: DefId,
        target: DefId,
        target_ports: &[Port],
        port_domains: &mut HashMap<String, DomainId>,
    ) {
        let Some(sum) = self.summaries.get(target) else {
            return;
        };
        let target_name = self.res.defs[target.0 as usize].name.clone();
        let mut deps_by_out: HashMap<String, Vec<String>> = HashMap::new();
        let mut reported = false;
        for port in target_ports.iter().filter(|p| p.direction == PortDir::Out) {
            let out = &port.name.text;
            let own = port_domains.get(out).copied().unwrap_or(DomainId::Timeless);
            if !matches!(self.resolve_dom(own), DomainId::Timeless) {
                if let (false, Some(deps)) = (sum.worst_case, sum.comb.get(out)) {
                    let at = Annotated {
                        target: &target_name,
                        port,
                        own,
                        deps,
                    };
                    self.check_annotated_output(inst, at, port_domains);
                }
                continue;
            }
            let Some(deps) = sum.comb.get(out) else {
                continue;
            };
            deps_by_out.insert(out.clone(), deps.iter().cloned().collect());
            let mut dom = DomainId::Timeless;
            let mut first: Option<(String, DomainId, Span)> = None;
            for input in deps {
                let Some(b) = inst.bindings.iter().find(|b| &b.port_name.text == input) else {
                    continue;
                };
                let declared = port_domains
                    .get(input)
                    .copied()
                    .unwrap_or(DomainId::Timeless);
                let d = if matches!(self.resolve_dom(declared), DomainId::Timeless) {
                    self.binding_domain(b)
                } else {
                    declared
                };
                let (cur, new) = (self.resolve_dom(dom), self.resolve_dom(d));
                if let (DomainId::Explicit(x), DomainId::Explicit(y)) = (cur, new) {
                    if x != y {
                        if let (false, Some(a)) = (reported, first.clone()) {
                            let merge = Merge {
                                target: &target_name,
                                output: out,
                                deps,
                                first: a,
                                second: (input.clone(), d, b.span),
                            };
                            self.err_cdc_instance_merge(inst, merge, b);
                            reported = true;
                        }
                        dom = DomainId::Error;
                        break;
                    }
                }
                if first.is_none() && !matches!(new, DomainId::Timeless) {
                    first = Some((input.clone(), d, b.span));
                }
                dom = self.join(dom, d, b.span, b.span);
            }
            port_domains.insert(out.clone(), dom);
        }
        self.clockless_deps
            .insert(inst_def, (target_name, deps_by_out));
    }

    /// E3001 (örnek biçimi, ADR-0114 K3): saatsiz hedef iki alandan gelen
    /// girişleri aynı çıkışta birleştiriyor. Beş parça: kod, konum (ikinci
    /// alanın bağlaması), açıklama, öneri (ikinci girişi ilk alana
    /// senkronize et), gerekçe notu (çıkışın hangi girişlere bağlı olduğu)
    /// + ikincil etiketler (ilk bağlama, iki alanın tanımı).
    fn err_cdc_instance_merge(&mut self, inst: &InstanceDecl, m: Merge<'_>, second: &PortBinding) {
        let (DomainId::Explicit(x), DomainId::Explicit(y)) =
            (self.resolve_dom(m.first.1), self.resolve_dom(m.second.1))
        else {
            return;
        };
        let inst_name = &inst.name.text;
        let (target, out, input) = (m.target, m.output, &m.second.0);
        let (dx, dy) = (
            self.domain_name(x).to_string(),
            self.domain_name(y).to_string(),
        );
        let ports_en = quote_list(m.deps, "and");
        let ports_tr = quote_list(m.deps, "ve");
        let fix = self.binding_sync_fix(second, m.first.1);
        let help = match &fix {
            Some((line, synced)) => lstr!(
                en: "synchronize '{input}' into @{dx} before the instance: add '{line}' above it and bind '{input}: {synced}'";
                tr: "'{input}' girişini örnekten önce @{dx} alanına senkronize edin: üstüne '{line}' ekleyin ve '{input}: {synced}' bağlayın"
            ),
            None => lstr!(
                en: "synchronize one input into the other's domain before the instance: let s = sync(<signal>, <clock of {dx}>), then bind s";
                tr: "örnekten önce bir girişi ötekinin alanına senkronize edin: let s = sync(<sinyal>, <{dx} saati>), sonra s'yi bağlayın"
            ),
        };
        let mut diag = Diagnostic::error(
            ErrorCode::E3001,
            lstr!(
                en: "instance '{inst_name}' combines two clock domains in '{target}.{out}'";
                tr: "'{inst_name}' örneği '{target}.{out}' çıkışında iki saat alanını birleştiriyor"
            ),
            LabeledSpan::primary(m.second.2, format!("@{dy}")),
            help,
        )
        .with_secondary(m.first.2, format!("@{dx}"))
        .with_secondary(self.domain_span(x), self.defined_here(x))
        .with_secondary(self.domain_span(y), self.defined_here(y))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "'{target}.{out}' depends on {ports_en}; '{target}' has no clock, so it is a combinational gate and the two domains meet inside it, where they can produce a glitch that the next register captures";
                tr: "'{target}.{out}', {ports_tr} girişlerine bağlı; '{target}' saatsizdir, yani kombinasyonel bir kapıdır ve iki alan onun içinde birleşir; oluşan geçici darbeyi (glitch) sonraki register yakalayabilir"
            ),
        );
        if let (Some((line, synced)), Some(anchor), Some(value)) =
            (fix, self.cur_stmt, second.value)
        {
            // suggestion: e3001_clockless_instance
            let s = Suggestion::line_above(anchor, line, Applicability::MaybeIncorrect)
                .and_replace(self.ast.exprs[value].span, synced);
            diag = diag.with_suggestion(s);
        }
        self.diagnostics.push(diag);
    }

    /// K8b (ADR-0114): saatsiz Volt modülünün `@Alan` anotasyonlu çıkışı
    /// kendi alanında kalır, ama özette bağlı olduğu anotasyonsuz girişlere
    /// bağlanan değerler o alanda olmalıdır: başka alandan gelen değer
    /// kapının içinden senkronizörsüz geçer. E3001 örnek sitesinde, çıkış
    /// başına bir kez, ilk uyuşmayan bağlamada; çıkışın okumaları
    /// anotasyonun alanını taşır (kaskad yok). Anotasyonlu giriş modülün
    /// içinde denetlenir (K6). Saatsiz extern'de anotasyon kullanıcının
    /// beyanıdır, denetlenmez (çağıran `worst_case`'i dışarıda bırakır).
    fn check_annotated_output(
        &mut self,
        inst: &InstanceDecl,
        at: Annotated<'_>,
        port_domains: &HashMap<String, DomainId>,
    ) {
        let DomainId::Explicit(x) = self.resolve_dom(at.own) else {
            return;
        };
        for input in at.deps {
            let Some(b) = inst.bindings.iter().find(|b| &b.port_name.text == input) else {
                continue;
            };
            let declared = port_domains
                .get(input)
                .copied()
                .unwrap_or(DomainId::Timeless);
            if !matches!(self.resolve_dom(declared), DomainId::Timeless) {
                continue;
            }
            let d = self.binding_domain(b);
            if let DomainId::Explicit(y) = self.resolve_dom(d) {
                if y != x {
                    self.err_cdc_annotated_output(inst, &at, input, (x, y), b);
                    return;
                }
            }
        }
    }

    /// E3001 (örnek biçimi, ADR-0114): saatsiz hedefin anotasyonlu çıkışına
    /// başka bir alandan değer ulaşıyor. Beş parça: kod, konum (uyuşmayan
    /// bağlama), açıklama, öneri (girişi çıkışın alanına senkronize et),
    /// gerekçe notu + ikincil etiketler (çıkışın anotasyonu, iki alanın
    /// tanımı).
    fn err_cdc_annotated_output(
        &mut self,
        inst: &InstanceDecl,
        at: &Annotated<'_>,
        input: &str,
        (x, y): (u32, u32),
        b: &PortBinding,
    ) {
        let inst_name = &inst.name.text;
        let (target, out) = (at.target, &at.port.name.text);
        let (dx, dy) = (
            self.domain_name(x).to_string(),
            self.domain_name(y).to_string(),
        );
        let fix = self.binding_sync_fix(b, at.own);
        let help = match &fix {
            Some((line, synced)) => lstr!(
                en: "synchronize '{input}' into @{dx} before the instance: add '{line}' above it and bind '{input}: {synced}'";
                tr: "'{input}' girişini örnekten önce @{dx} alanına senkronize edin: üstüne '{line}' ekleyin ve '{input}: {synced}' bağlayın"
            ),
            None => lstr!(
                en: "synchronize the value bound to '{input}' into @{dx} before the instance: let s = sync(<signal>, <clock of {dx}>), then bind s";
                tr: "'{input}' girişine bağlanan değeri örnekten önce @{dx} alanına senkronize edin: let s = sync(<sinyal>, <{dx} saati>), sonra s'yi bağlayın"
            ),
        };
        let ann_span = at
            .port
            .domain
            .as_ref()
            .map_or(at.port.name.span, |a| a.span);
        let mut diag = Diagnostic::error(
            ErrorCode::E3001,
            lstr!(
                en: "instance '{inst_name}' drives '{target}.{out}', declared @{dx}, from @{dy}";
                tr: "'{inst_name}' örneği @{dx} bildirilen '{target}.{out}' çıkışını @{dy} alanından sürüyor"
            ),
            LabeledSpan::primary(b.span, format!("@{dy}")),
            help,
        )
        .with_secondary(
            ann_span,
            lstr!(en: "'{target}.{out}' is declared @{dx} here";
                  tr: "'{target}.{out}' burada @{dx} bildirildi"),
        )
        .with_secondary(self.domain_span(x), self.defined_here(x))
        .with_secondary(self.domain_span(y), self.defined_here(y))
        .with_note(
            NoteKind::Reason,
            lstr!(
                en: "'{target}.{out}' depends on '{input}' without a register in between; '{target}' has no clock, so the @{dy} value reaches an output declared @{dx} without a synchronizer";
                tr: "'{target}.{out}', araya register girmeden '{input}' girişine bağlı; '{target}' saatsizdir, yani @{dy} değeri @{dx} bildirilen çıkışa senkronizörsüz ulaşır"
            ),
        );
        if let (Some((line, synced)), Some(anchor), Some(value)) = (fix, self.cur_stmt, b.value) {
            // suggestion: e3001_clockless_annotated_output
            let s = Suggestion::line_above(anchor, line, Applicability::MaybeIncorrect)
                .and_replace(self.ast.exprs[value].span, synced);
            diag = diag.with_suggestion(s);
        }
        self.diagnostics.push(diag);
    }

    /// Bağlamanın değerini `dst` alanına taşıyan `let` satırı ve yeni ad:
    /// yalnız tek bitlik, adı yazılabilen değer ve `dst`'nin bu modülde bir
    /// saat portu varken (`sync()` kuralları, K9).
    fn binding_sync_fix(&self, b: &PortBinding, dst: DomainId) -> Option<(String, String)> {
        let e = b.value?;
        let ExprKind::Path(path) = &self.ast.exprs[e].kind else {
            return None;
        };
        let [name] = path.segments.as_slice() else {
            return None;
        };
        if !self.is_one_bit(e) {
            return None;
        }
        let dst = self.resolve_dom(dst);
        let clock = self
            .module_clocks
            .iter()
            .find(|c| self.resolve_dom(c.dom) == dst)?;
        self.cur_stmt?;
        let synced = self.fresh_name(&format!("{}_sync", name.text));
        let line = format!("let {synced} = sync({}, {})", name.text, clock.name);
        Some((line, synced))
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

/// Saatsiz hedefte iki alanın birleştiği çıkış (E3001, örnek biçimi).
struct Merge<'m> {
    target: &'m str,
    output: &'m str,
    deps: &'m BTreeSet<String>,
    /// (giriş, alan, bağlama span'i): ilk belirli alan.
    first: (String, DomainId, Span),
    /// İlkiyle çelişen giriş.
    second: (String, DomainId, Span),
}

/// Saatsiz Volt hedefinin anotasyonlu çıkışı ve özetteki girişleri.
struct Annotated<'m> {
    target: &'m str,
    port: &'m Port,
    /// Anotasyonun alanı (örnek sitesindeki).
    own: DomainId,
    deps: &'m BTreeSet<String>,
}

/// `'a' and 'b'`, `'a', 'b' and 'c'`.
pub(super) fn quote_list(items: &BTreeSet<String>, and: &str) -> String {
    let q: Vec<String> = items.iter().map(|s| format!("'{s}'")).collect();
    match q.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} {and} {last}", init.join(", ")),
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
