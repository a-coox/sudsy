use core_syntax::grammars;
use lang_csharp::generated as csharp;

#[test]
fn generated_hash_matches_the_grammar() {
    assert_eq!(grammars::csharp().hash(), csharp::GRAMMAR_HASH);
}

#[test]
fn generated_kinds_agree_with_the_loader() {
    let g = grammars::csharp();
    assert_eq!(
        g.kind_by_name("MethodDeclaration"),
        Some(csharp::kind::METHOD_DECLARATION)
    );
    assert!(g.is_trivia(csharp::kind::WHITESPACE_TRIVIA));
    assert!(!g.is_trivia(csharp::kind::IDENTIFIER_TOKEN));
}
