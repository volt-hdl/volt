//! Saat alanı referansları: `@Ad` anotasyonunun çözümü (E3002,
//! domain-inference.md §5) ve `extern module` gövdesi ile sembolik alan
//! parametreleri (ADR-0047).

use volt_ast::{ExternDecl, Name};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};

use super::def::DefKind;
use super::scope::{ScopeId, ScopeKind};
use super::suggest::closest_match;
use super::Resolver;

impl Resolver<'_> {
    /// `extern module` gövdesi (ADR-0047): portlar tanım olarak bildirilir
    /// (K8 haritası onları saat/veri portu olarak tanısın), tipleri
    /// çözülür; `@Ad` anotasyonu bilinen bir domain'e ya da saat portuna
    /// çözülür, tanımsızsa extern'e özel SEMBOLİK domain parametresi
    /// (`DefKind::DomainParam`) açar. Sembolik alanın bir clock portunda
    /// taşınıp taşınmadığı domain çıkarımında denetlenir (E3002).
    pub(super) fn resolve_extern_body(&mut self, x: &ExternDecl) {
        let extern_def = self.lookup_item_def(&x.name.text);
        let scope = self.new_scope(ScopeKind::Extern(extern_def), Some(self.root));
        self.declare_generics(&x.generics, scope);
        // Çift port E1003; gölgeleme uyarısı YOK — extern'in gövdesi
        // olmadığından kök isimle çakışma hiçbir karışıklık yaratamaz.
        for p in &x.ports {
            if self.report_duplicate(&p.name, scope) {
                continue;
            }
            self.declare(&p.name, DefKind::Port { dir: p.direction }, scope, false);
        }
        for p in &x.ports {
            self.resolve_type(p.ty, scope);
            if let Some(domain) = &p.domain {
                self.resolve_symbolic_domain_ref(domain, scope);
            }
        }
    }

    /// Extern içinde `@Ad`: görünür bir isimse olağan domain çözümü
    /// (domain tanımı, saat portu ya da daha önce açılmış sembolik alan);
    /// değilse yeni sembolik alan. İlk geçiş hem tanım hem kullanımdır
    /// (K8 `port_domain_key` anotasyon span'ından okur).
    fn resolve_symbolic_domain_ref(&mut self, name: &Name, scope: ScopeId) {
        // Yalnız alan anlamı taşıyan tanımlar olağan yola gider; kök
        // kapsamdaki bir struct/const/fn aynı adı taşıyorsa sembolik alanı
        // GÖLGELEMEZ (aksi halde yanlış E3002 + denetim sessizce kapanırdı).
        let existing = self.lookup_visible(&name.text, scope);
        if existing.is_some_and(|d| {
            matches!(
                self.def(d).kind,
                DefKind::Domain
                    | DefKind::DomainParam
                    | DefKind::Import
                    | DefKind::Port { .. }
                    | DefKind::Error
            )
        }) {
            self.resolve_domain_ref(name, scope);
            return;
        }
        // `decl_spans`'e YAZILMAZ: bundle düzleştirmesi anotasyon span'ini
        // port adı span'iyle paylaşır (bundle.rs), o anahtar portundur.
        let def = self.add_def(DefKind::DomainParam, &name.text, name.span, scope, false);
        self.bind(scope, &name.text, def);
        self.reads.insert(def);
        self.use_spans.insert(name.span, def);
    }

    pub(super) fn resolve_domain_ref(&mut self, name: &Name, scope: ScopeId) {
        // Domain konumunda çözülemeyen isim E1001 değil E3002 üretir:
        // kullanıcı bir saat alanı bekliyordu, genel "tanımsız isim"
        // mesajı yanlış yöne götürür (domain-inference.md §5).
        let Some(def) = self.lookup_visible(&name.text, scope) else {
            self.err_undefined_domain(name);
            return;
        };

        self.reads.insert(def);
        self.use_spans.insert(name.span, def);
        let kind = self.def(def).kind;
        if !matches!(
            kind,
            DefKind::Domain
                | DefKind::DomainParam
                | DefKind::Error
                | DefKind::Import
                | DefKind::Port { .. }
        ) {
            self.diagnostics.push(Diagnostic::error(
                ErrorCode::E3002,
                lstr!(en: "'{}' is not a clock domain", name.text;
                      tr: "'{}' bir saat alanı değil", name.text),
                LabeledSpan::primary(
                    name.span,
                    lstr!(en: "expected a domain"; tr: "domain bekleniyor"),
                ),
                lstr!(en: "use a name defined with domain Name {{ clock = posedge ... }}";
                      tr: "domain Ad {{ clock = posedge ... }} ile tanımlanmış bir isim kullanın"),
            ));
        }
    }

    /// E3002 — `@Ad` hiçbir kapsamda yok; en yakın DOMAIN adı önerilir.
    fn err_undefined_domain(&mut self, name: &Name) {
        let candidates: Vec<String> = self
            .defs
            .iter()
            .filter(|d| matches!(d.kind, DefKind::Domain))
            .map(|d| d.name.clone())
            .collect();
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E3002,
                lstr!(en: "undefined clock domain: '{}'", name.text;
                      tr: "tanımsız saat alanı: '{}'", name.text),
                LabeledSpan::primary(
                    name.span,
                    lstr!(en: "no domain with this name"; tr: "bu isimde bir domain yok"),
                ),
                match closest_match(&name.text, &candidates) {
                    Some(s) => lstr!(en: "did you mean '@{}'?", s;
                                     tr: "'@{}' mi demek istediniz?", s),
                    None => lstr!(
                        en: "define it with domain {} {{ clock = posedge ... }}", name.text;
                        tr: "domain {} {{ clock = posedge ... }} ile tanımlayın", name.text
                    ),
                },
            )
            .with_note(
                NoteKind::Reason,
                lstr!(en: "the @ annotation can only refer to a defined clock domain \
                           or a clock port";
                      tr: "@ anotasyonu yalnız tanımlı bir saat alanına \
                           ya da clock portuna işaret edebilir"),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{codes, resolved};
    use super::super::DefKind;

    #[test]
    fn undefined_domain_annotation_is_e3002_not_e1001() {
        let c = codes("module M { in clk : clock in a : u8 @Yok out y : u8 y = a }");
        assert!(c.contains(&"E3002") && !c.contains(&"E1001"), "{c:?}");
    }

    #[test]
    fn annotation_naming_a_non_domain_is_e3002() {
        let c = codes("const K : u8 = 1;\nmodule M { in a : u8 @K out y : u8 y = a + K }");
        assert!(c.contains(&"E3002"), "{c:?}");
    }

    #[test]
    fn undefined_annotation_in_extern_opens_one_symbolic_domain() {
        let r = resolved(
            "extern module X {\n    in clk : clock @Src\n    in d : u8 @Src\n    out q : u8 @Src\n}\n",
        );
        assert!(r.error_codes().is_empty(), "{:?}", r.error_codes());
        let params = r
            .defs
            .iter()
            .filter(|d| d.kind == DefKind::DomainParam)
            .count();
        assert_eq!(params, 1);
    }
}
