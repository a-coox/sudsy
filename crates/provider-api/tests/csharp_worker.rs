//! Contract tests against the real C# worker (needs the .NET SDK; CI has it).

mod common;

use core_syntax::ast::AstNode;
use core_syntax::generated::csharp;
use core_syntax::{SyntaxKind, grammars, print, trivia};
use provider_api::protocol::{FlowGraphs, OpenWorkspace, Query, Request, UpdateText};
use provider_api::{
    Capabilities, LanguageProvider, ProviderError, ProxyError, SourceFile, WorkerConfig,
};
use std::ffi::OsString;

fn provider() -> provider_api::OutOfProcessProvider {
    provider_api::csharp::provider(&common::worker_dll())
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
    assert_eq!(p.capabilities(), Capabilities::NONE);
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
fn later_milestone_messages_are_unsupported() {
    let p = provider();
    let requests = [
        Request::OpenWorkspace(OpenWorkspace::default()),
        Request::UpdateText(UpdateText {
            file_id: "a".into(),
            version: 1,
            text: String::new(),
        }),
        Request::Query(Query {
            file_id: "a".into(),
            version: 1,
            batch: vec![],
        }),
        Request::FlowGraphs(FlowGraphs {
            file_id: "a".into(),
            version: 1,
        }),
    ];
    for r in requests {
        let e = p.proxy().request(r).unwrap_err();
        assert!(
            matches!(&e, ProxyError::Worker(w) if w.code == "unsupported"),
            "{e}"
        );
    }
    // The worker stays healthy afterwards.
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
