//! The transport side: the initialize handshake, the message loop and the translation between
//! LSP messages and calls on the [`Workspace`].

use crate::actions::{FIX_ALL_KIND, code_action};
use crate::analysis::{Analysis, Cancel};
use crate::diagnostics::diagnostic;
use crate::notifier::{DOWNLOAD_ACTION, Notifier, RUNTIME_PROMPT_ID};
use crate::workspace::Workspace;
use anyhow::{Result, anyhow};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    Cancel as CancelRequest, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, Notification as LspNotification,
};
use lsp_types::request::{
    CodeActionRequest, DocumentDiagnosticRequest, Formatting, RegisterCapability,
    Request as LspRequest, ShowDocument, WorkspaceDiagnosticRefresh,
};
use lsp_types::{
    CancelParams, CodeActionKind, CodeActionOptions, CodeActionParams,
    CodeActionProviderCapability, DiagnosticOptions, DiagnosticServerCapabilities,
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWatchedFilesRegistrationOptions, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentDiagnosticParams, DocumentFormattingParams,
    FileSystemWatcher, GlobPattern, InitializeParams, NumberOrString, OneOf, Registration,
    RegistrationParams, ServerCapabilities, ServerInfo, ShowDocumentParams, TextDocumentIdentifier,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri, WatchKind,
};
use provider_api::LanguageProvider;
use serde::Serialize;
use serde::de::DeserializeOwned;
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
pub(crate) fn path_of(uri: &Uri) -> Option<PathBuf> {
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
    notifier: Notifier,
    /// Runs diagnostics and code actions off the message loop.
    analysis: Analysis,
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
    let notifier = Notifier::new(conn.sender.clone());
    let analysis = Analysis::start(conn.sender.clone());
    let mut server = Server {
        conn,
        workspace,
        support,
        notifier,
        analysis,
        next_request_id: 0,
    };
    if let Some(error) = config_error {
        server.notifier.config_failed(error);
    }
    // `initialize_finish` has consumed the client's `initialized` notification.
    server.register_watchers()?;
    let result = server.run();
    server.analysis.finish();
    result
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
                Message::Response(response) => self.response(response)?,
            }
        }
        Ok(())
    }

    /// The only answer that matters is the one to the missing-runtime prompt.
    fn response(&mut self, response: Response) -> Result<()> {
        let chosen = response.id == RequestId::from(RUNTIME_PROMPT_ID.to_string())
            && response
                .response_result
                .as_ref()
                .ok()
                .and_then(|r| r.get("title"))
                .and_then(|t| t.as_str())
                == Some(DOWNLOAD_ACTION);
        if !chosen {
            return Ok(());
        }
        self.call::<ShowDocument>(ShowDocumentParams {
            uri: provider_api::RUNTIME_URL.parse()?,
            external: Some(true),
            take_focus: None,
            selection: None,
        })
    }

    fn send(&self, message: impl Into<Message>) -> Result<()> {
        self.conn
            .sender
            .send(message.into())
            .map_err(|_| anyhow!("the client closed the connection"))
    }

    fn call<R: LspRequest>(&mut self, params: R::Params) -> Result<()> {
        self.next_request_id += 1;
        let id = RequestId::from(self.next_request_id);
        self.send(Request::new(id, R::METHOD.to_string(), params))
    }

    fn request(&mut self, req: Request) -> Result<()> {
        let response = match req.method.as_str() {
            DocumentDiagnosticRequest::METHOD => {
                if self.workspace.is_deep() {
                    self.workspace.load_project();
                }
                return self.queue(
                    req,
                    |p: &DocumentDiagnosticParams| &p.text_document,
                    diagnostic,
                );
            }
            CodeActionRequest::METHOD => {
                return self.queue(req, |p: &CodeActionParams| &p.text_document, code_action);
            }
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

    /// Hands `req` to the analysis thread with a snapshot of the workspace; the thread sends the
    /// response.
    fn queue<P, R>(
        &mut self,
        req: Request,
        document: fn(&P) -> &TextDocumentIdentifier,
        handler: fn(&Workspace, &Notifier, P, &Cancel) -> R,
    ) -> Result<()>
    where
        P: DeserializeOwned + Send + 'static,
        R: Serialize + 'static,
    {
        let params = match serde_json::from_value::<P>(req.params) {
            Ok(params) => params,
            Err(e) => {
                return self.send(Response::new_err(
                    req.id,
                    ErrorCode::InvalidParams as i32,
                    e.to_string(),
                ));
            }
        };
        let path = path_of(&document(&params).uri);
        let (workspace, notifier) = (self.workspace.clone(), self.notifier.clone());
        self.analysis.submit(req.id, path, move |cancel| {
            serde_json::to_value(handler(&workspace, &notifier, params, cancel))
                .unwrap_or(serde_json::Value::Null)
        });
        Ok(())
    }

    fn formatting(&mut self, params: DocumentFormattingParams) -> Option<Vec<lsp_types::TextEdit>> {
        let uri = params.text_document.uri;
        let path = path_of(&uri)?;
        match self.workspace.format(&path) {
            Ok(edits) => Some(edits),
            Err(failure) => {
                self.notifier.failed(&uri, &failure);
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
                    if let Some((language, why)) = self.workspace.missing_language(&path)
                        && !self.workspace.ignores_language(&language)
                    {
                        self.notifier.language_missing(&language, &why);
                    }
                    self.workspace.open(path, doc.version, doc.text);
                }
            }
            DidChangeTextDocument::METHOD => {
                let p: DidChangeTextDocumentParams = serde_json::from_value(note.params)?;
                if let Some(path) = path_of(&p.text_document.uri) {
                    self.analysis.supersede(&path);
                    self.workspace.change(
                        &path,
                        p.text_document.version,
                        &p.content_changes,
                        self.analysis.is_busy(),
                    );
                }
            }
            DidCloseTextDocument::METHOD => {
                let p: DidCloseTextDocumentParams = serde_json::from_value(note.params)?;
                if let Some(path) = path_of(&p.text_document.uri) {
                    self.analysis.supersede(&path);
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
            CancelRequest::METHOD => {
                let p: CancelParams = serde_json::from_value(note.params)?;
                self.analysis.cancel(&match p.id {
                    NumberOrString::Number(n) => RequestId::from(n),
                    NumberOrString::String(s) => RequestId::from(s),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn reload_config(&mut self) -> Result<()> {
        self.notifier.reset();
        if let Some(error) = self.workspace.reload_config() {
            self.notifier.config_failed(error);
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
            watchers: ["**/*.yaml", "**/*.yml", "**/.editorconfig", "**/*.wasm"]
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
