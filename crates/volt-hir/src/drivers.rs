//! Sürücü analizi (docs/spec/type-inference.md §11).
//!
//! Tip kontrolüyle aynı geçişte doldurulur: her sinyalin hangi
//! atamalarca sürüldüğü kaydedilir. Aynı `on`/`comb` bloğu içindeki
//! koşullu atamalar TEK sürücü sayılır (§11.2 istisnası); modül
//! seviyesindeki her sürekli atama ayrı sürücüdür. Bit/aralık/alan
//! hedefli kısmi atamalar E4001'e girmez ama sinyali "sürülmüş" sayar.

use std::collections::HashMap;

use volt_ast::{ModuleDecl, PortDir};
use volt_diagnostics::{Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{DefId, DefKind, ResolveResult};

#[derive(Debug, Clone, Copy)]
struct DriverRecord {
    span: Span,
    /// Aynı gruba (aynı on/comb bloğu) ait atamalar tek sürücüdür.
    group: u32,
    /// Bit/aralık/alan hedefi — sinyalin tamamını sürmez.
    partial: bool,
}

/// Sinyal → onu süren atamaların kaydı.
#[derive(Debug, Default)]
pub struct DriverTable {
    drivers: HashMap<DefId, Vec<DriverRecord>>,
}

impl DriverTable {
    pub fn record(&mut self, target: DefId, span: Span, group: u32, partial: bool) {
        let records = self.drivers.entry(target).or_default();
        // Aynı blok içindeki ikinci tam atama yeni sürücü DEĞİL (§11.2).
        if !partial && records.iter().any(|r| !r.partial && r.group == group) {
            return;
        }
        records.push(DriverRecord {
            span,
            group,
            partial,
        });
    }

    pub fn is_driven(&self, def: DefId) -> bool {
        self.drivers.contains_key(&def)
    }

    /// E4001 — aynı sinyali süren birden fazla tam sürücü.
    pub fn check_multiple_drivers(&self, res: &ResolveResult, out: &mut Vec<Diagnostic>) {
        let mut defs: Vec<&DefId> = self.drivers.keys().collect();
        defs.sort_by_key(|d| d.0);
        for &def in defs {
            let full: Vec<&DriverRecord> =
                self.drivers[&def].iter().filter(|r| !r.partial).collect();
            if full.len() > 1 {
                let name = &res.defs[def.0 as usize].name;
                out.push(
                    Diagnostic::error(
                        ErrorCode::E4001,
                        format!("'{name}' zaten sürülüyor"),
                        LabeledSpan::primary(full[1].span, "ikinci sürücü burada"),
                        "tek bir atama kullanın veya koşullu ifade yazın",
                    )
                    .with_secondary(full[0].span, "ilk atama burada")
                    .with_note(
                        NoteKind::Reason,
                        "donanımda bir sinyali iki kaynak aynı anda süremez",
                    ),
                );
            }
        }
    }

    /// E4002 — sürücüsüz çıkış portu.
    pub fn check_undriven_outputs(
        &self,
        module: &ModuleDecl,
        res: &ResolveResult,
        out: &mut Vec<Diagnostic>,
    ) {
        for port in module.ports.iter().filter(|p| p.direction == PortDir::Out) {
            let Some(&def) = res.decl_spans.get(&port.name.span) else {
                continue;
            };
            if !self.is_driven(def) {
                out.push(
                    Diagnostic::error(
                        ErrorCode::E4002,
                        format!("'{}' çıkış portu sürülmüyor", port.name.text),
                        LabeledSpan::primary(port.span, "bu porta hiç atama yok"),
                        format!("{} = ... şeklinde bir atama ekleyin", port.name.text),
                    )
                    .with_note(
                        NoteKind::Reason,
                        "sürücüsüz çıkış SystemVerilog'da yüksek empedans (Z) üretir",
                    ),
                );
            }
        }
    }

    /// W4001/W4002 — sürülen ama hiç okunmayan sinyaller (§11.6 adım 3).
    pub fn check_write_only(&self, res: &ResolveResult, out: &mut Vec<Diagnostic>) {
        let mut defs: Vec<&DefId> = self.drivers.keys().collect();
        defs.sort_by_key(|d| d.0);
        for &def in defs {
            let data = &res.defs[def.0 as usize];
            if data.name.starts_with('_') || res.reads.contains(&def) {
                continue;
            }
            let (code, what) = match data.kind {
                DefKind::Register => (ErrorCode::W4002, "yazılıp hiç okunmayan register"),
                DefKind::Wire | DefKind::LocalBinding => (
                    ErrorCode::W4001,
                    "kullanılmayan sinyal: sürülüyor ama hiç okunmuyor",
                ),
                _ => continue,
            };
            out.push(Diagnostic::warning(
                code,
                format!("{what}: '{}'", data.name),
                LabeledSpan::primary(data.span, "burada tanımlı"),
                format!("'_' öneki ile susturabilirsiniz: _{}", data.name),
            ));
        }
    }
}
