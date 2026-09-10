//! L1 zamanlama denetimi testleri (ADR-0037, timing.rs).
//!
//! Kapsam: `@strict_timing` altında E5010 üretimi ve muafiyetler
//! (literal/sabit, koşul, tutma yazımı, açık anotasyon iddiası,
//! geri besleme döngüleri), `delay<K>` yeniden hizalama, çıkarım
//! zinciri ve `@strict_timing` olmayan modüllerin sessizliği.

use volt_hir::analyze;
use volt_syntax::{parse, FileId};

/// Kaynağı ayrıştırır, analiz eder, E5010 sayısını döndürür.
fn e5010_count(src: &str) -> usize {
    let parsed = parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hataları: {:?}",
        parsed.error_codes()
    );
    analyze(&parsed.ast)
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str() == "E5010")
        .count()
}

const TWO_STAGE_MISMATCH_BODY: &str = "
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = a + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
";

#[test]
fn two_stage_regs_mismatch_is_e5010() {
    let src = format!("@strict_timing\nmodule M {{ {TWO_STAGE_MISMATCH_BODY}");
    assert_eq!(e5010_count(&src), 1);
}

#[test]
fn non_strict_module_stays_silent() {
    // Geriye uyumluluk: @strict_timing yoksa aynı kaynak sessizdir.
    let src = format!("module M {{ {TWO_STAGE_MISMATCH_BODY}");
    assert_eq!(e5010_count(&src), 0);
}

#[test]
fn strictness_is_per_module() {
    // Katı modüldeki hata, katı olmayan komşuyu etkilemez ve tersi.
    let strict = format!("@strict_timing\nmodule S {{ {TWO_STAGE_MISMATCH_BODY}");
    let loose = format!("module L {{ {TWO_STAGE_MISMATCH_BODY}");
    assert_eq!(e5010_count(&format!("{strict}\n{loose}")), 1);
}

#[test]
fn equal_delays_combine_clean() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 1> = 0

    let sum = a + b

    on clk {
        a <= x
        b <= x
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn delay_k_realigns_clean() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = delay<1>(a) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn nested_delay_accumulates() {
    // delay<1>(delay<1>(x)): 0 → 2 çevrim; 2 çevrimlik register'la uyumlu.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = delay<1>(delay<1>(x)) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn delay_on_literal_pins_it() {
    // Zamanlamasız değere delay<K> sabitler: delay<2>(1) tam 2 çevrimdir.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = delay<2>(1) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn literal_operand_is_exempt() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0

    let sum = a + 5

    on clk { a <= x }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn const_operand_is_exempt() {
    assert_eq!(
        e5010_count(
            "const BIAS : u32 = 7

@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0

    let sum = a + BIAS

    on clk { a <= x }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn undelayed_port_mix_is_e5010() {
    // Giriş portu tanım gereği 0 çevrimdir (ADR-0037 §3).
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg tmp : Delayed<u32, 1> = 0
    reg b   : Delayed<u32, 2> = 0

    let sum = x + b

    on clk {
        tmp <= x
        b   <= tmp
    }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn annotated_reg_write_mismatch_is_e5010() {
    // Delayed<_, 2> register'ı 0 çevrimlik porttan yazılamaz: register
    // bir çevrim ekler, kaynak 1 çevrim olmalıydı.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg b : Delayed<u32, 2> = 0

    on clk { b <= x }

    y = b
}
"
        ),
        1
    );
}

#[test]
fn annotated_reg_chain_clean() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0
    reg s3 : Delayed<u32, 3> = 0

    on clk {
        s1 <= x
        s2 <= s1
        s3 <= s2
    }

    y = s3
}
"
        ),
        0
    );
}

#[test]
fn reg_hold_write_is_exempt() {
    // `s2 <= s2` tutma yazımıdır (stall deseni) — gecikme denklemine
    // katılmaz ve hata üretmez.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  en  : bool
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    on clk {
        s1 <= x
        if en {
            s2 <= s1
        } else {
            s2 <= s2
        }
    }

    y = s2
}
"
        ),
        0
    );
}

#[test]
fn annotated_let_is_retiming_assertion() {
    // Açık anotasyonlu let, sonucun gecikmesini BİLDİRİR; başlatıcısı
    // aşamaları karıştırabilir (forwarding kaçış kapısı).
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let fwd : Delayed<u32, 1> = if s2 != 0 { s2 } else { s1 }

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = fwd
}
"
        ),
        0
    );
}

#[test]
fn inference_propagates_through_unannotated_defs() {
    // Anotasyonsuz let/reg gecikmeyi taşır: d = s1 (1), t = d + 1 çevrim
    // (2), t + s2 (2) uyumludur.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let d = s1
    reg t : u32 = 0

    let sum = t + s2

    on clk {
        s1 <= x
        s2 <= s1
        t  <= d
    }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn unannotated_reg_inferred_delay_mismatch_is_e5010() {
    // t, s1'den yazılır → 2 çevrim çıkarılır; s1 (1) ile karışamaz.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg t : u32 = 0

    let sum = t + s1

    on clk {
        s1 <= x
        t  <= s1
    }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn feedback_counter_is_unconstrained() {
    // Geri beslemeli register (sayaç) sabit gecikmeye oturmaz → serbest.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg cnt : u32 = 0

    let sum = cnt + x

    on clk { cnt <= cnt + 1 }

    y = sum
}
"
        ),
        0
    );
}

#[test]
fn if_branch_mismatch_is_e5010() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  sel : bool
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let v = if sel { s1 } else { s2 }

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = v
}
"
        ),
        1
    );
}

#[test]
fn if_condition_is_exempt() {
    // Koşul kontrol sinyalidir: 0 çevrimlik `sel`, 2 çevrimlik dallarla
    // aynı ifadede yaşayabilir (ADR-0037 §2).
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  sel : bool
    out y   : u32

    reg s2 : Delayed<u32, 2> = 0
    reg s2b : Delayed<u32, 2> = 0

    let v = if sel { s2 } else { s2b }

    on clk {
        s2  <= delay<1>(x)
        s2b <= delay<1>(x)
    }

    y = v
}
"
        ),
        0
    );
}

#[test]
fn annotated_array_reg_index_mix_is_e5010() {
    // 3 çevrimlik dizinin 0 çevrimlik indeksle okunması karışımdır;
    // bilinçliyse okuma açık anotasyonlu bir let'e alınır.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  i   : u2
    out y   : u32

    reg arr : Delayed<[u32; 4], 3> = [0; 4]

    let v = arr[i]

    y = v
}
"
        ),
        1
    );
}

#[test]
fn delayed_cycles_must_be_int_literal() {
    // V0 sınırı: Delayed<_, N> içindeki N tamsayı literali olmalı.
    assert_eq!(
        e5010_count(
            "const K : u32 = 2

@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg b : Delayed<u32, K> = 0

    on clk { b <= x }

    y = b
}
"
        ),
        1
    );
}

#[test]
fn delay_k_const_name_is_rejected() {
    // V0 sınırı: delay<K> içindeki K tamsayı literali olmalı.
    assert_eq!(
        e5010_count(
            "const K : u32 = 1

@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg b : Delayed<u32, 2> = 0

    let sum = delay<K>(x) + b

    on clk { b <= delay<1>(x) }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn output_port_assign_mismatch_is_e5010() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = s1 + s2
}
"
        ),
        1
    );
}

#[test]
fn unary_preserves_delay() {
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    out y   : u32

    reg a : Delayed<u32, 1> = 0
    reg b : Delayed<u32, 2> = 0

    let sum = (~a) + b

    on clk {
        a <= x
        b <= a
    }

    y = sum
}
"
        ),
        1
    );
}

#[test]
fn annotated_input_port_overrides_default() {
    // Giriş portu açıkça Delayed<_, 2> bildirilebilir (dış boru hattından
    // gelen veri): 2 çevrimlik register'la doğrudan birleşir.
    assert_eq!(
        e5010_count(
            "@strict_timing
module M {
    in  clk : clock
    in  x   : u32
    in  late : Delayed<u32, 2>
    out y   : u32

    reg s1 : Delayed<u32, 1> = 0
    reg s2 : Delayed<u32, 2> = 0

    let sum = late + s2

    on clk {
        s1 <= x
        s2 <= s1
    }

    y = sum
}
"
        ),
        0
    );
}
