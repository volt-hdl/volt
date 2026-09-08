//! Volt LSP sunucusu (F5a) — error-recovery.md §9, Volt-UX-Anayasası
//! "LSP: Öğreten Editör".
//!
//! `volt lsp` alt komutu stdio üzerinden çalıştırır. Analiz boru hattı
//! driver ile aynıdır (analysis.rs); tanılar push modeliyle
//! (`publish_diagnostics`) ve pull modeliyle (`textDocument/diagnostic`)
//! sunulur. didChange 300ms debounce'lanır.

pub mod analysis;
pub mod completion;
pub mod convert;
pub mod definition;
pub mod docs;
pub mod hover;
pub mod symbols;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DiagnosticOptions,
    DiagnosticServerCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, DocumentSymbolParams,
    DocumentSymbolResponse, FullDocumentDiagnosticReport, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverContents, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, Location, MarkupContent, MarkupKind,
    MessageType, OneOf, Position, RelatedFullDocumentDiagnosticReport, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};

/// didChange sonrası analiz gecikmesi — her tuş vuruşunda tam analiz
/// koşmamak için (error-recovery.md §9 canlılık hedefi).
const DEBOUNCE_MS: u64 = 300;

struct DocState {
    text: String,
    version: i32,
}

type DocMap = Arc<Mutex<HashMap<Url, DocState>>>;

pub struct Backend {
    client: Client,
    docs: DocMap,
}

/// Belge metnini analiz edip LSP tanılarına çevirir (push ve pull
/// yolları ortak kullanır).
fn compute_diagnostics(uri: &Url, text: &str) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let analysis = analysis::analyze(uri.path(), text);
    analysis
        .diagnostics
        .iter()
        .map(|d| convert::to_lsp_diagnostic(d, &analysis.map, uri))
        .collect()
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            docs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn snapshot(&self, uri: &Url) -> Option<String> {
        self.docs
            .lock()
            .expect("docs kilidi zehirlenmemeli")
            .get(uri)
            .map(|d| d.text.clone())
    }

    /// Konumdaki belgeyi analiz eder ve bayt offset döndürür.
    fn analyze_at(&self, uri: &Url, position: Position) -> Option<(analysis::Analysis, u32)> {
        let text = self.snapshot(uri)?;
        let analysis = analysis::analyze(uri.path(), &text);
        let offset = analysis.map.byte_of_utf16_position(
            analysis.file_id,
            position.line,
            position.character,
        );
        Some((analysis, offset))
    }

    async fn publish_now(&self, uri: Url, text: &str, version: Option<i32>) {
        let diags = compute_diagnostics(&uri, text);
        self.client.publish_diagnostics(uri, diags, version).await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                // FULL senkronizasyon — artımlı sonraki fazda.
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
                    DiagnosticOptions {
                        identifier: Some("volt".to_string()),
                        inter_file_dependencies: false,
                        workspace_diagnostics: false,
                        work_done_progress_options: Default::default(),
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ":".to_string(),
                        "@".to_string(),
                        ".".to_string(),
                    ]),
                    ..CompletionOptions::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "volt-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "volt-lsp hazır")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;
        self.docs
            .lock()
            .expect("docs kilidi zehirlenmemeli")
            .insert(
                uri.clone(),
                DocState {
                    text: text.clone(),
                    version,
                },
            );
        self.publish_now(uri, &text, Some(version)).await;
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        // FULL sync: tek değişiklik, tüm metin.
        let Some(change) = params.content_changes.pop() else {
            return;
        };
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        self.docs
            .lock()
            .expect("docs kilidi zehirlenmemeli")
            .insert(
                uri.clone(),
                DocState {
                    text: change.text,
                    version,
                },
            );

        // Debounce: DEBOUNCE_MS içinde yeni değişiklik gelirse bu tur
        // atlanır (sürüm karşılaştırması).
        let docs = Arc::clone(&self.docs);
        let client = self.client.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
            let text = {
                let guard = docs.lock().expect("docs kilidi zehirlenmemeli");
                match guard.get(&uri) {
                    Some(doc) if doc.version == version => doc.text.clone(),
                    _ => return,
                }
            };
            let diags = compute_diagnostics(&uri, &text);
            client.publish_diagnostics(uri, diags, Some(version)).await;
        });
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = self.snapshot(&uri) {
            self.publish_now(uri, &text, None).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.docs
            .lock()
            .expect("docs kilidi zehirlenmemeli")
            .remove(&uri);
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> Result<DocumentDiagnosticReportResult> {
        let uri = params.text_document.uri;
        let items = self
            .snapshot(&uri)
            .map(|text| compute_diagnostics(&uri, &text))
            .unwrap_or_default();
        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            }),
        ))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some((analysis, offset)) = self.analyze_at(&uri, position) else {
            return Ok(None);
        };
        Ok(hover::hover(&analysis, offset).map(|(value, span)| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(convert::span_to_range(&analysis.map, span)),
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some((analysis, offset)) = self.analyze_at(&uri, position) else {
            return Ok(None);
        };
        let items = completion::completions(&analysis, offset);
        Ok((!items.is_empty()).then_some(CompletionResponse::Array(items)))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some((analysis, offset)) = self.analyze_at(&uri, position) else {
            return Ok(None);
        };
        Ok(definition::definition(&analysis, offset).map(|span| {
            GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: convert::span_to_range(&analysis.map, span),
            })
        }))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let Some(text) = self.snapshot(&uri) else {
            return Ok(None);
        };
        let analysis = analysis::analyze(uri.path(), &text);
        let syms = symbols::document_symbols(&analysis);
        Ok((!syms.is_empty()).then_some(DocumentSymbolResponse::Nested(syms)))
    }
}

/// Sunucuyu stdio üzerinden çalıştırır (`volt lsp` girişi). Kendi
/// tokio çalışma zamanını kurar — driver senkron kalır.
pub fn run_stdio() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio çalışma zamanı kurulamalı");
    runtime.block_on(async {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let (service, socket) = LspService::new(Backend::new);
        Server::new(stdin, stdout, socket).serve(service).await;
    });
}
