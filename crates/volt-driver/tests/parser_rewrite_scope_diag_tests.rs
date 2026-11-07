//! Ayrıştırıcının ad eşleyen öteki adımları kapsam zincirinden geçer
//! (#136, #137, #138): tanılar. Bundle dizisinin indeksi ve uzunluğu
//! (ADR-0056) kullanıldığı yerin kapsamında değerlendirilir: aynı adlı
//! port, modül `let`'i, blok `let`'i ya da döngü değişkeni kök const'u
//! gölgeler ve sabit değildir (E2008, adı taze olan girdi gibi). Bundle
//! portunu, çift yönlü portu ya da `regs` tutamacını gölgeleyen yerelin
//! kullanımı yeniden yazılmaz: tanı, adı taze olan aynı girdinin tanısıdır.
//! Açılımda kaybolan adı (bundle portu, `regs`) gölgeleyen yerel W1002
//! alır (name-resolution.md §6), kaynak satırı başına bir kez.
//!
//! Değerleri (simülasyon) `parser_rewrite_scope_sim_tests` sınar; bu dosya
//! tanıları ve derlemenin başarısını sınar, SV metnini karşılaştırmaz.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-rwdiag-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dizini");
    dir
}

/// `volt check --format json`; (önem, kod, birincil satır) kaynak sırasıyla.
fn check(tag: &str, src: &str) -> Vec<(String, String, u64)> {
    let dir = temp_dir(tag);
    let file = dir.join("case.volt");
    std::fs::write(&file, src).expect("yaz");
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "check", "--format", "json"])
        .arg(&file)
        .current_dir(&dir)
        .output()
        .expect("volt çalışmalı");
    let _ = std::fs::remove_dir_all(&dir);
    let env: Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("JSON değil ({e}): {}", String::from_utf8_lossy(&out.stderr)));
    let mut v: Vec<(String, String, u64)> = env["diagnostics"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|d| {
            let line = d["spans"]
                .as_array()
                .and_then(|s| s.iter().find(|s| s["primary"] == true))
                .and_then(|s| s["start"]["line"].as_u64())
                .unwrap_or(0);
            (
                d["severity"].as_str().unwrap_or_default().to_string(),
                d["code"].as_str().unwrap_or_default().to_string(),
                line,
            )
        })
        .collect();
    v.sort();
    v
}

fn errors(diags: &[(String, String, u64)]) -> Vec<(String, u64)> {
    diags
        .iter()
        .filter(|(s, _, _)| s == "error")
        .map(|(_, c, l)| (c.clone(), *l))
        .collect()
}

fn lines_of(diags: &[(String, String, u64)], code: &str) -> Vec<u64> {
    diags
        .iter()
        .filter(|(_, c, _)| c == code)
        .map(|(_, _, l)| *l)
        .collect()
}

/// `marker` içeren satırların numaraları (1'den).
fn lines_with(src: &str, marker: &str) -> Vec<u64> {
    src.lines()
        .enumerate()
        .filter(|(_, l)| l.contains(marker))
        .map(|(i, _)| i as u64 + 1)
        .collect()
}

/// Gölgeleyen girdinin hataları, gölgeleyen adı taze bir adla değiştirilmiş
/// aynı girdinin hatalarıyla aynı ve boş değil (§6: gölgeleme yalnız bir
/// uyarıdır; kullanım iç bildirime bağlanır). Gölgelemede W1002 tam olarak
/// `// shadows` işaretli satırlardadır, taze adlı girdide W1002 yoktur.
fn same_errors_as_fresh(tag: &str, shadowing: &str, fresh: &str) {
    let got = check(&format!("{tag}-s"), shadowing);
    let want = check(&format!("{tag}-f"), fresh);
    assert!(
        !errors(&want).is_empty(),
        "taze adlı girdi hata vermeli: {want:?}"
    );
    assert_eq!(
        errors(&got),
        errors(&want),
        "gölgeli: {got:?}\ntaze: {want:?}"
    );
    assert_eq!(
        lines_of(&got, "W1002"),
        lines_with(shadowing, "// shadows"),
        "{got:?}"
    );
    assert!(lines_of(&want, "W1002").is_empty(), "{want:?}");
}

// ═══ #136: bundle dizisinin indeksi ve uzunluğu ═════════════════════════

/// Kök `const K` ile aynı adı taşıyan iç bildirim indeksi sabit olmaktan
/// çıkarır: E2008, adı taze olan girdideki gibi indeksin satırında. Beş
/// gölgeleyen: modül `let`'i, port, blok `let`'i, `comb` içi `for`, `on`
/// bloğunda `if` içindeki `for`.
#[test]
fn a_name_that_shadows_a_root_const_is_not_a_constant_bundle_index() {
    let head = "const K : u32 = 0\nmodule M {\n    in  clk : clock\n    in  c   : bool\n    in  rx  : [Handshake<u8>; 2]\n    out y   : u8\n    reg r : u8 = 0\n    rx[0].ready = true\n    rx[1].ready = true\n";
    let cases: [(&str, &str); 5] = [
        (
            "modlet",
            "    let K : u32 = 1 // shadows\n    y = rx[K].data // index\n}\n",
        ),
        (
            "port",
            "    in  K : u1 // shadows\n    y = rx[K].data // index\n}\n",
        ),
        (
            "blocklet",
            "    comb {\n        let K : u32 = 1 // shadows\n        y = rx[K].data // index\n    }\n}\n",
        ),
        (
            "combfor",
            "    y = 0\n    comb {\n        for K in 0..2 { // shadows\n            rx[K].ready = true // index\n        }\n    }\n}\n",
        ),
        (
            "innerfor",
            "    y = r\n    on clk {\n        if c {\n            for K in 0..2 { // shadows\n                r <= rx[K].data // index\n            }\n        }\n    }\n}\n",
        ),
    ];
    for (tag, body) in cases {
        let src = format!("{head}{body}");
        // Taze ad: bildirim ve kullanım Q olur; kök K kullanılmaz.
        let fresh = src
            .replace("let K ", "let Q ")
            .replace("in  K ", "in  Q ")
            .replace("for K ", "for Q ")
            .replace("rx[K]", "rx[Q]");
        let got = check(&format!("idx-{tag}"), &src);
        let want = check(&format!("idx-{tag}-f"), &fresh);
        let index = lines_with(&src, "// index");
        let e2008: Vec<(String, u64)> = index.iter().map(|&l| ("E2008".to_string(), l)).collect();
        assert_eq!(errors(&got), e2008, "{tag}: {got:?}");
        assert_eq!(errors(&want), e2008, "{tag} (taze): {want:?}");
    }
}

/// Port `K` kök `const K`'yi bundle dizisinin uzunluğunda da gölgeler:
/// uzunluk sabit değildir, E2008 (taze adla aynı).
#[test]
fn a_port_that_shadows_a_root_const_is_not_a_constant_bundle_array_length() {
    let src = "const K : u32 = 2
module M {
    in  K  : u8 // shadows
    in  rx : [Handshake<u8>; K] // length
    out y  : u8
    y = 0
}
";
    let got = check("len", src);
    let want = check(
        "len-f",
        &src.replace("in  K ", "in  Q ").replace("; K]", "; Q]"),
    );
    let len = lines_with(src, "// length")[0];
    assert_eq!(errors(&got), vec![("E2008".to_string(), len)], "{got:?}");
    assert_eq!(errors(&want), errors(&got), "{want:?}");
}

/// Bekçi: modül `let K`'si indeksten SONRA bildirilir; indeksin yerinde
/// görünür olan kök `const K`'dir (§4, §5; çözücü de böyle bağlar). Dizi
/// kök const'larla kurulur ve indekslenir: hatasız derlenir.
#[test]
fn a_module_let_declared_after_the_index_does_not_shadow_it() {
    let src = "const K : u32 = 1
const N : u32 = 2
module M {
    in  rx : [Handshake<u8>; N]
    out y  : u8
    out z  : u8
    rx[0].ready = true
    rx[1].ready = true
    y = rx[K].data
    let K : u8 = 7
    z = K
}
";
    let got = check("after", src);
    assert!(errors(&got).is_empty(), "{got:?}");
}

// ═══ #137: bundle alanı, çift yönlü port, @mmio tutamacı ════════════════

const REQ: &str = "struct port Req {
    out addr  : u8
    in  ack   : bool
}
";

/// Bundle portunu gölgeleyen yerelin alan erişimi yerelindir: `u8` blok
/// `let`'inin, `bool` blok `let`'inin (atama hedefi), `comb` içi döngü
/// değişkeninin ve `[u8; 2]` blok `let`'inin alanı yoktur (E2003, taze
/// adla aynı).
#[test]
fn a_local_that_shadows_a_bundle_port_has_no_port_fields() {
    let cases: [(&str, String); 4] = [
        (
            "let_read",
            format!(
                "{REQ}module M {{
    in  clk : clock
    in  aw  : Req
    out y   : u8
    reg r : u8 = 0
    aw.ack = true
    on clk {{
        let aw : u8 = 3 // shadows
        r <= aw.addr
    }}
    y = r
}}
"
            ),
        ),
        (
            "let_write",
            format!(
                "{REQ}module M {{
    in  aw  : Req
    out y   : u8
    out z   : bool
    aw.ack = true
    comb {{
        let aw : bool = false // shadows
        aw.ack = true
        z = aw
    }}
    y = 0
}}
"
            ),
        ),
        (
            "comb_for",
            format!(
                "{REQ}module M {{
    in  aw  : Req
    out y   : [u8; 2]
    aw.ack = true
    comb {{
        for aw in 0..2 {{ // shadows
            y[aw] = aw.addr
        }}
    }}
}}
"
            ),
        ),
        (
            "array",
            format!(
                "{REQ}module M {{
    in  clk : clock
    in  rx  : [Req; 2]
    in  s   : u8
    out y   : u8
    reg r : u8 = 0
    rx[0].ack = true
    rx[1].ack = true
    on clk {{
        let rx : [u8; 2] = [s, s] // shadows
        r <= rx[1].addr
    }}
    y = r
}}
"
            ),
        ),
    ];
    for (tag, src) in cases {
        let local = if tag == "array" { "rx" } else { "aw" };
        let fresh = fresh_in_block(&src, local, "zz");
        same_errors_as_fresh(&format!("field-{tag}"), &src, &fresh);
    }
}

/// `inout dq`'yu gölgeleyen yerel: `dq.drive(w)` blok `let`'inde ve `for`
/// değişkeninde bir deyim değildir (E0001), `dq.read()` yerelin yöntemi
/// değildir (E0003); taze adla aynı.
#[test]
fn a_local_that_shadows_an_inout_port_has_no_port_methods() {
    let head = "module M {
    in  clk : clock
    in  w   : bits<8>
    inout dq : bits<8>
    out y   : bits<8>
    reg r : bits<8> = 0 as bits<8>
    on clk {
";
    let cases: [(&str, &str); 3] = [
        (
            "drive_let",
            "        let dq : bits<8> = w // shadows\n        dq.drive(w)\n        r <= dq\n",
        ),
        (
            "read_let",
            "        dq.drive(w)\n        let dq : bits<8> = w // shadows\n        r <= dq.read()\n",
        ),
        (
            "drive_for",
            "        for dq in 0..2 { // shadows\n            dq.drive(w)\n        }\n        r <= w\n",
        ),
    ];
    for (tag, body) in cases {
        let src = format!("{head}{body}    }}\n    y = r\n}}\n");
        let fresh = fresh_in_block(&src, "dq", "dz");
        let got = check(&format!("bidir-{tag}"), &src);
        let want = check(&format!("bidir-{tag}-f"), &fresh);
        assert!(!errors(&want).is_empty(), "{tag}: {want:?}");
        assert_eq!(errors(&got), errors(&want), "{tag}: {got:?}\n{want:?}");
    }
}

/// `regs` tutamacını gölgeleyen yerel: yerelin alanına `<=` yazmak E4001'dir
/// (taze adla aynı), bus'a ait alana RTL yazması (E4006) değil; `comb`
/// içi döngü değişkeninin alanı yoktur (E2003).
#[test]
fn a_local_that_shadows_the_register_handle_is_not_the_register_map() {
    let regs = "struct Ctl {
    enable : bool
    other  : bool
}
struct Regs {
    control : Ctl
    pad     : bool
}
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module G {
    in  clk : clock
    in  a   : bool
    out en  : [bool; 2]
    @reg(offset = 0x00, access = ReadWrite)
    control : {
        enable : bool,
        @reserved : bits<31>,
    }
";
    let write = format!(
        "{regs}    reg r : bool = false
    on clk {{
        let regs : Regs = Regs {{ control: Ctl {{ enable: a, other: a }}, pad: a }} // shadows
        regs.control.enable <= a
        r <= regs.control.enable
    }}
    en[0] = r
    en[1] = r
}}
"
    );
    let comb_for = format!(
        "{regs}    comb {{
        for regs in 0..2 {{ // shadows
            en[regs] = regs.control.enable
        }}
    }}
}}
"
    );
    same_errors_as_fresh("mmio-write", &write, &fresh_in_block(&write, "regs", "rg"));
    same_errors_as_fresh(
        "mmio-for",
        &comb_for,
        &fresh_in_block(&comb_for, "regs", "rg"),
    );
}

/// W1002 (§6): açılımda kaybolan adı (bundle portu, `regs` tutamacı)
/// gölgeleyen yerel uyarı alır; çözücü bu adları görmediği için uyarıyı
/// adım verir. Generic modülün iki monomorfunda kaynak satırı başına bir
/// uyarı; gölgelemeyen yerel ve kullanım uyarı almaz. Örnekleme bundle
/// portunu düz adıyla bağlar (ADR-0039).
#[test]
fn a_local_that_shadows_a_vanished_name_is_warned_once() {
    let src = format!(
        "{REQ}struct Pair {{
    addr : u8
    ack  : bool
}}
struct Ctl {{
    enable : bool
    other  : bool
}}
struct Regs {{
    control : Ctl
    pad     : bool
}}
module W<const N: u32> {{
    in  clk : clock
    in  aw  : Req
    in  s   : u8
    out y   : u8
    reg r : u8 = 0
    aw.ack = true
    on clk {{
        let aw : Pair = Pair {{ addr: s + N, ack: false }} // shadows
        let other : u8 = aw.addr
        r <= other
    }}
    y = r
}}
@mmio(base = 0x4000_0000, bus = AXI4Lite)
module G {{
    in  clk : clock
    in  a   : bool
    out en  : bool
    @reg(offset = 0x00, access = ReadWrite)
    control : {{
        enable : bool,
        @reserved : bits<31>,
    }}
    reg q : bool = false
    on clk {{
        let regs : Regs = Regs {{ control: Ctl {{ enable: a, other: a }}, pad: a }} // shadows
        q <= regs.control.enable
    }}
    en = q
}}
module Top {{
    in  clk : clock
    in  s   : u8
    out y1  : u8
    out y2  : u8
    let w1 = W<1> {{ clk: clk, aw_addr: s, s: s }}
    let w2 = W<2> {{ clk: clk, aw_addr: s, s: s }}
    y1 = w1.y
    y2 = w2.y
}}
"
    );
    let got = check("w1002", &src);
    assert!(errors(&got).is_empty(), "{got:?}");
    assert_eq!(
        lines_of(&got, "W1002"),
        lines_with(&src, "// shadows"),
        "{got:?}"
    );
}

/// Taze ad: `local` adının `// shadows` işaretli satırdan sonraki, işaretli
/// satırın bloğu kapanana kadarki geçişleri (bildirim dahil) `fresh` olur.
/// Blok, işaretli satırın girintisinden daha az girintili ilk `}` ile
/// kapanır.
fn fresh_in_block(src: &str, local: &str, fresh: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains("// shadows"))
        .expect("işaretli satır");
    let indent = |l: &str| l.len() - l.trim_start().len();
    let depth = indent(lines[start]);
    let end = (start + 1..lines.len())
        .find(|&i| lines[i].trim_start().starts_with('}') && indent(lines[i]) < depth)
        .unwrap_or(lines.len());
    let rename = |l: &str| {
        let mut out = String::new();
        let mut rest = l;
        while let Some(p) = rest.find(local) {
            let before = rest[..p].chars().next_back();
            let after = rest[p + local.len()..].chars().next();
            let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            out.push_str(&rest[..p]);
            out.push_str(if ident(before) || ident(after) {
                local
            } else {
                fresh
            });
            rest = &rest[p + local.len()..];
        }
        out.push_str(rest);
        out
    };
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        let l = if (start..end).contains(&i) {
            rename(l).replace("// shadows", "")
        } else {
            (*l).to_string()
        };
        out.push_str(&l);
        out.push('\n');
    }
    out
}
