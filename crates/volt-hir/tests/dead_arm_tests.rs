//! Tasarıma yazılmayan `match` kolu donanım analizlerine girmez (#328,
//! #331, #332).
//!
//! Bir match'te ilk muhafızsız joker koldan sonraki kol ve önceki
//! kolların kapsadığı kol hiç seçilmez ve SV'ye yazılmaz (sv-mapping.md
//! §19.1, W2014). Bakımcının kararı: böyle bir kol tip denetiminden geçer
//! ve W2014 alır, ama donanımı inceleyen hiçbir analiz onu görmez — sürücü
//! (E4001, E4002, E4012, type-inference.md §11.1), saat alanı (E3xxx),
//! güven seviyesi, sıfırlama alanı, el sıkışma (E4007), zamanlama.
//!
//! Her analiz için üç biçim: jokerden sonraki kol (`after`), önceki kolun
//! kapsadığı sayı kolu (`covered`), önceki kolun kapsadığı enum varyantı
//! (`variant`). Canlı kolların etkilenmediğini gösteren testler sonda.

use volt_hir::{analyze, AnalysisResult};
use volt_syntax::{parse, FileId};

fn check(src: &str) -> AnalysisResult {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "kaynak ayrışmalı: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
}

fn sorted(src: &str) -> Vec<&'static str> {
    let mut c = check(src).error_codes();
    c.sort_unstable();
    c
}

/// `s` (u2) ya da `e` (E) üzerindeki match'in üç ölü kol biçimi. `{live}`
/// canlı kolun gövdesi, `{dead}` ölü kolun gövdesi; ölü kol her biçimde
/// `s == 1` / `E::B`'yi adlandırır.
fn forms(live: &str, dead: &str) -> [(&'static str, String); 3] {
    [
        (
            "after",
            format!(
                "match s {{\n            0 => {{ {live} }}\n            _ => {{ {live} }}\n            1 => {{ {dead} }}\n        }}"
            ),
        ),
        (
            "covered",
            format!(
                "match s {{\n            1 => {{ {live} }}\n            1 => {{ {dead} }}\n            _ => {{ {live} }}\n        }}"
            ),
        ),
        (
            "variant",
            format!(
                "match e {{\n            E::A => {{ {live} }}\n            E::B => {{ {live} }}\n            E::B => {{ {dead} }}\n            E::C => {{ {live} }}\n        }}"
            ),
        ),
    ]
}

const ENUM: &str = "enum E { A, B, C }\n";

// ═══ Sürücü: E4002, E4001, E4012 (type-inference.md §11) ═════════

fn driver_module(m: &str, tail: &str) -> String {
    format!(
        "{ENUM}module M {{\n    in  s : u2\n    in  e : E\n    out a : u8\n    out c : u8\n    out u : bool\n    comb {{\n        {m}\n    }}\n    u = s == 0 || e == E::A\n{tail}}}\n"
    )
}

/// #331: yalnız ölü kolda atanan çıkışı hiçbir şey sürmez: E4002.
#[test]
fn output_assigned_only_in_a_dead_arm_is_undriven_e4002() {
    for (form, m) in forms("a = 1", "a = 9; c = 3") {
        let src = driver_module(&m, "");
        assert_eq!(sorted(&src), ["E4002", "W2014"], "{form}");
        let d = check(&src).diagnostics;
        let undriven = d.iter().find(|d| d.code.as_str() == "E4002").unwrap();
        assert!(
            undriven.message.contains("'c'"),
            "{form}: {}",
            undriven.message
        );
    }
}

/// #331: ölü koldaki atama blok dışındaki sürücüyle çakışmaz (E4001 yok).
#[test]
fn dead_arm_assignment_is_not_a_second_driver() {
    for (form, m) in forms("a = 1", "a = 9; c = 3") {
        let src = driver_module(&m, "    c = 5\n");
        assert_eq!(sorted(&src), ["W2014"], "{form}");
    }
}

/// #331: kısmi atama da aynı kural: yalnız ölü kolda sürülen bitler
/// sürülmez, çıkışın kalan bitleri E4012 alır (ADR-0077 Karar 6).
#[test]
fn bits_assigned_only_in_a_dead_arm_are_e4012() {
    for (form, m) in forms(
        "a = 1; c[3:0] = 1 as bits<4>",
        "a = 9; c[7:4] = 3 as bits<4>",
    ) {
        let src = driver_module(&m, "");
        assert_eq!(sorted(&src), ["E4012", "W2014"], "{form}");
        let d = check(&src).diagnostics;
        let gap = d.iter().find(|d| d.code.as_str() == "E4012").unwrap();
        assert!(gap.message.contains("4..=7"), "{form}: {}", gap.message);
    }
}

/// Ölü kolun içindeki iç içe `match`'in canlı kolu da ölüdür.
#[test]
fn nested_match_in_a_covered_arm_drives_nothing() {
    let src = driver_module(
        "match s {\n            1 => { a = 1 }\n            1 => {\n                match e {\n                    E::A => { c = 1 }\n                    _ => { c = 2 }\n                }\n            }\n            _ => { a = 7 }\n        }",
        "",
    );
    assert_eq!(sorted(&src), ["E4002", "W2014"]);
}

// ═══ Saat alanı: E3001 (domain-inference.md K5, K6) ═══════════════

const TWO_DOMAINS: &str = "domain Fast { clock = posedge, reset = sync active_high }\n\
                           domain Slow { clock = posedge, reset = sync active_high }\n";

fn domain_module(m: &str) -> String {
    format!(
        "{TWO_DOMAINS}{ENUM}module M {{\n    in  fast_clk  : clock @Fast\n    in  slow_clk  : clock @Slow\n    in  s         : u2 @Slow\n    in  e         : E @Slow\n    in  fast_data : u8 @Fast\n    out slow_data : u8 @Slow\n    comb {{\n        {m}\n    }}\n}}\n"
    )
}

/// #332: ölü koldaki alanlar arası atama donanımda yoktur: E3001 yok.
#[test]
fn crossing_only_in_a_dead_arm_is_not_e3001() {
    for (form, m) in forms("slow_data = 1", "slow_data = fast_data") {
        let src = domain_module(&m);
        let c = sorted(&src);
        assert!(!c.iter().any(|c| c.starts_with('E')), "{form}: {c:?}");
        assert!(c.contains(&"W2014"), "{form}: {c:?}");
    }
}

/// İfade biçimi: `match` ifadesinin tasarıma yazılmayan kolu (W2014)
/// zincirde de yoktur (sv-mapping.md §19.1). `{live}` canlı kolların,
/// `{dead}` ölü kolun değeri.
fn expr_forms(live: &str, dead: &str) -> [(&'static str, String); 3] {
    [
        (
            "after",
            format!("match s {{ 0 => {live}, _ => {live}, 1 => {dead} }}"),
        ),
        (
            "covered",
            format!("match s {{ 1 => {live}, 1 => {dead}, _ => {live} }}"),
        ),
        (
            "variant",
            format!("match e {{ E::A => {live}, E::B => {live}, E::B => {dead}, E::C => {live} }}"),
        ),
    ]
}

/// #332, ifade biçimi: ölü kolun okuduğu başka alandaki değer sonucun
/// alanına katılmaz (K5, ADR-0083 Karar 8): E3001 yok.
#[test]
fn foreign_value_read_only_in_a_dead_expression_arm_is_not_e3001() {
    for (form, m) in expr_forms("1", "fast_data") {
        let src = domain_module(&format!("slow_data = {m}"));
        let c = sorted(&src);
        assert!(!c.iter().any(|c| c.starts_with('E')), "{form}: {c:?}");
        assert!(c.contains(&"W2014"), "{form}: {c:?}");
    }
}

/// K4: register'ın alanı onu yazan `on` bloğundan gelir. Ölü koldaki
/// yazma yazıcı değildir: `r` yalnız `slow_clk`'ta yazılır, iki saatin
/// yazdığı register hatası (E3011) yoktur.
#[test]
fn register_write_in_a_dead_arm_is_not_a_second_clock() {
    for (form, m) in forms("f <= 1", "r <= 2") {
        let src = format!(
            "{TWO_DOMAINS}{ENUM}module M {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    in  s : u2 @Fast\n    in  e : E @Fast\n    out y : u8 @Slow\n    out u : bool @Fast\n    reg r : u8 = 0\n    reg f : u8 = 0\n    on slow_clk {{ r <= 3 }}\n    on fast_clk {{\n        f <= 0\n        {m}\n    }}\n    y = r\n    u = s == 0 || e == E::A || f == 0\n}}\n"
        );
        let c = sorted(&src);
        assert!(!c.iter().any(|c| c.starts_with('E')), "{form}: {c:?}");
        assert!(c.contains(&"W2014"), "{form}: {c:?}");
    }
}

/// K7: `on` bloğunda başka alandan okuma senkronizörsüz olamaz (E3012).
/// Ölü kolun muhafızı hiç değerlendirilmez: okuduğu hızlı sinyal yavaş
/// bloğa girmez.
#[test]
fn foreign_read_in_a_dead_arm_guard_is_not_e3012() {
    let src = format!(
        "{TWO_DOMAINS}module G {{
    in  fast_clk : clock @Fast
    in  slow_clk : clock @Slow
    in  s : u2 @Slow
    in  b : bool @Fast
    out y : u8 @Slow
    out z : bool @Fast
    reg r : u8 = 0
    on slow_clk {{
        match s {{
            0 => {{ r <= 1 }}
            _ => {{ r <= 2 }}
            1 if b => {{ r <= 3 }}
        }}
    }}
    y = r
    z = b
}}
"
    );
    let c = sorted(&src);
    assert!(!c.iter().any(|c| c.starts_with('E')), "{c:?}");
    assert!(c.contains(&"W2014"), "{c:?}");
}

// ═══ Güven seviyesi: E3009 (domain-inference.md K11, ADR-0052) ═════

const TRUST: &str = "domain Secure { clock = posedge, reset = sync active_high, trust_level = secret }\n\
                     domain Debug  { clock = posedge, reset = sync active_high, trust_level = public }\n";

fn trust_codes(src: &str) -> Vec<&'static str> {
    sorted(src)
        .into_iter()
        .filter(|c| matches!(*c, "E3009" | "W3008"))
        .collect()
}

/// Ölü koldaki gizli değer açık çıkışa akmaz: E3009 yok (deyim ve ifade).
#[test]
fn secret_flow_only_in_a_dead_arm_is_not_e3009() {
    let module = |body: &str| {
        format!(
            "{TRUST}{ENUM}module M {{\n    in  key : u8 @Secure\n    in  s : u2 @Debug\n    in  e : E @Debug\n    out dbg : u8 @Debug\n    out dbg2 : u8 @Debug\n    {body}\n}}\n"
        )
    };
    for ((form, m), (_, x)) in forms("dbg = 1", "dbg = key")
        .into_iter()
        .zip(expr_forms("1", "key"))
    {
        let src = module(&format!("comb {{\n        {m}\n    }}\n    dbg2 = {x}"));
        assert!(trust_codes(&src).is_empty(), "{form}: {:?}", sorted(&src));
        assert!(
            sorted(&src).contains(&"W2014"),
            "{form}: {:?}",
            sorted(&src)
        );
    }
}

// ═══ Zamanlama: E5010 (ADR-0037) ═════════════════════════════════

fn e5010(src: &str) -> usize {
    check(src)
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E5010")
        .count()
}

/// `b` 2 çevrim gecikmeli: canlı kaynağı `a` (1 çevrim) + register = 2.
/// Ölü koldaki `b <= x` (0 + 1 = 1) donanımda yoktur: E5010 yok.
#[test]
fn dead_arm_source_does_not_break_strict_timing() {
    for ((form, m), (_, x)) in forms("b <= a", "b <= x")
        .into_iter()
        .zip(expr_forms("a", "x"))
    {
        for body in [m, format!("b <= {x}")] {
            let src = format!(
                "{ENUM}@strict_timing\nmodule M {{\n    in  clk : clock\n    in  x : u32\n    in  s : u2\n    in  e : E\n    out y : u32\n    out u : bool\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    on clk {{\n        a <= x\n        {body}\n    }}\n    y = b\n    u = s == 0 || e == E::A\n}}\n"
            );
            assert_eq!(e5010(&src), 0, "{form}: {body}");
            assert!(
                sorted(&src).contains(&"W2014"),
                "{form}: {:?}",
                sorted(&src)
            );
        }
    }
}

// ═══ Modül özeti: E4007 ve K8b (ADR-0050, ADR-0114) ═══════════════

/// Ölü koldaki `valid = ready` kombinasyonel kenar değildir: E4007 yok.
#[test]
fn valid_from_ready_only_in_a_dead_arm_is_not_e4007() {
    for (form, m) in forms("tx.valid = have", "tx.valid = tx.ready") {
        let src = format!(
            "{ENUM}module Producer {{\n    in  clk : clock\n    in  have : bool\n    in  s : u2\n    in  e : E\n    out tx : Handshake<u8>\n    out u : bool\n    reg count : u8 = 0\n    on clk {{\n        if tx.fired {{\n            count <= count + 1\n        }}\n    }}\n    comb {{\n        {m}\n    }}\n    tx.data = count\n    u = s == 0 || e == E::A\n}}\n"
        );
        let c = sorted(&src);
        assert!(!c.iter().any(|c| c.starts_with('E')), "{form}: {c:?}");
        assert!(c.contains(&"W2014"), "{form}: {c:?}");
    }
}

/// K8b: saatsiz alt modülün çıkışı bağlı olduğu girişlerin alanını taşır.
/// `y` yalnız ölü kolda `b`'yi okur: özet `y -> b` kenarı taşımaz, üst
/// modülde `b`'ye bağlı hızlı veri yavaş çıkışa sızmaz (E3001 yok).
#[test]
fn clockless_child_read_only_in_a_dead_arm_is_not_a_summary_edge() {
    for (form, m) in forms("y = a", "y = b") {
        let src = format!(
            "{TWO_DOMAINS}{ENUM}module Pick {{\n    in  a : u8\n    in  b : u8\n    in  s : u2\n    in  e : E\n    out y : u8\n    comb {{\n        {m}\n    }}\n}}\nmodule Top {{\n    in  fast_clk  : clock @Fast\n    in  slow_clk  : clock @Slow\n    in  s         : u2 @Slow\n    in  e         : E @Slow\n    in  slow_in   : u8 @Slow\n    in  fast_data : u8 @Fast\n    out slow_out  : u8 @Slow\n    let p = Pick {{ a: slow_in, b: fast_data, s: s, e: e }}\n    slow_out = p.y\n}}\n"
        );
        let c = sorted(&src);
        assert!(!c.contains(&"E3001"), "{form}: {c:?}");
        assert!(c.contains(&"W2014"), "{form}: {c:?}");
    }
}

// ═══ SDC kısıtları: sync köprüsünün kaynak saati (ADR-0054) ════════

/// Register'ın saati onu yazan `on` bloğundan gelir. Yavaş bloktaki ölü
/// koldaki yazma yazıcı değildir: `r`'nin saati `fast_clk`, `sync`
/// köprüsü oradan başlar.
#[test]
fn dead_arm_write_does_not_set_the_bridge_source_clock() {
    for (form, m) in forms("q <= 1", "r <= 0") {
        let src = format!(
            "domain Fast {{ clock = posedge, frequency = 100.mhz }}\ndomain Slow {{ clock = posedge, frequency = 25.mhz }}\n{ENUM}module M {{\n    in  fast_clk : clock @Fast\n    in  slow_clk : clock @Slow\n    in  a : u8 @Fast\n    in  s : u2 @Slow\n    in  e : E @Slow\n    out y : u8 @Slow\n    out z : u8 @Slow\n    reg r : u8 = 0\n    reg q : u8 = 0\n    on slow_clk {{\n        {m}\n    }}\n    on fast_clk {{ r <= a }}\n    y = sync(r, slow_clk)\n    z = q\n}}\n"
        );
        let parsed = parse(FileId(0), &src);
        assert!(
            parsed.diagnostics.is_empty(),
            "{form}: {:?}",
            parsed.error_codes()
        );
        let c = sorted(&src);
        assert!(!c.iter().any(|c| c.starts_with('E')), "{form}: {c:?}");
        // Sürücünün kısıt kaynağı: aşama hattı (`volt build --emit sdc`).
        let mut out = Vec::new();
        let stages = volt_hir::run_semantic_stages(
            &parsed.ast,
            volt_hir::resolve_file(&parsed.ast),
            None,
            &volt_hir::unit_load::TestTarget::Partial(None),
            &mut out,
        );
        let r = stages.constraints.expect("kısıtlar");
        let b = &r.modules[0].bridges[0];
        assert_eq!(b.from_clock.as_deref(), Some("fast_clk"), "{form}");
    }
}

// ═══ Canlı kollar etkilenmez ═════════════════════════════════════

/// Aynı atamalar canlı bir kolda (jokerin kendisi, kapsanmayan kol)
/// eskisi gibi analize girer: E3001, E3009, E5010, E4007, E4001.
#[test]
fn the_same_assignments_in_live_arms_are_still_analysed() {
    let live = "match s {\n            1 => { X }\n            _ => { Y }\n        }";
    let domain = domain_module(
        &live
            .replace("X", "slow_data = 1")
            .replace("Y", "slow_data = fast_data"),
    );
    assert!(sorted(&domain).contains(&"E3001"), "{:?}", sorted(&domain));
    let domain_expr = domain_module("slow_data = match s { 1 => 1, 2 | 3 => fast_data, _ => 1 }");
    assert!(
        sorted(&domain_expr).contains(&"E3001"),
        "{:?}",
        sorted(&domain_expr)
    );
    let driver = driver_module(
        &live.replace("X", "a = 1").replace("Y", "a = 7; c = 3"),
        "    c = 5\n",
    );
    assert!(sorted(&driver).contains(&"E4001"), "{:?}", sorted(&driver));
    let trust = format!(
        "{TRUST}module M {{\n    in  key : u8 @Secure\n    in  s : u2 @Debug\n    out dbg : u8 @Debug\n    comb {{\n        {}\n    }}\n}}\n",
        live.replace("X", "dbg = 1").replace("Y", "dbg = key")
    );
    assert!(
        trust_codes(&trust).contains(&"E3009"),
        "{:?}",
        sorted(&trust)
    );
    let timing = format!(
        "@strict_timing\nmodule M {{\n    in  clk : clock\n    in  x : u32\n    in  s : u2\n    out y : u32\n    reg a : Delayed<u32, 1> = 0\n    reg b : Delayed<u32, 2> = 0\n    on clk {{\n        a <= x\n        {}\n    }}\n    y = b\n}}\n",
        live.replace("X", "b <= a").replace("Y", "b <= x")
    );
    assert_eq!(e5010(&timing), 1);
    let handshake = format!(
        "module Producer {{\n    in  clk : clock\n    in  have : bool\n    in  s : u2\n    out tx : Handshake<u8>\n    reg count : u8 = 0\n    on clk {{\n        if tx.fired {{\n            count <= count + 1\n        }}\n    }}\n    comb {{\n        {}\n    }}\n    tx.data = count\n}}\n",
        live.replace("X", "tx.valid = have")
            .replace("Y", "tx.valid = tx.ready")
    );
    assert!(
        sorted(&handshake).contains(&"E4007"),
        "{:?}",
        sorted(&handshake)
    );
}
