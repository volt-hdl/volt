//! Reset yakınsaması (R6, ADR-0065): aynı ham reset bir saate iki ayrı
//! zincirle senkronlanırsa iki bırakma farklı çevrime düşebilir.
//! Örnekleme ağacında (saat, ham reset) çifti başına en fazla bir zincir.
//!
//! `release_clocks(m, r)`: `m` modülünün `r` ham portunun, `m`'nin alt
//! ağacında bırakıldığı saat portları (tekilleştirilmiş): yerel zincir
//! ile ham portu `r`'ye bağlanan her örneğin kendi sonucunun ebeveyn
//! saatine eşlenmesi. Bir saat iki kaynaktan gelirse E3003 o modülde,
//! bir kez raporlanır; üst modüle tekilleştirilmiş küme çıkar.
//!
//! Örneğin saati ebeveynde bir saat portu ya da bir örneğin çıkışı (PLL,
//! saat bölücü) olabilir (#371): ikincisi de bir saat ağıdır ve aynı
//! çıkışa bağlı iki zincir yakınsar. Üst modüle yalnız port saatleri çıkar;
//! bir örneğin çıkışı ebeveynin dışında görünmez.

use volt_ast::{ExprKind, InstanceDecl, PortBinding, SourceFile, StmtKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::facts::{contains, feeds_of, reset_free_bindings, ModuleFacts};
use super::Rdc;
use crate::resolve::{DefId, DefKind};

/// Bir zincirin saati, modülün gözünden.
#[derive(Clone, PartialEq, Eq)]
enum ClockNet {
    /// Modülün saat portu.
    Port(DefId),
    /// Bir örneğin çıkışı (`p.clk_o`): örnek ve port adı.
    Output(DefId, String),
}

/// Bir bırakma senkronizörünün yeri.
#[derive(Clone, Copy)]
enum Site<'a> {
    /// Modülün kendi zinciri.
    Local,
    /// Ham portu bu bağlamayla beslenen örneğin alt ağacı.
    Instance(&'a InstanceDecl, Span),
}

impl<'a> Rdc<'a> {
    pub(super) fn release_clocks(&mut self, m: usize, raw: DefId) -> Vec<DefId> {
        if let Some(done) = self.release_memo.get(&(m, raw)) {
            return done.clone();
        }
        // Döngü koruması: özyinelemeli örnekleme zaten başka tanı üretir.
        self.release_memo.insert((m, raw), Vec::new());
        let facts = self.facts;
        let module = &facts.modules[m];
        let mut sites: Vec<(ClockNet, Site<'a>)> = Vec::new();
        if let Some(ri) = module.raws.iter().position(|r| r.def == raw) {
            for (clock, fed) in module.clocks.iter().zip(feeds_of(module)) {
                if fed == Some(ri) && self.chain_used(module, clock.def) {
                    sites.push((ClockNet::Port(clock.def), Site::Local));
                }
            }
        }
        for inst in instances(facts.ast, module) {
            self.instance_sites(module, inst, raw, &mut sites);
        }

        let mut nets: Vec<&ClockNet> = Vec::new();
        for (net, _) in &sites {
            if nets.contains(&net) {
                continue;
            }
            nets.push(net);
            let at: Vec<Site> = sites
                .iter()
                .filter(|(n, _)| n == net)
                .map(|&(_, s)| s)
                .collect();
            if at.len() > 1 {
                self.err_converge(module, raw, net, &at);
            }
        }
        // Üst modüle yalnız port saatleri çıkar.
        let clocks: Vec<DefId> = nets
            .into_iter()
            .filter_map(|n| match n {
                ClockNet::Port(d) => Some(*d),
                ClockNet::Output(..) => None,
            })
            .collect();
        self.release_memo.insert((m, raw), clocks.clone());
        clocks
    }

    /// Örneğin `raw`'a bağlı ham portlarının bırakma saatleri, ebeveynin
    /// saat ağına (saat portu ya da bir örneğin çıkışı) eşlenerek
    /// `sites`'e eklenir.
    fn instance_sites(
        &mut self,
        module: &ModuleFacts,
        inst: &'a InstanceDecl,
        raw: DefId,
        sites: &mut Vec<(ClockNet, Site<'a>)>,
    ) {
        let facts = self.facts;
        let Some(target) = self.target_of(inst) else {
            return;
        };
        let child = &facts.modules[target];
        for b in &inst.bindings {
            if self.bound_def(b) != Some(raw) {
                continue;
            }
            let Some(child_raw) = child
                .raws
                .iter()
                .find(|r| r.port.name.text == b.port_name.text)
            else {
                continue;
            };
            for child_clock in self.release_clocks(target, child_raw.def) {
                let Some(port) = child.clocks.iter().find(|c| c.def == child_clock) else {
                    continue;
                };
                let parent = inst
                    .bindings
                    .iter()
                    .find(|cb| cb.port_name.text == port.port.name.text)
                    .and_then(|cb| self.clock_net(module, cb));
                if let Some(parent) = parent {
                    sites.push((parent, Site::Instance(inst, b.span)));
                }
            }
        }
    }

    /// Modülün kendi zinciri bir şeyi sıfırlıyor mu: saat, kullanıcı
    /// örneği bağlaması ve reset taşımayan bağlama
    /// (`AsyncDualPortRam.wr_clk`, extern örneği) dışında da kullanılıyor (flop, `sync()`,
    /// yerleşik primitif) ya da bir çocuğun OTOMATİK reset'li saatine bağlı (zincir
    /// çıkışı o porta gider). Yalnız ham port geçiren ara seviyenin
    /// zinciri ölü mantıktır, yakınsama sayılmaz.
    fn chain_used(&self, module: &ModuleFacts, clock: DefId) -> bool {
        let facts = self.facts;
        let insts = instances(facts.ast, module);
        let mut skipped: Vec<Span> = insts
            .iter()
            .filter(|i| self.target_of(i).is_some())
            .flat_map(|i| i.bindings.iter().map(|b| b.span))
            .collect();
        skipped.extend(reset_free_bindings(facts.ast, self.res, module.decl));
        let inside = |s: &Span| skipped.iter().any(|b| contains(b, s));
        if self
            .res
            .use_spans
            .iter()
            .any(|(s, d)| *d == clock && !inside(s))
        {
            return true;
        }
        insts.iter().any(|inst| {
            let Some(child) = self.target_of(inst).map(|t| &facts.modules[t]) else {
                return false;
            };
            let feeds = feeds_of(child);
            inst.bindings
                .iter()
                .filter(|b| self.bound_def(b) == Some(clock))
                .any(|b| {
                    child.clocks.iter().zip(&feeds).any(|(c, f)| {
                        c.port.name.text == b.port_name.text && c.reset.is_some() && f.is_none()
                    })
                })
        })
    }

    /// Kullanıcı modülü örneğinin hedefi (`modules` indeksi).
    fn target_of(&self, inst: &InstanceDecl) -> Option<usize> {
        self.res
            .decl_spans
            .get(&inst.name.span)
            .and_then(|d| self.res.instance_module.get(d))
            .and_then(|t| self.facts.by_def.get(t).copied())
    }

    /// Saat bağlamasının ebeveyndeki ağı: modülün saat portu ya da bir
    /// örneğin çıkışı (`p.clk_o`, #371). Başka bir değer (sabit, ifade) bir
    /// saat ağı değildir; saat tipi denetimi onu zaten reddeder.
    fn clock_net(&self, module: &ModuleFacts, cb: &PortBinding) -> Option<ClockNet> {
        if let ExprKind::Field { base, field } = &self.facts.ast.exprs[cb.value].kind {
            let inst = *self.res.resolutions.get(base)?;
            return matches!(self.res.def_kind(inst), DefKind::Instance)
                .then(|| ClockNet::Output(inst, field.text.clone()));
        }
        let def = self.bound_def(cb)?;
        module
            .clocks
            .iter()
            .any(|c| c.def == def)
            .then_some(ClockNet::Port(def))
    }

    /// Bağlamanın değeri olan yerel tanım (`port: ad`; `port` kısayolu
    /// ayrıştırıcıda `port: port`'a açılmıştır).
    fn bound_def(&self, b: &PortBinding) -> Option<DefId> {
        self.res.resolutions.get(&b.value).copied()
    }

    /// E3003 (yakınsama biçimi, R6).
    fn err_converge(&mut self, module: &ModuleFacts, raw: DefId, clock: &ClockNet, at: &[Site]) {
        let raw_name = module.raws.iter().find(|r| r.def == raw).map_or_else(
            || self.res.defs[raw.0 as usize].name.clone(),
            |r| r.port.name.text.clone(),
        );
        let clk = match clock {
            ClockNet::Port(d) => self.res.defs[d.0 as usize].name.clone(),
            ClockNet::Output(inst, port) => {
                format!("{}.{port}", self.res.defs[inst.0 as usize].name)
            }
        };
        let top = &module.decl.name.text;
        let where_ = |s: &Site| match s {
            Site::Local => lstr!(en: "in '{top}'"; tr: "'{top}' içinde"),
            Site::Instance(i, _) => {
                lstr!(en: "in instance '{}'", i.name.text; tr: "'{}' örneğinde", i.name.text)
            }
        };
        let (first, second) = (where_(&at[0]), where_(&at[1]));
        let primary = match at[1] {
            Site::Instance(_, span) => span,
            Site::Local => module.decl.name.span,
        };
        let mut diag = Diagnostic::error(
            ErrorCode::E3003,
            lstr!(en: "raw reset '{raw_name}' is synchronized twice on '{clk}': {first} and {second}";
                  tr: "'{raw_name}' ham reset'i '{clk}' saatinde iki kez senkronlanıyor: {first} ve {second}"),
            LabeledSpan::primary(
                primary,
                lstr!(en: "second synchronizer of '{raw_name}' on '{clk}'";
                      tr: "'{clk}' üzerinde '{raw_name}' için ikinci senkronizör"),
            ),
            lstr!(en: "remove the child's raw reset port and its binding; the parent's synchronized \
                       reset then reaches the child through its automatic reset port";
                  tr: "çocuğun ham reset portunu ve bağlamasını kaldırın; ebeveynin senkronlanmış \
                       reset'i çocuğa otomatik reset portundan ulaşır"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "two synchronizers release independently: the same reset can let the two \
                       register groups leave reset in different cycles";
                  tr: "iki senkronizör bağımsız bırakır: aynı reset iki register grubunu \
                       reset'ten farklı çevrimlerde çıkarabilir"),
        );
        if let Site::Instance(_, span) = at[0] {
            diag = diag.with_secondary(
                span,
                lstr!(en: "first synchronizer of '{raw_name}' on '{clk}'";
                      tr: "'{clk}' üzerinde '{raw_name}' için ilk senkronizör"),
            );
        } else if let Some(c) = module
            .clocks
            .iter()
            .find(|c| *clock == ClockNet::Port(c.def))
        {
            diag = diag.with_secondary(
                c.port.name.span,
                lstr!(en: "'{top}' synchronizes '{raw_name}' to this clock";
                      tr: "'{top}' '{raw_name}' reset'ini bu saate senkronlar"),
            );
        }
        self.diagnostics.push(diag);
    }
}

/// Modül gövdesinin (üst seviye; `for` parser'da açılır) örnekleri.
fn instances<'a>(ast: &'a SourceFile, module: &ModuleFacts<'a>) -> Vec<&'a InstanceDecl> {
    module
        .decl
        .body
        .iter()
        .filter_map(|&s| {
            if let StmtKind::Instance(inst) = &ast.stmts[s].kind {
                Some(inst)
            } else {
                None
            }
        })
        .collect()
}
