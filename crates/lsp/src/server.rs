//! The transport side: the initialize handshake, the message loop and the translation between
//! LSP messages and calls on the [`Workspace`].

use crate::actions::FIX_ALL_KIND;
use crate::workspace::Workspace;
use anyhow::{Result, anyhow};
use core_incremental::{Failure, Stage};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    LogMessage, Notification as LspNotification, ShowMessage,
};
use lsp_types::request::{
    CodeActionRequest, DocumentDiagnosticRequest, Formatting, RegisterCapability,
    Request as LspRequest, WorkspaceDiagnosticRefresh,
};
use lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionParams, CodeActionProviderCapability,
    DiagnosticOptions, DiagnosticServerCapabilities, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWatchedFilesRegistrationOptions,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, DocumentFormattingParams,
    FileSystemWatcher, FullDocumentDiagnosticReport, GlobPattern, InitializeParams,
    LogMessageParams, MessageType, OneOf, Registration, RegistrationParams,
    RelatedFullDocumentDiagnosticReport, ServerCapabilities, ServerInfo, ShowMessageParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri, WatchKind,
};
use provider_api::LanguageProvider;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::path::PathBuf;

/// What the client told us it can do.
#[derive(Default)]
struct ClientSupport {
    watch_files: bool,
    diagnostic_refresh: bool,
}

fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL,
        )),
        diagnostic_provider: Some(DiagnosticServerCapabilities::Options(DiagnosticOptions {
            identifier: Some(crate::diagnostics::SOURCE.into()),
            inter_file_dependencies: false,
            workspace_diagnostics: false,
            ..Default::default()
        })),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![
                CodeActionKind::QUICKFIX,
                CodeActionKind::new(FIX_ALL_KIND),
            ]),
            ..Default::default()
        })),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

/// `file:` URIs only: a document that is not a file on disk has no config to find.
fn path_of(uri: &Uri) -> Option<PathBuf> {
    url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

fn root_of(params: &InitializeParams) -> PathBuf {
    #[allow(deprecated)]
    let root_uri = params.root_uri.as_ref();
    params
        .workspace_folders
        .iter()
        .flatten()
        .map(|f| &f.uri)
        .chain(root_uri)
        .find_map(path_of)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default()
}

pub struct Server {
    conn: Connection,
    workspace: Workspace,
    support: ClientSupport,
    /// Failures already shown to the user, so that one broken config is one message.
    shown: HashSet<String>,
    next_request_id: i32,
}

/// Runs the server on `conn` until the client shuts it down.
pub fn serve(conn: Connection, providers: Vec<Box<dyn LanguageProvider>>) -> Result<()> {
    let (id, params) = conn.initialize_start()?;
    let params: InitializeParams = serde_json::from_value(params)?;
    let workspace_caps = params.capabilities.workspace.as_ref();
    let support = ClientSupport {
        watch_files: workspace_caps
            .and_then(|w| w.did_change_watched_files)
            .and_then(|c| c.dynamic_registration)
            .unwrap_or(false),
        diagnostic_refresh: workspace_caps
            .and_then(|w| w.diagnostic.as_ref())
            .and_then(|d| d.refresh_support)
            .unwrap_or(false),
    };
    let result = serde_json::json!({
        "capabilities": capabilities(),
        "serverInfo": ServerInfo { name: "sudsy".into(), version: Some(env!("CARGO_PKG_VERSION").into()) },
    });
    conn.initialize_finish(id, result)?;
    let (workspace, config_error) = Workspace::new(root_of(&params), providers);
    let mut server = Server {
        conn,
        workspace,
        support,
        shown: HashSet::new(),
        next_request_id: 0,
    };
    if let Some(error) = config_error {
        server.config_failed(error);
    }
    // `initialize_finish` has consumed the client's `initialized` notification.
    server.register_watchers()?;
    server.run()
}

impl Server {
    fn run(&mut self) -> Result<()> {
        // Iterate over a clone of the receiver, so that handlers can borrow `self`.
        let receiver = self.conn.receiver.clone();
        for message in &receiver {
            match message {
                Message::Request(req) => {
                    if self.conn.handle_shutdown(&req)? {
                        return Ok(());
                    }
                    self.request(req)?;
                }
                Message::Notification(note) => self.notification(note)?,
                Message::Response(_) => {}
            }
        }
        Ok(())
    }

    fn send(&self, message: impl Into<Message>) -> Result<()> {
        self.conn
            .sender
            .send(message.into())
            .map_err(|_| anyhow!("the client closed the connection"))
    }

    fn notify<N: LspNotification>(&self, params: N::Params) -> Result<()> {
        self.send(Notification::new(N::METHOD.to_string(), params))
    }

    fn call<R: LspRequest>(&mut self, params: R::Params) -> Result<()> {
        self.next_request_id += 1;
        let id = RequestId::from(self.next_request_id);
        self.send(Request::new(id, R::METHOD.to_string(), params))
    }

    fn show(&self, typ: MessageType, message: String) {
        let _ = self.notify::<ShowMessage>(ShowMessageParams { typ, message });
    }

    fn log(&self, typ: MessageType, message: String) {
        let _ = self.notify::<LogMessage>(LogMessageParams { typ, message });
    }

    /// The config could not be loaded. The message is the one every file's failure carries, so it
    /// counts as shown for them.
    fn config_failed(&mut self, error: String) {
        let message = format!("Sudsy: {error}");
        if self.shown.insert(message.clone()) {
            self.show(MessageType::ERROR, message);
        }
    }

    /// Tells the user why a file could not be processed: once per distinct reason when it is
    /// something they can fix (config, worker), in the log when it is about one file's content.
    fn failed(&mut self, uri: &Uri, failure: &Failure) {
        let message = format!("Sudsy: {}", failure.message);
        match failure.stage {
            Stage::Unsupported => {}
            Stage::Format => self.log(
                MessageType::WARNING,
                format!("{}: {}", uri.as_str(), failure.message),
            ),
            Stage::Config | Stage::Options | Stage::Parse | Stage::Semantics => {
                if self.shown.insert(message.clone()) {
                    let level = if matches!(failure.stage, Stage::Parse | Stage::Semantics) {
                        MessageType::WARNING
                    } else {
                        MessageType::ERROR
                    };
                    self.show(level, message);
                }
            }
        }
    }

    fn request(&mut self, req: Request) -> Result<()> {
        let response = match req.method.as_str() {
            DocumentDiagnosticRequest::METHOD => self.answer(req, Self::diagnostic),
            CodeActionRequest::METHOD => self.answer(req, Self::code_action),
            Formatting::METHOD => self.answer(req, Self::formatting),
            _ => Response::new_err(
                req.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported request {}", req.method),
            ),
        };
        self.send(response)
    }

    /// Decodes the parameters of `req`, runs `handler` and wraps its result in a response.
    fn answer<P: DeserializeOwned, R: Serialize>(
        &mut self,
        req: Request,
        handler: impl FnOnce(&mut Self, P) -> R,
    ) -> Response {
        match serde_json::from_value::<P>(req.params) {
            Ok(params) => Response::new_ok(req.id, handler(self, params)),
            Err(e) => Response::new_err(req.id, ErrorCode::InvalidParams as i32, e.to_string()),
        }
    }

    fn diagnostic(&mut self, params: DocumentDiagnosticParams) -> DocumentDiagnosticReportResult {
        let uri = params.text_document.uri;
        let path = path_of(&uri);
        if self.workspace.is_deep() {
            self.workspace.load_project();
        }
        if let Some(failure) = path
            .as_deref()
            .and_then(|p| self.workspace.semantic_warning(p))
        {
            self.failed(&uri, &failure);
        }
        let items = match path.map(|p| self.workspace.diagnostics(&p, &uri)) {
            Some(Ok(items)) => items,
            Some(Err(failure)) => {
                self.failed(&uri, &failure);
                Vec::new()
            }
            None => Vec::new(),
        };
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
            RelatedFullDocumentDiagnosticReport {
                related_documents: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items,
                },
            },
        ))
    }

    fn code_action(
        &mut self,
        params: CodeActionParams,
    ) -> Option<Vec<lsp_types::CodeActionOrCommand>> {
        let uri = params.text_document.uri;
        let path = path_of(&uri)?;
        let only = params.context.only;
        match self
            .workspace
            .code_actions(&path, &uri, params.range, only.as_deref())
        {
            Ok(actions) => Some(actions),
            Err(failure) => {
                self.failed(&uri, &failure);
                None
            }
        }
    }

    fn formatting(&mut self, params: DocumentFormattingParams) -> Option<Vec<lsp_types::TextEdit>> {
        let uri = params.text_document.uri;
        let path = path_of(&uri)?;
        match self.workspace.format(&path) {
            Ok(edits) => Some(edits),
            Err(failure) => {
                self.failed(&uri, &failure);
                None
            }
        }
    }

    fn notification(&mut self, note: Notification) -> Result<()> {
        match note.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let p: DidOpenTextDocumentParams = serde_json::from_value(note.params)?;
                let doc = p.text_document;
                if let Some(path) = path_of(&doc.uri) {
                    self.workspace.open(path, doc.version, doc.text);
                }
            }
            DidChangeTextDocument::METHOD => {
                let p: DidChangeTextDocumentParams = serde_json::from_value(note.params)?;
                if let Some(path) = path_of(&p.text_document.uri) {
                    self.workspace
                        .change(&path, p.text_document.version, &p.content_changes);
                }
            }
            DidCloseTextDocument::METHOD => {
                let p: DidCloseTextDocumentParams = serde_json::from_value(note.params)?;
                if let Some(path) = path_of(&p.text_document.uri) {
                    self.workspace.close(&path);
                }
            }
            DidChangeWatchedFiles::METHOD => {
                let p: DidChangeWatchedFilesParams = serde_json::from_value(note.params)?;
                let config_changed = p
                    .changes
                    .iter()
                    .filter_map(|c| path_of(&c.uri))
                    .any(|path| self.workspace.is_config_file(&path));
                if config_changed {
                    self.reload_config()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn reload_config(&mut self) -> Result<()> {
        self.shown.clear();
        if let Some(error) = self.workspace.reload_config() {
            self.config_failed(error);
        }
        if self.support.diagnostic_refresh {
            self.call::<WorkspaceDiagnosticRefresh>(())?;
        }
        Ok(())
    }

    /// Asks the client to tell us when a config file or `.editorconfig` changes.
    fn register_watchers(&mut self) -> Result<()> {
        if !self.support.watch_files {
            return Ok(());
        }
        let watcher = |glob: &str| FileSystemWatcher {
            glob_pattern: GlobPattern::String(glob.into()),
            kind: Some(WatchKind::all()),
        };
        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: ["**/*.yaml", "**/*.yml", "**/.editorconfig"]
                .map(watcher)
                .into(),
        };
        self.call::<RegisterCapability>(RegistrationParams {
            registrations: vec![Registration {
                id: "sudsy-config-watcher".into(),
                method: DidChangeWatchedFiles::METHOD.into(),
                register_options: Some(serde_json::to_value(options)?),
            }],
        })
    }
}
