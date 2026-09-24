//! Bundle (port grubu) düzleştirme (ADR-0039).
//!
//! `struct port` tipli bir modül portu (`in aw : AxiWriteAddr`) parser
//! sonunda düz portlara açılır: her alan `<port>_<alan>` adlı bağımsız
//! bir `Port` olur, yönü alanın bildirilen yönüdür ve port `in` ise
//! TERS çevrilir. Gövde ve kontratlardaki `aw.addr` erişimleri düz
//! isme (`aw_addr`) yeniden yazılır. Çıktı sıradan portlardır — isim
//! çözümleme, tip denetimi, domain çıkarımı ve SV üretimi bundle'ı
//! görmez (ADR-0038'in silme ilkesi); yalnız `Port::bundle` kaynağı
//! E4005/E3013 tanıları için taşınır.
//!
//! Yerleşik `Handshake<T>` bundle'ı (ADR-0050) aynı yoldan açılır;
//! ayrıntısı `handshake.rs`'de: sanal alanlar (`fired`/`stalled`)
//! ifade olarak yeniden yazılır, protokol kontratları otomatik üretilir.

//!
//! Bundle DİZİSİ portu (ADR-0056, `in ch : [Handshake<u8>; 4]`) eleman
//! eleman aynı yoldan açılır: `ch_0_data`, `ch_0_valid`, ...; gövdedeki
//! `ch[0].valid` (indeks derleme zamanı sabiti — modül seviyesi `for`
//! değişkeni açılımda literale iner) düz isme yeniden yazılır. Sabit
//! olmayan ya da aralık dışı indeks E2008.

use std::collections::HashMap;

use volt_ast::{
    BinOp, Block, BlockStmt, BundleOrigin, ElseBranch, Expr, ExprKind, Idx, IfStmt, Item, ItemKind,
    LValue, LValueSuffix, Name, Path, Port, PortDir, Stmt, StmtKind, TypeRef, TypeRefKind, UnOp,
};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan, NoteKind};
use volt_span::Span;

use super::desugar::{collect_arm_idxs, expr_children, lvalue_suffix_exprs};
use super::handshake::{has_attr, HandshakeInfo, HANDSHAKE, NO_PROTOCOL_CHECK};
use super::mono::unroll::{collect_consts, eval_const};
use super::Parser;

/// Bundle dizisi portunun en fazla eleman sayısı (ADR-0056).
const MAX_BUNDLE_ARRAY: i128 = 256;

/// İç içe bundle derinlik sınırı (ADR-0067): aşılırsa E4010. Kendine
/// referanslı tanım buraya gelmez — döngü tespiti (E4009) onu
/// düzleştirmeden önce eler; bu sınır yalnız yığını korur.
pub(super) const MAX_NESTING: usize = 8;

/// Bir modülün düzleştirme sonrası en fazla port sayısı (ADR-0067):
/// 256 elemanlı bundle dizisi × 16 alanlık arayüz. Döngüsüz ama elmas
/// biçimli bir bundle çizgesi de üstel açılır; bütçe aşımı E4010'dur.
pub(super) const MAX_FLAT_PORTS: usize = 4096;

/// Düzleştirme bütçesinin hangi yönde aşıldığı (E4010).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Overflow {
    Ports,
    Depth,
}

/// `struct port` alanı (düzleştirme girdisi).
struct FieldInfo {
    dir: PortDir,
    name: String,
    ty: Idx<TypeRef>,
    domain: Option<Name>,
    /// Alan tipi başka bir bundle ise adı.
    nested: Option<String>,
}

/// Modül başına yeniden yazma haritası: `aw.addr` → `aw_addr`.
#[derive(Default)]
pub(super) struct Flat {
    pub names: HashMap<String, String>,
    /// Sanal alanlar (ADR-0050): `tx.fired` → `tx_valid && tx_ready`.
    pub virtuals: HashMap<String, Virtual>,
    /// Bundle dizisi portları (ADR-0056): ad → eleman sayısı; `ch[i]`
    /// indeksi sabit olmalı (E2008).
    pub arrays: HashMap<String, i128>,
    /// Bütçe aşımı (ADR-0067): ilk aşım ve aşıldığı port; açılım durur,
    /// modül başına bir E4010 üretilir.
    pub budget: Option<(Overflow, Name)>,
}

/// Bir sanal alanın açılımı: `valid && ready` ya da `valid && !ready`.
pub(super) struct Virtual {
    pub valid: String,
    pub ready: String,
    pub negate_ready: bool,
}

impl Flat {
    fn lookup(&self, key: &str) -> Option<&str> {
        self.names.get(key).map(String::as_str)
    }

    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.virtuals.is_empty()
    }
}

/// Bir portun / alanın tipi tek segmentli, argümansız bir yol ise adı.
fn simple_type_name(types: &volt_ast::Arena<TypeRef>, ty: Idx<TypeRef>) -> Option<&str> {
    match &types[ty].kind {
        TypeRefKind::Path { path, args } if args.is_empty() && path.segments.len() == 1 => {
            Some(path.segments[0].text.as_str())
        }
        _ => None,
    }
}

pub(super) fn flip(dir: PortDir) -> PortDir {
    match dir {
        PortDir::In => PortDir::Out,
        PortDir::Out => PortDir::In,
        PortDir::InOut => PortDir::InOut,
        PortDir::OpenDrain => PortDir::OpenDrain,
    }
}

/// Sentetik isimler için benzersiz span (decl_spans / use_spans
/// anahtarı). Önce port bildirimi içindeki tek karakterlik konumlar
/// (tanılar doğru satırı gösterir; bundle portunun kendi adı
/// bildirilmediğinden çakışmaz), tükenirse öğe sonundan geriye sayan
/// sıfır genişlikli konumlar (pipeline desugar'ı öğe BAŞINDAN ileri
/// sayar). `counter` modül başına tektir.
/// Mono bağlamı (`ctx`) korunur: düzleştirme monomorfizasyondan sonra
/// koşar, aynı şablonun iki monomorfu aynı konumları üretir.
pub(super) fn fresh_name_span(port_span: Span, item_span: Span, counter: &mut u32) -> Span {
    *counter += 1;
    if port_span.start + *counter <= port_span.end {
        let p = port_span.start + *counter - 1;
        Span::new(port_span.file, p, p + 1).with_ctx(port_span.ctx)
    } else {
        let p = item_span.end.saturating_sub(*counter).max(item_span.start);
        Span::new(item_span.file, p, p).with_ctx(item_span.ctx)
    }
}

/// `[T; N]` port tipi: (eleman tipi, uzunluk ifadesi).
fn array_type(
    types: &volt_ast::Arena<TypeRef>,
    ty: Idx<TypeRef>,
) -> Option<(Idx<TypeRef>, Idx<Expr>)> {
    match &types[ty].kind {
        TypeRefKind::Array { elem, len } => Some((*elem, *len)),
        _ => None,
    }
}

impl Parser<'_> {
    /// Dosyadaki tüm modül ve extern modüllerde bundle portlarını açar.
    pub(crate) fn flatten_bundles(&mut self) {
        let defs = self.collect_bundle_defs();
        // Kullanıcı `struct port Handshake` (generic olsun olmasın) yazdıysa
        // yerleşik devre dışı.
        let builtin_handshake = !self.user_defines_handshake() && self.uses_builtin_handshake();
        if defs.is_empty() && !builtin_handshake {
            return;
        }
        let plain = self.collect_plain_struct_defs();
        let consts = collect_consts(&self.ast);
        let items: Vec<_> = self.ast.items.clone();
        for item in items {
            let item_span = self.ast.items_arena[item].span;
            let item_no_check = has_attr(&self.ast.items_arena[item].attrs, NO_PROTOCOL_CHECK);
            let is_module = matches!(self.ast.items_arena[item].kind, ItemKind::Module(_));
            let ports = match &mut self.ast.items_arena[item].kind {
                ItemKind::Module(m) => std::mem::take(&mut m.ports),
                ItemKind::Extern(x) => std::mem::take(&mut x.ports),
                _ => continue,
            };
            let mut flat = Flat::default();
            let mut counter = 0u32;
            let mut out = Vec::with_capacity(ports.len());
            let mut handshakes: Vec<HandshakeInfo> = Vec::new();
            for port in ports {
                // Bundle dizisi (ADR-0056): eleman tipi bundle ise `N` kez
                // `<port>_<k>` ön ekiyle açılır.
                let (elem_ty, elems) = match array_type(&self.ast.types, port.ty) {
                    Some((elem, len)) if self.is_bundle_type(elem, &defs, builtin_handshake) => {
                        let Some(n) = self.bundle_array_len(&port, len, &consts) else {
                            continue;
                        };
                        flat.arrays.insert(port.name.text.clone(), n);
                        (elem, Some(n))
                    }
                    _ => (port.ty, None),
                };
                let Some(n) = elems else {
                    let prefix = port.name.text.clone();
                    self.expand_port(
                        port,
                        &prefix,
                        elem_ty,
                        &defs,
                        &plain,
                        builtin_handshake,
                        item_no_check,
                        is_module,
                        &mut out,
                        &mut flat,
                        &mut counter,
                        item_span,
                        &mut handshakes,
                    );
                    continue;
                };
                for k in 0..n {
                    let elem_port = Port {
                        span: port.span,
                        attrs: Vec::new(),
                        doc: None,
                        direction: port.direction,
                        name: Name {
                            text: format!("{}[{k}]", port.name.text),
                            span: port.name.span,
                        },
                        ty: elem_ty,
                        domain: port.domain.clone(),
                        bundle: None,
                    };
                    let prefix = format!("{}_{k}", port.name.text);
                    self.expand_port(
                        elem_port,
                        &prefix,
                        elem_ty,
                        &defs,
                        &plain,
                        builtin_handshake,
                        item_no_check || has_attr(&port.attrs, NO_PROTOCOL_CHECK),
                        is_module,
                        &mut out,
                        &mut flat,
                        &mut counter,
                        item_span,
                        &mut handshakes,
                    );
                }
            }
            if let Some((over, port_name)) = flat.budget.take() {
                self.err_flatten_budget(item, over, &port_name);
            }
            // Tanılar düzleştirilmiş adı (`hs_data`) değil kaynaktaki alan
            // yolunu (`hs.data`) gösterir — ADR-0072 açılım adı deseni
            // (ADR-0075). Anahtar düzleştirilmiş portun benzersiz ad span'i.
            for p in &out {
                if let Some(b) = &p.bundle {
                    self.ast
                        .generate
                        .source_names
                        .insert(p.name.span, format!("{}.{}", b.port.text, b.path));
                }
            }
            let (body, contracts) = match &mut self.ast.items_arena[item].kind {
                ItemKind::Module(m) => {
                    m.ports = out;
                    (
                        m.body.clone(),
                        m.contracts.iter().map(|c| c.expr).collect::<Vec<_>>(),
                    )
                }
                ItemKind::Extern(x) => {
                    x.ports = out;
                    continue;
                }
                _ => unreachable!(),
            };
            if flat.is_empty() {
                continue;
            }
            self.consts = consts.clone();
            for e in contracts {
                self.rw_bundle_expr(e, &flat);
            }
            for s in body {
                self.rw_bundle_stmt(s, &flat);
            }
            // Otomatik protokol kontratları düz isimlerle üretilir;
            // yeniden yazmaya girmez.
            let mut auto = Vec::new();
            for info in &handshakes {
                auto.extend(self.handshake_contracts(info, &mut counter, item_span));
            }
            if let ItemKind::Module(m) = &mut self.ast.items_arena[item].kind {
                m.contracts.extend(auto);
            }
        }
    }

    /// Tek portu (ya da bundle dizisinin bir elemanını) düz portlara
    /// açar; bundle değilse olduğu gibi geçirir.
    #[allow(clippy::too_many_arguments)]
    fn expand_port(
        &mut self,
        port: Port,
        prefix: &str,
        ty: Idx<TypeRef>,
        defs: &HashMap<String, Vec<FieldInfo>>,
        plain: &super::handshake::PlainDefs,
        builtin_handshake: bool,
        no_check: bool,
        is_module: bool,
        out: &mut Vec<Port>,
        flat: &mut Flat,
        counter: &mut u32,
        item_span: Span,
        handshakes: &mut Vec<HandshakeInfo>,
    ) {
        let bundle = simple_type_name(&self.ast.types, ty)
            .filter(|n| defs.contains_key(*n))
            .map(str::to_string);
        let payload = if builtin_handshake {
            self.handshake_payload(ty)
        } else {
            None
        };
        match (bundle, payload) {
            (Some(bundle), _) => {
                // `in` port yönleri tersler; `out`/`inout` bildirildiği gibi.
                let flipped = port.direction == PortDir::In;
                expand(
                    defs, &port, &bundle, prefix, "", flipped, 0, out, flat, counter, item_span,
                );
            }
            (None, Some(payload)) => {
                let payload = self.reject_bundle_payload(payload, defs);
                let no_check = no_check || has_attr(&port.attrs, NO_PROTOCOL_CHECK);
                let info = self
                    .expand_handshake(&port, prefix, payload, plain, out, flat, counter, item_span);
                // extern modülün kontratı yoktur: sentezlenmez.
                if !no_check && is_module {
                    handshakes.push(info);
                }
            }
            (None, None) => out.push(port),
        }
    }

    /// `Handshake<T>` payload'u yönlü bir bundle olamaz: iç içe
    /// `Handshake<Handshake<_>>` ya da `Handshake<BirStructPort>` (ADR-0070
    /// §3.1). Payload tek yönlü veridir (üretici sürer); içteki `ready`
    /// ters yönde akardı. E0003 + payload hata tipine çevrilir ki çözümleme
    /// açılmamış iç tipi "tanımsız ad" (E1001) diye raporlamasın.
    fn reject_bundle_payload(
        &mut self,
        payload: Idx<TypeRef>,
        defs: &HashMap<String, Vec<FieldInfo>>,
    ) -> Idx<TypeRef> {
        let inner = if self.handshake_payload(payload).is_some() {
            HANDSHAKE.to_string()
        } else if let Some(n) =
            simple_type_name(&self.ast.types, payload).filter(|n| defs.contains_key(*n))
        {
            n.to_string()
        } else {
            return payload;
        };
        let span = self.ast.types[payload].span;
        self.diagnostics.push(Diagnostic::error(
            ErrorCode::E0003,
            lstr!(
                en: "a Handshake payload cannot be a port bundle ('{inner}')";
                tr: "Handshake payload'u bir port bundle'ı olamaz ('{inner}')"
            ),
            LabeledSpan::primary(
                span,
                lstr!(en: "bundle used as payload"; tr: "payload olarak bundle"),
            ),
            lstr!(
                en: "use separate Handshake ports, or a plain struct (no 'port') as the payload";
                tr: "ayrı Handshake portları ya da payload olarak düz bir struct ('port' olmadan) kullanın"
            ),
        )
        .with_note(
            NoteKind::Note,
            lstr!(
                en: "the payload is data driven by the producer; a bundle has its own directions (e.g. 'ready' flows back), so it cannot be nested in 'data' (ADR-0050)";
                tr: "payload üreticinin sürdüğü veridir; bundle'ın kendi yönleri vardır (ör. 'ready' geri akar), 'data' içine yerleşemez (ADR-0050)"
            ),
        ));
        self.ast.types.alloc(TypeRef {
            span,
            kind: TypeRefKind::Error,
        })
    }

    /// Eleman tipi bir bundle (kullanıcı `struct port` ya da yerleşik
    /// `Handshake<T>`) mi?
    fn is_bundle_type(
        &self,
        ty: Idx<TypeRef>,
        defs: &HashMap<String, Vec<FieldInfo>>,
        builtin_handshake: bool,
    ) -> bool {
        simple_type_name(&self.ast.types, ty).is_some_and(|n| defs.contains_key(n))
            || (builtin_handshake && self.handshake_payload(ty).is_some())
    }

    /// `[Bundle; N]` uzunluğu — sabit değilse ya da 1..=256 dışındaysa
    /// E2008; port düşürülür (kaskad bastırma).
    fn bundle_array_len(
        &mut self,
        port: &Port,
        len: Idx<Expr>,
        consts: &HashMap<String, Idx<Expr>>,
    ) -> Option<i128> {
        let name = port.name.text.clone();
        match eval_const(&self.ast, consts, len, 0) {
            Some(n) if (1..=MAX_BUNDLE_ARRAY).contains(&n) => Some(n),
            Some(n) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E2008,
                        lstr!(
                            en: "bundle array port '{name}' has {n} elements; the limit is 1..={MAX_BUNDLE_ARRAY}";
                            tr: "'{name}' bundle dizisi portu {n} elemanlı; sınır 1..={MAX_BUNDLE_ARRAY}"
                        ),
                        LabeledSpan::primary(
                            port.span,
                            lstr!(en: "array length out of range"; tr: "dizi uzunluğu aralık dışı"),
                        ),
                        lstr!(
                            en: "each element becomes a set of flat ports; split the interface into modules";
                            tr: "her eleman bir düz port kümesi olur; arayüzü modüllere bölün"
                        ),
                    )
                    .with_note(NoteKind::Note, adr_note()),
                );
                None
            }
            None => {
                self.diagnostics.push(
                    Diagnostic::error(
                        ErrorCode::E2008,
                        lstr!(
                            en: "the length of bundle array port '{name}' must be a compile-time constant";
                            tr: "'{name}' bundle dizisi portunun uzunluğu derleme zamanı sabiti olmalı"
                        ),
                        LabeledSpan::primary(
                            self.ast.exprs[len].span,
                            lstr!(en: "not a constant"; tr: "sabit değil"),
                        ),
                        lstr!(
                            en: "use a literal, a const item or a const generic parameter: [Handshake<u8>; 4]";
                            tr: "literal, const öğe ya da const generic parametre kullanın: [Handshake<u8>; 4]"
                        ),
                    )
                    .with_note(NoteKind::Note, adr_note()),
                );
                None
            }
        }
    }

    /// E2008 — bundle dizisine sabit olmayan ya da aralık dışı indeks.
    fn err_bundle_index(&mut self, span: Span, port: &str, len: i128, index: Option<i128>) {
        let diag = match index {
            Some(k) => Diagnostic::error(
                ErrorCode::E2008,
                lstr!(
                    en: "index {k} is out of range for bundle array '{port}' ({len} elements)";
                    tr: "{k} indeksi '{port}' bundle dizisinin aralığı dışında ({len} eleman)"
                ),
                LabeledSpan::primary(span, lstr!(en: "out of range"; tr: "aralık dışı")),
                lstr!(
                    en: "valid indices are 0..{len}";
                    tr: "geçerli indeksler 0..{len}"
                ),
            ),
            None => Diagnostic::error(
                ErrorCode::E2008,
                lstr!(
                    en: "index into bundle array '{port}' must be a compile-time constant";
                    tr: "'{port}' bundle dizisinin indeksi derleme zamanı sabiti olmalı"
                ),
                LabeledSpan::primary(
                    span,
                    lstr!(en: "not a constant index"; tr: "sabit olmayan indeks"),
                ),
                lstr!(
                    en: "use a literal, a const, or a module-level 'for' variable (the loop is unrolled at compile time); a signal cannot select a bundle";
                    tr: "literal, const ya da modül seviyesi 'for' değişkeni kullanın (döngü derleme zamanında açılır); sinyal bundle seçemez"
                ),
            ),
        };
        self.diagnostics
            .push(diag.with_note(NoteKind::Reason, adr_note()));
    }

    /// Kullanıcı `Handshake` adlı bir `struct port` tanımlamış mı (generic
    /// olsa bile — o zaman düzleştirilmez ama yerleşik de devreye girmez)?
    fn user_defines_handshake(&self) -> bool {
        self.ast.items.iter().any(|&i| {
            matches!(&self.ast.items_arena[i].kind,
                ItemKind::Struct(s) if s.is_port && s.name.text == HANDSHAKE)
        })
    }

    /// Herhangi bir modül/extern portu `Handshake<T>` tipli mi?
    fn uses_builtin_handshake(&self) -> bool {
        self.ast.items.iter().any(|&i| {
            let ports = match &self.ast.items_arena[i].kind {
                ItemKind::Module(m) => &m.ports,
                ItemKind::Extern(x) => &x.ports,
                _ => return false,
            };
            ports.iter().any(|p| {
                // Bundle dizisi (ADR-0056): eleman tipine bakılır.
                let ty = array_type(&self.ast.types, p.ty).map_or(p.ty, |(elem, _)| elem);
                self.handshake_payload(ty).is_some()
            })
        })
    }

    /// Dosyadaki generic olmayan `struct port` bildirimleri. Bir döngüye
    /// ulaşan tanımlar (`recursive_types`, E4009 — ADR-0069 tip çizgesi)
    /// dışarıda bırakılır: açılım sonludur ve modül portu olduğu gibi kalır.
    fn collect_bundle_defs(&self) -> HashMap<String, Vec<FieldInfo>> {
        let port_structs: Vec<&volt_ast::StructDecl> = self
            .ast
            .items
            .iter()
            .filter_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Struct(s) if s.is_port && s.generics.is_empty() => Some(s),
                _ => None,
            })
            .collect();
        let mut defs: HashMap<String, Vec<FieldInfo>> = HashMap::new();
        for s in &port_structs {
            let fields: Vec<FieldInfo> = s
                .fields
                .iter()
                .map(|f| FieldInfo {
                    // Yönsüz alan parser hatası almıştır; kurtarma: out.
                    dir: f.direction.unwrap_or(PortDir::Out),
                    name: f.name.text.clone(),
                    ty: f.ty,
                    domain: f.domain.clone(),
                    nested: simple_type_name(&self.ast.types, f.ty)
                        .filter(|n| port_structs.iter().any(|p| p.name.text == *n))
                        .map(str::to_string),
                })
                .collect();
            defs.insert(s.name.text.clone(), fields);
        }
        defs.retain(|name, _| !self.recursive_types.contains(name));
        defs
    }

    /// Modül / extern adı (E4010 konumu).
    fn item_name(&self, item: Idx<Item>) -> Option<Name> {
        match &self.ast.items_arena[item].kind {
            ItemKind::Module(m) => Some(m.name.clone()),
            ItemKind::Extern(x) => Some(x.name.clone()),
            _ => None,
        }
    }

    /// E4010 — düzleştirme bütçesi (port sayısı ya da derinlik) aşıldı (ADR-0067).
    fn err_flatten_budget(&mut self, item: Idx<Item>, over: Overflow, port: &Name) {
        let Some(owner) = self.item_name(item) else {
            return;
        };
        let (span, message, label) = match over {
            Overflow::Ports => (
                owner.span,
                lstr!(
                    en: "flattening the bundle ports of '{}' would produce more than {MAX_FLAT_PORTS} flat ports", owner.text;
                    tr: "'{}' modülünün bundle portlarını açmak {MAX_FLAT_PORTS}'dan çok düz port üretirdi", owner.text
                ),
                lstr!(en: "too many flat ports"; tr: "çok fazla düz port"),
            ),
            Overflow::Depth => (
                port.span,
                lstr!(
                    en: "bundle port '{}' nests deeper than {MAX_NESTING} levels", port.text;
                    tr: "'{}' bundle portu {MAX_NESTING} seviyeden derin iç içe", port.text
                ),
                lstr!(en: "nested too deep"; tr: "çok derin iç içe"),
            ),
        };
        self.diagnostics.push(
            Diagnostic::error(
                ErrorCode::E4010,
                message,
                LabeledSpan::primary(span, label),
                lstr!(
                    en: "shrink the bundle array, flatten the type hierarchy or split the interface into several modules";
                    tr: "bundle dizisini küçültün, tip hiyerarşisini sadeleştirin ya da arayüzü birkaç modüle bölün"
                ),
            )
            .with_note(NoteKind::Note, flatten_note()),
        );
    }

    fn rw_bundle_stmt(&mut self, si: Idx<Stmt>, flat: &Flat) {
        let mut exprs: Vec<Idx<Expr>> = Vec::new();
        let mut blocks: Vec<Idx<Block>> = Vec::new();
        match &mut self.ast.stmts[si].kind {
            StmtKind::Reg(r) => exprs.push(r.init),
            StmtKind::Let(l) => exprs.push(l.value),
            StmtKind::Assign(a) => {
                exprs.push(a.rhs);
                exprs.extend(a.lhs.suffixes.iter().flat_map(lvalue_suffix_exprs));
            }
            StmtKind::On(on) => blocks.push(on.body),
            StmtKind::Comb(b) => blocks.push(*b),
            StmtKind::For(f) => {
                exprs.extend([f.start, f.end]);
                blocks.push(f.body);
            }
            StmtKind::Expr(e) => exprs.push(*e),
            StmtKind::Instance(inst) => {
                exprs.extend(inst.bindings.iter().filter_map(|b| b.value));
            }
            StmtKind::Wire(_) | StmtKind::Error => {}
        }
        if let StmtKind::Assign(a) = &mut self.ast.stmts[si].kind {
            let (span, base) = (a.lhs.span, a.lhs.base.clone());
            let mut lhs = std::mem::replace(
                &mut a.lhs,
                LValue {
                    span,
                    base,
                    suffixes: Vec::new(),
                },
            );
            self.rw_lvalue(&mut lhs, flat);
            if let StmtKind::Assign(a) = &mut self.ast.stmts[si].kind {
                a.lhs = lhs;
            }
        }
        for e in exprs {
            self.rw_bundle_expr(e, flat);
        }
        for b in blocks {
            self.rw_bundle_block(b, flat);
        }
    }

    fn rw_bundle_block(&mut self, bi: Idx<Block>, flat: &Flat) {
        let mut stmts = std::mem::take(&mut self.ast.blocks[bi].stmts);
        for bs in &mut stmts {
            self.rw_bundle_block_stmt(bs, flat);
        }
        self.ast.blocks[bi].stmts = stmts;
        if let Some(tail) = self.ast.blocks[bi].tail {
            self.rw_bundle_expr(tail, flat);
        }
    }

    fn rw_bundle_block_stmt(&mut self, bs: &mut BlockStmt, flat: &Flat) {
        match bs {
            BlockStmt::NonBlockAssign { lhs, rhs, .. }
            | BlockStmt::BlockAssign { lhs, rhs, .. } => {
                self.rw_lvalue(lhs, flat);
                self.rw_bundle_expr(*rhs, flat);
                let idxs: Vec<Idx<Expr>> =
                    lhs.suffixes.iter().flat_map(lvalue_suffix_exprs).collect();
                for e in idxs {
                    self.rw_bundle_expr(e, flat);
                }
            }
            BlockStmt::If(ifstmt) => self.rw_bundle_if(ifstmt, flat),
            BlockStmt::Match(m) => {
                self.rw_bundle_expr(m.scrutinee, flat);
                let (mut exprs, mut blocks) = (Vec::new(), Vec::new());
                collect_arm_idxs(&m.arms, &mut exprs, &mut blocks);
                for e in exprs {
                    self.rw_bundle_expr(e, flat);
                }
                for b in blocks {
                    self.rw_bundle_block(b, flat);
                }
            }
            BlockStmt::Let(l) => self.rw_bundle_expr(l.value, flat),
            BlockStmt::For(f) => {
                self.rw_bundle_expr(f.start, flat);
                self.rw_bundle_expr(f.end, flat);
                self.rw_bundle_block(f.body, flat);
            }
            BlockStmt::Error => {}
        }
    }

    fn rw_bundle_if(&mut self, ifstmt: &IfStmt, flat: &Flat) {
        self.rw_bundle_expr(ifstmt.cond, flat);
        self.rw_bundle_block(ifstmt.then_block, flat);
        match &ifstmt.else_branch {
            Some(ElseBranch::Block(b)) => self.rw_bundle_block(*b, flat),
            Some(ElseBranch::If(inner)) => self.rw_bundle_if(inner, flat),
            None => {}
        }
    }

    /// `aw.addr` (ve iç içe `aw.sub.addr`) alan zincirini düz porta
    /// yeniden yazar; eşleşmeyen düğümlerde çocuklara iner.
    fn rw_bundle_expr(&mut self, e: Idx<Expr>, flat: &Flat) {
        if let Some(key) = self.field_chain(e, flat) {
            if let Some(name) = flat.lookup(&key) {
                let span = self.ast.exprs[e].span;
                self.ast.exprs[e].kind = ExprKind::Path(Path {
                    span,
                    segments: vec![Name {
                        text: name.to_string(),
                        span,
                    }],
                });
                return;
            }
            if let Some(v) = flat.virtuals.get(&key) {
                self.rw_virtual(e, v);
                return;
            }
        }
        let (exprs, blocks) = expr_children(&self.ast.exprs[e].kind);
        for c in exprs {
            self.rw_bundle_expr(c, flat);
        }
        for b in blocks {
            self.rw_bundle_block(b, flat);
        }
    }

    /// `tx.fired` → `tx_valid && tx_ready`, `tx.stalled` → `tx_valid &&
    /// !tx_ready` (ADR-0050). İki isim farklı span alır (`use_spans`
    /// anahtarı): ifadenin ilk ve son karakteri.
    fn rw_virtual(&mut self, e: Idx<Expr>, v: &Virtual) {
        let span = self.ast.exprs[e].span;
        let first =
            Span::new(span.file, span.start, (span.start + 1).min(span.end)).with_ctx(span.ctx);
        let last = Span::new(
            span.file,
            span.end.saturating_sub(1).max(span.start),
            span.end,
        )
        .with_ctx(span.ctx);
        let path = |me: &mut Self, text: &str, nspan: Span| {
            me.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Path(Path {
                    span,
                    segments: vec![Name {
                        text: text.to_string(),
                        span: nspan,
                    }],
                }),
            })
        };
        let valid = path(self, &v.valid, first);
        let ready = path(self, &v.ready, last);
        let rhs = if v.negate_ready {
            self.ast.exprs.alloc(Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnOp::Not,
                    operand: ready,
                },
            })
        } else {
            ready
        };
        self.ast.exprs[e].kind = ExprKind::Binary {
            op: BinOp::And,
            lhs: valid,
            rhs,
        };
    }

    /// `a.b.c` biçimindeki zinciri `"a.b.c"` anahtarına çevirir; taban
    /// tek segmentli bir yol değilse None. Bundle dizisinde taban
    /// `ch[k]`tır (`"ch[0].valid"`); indeks sabit değilse E2008 üretir
    /// ve None döner.
    fn field_chain(&mut self, e: Idx<Expr>, flat: &Flat) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        let mut cur = e;
        loop {
            match &self.ast.exprs[cur].kind {
                ExprKind::Field { base, field } => {
                    parts.push(field.text.clone());
                    cur = *base;
                }
                ExprKind::Path(p) if p.segments.len() == 1 && !parts.is_empty() => {
                    parts.push(p.segments[0].text.clone());
                    parts.reverse();
                    return Some(parts.join("."));
                }
                ExprKind::Index { base, index } if !parts.is_empty() => {
                    let (base, index) = (*base, *index);
                    let ExprKind::Path(p) = &self.ast.exprs[base].kind else {
                        return None;
                    };
                    if p.segments.len() != 1 {
                        return None;
                    }
                    let port = p.segments[0].text.clone();
                    let &len = flat.arrays.get(&port)?;
                    let k = self.bundle_index(&port, len, index)?;
                    parts.push(format!("{port}[{k}]"));
                    parts.reverse();
                    return Some(parts.join("."));
                }
                _ => return None,
            }
        }
    }

    /// Bundle dizisi indeksi: sabit ve aralık içi ise değeri, değilse
    /// E2008 + None.
    fn bundle_index(&mut self, port: &str, len: i128, index: Idx<Expr>) -> Option<i128> {
        let span = self.ast.exprs[index].span;
        match eval_const(&self.ast, &self.consts, index, 0) {
            Some(k) if (0..len).contains(&k) => Some(k),
            other => {
                self.err_bundle_index(span, port, len, other);
                None
            }
        }
    }

    /// Sol taraf: `aw.ready = x` → `aw_ready = x`, `ch[0].ready = x` →
    /// `ch_0_ready = x` (öndeki indeks/alan sonekleri düşer, kalan
    /// indeks/aralık sonekleri kalır).
    fn rw_lvalue(&mut self, lv: &mut LValue, flat: &Flat) {
        let mut key = lv.base.text.clone();
        let mut best: Option<(usize, String)> = None;
        let mut start = 0;
        if let Some(&len) = flat.arrays.get(&lv.base.text) {
            let Some(LValueSuffix::Index(i)) = lv.suffixes.first() else {
                return;
            };
            let Some(k) = self.bundle_index(&lv.base.text.clone(), len, *i) else {
                return;
            };
            key = format!("{key}[{k}]");
            start = 1;
        }
        for (i, s) in lv.suffixes.iter().enumerate().skip(start) {
            let LValueSuffix::Field(f) = s else { break };
            key.push('.');
            key.push_str(&f.text);
            if let Some(name) = flat.lookup(&key) {
                best = Some((i + 1, name.to_string()));
            }
        }
        if let Some((n, name)) = best {
            lv.base.text = name;
            lv.suffixes.drain(..n);
        }
    }
}

fn adr_note() -> String {
    lstr!(
        en: "a bundle array is flattened per element at compile time (ADR-0056)";
        tr: "bundle dizisi derleme zamanında eleman eleman düzleşir (ADR-0056)"
    )
}

/// E4009/E4010 notu: düzleştirme derleme zamanında ve sonludur (ADR-0067).
pub(super) fn flatten_note() -> String {
    lstr!(
        en: "a port group is expanded field by field at compile time (ADR-0039); the expansion must be finite and bounded (ADR-0067)";
        tr: "port grubu derleme zamanında alan alan açılır (ADR-0039); açılım sonlu ve sınırlı olmalıdır (ADR-0067)"
    )
}

/// Bir bundle portunu düz portlara açar (iç içe bundle'larda özyineli).
#[allow(clippy::too_many_arguments)]
fn expand(
    defs: &HashMap<String, Vec<FieldInfo>>,
    port: &Port,
    bundle: &str,
    prefix: &str,
    path: &str,
    flipped: bool,
    depth: usize,
    out: &mut Vec<Port>,
    flat: &mut Flat,
    counter: &mut u32,
    item_span: Span,
) {
    if flat.budget.is_some() {
        return;
    }
    if depth > MAX_NESTING {
        flat.budget = Some((Overflow::Depth, port.name.clone()));
        return;
    }
    let Some(fields) = defs.get(bundle) else {
        return;
    };
    for f in fields {
        if flat.budget.is_some() {
            return;
        }
        let flat_name = format!("{prefix}_{}", f.name);
        let field_path = if path.is_empty() {
            f.name.clone()
        } else {
            format!("{path}.{}", f.name)
        };
        if let Some(inner) = &f.nested {
            expand(
                defs,
                port,
                inner,
                &flat_name,
                &field_path,
                flipped ^ (f.dir == PortDir::In),
                depth + 1,
                out,
                flat,
                counter,
                item_span,
            );
            continue;
        }
        if out.len() >= MAX_FLAT_PORTS {
            flat.budget = Some((Overflow::Ports, port.name.clone()));
            return;
        }
        // Bildirim span'i benzersiz olmalı (decl_spans anahtarı).
        let name_span = fresh_name_span(port.span, item_span, counter);
        flat.names.insert(
            format!("{}.{field_path}", port.name.text),
            flat_name.clone(),
        );
        out.push(Port {
            span: port.span,
            attrs: Vec::new(),
            doc: None,
            direction: if flipped { flip(f.dir) } else { f.dir },
            name: Name {
                text: flat_name,
                span: name_span,
            },
            ty: f.ty,
            // Anotasyon span'i de düzleştirilmiş port başına benzersiz
            // (use_spans anahtarı): aynı struct'ı iki extern kullanınca
            // sembolik alan tanımları birbirini ezmesin (ADR-0047).
            domain: f
                .domain
                .clone()
                .or_else(|| port.domain.clone())
                .map(|d| Name {
                    text: d.text,
                    span: name_span,
                }),
            bundle: Some(BundleOrigin {
                port: port.name.clone(),
                bundle: bundle.to_string(),
                path: field_path,
                declared: f.dir,
                flipped,
            }),
        });
    }
}
