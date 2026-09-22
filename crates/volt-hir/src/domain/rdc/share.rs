//! Tek otomatik reset portunun birden çok saatte paylaşımı (ADR-0065):
//! R5 (async, E3003), R5' (sync, W3010) ve birim kökünde senkron
//! bırakma varsayımı (R1, W3009).

use volt_ast::{ResetSpec, ResetSync};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::facts::{auto_port_name, spec_text, ClockFact, ModuleFacts};
use super::Rdc;

impl Rdc<'_> {
    /// Otomatik port adına göre gruplanan, kullanılan ve ham portla
    /// beslenmeyen saatler; iki ya da daha çok saatli grup paylaşımdır.
    /// E3003 üretildiyse `true` (kök uyarısı gereksizleşir).
    pub(super) fn check_shared_ports(&mut self, m: usize, feeds: &[Option<usize>]) -> bool {
        let facts = self.facts;
        let module = &facts.modules[m];
        let mut groups: Vec<(&'static str, Vec<&ClockFact>)> = Vec::new();
        for (clock, fed) in module.clocks.iter().zip(feeds) {
            let Some(spec) = clock.reset else { continue };
            if fed.is_some() || !clock.used {
                continue;
            }
            let name = auto_port_name(spec);
            match groups.iter_mut().find(|(n, _)| *n == name) {
                Some((_, members)) => members.push(clock),
                None => groups.push((name, vec![clock])),
            }
        }
        let mut error = false;
        for (port, members) in groups.iter().filter(|(_, g)| g.len() > 1) {
            let async_member = members
                .iter()
                .find(|c| c.reset.is_some_and(|s| s.sync == ResetSync::Async));
            match async_member {
                Some(first) => {
                    error = true;
                    let spec = first.reset.unwrap_or(super::facts::DEFAULT_RESET);
                    self.err_shared_async(port, spec, members);
                }
                None => self.warn_shared_sync(port, members),
            }
        }
        error
    }

    /// W3009 — birim kökünde async alanın otomatik portu: bırakmanın
    /// senkron geldiği dış dünyaya bırakılmış bir varsayım. Modül başına bir kez.
    pub(super) fn check_root_contract(&mut self, m: usize, feeds: &[Option<usize>]) {
        let facts = self.facts;
        let module = &facts.modules[m];
        if !module.is_root {
            return;
        }
        let clock = module.clocks.iter().zip(feeds).find(|(c, f)| {
            f.is_none() && c.used && c.reset.is_some_and(|s| s.sync == ResetSync::Async)
        });
        if let Some((clock, _)) = clock {
            self.warn_root_contract(module, clock);
        }
    }

    /// E3003 (ana biçim, R5).
    fn err_shared_async(&mut self, port: &str, spec: ResetSpec, members: &[&ClockFact]) {
        let names = quoted(members);
        let n = members.len();
        let first = members[0];
        let mut diag = Diagnostic::error(
            ErrorCode::E3003,
            lstr!(en: "asynchronous reset '{port}' is shared by {n} clock domains without synchronization";
                  tr: "'{port}' asenkron reset'i {n} saat alanınca senkronizasyonsuz paylaşılıyor"),
            LabeledSpan::primary(
                first.port.name.span,
                lstr!(en: "'{port}' can be released synchronously to at most one of {names}";
                      tr: "'{port}' en fazla birine senkron bırakılabilir: {names}"),
            ),
            raw_port_help(port, spec),
        );
        diag = with_reset_labels(diag, port, members);
        diag = diag
            .with_note(
                NoteKind::Reason,
                lstr!(en: "a reset release that is synchronous to one clock is asynchronous to the \
                           other; registers of the second domain can leave reset in different \
                           cycles or go metastable (recovery/removal violation)";
                      tr: "bir saate senkron reset bırakması diğerine asenkrondur; ikinci alanın \
                           register'ları reset'ten farklı çevrimlerde çıkabilir ya da metastabil \
                           olabilir (recovery/removal ihlali)"),
            )
            .with_note(
                NoteKind::Note,
                lstr!(en: "see ADR-0065 §1"; tr: "bkz. ADR-0065 §1"),
            );
        self.diagnostics.push(diag);
    }

    /// W3010 (R5') — senkron reset paylaşımı; modül başına port başına bir kez.
    fn warn_shared_sync(&mut self, port: &str, members: &[&ClockFact]) {
        let names = quoted(members);
        let first = members[0];
        let spec = first.reset.unwrap_or(super::facts::DEFAULT_RESET);
        let mut diag = Diagnostic::warning(
            ErrorCode::W3010,
            lstr!(en: "synchronous reset '{port}' is sampled by {} clock domains ({names}); its release is asynchronous to at least one of them", members.len();
                  tr: "'{port}' senkron reset'i {} saat alanınca örnekleniyor ({names}); bırakması en az birine asenkron", members.len()),
            LabeledSpan::primary(
                first.port.name.span,
                lstr!(en: "'{port}' is released synchronously to at most one of {names}";
                      tr: "'{port}' en fazla birine senkron bırakılır: {names}"),
            ),
            raw_port_help(port, spec),
        );
        diag = with_reset_labels(diag, port, members);
        diag = diag.with_note(
            NoteKind::Reason,
            lstr!(en: "every flip-flop samples a synchronous reset like data; the release edge of \
                       one clock is asynchronous to the other (ADR-0065 R5'; a warning until the \
                       examples are migrated)";
                  tr: "her flip-flop senkron reset'i veri gibi örnekler; bir saatin bırakma kenarı \
                       diğerine asenkrondur (ADR-0065 R5'; örnekler taşınana kadar uyarı)"),
        );
        self.diagnostics.push(diag);
    }

    /// W3009 (R1, birim kökü).
    fn warn_root_contract(&mut self, module: &ModuleFacts, clock: &ClockFact) {
        let spec = clock.reset.unwrap_or(super::facts::DEFAULT_RESET);
        let port = auto_port_name(spec);
        let (clk, top) = (&clock.port.name.text, &module.decl.name.text);
        let diag = Diagnostic::warning(
            ErrorCode::W3009,
            lstr!(en: "asynchronous reset '{port}' is assumed to be released synchronously to '{clk}'";
                  tr: "'{port}' asenkron reset'inin '{clk}' saatine senkron bırakıldığı varsayılıyor"),
            LabeledSpan::primary(
                clock.port.name.span,
                lstr!(en: "'{port}' must be released synchronously to this clock";
                      tr: "'{port}' bu saate senkron bırakılmalı"),
            ),
            lstr!(en: "if '{port}' comes from a pad or a power-on reset, declare it as raw and the \
                       compiler adds the synchronizer:\n    in {port} : {}", spec_text(spec);
                  tr: "'{port}' bir pad'den ya da güç açılış reset'inden geliyorsa ham port olarak \
                       bildirin, derleyici senkronizörü ekler:\n    in {port} : {}", spec_text(spec)),
        )
        .with_secondary(
            clock.reset_span,
            lstr!(en: "asynchronous reset → port '{port}'"; tr: "asenkron reset → '{port}' portu"),
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "'{top}' is not instantiated in this unit, so nothing in Volt synchronizes its \
                       reset release (ADR-0065 §1)";
                  tr: "'{top}' bu birimde örneklenmiyor; reset bırakmasını Volt'ta hiçbir şey \
                       senkronlamıyor (ADR-0065 §1)"),
        );
        self.diagnostics.push(diag);
    }
}

/// E3003/W3010 ortak help'i: ham portu açıkça al.
fn raw_port_help(port: &str, spec: ResetSpec) -> String {
    let decl = spec_text(spec);
    lstr!(en: "take the raw reset in explicitly; the compiler then synchronizes its release to \
               each clock with a two-stage synchronizer:\n    in {port} : {decl}";
          tr: "ham reset'i açıkça alın; derleyici bırakmasını her saate iki aşamalı bir \
               senkronizörle senkronlar:\n    in {port} : {decl}")
}

/// Reset alanı etiketleri (aynı alan iki saatte ise tek etiket) ve
/// ikinci saatlerin port etiketleri.
fn with_reset_labels(mut diag: Diagnostic, port: &str, members: &[&ClockFact]) -> Diagnostic {
    let mut seen = Vec::new();
    for (i, clock) in members.iter().enumerate() {
        // Örtük alanın (K2) reset'i yazılmamıştır: etiket port adına düşer.
        let implicit = clock.reset_span == clock.port.name.span;
        if !implicit && !seen.contains(&clock.reset_span) {
            seen.push(clock.reset_span);
            let label = match (i, clock.reset.map(|s| s.sync)) {
                (0, Some(ResetSync::Async)) => {
                    lstr!(en: "asynchronous reset → port '{port}'"; tr: "asenkron reset → '{port}' portu")
                }
                (0, _) => lstr!(en: "reset → port '{port}'"; tr: "reset → '{port}' portu"),
                _ => lstr!(en: "same port '{port}'"; tr: "aynı '{port}' portu"),
            };
            diag = diag.with_secondary(clock.reset_span, label);
        }
        if i > 0 {
            diag = diag.with_secondary(
                clock.port.name.span,
                lstr!(en: "also reset by '{port}'"; tr: "bu saat de '{port}' ile sıfırlanıyor"),
            );
        }
    }
    diag
}

fn quoted(members: &[&ClockFact]) -> String {
    members
        .iter()
        .map(|c| format!("'{}'", c.port.name.text))
        .collect::<Vec<_>>()
        .join(", ")
}
