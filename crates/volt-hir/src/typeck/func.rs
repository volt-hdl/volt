//! Fonksiyon tip denetimi (ADR-0081): imza, gövde ve çağrı.
//!
//! fn saf kombinasyonel bir ifadedir (Karar 1). Gövde imzaya karşı bir kez,
//! tanımda denetlenir (Karar 2): `let`'ler modül `let`'iyle aynı kurallar,
//! son ifade dönüş tipine `check` edilir. Dönüş tipi ya da son ifade yoksa
//! E2015. Çağrı (Karar 5): arity E2003, her argüman parametre tipine
//! `check`, çağrının tipi dönüş tipi. Tanının yeri ilkesi: fn'in kendisiyle
//! ilgili hata tanımda, kullanımla ilgili hata çağrı yerinde.

use volt_ast::{BlockStmt, Expr, ExprKind, FnDecl, Idx, ItemKind};
use volt_diagnostics::{lstr, ErrorCode};
use volt_span::Span;

use super::TypeChecker;
use crate::resolve::DefId;
use crate::ty::{StructId, Ty, TypeId};

/// Bir fn'in tiplenmiş imzası; çağrılar tanım sırasından bağımsız
/// olarak buna bakar.
#[derive(Debug, Clone)]
pub(super) struct FnSig {
    pub(super) params: Vec<TypeId>,
    /// `None`: dönüş tipi yazılmamış (E2015 tanımda).
    pub(super) ret: Option<TypeId>,
}

impl TypeChecker<'_, '_> {
    /// fn tanımını denetler: imza tipleri, gövde `let`'leri, son ifade.
    pub(super) fn check_fn(&mut self, f: &FnDecl) {
        let sig = self.fn_sig_of_decl(f);
        for (p, &ty) in f.params.iter().zip(&sig.params) {
            self.record_def_type(&p.name, ty);
        }
        self.check_fn_signature(f, &sig);
        let ast = self.ast;
        let block = &ast.blocks[f.body];
        let prev = std::mem::replace(&mut self.in_fn, true);
        // Parser gövdede yalnız let/for/son ifade bırakır; atamalar ve
        // diğer deyimler E0001/E2016 ile Error olur, `for` E0003
        // (`crate::functions`).
        for stmt in &block.stmts {
            if let BlockStmt::Let(l) = stmt {
                self.handle_let(l);
            }
        }
        match (sig.ret, block.tail) {
            (Some(ret), Some(tail)) => self.check(tail, ret),
            (ret, tail) => {
                if let Some(tail) = tail {
                    self.synth(tail);
                }
                self.err_fn_no_result(f, ret.is_none(), block.span);
            }
        }
        self.in_fn = prev;
    }

    /// Karar 4: imzada saat/sıfırlama E2016 (fn'i bir saat alanına
    /// bağlar); `Delayed`, `struct port` ve dizi dönüşü bu turda E0003.
    fn check_fn_signature(&mut self, f: &FnDecl, sig: &FnSig) {
        let ast = self.ast;
        let params = f
            .params
            .iter()
            .map(|p| p.ty)
            .zip(sig.params.iter().copied());
        let ret = f.return_ty.zip(sig.ret);
        for (i, (ty_ref, ty)) in params.chain(ret).enumerate() {
            let is_ret = i == f.params.len();
            let span = ast.types[ty_ref].span;
            if ast.timing.delayed_types.contains_key(&ty_ref) {
                self.err_fn_sig_unsupported(
                    span,
                    &lstr!(en: "'Delayed<T, N>' in a function signature"; tr: "fonksiyon imzasında 'Delayed<T, N>'"),
                    &lstr!(en: "take the plain value type; the call's latency is inferred from its arguments at the call site (ADR-0081)";
                           tr: "düz değer tipini alın; çağrının gecikmesi çağrı yerinde argümanlardan çıkarılır (ADR-0081)"),
                );
                continue;
            }
            match *self.types.ty(ty) {
                Ty::Clock | Ty::Reset { .. } => {
                    let what = if matches!(self.types.ty(ty), Ty::Clock) {
                        lstr!(en: "a clock"; tr: "saat")
                    } else {
                        lstr!(en: "a reset"; tr: "sıfırlama")
                    };
                    self.error(
                        ErrorCode::E2016,
                        span,
                        lstr!(en: "a function must be combinational: {what} cannot appear in its signature";
                              tr: "fonksiyon kombinasyonel olmalı: imzasında {what} bulunamaz"),
                        lstr!(en: "ties the function to a clock domain"; tr: "fonksiyonu bir saat alanına bağlar"),
                        lstr!(en: "remove it: a function computes a value from its arguments; if you need a clock, write a module";
                              tr: "kaldırın: fonksiyon değerini argümanlarından hesaplar; saat gerekiyorsa modül yazın"),
                    );
                }
                Ty::Struct(StructId(id)) if self.is_struct_port(DefId(id)) => {
                    self.err_fn_sig_unsupported(
                        span,
                        &lstr!(en: "a 'struct port' bundle in a function signature"; tr: "fonksiyon imzasında 'struct port' bundle'ı"),
                        &lstr!(en: "a bundle groups ports and is not a value; pass its fields, or a plain struct (ADR-0077)";
                               tr: "bundle portları gruplar, değer değildir; alanlarını ya da düz bir struct geçirin (ADR-0077)"),
                    );
                }
                Ty::Array { .. } if is_ret => {
                    self.err_fn_sig_unsupported(
                        span,
                        &lstr!(en: "an array as a function's return type"; tr: "fonksiyon dönüş tipi olarak dizi"),
                        &lstr!(en: "return a plain struct, or one element per function (ADR-0081)";
                               tr: "düz bir struct ya da fonksiyon başına bir eleman döndürün (ADR-0081)"),
                    );
                }
                _ => {}
            }
        }
    }

    fn is_struct_port(&self, def: DefId) -> bool {
        let ast = self.ast;
        self.res.item_of_def.get(&def).is_some_and(
            |&item| matches!(&ast.items_arena[item].kind, ItemKind::Struct(s) if s.is_port),
        )
    }

    /// E0003 — imzada bu turda eşlemesi olmayan tip.
    fn err_fn_sig_unsupported(&mut self, span: Span, what: &str, help: &str) {
        self.error(
            ErrorCode::E0003,
            span,
            lstr!(en: "not supported yet: {what}"; tr: "henüz desteklenmiyor: {what}"),
            lstr!(en: "no hardware mapping yet"; tr: "henüz donanım eşlemesi yok"),
            help.to_string(),
        );
    }

    /// E2015 — dönüş tipi yok (ad üzerinde) ya da son ifade yok (kapanış
    /// `}` üzerinde).
    fn err_fn_no_result(&mut self, f: &FnDecl, missing_ret: bool, body: Span) {
        let name = &f.name.text;
        if missing_ret {
            self.error(
                ErrorCode::E2015,
                f.name.span,
                lstr!(en: "function '{name}' has no return type"; tr: "'{name}' fonksiyonunun dönüş tipi yok"),
                lstr!(en: "a function must compute a value"; tr: "fonksiyon bir değer hesaplamalı"),
                lstr!(en: "write the result type after the parameters: fn {name}(...) -> u8 {{ ... }}";
                      tr: "sonuç tipini parametrelerin ardına yazın: fn {name}(...) -> u8 {{ ... }}"),
            );
        } else {
            let close =
                Span::new(body.file, body.end.saturating_sub(1), body.end).with_ctx(body.ctx);
            self.error(
                ErrorCode::E2015,
                close,
                lstr!(en: "function '{name}' does not end with a final expression"; tr: "'{name}' fonksiyonu son ifadeyle bitmiyor"),
                lstr!(en: "the result is missing here"; tr: "sonuç burada eksik"),
                lstr!(en: "end the body with the value to return, e.g. the last let's name";
                      tr: "gövdeyi döndürülecek değerle bitirin, ör. son let'in adı"),
            );
        }
    }

    /// Tanım → imza (önbellekli). fn olmayan tanım için `None`.
    pub(super) fn fn_sig(&mut self, def: DefId) -> Option<FnSig> {
        if let Some(sig) = self.fn_sigs.get(&def) {
            return Some(sig.clone());
        }
        let ast = self.ast;
        let item = *self.res.item_of_def.get(&def)?;
        let ItemKind::Fn(f) = &ast.items_arena[item].kind else {
            return None;
        };
        let sig = self.fn_sig_of_decl(f);
        self.fn_sigs.insert(def, sig.clone());
        Some(sig)
    }

    fn fn_sig_of_decl(&mut self, f: &FnDecl) -> FnSig {
        if let Some(&def) = self.res.decl_spans.get(&f.name.span) {
            if let Some(sig) = self.fn_sigs.get(&def) {
                return sig.clone();
            }
        }
        let params = f
            .params
            .iter()
            .map(|p| self.resolve_type_ref(p.ty))
            .collect();
        let ret = f.return_ty.map(|t| self.resolve_type_ref(t));
        let sig = FnSig { params, ret };
        if let Some(&def) = self.res.decl_spans.get(&f.name.span) {
            self.fn_sigs.insert(def, sig.clone());
        }
        sig
    }

    /// Kullanıcı fn çağrısı (Karar 5): arity, argüman `check`'i, dönüş tipi.
    pub(super) fn synth_fn_call(
        &mut self,
        sig: &FnSig,
        name: &str,
        args: &[Idx<Expr>],
        span: Span,
    ) -> TypeId {
        if args.len() != sig.params.len() {
            let (want, got) = (sig.params.len(), args.len());
            self.error(
                ErrorCode::E2003,
                span,
                lstr!(en: "function '{name}' takes {want} argument(s), {got} given";
                      tr: "'{name}' fonksiyonu {want} argüman alır, {got} verildi"),
                lstr!(en: "wrong number of arguments"; tr: "yanlış argüman sayısı"),
                lstr!(en: "pass one argument per parameter of '{name}'";
                      tr: "'{name}' fonksiyonunun her parametresine bir argüman verin"),
            );
            for &a in args {
                self.synth(a);
            }
        } else {
            for (&a, &ty) in args.iter().zip(&sig.params) {
                self.check(a, ty);
                // Karar 4: dizi parametre argümanın adıyla yer değiştirir.
                let ast = self.ast;
                if matches!(self.types.ty(ty), Ty::Array { .. })
                    && !matches!(ast.exprs[a].kind, ExprKind::Path(_))
                {
                    let span = ast.exprs[a].span;
                    self.err_fn_sig_unsupported(
                        span,
                        &lstr!(en: "an array argument that is not a plain name"; tr: "yalın ad olmayan dizi argümanı"),
                        &lstr!(en: "bind the array to a let or wire and pass its name (ADR-0081)";
                               tr: "diziyi bir let'e ya da wire'a bağlayıp adını geçirin (ADR-0081)"),
                    );
                }
            }
        }
        sig.ret.unwrap_or_else(|| self.types.error())
    }
}
