//! Üreticinin ad çözümü (#128): bir ad, kullanıldığı yerin kapsam
//! zincirinde çözülür (name-resolution.md §2, §3.1, §6). Açılmış fn
//! gövdesinin const'u, fn bildiriminin tip ve desen ifadeleri ve const
//! öğesinin değeri kök kapsamdadır (ADR-0081 Karar 12.1): çağıranın aynı
//! adlı döngü değişkeni, blok `let`'i ya da sinyali onları yakalamaz.
//! Blok kapsamında iç ad dıştakini gölgeler: döngü değişkeni modül
//! sinyalini, blok `let`'i const'u ve döngü değişkenini.
//!
//! Her testte iki kanıt var:
//! 1. Beklenen SV satırı elle, şartnameden: değer seçilen ada göre
//!    hesaplandı, biçim sv-mapping kuralından (paketlenmiş dizi elemanı
//!    `xs[8 * (i) +: 8]`, sabit indeks katlanır `xs[16 +: 8]`, const
//!    bildirilen tipinde boyutlu literaldir `32'd1`, ADR-0041/0056).
//! 2. Metamorfik: çakışan adı (döngü değişkeni, `let`, port) taze bir adla
//!    değiştirmek anlamı değiştirmez; SV, değiştirilen ad dışında aynıdır.
//!
//! Emitter doğrudan çağrılır (HIR'sız): W1002 uyarısının çıktığı
//! volt-driver'ın `name_hygiene_tests`'inde sınanır.

use volt_span::FileId;
use volt_sv_emit::{emit_full, EmitOutput, SvaMode};

fn emit_src(src: &str) -> EmitOutput {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    emit_full(&parsed.ast, "t.volt", src, SvaMode::None)
}

/// Hatasız üretilen SV; tanı kodlarıyla birlikte.
fn sv(src: &str) -> String {
    let out = emit_src(src);
    let codes: Vec<&str> = out.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.is_empty(), "tanı beklenmiyor: {codes:?}\n{}", out.sv);
    out.sv
}

/// `text` içinde `from` tanımlayıcısının her tam sözcük geçişi `to` olur.
fn rename_ident(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        out.push_str(if word == from { to } else { word });
        word.clear();
    };
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Metamorfik ilişki: `renamed` kaynağı `src`'den yalnız çakışan bir
/// bağlamanın adıyla (`fresh`) ayrılır; SV'leri, `fresh` geri `name`
/// yazıldığında aynıdır (yorumlardaki dosya:satır dahil).
fn assert_same_meaning(src: &str, renamed: &str, name: &str, fresh: &str) {
    let a = sv(src);
    let b = rename_ident(&sv(renamed), fresh, name);
    assert_eq!(
        a, b,
        "yeniden adlandırma anlamı değiştirdi\n--- özgün\n{a}\n--- {fresh}\n{b}"
    );
}

fn has_line(sv: &str, line: &str) -> bool {
    sv.lines().any(|l| l.trim() == line)
}

fn count_lines(sv: &str, line: &str) -> usize {
    sv.lines().filter(|l| l.trim() == line).count()
}

// ═══ Açılmış fn gövdesinin const'u, çağıranın döngü değişkeni ══════════

const ISSUE_128: &str = "const I : u32 = 1
fn pick(x: [u8; 4]) -> u8 { x[I] }
module M {
    in  xs : [u8; 4]
    out ys : [u8; 4]
    comb { for I in 0..4 { ys[I] = pick(xs) } }
}
";

/// #128: `pick`'in `I`'sı fn kapsamında köke, const'a bağlanır (ADR-0081
/// Karar 12.1); `ys[k] = pick(xs) = xs[1]` her k için. Hatada `xs[k]`.
#[test]
fn issue_128_a_function_const_in_an_index_is_not_the_callers_loop_variable() {
    let sv = sv(ISSUE_128);
    for k in 0..4 {
        let line = format!("ys[{} +: 8] = xs[8 * (32'd1) +: 8];", 8 * k);
        assert!(has_line(&sv, &line), "{line}\n{sv}");
    }
    assert_same_meaning(
        ISSUE_128,
        &ISSUE_128.replace("for I in 0..4 { ys[I]", "for q in 0..4 { ys[q]"),
        "I",
        "q",
    );
}

/// Hesaplanan indeks sabit katlanır: `I + 1` = 2 → `xs[16 +: 8]`.
#[test]
fn a_function_const_in_a_computed_index_is_not_the_callers_loop_variable() {
    let src = ISSUE_128.replace("x[I] }", "x[I + 1] }");
    let sv = sv(&src);
    assert_eq!(count_lines(&sv, "ys[8 +: 8] = xs[16 +: 8];"), 1, "{sv}");
    assert_eq!(sv.matches("= xs[16 +: 8];").count(), 4, "{sv}");
}

const PART: &str = "const I : u32 = 1
fn sl(a: u8) -> u2 { a[I +: 2] as u2 }
module M {
    in  a  : u8
    out ys : [u2; 2]
    comb { for I in 0..2 { ys[I] = sl(a) } }
}
";

/// Parça seçiminin başlangıcı: `a[1 +: 2]` her k için (bitler 2:1).
#[test]
fn a_function_const_as_a_part_select_start_is_not_the_callers_loop_variable() {
    let sv = sv(PART);
    assert!(has_line(&sv, "ys[0 +: 2] = a[32'd1 +: 2];"), "{sv}");
    assert!(has_line(&sv, "ys[2 +: 2] = a[32'd1 +: 2];"), "{sv}");
    assert_same_meaning(
        PART,
        &PART.replace("for I in 0..2 { ys[I]", "for q in 0..2 { ys[q]"),
        "I",
        "q",
    );
}

/// Aralık sınırı: `a[I + 1:I]` = `a[2:1]`; hatada ilk yinelemede `a[2:0]`
/// (üç bit iki bitlik hedefe).
#[test]
fn a_function_const_as_a_range_bound_is_not_the_callers_loop_variable() {
    let src = PART.replace("a[I +: 2] as u2", "a[I + 1:I] as u2");
    let sv = sv(&src);
    assert!(has_line(&sv, "ys[0 +: 2] = a[2:32'd1];"), "{sv}");
    assert!(has_line(&sv, "ys[2 +: 2] = a[2:32'd1];"), "{sv}");
}

const CONCAT: &str = "const K : u4 = 5
struct P { hi: u4, lo: u4 }
fn hi(a: u4) -> u8 { P { hi: K, lo: a } as u8 }
module M {
    in  a  : u4
    out ys : [u8; 2]
    comb { for K in 0..2 { ys[K] = hi(a) } }
}
";

/// Birleştirmenin genişliğe bağlı öğesi (ADR-0077 Karar 5): `K` u4 const,
/// `{4'd5, a}`; döngü değişkeni olsaydı genişliği bağlamdan alırdı.
#[test]
fn a_function_const_in_a_concatenation_keeps_its_declared_width() {
    let sv = sv(CONCAT);
    assert!(has_line(&sv, "ys[0 +: 8] = {4'd5, a};"), "{sv}");
    assert!(has_line(&sv, "ys[8 +: 8] = {4'd5, a};"), "{sv}");
}

const TYPE_WIDTH: &str = "const W : u32 = 4
fn inc(a: uint<W>) -> uint<W> { a + 1 }
module M {
    in  a  : u4
    out ys : [u4; 2]
    comb { for W in 0..2 { ys[W] = inc(a) } }
}
";

/// Tip ifadesi (`uint<W>`) fn bildiriminde kök kapsamdadır: W = 4.
/// Hatada döngü değişkeni (0) genişlik olur ve E2005 çıkar.
#[test]
fn a_function_type_width_names_the_const_not_the_callers_loop_variable() {
    let sv = sv(TYPE_WIDTH);
    assert!(has_line(&sv, "ys[0 +: 4] = 4'(a + 4'd1);"), "{sv}");
    assert!(has_line(&sv, "ys[4 +: 4] = 4'(a + 4'd1);"), "{sv}");
    assert_same_meaning(
        TYPE_WIDTH,
        &TYPE_WIDTH.replace("for W in 0..2 { ys[W]", "for q in 0..2 { ys[q]"),
        "W",
        "q",
    );
}

const LET_TYPE: &str = "const W : u32 = 4
fn lo(a: u8) -> u4 {
    let t : uint<W> = a[3:0] as u4
    t
}
module M {
    in  a : u8
    in  W : u8
    out y : u4
    out z : u4
    y = lo(a)
    comb { z = lo(a) }
}
";

/// `let`'in tipi (`uint<W>`) çağıranın aynı adlı portuna bağlanmaz: tel
/// kipinde `wire [3:0]`, ikame kipinde `4'(...)`. Hatada E2005.
#[test]
fn a_function_let_type_names_the_const_not_a_same_named_port() {
    let sv = sv(LET_TYPE);
    assert!(has_line(&sv, "wire [3:0] lo_0_t = a[3:0];"), "{sv}");
    assert!(has_line(&sv, "z = 4'(a[3:0]);"), "{sv}");
    assert_same_meaning(
        LET_TYPE,
        &LET_TYPE.replace("in  W : u8", "in  Wq : u8"),
        "W",
        "Wq",
    );
}

const NESTED: &str = "const I : u32 = 1
fn b(x: u4) -> bool { x[I] }
fn outer(x: u4) -> bool { b(x) }
module M {
    in  a  : u4
    out ys : [bool; 4]
    comb { for I in 0..4 { ys[I] = outer(a) } }
}
";

/// İç içe açılım: `outer` → `b`; `b`'nin `I`'sı yine const, `a[1]`.
#[test]
fn a_nested_function_const_is_not_the_callers_loop_variable() {
    let sv = sv(NESTED);
    for k in 0..4 {
        let line = format!("ys[{k} +: 1] = a[32'd1];");
        assert!(has_line(&sv, &line), "{line}\n{sv}");
    }
}

const PATTERN: &str = "const I : u2 = 1
fn cls(x: u2) -> u8 { match x { I => 7, _ => 0 } }
module Top {
    in  s  : u2
    out ys : [u8; 4]
    comb { for I in 0..4 { ys[I] = cls(s) } }
}
";

/// Desendeki çıplak ad değerdir (§5.1, ADR-0085), fn kapsamında const:
/// her k için `s == 1`. Hatada `s == k`.
#[test]
fn a_function_const_pattern_is_not_the_callers_loop_variable() {
    let sv = sv(PATTERN);
    assert_eq!(
        sv.matches("= 8'((s == 2'd1) ? 8'd7 : 8'd0);").count(),
        4,
        "{sv}"
    );
    assert_same_meaning(
        PATTERN,
        &PATTERN.replace("for I in 0..4 { ys[I]", "for q in 0..4 { ys[q]"),
        "I",
        "q",
    );
}

const ENUM_PATTERN: &str = "enum Mode { A, B, C }
enum Other { X, Y, Z }
const M : Mode = Mode::B
fn sel(x: Mode) -> u8 { match x { M => 1, _ => 0 } }
module Top {
    in  m : Mode
    in  M : Other
    out y : u8
    y = sel(m)
}
";

/// Enum tipli const deseni: `Mode_B`; çağıranın `M` portu (başka enum)
/// değil.
#[test]
fn a_function_enum_const_pattern_is_not_a_same_named_port() {
    let sv = sv(ENUM_PATTERN);
    assert!(has_line(&sv, "Mode_B: y = 8'd1;"), "{sv}");
    assert!(!has_line(&sv, "M: y = 8'd1;"), "{sv}");
    assert_same_meaning(
        ENUM_PATTERN,
        &ENUM_PATTERN.replace("in  M : Other", "in  Mq : Other"),
        "M",
        "Mq",
    );
}

const TABLE: &str = "const T : [u8; 4] = [10, 20, 30, 40]
fn lut() -> u8 { T[2] }
fn lutv(i: u2) -> u8 { T[i] }
module Top {
    in  i  : u2
    out ys : [u8; 2]
    out zs : [u8; 2]
    comb { for T in 0..2 { ys[T] = lut()
                           zs[T] = lutv(i) } }
}
";

/// Dizi sabiti: `T[2]` = 30, `T[i]` tablo işlevi. Hatada döngü değişkeni
/// T'yi gölgeler ve "sabit dizi değer olarak kullanılamaz" (E2005) çıkar.
#[test]
fn a_function_const_array_is_not_the_callers_loop_variable() {
    let sv = sv(TABLE);
    assert!(has_line(&sv, "ys[0 +: 8] = 8'd30;"), "{sv}");
    assert!(has_line(&sv, "zs[8 +: 8] = T_at(i);"), "{sv}");
    assert!(sv.contains("function automatic logic [7:0] T_at("), "{sv}");
}

/// Dizi sabiti, çağıranın aynı adlı port dizisi: yine tablo.
#[test]
fn a_function_const_array_is_not_a_same_named_port() {
    let src = "const T : [u8; 4] = [10, 20, 30, 40]
fn lutv(i: u2) -> u8 { T[i] }
fn lut() -> u8 { T[2] }
module Top {
    in  i : u2
    in  T : [u8; 4]
    out y : u8
    out z : u8
    y = lutv(i)
    z = lut()
}
";
    let sv = sv(src);
    assert!(has_line(&sv, "assign y = T_at(i);"), "{sv}");
    assert!(has_line(&sv, "assign z = 8'd30;"), "{sv}");
}

/// Aralık dış kapsamda (const, 2 yineleme); gövdedeki çağrı yine const:
/// `xs[2]`.
#[test]
fn a_loop_range_names_the_const_its_variable_shadows() {
    let src = "const I : u32 = 2
fn pick(x: [u8; 4]) -> u8 { x[I] }
module M {
    in  xs : [u8; 4]
    out ys : [u8; 4]
    comb {
        ys = xs
        for I in 0..I { ys[I] = pick(xs) }
    }
}
";
    let sv = sv(src);
    assert!(has_line(&sv, "ys[0 +: 8] = xs[8 * (32'd2) +: 8];"), "{sv}");
    assert!(has_line(&sv, "ys[8 +: 8] = xs[8 * (32'd2) +: 8];"), "{sv}");
    assert!(!sv.contains("ys[16 +: 8]"), "iki yineleme: {sv}");
}

/// Const'un değeri kök kapsamda değerlendirilir: `J = I + 1` = 2, kullanıldığı
/// döngünün `I`'sı onu değiştirmez.
#[test]
fn a_const_value_is_evaluated_in_the_root_scope() {
    let src = "const I : u32 = 1
const J : u32 = I + 1
module M {
    in  xs : [u8; 4]
    out ys : [u8; 4]
    comb { for I in 0..4 { ys[I] = xs[J] } }
}
";
    let sv = sv(src);
    assert_eq!(sv.matches("= xs[8 * (32'd2) +: 8];").count(), 4, "{sv}");
}

/// Const literale katlanır, bağlamla boyutlanır: `8'd5`. Aynı adlı port
/// onu sinyal sanıp `8'(8'd5)` diye sarmamalı (değer aynı, biçim değil).
#[test]
fn a_function_const_beside_a_same_named_port_is_a_literal_not_a_signal() {
    let src = "const K : u4 = 5
fn f(a: u8) -> u8 { a + K }
module Top {
    in  a : u8
    in  K : u4
    out y : u8
    y = f(a)
}
";
    let sv = sv(src);
    assert!(has_line(&sv, "assign y = a + 8'd5;"), "{sv}");
}

/// Esnek aralık (ADR-0025): gövdedeki `K` const'tur (u16, aralık 16..16);
/// çağıranın aynı adlı esnek `let`'inin (8..9) alt ucu ona sızmaz. `t = K + K`
/// 16..17, `u = t ^ x` (x : u12) kesişmez → doğal genişlik 17. Sızsaydı
/// t 8..17 olurdu ve u 12 bit. (HIR bu girdiyi E2031 ile reddeder; emitter
/// doğrudan çağrıldığında da çözüm aynı kalmalı.)
#[test]
fn a_function_const_range_ignores_a_same_named_flexible_let() {
    let src = "const K : u16 = 1000
fn f(x: u12) -> u17 {
    let t = K + K
    let u = t ^ x
    u
}
module Top {
    in  a : u8
    in  b : u8
    in  x : u12
    out y : u17
    out z : u9
    let K = a + b
    y = f(x)
    z = K
}
";
    let out = emit_src(src);
    assert!(out.sv.contains("wire [16:0] f_0_t = "), "{}", out.sv);
    assert!(out.sv.contains("wire [16:0] f_0_u = "), "{}", out.sv);
}

// ═══ Blok kapsamı: iç ad dıştakini gölgeler (§2, §6) ═══════════════════

/// Blok `let`'i const'u gölgeler: `xs[K + 0]`, K yerel (s); hatada const 3
/// (`xs[24 +: 8]`).
#[test]
fn a_block_let_shadows_a_const_in_a_computed_index() {
    let src = "const K : u32 = 3
module M {
    in  xs : [u8; 4]
    in  s  : u2
    out y  : u8
    comb { let K = s
           y = xs[K + 0] }
}
";
    let sv = sv(src);
    assert!(has_line(&sv, "y = xs[8 * ((3'(K) + 3'd0)) +: 8];"), "{sv}");
    assert!(!sv.contains("xs[24 +: 8]"), "{sv}");
    assert_same_meaning(
        src,
        &src.replace("let K = s", "let q = s")
            .replace("xs[K + 0]", "xs[q + 0]"),
        "K",
        "q",
    );
}

/// Modül sinyali const'u gölgeler: `xs[K + 0]`, K port; hatada const 3.
#[test]
fn a_module_signal_shadows_a_const_in_a_computed_index() {
    let src = "const K : u32 = 3
module M {
    in  xs : [u8; 4]
    in  K  : u2
    out y  : u8
    y = xs[K + 0]
}
";
    let sv = sv(src);
    assert!(
        has_line(&sv, "assign y = xs[8 * ((3'(K) + 3'd0)) +: 8];"),
        "{sv}"
    );
    assert_same_meaning(
        src,
        &src.replace("in  K  : u2", "in  q  : u2")
            .replace("xs[K + 0]", "xs[q + 0]"),
        "K",
        "q",
    );
}

const LOOP_OVER_PORT: &str = "module M {
    in  i  : u2
    in  xs : [u8; 4]
    out ys : [u8; 4]
    out zs : [u2; 4]
    out ws : [u8; 4]
    comb { for i in 0..4 { ys[i] = xs[i]
                           zs[i] = i
                           ws[i] = xs[i + 0] } }
}
";

/// Döngü değişkeni aynı adlı portu gölgeler: değer konumunda `2'dk`,
/// hesaplanan indekste `xs[k]`. Hatada port `i` okunur.
#[test]
fn a_loop_variable_shadows_a_same_named_port() {
    let sv = sv(LOOP_OVER_PORT);
    for k in 0..4 {
        for line in [
            format!("ys[{} +: 8] = xs[{} +: 8];", 8 * k, 8 * k),
            format!("zs[{} +: 2] = 2'd{k};", 2 * k),
            format!("ws[{} +: 8] = xs[{} +: 8];", 8 * k, 8 * k),
        ] {
            assert!(has_line(&sv, &line), "{line}\n{sv}");
        }
    }
    assert_same_meaning(
        LOOP_OVER_PORT,
        &LOOP_OVER_PORT.replace("in  i  : u2", "in  iq : u2"),
        "i",
        "iq",
    );
}

/// Döngü gövdesindeki blok `let`'i döngü değişkenini gölgeler: `xs[i]`
/// yereldir (s), her yinelemede `xs[8 * (i) +: 8]`; hatada `xs[k]`.
#[test]
fn a_block_let_in_a_loop_body_shadows_the_loop_variable() {
    let src = "module M {
    in  s  : u2
    in  xs : [u8; 4]
    out ys : [u8; 2]
    comb { for i in 0..2 { let i = s
                           ys[0] = xs[i] }
           ys[1] = 0 }
}
";
    let sv = sv(src);
    assert_eq!(
        count_lines(&sv, "ys[0 +: 8] = xs[8 * (i) +: 8];"),
        2,
        "{sv}"
    );
    assert_same_meaning(
        src,
        &src.replace("let i = s", "let q = s")
            .replace("xs[i] }", "xs[q] }"),
        "i",
        "q",
    );
}

/// Döngüden önce bildirilen blok `let`'i döngü değişkeninin dışındadır:
/// döngüde `i` döngü değişkenidir, döngüden sonra yine yerel.
#[test]
fn a_loop_variable_shadows_an_outer_block_let() {
    let src = "module M {
    in  s  : u2
    in  xs : [u8; 4]
    out ys : [u8; 2]
    out y  : u8
    comb { let i = s
           for i in 0..2 { ys[i] = xs[i] }
           y = xs[i] }
}
";
    let sv = sv(src);
    assert!(has_line(&sv, "ys[0 +: 8] = xs[0 +: 8];"), "{sv}");
    assert!(has_line(&sv, "ys[8 +: 8] = xs[8 +: 8];"), "{sv}");
    assert!(has_line(&sv, "y = xs[8 * (i) +: 8];"), "{sv}");
}

/// Tanı kodları (HIR'sız emitter; döngüsel const'u HIR E2020 ile reddeder).
fn emit_codes(src: &str) -> (Vec<String>, String) {
    let out = emit_src(src);
    let codes = out
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_string())
        .collect();
    (codes, out.sv)
}

/// Döngüsel const zinciri (`A → B → A`) emitter'da sonsuz özyinelemeye
/// girmez: derinlik sınırında değerlendirme biter, katlanamayan const
/// E0003 olur. (HIR bu girdiyi E2020 ile emitter'a varmadan reddeder;
/// emitter'ın kendi sınırı yine de yığın taşmasını önler.)
#[test]
fn a_cyclic_const_chain_stops_at_the_depth_limit() {
    let (codes, sv) = emit_codes(
        "const A : u32 = B
const B : u32 = A
module M {
    out o : u32
    o = A
}
",
    );
    assert_eq!(codes, ["E0003"], "{sv}");
}

/// Enum tipli döngüsel const zinciri match'in sınananında: `const_match_key`
/// kendini çağırır; derinlik sınırı orada da denetlenir. Düzeltmeden önce
/// yığın taşıyordu.
#[test]
fn a_cyclic_enum_const_chain_in_a_match_stops_at_the_depth_limit() {
    let (codes, sv) = emit_codes(
        "enum E { X, Y }
const A : E = B
const B : E = A
const N : u8 = match A { E::X => 1, E::Y => 2 }
module M {
    out o : u8
    o = N
}
",
    );
    assert_eq!(codes, ["E0003"], "{sv}");
}

/// `n` halkalı enum const zinciri: `A0 = A1`, …, `A{n-1} = E::Y`.
fn enum_chain(n: usize) -> String {
    let mut src = String::from("enum E { X, Y }\n");
    for j in 0..n {
        let value = if j + 1 == n {
            "E::Y".to_string()
        } else {
            format!("A{}", j + 1)
        };
        src.push_str(&format!("const A{j} : E = {value}\n"));
    }
    src.push_str(
        "const N : u8 = match A0 { E::X => 1, E::Y => 2 }
module M {
    out o : u8
    o = N
}
",
    );
    src
}

/// Derinlik sınırı 64'tür (u32 zinciriyle aynı sınır). Derinlik 0'dan
/// sayılır: N'nin match'i 0, sınanan `A0` 1, `A{j}` j + 1, son halkanın
/// değeri `E::Y` n + 1; 64'ü aşan derinlik değerlendirilmez. n = 63'te
/// `E::Y` 64. derinlikte: E::Y kolu seçilir, N = 2, u8 → `8'd2`. n = 64'te
/// 65. derinlikte: N katlanamaz, E0003.
#[test]
fn an_enum_const_chain_folds_up_to_the_depth_limit() {
    let src = enum_chain(63);
    let (codes, sv) = emit_codes(&src);
    assert!(codes.is_empty(), "{codes:?}\n{sv}");
    assert!(has_line(&sv, "assign o = 8'd2;"), "{sv}");

    let (codes, sv) = emit_codes(&enum_chain(64));
    assert_eq!(codes, ["E0003"], "{sv}");
}
