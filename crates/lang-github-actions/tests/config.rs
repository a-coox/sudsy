//! A workflow keeps the `yaml:` section and the YAML rules; `github-actions:` goes on top.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, lint};
use lang_github_actions::GithubActionsProvider;
use provider_api::LanguageProvider;
use std::path::PathBuf;

fn rules(config: &str) -> Vec<String> {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_yaml::YamlProvider::new()),
        Box::new(GithubActionsProvider::new()),
    ];
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(config));
    let path = PathBuf::from(".github/workflows/a.yml");
    let text = "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n";
    let file = File::new(&db, path, 1, text.into(), false, EditorConfig::default());
    let linted = lint(&db, file).as_ref().unwrap();
    linted.diagnostics.iter().map(|d| d.rule.clone()).collect()
}

#[test]
fn the_yaml_section_applies_and_the_workflow_section_overrides() {
    let on = "yaml:\n  lint:\n    rules:\n      document-start: warn\n";
    assert_eq!(rules(on), ["document-start"]);
    let off = format!("{on}github-actions:\n  lint:\n    rules:\n      document-start: \"off\"\n");
    assert!(rules(&off).is_empty());
}
