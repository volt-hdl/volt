//! Sürücü analizi (docs/spec/type-inference.md §11, ADR-0073).
//!
//! Tip kontrolüyle aynı geçişte doldurulur: her sinyalin hangi
//! kaynaklarca sürüldüğü kaydedilir. Aynı `on`/`comb` bloğu içindeki
//! atamalar TEK sürücü sayılır (§11.2 istisnası); modül seviyesindeki
//! her sürekli atama ayrı sürücüdür.
//!
//! ADR-0073: çift sürücü denetimi yalnız burada yapılır ve her sürücü
//! türünü kapsar — atama, `let` başlangıç değeri, üst modülün sürdüğü
//! giriş portu ve alt modülün inout/opendrain portuyla paylaşılan hat.
//! Kısmi (bit/aralık/eleman) hedefler bit aralığı taşır; iki farklı
//! gruptaki sürücü ancak aralıkları kesişirse çakışır. Aralığı derleme
//! zamanında bilinmeyen hedef bütün sinyali sürüyor sayılır.

use std::collections::HashMap;

use volt_ast::{GenerateInfo, ModuleDecl, PortDir};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{DefId, DefKind, ResolveResult};

/// Sinyali süren kaynağın türü (ADR-0073 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriverKind {
    /// `=`/`<=` ataması (modül seviyesi, `comb` veya `on`).
    Assign,
    /// `let v = e` — başlangıç değeri sürekli bir sürücüdür.
    LetInit,
    /// Giriş portu: üst modül sürer (konum port bildirimidir).
    ParentInput,
    /// Alt modülün inout/opendrain portuna bağlı hat: üç durumlu
    /// sürücü. Birbiriyle uyumludur, push-pull sürücüyle çakışır.
    SharedLine { instance: String, port: String },
}

#[derive(Debug, Clone)]
struct DriverRecord {
    span: Span,
    /// Aynı gruba (aynı on/comb bloğu) ait sürücüler tek sürücüdür.
    group: u32,
    kind: DriverKind,
    /// Sürülen bitler `[lo, hi)`; `None` bütün sinyal (ya da derleme
    /// zamanında bilinmeyen aralık).
    bits: Option<(u32, u32)>,
    /// Bit/aralık/eleman hedefi — E4002 için yine "sürülmüş" sayılır.
    partial: bool,
}

impl DriverRecord {
    fn overlaps(&self, other: &DriverRecord) -> bool {
        match (self.bits, other.bits) {
            (Some((a_lo, a_hi)), Some((b_lo, b_hi))) => a_lo < b_hi && b_lo < a_hi,
            _ => true,
        }
    }

    /// İki kayıt aynı donanım bitini iki ayrı kaynaktan mı sürüyor?
    fn conflicts_with(&self, other: &DriverRecord) -> bool {
        let both_shared = matches!(self.kind, DriverKind::SharedLine { .. })
            && matches!(other.kind, DriverKind::SharedLine { .. });
        self.group != other.group && !both_shared && self.overlaps(other)
    }
}

/// Sinyal → onu süren kaynakların kaydı.
#[derive(Debug, Default)]
pub struct DriverTable {
    drivers: HashMap<DefId, Vec<DriverRecord>>,
}

impl DriverTable {
    /// Atama kaydı; `bits` kısmi hedefin bilinen aralığıdır.
    pub fn record(
        &mut self,
        target: DefId,
        span: Span,
        group: u32,
        partial: bool,
        bits: Option<(u32, u32)>,
    ) {
        self.push(target, span, group, DriverKind::Assign, partial, bits);
    }

    /// Atama dışı sürücü kaydı (`let` başlangıcı, giriş portu, paylaşılan hat).
    pub fn record_kind(&mut self, target: DefId, span: Span, group: u32, kind: DriverKind) {
        self.push(target, span, group, kind, false, None);
    }

    fn push(
        &mut self,
        target: DefId,
        span: Span,
        group: u32,
        kind: DriverKind,
        partial: bool,
        bits: Option<(u32, u32)>,
    ) {
        self.drivers.entry(target).or_default().push(DriverRecord {
            span,
            group,
            kind,
            bits,
            partial,
        });
    }

    /// Kullanıcı ataması var mı (E4002, W4001/W4002 yalnız atamalara bakar).
    pub fn is_driven(&self, def: DefId) -> bool {
        self.drivers
            .get(&def)
            .is_some_and(|rs| rs.iter().any(|r| r.kind == DriverKind::Assign))
    }

    /// E4001 — aynı biti süren iki ayrı kaynak. Sinyal başına ilk
    /// çakışan çift raporlanır: sonraki birincil, önceki ikincil etiket.
    /// Ad, `for` açılımında üretilmişse kaynaktaki biçimiyle gösterilir
    /// (`t_0` → `t`); özdeş kopyalar ADR-0068 katlamasıyla tekilleşir.
    pub fn check_multiple_drivers(
        &self,
        res: &ResolveResult,
        generate: &GenerateInfo,
        out: &mut Vec<Diagnostic>,
    ) {
        let mut defs: Vec<&DefId> = self.drivers.keys().collect();
        defs.sort_by_key(|d| d.0);
        for &def in defs {
            let records = &self.drivers[&def];
            let pair = (1..records.len()).find_map(|j| {
                records[..j]
                    .iter()
                    .find(|r| r.conflicts_with(&records[j]))
                    .map(|first| (first, &records[j]))
            });
            if let Some((first, second)) = pair {
                let data = &res.defs[def.0 as usize];
                let name = generate.source_name(data.span, &data.name);
                out.push(double_driver(name, first, second));
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
                        lstr!(
                            en: "output port '{}' is not driven", port.name.text;
                            tr: "'{}' çıkış portu sürülmüyor", port.name.text
                        ),
                        LabeledSpan::primary(
                            port.span,
                            lstr!(en: "this port is never assigned"; tr: "bu porta hiç atama yok"),
                        ),
                        lstr!(
                            en: "add an assignment like {} = ...", port.name.text;
                            tr: "{} = ... şeklinde bir atama ekleyin", port.name.text
                        ),
                    )
                    .with_note(
                        NoteKind::Reason,
                        lstr!(
                            en: "an undriven output produces high impedance (Z) in SystemVerilog";
                            tr: "sürücüsüz çıkış SystemVerilog'da yüksek empedans (Z) üretir"
                        ),
                    ),
                );
            }
        }
    }

    /// W4001/W4002 — atanan ama hiç okunmayan sinyaller (§11.6 adım 3).
    pub fn check_write_only(&self, res: &ResolveResult, out: &mut Vec<Diagnostic>) {
        let mut defs: Vec<&DefId> = self.drivers.keys().collect();
        defs.sort_by_key(|d| d.0);
        for &def in defs {
            let data = &res.defs[def.0 as usize];
            if !self.is_driven(def) || data.name.starts_with('_') || res.reads.contains(&def) {
                continue;
            }
            let (code, what) = match data.kind {
                DefKind::Register => (
                    ErrorCode::W4002,
                    lstr!(
                        en: "register written but never read";
                        tr: "yazılıp hiç okunmayan register"
                    ),
                ),
                DefKind::Wire | DefKind::LocalBinding => (
                    ErrorCode::W4001,
                    lstr!(
                        en: "unused signal: driven but never read";
                        tr: "kullanılmayan sinyal: sürülüyor ama hiç okunmuyor"
                    ),
                ),
                _ => continue,
            };
            out.push(Diagnostic::warning(
                code,
                format!("{what}: '{}'", data.name),
                LabeledSpan::primary(data.span, lstr!(en: "defined here"; tr: "burada tanımlı")),
                lstr!(
                    en: "add a '_' prefix to silence: _{}", data.name;
                    tr: "'_' öneki ile susturabilirsiniz: _{}", data.name
                ),
            ));
        }
    }
}

/// E4001 tanısı: beş parça (kod, iki konum, açıklama, öneri, gerekçe +
/// spec/ADR atfı). Öneri ve etiketler sürücü türlerine göre seçilir.
fn double_driver(name: &str, first: &DriverRecord, second: &DriverRecord) -> Diagnostic {
    let mut diag = Diagnostic::error(
        ErrorCode::E4001,
        lstr!(en: "'{name}' is already driven"; tr: "'{name}' zaten sürülüyor"),
        LabeledSpan::primary(second.span, primary_label(second)),
        help(name, first, second),
    )
    .with_secondary(first.span, secondary_label(first));
    if (first.span.start, first.span.end) == (second.span.start, second.span.end) {
        diag = diag.with_note(
            NoteKind::Note,
            lstr!(en: "each iteration of the unrolled 'for' loop drives '{name}' again; index the target with the loop variable";
                  tr: "açılan 'for' döngüsünün her yinelemesi '{name}' sinyalini yeniden sürüyor; hedefi döngü değişkeniyle indeksleyin"),
        );
    }
    if let (Some((a_lo, a_hi)), Some((b_lo, b_hi))) = (first.bits, second.bits) {
        let (lo, hi) = (a_lo.max(b_lo), a_hi.min(b_hi) - 1);
        diag = diag.with_note(
            NoteKind::Note,
            lstr!(en: "bits {lo}..={hi} of '{name}' are driven by both";
                  tr: "'{name}' sinyalinin {lo}..={hi} bitlerini ikisi de sürüyor"),
        );
    }
    diag.with_note(
        NoteKind::Reason,
        lstr!(
            en: "in hardware, two sources cannot drive the same signal at once (type-inference.md §11, ADR-0073)";
            tr: "donanımda bir sinyali iki kaynak aynı anda süremez (type-inference.md §11, ADR-0073)"
        ),
    )
}

fn primary_label(r: &DriverRecord) -> String {
    match &r.kind {
        DriverKind::SharedLine { instance, port } => lstr!(
            en: "'{instance}.{port}' also drives this line"; tr: "'{instance}.{port}' de bu hattı sürüyor"
        ),
        DriverKind::LetInit => {
            lstr!(en: "the 'let' initializer drives it here"; tr: "'let' başlangıç değeri burada sürüyor")
        }
        _ if r.partial => {
            lstr!(en: "second driver of these bits here"; tr: "bu bitlerin ikinci sürücüsü burada")
        }
        _ => lstr!(en: "second driver here"; tr: "ikinci sürücü burada"),
    }
}

fn secondary_label(r: &DriverRecord) -> String {
    match &r.kind {
        DriverKind::LetInit => lstr!(
            en: "the 'let' initializer already drives it"; tr: "'let' başlangıç değeri zaten sürüyor"
        ),
        DriverKind::ParentInput => lstr!(
            en: "input port: the instantiating module drives it"; tr: "giriş portu: örnekleyen üst modül sürüyor"
        ),
        DriverKind::SharedLine { instance, port } => lstr!(
            en: "shared with the tri-state port '{instance}.{port}' here"; tr: "üç durumlu '{instance}.{port}' portuyla burada paylaşılıyor"
        ),
        DriverKind::Assign => lstr!(en: "first assignment here"; tr: "ilk atama burada"),
    }
}

fn help(name: &str, first: &DriverRecord, second: &DriverRecord) -> String {
    let kinds = [&first.kind, &second.kind];
    if kinds.contains(&&DriverKind::ParentInput) {
        return lstr!(
            en: "an input port is read-only inside the module; drive an internal 'wire' instead";
            tr: "giriş portu modül içinde salt okunurdur; bunun yerine iç bir 'wire' sürün"
        );
    }
    if kinds
        .iter()
        .any(|k| matches!(k, DriverKind::SharedLine { .. }))
    {
        return lstr!(
            en: "a line shared with an inout/opendrain port is driven only through that port (drive/release); remove the direct assignment to '{name}'";
            tr: "inout/opendrain portla paylaşılan hat yalnız o port üzerinden (drive/release) sürülür; '{name}' ataması kaldırılmalı"
        );
    }
    if kinds.contains(&&DriverKind::LetInit) {
        return lstr!(
            en: "a 'let' is driven once by its initializer; declare 'wire {name} : T' and assign it once, or fold the choice into the initializer";
            tr: "'let' başlangıç değeriyle bir kez sürülür; 'wire {name} : T' bildirip tek atama yapın ya da seçimi başlangıç ifadesine katın"
        );
    }
    if first.partial || second.partial {
        return lstr!(
            en: "drive disjoint bits from each source, or assign '{name}' in a single block";
            tr: "her kaynaktan ayrık bitleri sürün ya da '{name}' atamalarını tek blokta yapın"
        );
    }
    lstr!(
        en: "use a single assignment or write a conditional expression";
        tr: "tek bir atama kullanın veya koşullu ifade yazın"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_span::FileId;

    fn rec(group: u32, kind: DriverKind, bits: Option<(u32, u32)>) -> DriverRecord {
        DriverRecord {
            span: Span::new(FileId(0), group, group + 1),
            group,
            kind,
            bits,
            partial: bits.is_some(),
        }
    }

    #[test]
    fn only_overlapping_bits_from_different_groups_conflict() {
        let low = rec(1, DriverKind::Assign, Some((0, 4)));
        let high = rec(2, DriverKind::Assign, Some((4, 8)));
        let mid = rec(3, DriverKind::Assign, Some((3, 5)));
        let whole = rec(4, DriverKind::Assign, None);
        assert!(!low.conflicts_with(&high), "komşu aralıklar ayrık");
        assert!(low.conflicts_with(&mid) && high.conflicts_with(&mid));
        assert!(whole.conflicts_with(&low), "bilinmeyen aralık bütün sinyal");
        let same_block = rec(1, DriverKind::Assign, Some((0, 8)));
        assert!(!low.conflicts_with(&same_block), "aynı blok tek sürücü");
    }

    #[test]
    fn shared_lines_are_compatible_only_with_each_other() {
        let line = |g| {
            rec(
                g,
                DriverKind::SharedLine {
                    instance: "p".into(),
                    port: "dq".into(),
                },
                None,
            )
        };
        assert!(!line(1).conflicts_with(&line(2)));
        assert!(line(1).conflicts_with(&rec(3, DriverKind::Assign, None)));
        assert!(rec(3, DriverKind::LetInit, None).conflicts_with(&line(1)));
    }

    #[test]
    fn let_initializers_and_parent_inputs_are_not_user_assignments() {
        let mut table = DriverTable::default();
        let span = Span::new(FileId(0), 0, 1);
        table.record_kind(DefId(1), span, 1, DriverKind::LetInit);
        table.record_kind(DefId(2), span, 2, DriverKind::ParentInput);
        assert!(!table.is_driven(DefId(1)) && !table.is_driven(DefId(2)));
        table.record(DefId(2), span, 3, false, None);
        assert!(table.is_driven(DefId(2)));
    }
}
