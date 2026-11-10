//! The commands of a recipe are linted as Shell at their positions in the Makefile.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, lint};
use provider_api::LanguageProvider;
use std::path::PathBuf;

/// `(rule, text at the start of its range)` of the Makefile's diagnostics.
fn found(text: &str) -> Vec<(String, String)> {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_makefile::provider()),
        Box::new(lang_shell::provider()),
    ];
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(""));
    let file = File::new(
        &db,
        PathBuf::from("Makefile"),
        1,
        text.into(),
        false,
        EditorConfig::default(),
    );
    let linted = lint(&db, file).as_ref().unwrap();
    linted
        .diagnostics
        .iter()
        .map(|d| {
            let at = &text[usize::from(d.range.start())..];
            (d.rule.clone(), at.chars().take(4).collect())
        })
        .collect()
}

#[test]
fn commands_are_shell_without_make_syntax() {
    let text =
        "all:\n\t@cd /x; make $(FLAGS) $@\n\t-echo $$HOME $(shell date) \\\n\t  && cd /y; make\n";
    assert_eq!(
        found(text),
        [
            ("cd-without-exit".into(), "cd /".into()),
            ("cd-without-exit".into(), "cd /".into())
        ]
    );
    // The Makefile's suppression comment silences the shell's rule.
    let text = "all:\n# sudsy-disable-next-line cd-without-exit\n\tcd /x; make\n";
    assert!(found(text).is_empty());
}
