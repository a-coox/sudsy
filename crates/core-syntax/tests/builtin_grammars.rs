use core_syntax::ast::AstNode;
use core_syntax::generated::{csharp, json, yaml};
use core_syntax::{LanguageId, SyntaxNode, TreeBuilder, grammars, print, trivia};

#[test]
fn builtin_grammars_load_and_match_generated_hashes() {
    assert_eq!(grammars::json().hash(), json::GRAMMAR_HASH);
    assert_eq!(grammars::csharp().hash(), csharp::GRAMMAR_HASH);
    assert_eq!(grammars::yaml().hash(), yaml::GRAMMAR_HASH);
    assert_eq!(grammars::json().language_id, LanguageId::JSON);
    assert_eq!(grammars::csharp().language_id, LanguageId::CSHARP);
}

#[test]
fn generated_kinds_agree_with_the_loader() {
    let g = grammars::json();
    assert_eq!(g.kind_by_name("Member"), Some(json::kind::MEMBER));
    assert_eq!(g.kind_by_name("Comma"), Some(json::kind::COMMA));
    let g = grammars::csharp();
    assert_eq!(
        g.kind_by_name("MethodDeclaration"),
        Some(csharp::kind::METHOD_DECLARATION)
    );
    assert!(g.is_trivia(csharp::kind::WHITESPACE_TRIVIA));
    assert!(!g.is_trivia(csharp::kind::IDENTIFIER_TOKEN));
}

/// `{ "a": 1 }` with the trivia placed per the attachment rule.
fn sample() -> SyntaxNode {
    use json::kind::*;
    let mut b = TreeBuilder::new();
    b.start_node(DOCUMENT);
    b.start_node(OBJECT);
    b.token(L_BRACE, "{");
    b.token(WHITESPACE, " ");
    b.start_node(MEMBER);
    b.token(STRING, "\"a\"");
    b.token(COLON, ":");
    b.token(WHITESPACE, " ");
    b.start_node(LITERAL);
    b.token(NUMBER, "1");
    b.token(WHITESPACE, " ");
    b.finish_node();
    b.finish_node();
    b.token(R_BRACE, "}");
    b.finish_node();
    b.token(EOF, "");
    b.finish_node();
    SyntaxNode::new_root(b.finish())
}

#[test]
fn typed_wrappers_navigate_the_tree() {
    let root = sample();
    assert_eq!(print(&root), "{ \"a\": 1 }");
    let doc = json::Document::cast(root).unwrap();
    let value = doc.value().unwrap();
    let object = json::Object::cast(value.syntax().clone()).unwrap();
    let members: Vec<_> = object.members().collect();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].key().unwrap().text(), "\"a\"");
    let literal = json::Literal::cast(members[0].value().unwrap().syntax().clone()).unwrap();
    assert_eq!(literal.value().unwrap().text(), "1");
    assert!(json::Array::cast(object.syntax().clone()).is_none());
}

#[test]
fn trivia_attachment_rule_holds_for_the_sample() {
    assert_eq!(
        trivia::check_attachment(&sample(), |k| grammars::json().is_trivia(k)),
        Ok(())
    );
}
