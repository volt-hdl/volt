//! W3011 (ADR-0103, #93): ham reset portu modülün kullanılan bir saatini
//! besliyor — register'lar bırakma senkronizörünün son aşamasıyla
//! sıfırlanıyor (sv-emit `reset_sync.rs`) — ve aynı ham port bir extern
//! örneğine bağlanıyor. Extern bağlananı alır: flop'ları register'lardan
//! farklı çevrimde ve saate hizasız reset'ten çıkar. Davranış değişmez;
//! yalnız uyarı.

use volt_ast::StmtKind;
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::facts::{contains, is_extern_instance};
use super::Rdc;

impl Rdc<'_> {
    pub(super) fn check_extern_raw_reset(&mut self, m: usize, feeds: &[Option<usize>]) {
        let module = &self.facts.modules[m];
        let ast = self.facts.ast;
        for (ri, raw) in module.raws.iter().enumerate() {
            let synced: Vec<&str> = module
                .clocks
                .iter()
                .zip(feeds)
                .filter(|(c, f)| **f == Some(ri) && c.used)
                .map(|(c, _)| c.port.name.text.as_str())
                .collect();
            if synced.is_empty() {
                continue;
            }
            for &s in &module.decl.body {
                let StmtKind::Instance(inst) = &ast.stmts[s].kind else {
                    continue;
                };
                if !is_extern_instance(ast, self.res, inst) {
                    continue;
                }
                for b in &inst.bindings {
                    let reads_raw = self
                        .res
                        .use_spans
                        .iter()
                        .any(|(span, def)| *def == raw.def && contains(&b.span, span));
                    if reads_raw {
                        let diag = warning(
                            &raw.port.name.text,
                            &inst.name.text,
                            &synced.join("', '"),
                            b.span,
                            raw.port.name.span,
                        );
                        self.diagnostics.push(diag);
                    }
                }
            }
        }
    }
}

fn warning(
    port: &str,
    inst: &str,
    clocks: &str,
    binding: volt_span::Span,
    decl: volt_span::Span,
) -> Diagnostic {
    Diagnostic::warning(
        ErrorCode::W3011,
        lstr!(en: "raw reset '{port}' goes to extern instance '{inst}', but this module's registers use its synchronized copy";
              tr: "ham reset '{port}' extern örneği '{inst}''e gidiyor, ama bu modülün register'ları senkronize kopyasını kullanıyor"),
        LabeledSpan::primary(
            binding,
            lstr!(en: "'{inst}' gets '{port}' as it is, not synchronized to '{clocks}'";
                  tr: "'{inst}' '{port}'u olduğu gibi alır, '{clocks}'ye senkronize değil"),
        ),
        lstr!(en: "check that the SystemVerilog module synchronizes the release of '{port}' itself, or that its flip-flops may leave reset a few cycles apart from this module's registers";
              tr: "SystemVerilog modülünün '{port}' bırakmasını kendisi senkronladığını ya da flip-flop'larının bu modülün register'larından birkaç çevrim ayrı reset'ten çıkabileceğini denetleyin"),
    )
    .with_secondary(
        decl,
        lstr!(en: "Volt releases '{port}' to the registers through a synchronizer on '{clocks}'";
              tr: "Volt '{port}'u register'lara '{clocks}' üzerindeki bir senkronizörle bırakır"),
    )
    .with_note(
        NoteKind::Reason,
        lstr!(en: "a reset released asynchronously to the clock can violate recovery/removal timing, and the extern leaves reset on a different cycle than the registers beside it";
              tr: "saate asenkron bırakılan reset recovery/removal zamanlamasını bozabilir; extern yanındaki register'lardan farklı bir çevrimde reset'ten çıkar"),
    )
}
