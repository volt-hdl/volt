//! Reset'in örneğe yolu (ADR-0124, #340).
//!
//! Volt bir reset'i modüle girdiği yerde, ham reset GİRİŞ portunda
//! senkronize eder ve oradan izler (ADR-0065 §1–2). İki kural bu yolu
//! korur; ikisi de yalnız burada denetlenir:
//!
//! 1. `sync()`/`sync3()` kaynağı reset tipliyse E2003: veri senkronizörünün
//!    aşamaları hedef alanın reset'iyle temizlenir; kaynak o reset'in
//!    kendisiyse çıkış assert süresince hep temiz değerdir (#340).
//! 2. Reset tipli bir porta bağlanan değer ebeveynin ham reset portunun
//!    adı değilse E2003 (ADR-0065 §1 tablosu, "Örneklemede ham port"):
//!    `let`, `if`, `match` ya da `sync()` ile yapılan kopya ne
//!    senkronizörden geçer ne de E3003/W3011'e görünür.

use volt_ast::{Expr, ExprKind, Idx, InstanceDecl, ItemKind, ModuleDecl, PortBinding, StmtKind};
use volt_diagnostics::{
    lstr, Applicability, Diagnostic, ErrorCode, LabeledSpan, NoteKind, Suggestion,
};
use volt_span::Span;

use super::TypeChecker;
use crate::domain::is_raw_reset;
use crate::resolve::{DefId, DefKind};
use crate::ty::{Ty, TypeId};

impl TypeChecker<'_, '_> {
    fn is_reset(&self, ty: TypeId) -> bool {
        matches!(self.types.ty(ty), Ty::Reset { .. })
    }

    /// Kural 1: `sync(src, clk)` / `sync3(src, clk)` kaynağı reset.
    pub(super) fn check_sync_source(&mut self, name: &str, src: Idx<Expr>, src_ty: TypeId) {
        if !self.is_reset(src_ty) {
            return;
        }
        let ast = self.ast;
        let arg = if let ExprKind::Path(p) = &ast.exprs[src].kind {
            p.segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join("::")
        } else {
            "reset".to_string()
        };
        let span = ast.exprs[src].span;
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E2003,
                lstr!(en: "{name}() cannot carry a reset: '{arg}' is a reset";
                      tr: "{name}() bir reset taşıyamaz: '{arg}' bir reset"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "the synchronizer's flip-flops are cleared while '{arg}' is asserted";
                          tr: "senkronizörün flip-flop'ları '{arg}' assert iken temizlenir"),
                ),
                lstr!(en: "do not write a reset port on a submodule: Volt connects its automatic reset port to this module's synchronized reset for its clock; bind an extern module's reset port to '{arg}' itself";
                      tr: "alt modüle reset portu yazmayın: Volt onun otomatik reset portunu bu modülün o saat için senkronize ettiği reset'e bağlar; extern modülün reset portuna '{arg}'ın kendisini bağlayın"),
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "{name}() is a data synchronizer: its flip-flops are cleared by the reset of the destination clock. When that reset is '{arg}' itself, they are cleared for exactly as long as '{arg}' is asserted, so an active-high reset never comes out and an active-low one only by accident; a reset needs a synchronizer that asserts at once and releases on a clock edge, and Volt builds that one from a reset input port";
                      tr: "{name}() bir veri senkronizörüdür: flip-flop'larını hedef saatin reset'i temizler. O reset '{arg}'ın kendisiyse flip-flop'lar tam '{arg}' assert olduğu sürece temizlenir; active_high bir reset hiç çıkmaz, active_low bir reset ise yalnız tesadüfen çıkar. Reset'e hemen assert eden ve saat kenarında bırakan bir senkronizör gerekir; Volt onu bir reset giriş portundan kurar"),
            ),
        );
    }

    /// Kural 2: reset tipli porta bağlanan değer ham reset portunun adı.
    /// Tip uyuşmazlığı (değer reset değil) olağan E2003'tür; burada ikinci
    /// kez söylenmez.
    pub(super) fn check_reset_binding(
        &mut self,
        inst: &InstanceDecl,
        target: DefId,
        b: &PortBinding,
    ) {
        let value = b.value;
        let Some(&vt) = self.expr_types.get(&value) else {
            return;
        };
        if !self.is_reset(vt) || self.raw_reset_def(value).is_some() {
            return;
        }
        let ast = self.ast;
        let res = self.res;
        let port = &b.port_name.text;
        let inst_name = &inst.name.text;
        let child = res.item_of_def.get(&target).and_then(|&i| {
            if let ItemKind::Module(m) = &ast.items_arena[i].kind {
                Some(m)
            } else {
                None
            }
        });
        let span = ast.exprs[value].span;
        // Volt alt modülü: portu yazma (makine önerisi, koşullar
        // sağlanırsa); extern: öneri yok, iki yol bedelleriyle notta.
        let (help, fix, extern_note) = match child {
            Some(child) => {
                let cname = &child.name.text;
                let help = lstr!(en: "remove the reset port '{port}' from '{cname}' and this binding: '{cname}' then gets an automatic reset port, which Volt connects to this module's synchronized reset for its clock";
                                 tr: "'{cname}' modülünden '{port}' reset portunu ve bu bağlamayı kaldırın: '{cname}' o zaman otomatik bir reset portu alır ve Volt onu bu modülün o saat için senkronize ettiği reset'e bağlar");
                (help, self.child_port_fix(inst, b, child), None)
            }
            None => {
                let raw = self
                    .raw_reset_read_by(value)
                    .unwrap_or_else(|| "rst".to_string());
                let help = lstr!(en: "bind the reset input port itself, '{port}: {raw}', and let the SystemVerilog module synchronize the release";
                                 tr: "reset giriş portunun kendisini bağlayın, '{port}: {raw}', ve bırakmayı SystemVerilog modülü senkronlasın");
                let note = lstr!(en: "Volt cannot give an extern module the synchronized reset yet: it connects the synchronizer's output only to the automatic reset port of a Volt module, and an extern module has none. Two choices remain. Bind the raw reset: when this module's registers on the same clock are reset through the synchronizer, Volt warns W3011, because the extern then leaves reset on a different cycle than they do and, unless its SystemVerilog code synchronizes the release, not in step with the clock. Or synchronize inside the extern (two flip-flops that assert at once and release on a clock edge): the raw reset is then safe, and W3011 only reminds that the two sides leave reset on different cycles";
                                 tr: "Volt bir extern modüle senkronize reset'i henüz veremez: senkronizörün çıkışını yalnız bir Volt modülünün otomatik reset portuna bağlar, extern modülün böyle bir portu yoktur. İki yol kalır. Ham reset'i bağlamak: bu modülün aynı saatteki register'ları senkronizörden geçen reset'le sıfırlanıyorsa Volt W3011 uyarısı verir, çünkü extern reset'ten onlardan farklı bir çevrimde ve SystemVerilog kodu bırakmayı senkronlamıyorsa saatle hizasız çıkar. Ya da senkronlamayı extern'in içinde yapmak (hemen assert eden, saat kenarında bırakan iki flip-flop): ham reset o zaman güvenlidir, W3011 yalnız iki tarafın reset'ten farklı çevrimlerde çıktığını hatırlatır");
                (help, None, Some(note))
            }
        };
        let mut diag = Diagnostic::error(
            ErrorCode::E2003,
            lstr!(en: "reset port '{port}' of instance '{inst_name}' is not bound to a reset input port";
                  tr: "'{inst_name}' örneğinin '{port}' reset portu bir reset giriş portuna bağlı değil"),
            LabeledSpan::primary(
                span,
                lstr!(en: "this value is computed from a reset; it is not a reset input port of this module";
                      tr: "bu değer bir reset'ten hesaplanıyor; bu modülün bir reset giriş portu değil"),
            ),
            help,
        )
        .with_note(
            NoteKind::Reason,
            lstr!(en: "Volt synchronizes a reset where it enters a module, at its reset input port, and follows it from there into submodules. A copy made with let, if, match or sync() is an ordinary signal: no synchronizer releases it, and the reset checks cannot see it";
                  tr: "Volt bir reset'i modüle girdiği yerde, reset giriş portunda senkronize eder ve oradan alt modüllere izler. let, if, match ya da sync() ile yapılan kopya sıradan bir sinyaldir: onu hiçbir senkronizör bırakmaz ve reset denetimleri onu göremez"),
        );
        if let Some(note) = extern_note {
            diag = diag.with_note(NoteKind::Note, note);
        }
        if let Some((first, rest)) = fix {
            // suggestion: e2003_reset_through_sync, e2003_reset_alias
            let s = rest.into_iter().fold(
                Suggestion::replace(first, "", Applicability::MachineApplicable),
                |s, span| s.and_replace(span, ""),
            );
            diag = diag.with_suggestion(s);
        }
        self.diagnostics.push(diag);
    }

    /// Değer, bu modülün ham reset portunu adlandıran tek segmentli yol mu.
    fn raw_reset_def(&self, value: Idx<Expr>) -> Option<DefId> {
        let ExprKind::Path(p) = &self.ast.exprs[value].kind else {
            return None;
        };
        if p.segments.len() != 1 {
            return None;
        }
        let def = *self.res.resolutions.get(&value)?;
        self.is_raw_reset_def(def).then_some(def)
    }

    /// `def` bir ham reset giriş portu mu (`domain::is_raw_reset`).
    fn is_raw_reset_def(&self, def: DefId) -> bool {
        if !matches!(self.res.def_kind(def), DefKind::Port { .. }) {
            return false;
        }
        let at = self.res.defs[def.0 as usize].span;
        self.ast.items.iter().any(|&i| {
            let ItemKind::Module(m) = &self.ast.items_arena[i].kind else {
                return false;
            };
            m.ports
                .iter()
                .any(|p| p.name.span == at && is_raw_reset(self.ast, p))
        })
    }

    /// Değerin (ya da adlandırdığı `let`'in) okuduğu ham reset portunun adı.
    fn raw_reset_read_by(&self, value: Idx<Expr>) -> Option<String> {
        let mut spans = vec![self.ast.exprs[value].span];
        if let Some((_, l)) = self.let_named_by(value) {
            spans.push(l);
        }
        // Kaynakta ilk okunan (HashMap sırası belirsiz).
        self.res
            .use_spans
            .iter()
            .filter(|(s, d)| spans.iter().any(|r| inside(s, r)) && self.is_raw_reset_def(**d))
            .min_by_key(|(s, _)| (s.file.0, s.start))
            .map(|(_, d)| self.res.defs[d.0 as usize].name.clone())
    }

    /// Değer tek segmentli bir yol olarak bir modül düzeyi `let`'i
    /// adlandırıyorsa: `let`'in tanımı ve deyiminin konumu.
    fn let_named_by(&self, value: Idx<Expr>) -> Option<(DefId, Span)> {
        let ExprKind::Path(p) = &self.ast.exprs[value].kind else {
            return None;
        };
        if p.segments.len() != 1 {
            return None;
        }
        let def = *self.res.resolutions.get(&value)?;
        if self.res.def_kind(def) != DefKind::LocalBinding {
            return None;
        }
        let at = self.res.defs[def.0 as usize].span;
        self.ast.stmts.iter().find_map(|s| {
            let StmtKind::Let(l) = &s.kind else {
                return None;
            };
            (l.name.span == at).then_some((def, s.span))
        })
    }

    /// Volt alt modülü için düzeltme: bağlamayı, çocuğun reset portunu ve
    /// (yalnız burada okunan) `let`'i silen aralıklar; ilki bağlamanın.
    /// Uygulanınca yeni tanı doğabilecekse `None` (ADR-0099).
    fn child_port_fix(
        &self,
        inst: &InstanceDecl,
        b: &PortBinding,
        child: &ModuleDecl,
    ) -> Option<(Span, Vec<Span>)> {
        let ast = self.ast;
        let res = self.res;
        // Açılımdan (`for`, generic kopya) gelen bağlama: metni tek değil.
        if b.span.ctx != 0 {
            return None;
        }
        // Çocuğun tek ham reset portu (reset tipli her giriş portu ham
        // olduğundan bağlanan port odur) gövdede okunmamalı.
        let raws: Vec<usize> = (0..child.ports.len())
            .filter(|&i| is_raw_reset(ast, &child.ports[i]))
            .collect();
        let [pi] = raws.as_slice() else {
            return None;
        };
        let port = &child.ports[*pi];
        let port_def = *res.decl_spans.get(&port.name.span)?;
        if res.use_spans.values().any(|d| *d == port_def) {
            return None;
        }
        // Portu başka bir örnek bağlamamalı (kaldırılınca E1009 olurdu).
        let target = &child.name.text;
        let elsewhere = ast.stmts.iter().any(|s| {
            let StmtKind::Instance(i) = &s.kind else {
                return false;
            };
            i.name.span != inst.name.span
                && matches!(i.module_path.segments.as_slice(), [seg] if seg.text == *target)
                && i.bindings
                    .iter()
                    .any(|x| x.port_name.text == port.name.text)
        });
        if elsewhere {
            return None;
        }
        let parent = enclosing_module(ast, inst)?;
        let first = between(&inst.bindings, |x| x.span, b.span)?;
        let mut rest = vec![between(&child.ports, |p| p.span, port.span)?];
        let value = b.value;
        let mut removed = vec![ast.exprs[value].span];
        let let_def = match self.let_named_by(value) {
            Some((def, stmt_span)) => {
                // Let yalnız bu bağlamada okunmalı.
                let uses = res.use_spans.values().filter(|d| **d == def).count();
                if uses != 1 {
                    return None;
                }
                let body: Vec<Span> = parent.body.iter().map(|&s| ast.stmts[s].span).collect();
                rest.push(between(&body, |s| *s, stmt_span)?);
                removed.push(stmt_span);
                Some(def)
            }
            // Let olmayan bir ad (tel, çıkış portu): öneri yok.
            None if matches!(ast.exprs[value].kind, ExprKind::Path(_)) => return None,
            None => None,
        };
        // Silinen metnin okuduğu her ad başka yerde de okunmalı (W1001
        // doğmasın); ham reset portu ve yerleşikler muaf.
        let orphan = res
            .use_spans
            .iter()
            .filter(|(s, _)| removed.iter().any(|r| inside(s, r)))
            .any(|(_, d)| {
                Some(*d) != let_def
                    && !self.is_raw_reset_def(*d)
                    && !matches!(res.def_kind(*d), DefKind::Builtin(_))
                    && !res
                        .use_spans
                        .iter()
                        .any(|(s2, d2)| d2 == d && !removed.iter().any(|r| inside(s2, r)))
            });
        if orphan {
            return None;
        }
        Some((first, rest))
    }
}

/// `inner` `outer`'ın içinde mi (aynı dosya ve bağlam).
fn inside(inner: &Span, outer: &Span) -> bool {
    inner.file == outer.file && outer.start <= inner.start && inner.end <= outer.end
}

/// Örneğin bildirildiği modül (modül gövdesinin doğrudan deyimi).
fn enclosing_module<'a>(
    ast: &'a volt_ast::SourceFile,
    inst: &InstanceDecl,
) -> Option<&'a ModuleDecl> {
    ast.items.iter().find_map(|&i| {
        let ItemKind::Module(m) = &ast.items_arena[i].kind else {
            return None;
        };
        m.body
            .iter()
            .any(|&s| {
                matches!(&ast.stmts[s].kind, StmtKind::Instance(x) if x.name.span == inst.name.span)
            })
            .then_some(m)
    })
}

/// Listedeki `target` öğesini ayırıcısıyla birlikte silen aralık:
/// önceki öğe varsa onun sonundan bu öğenin sonuna, yoksa bu öğenin
/// başından sonraki öğenin başına. Tek öğede `None` (silinecek ayırıcı
/// yok; öneri verilmez).
fn between<T>(items: &[T], span_of: impl Fn(&T) -> Span, target: Span) -> Option<Span> {
    let k = items.iter().position(|x| span_of(x) == target)?;
    let (start, end) = if let Some(p) = k.checked_sub(1) {
        (span_of(&items[p]).end, target.end)
    } else {
        (target.start, span_of(items.get(k + 1)?).start)
    };
    Some(Span {
        start,
        end,
        ..target
    })
}

#[cfg(test)]
mod tests {
    use volt_diagnostics::{Applicability, ErrorCode};

    use super::super::testutil::{codes, diagnostics};

    const CHILD: &str = "module Child {
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    reg r : u8 = 5
    on clk {
        r <= r + 1
    }
    v = r
}

";

    const LATCH: &str = "extern module Latch {
    in  clk  : clock
    in  rst  : reset(sync, active_high)
    out seen : bool
}

";

    /// `Top`: ham `rst`, `clk` üzerinde register'ı var; `lets` satırları
    /// örnekten önce, `inst` örnek satırı.
    fn top(lets: &str, inst: &str) -> String {
        format!(
            "module Top {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    in  c   : bool
    out v   : u8
    out w   : u8

    reg p : u8 = 0
    on clk {{
        if c {{
            p <= p + 1
        }}
    }}
    w = p
{lets}
    {inst}
    v = ch.v
}}
"
        )
    }

    fn e2003(src: &str) -> usize {
        codes(src).iter().filter(|c| **c == "E2003").count()
    }

    fn child(lets: &str, value: &str) -> String {
        format!(
            "{CHILD}{}",
            top(
                lets,
                &format!("let ch = Child {{ clk: clk, rst: {value} }}")
            )
        )
    }

    #[test]
    fn a_reset_through_sync_is_e2003_at_the_call_and_at_the_binding() {
        // #340: sync() aşamaları hedef reset'le temizlenir; reset'in kendisi
        // assert iken çıkış hep temizlenmiş değerdir (active_high: hiç gelmez).
        let src = child("    let rs = sync(rst, clk)", "rs");
        assert_eq!(e2003(&src), 2, "{:?}", codes(&src));
    }

    #[test]
    fn a_reset_through_sync3_is_e2003() {
        let src = child("    let rs = sync3(rst, clk)", "rs");
        assert_eq!(e2003(&src), 2, "{:?}", codes(&src));
    }

    #[test]
    fn an_active_low_reset_through_sync_is_e2003() {
        // active_low'da aşamaların temiz değeri (0) assert değeridir: devre
        // tesadüfen bir reset verir; kural kutba bakmaz.
        let src = "domain D {
    clock = posedge
    reset = sync active_low
}

extern module Probe {
    in  clk   : clock @D
    in  rst_n : reset(sync, active_low)
    out o     : bool @D
}

module Top {
    in  clk   : clock @D
    in  rst_n : reset(sync, active_low)
    out o     : bool @D

    reg p : u8 = 0
    on clk {
        p <= p + 1
    }
    let rs = sync(rst_n, clk)
    let pr = Probe { clk: clk, rst_n: rs }
    o = pr.o && p == 0
}
";
        assert_eq!(e2003(src), 2, "{:?}", codes(src));
    }

    #[test]
    fn sync_of_a_reset_is_e2003_even_when_nothing_reads_it() {
        let src = child("    let _rs = sync(rst, clk)", "rst");
        assert_eq!(e2003(&src), 1, "{:?}", codes(&src));
    }

    #[test]
    fn a_reset_through_an_untyped_let_is_e2003() {
        let src = child("    let rs = rst", "rs");
        assert_eq!(e2003(&src), 1, "{:?}", codes(&src));
    }

    #[test]
    fn a_reset_through_a_typed_let_is_e2003() {
        let src = child("    let rs : reset(sync, active_high) = rst", "rs");
        assert_eq!(e2003(&src), 1, "{:?}", codes(&src));
    }

    #[test]
    fn a_reset_through_if_or_match_is_e2003() {
        let via_if = child("    let rs = if c { rst } else { rst }", "rs");
        assert_eq!(e2003(&via_if), 1, "{:?}", codes(&via_if));
        let inline = child("", "if c { rst } else { rst }");
        assert_eq!(e2003(&inline), 1, "{:?}", codes(&inline));
        let via_match = child("    let rs = match c { true => rst, _ => rst }", "rs");
        assert_eq!(e2003(&via_match), 1, "{:?}", codes(&via_match));
    }

    #[test]
    fn a_reset_through_a_let_to_an_extern_is_e2003() {
        let src = format!(
            "{LATCH}{}",
            top("    let rs = rst", "let l = Latch { clk: clk, rst: rs }")
                .replace("    v = ch.v\n", "    v = 0\n")
        );
        assert_eq!(e2003(&src), 1, "{:?}", codes(&src));
    }

    #[test]
    fn the_raw_reset_port_itself_is_accepted() {
        let long = child("", "rst");
        assert_eq!(e2003(&long), 0, "{:?}", codes(&long));
        let shorthand = format!("{CHILD}{}", top("", "let ch = Child { clk, rst }"));
        assert_eq!(e2003(&shorthand), 0, "{:?}", codes(&shorthand));
    }

    #[test]
    fn sync_of_data_is_unchanged() {
        let src = child("    let cs = sync(c, clk)\n    let _x = cs", "rst");
        assert_eq!(e2003(&src), 0, "{:?}", codes(&src));
    }

    #[test]
    fn a_mismatched_type_is_reported_once() {
        // Tip uyuşmazlığı zaten E2003; biçim kuralı ikinci kez söylemez.
        let src = child("", "c");
        assert_eq!(e2003(&src), 1, "{:?}", codes(&src));
    }

    #[test]
    fn a_volt_child_gets_a_machine_applicable_fix_and_an_extern_none() {
        let volt = child("    let rs = sync(rst, clk)", "rs");
        let diags = diagnostics(&volt);
        let binding = diags
            .iter()
            .filter(|d| d.code == ErrorCode::E2003)
            .find(|d| !d.suggestions.is_empty())
            .expect("Volt alt modülünde önerili E2003");
        assert_eq!(
            binding.suggestions[0].applicability,
            Applicability::MachineApplicable
        );
        // Port, bağlama ve let silinir.
        assert_eq!(binding.suggestions[0].edits.len(), 3);

        let ext = format!(
            "{LATCH}{}",
            top("    let rs = rst", "let l = Latch { clk: clk, rst: rs }")
                .replace("    v = ch.v\n", "    v = 0\n")
        );
        let diags = diagnostics(&ext);
        let d = diags
            .iter()
            .find(|d| d.code == ErrorCode::E2003)
            .expect("extern E2003");
        assert!(d.suggestions.is_empty(), "{:?}", d.suggestions);
        assert!(
            d.notes.iter().any(|n| n.text.contains("W3011")),
            "{:?}",
            d.notes
        );
    }

    #[test]
    fn no_fix_when_the_child_port_is_bound_elsewhere_or_read() {
        // Portu kaldırmak öteki örneği bozar: öneri yok, yalnız metin.
        let src = format!(
            "{CHILD}{}",
            top(
                "    let rs = rst",
                "let ch = Child { clk: clk, rst: rs }\n    let ch2 = Child { clk: clk, rst: rst }\n    let _u = ch2.v"
            )
        );
        let diags = diagnostics(&src);
        let d = diags
            .iter()
            .find(|d| d.code == ErrorCode::E2003)
            .expect("E2003");
        assert!(d.suggestions.is_empty(), "{:?}", d.suggestions);
    }

    #[test]
    fn no_fix_when_removing_the_value_leaves_a_name_unread() {
        // `c` yalnız silinecek if'te okunuyorsa öneri W1001 doğururdu.
        let src = format!(
            "{CHILD}module Top {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    in  c   : bool
    out v   : u8

    let rs = if c {{ rst }} else {{ rst }}
    let ch = Child {{ clk: clk, rst: rs }}
    v = ch.v
}}
"
        );
        let diags = diagnostics(&src);
        let d = diags
            .iter()
            .find(|d| d.code == ErrorCode::E2003)
            .expect("E2003");
        assert!(d.suggestions.is_empty(), "{:?}", d.suggestions);
    }

    /// Extern'e giden E2003'ün yardım metni.
    fn extern_help(src: &str) -> String {
        diagnostics(src)
            .into_iter()
            .find(|d| d.code == ErrorCode::E2003)
            .and_then(|d| d.help)
            .expect("extern E2003")
    }

    #[test]
    fn the_extern_help_names_the_reset_input_port_read_first() {
        let one = format!(
            "{LATCH}module Top {{
    in  clk     : clock
    in  ext_rst : reset(sync, active_high)
    out seen    : bool

    let rs = ext_rst
    let l = Latch {{ clk: clk, rst: rs }}
    seen = l.seen
}}
"
        );
        assert!(
            extern_help(&one).contains("'rst: ext_rst'"),
            "{}",
            extern_help(&one)
        );
        // İki ham port okunuyorsa kaynakta ilk okunanı.
        let two = format!(
            "{LATCH}module Top {{
    in  clk   : clock
    in  rst_a : reset(sync, active_high)
    in  rst_b : reset(sync, active_high)
    in  c     : bool
    out seen  : bool

    let l = Latch {{ clk: clk, rst: if c {{ rst_b }} else {{ rst_a }} }}
    seen = l.seen
}}
"
        );
        assert!(
            extern_help(&two).contains("'rst: rst_b'"),
            "{}",
            extern_help(&two)
        );
    }

    #[test]
    fn a_fix_is_offered_beside_an_unrelated_instance() {
        // Başka modülün örneği aynı adlı portu bağlıyor: bu çocuğun portu
        // yine yalnız burada bağlı, öneri verilir.
        let src = format!(
            "{CHILD}module Other {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    reg r : u8 = 1
    on clk {{
        r <= r + 1
    }}
    v = r
}}

{}",
            top(
                "    let rs = sync(rst, clk)",
                "let o = Other { clk: clk, rst: rst }
    let ch = Child { clk: clk, rst: rs }
    let _o = o.v"
            )
        );
        let diags = diagnostics(&src);
        assert!(
            diags
                .iter()
                .any(|d| d.code == ErrorCode::E2003 && !d.suggestions.is_empty()),
            "{diags:?}"
        );
    }

    #[test]
    fn no_fix_when_the_child_reads_its_reset_port() {
        // Çocuk portu bir torununa geçiriyor: port kaldırılınca bağlama
        // çözülmezdi.
        let src = format!(
            "module Grand {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    reg r : u8 = 2
    on clk {{
        r <= r + 1
    }}
    v = r
}}

module Child {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let g = Grand {{ clk: clk, rst: rst }}
    v = g.v
}}

{}",
            top("    let rs = rst", "let ch = Child { clk: clk, rst: rs }")
        );
        let diags = diagnostics(&src);
        let d = diags
            .iter()
            .find(|d| d.code == ErrorCode::E2003)
            .expect("E2003");
        assert!(d.suggestions.is_empty(), "{:?}", d.suggestions);
    }

    #[test]
    fn no_fix_for_an_instance_expanded_from_a_for_loop() {
        let src = format!(
            "{CHILD}module Top {{
    in  clk : clock
    in  rst : reset(sync, active_high)
    out v   : u8

    let rs = rst
    for i in 0..1 {{
        let ch = Child {{ clk: clk, rst: rs }}
    }}
    v = 0
}}
"
        );
        let diags = diagnostics(&src);
        let d = diags
            .iter()
            .find(|d| d.code == ErrorCode::E2003)
            .expect("E2003");
        assert!(d.suggestions.is_empty(), "{:?}", d.suggestions);
    }

    #[test]
    fn the_text_carries_no_adr_or_spec_reference() {
        let src = child("    let rs = sync(rst, clk)", "rs");
        for d in diagnostics(&src)
            .iter()
            .filter(|d| d.code == ErrorCode::E2003)
        {
            let mut text = format!("{} {:?}", d.message, d.help);
            for n in &d.notes {
                text.push_str(&n.text);
            }
            for s in &d.spans {
                text.push_str(&s.label);
            }
            assert!(!text.contains("ADR"), "{text}");
            assert!(!text.contains(".md"), "{text}");
        }
    }
}
