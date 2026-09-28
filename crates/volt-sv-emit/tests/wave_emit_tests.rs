//! Dalga formunda enum ve Trit adları (ADR-0092): emit'in kaydettiği
//! sinyaller ve çeviri tabloları. Tablolar enum KODLAMASIYLA birebirdir
//! (ADR-0074 `enum_layout`): açık değerli, taban tipli ve `match`'in son
//! kolu `default` olan enum dahil.

use volt_span::FileId;
use volt_sv_emit::{emit_full, filter_text, SvaMode, WaveInfo};

fn waves(src: &str) -> WaveInfo {
    let parsed = volt_syntax::parser::parse(FileId(0), src);
    assert!(
        parsed.diagnostics.is_empty(),
        "parse hatasız olmalı: {:?}",
        parsed.error_codes()
    );
    let out = emit_full(&parsed.ast, "test.volt", src, SvaMode::None);
    assert!(
        out.diagnostics.is_empty(),
        "emit hatasız olmalı: {:?}",
        out.diagnostics
            .iter()
            .map(|d| format!("{} {}", d.code.as_str(), d.message))
            .collect::<Vec<_>>()
    );
    out.waves
}

/// Tablonun (kod, ad) satırları.
fn table(info: &WaveInfo, name: &str) -> Vec<(u128, String)> {
    info.tables
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("{name} tablosu yok: {info:?}"))
        .entries
        .clone()
}

fn rows(entries: &[(u128, &str)]) -> Vec<(u128, String)> {
    entries.iter().map(|&(c, n)| (c, n.to_string())).collect()
}

const HIERARCHY: &str = "\
enum Phase { Idle, Run, Done }

struct Req {
    kind : Phase
    last : bool
}

module Sub {
    in  clk : clock
    in  go  : bool
    out ph  : Phase
    out tv  : Trit

    reg st : Phase = Phase::Idle
    reg t  : Trit  = 0

    on clk {
        match st {
            Phase::Idle => { if go { st <= Phase::Run } }
            Phase::Run  => { st <= Phase::Done }
            Phase::Done => { st <= Phase::Idle }
        }
        t <= -t
    }
    ph = st
    tv = t
}

module Top {
    in  clk  : clock
    in  go   : bool
    in  req  : Req
    out busy : bool

    let u = Sub { clk: clk, go: go }
    reg hold : Req = Req { kind: Phase::Idle, last: false }
    let now : Phase = u.ph

    on clk {
        hold.kind <= req.kind
        hold.last <= req.last
    }
    busy = now != Phase::Idle || hold.kind != Phase::Idle
}
";

#[test]
fn records_ports_struct_fields_instance_outputs_and_lets() {
    let info = waves(HIERARCHY);
    let session = info.session("Top", "TOP.Top").expect("iz var");
    let paths: Vec<(&str, &str)> = session
        .traces
        .iter()
        .map(|t| (t.path.as_str(), t.table.as_str()))
        .collect();
    assert_eq!(
        paths,
        [
            // Struct portu yaprak porta, struct register'ı yaprak register'a
            // indirgenir (ADR-0077): enum alanı kendi adıyla görünür.
            ("TOP.Top.req_kind[1:0]", "Phase"),
            ("TOP.Top.hold_kind[1:0]", "Phase"),
            ("TOP.Top.now[1:0]", "Phase"),
            // Örnek çıkış telleri `<örnek>_<port>` (ADR-0090 adları).
            ("TOP.Top.u_ph[1:0]", "Phase"),
            ("TOP.Top.u_tv[1:0]", "Trit"),
            // Alt örneğin kendi sinyalleri.
            ("TOP.Top.u.ph[1:0]", "Phase"),
            ("TOP.Top.u.tv[1:0]", "Trit"),
            ("TOP.Top.u.st[1:0]", "Phase"),
            ("TOP.Top.u.t[1:0]", "Trit"),
        ]
    );
    // Formal karşı örneğinde üst kapsam modülün kendisidir.
    let formal = info.session("Top", "Top").expect("iz var");
    assert_eq!(formal.traces[0].path, "Top.req_kind[1:0]");
}

#[test]
fn implicit_codes_count_up_from_zero() {
    let info = waves(HIERARCHY);
    assert_eq!(
        table(&info, "Phase"),
        rows(&[(0, "Idle"), (1, "Run"), (2, "Done")])
    );
    let text = filter_text(
        info.tables
            .iter()
            .find(|t| t.name == "Phase")
            .expect("tablo"),
    );
    assert!(
        text.ends_with("00 Idle\n01 Run\n10 Done\n11 ?red?invalid 3\n"),
        "{text}"
    );
}

#[test]
fn trit_table_is_the_signed_two_bit_encoding() {
    let info = waves(HIERARCHY);
    assert_eq!(
        table(&info, "Trit"),
        rows(&[(0b00, "0"), (0b01, "+1"), (0b11, "-1")])
    );
}

/// Açık değerler ve taban tipi: tablo bildirim sırasına değil koda göre
/// sıralı, genişlik taban tipinden (u7).
#[test]
fn explicit_values_with_a_base_type_follow_the_encoding() {
    let src = "\
enum Opcode : u7 { Store = 0b0100011, Load = 0b0000011, Alu = 0b0110011 }

module OpReg {
    in  clk  : clock
    in  f    : u7
    out code : u7

    reg op_r : Opcode = Opcode::Load

    on clk {
        match f {
            3  => { op_r <= Opcode::Load }
            35 => { op_r <= Opcode::Store }
            _  => { op_r <= Opcode::Alu }
        }
    }
    code = op_r as u7
}
";
    let info = waves(src);
    assert_eq!(
        table(&info, "Opcode"),
        rows(&[(3, "Load"), (35, "Store"), (51, "Alu")])
    );
    let op = &info.tables[0];
    assert_eq!(op.width, 7);
    let text = filter_text(op);
    // 7 bit: 128 kodun hepsi (3 varyant + 125 geçersiz) + başlık.
    assert_eq!(text.lines().count(), 129);
    assert!(text.contains("\n0000011 Load\n"), "{text}");
    assert!(text.contains("\n0100011 Store\n"), "{text}");
    assert!(text.contains("\n0000000 ?red?invalid 0\n"), "{text}");
    let session = info.session("OpReg", "TOP.OpReg").expect("iz var");
    assert_eq!(session.traces[0].path, "TOP.OpReg.op_r[6:0]");
}

/// `match`'in son kolu SV'de `default` olur (ADR-0083 Karar 4); tablo
/// kodlamadan gelir, `case` biçiminden etkilenmez.
#[test]
fn last_arm_default_does_not_change_the_table() {
    let src = "\
enum Mode { Off = 2, Slow = 5, Fast = 7 }

module Fan {
    in  clk : clock
    in  up  : bool
    out spin : bool

    reg m : Mode = Mode::Off

    on clk {
        if up {
            match m {
                Mode::Off  => { m <= Mode::Slow }
                Mode::Slow => { m <= Mode::Fast }
                Mode::Fast => { m <= Mode::Off }
            }
        }
    }
    spin = m != Mode::Off
}
";
    let info = waves(src);
    assert_eq!(
        table(&info, "Mode"),
        rows(&[(2, "Off"), (5, "Slow"), (7, "Fast")])
    );
    let text = filter_text(&info.tables[0]);
    assert!(
        text.ends_with(
            "000 ?red?invalid 0\n001 ?red?invalid 1\n010 Off\n011 ?red?invalid 3\n\
             100 ?red?invalid 4\n101 Slow\n110 ?red?invalid 6\n111 Fast\n"
        ),
        "{text}"
    );
}

/// Tek bitlik enum (iki varyant): GTKWave adı aralıksızdır.
#[test]
fn one_bit_enum_has_no_range_suffix() {
    let src = "\
enum Dir { Rx, Tx }

module Pin {
    in  clk : clock
    in  t   : bool
    out d   : Dir

    reg r : Dir = Dir::Rx
    on clk {
        if t { r <= Dir::Tx }
    }
    d = r
}
";
    let info = waves(src);
    let session = info.session("Pin", "TOP.Pin").expect("iz var");
    let paths: Vec<&str> = session.traces.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, ["TOP.Pin.d", "TOP.Pin.r"]);
}

/// Enum ve Trit kullanmayan tasarım: oturum yok (sürücü dosya üretmez).
#[test]
fn design_without_enums_or_trits_has_no_session() {
    let src = "\
module Counter {
    in  clk   : clock
    out count : u8

    reg c : u8 = 0
    on clk { c <= c + 1 }
    count = c
}
";
    let info = waves(src);
    assert!(info.tables.is_empty(), "{info:?}");
    assert!(info.session("Counter", "TOP.Counter").is_none());
}

/// Trit dizisi paketlenmiş vektördür (ADR-0056): eleman başına ad yok,
/// dizi oturuma girmez.
#[test]
fn trit_arrays_are_not_translated() {
    let src = "\
module Weights {
    in  w   : [Trit; 4]
    in  one : Trit
    out o   : i8

    o = one * 3
}
";
    let info = waves(src);
    let session = info.session("Weights", "TOP.Weights").expect("iz var");
    let paths: Vec<&str> = session.traces.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, ["TOP.Weights.one[1:0]"]);
}
