//! Yerleşik `Handshake<T>` bundle'ı (ADR-0050): tek saat alanında
//! valid/ready el sıkışması.
//!
//! ```volt
//! struct port Handshake<T> {      // yerleşik; kullanıcı yazmaz
//!     out data  : T
//!     out valid : bool
//!     in  ready : bool
//! }
//! out tx : Handshake<u8>          // üretici: data/valid çıkış, ready giriş
//! in  rx : Handshake<u8>          // tüketici: yönler terslenir
//! ```
//!
//! Düzleştirme ADR-0039 kuralıyla aynıdır (`tx_data`, `tx_valid`,
//! `tx_ready`); payload sade bir `struct` ise alanları `tx_data_<alan>`
//! olarak açılır (SV'de struct tipli port yoktur). İki sanal alan
//! ifade olarak yeniden yazılır: `tx.fired` → `tx_valid && tx_ready`,
//! `tx.stalled` → `tx_valid && !tx_ready`.
//!
//! Her Handshake portu için protokol kontratları OTOMATİK üretilir
//! (`@no_protocol_check` ile port ya da modül düzeyinde kapatılır):
//!
//! * `prev(valid) && !prev(ready) -> valid` — valid, ready gelene dek düşmez;
//! * `prev(valid) && !prev(ready) -> data == prev(data)` — veri sabit kalır
//!   (düz veri alanı başına bir kontrat; dizi/tuple payload atlanır).
//!
//! Üretici (`out`) tarafta bunlar `invariant`, tüketici (`in`) tarafta
//! `assume`dır: bir modül kendi girişini kanıtlayamaz, ortamdan bekler.
//! Kullanıcı aynı adla `struct port Handshake` tanımlarsa kullanıcı
//! tanımı kazanır ve yerleşik devre dışı kalır.

use std::collections::HashMap;

use volt_ast::{
    Attribute, BinOp, BundleOrigin, Contract, ContractKind, Expr, ExprKind, GenericArg, Idx,
    ItemKind, Name, Path, Port, PortDir, TypeRef, TypeRefKind, UnOp,
};
use volt_span::Span;

use super::bundle::{flip, fresh_name_span, Flat, Virtual, MAX_NESTING};
use super::Parser;

/// Yerleşik bundle'ın adı.
pub(super) const HANDSHAKE: &str = "Handshake";
/// Otomatik protokol kontratlarını kapatan nitelik.
pub(super) const NO_PROTOCOL_CHECK: &str = "no_protocol_check";

const FIELD_DATA: &str = "data";
const FIELD_VALID: &str = "valid";
const FIELD_READY: &str = "ready";
const FIELD_FIRED: &str = "fired";
const FIELD_STALLED: &str = "stalled";
const PREV: &str = "prev";

/// Sade (yönsüz, generic olmayan) struct tanımları: payload açılımı için.
pub(super) type PlainDefs = HashMap<String, Vec<(String, Idx<TypeRef>)>>;

/// Düzleştirilmiş bir Handshake portu — kontrat üretimi girdisi.
pub(super) struct HandshakeInfo {
    /// Port bildirimi (kontrat span'i; E5001 buraya işaret eder).
    pub span: Span,
    pub valid: String,
    pub ready: String,
    /// Kararlılık kontratı üretilecek düz veri adları.
    pub data: Vec<String>,
    /// `valid`i bu modül sürüyor (port `out`/`inout`).
    pub producer: bool,
}

pub(super) fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.name.text == name)
}

/// Tek segmentli, argümansız tip yolu adı (`Aw`, `Mode`).
fn plain_path_name(types: &volt_ast::Arena<TypeRef>, ty: Idx<TypeRef>) -> Option<&str> {
    match &types[ty].kind {
        TypeRefKind::Path { path, args } if args.is_empty() && path.segments.len() == 1 => {
            Some(path.segments[0].text.as_str())
        }
        _ => None,
    }
}

/// Kararlılık kontratı (`==` + `prev`) üretilebilecek tip mi? Sade
/// struct'lar alan alan açıldığından burada görünmez; görünürse (derinlik
/// sınırı) kontrat üretilmez.
fn is_comparable(types: &volt_ast::Arena<TypeRef>, plain: &PlainDefs, ty: Idx<TypeRef>) -> bool {
    match &types[ty].kind {
        TypeRefKind::Bool
        | TypeRefKind::UInt(_)
        | TypeRefKind::SInt(_)
        | TypeRefKind::Bits(_)
        | TypeRefKind::UIntN(_)
        | TypeRefKind::SIntN(_) => true,
        // enum / alias: eşitlik tanımlı; struct değil.
        TypeRefKind::Path { .. } => {
            plain_path_name(types, ty).is_some_and(|n| !plain.contains_key(n))
        }
        _ => false,
    }
}

/// Payload'ı düz (yol, tip) çiftlerine açar: sade struct alanları
/// özyineli (`data.i.x`), derinlik `MAX_NESTING` ile sınırlı.
fn payload_fields(
    types: &volt_ast::Arena<TypeRef>,
    plain: &PlainDefs,
    ty: Idx<TypeRef>,
    path: &str,
    depth: usize,
    out: &mut Vec<(String, Idx<TypeRef>)>,
) {
    if depth < MAX_NESTING {
        if let Some(fields) = plain_path_name(types, ty).and_then(|n| plain.get(n)) {
            for (name, fty) in fields {
                payload_fields(
                    types,
                    plain,
                    *fty,
                    &format!("{path}.{name}"),
                    depth + 1,
                    out,
                );
            }
            return;
        }
    }
    out.push((path.to_string(), ty));
}

impl Parser<'_> {
    /// Dosyadaki sade struct'lar (`struct Aw { addr : u32, prot : u3 }`).
    pub(super) fn collect_plain_struct_defs(&self) -> PlainDefs {
        self.ast
            .items
            .iter()
            .filter_map(|&i| match &self.ast.items_arena[i].kind {
                ItemKind::Struct(s) if !s.is_port && s.generics.is_empty() => Some(s),
                _ => None,
            })
            .map(|s| {
                let fields = s
                    .fields
                    .iter()
                    .map(|f| (f.name.text.clone(), f.ty))
                    .collect();
                (s.name.text.clone(), fields)
            })
            .collect()
    }

    /// Port tipi `Handshake<T>` (tam bir tip argümanı) ise `T`.
    pub(super) fn handshake_payload(&self, ty: Idx<TypeRef>) -> Option<Idx<TypeRef>> {
        match &self.ast.types[ty].kind {
            TypeRefKind::Path { path, args }
                if path.segments.len() == 1
                    && path.segments[0].text == HANDSHAKE
                    && args.len() == 1 =>
            {
                match args[0] {
                    GenericArg::Type(t) => Some(t),
                    GenericArg::Const(_) => None,
                }
            }
            _ => None,
        }
    }

    /// Bir `Handshake<T>` portunu `data`/`valid`/`ready` düz portlarına
    /// açar, alan zinciri ve sanal alan (`fired`/`stalled`) haritalarını
    /// doldurur.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn expand_handshake(
        &mut self,
        port: &Port,
        payload: Idx<TypeRef>,
        plain: &PlainDefs,
        out: &mut Vec<Port>,
        flat: &mut Flat,
        counter: &mut u32,
        item_span: Span,
    ) -> HandshakeInfo {
        let flipped = port.direction == PortDir::In;
        let prefix = port.name.text.clone();
        let bool_ty = self.ast.types.alloc(TypeRef {
            span: port.span,
            kind: TypeRefKind::Bool,
        });

        // Payload: sade struct ise alan alan (özyineli), değilse tek `data`.
        let mut payload_paths = Vec::new();
        payload_fields(
            &self.ast.types,
            plain,
            payload,
            FIELD_DATA,
            0,
            &mut payload_paths,
        );
        let mut fields: Vec<(String, Idx<TypeRef>, PortDir)> = payload_paths
            .into_iter()
            .map(|(p, t)| (p, t, PortDir::Out))
            .collect();
        fields.push((FIELD_VALID.to_string(), bool_ty, PortDir::Out));
        fields.push((FIELD_READY.to_string(), bool_ty, PortDir::In));

        let mut data = Vec::new();
        for (path, ty, declared) in fields {
            let flat_name = format!("{prefix}_{}", path.replace('.', "_"));
            if path != FIELD_VALID
                && path != FIELD_READY
                && is_comparable(&self.ast.types, plain, ty)
            {
                data.push(flat_name.clone());
            }
            let name_span = fresh_name_span(port.span, item_span, counter);
            flat.names
                .insert(format!("{prefix}.{path}"), flat_name.clone());
            out.push(Port {
                span: port.span,
                attrs: Vec::new(),
                doc: None,
                direction: if flipped { flip(declared) } else { declared },
                name: Name {
                    text: flat_name,
                    span: name_span,
                },
                ty,
                domain: port.domain.clone().map(|d| Name {
                    text: d.text,
                    span: name_span,
                }),
                bundle: Some(BundleOrigin {
                    port: port.name.clone(),
                    bundle: HANDSHAKE.to_string(),
                    path,
                    declared,
                    flipped,
                }),
            });
        }

        let valid = format!("{prefix}_{FIELD_VALID}");
        let ready = format!("{prefix}_{FIELD_READY}");
        for (field, negate_ready) in [(FIELD_FIRED, false), (FIELD_STALLED, true)] {
            flat.virtuals.insert(
                format!("{prefix}.{field}"),
                Virtual {
                    valid: valid.clone(),
                    ready: ready.clone(),
                    negate_ready,
                },
            );
        }
        HandshakeInfo {
            span: port.span,
            valid,
            ready,
            data,
            producer: !flipped,
        }
    }

    /// Otomatik protokol kontratları (modül başlığı görünümü):
    /// `invariant: prev(v) && !prev(r) -> v` ve her veri alanı için
    /// `invariant: prev(v) && !prev(r) -> d == prev(d)`. Tüketici
    /// tarafta `assume`.
    pub(super) fn handshake_contracts(
        &mut self,
        info: &HandshakeInfo,
        counter: &mut u32,
        item_span: Span,
    ) -> Vec<Contract> {
        let kind = if info.producer {
            ContractKind::Invariant
        } else {
            ContractKind::Assume
        };
        let span = info.span;
        let mut b = ExprBuilder {
            p: self,
            span,
            item_span,
            counter,
        };
        let mut contracts = Vec::new();
        // Tutma: valid, ready gelene dek düşmez.
        let pending = b.pending(&info.valid, &info.ready);
        let v = b.path(&info.valid);
        let hold = b.bin(BinOp::Imp, pending, v);
        contracts.push(Contract {
            span,
            kind,
            expr: hold,
        });
        // Kararlılık: veri, el sıkışma tamamlanana dek sabit.
        for d in &info.data {
            let pending = b.pending(&info.valid, &info.ready);
            let cur = b.path(d);
            let past = b.prev(d);
            let eq = b.bin(BinOp::Eq, cur, past);
            let stable = b.bin(BinOp::Imp, pending, eq);
            contracts.push(Contract {
                span,
                kind,
                expr: stable,
            });
        }
        contracts
    }
}

/// Kontrat ifadesi kurucu: her isim benzersiz span alır (`use_spans`
/// anahtarı), ifade span'i port bildirimidir.
struct ExprBuilder<'a, 'p> {
    p: &'a mut Parser<'p>,
    span: Span,
    item_span: Span,
    counter: &'a mut u32,
}

impl ExprBuilder<'_, '_> {
    fn path(&mut self, name: &str) -> Idx<Expr> {
        let nspan = fresh_name_span(self.span, self.item_span, self.counter);
        self.p.ast.exprs.alloc(Expr {
            span: self.span,
            kind: ExprKind::Path(Path {
                span: self.span,
                segments: vec![Name {
                    text: name.to_string(),
                    span: nspan,
                }],
            }),
        })
    }

    fn prev(&mut self, name: &str) -> Idx<Expr> {
        let callee = self.path(PREV);
        let arg = self.path(name);
        self.p.ast.exprs.alloc(Expr {
            span: self.span,
            kind: ExprKind::Call {
                callee,
                args: vec![arg],
            },
        })
    }

    fn not(&mut self, operand: Idx<Expr>) -> Idx<Expr> {
        self.p.ast.exprs.alloc(Expr {
            span: self.span,
            kind: ExprKind::Unary {
                op: UnOp::Not,
                operand,
            },
        })
    }

    fn bin(&mut self, op: BinOp, lhs: Idx<Expr>, rhs: Idx<Expr>) -> Idx<Expr> {
        self.p.ast.exprs.alloc(Expr {
            span: self.span,
            kind: ExprKind::Binary { op, lhs, rhs },
        })
    }

    /// `prev(valid) && !prev(ready)` — el sıkışma bekliyor.
    fn pending(&mut self, valid: &str, ready: &str) -> Idx<Expr> {
        let pv = self.prev(valid);
        let pr = self.prev(ready);
        let npr = self.not(pr);
        self.bin(BinOp::And, pv, npr)
    }
}
