//! Test ifadelerinin kapsam ve tip denetimi (ADR-0058).
//!
//! Test değerleri iki türdür: 64 bit sayı ya da sayı dizisi. Diziler
//! yalnız `let` ile bağlanır (dizi literali ya da `read_hex`); içerikleri
//! derleme zamanında bilinir, bu yüzden uzunluk ve en büyük değer
//! kapsamda taşınır (`load` boyut denetimi için).

use std::collections::HashMap;

use volt_ast::{Name, TestExpr, TestExprKind};
use volt_diagnostics::{lstr, Diagnostic, ErrorCode, LabeledSpan};

use crate::sim::{bad_arity, check_port_read, unknown_builtin, DutMap, TEST_VALUE_BUILTINS};
use crate::sim_const::TestConsts;
use crate::testdata::{
    normalize_data_path, parse_readmemh, HexError, HexErrorReason, TestFileError, TestFileLoader,
};

/// Derleme zamanında bilinen dizi özeti; dosya okunamadıysa `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArrayInfo {
    pub len: usize,
    pub max: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarKind {
    Scalar,
    Array(Option<ArrayInfo>),
}

/// Test kapsamı: tek DUT + iç içe blokların yerel değişkenleri.
#[derive(Default)]
pub(crate) struct Scope<'a> {
    pub duts: DutMap<'a>,
    /// Derleme zamanında bilinen değerler (ADR-0060); çerçeveleri
    /// `frames` ile birlikte açılıp kapanır.
    pub consts: TestConsts,
    /// Tek dosyalık analiz (kardeş dosya yüklenmedi): bilinmeyen ad
    /// kardeşin üst düzey `const`u olabilir — E8506 tam denetime kalır.
    pub assume_external_names: bool,
    frames: Vec<HashMap<String, VarKind>>,
}

impl Scope<'_> {
    pub fn with_consts(consts: TestConsts, assume_external_names: bool) -> Self {
        Self {
            consts,
            assume_external_names,
            ..Self::default()
        }
    }

    pub fn push(&mut self) {
        self.frames.push(HashMap::new());
        self.consts.push();
    }

    pub fn pop(&mut self) {
        self.frames.pop();
        self.consts.pop();
    }

    pub fn lookup(&self, name: &str) -> Option<VarKind> {
        self.frames.iter().rev().find_map(|f| f.get(name).copied())
    }

    pub fn is_defined(&self, name: &str) -> bool {
        self.duts.contains_key(name) || self.lookup(name).is_some()
    }

    /// `constant`: sayının derleme zamanı değeri; dizi, döngü sayacı ve
    /// çalışma zamanı değerinde `None`.
    pub fn define(&mut self, name: &str, kind: VarKind, constant: Option<u64>) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.to_string(), kind);
        }
        self.consts.bind(name, constant);
    }

    /// Dizi adı bekleyen konumlar (`ad[i]`, `len(ad)`, `load` kaynağı).
    pub fn expect_array(&self, name: &Name, diags: &mut Vec<Diagnostic>) -> Option<ArrayInfo> {
        match self.lookup(&name.text) {
            Some(VarKind::Array(info)) => info,
            Some(VarKind::Scalar) => {
                diags.push(type_mismatch(
                    name.span,
                    lstr!(en: "'{}' is a number, not an array", name.text;
                          tr: "'{}' bir sayı, dizi değil", name.text),
                ));
                None
            }
            // Üst düzey `const` tanımlıdır ama sayıdır (ADR-0060).
            None if self.consts.global(&name.text).is_some() => {
                diags.push(type_mismatch(
                    name.span,
                    lstr!(en: "'{}' is a const number, not an array", name.text;
                          tr: "'{}' bir sabit sayı, dizi değil", name.text),
                ));
                None
            }
            None => {
                diags.push(undefined_name(name));
                None
            }
        }
    }

    /// Sayı bekleyen konumlar: port değeri, `step`, assert, operand,
    /// indeks, döngü sınırı.
    pub fn expect_scalar(&self, expr: &TestExpr, diags: &mut Vec<Diagnostic>) {
        match &expr.kind {
            TestExprKind::Int(_) | TestExprKind::Bool(_) => {}
            TestExprKind::PortRead { dut, port } => {
                if self.lookup(&dut.text).is_some() {
                    diags.push(type_mismatch(
                        dut.span,
                        lstr!(en: "'{}' is a test value; it has no ports", dut.text;
                              tr: "'{}' bir test değeri; portu yok", dut.text),
                    ));
                    return;
                }
                check_port_read(&self.duts, dut, port, diags);
            }
            TestExprKind::Var(name) => self.expect_scalar_var(name, diags),
            TestExprKind::Variant { enum_name, variant } => {
                self.expect_variant(enum_name, variant, diags)
            }
            TestExprKind::Index { base, index } => {
                let info = self.expect_array(base, diags);
                self.expect_scalar(index, diags);
                if let (Some(info), Some(i)) = (info, self.consts.eval(index)) {
                    if usize::try_from(i).map_or(true, |i| i >= info.len) {
                        diags.push(type_mismatch(
                            index.span,
                            lstr!(en: "index {i} is out of bounds: '{}' has {} element(s)", base.text, info.len;
                                  tr: "{i} indeksi sınır dışı: '{}' {} elemanlı", base.text, info.len),
                        ));
                    }
                }
            }
            TestExprKind::Unary { operand, .. } => self.expect_scalar(operand, diags),
            TestExprKind::Binary { lhs, rhs, .. } => {
                self.expect_scalar(lhs, diags);
                self.expect_scalar(rhs, diags);
            }
            TestExprKind::Call { func, args } => self.check_value_call(func, args, expr, diags),
            TestExprKind::Array(_) => diags.push(type_mismatch(
                expr.span,
                lstr!(en: "an array literal is not a number"; tr: "dizi literali sayı değildir"),
            )),
            TestExprKind::Str(_) => diags.push(type_mismatch(
                expr.span,
                lstr!(en: "a string is only valid as the read_hex() argument";
                      tr: "string yalnız read_hex() argümanı olarak geçerlidir"),
            )),
            // Tek dosyalık analiz: DUT kardeş dosyada, portun struct olup
            // olmadığı bilinmez — tam denetime kalır.
            TestExprKind::StructLit { .. } if self.assume_external_names => {}
            TestExprKind::StructLit { name, .. } => diags.push(type_mismatch(
                expr.span,
                lstr!(en: "a '{}' struct literal is not a number; it is valid only as a whole struct port value (dut.p = {} {{ ... }}, assert_eq(dut.q, {} {{ ... }}))", name.text, name.text, name.text;
                      tr: "'{}' struct literali sayı değildir; yalnız bütün struct portu değeri olarak geçerlidir (dut.p = {} {{ ... }}, assert_eq(dut.q, {} {{ ... }}))", name.text, name.text, name.text),
            )),
            // Dış DUT'ta `dut.q.a` bir struct portunun alanı olabilir
            // (ADR-0077); karar kardeşli tam denetimde.
            TestExprKind::MemberPath { dut, .. }
                if matches!(self.duts.get(dut.text.as_str()), Some(None)) => {}
            TestExprKind::MemberPath { .. } => diags.push(type_mismatch(
                expr.span,
                lstr!(en: "a path into a sub-instance is only valid as the load() target";
                      tr: "alt örneğe uzanan yol yalnız load() hedefi olarak geçerlidir"),
            )),
        }
    }

    /// `Enum::Varyant`: enum birimde bildirilmiş ve varyantı var mı
    /// (ADR-0074). Tek dosyalık analizde enum kardeş dosyada olabilir.
    fn expect_variant(&self, enum_name: &Name, variant: &Name, diags: &mut Vec<Diagnostic>) {
        let Some(variants) = self.consts.enum_variants(&enum_name.text) else {
            if !self.assume_external_names {
                diags.push(undefined_name(enum_name));
            }
            return;
        };
        if !variants.iter().any(|(n, _)| *n == variant.text) {
            let names: Vec<&str> = variants.iter().map(|(n, _)| n.as_str()).collect();
            diags.push(type_mismatch(
                variant.span,
                lstr!(en: "enum '{}' has no variant '{}' (variants: {})", enum_name.text, variant.text, names.join(", ");
                      tr: "'{}' enum'unda '{}' varyantı yok (varyantlar: {})", enum_name.text, variant.text, names.join(", ")),
            ));
        }
    }

    fn expect_scalar_var(&self, name: &Name, diags: &mut Vec<Diagnostic>) {
        match self.lookup(&name.text) {
            Some(VarKind::Scalar) => {}
            Some(VarKind::Array(_)) => diags.push(type_mismatch(
                name.span,
                lstr!(en: "'{}' is an array, not a number", name.text;
                      tr: "'{}' bir dizi, sayı değil", name.text),
            )),
            None if self.duts.contains_key(name.text.as_str()) => diags.push(type_mismatch(
                name.span,
                lstr!(en: "'{}' is the instance, not a value", name.text;
                      tr: "'{}' örneğin kendisi, değer değil", name.text),
            )),
            // Üst düzey `const` (ADR-0060): yalnız düz literal görünür.
            None => match self.consts.global(&name.text) {
                Some(Some(_)) => {}
                Some(None) => diags.push(computed_const(name)),
                None if self.assume_external_names => {}
                None => diags.push(undefined_name(name)),
            },
        }
    }

    /// Sayı konumunda değer yerleşiği: yalnız `len(dizi)`.
    fn check_value_call(
        &self,
        func: &Name,
        args: &[TestExpr],
        whole: &TestExpr,
        diags: &mut Vec<Diagnostic>,
    ) {
        let Some((_, arity)) = TEST_VALUE_BUILTINS.iter().find(|(n, _)| *n == func.text) else {
            diags.push(unknown_builtin(func));
            return;
        };
        if args.len() != *arity {
            diags.push(bad_arity(func, *arity, args.len(), whole.span));
            return;
        }
        if func.text == "read_hex" {
            diags.push(type_mismatch(
                whole.span,
                lstr!(en: "read_hex() yields an array; bind it with 'let' first";
                      tr: "read_hex() dizi üretir; önce 'let' ile bağlayın"),
            ));
            return;
        }
        match &args[0].kind {
            TestExprKind::Var(name) => {
                self.expect_array(name, diags);
            }
            _ => diags.push(type_mismatch(
                args[0].span,
                lstr!(en: "len() takes the name of an array"; tr: "len() bir dizi adı alır"),
            )),
        }
    }

    /// `let ad = <değer>;` sağ tarafı: dizi literali, `read_hex` ya da sayı.
    pub fn let_value(
        &self,
        value: &TestExpr,
        files: Option<&dyn TestFileLoader>,
        diags: &mut Vec<Diagnostic>,
    ) -> VarKind {
        match &value.kind {
            TestExprKind::Array(items) => VarKind::Array(array_literal(value, items, diags)),
            TestExprKind::Call { func, args } if func.text == "read_hex" => {
                if args.len() != 1 {
                    diags.push(bad_arity(func, 1, args.len(), value.span));
                    return VarKind::Array(None);
                }
                VarKind::Array(read_hex_info(&args[0], files, diags))
            }
            _ => {
                self.expect_scalar(value, diags);
                VarKind::Scalar
            }
        }
    }
}

/// Dizi literali: boş olamaz, elemanları sabit sayıdır.
fn array_literal(
    whole: &TestExpr,
    items: &[TestExpr],
    diags: &mut Vec<Diagnostic>,
) -> Option<ArrayInfo> {
    if items.is_empty() {
        diags.push(type_mismatch(
            whole.span,
            lstr!(en: "an array literal cannot be empty"; tr: "dizi literali boş olamaz"),
        ));
        return None;
    }
    let mut max = 0u64;
    let mut ok = true;
    for item in items {
        match &item.kind {
            TestExprKind::Int(n) => max = max.max(*n),
            TestExprKind::Bool(b) => max = max.max(u64::from(*b)),
            _ => {
                ok = false;
                diags.push(type_mismatch(
                    item.span,
                    lstr!(en: "array elements must be constant numbers";
                          tr: "dizi elemanları sabit sayı olmalıdır"),
                ));
            }
        }
    }
    ok.then_some(ArrayInfo {
        len: items.len(),
        max,
    })
}

/// `read_hex("yol")`: yol kuralı (E8507), dosya (E8507), biçim (E8508).
fn read_hex_info(
    arg: &TestExpr,
    files: Option<&dyn TestFileLoader>,
    diags: &mut Vec<Diagnostic>,
) -> Option<ArrayInfo> {
    let TestExprKind::Str(path) = &arg.kind else {
        diags.push(type_mismatch(
            arg.span,
            lstr!(en: "read_hex() takes a string literal path";
                  tr: "read_hex() string literali bir yol alır"),
        ));
        return None;
    };
    let Some(files) = files else {
        // Dosya erişimi yok: kök bilinmediğinden test dosyasının dizini
        // kök sayılır, içerik bilinmiyor kalır.
        if normalize_data_path(path, 0).is_none() {
            diags.push(outside_project(arg, path));
        }
        return None;
    };
    let text = match files.load(path) {
        Ok(text) => text,
        Err(TestFileError::OutsideProject) => {
            diags.push(outside_project(arg, path));
            return None;
        }
        Err(TestFileError::NotFound(os_error)) => {
            diags.push(Diagnostic::error(
                ErrorCode::E8507,
                lstr!(en: "cannot read test data file '{path}': {os_error}";
                      tr: "test veri dosyası '{path}' okunamadı: {os_error}"),
                LabeledSpan::primary(
                    arg.span,
                    lstr!(en: "no such file"; tr: "böyle bir dosya yok"),
                ),
                lstr!(en: "the path is resolved relative to the test file's directory";
                      tr: "yol, test dosyasının dizinine göre çözülür"),
            ));
            return None;
        }
    };
    match parse_readmemh(&text) {
        Ok(image) => Some(ArrayInfo {
            len: image.words.len(),
            max: image.words.iter().copied().max().unwrap_or(0),
        }),
        Err(err) => {
            diags.push(bad_hex(arg, path, &err));
            None
        }
    }
}

fn outside_project(arg: &TestExpr, path: &str) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8507,
        lstr!(en: "test data path '{path}' leaves the project";
              tr: "test veri yolu '{path}' projenin dışına çıkıyor"),
        LabeledSpan::primary(
            arg.span,
            lstr!(en: "outside the project directory"; tr: "proje dizininin dışında"),
        ),
        lstr!(en: "use a path relative to the test file that stays inside the project (no absolute paths, no '..' past the root)";
              tr: "test dosyasına göre göreli ve proje içinde kalan bir yol kullanın (mutlak yol yok, kökü aşan '..' yok)"),
    )
}

fn bad_hex(arg: &TestExpr, path: &str, err: &HexError) -> Diagnostic {
    let line = err.line;
    let reason = match &err.reason {
        HexErrorReason::BadDigit(c) => {
            lstr!(en: "'{c}' is not a hex digit"; tr: "'{c}' onaltılık basamak değil")
        }
        HexErrorReason::UnknownDigit => {
            lstr!(en: "x/z digits are not supported"; tr: "x/z basamakları desteklenmez")
        }
        HexErrorReason::TooWide => {
            lstr!(en: "word is wider than 64 bits"; tr: "kelime 64 bitten geniş")
        }
        HexErrorReason::MissingAddress => {
            lstr!(en: "'@' must be followed by a hex address"; tr: "'@' sonrasında onaltılık adres olmalı")
        }
        HexErrorReason::AddressTooLarge => {
            lstr!(en: "address exceeds 1M elements"; tr: "adres 1M elemanı aşıyor")
        }
        HexErrorReason::UnclosedComment => {
            lstr!(en: "unclosed '/*' comment"; tr: "kapanmamış '/*' yorumu")
        }
        HexErrorReason::Empty => lstr!(en: "the file holds no data"; tr: "dosyada veri yok"),
    };
    Diagnostic::error(
        ErrorCode::E8508,
        lstr!(en: "malformed hex file '{path}' (line {line}): {reason}";
              tr: "bozuk hex dosyası '{path}' (satır {line}): {reason}"),
        LabeledSpan::primary(arg.span, lstr!(en: "read here"; tr: "burada okunuyor")),
        lstr!(en: "read_hex() expects $readmemh text: hex words, '_', comments and '@addr'";
              tr: "read_hex() $readmemh metni bekler: onaltılık kelimeler, '_', yorumlar ve '@adres'"),
    )
}

fn undefined_name(name: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "'{}' is not defined in this test", name.text;
              tr: "'{}' bu testte tanımlı değil", name.text),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "used before 'let'"; tr: "'let'ten önce kullanım"),
        ),
        lstr!(en: "define it first: let {} = ...;", name.text;
              tr: "önce tanımlayın: let {} = ...;", name.text),
    )
}

/// Üst düzey `const` var ama değeri düz literal değil: test denetimi
/// const değerlendiriciden bağımsızdır (ADR-0033), değeri göremez.
fn computed_const(name: &Name) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8506,
        lstr!(en: "const '{}' is not a plain literal; its value is not visible in a test", name.text;
              tr: "'{}' sabiti düz literal değil; değeri testte görünmez", name.text),
        LabeledSpan::primary(
            name.span,
            lstr!(en: "computed const"; tr: "hesaplanmış sabit"),
        ),
        lstr!(en: "tests see only literal consts (const N : u8 = 8); bind the value here: let {} = ...;", name.text;
              tr: "testler yalnız literal sabitleri görür (const N : u8 = 8); değeri burada bağlayın: let {} = ...;", name.text),
    )
}

pub(crate) fn type_mismatch(span: volt_span::Span, message: String) -> Diagnostic {
    Diagnostic::error(
        ErrorCode::E8511,
        message,
        LabeledSpan::primary(
            span,
            lstr!(en: "wrong kind of value"; tr: "yanlış türde değer"),
        ),
        lstr!(en: "test values are numbers or arrays; see 'volt explain E8511'";
              tr: "test değerleri sayı ya da dizidir; bkz. 'volt explain E8511'"),
    )
}
