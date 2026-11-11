//! Yerleşik primitif özetlerinin makine denetimi (ADR-0114 K7).
//!
//! Yerleşik primitifler Volt'ta değil, sv-emit'in şablonlarında yazılıdır;
//! özetleri `BuiltinPrim::comb_inputs` tablosundadır. Tablonun her satırı
//! burada primitifin ÜRETTİĞİ SystemVerilog'a karşı denetlenir: her
//! primitif bütün portları dışarı açılmış bir Volt modülünde örneklenir,
//! `volt build` ile SV üretilir, Yosys onu netlist'e çevirir (`proc;
//! flatten; memory`) ve çıkış portu bitlerinden geriye, flip-flop
//! hücrelerinde durarak, hangi giriş portlarına kombinasyonel yol olduğu
//! hesaplanır. Sonuç tabloyla birebir aynı olmalı: eksik satır E4007'yi
//! kaçırır, fazla satır yanlış alarm verir.
//!
//! Yosys ister (`tools::require`, ADR-0079/0107): araçlı CI işinde koşar.

mod tools;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tools::Tool;
use volt_ast::builtin::{BuiltinPrim, DomainRole, PortKind};
use volt_ast::PortDir;

/// Bütün yerleşik primitifler. Eşleme bilerek kapsamlıdır: yeni bir
/// primitif eklenince bu test derlenmez ve listeye girmesi gerekir.
fn all_prims() -> Vec<BuiltinPrim> {
    let all = [
        BuiltinPrim::AsyncFifo,
        BuiltinPrim::HandshakeSync,
        BuiltinPrim::PulseSync,
        BuiltinPrim::SyncFifo,
        BuiltinPrim::Ram,
        BuiltinPrim::DualPortRam,
        BuiltinPrim::Counter,
        BuiltinPrim::ShiftRegister,
        BuiltinPrim::RoundRobinArbiter,
        BuiltinPrim::PriorityArbiter,
        BuiltinPrim::EdgeDetect,
        BuiltinPrim::AsyncDualPortRam,
    ];
    for p in all {
        match p {
            BuiltinPrim::AsyncFifo
            | BuiltinPrim::HandshakeSync
            | BuiltinPrim::PulseSync
            | BuiltinPrim::SyncFifo
            | BuiltinPrim::Ram
            | BuiltinPrim::DualPortRam
            | BuiltinPrim::Counter
            | BuiltinPrim::ShiftRegister
            | BuiltinPrim::RoundRobinArbiter
            | BuiltinPrim::PriorityArbiter
            | BuiltinPrim::EdgeDetect
            | BuiltinPrim::AsyncDualPortRam => {}
        }
    }
    all.to_vec()
}

/// Örnekleme parametreleri: T = u8, DEPTH = 4 (adres 2 bit), WIDTH /
/// LEN / N = 2.
const DEPTH: u32 = 4;
const DIM: u32 = 2;

fn generics(p: BuiltinPrim) -> String {
    let mut args = Vec::new();
    if p.type_arg_count() == 1 {
        args.push("u8".to_string());
    }
    if p.const_arg_count() == 1 {
        let v = match p {
            BuiltinPrim::AsyncFifo
            | BuiltinPrim::SyncFifo
            | BuiltinPrim::Ram
            | BuiltinPrim::DualPortRam
            | BuiltinPrim::AsyncDualPortRam => DEPTH,
            _ => DIM,
        };
        args.push(v.to_string());
    }
    if args.is_empty() {
        String::new()
    } else {
        format!("<{}>", args.join(", "))
    }
}

fn port_type(kind: PortKind) -> String {
    match kind {
        PortKind::Clock => "clock".to_string(),
        PortKind::Data => "u8".to_string(),
        PortKind::Bool => "bool".to_string(),
        PortKind::Addr => "bits<2>".to_string(),
        PortKind::Dim => format!("bits<{DIM}>"),
        PortKind::Taps => format!("bits<{}>", DIM * 8),
    }
}

/// Primitifin bütün portlarını dışarı açan Volt modülü `Wrap`.
fn wrapper(p: BuiltinPrim) -> String {
    let two = p.has_dst_clock();
    let mut src = String::new();
    if two {
        src.push_str(
            "domain Src {\n    clock = posedge\n    reset = sync active_high\n}\n\n\
             domain Dst {\n    clock = posedge\n    reset = sync active_high\n}\n\n",
        );
    }
    src.push_str("module Wrap {\n");
    for port in p.ports() {
        let dir = if port.dir == PortDir::In {
            "in "
        } else {
            "out"
        };
        let ann = match (two, port.role) {
            (false, _) => "",
            (true, DomainRole::Src) => " @Src",
            (true, DomainRole::Dst) => " @Dst",
        };
        src.push_str(&format!(
            "    {dir} {} : {}{ann}\n",
            port.name,
            port_type(port.kind)
        ));
    }
    let binds: Vec<String> = p
        .ports()
        .iter()
        .filter(|q| q.dir == PortDir::In)
        .map(|q| format!("{0}: {0}", q.name))
        .collect();
    src.push_str(&format!(
        "    let u = {}{} {{ {} }}\n",
        p.name(),
        generics(p),
        binds.join(", ")
    ));
    for q in p.ports().iter().filter(|q| q.dir == PortDir::Out) {
        src.push_str(&format!("    {0} = u.{0}\n", q.name));
    }
    src.push_str("}\n");
    src
}

fn work_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("volt-k7-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");
    dir
}

/// `volt build` → `build/rtl/*.sv`.
fn build_sv(dir: &Path, src: &str) -> Vec<PathBuf> {
    std::fs::write(dir.join("wrap.volt"), src).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_volt"))
        .args(["--lang", "en", "build", "wrap.volt"])
        .current_dir(dir)
        .env("VOLT_TOOL_BACKEND", "local")
        .output()
        .expect("volt çalışmalı");
    assert!(
        out.status.success(),
        "volt build başarısız:\n{src}\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir.join("build/rtl"))
        .expect("build/rtl")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "sv"))
        .collect();
    files.sort();
    files
}

/// Yosys JSON netlist'i (`Wrap`, düzleştirilmiş, bellek flop'lara açık).
fn netlist(yosys: &Path, dir: &Path, files: &[PathBuf]) -> Value {
    let list: Vec<String> = files
        .iter()
        .map(|f| format!("\"{}\"", f.display()))
        .collect();
    let json = dir.join("wrap.json");
    let script = format!(
        "read_verilog -sv {}; hierarchy -check -top Wrap; proc; flatten; memory; opt_clean; write_json \"{}\"",
        list.join(" "),
        json.display()
    );
    let out = Command::new(yosys)
        .args(["-q", "-p", &script])
        .output()
        .expect("yosys çalışmalı");
    assert!(
        out.status.success(),
        "yosys başarısız:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&std::fs::read(&json).expect("json")).expect("geçerli json")
}

/// Flip-flop ya da mandal hücresi mi (`$dff`, `$adff`, `$sdffe`, `$aldff`,
/// `$dffsr`, `$ff`, `$dlatch`, ...): kombinasyonel yol burada durur.
fn is_state_cell(ty: &str) -> bool {
    ty.contains("ff") || ty.contains("dlatch") || ty.starts_with("$mem")
}

/// Netlist'te her çıkış portunun kombinasyonel bağlı olduğu giriş portları.
fn comb_paths(net: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let m = &net["modules"]["Wrap"];
    let bits = |v: &Value| -> Vec<u64> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .collect()
    };
    // bit → onu süren kombinasyonel hücrenin giriş bitleri.
    let mut drivers: HashMap<u64, Vec<u64>> = HashMap::new();
    for cell in m["cells"].as_object().expect("cells").values() {
        let ty = cell["type"].as_str().unwrap_or_default();
        assert!(
            !ty.starts_with("$mem"),
            "bellek hücresi flop'lara açılmamış: {ty}"
        );
        if is_state_cell(ty) {
            continue;
        }
        let dirs = cell["port_directions"].as_object().expect("yönler");
        let conns = cell["connections"].as_object().expect("bağlantılar");
        let mut ins = Vec::new();
        let mut outs = Vec::new();
        for (name, c) in conns {
            match dirs.get(name).and_then(Value::as_str) {
                Some("input") => ins.extend(bits(c)),
                Some("output") => outs.extend(bits(c)),
                _ => {}
            }
        }
        for o in outs {
            drivers.entry(o).or_default().extend(ins.iter().copied());
        }
    }
    let ports = m["ports"].as_object().expect("ports");
    let mut input_of: HashMap<u64, String> = HashMap::new();
    for (name, p) in ports {
        if p["direction"] == "input" {
            for b in bits(&p["bits"]) {
                input_of.insert(b, name.clone());
            }
        }
    }
    let mut out = BTreeMap::new();
    for (name, p) in ports {
        if p["direction"] != "output" {
            continue;
        }
        let mut seen = HashSet::new();
        let mut stack = bits(&p["bits"]);
        let mut deps = BTreeSet::new();
        while let Some(b) = stack.pop() {
            if !seen.insert(b) {
                continue;
            }
            if let Some(i) = input_of.get(&b) {
                deps.insert(i.clone());
            }
            if let Some(ds) = drivers.get(&b) {
                stack.extend(ds.iter().copied());
            }
        }
        out.insert(name.clone(), deps);
    }
    out
}

/// Tablo, primitifin portlarından türetilen beklenti biçiminde.
fn table(p: BuiltinPrim) -> BTreeMap<String, BTreeSet<String>> {
    p.ports()
        .iter()
        .filter(|q| q.dir == PortDir::Out)
        .map(|q| {
            let ins = p
                .comb_inputs(q.name)
                .iter()
                .map(|s| s.to_string())
                .collect();
            (q.name.to_string(), ins)
        })
        .collect()
}

#[test]
fn table_names_only_real_ports() {
    for p in all_prims() {
        for q in p.ports() {
            for i in p.comb_inputs(q.name) {
                let port = p
                    .port(i)
                    .unwrap_or_else(|| panic!("{}: '{i}' yok", p.name()));
                assert_eq!(
                    q.dir,
                    PortDir::Out,
                    "{}: '{}' çıkış değil",
                    p.name(),
                    q.name
                );
                assert_eq!(port.dir, PortDir::In, "{}: '{i}' giriş değil", p.name());
                assert_ne!(port.kind, PortKind::Clock, "{}: '{i}' saat", p.name());
            }
        }
        // Var olmayan çıkış adı boş döner.
        assert!(p.comb_inputs("no_such_port").is_empty());
    }
}

/// Sarmalayıcıların her biri derlenir (araç istemez): Yosys testinin
/// girdisi araçsız işte de bozulmasın.
#[test]
fn every_wrapper_builds() {
    for p in all_prims() {
        let dir = work_dir(&format!("build-{}", p.name()));
        let files = build_sv(&dir, &wrapper(p));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!files.is_empty(), "{}: SV yok", p.name());
    }
}

/// Her satır primitifin ürettiği SV'nin kombinasyonel yol analizine eşit.
#[test]
fn builtin_summaries_match_the_generated_sv() {
    let Some(yosys) = tools::require(Tool::Yosys) else {
        return;
    };
    let mut bad = Vec::new();
    for p in all_prims() {
        let dir = work_dir(p.name());
        let src = wrapper(p);
        let files = build_sv(&dir, &src);
        let net = netlist(&yosys, &dir, &files);
        let found = comb_paths(&net);
        let expected = table(p);
        // Kanıt satırı (CI günlüğü): primitif ve Yosys'in bulduğu yollar.
        println!("K7 {}: {found:?}", p.name());
        if found != expected {
            bad.push(format!("{}: tablo {expected:?}, Yosys {found:?}", p.name()));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Analizin kendisi: bilinen bir kombinasyonel yolu ve bir register'ı
/// ayırt ediyor mu (yanlış bir analiz tabloyu boş yere doğrulamasın).
#[test]
fn comb_path_analysis_tells_a_gate_from_a_register() {
    let Some(yosys) = tools::require(Tool::Yosys) else {
        return;
    };
    let dir = work_dir("self");
    let sv = dir.join("wrap.sv");
    std::fs::write(
        &sv,
        "module Wrap(input logic clk, input logic a, input logic b, output logic y, output logic q);\n\
         \x20 logic r;\n  always_ff @(posedge clk) r <= b;\n  assign y = a & r;\n  assign q = r;\nendmodule\n",
    )
    .unwrap();
    let found = comb_paths(&netlist(&yosys, &dir, &[sv]));
    let _ = std::fs::remove_dir_all(&dir);
    let want: BTreeMap<String, BTreeSet<String>> = [
        ("q".to_string(), BTreeSet::new()),
        ("y".to_string(), ["a".to_string()].into_iter().collect()),
    ]
    .into_iter()
    .collect();
    assert_eq!(found, want);
}
