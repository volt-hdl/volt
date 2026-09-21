//! Birim testleri için ortak yardımcılar: kaynak ayrıştırılır, isim
//! çözümleme + tip kontrolü koşar, ardından YALNIZ saat alanı çıkarımı
//! (güven denetimi ve sonraki geçişler koşmaz).

use volt_ast::SourceFile;
use volt_syntax::{parse, FileId};

use super::{infer_domains, DomainId, DomainResult, Inferencer};
use crate::consteval::ConstEvaluator;
use crate::resolve::{resolve_file, ResolveResult};
use crate::typeck::{typecheck, TypeckResult};

/// İki saatli test modülleri için ortak domain önsözü.
pub(super) const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                                      domain Slow { clock = posedge, reset = sync active_high }\n";

/// `fast_clk @Fast` + `slow_clk @Slow` portlu modül; `body` port ve deyimler.
pub(super) fn two_clock(body: &str) -> String {
    format!(
        "{TWO_DOMAINS}module M {{\n    in fast_clk : clock @Fast\n    \
         in slow_clk : clock @Slow\n{body}\n}}\n"
    )
}

pub(super) struct Inferred {
    pub res: ResolveResult,
    pub dom: DomainResult,
}

impl Inferred {
    /// Domain tanı kodları, üretim sırasıyla.
    pub fn codes(&self) -> Vec<&'static str> {
        self.dom
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect()
    }

    pub fn count(&self, code: &str) -> usize {
        self.codes().iter().filter(|c| **c == code).count()
    }

    pub fn domain_of(&self, name: &str) -> DomainId {
        let (def, _) = self
            .res
            .def_by_name(name)
            .unwrap_or_else(|| panic!("'{name}' tanımı bulunmalı"));
        self.dom.signal_domains[&def]
    }

    /// Sinyalin belirli alanının adı; belirli değilse `None`.
    pub fn domain_name_of(&self, name: &str) -> Option<&str> {
        match self.domain_of(name) {
            DomainId::Explicit(id) => Some(self.dom.domains[id as usize].name.as_str()),
            _ => None,
        }
    }
}

fn front_end(src: &str) -> (SourceFile, ResolveResult, TypeckResult) {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "kaynak ayrışmalı: {:?}",
        parsed.error_codes()
    );
    let res = resolve_file(&parsed.ast);
    let tyck = {
        let mut evaluator = ConstEvaluator::new(&parsed.ast, &res);
        evaluator.eval_all_consts();
        evaluator.check_type_positions();
        typecheck(&parsed.ast, &res, &mut evaluator)
    };
    (parsed.ast, res, tyck)
}

pub(super) fn inferred(src: &str) -> Inferred {
    let (ast, res, tyck) = front_end(src);
    let dom = infer_domains(&ast, &res, &tyck);
    Inferred { res, dom }
}

/// Domain bildirimleri toplanmış çıplak bir `Inferencer` üzerinde `f`
/// koşar (kafes işlemlerini modül yürüyüşü olmadan sınamak için).
pub(super) fn with_inferencer<R>(src: &str, f: impl FnOnce(&mut Inferencer<'_>) -> R) -> R {
    let (ast, res, tyck) = front_end(src);
    let mut inf = Inferencer::new(&ast, &res, &tyck);
    inf.collect_domain_decls();
    f(&mut inf)
}
