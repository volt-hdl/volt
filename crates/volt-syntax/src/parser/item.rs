//! Öğe seviyesi ayrıştırma (F1): ModuleDecl, DomainDecl, FnDecl,
//! StructDecl, EnumDecl, ConstDecl, TypeAlias, ExternDecl, package/use,
//! generics, kontratlar ve nitelikler.

use volt_ast::{
    AttrArg, Attribute, BlockContext, BlockStmt, ClockEdge, ConstDecl, Contract, ContractKind,
    DomainDecl, DomainField, DomainKey, DomainValue, EnumDecl, EnumVariant, ExternDecl, FnDecl,
    GenericArg, GenericParam, GenericParamKind, Idx, Item, ItemKind, ModuleDecl, Name, PackageDecl,
    Param, Path, Port, PortDir, ResetPolarity, ResetSpec, ResetSync, StructDecl, StructField,
    TypeAlias, TypeRef, TypeRefKind, UseDecl, UseTree, VariantData, Visibility,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};
use volt_span::Span;

use crate::token::TokenKind::*;
use crate::token::{Token, TokenKind};

use super::expr::ABOVE_SHIFT_BP;
use super::recovery::{ITEM_START, PORT_RECOVERY};
use super::Parser;

/// Tanınan nitelikler (grammar-full.ebnf §2) — dışındakiler W0020.
const KNOWN_ATTRIBUTES: &[&str] = &[
    "domain",
    "mmio",
    "reg",
    "offset",
    "access",
    "reserved",
    "budget",
    "timing",
    "false_path",
    "multicycle",
    "version",
    "abi_version",
    "dft",
    "debug_visible",
    "debug_trace",
    "synthesis_target",
];

/// Kontrat anahtar kelimesi → tür eşlemesi.
fn contract_kind(kind: TokenKind) -> Option<ContractKind> {
    Some(match kind {
        KwRequires => ContractKind::Requires,
        KwEnsures => ContractKind::Ensures,
        KwInvariant => ContractKind::Invariant,
        KwCover => ContractKind::Cover,
        KwAssert => ContractKind::Assert,
        KwAssume => ContractKind::Assume,
        _ => return None,
    })
}

impl Parser<'_> {
    pub(crate) fn parse_source_file(&mut self) {
        while !self.at_eof() {
            let before = self.pos;
            match self.current() {
                Some(KwPackage) => self.parse_package(),
                Some(KwUse) => self.parse_use(),
                _ => self.parse_item(),
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
    }

    /// `package yol::adi ;`
    fn parse_package(&mut self) {
        let start = self.pos;
        self.bump_any(); // 'package'
        if !self.at(Ident) {
            self.error_expected(
                &lstr!(en: "package path"; tr: "paket yolu"),
                &lstr!(en: "write it as package cip::alt_sistem;"; tr: "package cip::alt_sistem; biçiminde yazın"),
            );
            self.recover_silent(ITEM_START);
            return;
        }
        let path = self.parse_path();
        self.eat(Semi);
        let span = self.span_from(start);
        if self.ast.package.is_some() {
            self.push_error(Diagnostic::error(
                ErrorCode::E0001,
                lstr!(en: "duplicate 'package' declaration"; tr: "yinelenen 'package' bildirimi"),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "a file can have only one 'package'"; tr: "dosyada tek 'package' olabilir"),
                ),
                lstr!(en: "remove the extra declaration"; tr: "fazladan bildirimi kaldırın"),
            ));
        } else {
            self.ast.package = Some(PackageDecl { span, path });
        }
    }

    /// `use yol [ :: (* | {liste}) ] [ as takma ] ;`
    fn parse_use(&mut self) {
        let start = self.pos;
        self.bump_any(); // 'use'
        if !self.at(Ident) {
            self.error_expected(
                &lstr!(en: "use path"; tr: "use yolu"),
                &lstr!(en: "write it as use paket::modul;"; tr: "use paket::modul; biçiminde yazın"),
            );
            self.recover_silent(ITEM_START);
            return;
        }
        let path = self.parse_path();

        let mut tree = None;
        if self.at(ColonColon) {
            // parse_path yalnız `:: Ident` tüketir; kalan `::*` veya `::{...}`
            self.bump_any();
            match self.current() {
                Some(Star) => {
                    self.bump_any();
                    tree = Some(UseTree::Glob);
                }
                Some(LBrace) => {
                    let open = self.bump();
                    let mut paths = Vec::new();
                    while !self.at(RBrace) && !self.at_eof() {
                        let before = self.pos;
                        if self.at(Ident) {
                            paths.push(self.parse_path());
                        } else {
                            self.error_expected(
                                &lstr!(en: "path in the use list"; tr: "use listesinde yol"),
                                &lstr!(en: "write it as use a::{{b, c::d}}"; tr: "use a::{{b, c::d}} biçiminde yazın"),
                            );
                        }
                        if !self.eat(Comma) && self.pos == before {
                            self.bump_any(); // ilerleme garantisi
                        }
                    }
                    self.expect_closing(RBrace, "}", open);
                    tree = Some(UseTree::List(paths));
                }
                _ => {
                    self.error_expected(
                        &lstr!(en: "'*', '{{' or a name after '::'"; tr: "'::' sonrası '*', '{{' veya isim"),
                        &lstr!(en: "write it as use a::* or use a::{{b, c}}"; tr: "use a::* veya use a::{{b, c}} biçiminde yazın"),
                    );
                }
            }
        } else if self.eat(KwAs) {
            if self.at(Ident) {
                tree = Some(UseTree::Alias(self.parse_name()));
            } else {
                self.error_expected(
                    &lstr!(en: "alias after 'as'"; tr: "'as' sonrası takma ad"),
                    &lstr!(en: "write it as use a::b as c;"; tr: "use a::b as c; biçiminde yazın"),
                );
            }
        }

        self.eat(Semi);
        self.ast.uses.push(UseDecl {
            span: self.span_from(start),
            path,
            tree,
        });
    }

    fn parse_item(&mut self) {
        let start = self.pos;
        let doc = self.collect_doc_comments();
        let attrs = self.parse_attributes();
        let visibility = if self.eat(KwPub) {
            Visibility::Public
        } else {
            Visibility::Private
        };

        let kind = match self.current() {
            Some(KwModule) => self.parse_module(),
            Some(KwDomain) => self.parse_domain(),
            Some(KwFn) => self.parse_fn(),
            Some(KwStruct) => self.parse_struct(),
            Some(KwEnum) => self.parse_enum(),
            Some(KwConst) => self.parse_const(),
            Some(KwType) => self.parse_type_alias(),
            Some(KwExtern) => self.parse_extern(),
            // `pub use` / nitelikli use: gramer dışı ama toleranslı ayrıştır.
            Some(KwUse) => {
                self.parse_use();
                return;
            }
            Some(KwPackage) => {
                self.parse_package();
                return;
            }
            // Bağlamsal 'test' (ADR-0033): Ident("test") + StringLit.
            Some(Ident) if self.at_test_decl() => self.parse_test(),
            None => return,
            _ => {
                let err = Diagnostic::error(
                    ErrorCode::E0001,
                    lstr!(en: "unexpected '{}', expected an item", self.current_text(); tr: "beklenmeyen '{}', öğe bekleniyor", self.current_text()),
                    LabeledSpan::primary(
                        self.current_span(),
                        lstr!(en: "expected an item"; tr: "öğe bekleniyor"),
                    ),
                    lstr!(en: "expected module, domain, fn, struct, enum, const, type or extern"; tr: "module, domain, fn, struct, enum, const, type veya extern bekleniyor"),
                );
                self.recover(ITEM_START, err);
                ItemKind::Error
            }
        };

        let item = Item {
            span: self.span_from(start),
            attrs,
            doc,
            visibility,
            kind,
        };
        let idx = self.ast.items_arena.alloc(item);
        self.ast.items.push(idx);
    }

    /// Ardışık `///` yorumlarını tek doc metnine toplar.
    pub(crate) fn collect_doc_comments(&mut self) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();
        while self.at(DocComment) {
            let span = self.bump();
            let text = self
                .text_of(span)
                .trim_start_matches('/')
                .trim()
                .to_string();
            lines.push(text);
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    // ═══ Nitelikler ═══════════════════════════════════════════════

    /// `@nitelik(...)` dizisini AST'ye ayrıştırır; yorumlamaz.
    /// Bilinmeyen nitelik W0020 üretir (grammar-full.ebnf §2).
    pub(crate) fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attrs = Vec::new();
        while self.at(At) {
            let start = self.pos;
            self.bump_any(); // '@'

            // '@domain' ve '@reg' nitelik adları anahtar kelimeyle çakışır.
            let name = match self.current() {
                Some(Ident) | Some(KwDomain) | Some(KwReg) => self.parse_name(),
                _ => {
                    self.error_expected(
                        &lstr!(en: "attribute name"; tr: "nitelik adı"),
                        &lstr!(en: "write it as @attribute or @attribute(args)"; tr: "@nitelik veya @nitelik(argümanlar) biçiminde yazın"),
                    );
                    continue; // '@' tüketildi, ilerleme garantili
                }
            };

            if !KNOWN_ATTRIBUTES.contains(&name.text.as_str()) {
                let known = KNOWN_ATTRIBUTES
                    .iter()
                    .map(|a| format!("@{a}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                self.push_error(Diagnostic::warning(
                    ErrorCode::W0020,
                    lstr!(en: "unknown attribute: '@{}'", name.text; tr: "bilinmeyen nitelik: '@{}'", name.text),
                    LabeledSpan::primary(
                        name.span,
                        lstr!(en: "unrecognized attribute"; tr: "tanınmayan nitelik"),
                    ),
                    lstr!(en: "known attributes: {known}"; tr: "tanınan nitelikler: {known}"),
                ));
            }

            let mut args = Vec::new();
            if self.at(LParen) {
                let open = self.bump();
                while !self.at(RParen) && !self.at_eof() {
                    let before = self.pos;
                    if self.at(Ident) && matches!(self.peek(1), Some(Eq)) {
                        let arg_name = self.parse_name();
                        self.bump_any(); // '='
                        let value = if self.at_expr_start() {
                            self.parse_expr()
                        } else {
                            self.push_error(Diagnostic::error(
                                ErrorCode::E0009,
                                lstr!(en: "missing value in the '{}' argument", arg_name.text; tr: "'{}' argümanında değer eksik", arg_name.text),
                                LabeledSpan::primary(
                                    self.current_span(),
                                    lstr!(en: "expected a value"; tr: "değer bekleniyor"),
                                ),
                                lstr!(en: "write it as @budget(lut = 5000)"; tr: "@budget(lut = 5000) biçiminde yazın"),
                            ));
                            self.alloc_error_expr(self.current_span())
                        };
                        args.push(AttrArg::Named {
                            name: arg_name,
                            value,
                        });
                    } else if self.at_expr_start() {
                        args.push(AttrArg::Positional(self.parse_expr()));
                    } else {
                        self.push_error(Diagnostic::error(
                            ErrorCode::E0009,
                            lstr!(en: "invalid attribute argument: '{}'", self.current_text(); tr: "geçersiz nitelik argümanı: '{}'", self.current_text()),
                            LabeledSpan::primary(
                                self.current_span(),
                                lstr!(en: "expected an argument"; tr: "argüman bekleniyor"),
                            ),
                            lstr!(en: "write it as name = value or value"; tr: "isim = değer veya değer biçiminde yazın"),
                        ));
                    }
                    if !self.eat(Comma) && self.pos == before {
                        self.bump_any(); // ilerleme garantisi
                    }
                }
                self.expect_closing(RParen, ")", open);
            }

            attrs.push(Attribute {
                span: self.span_from(start),
                name,
                args,
            });
        }
        attrs
    }

    // ═══ Generics ═════════════════════════════════════════════════

    /// `<` tüketilmemişken çağrılır; `<const N: u32, T: Bound>` ayrıştırır.
    pub(crate) fn parse_generic_params(&mut self) -> Vec<GenericParam> {
        let open = self.bump(); // '<'
        let mut params = Vec::new();
        while !self.at(Gt) && !self.at(Shr) && !self.at(LBrace) && !self.at_eof() {
            let before = self.pos;
            let start = self.pos;
            match self.current() {
                Some(KwConst) => {
                    self.bump_any();
                    let name = if self.at(Ident) {
                        self.parse_name()
                    } else {
                        self.error_expected(
                            &lstr!(en: "const parameter name"; tr: "const parametre adı"),
                            &lstr!(en: "write it as <const N: u32>"; tr: "<const N: u32> biçiminde yazın"),
                        );
                        Name {
                            text: String::new(),
                            span: self.current_span(),
                        }
                    };
                    self.expect(
                        Colon,
                        &lstr!(en: "':' in the const parameter"; tr: "const parametrede ':'"),
                        &lstr!(en: "write it as <const N: u32>"; tr: "<const N: u32> biçiminde yazın"),
                    );
                    let ty = self.parse_type_or_error();
                    params.push(GenericParam {
                        span: self.span_from(start),
                        kind: GenericParamKind::Const { name, ty },
                    });
                }
                Some(Ident) => {
                    let name = self.parse_name();
                    let mut bounds = Vec::new();
                    if self.eat(Colon) {
                        loop {
                            if self.at(Ident) {
                                bounds.push(self.parse_path());
                            } else {
                                self.error_expected(
                                    &lstr!(en: "type bound"; tr: "tip sınırı"),
                                    &lstr!(en: "write it as <T: Bound>"; tr: "<T: Bound> biçiminde yazın"),
                                );
                                break;
                            }
                            if !self.eat(Plus) {
                                break;
                            }
                        }
                    }
                    params.push(GenericParam {
                        span: self.span_from(start),
                        kind: GenericParamKind::Type { name, bounds },
                    });
                }
                _ => {
                    self.error_expected(
                        &lstr!(en: "generic parameter"; tr: "generic parametre"),
                        &lstr!(en: "write it as <const N: u32> or <T: Bound>"; tr: "<const N: u32> veya <T: Bound> biçiminde yazın"),
                    );
                }
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        if !self.eat_generic_close() {
            self.expect_closing(Gt, ">", open);
        }
        params
    }

    /// Generic kapanışı `>` tüketir; `>>` tokenını ikiye böler
    /// (`Fifo<Entry<8>>` durumunda lexer Shr üretir).
    pub(crate) fn eat_generic_close(&mut self) -> bool {
        if self.eat(Gt) {
            return true;
        }
        if self.at(Shr) {
            let span = self.current_span();
            self.tokens[self.pos] = Token {
                kind: Gt,
                span: Span::new(span.file, span.start + 1, span.end),
            };
            return true;
        }
        false
    }

    /// `<Tip, İfade, ...>` — GenericArg = Type | Expr (grammar §6).
    pub(crate) fn parse_generic_args(&mut self) -> Vec<GenericArg> {
        let open = self.bump(); // '<'
        let mut args = Vec::new();
        while !self.at(Gt) && !self.at(Shr) && !self.at_eof() && !self.at(LBrace) {
            let before = self.pos;
            match self.current() {
                Some(
                    KwBool | KwClock | KwReset | UIntType | SIntType | KwTrit | KwBits | LBracket
                    | LParen | Ident,
                ) => {
                    args.push(GenericArg::Type(self.parse_type_or_error()));
                }
                _ if self.at_expr_start() => {
                    // '>'/'>>' kapanış sanılsın diye kaydırma-üstü bp
                    args.push(GenericArg::Const(self.parse_expr_bp(ABOVE_SHIFT_BP)));
                }
                _ => {
                    self.error_expected(
                        &lstr!(en: "generic argument"; tr: "generic argüman"),
                        &lstr!(en: "expected a type or constant expression"; tr: "tip veya sabit ifade bekleniyor"),
                    );
                }
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        if !self.eat_generic_close() {
            self.expect_closing(Gt, ">", open);
        }
        args
    }

    // ═══ Modül ════════════════════════════════════════════════════

    fn parse_module(&mut self) -> ItemKind {
        self.bump_any(); // 'module'

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "module name"; tr: "modül adı"),
                &lstr!(en: "write it as module Name {{ ... }}"; tr: "module Ad {{ ... }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let generics = if self.at(Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the module body"; tr: "modül gövdesi için '{{'"),
            &lstr!(en: "write it as module Name {{ ... }}"; tr: "module Ad {{ ... }} biçiminde yazın"),
        );

        // Gövde: portlar, kontratlar ve deyimler. Gramer sırayı önerir ama
        // kurtarma dostu olması için tek döngüde ayrıştırılır — bozuk bir
        // porttan sonra gelen portlar da yakalanır (error-recovery.md §4.2).
        let mut ports = Vec::new();
        let mut contracts = Vec::new();
        let mut body = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            let doc = self.collect_doc_comments();
            let attrs = self.parse_attributes();
            match self.current() {
                Some(KwIn) | Some(KwOut) | Some(KwInout) => {
                    if let Some(port) = self.parse_port(attrs, doc) {
                        ports.push(port);
                    }
                }
                Some(k) if contract_kind(k).is_some() => {
                    contracts.push(self.parse_contract());
                }
                Some(RBrace) | None => break,
                _ => {
                    body.push(self.parse_stmt(attrs));
                }
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);

        // Opsiyonel sonlandırıcı: '} module Ad' — LL(2) ayrımı:
        // 'module Ad {' yeni öğedir, 'module Ad' (ardında '{' yok) sonlandırıcıdır.
        let mut closing_name = None;
        if self.at(KwModule)
            && matches!(self.peek(1), Some(Ident))
            && !matches!(self.peek(2), Some(LBrace) | Some(Lt))
        {
            self.bump_any(); // 'module'
            let cname = self.parse_name();
            if !name.text.is_empty() && cname.text != name.text {
                self.push_error(
                    Diagnostic::error(
                        ErrorCode::E0004,
                        lstr!(en: "block closing name does not match: expected '{}', found '{}'", name.text, cname.text; tr: "blok sonlandırma ismi uyuşmuyor: '{}' bekleniyor, '{}' bulundu", name.text, cname.text),
                        LabeledSpan::primary(
                            cname.span,
                            lstr!(en: "wrong name"; tr: "yanlış isim"),
                        ),
                        lstr!(en: "write 'module {}' or remove the terminator", name.text; tr: "'module {}' yazın veya sonlandırıcıyı kaldırın", name.text),
                    )
                    .with_secondary(
                        name.span,
                        lstr!(en: "module defined here"; tr: "modül burada tanımlandı"),
                    ),
                );
            }
            closing_name = Some(cname);
        }

        ItemKind::Module(ModuleDecl {
            name,
            generics,
            ports,
            contracts,
            body,
            closing_name,
        })
    }

    fn parse_port(&mut self, attrs: Vec<Attribute>, doc: Option<String>) -> Option<Port> {
        let start = self.pos;
        let direction = match self.current() {
            Some(KwIn) => PortDir::In,
            Some(KwOut) => PortDir::Out,
            _ => PortDir::InOut,
        };
        self.bump_any();

        if !self.at(Ident) {
            self.error_expected(
                &lstr!(en: "port name"; tr: "port adı"),
                &lstr!(en: "write it as in name : type"; tr: "in isim : tip biçiminde yazın"),
            );
            self.recover_silent(PORT_RECOVERY);
            return None;
        }
        let name = self.parse_name();

        if !self.eat(Colon) {
            self.error_expected(
                &lstr!(en: "':' in the port definition"; tr: "port tanımında ':'"),
                &lstr!(en: "write it as in {} : u8", name.text; tr: "in {} : u8 şeklinde yazın", name.text),
            );
            self.recover_silent(PORT_RECOVERY);
            let ty = self.alloc_error_type(self.current_span());
            return Some(Port {
                span: self.span_from(start),
                attrs,
                doc,
                direction,
                name,
                ty,
                domain: None,
            });
        }

        let ty = self.parse_type_or_error();

        let domain = if self.eat(At) {
            if self.at(Ident) {
                Some(self.parse_name())
            } else {
                self.error_expected(
                    &lstr!(en: "domain name after '@'"; tr: "'@' sonrasında domain adı"),
                    &lstr!(en: "write it as in a : u8 @Fast"; tr: "in a : u8 @Fast biçiminde yazın"),
                );
                None
            }
        } else {
            None
        };

        self.eat(Comma);
        Some(Port {
            span: self.span_from(start),
            attrs,
            doc,
            direction,
            name,
            ty,
            domain,
        })
    }

    /// `requires: ifade [;]` — davranışsal kontrat (grammar §5).
    /// Yalnız ayrıştırılır; doğrulama F4'te.
    pub(crate) fn parse_contract(&mut self) -> Contract {
        let start = self.pos;
        let kind = contract_kind(self.current().unwrap_or(Error)).unwrap_or(ContractKind::Assert);
        self.bump_any();
        self.expect(
            Colon,
            &lstr!(en: "':' for the contract"; tr: "kontrat için ':'"),
            &lstr!(en: "write it as requires: cond"; tr: "requires: koşul biçiminde yazın"),
        );
        let expr = if self.at_expr_start() {
            self.parse_expr()
        } else {
            self.error_expected(
                &lstr!(en: "contract condition"; tr: "kontrat koşulu"),
                &lstr!(en: "write it as requires: a < b"; tr: "requires: a < b biçiminde yazın"),
            );
            self.alloc_error_expr(self.current_span())
        };
        self.eat(Semi);
        Contract {
            span: self.span_from(start),
            kind,
            expr,
        }
    }

    // ═══ Fonksiyon ════════════════════════════════════════════════

    /// `fn ad(params) -> Tip { ... son_ifade }` (grammar §8).
    fn parse_fn(&mut self) -> ItemKind {
        self.bump_any(); // 'fn'

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "function name"; tr: "fonksiyon adı"),
                &lstr!(en: "write it as fn name(a: u8) -> u8 {{ ... }}"; tr: "fn ad(a: u8) -> u8 {{ ... }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let generics = if self.at(Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };

        let mut params = Vec::new();
        let open = self.current_span();
        if self.expect(
            LParen,
            &lstr!(en: "'(' for the parameter list"; tr: "parametre listesi için '('"),
            &lstr!(en: "write it as fn name(a: u8)"; tr: "fn ad(a: u8) biçiminde yazın"),
        ) {
            while !self.at(RParen) && !self.at_eof() {
                let before = self.pos;
                let pstart = self.pos;
                if self.at(Ident) {
                    let pname = self.parse_name();
                    self.expect(
                        Colon,
                        &lstr!(en: "':' in the parameter"; tr: "parametrede ':'"),
                        &lstr!(en: "write it as a: u8"; tr: "a: u8 biçiminde yazın"),
                    );
                    let ty = self.parse_type_or_error();
                    params.push(Param {
                        span: self.span_from(pstart),
                        name: pname,
                        ty,
                    });
                } else {
                    self.error_expected(
                        &lstr!(en: "parameter name"; tr: "parametre adı"),
                        &lstr!(en: "write it as a: u8"; tr: "a: u8 biçiminde yazın"),
                    );
                }
                if !self.eat(Comma) && self.pos == before {
                    self.bump_any(); // ilerleme garantisi
                }
            }
            self.expect_closing(RParen, ")", open);
        }

        let return_ty = if self.eat(Arrow) {
            Some(self.parse_type_or_error())
        } else {
            None
        };

        let mut contracts = Vec::new();
        while contract_kind(self.current().unwrap_or(Error)).is_some() {
            contracts.push(self.parse_contract());
        }

        let body = self.parse_fn_block();

        ItemKind::Fn(FnDecl {
            name,
            generics,
            params,
            return_ty,
            contracts,
            body,
        })
    }

    // ═══ Struct / Enum ════════════════════════════════════════════

    fn parse_struct(&mut self) -> ItemKind {
        self.bump_any(); // 'struct'

        // `struct port İsim` — 'port' bağlamsal belirteç [V1].
        let is_port =
            self.at(Ident) && self.current_text() == "port" && matches!(self.peek(1), Some(Ident));
        if is_port {
            self.bump_any();
        }

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "struct name"; tr: "struct adı"),
                &lstr!(en: "write it as struct Name {{ field: type }}"; tr: "struct Ad {{ alan: tip }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let generics = if self.at(Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the struct body"; tr: "struct gövdesi için '{{'"),
            &lstr!(en: "write it as struct Name {{ ... }}"; tr: "struct Ad {{ ... }} biçiminde yazın"),
        );
        let fields = self.parse_struct_fields();
        self.expect_closing(RBrace, "}", open);

        ItemKind::Struct(StructDecl {
            name,
            is_port,
            generics,
            fields,
        })
    }

    /// `{` sonrası alan listesi; `}` tüketmez.
    fn parse_struct_fields(&mut self) -> Vec<StructField> {
        let mut fields = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            let start = self.pos;
            let doc = self.collect_doc_comments();
            let attrs = self.parse_attributes();
            if self.at(Ident) {
                let name = self.parse_name();
                self.expect(
                    Colon,
                    &lstr!(en: "':' in the struct field"; tr: "struct alanında ':'"),
                    &lstr!(en: "write it as field: u8"; tr: "alan: u8 biçiminde yazın"),
                );
                let ty = self.parse_type_or_error();
                fields.push(StructField {
                    span: self.span_from(start),
                    attrs,
                    doc,
                    name,
                    ty,
                });
            } else if !self.at(RBrace) {
                self.error_expected(
                    &lstr!(en: "struct field"; tr: "struct alanı"),
                    &lstr!(en: "write it as field: type"; tr: "alan: tip biçiminde yazın"),
                );
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }
        fields
    }

    fn parse_enum(&mut self) -> ItemKind {
        self.bump_any(); // 'enum'

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "enum name"; tr: "enum adı"),
                &lstr!(en: "write it as enum Name {{ Variant }}"; tr: "enum Ad {{ Varyant }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let generics = if self.at(Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };

        // `enum State : bits<2>` — temel tip.
        let repr = if self.eat(Colon) {
            Some(self.parse_type_or_error())
        } else {
            None
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the enum body"; tr: "enum gövdesi için '{{'"),
            &lstr!(en: "write it as enum Name {{ Variant }}"; tr: "enum Ad {{ Varyant }} biçiminde yazın"),
        );

        let mut variants = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            let start = self.pos;
            let doc = self.collect_doc_comments();
            if self.at(Ident) {
                let vname = self.parse_name();
                let data = match self.current() {
                    Some(LParen) => {
                        let popen = self.bump();
                        let mut tys = Vec::new();
                        while !self.at(RParen) && !self.at_eof() {
                            let tbefore = self.pos;
                            tys.push(self.parse_type_or_error());
                            if !self.eat(Comma) && self.pos == tbefore {
                                self.bump_any(); // ilerleme garantisi
                            }
                        }
                        self.expect_closing(RParen, ")", popen);
                        VariantData::Tuple(tys)
                    }
                    Some(LBrace) => {
                        let sopen = self.bump();
                        let fields = self.parse_struct_fields();
                        self.expect_closing(RBrace, "}", sopen);
                        VariantData::Struct(fields)
                    }
                    _ => VariantData::Unit,
                };
                let discriminant = if self.eat(Eq) {
                    if self.at_expr_start() {
                        Some(self.parse_expr())
                    } else {
                        self.error_expected(
                            &lstr!(en: "variant value"; tr: "varyant değeri"),
                            &lstr!(en: "write it as Idle = 0"; tr: "Idle = 0 biçiminde yazın"),
                        );
                        None
                    }
                } else {
                    None
                };
                variants.push(EnumVariant {
                    span: self.span_from(start),
                    doc,
                    name: vname,
                    data,
                    discriminant,
                });
            } else if !self.at(RBrace) {
                self.error_expected(
                    &lstr!(en: "enum variant"; tr: "enum varyantı"),
                    &lstr!(en: "expected Variant, Variant(type) or Variant = value"; tr: "Varyant, Varyant(tip) veya Varyant = değer bekleniyor"),
                );
            }
            if !self.eat(Comma) && self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        ItemKind::Enum(EnumDecl {
            name,
            generics,
            repr,
            variants,
        })
    }

    // ═══ Const / Type alias / Extern ══════════════════════════════

    /// `const AD : Tip = ifade ;`
    fn parse_const(&mut self) -> ItemKind {
        self.bump_any(); // 'const'

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "constant name"; tr: "sabit adı"),
                &lstr!(en: "write it as const NAME : u32 = 8;"; tr: "const AD : u32 = 8; biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        self.expect(
            Colon,
            &lstr!(en: "':' in the const declaration"; tr: "const bildiriminde ':'"),
            &lstr!(en: "write it as const NAME : u32 = 8;"; tr: "const AD : u32 = 8; biçiminde yazın"),
        );
        let ty = self.parse_type_or_error();
        self.expect(
            Eq,
            &lstr!(en: "'=' for the const"; tr: "const için '='"),
            &lstr!(en: "write it as const NAME : u32 = 8;"; tr: "const AD : u32 = 8; biçiminde yazın"),
        );
        let value = if self.at_expr_start() {
            self.parse_expr()
        } else {
            self.error_expected(
                &lstr!(en: "constant value"; tr: "sabit değeri"),
                &lstr!(en: "write it as const NAME : u32 = 8;"; tr: "const AD : u32 = 8; biçiminde yazın"),
            );
            self.alloc_error_expr(self.current_span())
        };
        self.eat(Semi);

        ItemKind::Const(ConstDecl { name, ty, value })
    }

    /// `type Ad = Tip ;`
    fn parse_type_alias(&mut self) -> ItemKind {
        self.bump_any(); // 'type'

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "type alias name"; tr: "tip takma adı"),
                &lstr!(en: "write it as type Kelime = bits<32>;"; tr: "type Kelime = bits<32>; biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let generics = if self.at(Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };

        self.expect(
            Eq,
            &lstr!(en: "'=' for the type alias"; tr: "type için '='"),
            &lstr!(en: "write it as type Kelime = bits<32>;"; tr: "type Kelime = bits<32>; biçiminde yazın"),
        );
        let target = self.parse_type_or_error();
        self.eat(Semi);

        ItemKind::TypeAlias(TypeAlias {
            name,
            generics,
            target,
        })
    }

    /// `extern module Ad { portlar }` — harici SV modülü sarmalayıcı.
    fn parse_extern(&mut self) -> ItemKind {
        self.bump_any(); // 'extern'
        self.expect(
            KwModule,
            &lstr!(en: "'module' after 'extern'"; tr: "'extern' sonrası 'module'"),
            &lstr!(en: "write it as extern module Name {{ in a : u8 }}"; tr: "extern module Ad {{ in a : u8 }} biçiminde yazın"),
        );

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "extern module name"; tr: "extern modül adı"),
                &lstr!(en: "write it as extern module Name {{ ... }}"; tr: "extern module Ad {{ ... }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let generics = if self.at(Lt) {
            self.parse_generic_params()
        } else {
            Vec::new()
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the extern body"; tr: "extern gövdesi için '{{'"),
            &lstr!(en: "write it as extern module Name {{ ... }}"; tr: "extern module Ad {{ ... }} biçiminde yazın"),
        );

        let mut ports = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            let doc = self.collect_doc_comments();
            let attrs = self.parse_attributes();
            match self.current() {
                Some(KwIn) | Some(KwOut) | Some(KwInout) => {
                    if let Some(port) = self.parse_port(attrs, doc) {
                        ports.push(port);
                    }
                }
                Some(RBrace) | None => break,
                _ => {
                    self.error_expected(
                        &lstr!(en: "port in the extern module"; tr: "extern modülde port"),
                        &lstr!(en: "an extern body contains only ports"; tr: "extern gövdesi yalnız port içerir"),
                    );
                    self.recover_silent(PORT_RECOVERY);
                }
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        ItemKind::Extern(ExternDecl {
            name,
            generics,
            ports,
        })
    }

    // ═══ Tipler ═══════════════════════════════════════════════════

    /// Tip ayrıştırma (grammar §6); tanınmazsa E0001 + Error tipi
    /// (token TÜKETMEZ — çağıran bağlam kurtarır).
    pub(crate) fn parse_type_or_error(&mut self) -> Idx<TypeRef> {
        let start = self.pos;
        let kind = match self.current() {
            Some(KwBool) => {
                self.bump_any();
                TypeRefKind::Bool
            }
            Some(KwClock) => {
                self.bump_any();
                TypeRefKind::Clock
            }
            Some(KwReset) => {
                self.bump_any();
                let spec = if self.at(LParen) {
                    let open = self.bump();
                    let sync = match self.current() {
                        Some(Ident) if self.current_text() == "sync" => {
                            self.bump_any();
                            ResetSync::Sync
                        }
                        Some(Ident) if self.current_text() == "async" => {
                            self.bump_any();
                            ResetSync::Async
                        }
                        _ => {
                            self.error_expected(
                                &lstr!(en: "reset kind"; tr: "reset türü"),
                                &lstr!(en: "write it as reset(sync, active_high)"; tr: "reset(sync, active_high) biçiminde yazın"),
                            );
                            ResetSync::Sync
                        }
                    };
                    self.expect(
                        Comma,
                        &lstr!(en: "',' in the reset kind"; tr: "reset türünde ','"),
                        &lstr!(en: "write it as reset(sync, active_high)"; tr: "reset(sync, active_high) biçiminde yazın"),
                    );
                    let polarity = match self.current() {
                        Some(KwActiveHigh) => {
                            self.bump_any();
                            ResetPolarity::ActiveHigh
                        }
                        Some(KwActiveLow) => {
                            self.bump_any();
                            ResetPolarity::ActiveLow
                        }
                        _ => {
                            self.error_expected(
                                &lstr!(en: "reset polarity"; tr: "sıfırlama polaritesi"),
                                &lstr!(en: "write it as reset(sync, active_high)"; tr: "reset(sync, active_high) biçiminde yazın"),
                            );
                            ResetPolarity::ActiveHigh
                        }
                    };
                    self.expect_closing(RParen, ")", open);
                    Some(ResetSpec { sync, polarity })
                } else {
                    None
                };
                TypeRefKind::Reset(spec)
            }
            Some(UIntType) | Some(SIntType) => self.parse_int_type(),
            Some(KwTrit) => {
                self.bump_any();
                TypeRefKind::Trit
            }
            Some(KwBits) => {
                self.bump_any();
                self.expect(
                    Lt,
                    &lstr!(en: "'<' for bits"; tr: "bits için '<'"),
                    &lstr!(en: "write it as bits<8>"; tr: "bits<8> biçiminde yazın"),
                );
                // '>'/'>>' kapanış sanılsın diye kaydırma-üstü bp ile ayrıştır
                let n = if self.at_expr_start() {
                    self.parse_expr_bp(ABOVE_SHIFT_BP)
                } else {
                    self.error_expected(
                        &lstr!(en: "bits width expression"; tr: "bits genişlik ifadesi"),
                        &lstr!(en: "write it as bits<8>"; tr: "bits<8> biçiminde yazın"),
                    );
                    self.alloc_error_expr(self.current_span())
                };
                if !self.eat_generic_close() {
                    self.error_expected(
                        &lstr!(en: "closing '>' for bits"; tr: "bits için kapanış '>'"),
                        &lstr!(en: "write it as bits<8>"; tr: "bits<8> biçiminde yazın"),
                    );
                }
                TypeRefKind::Bits(n)
            }
            // `[Tip; N]` — dizi tipi
            Some(LBracket) => {
                let open = self.bump();
                let elem = self.parse_type_or_error();
                self.expect(
                    Semi,
                    &lstr!(en: "';' in the array type"; tr: "dizi tipinde ';'"),
                    &lstr!(en: "write it as [u8; 4]"; tr: "[u8; 4] biçiminde yazın"),
                );
                let len = if self.at_expr_start() {
                    self.parse_expr()
                } else {
                    self.error_expected(
                        &lstr!(en: "array length"; tr: "dizi uzunluğu"),
                        &lstr!(en: "write it as [u8; 4]"; tr: "[u8; 4] biçiminde yazın"),
                    );
                    self.alloc_error_expr(self.current_span())
                };
                self.expect_closing(RBracket, "]", open);
                TypeRefKind::Array { elem, len }
            }
            // `(T, U)` — tuple tipi
            Some(LParen) => {
                let open = self.bump();
                let mut elems = Vec::new();
                while !self.at(RParen) && !self.at_eof() {
                    let before = self.pos;
                    elems.push(self.parse_type_or_error());
                    if !self.eat(Comma) && self.pos == before {
                        self.bump_any(); // ilerleme garantisi
                    }
                }
                self.expect_closing(RParen, ")", open);
                TypeRefKind::Tuple(elems)
            }
            // Kullanıcı tanımlı tip: `State`, `Fifo<8>`, `pkg::Tip`
            Some(Ident) => {
                let path = self.parse_path();
                let args = if self.at(Lt) {
                    self.parse_generic_args()
                } else {
                    Vec::new()
                };
                TypeRefKind::Path { path, args }
            }
            _ => {
                self.error_expected(
                    &lstr!(en: "type"; tr: "tip"),
                    &lstr!(en: "use bool, clock, reset, u8..u64, i8..i64, Trit, bits<N>, [T; N], (T, U) or a type name"; tr: "bool, clock, reset, u8..u64, i8..i64, Trit, bits<N>, [T; N], (T, U) veya tip adı kullanın"),
                );
                TypeRefKind::Error
            }
        };
        let span = self.span_from(start);
        self.ast.types.alloc(TypeRef { span, kind })
    }

    /// `uN`/`iN` tip ailesi (ADR-0031): 1..=64 doğrudan UInt/SInt olur.
    /// 64 üstü genişlikler eski davranışla Path (widened) taşınır —
    /// typeck aileyi tanır, SV eşlemesi sonraki aşamaların işidir.
    fn parse_int_type(&mut self) -> TypeRefKind {
        let signed = self.at(SIntType);
        let name = self.parse_name();
        let width = name.text[1..]
            .parse::<u32>()
            .ok()
            .filter(|w| (1..=64).contains(w));
        match width {
            Some(w) if signed => TypeRefKind::SInt(w as u8),
            Some(w) => TypeRefKind::UInt(w as u8),
            None => TypeRefKind::Path {
                path: Path {
                    span: name.span,
                    segments: vec![name],
                },
                args: Vec::new(),
            },
        }
    }

    // ═══ Domain ═══════════════════════════════════════════════════

    fn parse_domain(&mut self) -> ItemKind {
        self.bump_any(); // 'domain'

        let name = if self.at(Ident) {
            self.parse_name()
        } else {
            self.error_expected(
                &lstr!(en: "domain name"; tr: "domain adı"),
                &lstr!(en: "write it as domain Name {{ clock = posedge }}"; tr: "domain Ad {{ clock = posedge }} biçiminde yazın"),
            );
            Name {
                text: String::new(),
                span: self.current_span(),
            }
        };

        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the domain body"; tr: "domain gövdesi için '{{'"),
            &lstr!(en: "write it as domain Name {{ ... }}"; tr: "domain Ad {{ ... }} biçiminde yazın"),
        );

        let mut fields = Vec::new();
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            if let Some(field) = self.parse_domain_field() {
                fields.push(field);
            }
            self.eat(Comma);
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        ItemKind::Domain(DomainDecl { name, fields })
    }

    fn parse_domain_field(&mut self) -> Option<DomainField> {
        let start = self.pos;
        let key = match self.current() {
            Some(KwClock) => {
                self.bump_any();
                DomainKey::Clock
            }
            Some(KwReset) => {
                self.bump_any();
                DomainKey::Reset
            }
            Some(Ident) => {
                let name = self.parse_name();
                match name.text.as_str() {
                    "frequency" => DomainKey::Frequency,
                    "reset_cycles" => DomainKey::ResetCycles,
                    "reset_sequence" => DomainKey::ResetSequence,
                    _ => {
                        self.push_error(Diagnostic::warning(
                            ErrorCode::W0020,
                            lstr!(en: "unknown domain key: '{}'", name.text; tr: "bilinmeyen domain anahtarı: '{}'", name.text),
                            LabeledSpan::primary(
                                name.span,
                                lstr!(en: "unrecognized key"; tr: "tanınmayan anahtar"),
                            ),
                            lstr!(en: "valid keys: clock, frequency, reset, reset_cycles, reset_sequence"; tr: "geçerli anahtarlar: clock, frequency, reset, reset_cycles, reset_sequence"),
                        ));
                        DomainKey::Unknown(name)
                    }
                }
            }
            _ => {
                self.error_expected(
                    &lstr!(en: "domain key"; tr: "domain anahtarı"),
                    &lstr!(en: "write a field such as clock = posedge"; tr: "clock = posedge gibi bir alan yazın"),
                );
                return None;
            }
        };

        self.expect(
            Eq,
            &lstr!(en: "'=' in the domain field"; tr: "domain alanında '='"),
            &lstr!(en: "write it as clock = posedge"; tr: "clock = posedge biçiminde yazın"),
        );
        // ADR-0023: sync/async yalnız "reset =" değer konumunda anahtar kelime.
        let value = self.parse_domain_value(matches!(key, DomainKey::Reset));
        Some(DomainField {
            span: self.span_from(start),
            key,
            value,
        })
    }

    fn parse_domain_value(&mut self, in_reset: bool) -> DomainValue {
        match self.current() {
            Some(KwPosedge) => {
                self.bump_any();
                DomainValue::ClockEdge(ClockEdge::Posedge)
            }
            Some(KwNegedge) => {
                self.bump_any();
                DomainValue::ClockEdge(ClockEdge::Negedge)
            }
            Some(KwNone) => {
                self.bump_any();
                DomainValue::ClockEdge(ClockEdge::None)
            }
            // Bağlamsal anahtar kelime (ADR-0023): lexer Ident üretti,
            // yalnız reset değer konumunda ResetSync olarak yorumlanır.
            Some(Ident) if in_reset && matches!(self.current_text(), "sync" | "async") => {
                let sync = if self.current_text() == "sync" {
                    ResetSync::Sync
                } else {
                    ResetSync::Async
                };
                self.bump_any();
                let polarity = match self.current() {
                    Some(KwActiveHigh) => {
                        self.bump_any();
                        ResetPolarity::ActiveHigh
                    }
                    Some(KwActiveLow) => {
                        self.bump_any();
                        ResetPolarity::ActiveLow
                    }
                    _ => {
                        self.error_expected(
                            &lstr!(en: "reset polarity"; tr: "sıfırlama polaritesi"),
                            &lstr!(en: "write it as sync active_high or async active_low"; tr: "sync active_high veya async active_low biçiminde yazın"),
                        );
                        ResetPolarity::ActiveHigh
                    }
                };
                DomainValue::Reset(ResetSpec { sync, polarity })
            }
            Some(KwTrue) => {
                self.bump_any();
                DomainValue::Bool(true)
            }
            Some(KwFalse) => {
                self.bump_any();
                DomainValue::Bool(false)
            }
            _ if self.at_expr_start() => DomainValue::Literal(self.parse_expr()),
            _ => {
                self.error_expected(
                    &lstr!(en: "domain value"; tr: "domain değeri"),
                    &lstr!(en: "expected posedge, negedge, none, sync/async + polarity or a literal"; tr: "posedge, negedge, none, sync/async + polarite veya literal bekleniyor"),
                );
                DomainValue::Error
            }
        }
    }

    // ═══ Fonksiyon gövdesi ════════════════════════════════════════

    /// `FnBlock = "{" { Stmt } [ Expr ] "}"` — son ifade dönüş değeri
    /// (Rust semantiği). AST'de Block::tail alanına yazılır.
    fn parse_fn_block(&mut self) -> Idx<volt_ast::Block> {
        let start = self.pos;
        let open = self.current_span();
        self.expect(
            LBrace,
            &lstr!(en: "'{{' for the function body"; tr: "fonksiyon gövdesi için '{{'"),
            &lstr!(en: "write it as fn name() -> u8 {{ expression }}"; tr: "fn ad() -> u8 {{ ifade }} biçiminde yazın"),
        );

        let mut stmts = Vec::new();
        let mut tail = None;
        while !self.at(RBrace) && !self.at_eof() {
            let before = self.pos;
            match self.current() {
                Some(KwLet) => match self.parse_let() {
                    Some(decl) => stmts.push(BlockStmt::Let(decl)),
                    None => stmts.push(BlockStmt::Error),
                },
                Some(KwFor) => {
                    stmts.push(BlockStmt::For(self.parse_for_stmt(BlockContext::Function)));
                }
                _ if self.at_expr_start() => {
                    let expr = self.parse_expr();
                    if self.at(RBrace) {
                        tail = Some(expr);
                    } else {
                        self.error_expected(
                            &lstr!(en: "the final expression of the function right before '}}'"; tr: "fonksiyonda son ifade '}}' öncesinde"),
                            &lstr!(en: "bind intermediate values with let; the final expression is the return value"; tr: "ara değerleri let ile bağlayın; son ifade dönüş değeridir"),
                        );
                        self.eat(Semi);
                        stmts.push(BlockStmt::Error);
                    }
                }
                _ => {
                    let err = Diagnostic::error(
                        ErrorCode::E0001,
                        lstr!(en: "unexpected '{}', expected let or an expression in the function body", self.current_text(); tr: "beklenmeyen '{}', fonksiyon gövdesinde let veya ifade bekleniyor", self.current_text()),
                        LabeledSpan::primary(
                            self.current_span(),
                            lstr!(en: "expected a statement"; tr: "deyim bekleniyor"),
                        ),
                        lstr!(en: "write a let binding or a return expression"; tr: "let bağlaması veya dönüş ifadesi yazın"),
                    );
                    self.recover(super::recovery::BLOCK_STMT_START, err);
                    stmts.push(BlockStmt::Error);
                }
            }
            if self.pos == before {
                self.bump_any(); // ilerleme garantisi
            }
        }

        self.expect_closing(RBrace, "}", open);
        let span = self.span_from(start);
        self.ast.blocks.alloc(volt_ast::Block {
            span,
            stmts,
            tail,
            context: BlockContext::Function,
        })
    }
}
