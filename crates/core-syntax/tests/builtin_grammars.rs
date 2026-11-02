use core_syntax::{LanguageId, grammars};

#[test]
fn builtin_grammars_load() {
    assert_eq!(grammars::json().language_id, LanguageId::JSON);
    assert_eq!(grammars::csharp().language_id, LanguageId::CSHARP);
    assert_eq!(grammars::yaml().language_id, LanguageId::YAML);
}
