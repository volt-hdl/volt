//! Simülasyon kontrat izleyicilerinin çıktısı (ADR-0064): testbench'in
//! `VOLT-CONTRACT-FAIL` / `VOLT-COVER` satırları ayrıştırılır, kimlik
//! (`Modül.inv_0`) derleyicinin `SvaProp` kaydıyla Volt kaynağına
//! eşlenir ve cargo biçimli rapora çevrilir. Rapor BİLEREK İngilizcedir
//! (makine-okur test çıktısı; bkz. `report`).

use std::collections::HashMap;

use volt_span::SourceMap;
use volt_sv_emit::SvaProp;

/// Kontratın kaynak bilgisi (`SvaProp` + kaynak metni).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ContractSource {
    /// Kontrat anahtar kelimesi (`invariant`, `assume`, ...).
    pub keyword: &'static str,
    /// `dosya.volt:satır`.
    pub loc: String,
    /// Kontrat ifadesi (tek satıra indirgenmiş); primitif kontratında
    /// `stdlib AsyncFifo contract 'f_inv_0'` açıklaması.
    pub text: String,
}

/// Bir testteki ilk kontrat ihlali.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ContractViolation {
    /// `Modül.ad` (`Cnt.inv_0`).
    pub id: String,
    /// Test başlangıç reset'inden sonraki çevrim (1'den başlar).
    pub cycle: u64,
    /// Örnek yolu (`dut`, `dut.tx_unit`); `normalize_instance` sonrası.
    pub inst: String,
    /// `for` içindeyse sayaçlar (`i = 3`).
    pub loop_ctx: Option<String>,
    /// Kimliğin kaynak bilgisi; eşlenemediyse None.
    pub source: Option<ContractSource>,
}

impl ContractViolation {
    /// Kontrat bir `requires`/`assume` mı (hangi örnekte olursa olsun)?
    fn is_assume_kind(&self) -> bool {
        self.source
            .as_ref()
            .is_some_and(|s| matches!(s.keyword, "requires" | "assume"))
    }

    /// Uyarıcı (stimulus) hatası mı? Yalnız DUT'un KENDİ `requires`/
    /// `assume`'u testin sürdüğü girdileri kısıtlar. Alt örneğin
    /// varsayımını onu süren üst modül bozar — bu bir tasarım hatasıdır
    /// (ör. tüketici tarafındaki `Handshake<T>` protokol kontratı).
    pub fn is_assumption(&self) -> bool {
        self.is_assume_kind() && self.inst == "dut"
    }

    /// Rapor satırları (cargo biçimli `failures:` bölümü).
    pub fn report_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        let heading = if self.is_assumption() {
            "assumption violated by test stimulus"
        } else {
            "contract violated"
        };
        match &self.source {
            Some(src) => {
                lines.push(format!("  {heading}: {} ({})", src.keyword, src.loc));
                lines.push(format!("    {}: {}", src.keyword, src.text));
            }
            None => lines.push(format!("  {heading}: {}", self.id)),
        }
        lines.push(format!("  at cycle {}", self.cycle));
        lines.push(format!("  in instance: {}", self.inst));
        if let Some(ctx) = &self.loop_ctx {
            lines.push(format!("  loop: {ctx}"));
        }
        if self.is_assumption() {
            lines.push(
                "  note: the test drove the design outside its declared assumptions; \
                 fix the stimulus (or the assumption), not the design"
                    .to_string(),
            );
        } else if self.is_assume_kind() {
            lines.push(
                "  note: this assumption belongs to an instantiated module; \
                 its parent drives those inputs, so the parent broke it"
                    .to_string(),
            );
        }
        lines
    }
}

/// `Cnt.inv_0 cycle=6 [loop=i=3,j=1] inst=TOP.Cnt.u` → ihlal (kaynaksız).
/// Kapsam adı satır sonuna kadar sürer (boşluk içerebilir).
pub(super) fn parse_contract_fail(rest: &str) -> Option<ContractViolation> {
    let (head, inst) = rest.split_once(" inst=")?;
    let mut parts = head.split_whitespace();
    let id = parts.next()?.to_string();
    let cycle = parts.next()?.strip_prefix("cycle=")?.parse().ok()?;
    let mut loop_ctx = None;
    for extra in parts {
        if let Some(vars) = extra.strip_prefix("loop=") {
            loop_ctx = Some(vars.replace('=', " = ").replace(',', ", "));
        }
    }
    Some(ContractViolation {
        id,
        cycle,
        inst: inst.trim_end().to_string(),
        loop_ctx,
        source: None,
    })
}

/// Tek cover'ın toplam tetiklenme sayısı (tüm testler ve örnekler).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoverCount {
    /// `Modül.cov_0`.
    pub id: String,
    /// `dosya.volt:satır`; eşlenemediyse boş.
    pub loc: String,
    pub hits: u64,
}

/// `Cnt.cov_0 12` → (kimlik, sayı).
pub(super) fn parse_cover(rest: &str) -> Option<(String, u64)> {
    let (id, hits) = rest.trim_end().rsplit_once(' ')?;
    Some((id.to_string(), hits.parse().ok()?))
}

/// Testbench stdout'undaki `VOLT-COVER` satırları.
pub(super) fn covers_in(stdout: &str) -> Vec<(String, u64)> {
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix("VOLT-COVER "))
        .filter_map(parse_cover)
        .collect()
}

/// Kimlikten kaynağa eşleme — DUT SV'sini üreten derlemenin kayıtları.
pub(super) struct ContractIndex {
    /// Verilator kapsam öneki (`TOP.<Üst>`) → `dut`.
    top_scope: String,
    by_id: HashMap<String, ContractSource>,
}

impl ContractIndex {
    pub fn new(props: &[SvaProp], map: &SourceMap, top: &str) -> Self {
        let by_id = props
            .iter()
            .map(|p| (format!("{}.{}", p.module_name, p.name), source_of(p, map)))
            .collect();
        Self {
            top_scope: format!("TOP.{top}"),
            by_id,
        }
    }

    /// İhlale kaynak bilgisini ekler ve örnek yolunu `dut.` biçimine çevirir.
    pub fn resolve(&self, mut v: ContractViolation) -> ContractViolation {
        v.source = self.by_id.get(&v.id).cloned();
        v.inst = normalize_instance(&v.inst, &self.top_scope);
        v
    }

    /// Ham cover sayılarını kaynak konumlu kayıtlara çevirir.
    pub fn covers(&self, raw: &[(String, u64)]) -> Vec<CoverCount> {
        raw.iter()
            .map(|(id, hits)| CoverCount {
                id: id.clone(),
                loc: self
                    .by_id
                    .get(id)
                    .map(|s| s.loc.clone())
                    .unwrap_or_default(),
                hits: *hits,
            })
            .collect()
    }
}

/// `TOP.Top` → `dut`, `TOP.Top.u.v` → `dut.u.v`; başka biçim olduğu gibi.
fn normalize_instance(scope: &str, top_scope: &str) -> String {
    match scope.strip_prefix(top_scope) {
        Some("") => "dut".to_string(),
        Some(rest) if rest.starts_with('.') => format!("dut{rest}"),
        _ => scope.to_string(),
    }
}

fn source_of(p: &SvaProp, map: &SourceMap) -> ContractSource {
    let file = map
        .path(p.span.file)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (line, _) = map.line_col(p.span);
    let text = match p.primitive {
        Some(prim) => format!("stdlib {prim} contract '{}'", p.name),
        None => {
            let src = map.source(p.span.file);
            let raw = src
                .get(p.span.start as usize..p.span.end as usize)
                .unwrap_or("?");
            raw.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    };
    ContractSource {
        keyword: p.keyword,
        loc: format!("{file}:{line}"),
        text,
    }
}

/// Grupların cover sayılarını birleştirir: aynı kimlik toplanır, ilk
/// görülme sırası korunur.
pub(super) fn merge_covers(into: &mut Vec<CoverCount>, more: Vec<CoverCount>) {
    for c in more {
        match into.iter_mut().find(|x| x.id == c.id && x.loc == c.loc) {
            Some(x) => x.hits += c.hits,
            None => into.push(c),
        }
    }
}

/// `cover summary:` bölümü; cover yoksa boş.
pub(super) fn cover_summary_lines(covers: &[CoverCount]) -> Vec<String> {
    if covers.is_empty() {
        return Vec::new();
    }
    let labels: Vec<String> = covers
        .iter()
        .map(|c| {
            if c.loc.is_empty() {
                c.id.clone()
            } else {
                format!("{} ({})", c.id, c.loc)
            }
        })
        .collect();
    let width = labels.iter().map(String::len).max().unwrap_or(0);
    let mut lines = vec!["cover summary:".to_string()];
    for (c, label) in covers.iter().zip(&labels) {
        let status = match c.hits {
            0 => "NEVER HIT".to_string(),
            1 => "hit 1 time".to_string(),
            n => format!("hit {n} times"),
        };
        lines.push(format!("  {label:<width$}  {status}"));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_span::Span;

    fn index() -> ContractIndex {
        let mut map = SourceMap::new();
        let text = "module Cnt {\n    invariant: count  <\n   5\n    assume: en\n}\n";
        let file = map.add_file("dir/cnt.volt", text);
        let at = |needle: &str, len: usize| {
            let start = text.find(needle).expect("metin") as u32;
            Span::new(file, start, start + len as u32)
        };
        let prop = |name: &str, keyword, span, primitive| SvaProp {
            module_name: "Cnt".to_string(),
            name: name.to_string(),
            keyword,
            span,
            primitive,
        };
        let props = [
            prop("inv_0", "invariant", at("count", 16), None),
            prop("asm_0", "assume", at("en", 2), None),
            prop("f_cov_0", "cover", at("assume", 6), Some("SyncFifo")),
        ];
        ContractIndex::new(&props, &map, "Cnt")
    }

    #[test]
    fn parse_contract_fail_reads_id_cycle_loop_and_instance() {
        let v = parse_contract_fail("Cnt.inv_0 cycle=47 loop=i=3,j=1 inst=TOP.Cnt.tx unit")
            .expect("ayrışmalı");
        assert_eq!(v.id, "Cnt.inv_0");
        assert_eq!(v.cycle, 47);
        assert_eq!(v.loop_ctx.as_deref(), Some("i = 3, j = 1"));
        assert_eq!(v.inst, "TOP.Cnt.tx unit");
        assert!(parse_contract_fail("Cnt.inv_0 cycle=x inst=TOP.Cnt").is_none());
        assert!(parse_contract_fail("Cnt.inv_0 cycle=3").is_none());
    }

    #[test]
    fn resolve_maps_id_to_source_line_and_one_line_text() {
        let v = parse_contract_fail("Cnt.inv_0 cycle=6 inst=TOP.Cnt").expect("ayrışmalı");
        let v = index().resolve(v);
        assert_eq!(v.inst, "dut");
        let src = v.source.as_ref().expect("eşlenmeli");
        assert_eq!(src.loc, "cnt.volt:2");
        assert_eq!(src.text, "count < 5");
        assert!(!v.is_assumption());
    }

    #[test]
    fn instance_path_is_rewritten_below_the_top_only() {
        assert_eq!(normalize_instance("TOP.Cnt.a.b", "TOP.Cnt"), "dut.a.b");
        assert_eq!(normalize_instance("TOP.Cnt", "TOP.Cnt"), "dut");
        // Önek eşleşmesi tam ad sınırında olmalı.
        assert_eq!(normalize_instance("TOP.Cntx", "TOP.Cnt"), "TOP.Cntx");
    }

    #[test]
    fn assumption_violation_has_its_own_heading_and_note() {
        let v = index().resolve(
            parse_contract_fail("Cnt.asm_0 cycle=9 loop=i=2 inst=TOP.Cnt").expect("ayrışmalı"),
        );
        assert!(v.is_assumption());
        let lines = v.report_lines();
        assert_eq!(
            lines[..5],
            [
                "  assumption violated by test stimulus: assume (cnt.volt:4)",
                "    assume: en",
                "  at cycle 9",
                "  in instance: dut",
                "  loop: i = 2",
            ]
        );
        assert!(lines[5].contains("not the design"));
    }

    #[test]
    fn submodule_assumption_is_a_design_violation() {
        // Alt örneğin girdisini test değil üst modül sürer.
        let v = index()
            .resolve(parse_contract_fail("Cnt.asm_0 cycle=2 inst=TOP.Cnt.u").expect("ayrışmalı"));
        assert!(!v.is_assumption());
        let lines = v.report_lines();
        assert_eq!(lines[0], "  contract violated: assume (cnt.volt:4)");
        assert_eq!(lines[3], "  in instance: dut.u");
        assert!(lines[4].contains("the parent broke it"), "{lines:?}");
    }

    #[test]
    fn unknown_id_is_reported_by_its_raw_name() {
        let v = index()
            .resolve(parse_contract_fail("Other.inv_3 cycle=1 inst=TOP.Cnt").expect("ayrışmalı"));
        assert_eq!(v.report_lines()[0], "  contract violated: Other.inv_3");
    }

    #[test]
    fn primitive_contract_is_described_not_quoted() {
        let covers = index().covers(&[("Cnt.f_cov_0".to_string(), 0)]);
        assert_eq!(covers[0].loc, "cnt.volt:4");
        let src = &index().by_id["Cnt.f_cov_0"];
        assert_eq!(src.text, "stdlib SyncFifo contract 'f_cov_0'");
    }

    #[test]
    fn covers_are_parsed_merged_and_summarised() {
        let raw = covers_in("noise\nVOLT-COVER Cnt.cov_0 12\nVOLT-COVER Cnt.cov_1 0\n");
        assert_eq!(
            raw,
            [("Cnt.cov_0".to_string(), 12), ("Cnt.cov_1".to_string(), 0)]
        );
        let mut all = Vec::new();
        let cover = |id: &str, hits| CoverCount {
            id: id.to_string(),
            loc: "c.volt:3".to_string(),
            hits,
        };
        merge_covers(&mut all, vec![cover("A.cov_0", 2), cover("A.cov_10", 0)]);
        merge_covers(&mut all, vec![cover("A.cov_0", 3)]);
        merge_covers(&mut all, vec![cover("A.cov_1", 1)]);
        assert_eq!(
            cover_summary_lines(&all),
            [
                "cover summary:",
                "  A.cov_0 (c.volt:3)   hit 5 times",
                "  A.cov_10 (c.volt:3)  NEVER HIT",
                "  A.cov_1 (c.volt:3)   hit 1 time",
            ]
        );
        assert!(cover_summary_lines(&[]).is_empty());
        assert_eq!(parse_cover("X.cov_0 nope"), None);
    }
}
