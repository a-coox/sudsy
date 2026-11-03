//! `lsp-latency CORPUS_DIR WORKER_DLL [FILES] [REPEATS]`: how responsive the language server is
//! while it analyses a C# file. Runs the real server
//! on an in-process connection with the real C# worker, on `FILES` (default 300) files of the
//! corpus as the workspace and the one of about 20 KB among them as the open document.
//!
//! * first build: the first diagnostics pull builds the workspace's compilation; how long does a
//!   formatting request, sent right after it, take to be answered?
//! * typing: with the compilation warm, the client types (an edit every 30 ms) as
//!   vscode-languageclient does: each edit cancels the pull in flight and asks for a new one.
//!   How long after the last edit does the answer to the last pull arrive?

use anyhow::{Result, anyhow, bail};
use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

struct Client {
    conn: Connection,
    server: Option<std::thread::JoinHandle<anyhow::Result<()>>>,
    next: i32,
    /// When each answered request was answered, and whether with a result.
    done: HashMap<i32, (Instant, bool)>,
}

impl Client {
    fn start(root: &Path, worker: &Path) -> Client {
        let (server_end, conn) = Connection::memory();
        let providers: Vec<Box<dyn provider_api::LanguageProvider>> =
            vec![Box::new(lang_csharp::provider(worker))];
        let server = std::thread::spawn(move || lsp::serve(server_end, providers));
        let mut client = Client {
            conn,
            server: Some(server),
            next: 0,
            done: HashMap::new(),
        };
        let root = url::Url::from_directory_path(root).unwrap().to_string();
        let id = client.request(
            "initialize",
            json!({"capabilities": {}, "workspaceFolders": [{"uri": root, "name": "root"}]}),
        );
        client.wait(id);
        client.notify("initialized", json!({}));
        client
    }

    fn request(&mut self, method: &str, params: Value) -> i32 {
        self.next += 1;
        let id = self.next;
        let request = Request::new(RequestId::from(id), method.into(), params);
        self.conn.sender.send(request.into()).unwrap();
        id
    }

    fn notify(&self, method: &str, params: Value) {
        let note = Notification::new(method.into(), params);
        self.conn.sender.send(note.into()).unwrap();
    }

    /// Reads what the server sent for up to `time`.
    fn pump(&mut self, time: Duration) {
        let end = Instant::now() + time;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match self.conn.receiver.recv_timeout(left) {
                Ok(Message::Response(Response {
                    id,
                    response_result,
                })) => {
                    let id: i32 = id.to_string().parse().unwrap();
                    self.done
                        .insert(id, (Instant::now(), response_result.is_ok()));
                }
                Ok(Message::Request(r)) => {
                    let ok = Response::new_ok(r.id, Value::Null);
                    self.conn.sender.send(ok.into()).unwrap();
                }
                Ok(Message::Notification(_)) => {}
                Err(_) => break,
            }
        }
    }

    /// Waits for the answer to `id` and returns when it came.
    fn wait(&mut self, id: i32) -> Instant {
        let give_up = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some((at, _)) = self.done.get(&id) {
                return *at;
            }
            assert!(Instant::now() < give_up, "no answer to request {id}");
            self.pump(Duration::from_millis(5));
        }
    }

    fn open(&self, path: &Path, text: &str) -> String {
        let uri = url::Url::from_file_path(path).unwrap().to_string();
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "csharp", "version": 1, "text": text}}),
        );
        uri
    }

    fn pull(&mut self, uri: &str) -> i32 {
        self.request(
            "textDocument/diagnostic",
            json!({"textDocument": {"uri": uri}}),
        )
    }

    fn format(&mut self, uri: &str) -> i32 {
        self.request(
            "textDocument/formatting",
            json!({"textDocument": {"uri": uri}, "options": {"tabSize": 4, "insertSpaces": true}}),
        )
    }

    fn shutdown(mut self) {
        let id = self.request("shutdown", Value::Null);
        self.wait(id);
        self.notify("exit", Value::Null);
        let _ = self.server.take().unwrap().join();
    }
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "cs") {
            out.push(path);
        }
    }
    Ok(())
}

const CONFIG: &str = "analysis: standard
lint:
  rules:
    unreachable-code: warn
    unused-variable: warn
    unused-assignment: warn
    remove-unused-usings: warn
    possible-null-dereference: warn
    unused-private-member: warn
";

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The workspace on disk and the file that is open in it.
fn workspace(corpus: &Path, files: usize) -> Result<(PathBuf, PathBuf, String)> {
    let mut all = Vec::new();
    collect(corpus, &mut all)?;
    let step = (all.len() / files).max(1);
    let root = std::env::temp_dir().join(format!("sudsy-lsp-latency-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("sudsy.yaml"), CONFIG)?;
    let mut target: Option<(u64, PathBuf, String)> = None;
    for (i, source) in all.iter().step_by(step).take(files).enumerate() {
        let Ok(text) = std::fs::read_to_string(source) else {
            continue;
        };
        let copy = root.join(format!("F{i}.cs"));
        std::fs::write(&copy, &text)?;
        let distance = (text.len() as i64 - 20_000).unsigned_abs();
        if target.as_ref().is_none_or(|(d, ..)| distance < *d) {
            target = Some((distance, copy, text));
        }
    }
    let (_, path, text) =
        target.ok_or_else(|| anyhow!("no .cs files under {}", corpus.display()))?;
    Ok((root, path, text))
}

fn first_build(root: &Path, worker: &Path, file: &Path, text: &str) -> (f64, f64) {
    let mut client = Client::start(root, worker);
    // The editor has the workspace open for a moment before the user touches a file.
    client.pump(Duration::from_millis(1500));
    let uri = client.open(file, text);
    let sent = Instant::now();
    let pull = client.pull(&uri);
    let format = client.format(&uri);
    let format_at = client.wait(format);
    let pull_at = client.wait(pull);
    client.shutdown();
    (ms(format_at - sent), ms(pull_at - sent))
}

/// Typing at one edit per `interval` for `keys` edits; returns the time from the last edit to the
/// answer to the last pull, and how many pulls came back with a result.
fn typing(root: &Path, worker: &Path, file: &Path, text: &str) -> (f64, usize, usize) {
    const KEYS: i32 = 40;
    let interval = Duration::from_millis(30);
    let mut client = Client::start(root, worker);
    let uri = client.open(file, text);
    let warm = client.pull(&uri);
    client.wait(warm);
    let mut pulls = Vec::new();
    let mut last_edit = Instant::now();
    for version in 2..2 + KEYS {
        client.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
                    "text": "\n"
                }]
            }),
        );
        last_edit = Instant::now();
        if let Some(&previous) = pulls.last()
            && !client.done.contains_key(&previous)
        {
            client.notify("$/cancelRequest", json!({"id": previous}));
        }
        pulls.push(client.pull(&uri));
        client.pump(interval);
    }
    let answered = client.wait(*pulls.last().unwrap());
    let with_result = pulls
        .iter()
        .filter(|id| client.done.get(id).is_some_and(|d| d.1))
        .count();
    client.shutdown();
    (ms(answered - last_edit), with_result, pulls.len())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [corpus, worker, rest @ ..] = args.as_slice() else {
        bail!("usage: lsp-latency CORPUS_DIR WORKER_DLL [FILES] [REPEATS]");
    };
    let files: usize = rest.first().map_or(Ok(300), |s| s.parse())?;
    let repeats: usize = rest.get(1).map_or(Ok(3), |s| s.parse())?;
    let (worker, (root, file, text)) = (Path::new(worker), workspace(Path::new(corpus), files)?);
    println!(
        "{} files in the workspace, open document {} KB",
        files,
        text.len() / 1000
    );
    for run in 1..=repeats {
        let (format, pull) = first_build(&root, worker, &file, &text);
        println!(
            "first build, run {run}: format answered after {format:.0} ms, pull after {pull:.0} ms"
        );
    }
    for run in 1..=repeats {
        let (last, ok, total) = typing(&root, worker, &file, &text);
        println!(
            "typing, run {run}: last pull answered {last:.0} ms after the last edit ({ok} of {total} pulls returned diagnostics)"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
    Ok(())
}
