//! Contract tests against the real C# worker (needs the .NET SDK; CI has it).

mod common;

use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, grammars, print, trivia};
use lang_csharp::generated as csharp;
use provider_api::protocol::{Query, Request, UpdateText};
use provider_api::{LanguageProvider, ProviderError, ProxyError, SourceFile, WorkerConfig};
use std::ffi::OsString;

fn provider() -> provider_api::OutOfProcessProvider {
    lang_csharp::provider(&common::worker_dll())
}

const SAMPLE: &str = "using System;\n\nnamespace N // ns\n{\n    /// <summary>Doc</summary>\n    public class Greeter\n    {\n        string s = \"h\u{e9}llo \u{1F642}\"; /* c */\n        void M() { }\n    }\n}\n";

#[test]
fn parses_losslessly_with_the_shared_trivia_rule() {
    let p = provider();
    let result = p.parse(&SourceFile::new("a.cs", 1, SAMPLE)).unwrap();
    assert_eq!(print(&result.tree), SAMPLE);
    assert!(result.errors.is_empty());
    assert_eq!(result.tree.kind(), csharp::kind::COMPILATION_UNIT);
    assert_eq!(
        trivia::check_attachment(&result.tree, |k| grammars::csharp().is_trivia(k)),
        Ok(())
    );
    assert!(p.capabilities().flow && p.capabilities().binding);
}

#[test]
fn generated_wrappers_work_on_worker_trees() {
    let result = provider()
        .parse(&SourceFile::new("a.cs", 1, SAMPLE))
        .unwrap();
    let class = result
        .tree
        .descendants()
        .find_map(csharp::ClassDeclaration::cast)
        .unwrap();
    assert_eq!(class.identifier().unwrap().text(), "Greeter");
    assert_eq!(class.members().count(), 2);
}

#[test]
fn reports_parse_errors_in_byte_offsets_and_keeps_every_byte() {
    let text = "class A { string s = \"\u{e9}\"; int x = ; }";
    let result = provider().parse(&SourceFile::new("b.cs", 2, text)).unwrap();
    assert_eq!(print(&result.tree), text);
    let e = &result.errors[0];
    assert_eq!(e.code, "CS1525");
    let expected = text.find("; }").unwrap() as u32; // bytes, not UTF-16 units
    assert_eq!(e.start, expected);
}

#[test]
fn language_version_is_honored_and_validated() {
    let p = provider();
    let mut file = SourceFile::new("c.cs", 1, "var x = 1;");
    file.language_version = Some("7.3".into());
    assert!(p.parse(&file).is_ok());
    file.language_version = Some("nope".into());
    assert!(
        matches!(p.parse(&file), Err(ProviderError::Worker { code, .. }) if code == "bad_request")
    );
}

#[test]
fn grammar_hash_mismatch_is_refused() {
    let config = WorkerConfig::new(
        vec!["dotnet".into(), common::worker_dll().into_os_string()],
        "stale",
    );
    let p = provider_api::OutOfProcessProvider::new(
        core_syntax::LanguageId::CSHARP,
        grammars::csharp(),
        vec![],
        config,
    );
    let e = p
        .parse(&SourceFile::new("a.cs", 1, "class A {}"))
        .unwrap_err();
    assert!(
        matches!(e, ProviderError::Unavailable(m) if m.contains("refused the handshake") && m.contains("grammar hash"))
    );
}

#[test]
fn semantic_requests_reach_the_workspace() {
    let p = provider();
    let update = UpdateText {
        file_id: "a.cs".into(),
        version: 1,
        text: "class A { int M() { int k = 5; return 0; } }".into(),
    };
    let query = Query {
        file_id: "a.cs".into(),
        version: 1,
        items: vec!["diagnostics".into(), "declarations".into()],
        diagnostic_ids: vec!["CS0219".into()],
    };
    let (answers, graphs) = p.proxy().analyze(update, Some(query), true).unwrap();
    let answers = answers.unwrap();
    assert_eq!(answers.diagnostics.len(), 1);
    assert!(answers.declarations.iter().any(|d| d.name == "M"));
    let graphs = graphs.unwrap();
    assert_eq!(graphs.bodies.len(), 1);
    assert_eq!(graphs.bodies[0].locals[0].name, "k");
    // A query for a file the worker was never given is an error reply; the worker stays healthy.
    let unknown = Query {
        file_id: "nope.cs".into(),
        version: 1,
        items: vec![],
        diagnostic_ids: vec![],
    };
    let e = p.proxy().request(Request::Query(unknown)).unwrap_err();
    assert!(
        matches!(&e, ProxyError::Worker(w) if w.code == "bad_request"),
        "{e}"
    );
    assert!(p.parse(&SourceFile::new("a.cs", 1, "class A {}")).is_ok());
}

#[test]
fn missing_dotnet_assembly_is_unavailable_not_a_panic() {
    let config = WorkerConfig::new(vec!["dotnet".into(), OsString::from("missing.dll")], "h");
    let p = provider_api::OutOfProcessProvider::new(
        core_syntax::LanguageId::CSHARP,
        grammars::csharp(),
        vec![],
        config,
    );
    assert!(matches!(
        p.parse(&SourceFile::new("a.cs", 1, "")),
        Err(ProviderError::Unavailable(_))
    ));
    let _ = SyntaxKind::ERROR;
}
