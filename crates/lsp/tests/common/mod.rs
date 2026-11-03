//! An in-process LSP client: the server runs on a thread at the other end of an in-memory
//! connection, and the tests drive it with the same messages an editor sends.

#![allow(dead_code)]

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    Initialized,
};
use lsp_types::request::{
    CodeActionRequest, DocumentDiagnosticRequest, Formatting, Initialize, Shutdown,
};
use lsp_types::{
    ClientCapabilities, CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    Diagnostic, DiagnosticWorkspaceClientCapabilities, DidChangeTextDocumentParams,
    DidChangeWatchedFilesClientCapabilities, DidChangeWatchedFilesParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, DocumentFormattingParams,
    FileChangeType, FileEvent, FormattingOptions, InitializeParams, InitializedParams, Position,
    Range, TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, TextEdit, Uri,
    VersionedTextDocumentIdentifier, WorkspaceClientCapabilities, WorkspaceEdit, WorkspaceFolder,
};
use provider_api::LanguageProvider;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(60);

pub struct Client {
    conn: Connection,
    server: Option<JoinHandle<anyhow::Result<()>>>,
    next_id: i32,
    /// Notifications from the server not yet looked at, as `(method, params)`.
    pub notes: Vec<(String, serde_json::Value)>,
    /// Requests the server made (registrations, refreshes), as method names.
    pub server_requests: Vec<String>,
    pub root: PathBuf,
}

pub fn uri(path: &Path) -> Uri {
    Uri::from_str(url::Url::from_file_path(path).unwrap().as_str()).unwrap()
}

impl Client {
    /// Starts a server for `root` with the given providers and completes the handshake. The
    /// client announces that it can register file watchers and refresh diagnostics.
    pub fn start(root: &Path, providers: Vec<Box<dyn LanguageProvider>>) -> Client {
        let (server_end, conn) = Connection::memory();
        let server = std::thread::spawn(move || lsp::serve(server_end, providers));
        let mut client = Client {
            conn,
            server: Some(server),
            next_id: 0,
            notes: Vec::new(),
            server_requests: Vec::new(),
            root: root.to_path_buf(),
        };
        let params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: uri(root),
                name: "root".into(),
            }]),
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
                        dynamic_registration: Some(true),
                        ..Default::default()
                    }),
                    diagnostic: Some(DiagnosticWorkspaceClientCapabilities {
                        refresh_support: Some(true),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        client.request::<Initialize>(params);
        client.notify::<Initialized>(InitializedParams {});
        client
    }

    fn send(&self, message: impl Into<Message>) {
        self.conn.sender.send(message.into()).unwrap();
    }

    pub fn notify<N: lsp_types::notification::Notification>(&self, params: N::Params)
    where
        N::Params: Serialize,
    {
        self.send(Notification::new(N::METHOD.to_string(), params));
    }

    /// Sends a request and returns its result, collecting what the server sends meanwhile.
    pub fn request<R: lsp_types::request::Request>(&mut self, params: R::Params) -> R::Result
    where
        R::Params: Serialize,
        R::Result: DeserializeOwned,
    {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.send(Request::new(id.clone(), R::METHOD.to_string(), params));
        loop {
            match self
                .conn
                .receiver
                .recv_timeout(TIMEOUT)
                .expect("the server answers")
            {
                Message::Response(Response {
                    id: got,
                    response_result,
                }) if got == id => {
                    return match response_result {
                        Ok(value) => serde_json::from_value(value).unwrap(),
                        Err(e) => panic!("{} failed: {}", R::METHOD, e.message),
                    };
                }
                Message::Response(_) => {}
                Message::Notification(n) => self.notes.push((n.method, n.params)),
                Message::Request(r) => {
                    self.server_requests.push(r.method);
                    self.send(Response::new_ok(r.id, serde_json::Value::Null));
                }
            }
        }
    }

    /// Sends a request without waiting for the answer; see [`Client::response`].
    pub fn send_request<R: lsp_types::request::Request>(&mut self, params: R::Params) -> RequestId
    where
        R::Params: Serialize,
    {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.send(Request::new(id.clone(), R::METHOD.to_string(), params));
        id
    }

    /// `$/cancelRequest` for a request sent with [`Client::send_request`].
    pub fn cancel(&self, id: &RequestId) {
        let number: i32 = id.to_string().parse().unwrap();
        self.notify::<lsp_types::notification::Cancel>(lsp_types::CancelParams {
            id: lsp_types::NumberOrString::Number(number),
        });
    }

    /// The answer to `id`: the result, or the error's code and data.
    pub fn response(
        &mut self,
        id: &RequestId,
    ) -> Result<serde_json::Value, (i32, Option<serde_json::Value>)> {
        loop {
            match self
                .conn
                .receiver
                .recv_timeout(TIMEOUT)
                .expect("the server answers")
            {
                Message::Response(Response {
                    id: got,
                    response_result,
                }) if &got == id => {
                    return response_result.map_err(|e| (e.code, e.data));
                }
                Message::Response(_) => {}
                Message::Notification(n) => self.notes.push((n.method, n.params)),
                Message::Request(r) => {
                    self.server_requests.push(r.method);
                    self.send(Response::new_ok(r.id, serde_json::Value::Null));
                }
            }
        }
    }

    /// Reads messages until a notification of `method` arrives (or one is waiting already).
    pub fn wait_for(&mut self, method: &str) -> serde_json::Value {
        if let Some(i) = self.notes.iter().position(|(m, _)| m == method) {
            return self.notes.remove(i).1;
        }
        loop {
            match self
                .conn
                .receiver
                .recv_timeout(TIMEOUT)
                .expect("a notification arrives")
            {
                Message::Notification(n) if n.method == method => return n.params,
                Message::Notification(n) => self.notes.push((n.method, n.params)),
                Message::Request(r) => {
                    self.server_requests.push(r.method);
                    self.send(Response::new_ok(r.id, serde_json::Value::Null));
                }
                Message::Response(_) => {}
            }
        }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn open(&self, name: &str, language: &str, text: &str) -> Uri {
        let uri = uri(&self.path(name));
        self.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(uri.clone(), language.into(), 1, text.into()),
        });
        uri
    }

    pub fn change(&self, uri: &Uri, version: i32, range: Option<Range>, text: &str) {
        self.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri.clone(), version),
            content_changes: vec![TextDocumentContentChangeEvent {
                range,
                range_length: None,
                text: text.into(),
            }],
        });
    }

    pub fn close(&self, uri: &Uri) {
        self.notify::<DidCloseTextDocument>(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri.clone()),
        });
    }

    /// The config or `.editorconfig` at `name` changed on disk.
    pub fn file_changed(&self, name: &str) {
        self.notify::<DidChangeWatchedFiles>(DidChangeWatchedFilesParams {
            changes: vec![FileEvent::new(
                uri(&self.path(name)),
                FileChangeType::CHANGED,
            )],
        });
    }

    pub fn diagnostics(&mut self, uri: &Uri) -> Vec<Diagnostic> {
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier::new(uri.clone()),
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        match self.request::<DocumentDiagnosticRequest>(params) {
            DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(r)) => {
                r.full_document_diagnostic_report.items
            }
            other => panic!("unexpected report {other:?}"),
        }
    }

    pub fn code_actions(
        &mut self,
        uri: &Uri,
        range: Range,
        only: Option<Vec<CodeActionKind>>,
    ) -> Vec<lsp_types::CodeAction> {
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier::new(uri.clone()),
            range,
            context: CodeActionContext {
                diagnostics: vec![],
                only,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        self.request::<CodeActionRequest>(params)
            .unwrap_or_default()
            .into_iter()
            .map(|a| match a {
                CodeActionOrCommand::CodeAction(a) => a,
                CodeActionOrCommand::Command(c) => panic!("unexpected command {c:?}"),
            })
            .collect()
    }

    pub fn format(&mut self, uri: &Uri) -> Vec<TextEdit> {
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier::new(uri.clone()),
            options: FormattingOptions::default(),
            work_done_progress_params: Default::default(),
        };
        self.request::<Formatting>(params).unwrap_or_default()
    }

    pub fn shutdown(mut self) {
        self.request::<Shutdown>(());
        self.notify::<lsp_types::notification::Exit>(());
        self.server.take().unwrap().join().unwrap().unwrap();
    }
}

/// `text` with the edits applied (as LSP defines it: all against the original text).
pub fn apply(text: &str, edits: &[TextEdit]) -> String {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let offset = |p: Position| -> usize {
        let before: usize = lines.iter().take(p.line as usize).map(|l| l.len()).sum();
        let line = lines.get(p.line as usize).copied().unwrap_or("");
        let mut units = 0;
        let mut at = line.len();
        for (i, c) in line.char_indices() {
            if units >= p.character {
                at = i;
                break;
            }
            units += c.len_utf16() as u32;
        }
        before + at
    };
    let mut ordered: Vec<&TextEdit> = edits.iter().collect();
    ordered.sort_by_key(|e| std::cmp::Reverse((e.range.start.line, e.range.start.character)));
    let mut out = text.to_string();
    for e in ordered {
        out.replace_range(offset(e.range.start)..offset(e.range.end), &e.new_text);
    }
    out
}

/// The text after the edits of a code action.
pub fn apply_action(text: &str, uri: &Uri, edit: &WorkspaceEdit) -> String {
    apply(text, &edit.changes.as_ref().unwrap()[uri])
}
