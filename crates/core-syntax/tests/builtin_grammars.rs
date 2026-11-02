use core_syntax::generated::csharp;
use core_syntax::{LanguageId, grammars};

#[test]
fn builtin_grammars_load_and_match_generated_hashes() {
    assert_eq!(grammars::csharp().hash(), csharp::GRAMMAR_HASH);
    assert_eq!(grammars::json().language_id, LanguageId::JSON);
    assert_eq!(grammars::csharp().language_id, LanguageId::CSHARP);
    assert_eq!(grammars::yaml().language_id, LanguageId::YAML);
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
