//! Handshake<T> protokol denetimi (ADR-0050): E4007.
//!
//! Üretici tarafta (`out tx : Handshake<T>`) `tx.valid`, `tx.ready`'ye
//! KOMBİNASYONEL bağımlı olamaz. AXI'nin (A3.3.1) ve Volt Handshake
//! kontratının temel kuralı: üretici valid'i yükseltmek için ready'yi
//! beklemez — iki taraf da karşısını beklerse el sıkışma hiç
//! tamamlanmaz (kilitlenme). Tüketicinin ready'si valid'e bağlı
//! olabilir; bu yüzden yalnız üretici portları denetlenir.
//!
//! Denetim modül gövdesindeki sürekli atamaları (`x = e`), `let`
//! bağlarını ve `comb` bloklarını (koşullar dâhil) bir bağımlılık
//! grafiğine çevirir; `on` blokları register olduğundan yolu keser.
//! Örnek çıkışı (`inst.port`) modül özetiyle açılır (ADR-0114): yol alt
//! modülün, saatsiz extern'in (en kötü durum) ve yerleşik primitifin
//! içinden geçer; saatli extern opaktır. Graf alan çıkarımıyla ortak
//! kurucudan gelir ([`crate::summary::CombGraph`]). `@no_protocol_check`
//! taşıyan portta yalnız saatsiz extern'in en kötü durum varsayımına
//! dayanan yollar yok sayılır (ADR-0114 K4); görünür bir yol yapısal
//! kuraldır ve yine E4007'dir.

use std::collections::HashMap;

use volt_ast::{ItemKind, PortDir, SourceFile};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use crate::resolve::{DefId, ResolveResult};
use crate::summary::{CombGraph, Dep, Summaries};

const HANDSHAKE: &str = "Handshake";
const FIELD_VALID: &str = "valid";
const FIELD_READY: &str = "ready";

/// Her üretici Handshake portu için E4007 denetimi.
pub fn check_handshakes(
    ast: &SourceFile,
    res: &ResolveResult,
    sums: &Summaries,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for &item in &ast.items {
        let ItemKind::Module(m) = &ast.items_arena[item].kind else {
            continue;
        };
        let mut valids: Vec<(DefId, String, Span, bool)> = Vec::new();
        let mut readys: HashMap<String, DefId> = HashMap::new();
        for p in &m.ports {
            let Some(origin) = &p.bundle else { continue };
            if origin.bundle != HANDSHAKE {
                continue;
            }
            let Some(&def) = res.decl_spans.get(&p.name.span) else {
                continue;
            };
            match (origin.path.as_str(), p.direction) {
                (FIELD_VALID, PortDir::Out) => {
                    valids.push((
                        def,
                        origin.port.text.clone(),
                        origin.port.span,
                        origin.no_protocol_check,
                    ));
                }
                (FIELD_READY, PortDir::In) => {
                    readys.insert(origin.port.text.clone(), def);
                }
                _ => {}
            }
        }
        if valids.is_empty() {
            continue;
        }
        let graph = CombGraph::build(ast, res, m, sums);
        for (valid, port, port_span, no_check) in valids {
            let Some(&ready) = readys.get(&port) else {
                continue;
            };
            // Önce varsayımsız (görünür) yol: yapısal kural, nitelikle
            // kapanmaz. Yalnız saatsiz extern varsayımıyla kurulan yol
            // `@no_protocol_check` ile kapanır (ADR-0114 K4).
            let certain = graph.find_path(valid, ready, &|d: &Dep| !d.is_assumed());
            let found = match certain {
                Some(chain) => Some((chain, false)),
                None if no_check => None,
                None => graph
                    .find_path(valid, ready, &|_: &Dep| true)
                    .map(|chain| (chain, true)),
            };
            if let Some((chain, assumed)) = found {
                diags.push(e4007(res, &port, port_span, &chain, assumed));
            }
        }
    }
    diags
}

/// E4007 — beş parça: kod, konum (valid'in sürücüsü), açıklama, öneri,
/// gerekçe notu (ADR-0050) + ikincil etiketler (ready okuması, port).
/// Yol bir örneğin içinden geçiyorsa adımı yazılır:
/// `tx.valid <- g.y (AndGate: y <- b) <- tx_ready` (ADR-0114).
/// `assumed`: yol yalnız saatsiz extern'in en kötü durum varsayımıyla
/// kuruluyor; öneri `@no_protocol_check`'i de anar.
fn e4007(
    res: &ResolveResult,
    port: &str,
    port_span: Span,
    chain: &[Dep],
    assumed: bool,
) -> Diagnostic {
    let first = &chain[0];
    let last = &chain[chain.len() - 1];
    let mut names = vec![format!("{port}.valid")];
    for d in chain {
        if let Some(h) = &d.hop {
            names.push(format!(
                "{}.{} ({}: {} <- {})",
                h.inst, h.output, h.target, h.output, h.input
            ));
        }
        names.push(res.defs[d.def.0 as usize].name.clone());
    }
    let path = names.join(" <- ");
    // Saatsiz extern'in içi görünmez (en kötü durum, ADR-0114 K4): yol
    // yalnız bu varsayımla kuruluyorsa ve gerçekte yoksa çıkış yolu
    // portu `@no_protocol_check` ile işaretlemektir.
    let hop = chain
        .iter()
        .filter_map(|d| d.hop.as_ref())
        .find(|h| h.assumed)
        .filter(|_| assumed);
    let help = match hop {
        Some(h) if h.worst_case => {
            let (target, output, input) = (&h.target, &h.output, &h.input);
            lstr!(en: "decide valid from registered state (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r). The body of extern '{target}' is not visible, so '{output}' is assumed to depend on every input; if '{output}' does not depend on '{input}', mark the port @no_protocol_check";
                  tr: "valid'i register'lanmış durumdan üretin (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r). '{target}' extern'ünün gövdesi görünmez, bu yüzden '{output}' her girişe bağlı sayılır; '{output}', '{input}'a bağlı değilse portu @no_protocol_check ile işaretleyin")
        }
        Some(h) => {
            let (inst, target) = (&h.inst, &h.target);
            lstr!(en: "decide valid from registered state (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r). The path through '{inst}' rests on an extern without a clock inside '{target}', whose outputs are assumed to depend on every input; if the real module has no such path, mark the port @no_protocol_check";
                  tr: "valid'i register'lanmış durumdan üretin (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r). '{inst}' içinden geçen yol '{target}' içindeki saatsiz bir extern'e dayanıyor; onun çıkışları her girişe bağlı sayılır. Gerçek modülde böyle bir yol yoksa portu @no_protocol_check ile işaretleyin")
        }
        None => {
            lstr!(en: "decide valid from registered state (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r); the consumer may derive ready from valid, the producer must not derive valid from ready";
                  tr: "valid'i register'lanmış durumdan üretin (reg valid_r : bool = false; on clk {{ ... }}; {port}.valid = valid_r); tüketici ready'yi valid'den türetebilir, üretici valid'i ready'den türetemez")
        }
    };
    Diagnostic::error(
        ErrorCode::E4007,
        lstr!(en: "handshake '{port}': valid depends combinationally on ready";
              tr: "'{port}' el sıkışması: valid, ready'ye kombinasyonel bağımlı"),
        LabeledSpan::primary(
            first.stmt,
            lstr!(en: "valid is driven here"; tr: "valid burada sürülüyor"),
        ),
        help,
    )
    .with_secondary(
        last.site,
        lstr!(en: "ready is read here"; tr: "ready burada okunuyor"),
    )
    .with_secondary(
        port_span,
        lstr!(en: "handshake port declared here"; tr: "el sıkışma portu burada bildirildi"),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "combinational path: {path}. A producer that waits for ready before raising valid deadlocks against a consumer that waits for valid (AXI A3.3.1)";
              tr: "kombinasyonel yol: {path}. valid'i yükseltmek için ready'yi bekleyen üretici, valid'i bekleyen tüketiciyle kilitlenir (AXI A3.3.1)"),
    )
}
