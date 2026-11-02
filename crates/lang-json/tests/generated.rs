//! The generated typed wrappers agree with the grammar loader and navigate a parsed tree.

use core_syntax::ast::AstNode;
use core_syntax::grammars;
use lang_json::JsonProvider;
use lang_json::generated::{self as json, kind};
use provider_api::{LanguageProvider, SourceFile};

#[test]
fn generated_code_matches_the_grammar() {
    let g = grammars::json();
    assert_eq!(g.hash(), json::GRAMMAR_HASH);
    assert_eq!(g.kind_by_name("Member"), Some(kind::MEMBER));
    assert_eq!(g.kind_by_name("Comma"), Some(kind::COMMA));
}

#[test]
fn typed_wrappers_navigate_the_tree() {
    let parsed = JsonProvider::new()
        .parse(&SourceFile::new("a.json", 1, "{ \"a\": 1 }"))
        .unwrap();
    let doc = json::Document::cast(parsed.tree).unwrap();
    let object = json::Object::cast(doc.value().unwrap().syntax().clone()).unwrap();
    let members: Vec<_> = object.members().collect();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].key().unwrap().text(), "\"a\"");
    let literal = json::Literal::cast(members[0].value().unwrap().syntax().clone()).unwrap();
    assert_eq!(literal.value().unwrap().text(), "1");
    assert!(json::Array::cast(object.syntax().clone()).is_none());
}
