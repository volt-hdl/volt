//! Sunucu işleyicileri süreç içinde (ADR-0091): initialize yetenekleri ve
//! ipucu ayarları, didOpen/didChange/didSave/didClose, pull tanıları,
//! inlayHint, codeAction, hover, documentSymbol, tamamlama, tanıma git.
//!
//! Gerçek stdio JSON-RPC yolu `volt-driver/tests/lsp_protocol_tests.rs`'tedir;
//! burada işleyiciler doğrudan çağrılır (başlatılmamış istemciye giden
//! bildirimleri tower-lsp sessizce düşürür).

use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;
use volt_lsp::inlay::HintConfig;

const SRC: &str = "\
module Count {
    in  clk : clock
    in  a   : u8
    out q   : u9

    let s = a + 1
    reg c : u9 = 0
    on clk {
        c = s
    }
    q = c
}
";

fn uri() -> Url {
    Url::from_file_path(std::env::temp_dir().join("volt_server_test.volt")).expect("uri")
}

fn pos(src: &str, needle: &str) -> Position {
    let offset = src.find(needle).expect("metin kaynakta olmalı");
    let line = src[..offset].matches('\n').count() as u32;
    let col = offset - src[..offset].rfind('\n').map_or(0, |i| i + 1);
    Position::new(line, col as u32)
}

fn doc() -> TextDocumentIdentifier {
    TextDocumentIdentifier { uri: uri() }
}

fn at(src: &str, needle: &str) -> TextDocumentPositionParams {
    TextDocumentPositionParams {
        text_document: doc(),
        position: pos(src, needle),
    }
}

fn whole(src: &str) -> Range {
    Range::new(
        Position::new(0, 0),
        Position::new(src.lines().count() as u32, 0),
    )
}

async fn open(server: &volt_lsp::Backend, text: &str) {
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri(),
                language_id: "volt".into(),
                version: 1,
                text: text.into(),
            },
        })
        .await;
}

async fn pull(server: &volt_lsp::Backend) -> Vec<Diagnostic> {
    let res = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: doc(),
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("diagnostic");
    match res {
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(r)) => {
            r.full_document_diagnostic_report.items
        }
        other => panic!("beklenmeyen rapor: {other:?}"),
    }
}

async fn hints(server: &volt_lsp::Backend, src: &str) -> Vec<String> {
    let res = server
        .inlay_hint(InlayHintParams {
            text_document: doc(),
            range: whole(src),
            work_done_progress_params: Default::default(),
        })
        .await
        .expect("inlay_hint")
        .unwrap_or_default();
    res.into_iter()
        .map(|h| match h.label {
            InlayHintLabel::String(s) => s,
            InlayHintLabel::LabelParts(p) => p.into_iter().map(|x| x.value).collect(),
        })
        .collect()
}

/// JSON metninden ayar değeri (`LSPAny` = serde_json::Value).
fn json(text: &str) -> LSPAny {
    text.parse().expect("geçerli JSON")
}

fn init_params(options: Option<LSPAny>) -> InitializeParams {
    InitializeParams {
        initialization_options: options,
        ..InitializeParams::default()
    }
}

#[tokio::test]
async fn initialize_advertises_inlay_hints_and_quick_fixes() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    let res = server.initialize(init_params(None)).await.expect("init");
    let caps = res.capabilities;
    assert_eq!(caps.inlay_hint_provider, Some(OneOf::Left(true)));
    match caps.code_action_provider {
        Some(CodeActionProviderCapability::Options(o)) => {
            assert_eq!(o.code_action_kinds, Some(vec![CodeActionKind::QUICKFIX]));
        }
        other => panic!("code action yeteneği: {other:?}"),
    }
    assert!(caps.hover_provider.is_some());
    assert!(caps.document_symbol_provider.is_some());
    assert!(caps.diagnostic_provider.is_some());
    assert_eq!(server.hint_config(), HintConfig::default());
    server.initialized(InitializedParams {}).await;
    assert!(server.shutdown().await.is_ok());
}

#[tokio::test]
async fn initialization_options_switch_hint_kinds_off() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    let opts = json(r#"{"inlayHints": {"types": false,"latency": false}}"#);
    server
        .initialize(init_params(Some(opts)))
        .await
        .expect("init");
    let c = server.hint_config();
    assert!(!c.types && c.domains && !c.latency, "{c:?}");
    // Temiz kaynakta tip ipucu olurdu (`: u9`); tür kapalı.
    let src = SRC.replace("c = s", "c <= s");
    open(server, &src).await;
    assert!(hints(server, &src).await.is_empty());
}

#[tokio::test]
async fn did_change_configuration_updates_hint_kinds() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    server.initialize(init_params(None)).await.expect("init");
    let src = SRC.replace("c = s", "c <= s");
    open(server, &src).await;
    assert_eq!(hints(server, &src).await, vec![": u9"]);
    let off = json(r#"{"volt": {"inlayHints": {"types": false}}}"#);
    server
        .did_change_configuration(DidChangeConfigurationParams { settings: off })
        .await;
    assert!(hints(server, &src).await.is_empty());
    // `volt` anahtarı olmayan ayar ipucu ayarına dokunmaz.
    let unrelated = json(r#"{"editor": {"tabSize": 4}}"#);
    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: unrelated,
        })
        .await;
    assert!(!server.hint_config().types);
}

#[tokio::test]
async fn document_lifecycle_drives_pull_diagnostics() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    open(server, SRC).await;
    let codes = |ds: Vec<Diagnostic>| -> Vec<String> {
        ds.into_iter()
            .filter_map(|d| match d.code {
                Some(NumberOrString::String(s)) => Some(s),
                _ => None,
            })
            .collect()
    };
    assert_eq!(codes(pull(server).await), ["E0006"]);

    let fixed = SRC.replace("c = s", "c <= s");
    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: fixed.clone(),
            }],
        })
        .await;
    assert!(codes(pull(server).await).is_empty());
    // Değişikliksiz didChange yok sayılır.
    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri(),
                version: 3,
            },
            content_changes: vec![],
        })
        .await;
    server
        .did_save(DidSaveTextDocumentParams {
            text_document: doc(),
            text: None,
        })
        .await;
    assert!(codes(pull(server).await).is_empty());

    server
        .did_close(DidCloseTextDocumentParams {
            text_document: doc(),
        })
        .await;
    assert!(pull(server).await.is_empty());
    let hover = server
        .hover(HoverParams {
            text_document_position_params: at(&fixed, "s\n"),
            work_done_progress_params: Default::default(),
        })
        .await
        .expect("hover");
    assert!(hover.is_none(), "kapalı belgede hover yok");
}

#[tokio::test]
async fn code_action_returns_the_quick_fix_edit() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    open(server, SRC).await;
    let eq = pos(SRC, "= s");
    let res = server
        .code_action(CodeActionParams {
            text_document: doc(),
            range: Range::new(eq, eq),
            context: CodeActionContext::default(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("code_action")
        .expect("eylem");
    let CodeActionOrCommand::CodeAction(action) = &res[0] else {
        panic!("CodeAction bekleniyordu");
    };
    assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
    assert_eq!(action.is_preferred, Some(true));
    let edits = &action
        .edit
        .as_ref()
        .expect("edit")
        .changes
        .as_ref()
        .expect("changes")[&uri()];
    assert_eq!(edits[0].new_text, "<=");
    assert_eq!(edits[0].range.start, eq);
    let diag = &action.diagnostics.as_ref().expect("tanı")[0];
    assert_eq!(diag.code, Some(NumberOrString::String("E0006".into())));

    // Tanısız aralık → eylem yok; açılmamış belge → yok.
    let top = Range::new(Position::new(0, 0), Position::new(0, 1));
    let none = server
        .code_action(CodeActionParams {
            text_document: doc(),
            range: top,
            context: CodeActionContext::default(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("code_action");
    assert!(none.is_none());
}

#[tokio::test]
async fn editor_requests_answer_on_a_clean_document() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    let src = SRC.replace("c = s", "c <= s");
    open(server, &src).await;

    let hover = server
        .hover(HoverParams {
            text_document_position_params: at(&src, "c : u9"),
            work_done_progress_params: Default::default(),
        })
        .await
        .expect("hover")
        .expect("hover içeriği");
    let HoverContents::Markup(md) = hover.contents else {
        panic!("markdown bekleniyordu");
    };
    assert!(md.value.contains("c : u9"), "{}", md.value);

    let syms = server
        .document_symbol(DocumentSymbolParams {
            text_document: doc(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("symbols");
    let Some(DocumentSymbolResponse::Nested(syms)) = syms else {
        panic!("iç içe semboller bekleniyordu");
    };
    assert_eq!(syms[0].name, "Count");

    let def = server
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: at(&src, "s\n    }"),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("definition");
    let Some(GotoDefinitionResponse::Scalar(loc)) = def else {
        panic!("tanım konumu bekleniyordu");
    };
    assert_eq!(loc.range.start, pos(&src, "s = a"));

    let completion = server
        .completion(CompletionParams {
            text_document_position: at(&src, "a + 1"),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        })
        .await
        .expect("completion");
    assert!(completion.is_some());

    assert_eq!(hints(server, &src).await, vec![": u9"]);
}

#[tokio::test]
async fn requests_on_an_unknown_document_return_nothing() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    let p = at(SRC, "clk");
    assert!(server
        .completion(CompletionParams {
            text_document_position: p.clone(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        })
        .await
        .expect("completion")
        .is_none());
    assert!(server
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: p,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("definition")
        .is_none());
    assert!(server
        .document_symbol(DocumentSymbolParams {
            text_document: doc(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("symbols")
        .is_none());
    assert!(server
        .inlay_hint(InlayHintParams {
            text_document: doc(),
            range: whole(SRC),
            work_done_progress_params: Default::default(),
        })
        .await
        .expect("inlay")
        .is_none());
    assert!(server
        .code_action(CodeActionParams {
            text_document: doc(),
            range: whole(SRC),
            context: CodeActionContext::default(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("code_action")
        .is_none());
}

#[test]
fn hint_config_parsing_keeps_missing_and_non_bool_keys() {
    let base = HintConfig {
        types: true,
        domains: false,
        latency: true,
    };
    let v = json(r#"{"types": false,"clockDomains": "yes"}"#);
    let c = volt_lsp::hint_config_from(Some(&v), base);
    assert_eq!(
        c,
        HintConfig {
            types: false,
            domains: false,
            latency: true
        }
    );
    assert_eq!(volt_lsp::hint_config_from(None, base), base);
}

/// Yenilemeyi destekleyen istemciye `workspace/inlayHint/refresh` gider
/// (burada istemci başlatılmamış: istek hemen hatayla döner, işleyici
/// takılmaz); desteklemeyene gitmez (lsp_protocol_tests kapanış süresi).
#[tokio::test]
async fn hint_refresh_is_requested_only_from_supporting_clients() {
    let (service, _socket) = volt_lsp::service();
    let server = service.inner();
    let mut params = init_params(None);
    params.capabilities.workspace = Some(WorkspaceClientCapabilities {
        inlay_hint: Some(InlayHintWorkspaceClientCapabilities {
            refresh_support: Some(true),
        }),
        ..WorkspaceClientCapabilities::default()
    });
    server.initialize(params).await.expect("init");
    let settings = json(r#"{"volt": {"inlayHints": {"latency": false}}}"#);
    server
        .did_change_configuration(DidChangeConfigurationParams { settings })
        .await;
    assert!(!server.hint_config().latency);
}
