//! `volt lsp` protokol düzeyinde testler (ADR-0070): gerçek ikili stdio
//! üzerinden JSON-RPC konuşulur — katlama, üst sınır ve hover, editörün
//! gördüğü biçimde doğrulanır.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use serde_json::{json, Value};

const TIMEOUT: Duration = Duration::from_secs(30);

struct Lsp {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Value>,
    next_id: i64,
}

impl Lsp {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_volt"))
            .arg("lsp")
            .env("VOLT_LANG", "en")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("volt lsp başlamalı");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut len = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let line = line.trim_end();
                    if line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.strip_prefix("Content-Length:") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; len];
                if reader.read_exact(&mut body).is_err() {
                    return;
                }
                if let Ok(v) = serde_json::from_slice::<Value>(&body) {
                    if tx.send(v).is_err() {
                        return;
                    }
                }
            }
        });
        let mut lsp = Self {
            child,
            stdin,
            rx,
            next_id: 1,
        };
        lsp.request("initialize", json!({ "capabilities": {}, "rootUri": null }));
        lsp.notify("initialized", json!({}));
        lsp
    }

    fn send(&mut self, msg: &Value) {
        let body = serde_json::to_vec(msg).expect("json");
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).expect("yazılmalı");
        self.stdin.write_all(&body).expect("yazılmalı");
        self.stdin.flush().expect("flush");
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let msg = self.rx.recv_timeout(TIMEOUT).expect("LSP yanıtı gelmeli");
            if msg.get("id") == Some(&json!(id)) {
                assert!(msg.get("error").is_none(), "LSP hatası: {msg}");
                return msg["result"].clone();
            }
        }
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({ "textDocument": { "uri": uri, "languageId": "volt", "version": 1, "text": text } }),
        );
    }

    /// Pull modeli (`textDocument/diagnostic`) — editörün gördüğü liste.
    fn diagnostics(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.open(uri, text);
        let res = self.request(
            "textDocument/diagnostic",
            json!({ "textDocument": { "uri": uri } }),
        );
        res["items"].as_array().cloned().unwrap_or_default()
    }

    /// `needle`'ın `nth` geçişinin ilk karakterinde hover markdown'u.
    fn hover(&mut self, uri: &str, text: &str, needle: &str, nth: usize) -> String {
        self.open(uri, text);
        let offset = text
            .match_indices(needle)
            .nth(nth)
            .map(|(i, _)| i)
            .expect("aranan metin kaynakta olmalı");
        let line = text[..offset].matches('\n').count();
        let col = offset - text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let res = self.request(
            "textDocument/hover",
            json!({ "textDocument": { "uri": uri }, "position": { "line": line, "character": col } }),
        );
        res["contents"]["value"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

impl Drop for Lsp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn uri(name: &str) -> String {
    let dir = std::env::temp_dir();
    let path = dir.join(name);
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

fn codes(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .map(|d| d["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

// ═══ Katlama (ADR-0068 LSP açığı, ADR-0070 §3) ═══════════════════════

/// 283 yinelemeli `for` gövdesinde iki farklı hata: CLI 2 tanı basar;
/// katlamadan önce editör 566 tanı alıyordu.
#[test]
fn lsp_folds_unrolled_duplicate_diagnostics_like_cli() {
    let src = "const N : u32 = 283\nmodule M { in a : u8\n out o : [i8; N]\n out p : [i8; N]\n for i in 0..N { o[i] = a\n p[i] = a } }\n";
    let mut lsp = Lsp::start();
    let items = lsp.diagnostics(&uri("lsp_fold283.volt"), src);
    assert_eq!(codes(&items), ["E2002", "E2002"], "{items:#?}");
    for d in &items {
        let msg = d["message"].as_str().unwrap_or_default();
        assert!(
            msg.contains(
                "note: reported once; occurs in 283 unrolled 'for' iterations (i = 0..282)"
            ),
            "katlama notu mesajda olmalı: {msg}"
        );
    }
}

/// ADR-0068 fuzz regresyon girdisi editörde de açılım notunu taşır. §6'dan
/// beri kurtarma düğümlü gövde bir kez açılır: kopya kalmaz, not tekil
/// yineleme notudur ("in the unrolled 'for' iteration i = 0, …"); katlama
/// notu ("… iterations") da aynı alt dizgeyi içerir.
#[test]
fn lsp_fuzz_flood_input_carries_fold_note() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fuzz_regressions/oom_nested_for_diag_flood_1157b.volt");
    let src = std::fs::read_to_string(path).expect("regresyon girdisi");
    let mut lsp = Lsp::start();
    let items = lsp.diagnostics(&uri("lsp_flood.volt"), &src);
    assert!(items.len() < 100, "{} tanı", items.len());
    assert!(
        items.iter().any(|d| d["message"]
            .as_str()
            .unwrap_or_default()
            .contains("unrolled 'for' iteration")),
        "açılım notu yok"
    );
}

/// Katlanmayan (farklı) tanılar sınırı aşarsa editör LSP_MAX_DIAGNOSTICS
/// kadarını + W0023 alır; W0023 çözümü `volt check`'i gösterir.
#[test]
fn lsp_caps_distinct_diagnostics_with_w0023() {
    let mut src = String::from("module M {\n    out y : u8\n    y = 0\n");
    for i in 0..300 {
        src.push_str(&format!("    let w{i} : u8 = undefined_{i}\n"));
    }
    src.push_str("}\n");
    let mut lsp = Lsp::start();
    let items = lsp.diagnostics(&uri("lsp_cap.volt"), &src);
    let limit = volt_diagnostics::LSP_MAX_DIAGNOSTICS;
    assert_eq!(items.len(), limit + 1, "{:?}", codes(&items));
    let last = items.last().expect("W0023");
    assert_eq!(last["code"], "W0023");
    let msg = last["message"].as_str().unwrap_or_default();
    // Her satır E1001 + W1001 üretir: 600 tanı, 400'ü gizli.
    assert!(msg.contains(&format!("{limit} shown, 400 hidden")), "{msg}");
    assert!(msg.contains("volt check"), "{msg}");
}

// ═══ Hover tip adı (ADR-0070 §3.2) ══════════════════════════════════

const HOVER_SRC: &str = "struct P {\n    a : u8,\n    b : bool,\n}\n\nenum E { X, Y }\n\nstruct port Bus {\n    out d : u8\n    in r : bool\n}\n\nmodule M {\n    in clk : clock\n    in p : P\n    in e : E\n    out bus : Bus\n    out hs : Handshake<u8>\n    in arr : [P; 2]\n    out y : u8\n    y = 0\n}\n";

/// Struct/enum tipli port adıyla görünür (`p : P`, `struct` değil).
#[test]
fn lsp_hover_shows_struct_and_enum_type_names() {
    let mut lsp = Lsp::start();
    let u = uri("lsp_hover.volt");
    assert!(lsp.hover(&u, HOVER_SRC, "p : P", 0).contains("p : P"));
    assert!(lsp.hover(&u, HOVER_SRC, "e : E", 0).contains("e : E"));
    assert!(lsp
        .hover(&u, HOVER_SRC, "arr : [P; 2]", 0)
        .contains("arr : [P; 2]"));
}

/// Bundle portu yazıldığı tiple ve açılan portlarla görünür.
#[test]
fn lsp_hover_shows_bundle_port_type_and_flattened_ports() {
    let mut lsp = Lsp::start();
    let u = uri("lsp_hover_bundle.volt");
    let bus = lsp.hover(&u, HOVER_SRC, "bus : Bus", 0);
    assert!(bus.contains("bus : Bus"), "{bus}");
    assert!(
        bus.contains("`out bus_d : u8`") && bus.contains("`in bus_r : bool`"),
        "{bus}"
    );
    let hs = lsp.hover(&u, HOVER_SRC, "hs : Handshake", 0);
    assert!(hs.contains("hs : Handshake<u8>"), "{hs}");
    assert!(hs.contains("`in hs_ready : bool`"), "{hs}");
}

// ═══ Derinlik sınırı (ADR-0080) ══════════════════════════════════════

impl Lsp {
    /// `didChange` → debounce sonrası tokio işçisinde analiz → push tanıları.
    fn change_and_wait(&mut self, uri: &str, text: &str, version: i64) -> Vec<Value> {
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }],
            }),
        );
        loop {
            let msg = self
                .rx
                .recv_timeout(TIMEOUT)
                .expect("push tanıları gelmeli");
            if msg["method"] == "textDocument/publishDiagnostics"
                && msg["params"]["uri"] == uri
                && msg["params"]["version"] == version
            {
                return msg["params"]["diagnostics"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
            }
        }
    }
}

fn stack_regression(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fuzz_regressions")
        .join(name);
    std::fs::read_to_string(path).expect("regresyon girdisi")
}

/// Derin girdi editörde tek E0018 alır; sunucu (analiz tokio işçisinde de
/// koşar — `didChange`) ölmez ve sonraki isteklere yanıt verir. Düzeltmeden
/// önce yığın taşması dil sunucusunu abort ettiriyordu.
#[test]
fn lsp_deep_inputs_get_e0018_and_the_server_survives() {
    let mut lsp = Lsp::start();
    let u = uri("lsp_deep.volt");
    for name in [
        "stack_chain_generic_30000.volt",
        "stack_else_if_3000.volt",
        "stack_alias_chain_3000.volt",
    ] {
        let items = lsp.diagnostics(&u, &stack_regression(name));
        assert_eq!(codes(&items), ["E0018"], "{name}");
    }
    let pushed = lsp.change_and_wait(&u, &stack_regression("stack_paren_20000.volt"), 2);
    assert_eq!(codes(&pushed), ["E0018"]);
    // Sunucu hâlâ ayakta: sıradan bir hover yanıtlanır.
    let h = lsp.hover(&uri("lsp_deep_after.volt"), HOVER_SRC, "p : P", 0);
    assert!(h.contains("p : P"), "{h}");
}

/// Sınırın hemen altındaki geçerli tasarım editörde tam analiz edilir
/// (çözümleme, tip, saat alanı), en derin yaprakta hover ve belge
/// sembolleri yanıtlanır — hepsi derleyici yığınında.
#[test]
fn lsp_design_just_below_the_limit_is_analyzed_and_hovered() {
    let depth = volt_syntax::MAX_DEPTH as usize - 8;
    let text = format!(
        "module Top {{\n    in a : u8\n    out y : u8\n    y = {}\n}}\n",
        vec!["a"; depth].join(" ^ ")
    );
    let mut lsp = Lsp::start();
    let u = uri("lsp_near_limit.volt");
    let items = lsp.diagnostics(&u, &text);
    assert!(items.is_empty(), "{:?}", codes(&items));
    let pushed = lsp.change_and_wait(&u, &text, 2);
    assert!(pushed.is_empty(), "{:?}", codes(&pushed));
    let last_a = text.rfind('a').expect("en derin yaprak");
    let line = text[..last_a].matches('\n').count();
    let col = last_a - text[..last_a].rfind('\n').map_or(0, |i| i + 1);
    let hover = lsp.request(
        "textDocument/hover",
        json!({ "textDocument": { "uri": u }, "position": { "line": line, "character": col } }),
    );
    assert!(
        hover["contents"]["value"]
            .as_str()
            .unwrap_or_default()
            .contains('a'),
        "{hover}"
    );
    let symbols = lsp.request(
        "textDocument/documentSymbol",
        json!({ "textDocument": { "uri": u } }),
    );
    assert!(symbols.to_string().contains("Top"), "{symbols}");
    // En ağır yapılar (ADR-0080 §1.2: SV doğrulaması `as` zincirinde kat
    // başına ~7 KB) `didChange` yolunda, yani tokio işçisinde.
    for (version, e) in [
        (3, format!("a{}", " as u8".repeat(depth))),
        (4, format!("{}a", "~".repeat(depth))),
    ] {
        let text = format!("module Top {{\n    in a : u8\n    out y : u8\n    y = {e}\n}}\n");
        let pushed = lsp.change_and_wait(&u, &text, version);
        assert!(pushed.is_empty(), "{:?}", codes(&pushed));
    }
}

// ═══ Fonksiyonlar (ADR-0081 Karar 13) ═══════════════════════════════

const FN_SRC: &str = "enum Fmt { I, S }\n\n/// Immediate of an I-type instruction.\nfn imm(instr: u32, f: Fmt) -> u32 {\n    if f == Fmt::I { instr >> 20 } else { instr }\n}\n\nmodule M {\n    in  instr : u32\n    in  f     : Fmt\n    out y     : u32\n    y = imm(instr, f)\n}\n";

/// Hover imzayı gösterir — tanımda ve çağrı yerinde aynı; doc yorumu da.
#[test]
fn lsp_hover_shows_the_fn_signature_at_definition_and_call() {
    let mut lsp = Lsp::start();
    let u = uri("lsp_fn_hover.volt");
    let sig = "fn imm(instr: u32, f: Fmt) -> u32";
    let at_def = lsp.hover(&u, FN_SRC, "imm(", 0);
    let at_call = lsp.hover(&u, FN_SRC, "imm(", 1);
    assert!(at_def.contains(sig), "{at_def}");
    assert!(
        at_def.contains("Immediate of an I-type instruction."),
        "{at_def}"
    );
    assert_eq!(at_def, at_call);
}

/// Tanıma git: çağrıdan fn tanımına (çözüm tabanlı `def_at`).
#[test]
fn lsp_goto_definition_jumps_from_a_call_to_the_fn() {
    let mut lsp = Lsp::start();
    let u = uri("lsp_fn_def.volt");
    lsp.open(&u, FN_SRC);
    let call = FN_SRC.rfind("imm(").expect("çağrı");
    let line = FN_SRC[..call].matches('\n').count();
    let col = call - FN_SRC[..call].rfind('\n').map_or(0, |i| i + 1);
    let res = lsp.request(
        "textDocument/definition",
        json!({ "textDocument": { "uri": u }, "position": { "line": line, "character": col } }),
    );
    let loc = if res.is_array() { res[0].clone() } else { res };
    // `fn imm(` tanımı 4. satırda (0-tabanlı 3), ad sütun 3'te.
    assert_eq!(loc["range"]["start"]["line"], 3, "{loc}");
    assert_eq!(loc["range"]["start"]["character"], 3, "{loc}");
}

/// Tamamlama fn'i FUNCTION türüyle ve imzası `detail`'de önerir.
#[test]
fn lsp_completion_offers_fn_with_signature_detail() {
    let mut lsp = Lsp::start();
    let u = uri("lsp_fn_completion.volt");
    let src = FN_SRC.replace("    y = imm(instr, f)\n", "    y = im\n");
    lsp.open(&u, &src);
    let at = src.find("= im\n").expect("imleç") + 4;
    let line = src[..at].matches('\n').count();
    let col = at - src[..at].rfind('\n').map_or(0, |i| i + 1);
    let res = lsp.request(
        "textDocument/completion",
        json!({ "textDocument": { "uri": u }, "position": { "line": line, "character": col } }),
    );
    let items = if res.is_array() {
        res.as_array().cloned().unwrap_or_default()
    } else {
        res["items"].as_array().cloned().unwrap_or_default()
    };
    let imm = items
        .iter()
        .find(|i| i["label"] == "imm")
        .unwrap_or_else(|| panic!("imm önerilmeli: {items:?}"));
    // CompletionItemKind::FUNCTION = 3
    assert_eq!(imm["kind"], 3, "{imm}");
    assert_eq!(imm["detail"], "fn imm(instr: u32, f: Fmt) -> u32", "{imm}");
}
