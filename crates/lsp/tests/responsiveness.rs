//! The message loop stays free while a diagnostics pull is running (plan.md "LSP server",
//! docs/decisions.md "Editor responsiveness"). A rule that blocks until the test releases it makes
//! the pull slow without a sleep.

mod common;

use common::Client;
use core_config::Level;
use core_format::NodeFormatter;
use core_rules::{Category, FileContext, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, LanguageId};
use lsp_types::request::DocumentDiagnosticRequest;
use lsp_types::{DocumentDiagnosticParams, TextDocumentIdentifier};
use provider_api::{
    Capabilities, FileMatcher, LanguageProvider, ParseResult, ProviderError, SourceFile,
};
use std::collections::HashMap;
use std::sync::{Condvar, Mutex};

#[derive(Default)]
struct Gate {
    /// How many times the rule started on the file.
    entered: usize,
    released: bool,
}

static GATES: Mutex<Option<HashMap<String, Gate>>> = Mutex::new(None);
static CHANGED: Condvar = Condvar::new();

fn gates() -> std::sync::MutexGuard<'static, Option<HashMap<String, Gate>>> {
    let mut g = GATES.lock().unwrap();
    g.get_or_insert_with(HashMap::new);
    g
}

/// Files named `name` block in the rule until [`release`].
fn arm(name: &str) {
    gates()
        .as_mut()
        .unwrap()
        .insert(name.into(), Gate::default());
}

fn release(name: &str) {
    gates().as_mut().unwrap().get_mut(name).unwrap().released = true;
    CHANGED.notify_all();
}

/// Waits until the rule has started on the file `times` times.
fn wait_entered(name: &str, times: usize) {
    let mut g = gates();
    while g.as_ref().unwrap()[name].entered < times {
        g = CHANGED.wait(g).unwrap();
    }
}

fn entered(name: &str) -> usize {
    gates().as_ref().unwrap()[name].entered
}

struct Blocker;

static BLOCKER: RuleMeta = RuleMeta {
    id: "blocker",
    category: Category::Style,
    description: "Blocks until the test says so.",
    example: "",
    default_level: Level::Warn,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for Blocker {
    fn meta(&self) -> &'static RuleMeta {
        &BLOCKER
    }

    fn check_file(&self, file: &FileContext<'_>, _ctx: &mut RuleContext<'_>) {
        let name = file.path.file_name().unwrap().to_string_lossy().to_string();
        let mut g = gates();
        let Some(gate) = g.as_mut().unwrap().get_mut(&name) else {
            return;
        };
        gate.entered += 1;
        CHANGED.notify_all();
        while !g.as_ref().unwrap()[&name].released {
            g = CHANGED.wait(g).unwrap();
        }
    }
}

static RULES: &[&dyn Rule] = &[&Blocker];

/// JSON with one more rule.
struct Gated(lang_json::JsonProvider);

impl LanguageProvider for Gated {
    fn id(&self) -> LanguageId {
        self.0.id()
    }
    fn grammar(&self) -> &Grammar {
        self.0.grammar()
    }
    fn file_matchers(&self) -> &[FileMatcher] {
        self.0.file_matchers()
    }
    fn capabilities(&self) -> Capabilities {
        self.0.capabilities()
    }
    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        self.0.parse(file)
    }
    fn formatter(&self) -> &dyn NodeFormatter {
        self.0.formatter()
    }
    fn option_schema(&self) -> &core_config::OptionSchema {
        self.0.option_schema()
    }
    fn rules(&self) -> &'static [&'static dyn Rule] {
        RULES
    }
}

fn start(dir: &std::path::Path) -> Client {
    Client::start(dir, vec![Box::new(Gated(lang_json::JsonProvider::new()))])
}

fn pull(client: &mut Client, uri: &lsp_types::Uri) -> lsp_server::RequestId {
    client.send_request::<DocumentDiagnosticRequest>(DocumentDiagnosticParams {
        text_document: TextDocumentIdentifier::new(uri.clone()),
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    })
}

const REQUEST_CANCELLED: i32 = -32800;
const SERVER_CANCELLED: i32 = -32802;

#[test]
fn formatting_is_answered_while_a_pull_is_running() {
    let t = tempfile::tempdir().unwrap();
    let mut client = start(t.path());
    arm("format.json");
    let uri = client.open("format.json", "json", "{\"a\":1}");
    let slow = pull(&mut client, &uri);
    wait_entered("format.json", 1);

    // The pull is stuck in the rule; formatting is not behind it.
    assert!(!client.format(&uri).is_empty());
    // Not even when the document was edited in the meantime.
    client.change(&uri, 2, None, "{\"b\":2}");
    assert!(!client.format(&uri).is_empty());

    release("format.json");
    // The pull asked about version 1: the server drops it and the client asks again.
    let (code, data) = client.response(&slow).unwrap_err();
    assert_eq!(code, SERVER_CANCELLED);
    assert_eq!(data.unwrap()["retriggerRequest"], true);
    assert!(client.diagnostics(&uri).is_empty());
    client.shutdown();
}

#[test]
fn a_cancelled_pull_is_answered_with_request_cancelled() {
    let t = tempfile::tempdir().unwrap();
    let mut client = start(t.path());
    arm("cancel.json");
    let uri = client.open("cancel.json", "json", "{}");
    let slow = pull(&mut client, &uri);
    wait_entered("cancel.json", 1);
    let queued = pull(&mut client, &uri);
    client.cancel(&slow);
    client.cancel(&queued);
    // A request the loop answers itself: by then it has read the notifications before it.
    client.format(&uri);
    release("cancel.json");
    assert_eq!(client.response(&slow).unwrap_err().0, REQUEST_CANCELLED);
    assert_eq!(client.response(&queued).unwrap_err().0, REQUEST_CANCELLED);
    // The queued pull never started its rules.
    assert_eq!(entered("cancel.json"), 1);
    // The server is still fine.
    assert!(client.diagnostics(&uri).is_empty());
    client.shutdown();
}

#[test]
fn a_newer_version_drops_the_pulls_of_the_old_one() {
    let t = tempfile::tempdir().unwrap();
    let mut client = start(t.path());
    arm("newer.json");
    let uri = client.open("newer.json", "json", "{}");
    let running = pull(&mut client, &uri);
    wait_entered("newer.json", 1);
    let queued = pull(&mut client, &uri);
    client.change(&uri, 2, None, "{\"a\": ");
    client.format(&uri);
    release("newer.json");
    assert_eq!(client.response(&running).unwrap_err().0, SERVER_CANCELLED);
    assert_eq!(client.response(&queued).unwrap_err().0, SERVER_CANCELLED);
    assert_eq!(entered("newer.json"), 1);

    // A pull for the new version sees the new text.
    let found = client.diagnostics(&uri);
    assert!(!found.is_empty());
    client.shutdown();
}
