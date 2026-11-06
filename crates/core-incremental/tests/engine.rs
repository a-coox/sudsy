//! Memoization, backdating and failure behaviour of the query engine, asserted on salsa's
//! execution events rather than on timing.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{
    ConfigFile, Database, Db, File, Stage, config, format, lint, options, parse,
};
use core_syntax::grammars;
use provider_api::{FileMatcher, LanguageProvider, OutOfProcessProvider, WorkerConfig};
use salsa::{EventKind, Setter};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const UGLY_JSON: &str = "{\"a\":1,\n\"b\":[1,2,3]}";
const UGLY_YAML: &str = "a:   1\nb:\n    - x\n";

struct Harness {
    db: Database,
    log: Arc<Mutex<Vec<String>>>,
}

impl Harness {
    fn new(providers: Vec<Box<dyn LanguageProvider>>) -> Self {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        let db = Database::with_event_hook(
            providers,
            Box::new(move |event| {
                if let EventKind::WillExecute { database_key } = event.kind {
                    sink.lock().unwrap().push(format!("{database_key:?}"));
                }
            }),
        );
        Harness { db, log }
    }

    fn standard() -> Self {
        Self::new(vec![
            Box::new(lang_json::JsonProvider::new()),
            Box::new(lang_yaml::YamlProvider::new()),
        ])
    }

    fn file(&self, path: &str, text: &str) -> File {
        File::new(
            &self.db,
            PathBuf::from(path),
            1,
            text.to_string(),
            false,
            EditorConfig::default(),
        )
    }

    /// How many times each query ran since the last call, e.g. `["parse", "parse", "format"]`
    /// sorted; the ids are dropped.
    fn executed(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .log
            .lock()
            .unwrap()
            .drain(..)
            .map(|k| k.split('(').next().unwrap().to_string())
            .collect();
        names.sort();
        names
    }

    fn count(names: &[String], query: &str) -> usize {
        names.iter().filter(|n| n.as_str() == query).count()
    }

    fn format_ok(&self, file: File) -> String {
        format(&self.db, file).clone().expect("formats")
    }
}

#[test]
fn query_names_are_stable() {
    let h = Harness::standard();
    let f = h.file("a.json", UGLY_JSON);
    h.format_ok(f);
    let names = h.executed();
    for q in [
        "config",
        "selection",
        "options",
        "parse_settings",
        "parse",
        "format",
    ] {
        assert_eq!(Harness::count(&names, q), 1, "{q} in {names:?}");
    }
}

#[test]
fn editing_one_file_reparses_only_that_file() {
    let mut h = Harness::standard();
    let a = h.file("a.json", UGLY_JSON);
    let b = h.file("b.json", UGLY_JSON);
    let y = h.file("c.yaml", UGLY_YAML);
    for f in [a, b, y] {
        h.format_ok(f);
    }
    h.executed();

    a.set_text(&mut h.db).to("{\"a\":2}".to_string());
    for f in [a, b, y] {
        h.format_ok(f);
    }
    let names = h.executed();
    assert_eq!(Harness::count(&names, "parse"), 1, "{names:?}");
    assert_eq!(Harness::count(&names, "format"), 1, "{names:?}");
    for untouched in ["config", "selection", "options", "parse_settings"] {
        assert_eq!(Harness::count(&names, untouched), 0, "{names:?}");
    }
}

#[test]
fn config_change_re_resolves_options_without_reparsing() {
    let mut h = Harness::standard();
    let a = h.file("a.json", UGLY_JSON);
    let y = h.file("b.yaml", UGLY_YAML);
    let before_a = h.format_ok(a);
    h.format_ok(y);
    h.executed();

    h.db.set_config(ConfigChain::parse("json:\n  format:\n    indent_size: 4\n"));
    let after_a = h.format_ok(a);
    h.format_ok(y);
    let names = h.executed();
    assert_eq!(Harness::count(&names, "config"), 1, "{names:?}");
    assert_eq!(Harness::count(&names, "options"), 2, "{names:?}");
    assert_eq!(Harness::count(&names, "parse"), 0, "{names:?}");
    // The yaml options came out equal, so only the json file formats again.
    assert_eq!(Harness::count(&names, "format"), 1, "{names:?}");
    assert_ne!(before_a, after_a);
}

#[test]
fn equal_options_and_equal_config_are_backdated() {
    let mut h = Harness::standard();
    let a = h.file("a.json", UGLY_JSON);
    h.db.set_config(ConfigChain::parse("yaml:\n  format:\n    indent_size: 4\n"));
    h.format_ok(a);
    h.executed();

    // A comment-only edit moves every value down a line, so the config is not equal: options
    // are resolved again, to an equal value, and nothing downstream of them runs.
    h.db.set_config(ConfigChain::parse(
        "# note\nyaml:\n  format:\n    indent_size: 4\n",
    ));
    h.format_ok(a);
    let names = h.executed();
    assert_eq!(names, ["config", "options"], "{names:?}");

    // A change to another language re-resolves this file's options to an equal value: nothing
    // downstream of `options` runs.
    h.db.set_config(ConfigChain::parse("yaml:\n  format:\n    indent_size: 3\n"));
    h.format_ok(a);
    let names = h.executed();
    assert_eq!(names, ["config", "options"], "{names:?}");
}

#[salsa::tracked(returns(copy))]
fn output_len(db: &dyn Db, file: File) -> usize {
    format(db, file).as_ref().map_or(0, String::len)
}

#[test]
fn a_whitespace_edit_with_equal_output_stops_at_the_formatted_text() {
    let mut h = Harness::standard();
    let a = h.file("a.json", "{\n  \"a\": 1\n}\n");
    let len = output_len(&h.db, a);
    h.executed();

    // Different tree (the tree is lossless), same formatted output.
    a.set_text(&mut h.db)
        .to("{\n\n  \"a\": 1\n\n}\n".to_string());
    assert_eq!(output_len(&h.db, a), len);
    let names = h.executed();
    assert_eq!(Harness::count(&names, "parse"), 1, "{names:?}");
    assert_eq!(Harness::count(&names, "format"), 1, "{names:?}");
    // `format` produced an equal string, so its dependent is not re-run.
    assert_eq!(Harness::count(&names, "output_len"), 0, "{names:?}");
}

#[test]
fn a_new_version_of_equal_text_backdates_the_parse() {
    let mut h = Harness::standard();
    let a = h.file("a.json", UGLY_JSON);
    let before = parse(&h.db, a).clone().unwrap();
    h.executed();

    a.set_version(&mut h.db).to(2);
    let after = parse(&h.db, a).clone().unwrap();
    assert_eq!(h.executed(), ["parse"]);
    assert_eq!(before, after);
}

#[test]
fn config_errors_and_option_errors_are_values() {
    let mut h = Harness::standard();
    let a = h.file("a.json", UGLY_JSON);
    h.db.set_config(ConfigChain::parse("json: ["));
    let failure = config(&h.db).as_ref().unwrap_err();
    assert_eq!(failure.stage, Stage::Config);
    assert!(failure.message.starts_with("1:"), "{failure}");
    assert_eq!(format(&h.db, a).as_ref().unwrap_err().stage, Stage::Config);

    // Option names and values are checked together with the config.
    h.db.set_config(ConfigChain::parse("json:\n  format:\n    nope: 1\n"));
    let failure = config(&h.db).as_ref().unwrap_err();
    assert_eq!(failure.stage, Stage::Config);
    assert!(failure.message.starts_with("3:11:"), "{failure}");
    assert!(
        failure.message.contains("format option `nope`: unknown"),
        "{failure}"
    );
    assert_eq!(options(&h.db, a).as_ref().unwrap_err().stage, Stage::Config);
}

#[test]
fn syntax_errors_are_values() {
    let h = Harness::standard();
    let a = h.file("a.json", "{\"a\": ");
    let failure = format(&h.db, a).as_ref().unwrap_err();
    assert_eq!(failure.stage, Stage::Format);
}

fn unreachable_worker() -> Box<dyn LanguageProvider> {
    Box::new(OutOfProcessProvider::new(
        grammars::CSHARP,
        vec![FileMatcher::extension("cs")],
        WorkerConfig::new(vec!["sudsy-no-such-worker-binary".into()], ""),
    ))
}

#[test]
fn a_worker_failure_is_an_error_value_and_is_memoized() {
    let mut h = Harness::new(vec![unreachable_worker()]);
    let f = h.file("Program.cs", "class C { }");

    let failure = parse(&h.db, f).as_ref().unwrap_err();
    assert_eq!(failure.stage, Stage::Runtime);
    assert!(!failure.message.is_empty());
    let formatted = format(&h.db, f).as_ref().unwrap_err();
    assert_eq!(formatted, failure, "format reports the parse failure");
    assert_eq!(
        Harness::count(&h.executed(), "parse"),
        1,
        "failure memoized"
    );

    // Setting the file again (a new version) retries.
    f.set_version(&mut h.db).to(2);
    parse(&h.db, f).as_ref().unwrap_err();
    assert_eq!(Harness::count(&h.executed(), "parse"), 1);
}

#[test]
fn unsupported_files_fail_cleanly() {
    let h = Harness::standard();
    assert!(!h.db.supports(&PathBuf::from("a.txt")));
    let f = h.file("a.txt", "x");
    assert_eq!(
        format(&h.db, f).as_ref().unwrap_err().stage,
        Stage::Unsupported
    );
}

#[test]
fn handles_on_other_threads_share_the_memos() {
    let h = Harness::standard();
    let files: Vec<File> = (0..16)
        .map(|i| h.file(&format!("f{i}.json"), UGLY_JSON))
        .collect();
    let outputs: Vec<String> = std::thread::scope(|s| {
        let workers: Vec<_> = files
            .chunks(4)
            .map(|chunk| {
                let db = h.db.clone();
                s.spawn(move || {
                    chunk
                        .iter()
                        .map(|f| format(&db, *f).clone().unwrap())
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    assert!(outputs.iter().all(|o| o == &outputs[0]));
    let names = h.executed();
    assert_eq!(Harness::count(&names, "parse"), 16, "{names:?}");
    // Same results when asked again through the original handle, without executing anything.
    format(&h.db, files[3]).as_ref().unwrap();
    assert!(h.executed().is_empty());
    let _ = ConfigFile::get(&h.db);
}

#[test]
fn lint_reads_parse_and_rule_settings_only() {
    let mut h = Harness::standard();
    let f = h.file("a.json", "{\"b\": 1, \"a\": 2}");
    let count = |d: &[core_rules::Diagnostic]| d.iter().filter(|d| d.rule == "sorted-keys").count();
    assert_eq!(count(&lint(&h.db, f).as_ref().unwrap().diagnostics), 0);
    let names = h.executed();
    assert_eq!(Harness::count(&names, "lint"), 1, "{names:?}");
    assert_eq!(Harness::count(&names, "parse"), 1, "{names:?}");

    // Asked again: nothing runs.
    lint(&h.db, f).as_ref().unwrap();
    assert!(h.executed().is_empty());

    // A format option changes the config but not the rule settings: the rules do not run again.
    h.db.set_config(ConfigChain::parse("json:\n  format:\n    indent_size: 4\n"));
    lint(&h.db, f).as_ref().unwrap();
    let names = h.executed();
    assert_eq!(Harness::count(&names, "lint"), 0, "{names:?}");

    // A rule level does.
    h.db.set_config(ConfigChain::parse(
        "lint:\n  rules:\n    sorted-keys: warn\n",
    ));
    let linted = lint(&h.db, f).as_ref().unwrap();
    assert_eq!(count(&linted.diagnostics), 1);
    let names = h.executed();
    assert_eq!(Harness::count(&names, "lint"), 1, "{names:?}");
    assert_eq!(Harness::count(&names, "parse"), 0, "{names:?}");
}

#[test]
fn rule_config_errors_and_syntax_errors_are_lint_values() {
    let mut h = Harness::standard();
    let bad = h.file("bad.json", "{\"a\": }");
    let linted = lint(&h.db, bad).as_ref().unwrap();
    assert!(!linted.parses);
    assert_eq!(linted.diagnostics[0].rule, "syntax-error");

    let good = h.file("a.json", "{}");
    h.db.set_config(ConfigChain::parse(
        "lint:\n  rules:\n    sorted-keys:\n      level: warn\n      options:\n        order: 1\n",
    ));
    let failure = lint(&h.db, good).as_ref().unwrap_err();
    assert_eq!(failure.stage, Stage::Config);
    assert!(failure.message.contains("must be a list of strings"));
}

mod extension {
    use core_format::Directive;
    use core_rules::{
        Category, ExtensionRule, Extensions, LayoutRole, Rule, RuleContext, RuleMeta,
    };
    use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

    static META: RuleMeta = RuleMeta {
        id: "every-array",
        category: Category::Style,
        description: "Reports every array.",
        example: "",
        default_level: core_config::Level::Warn,
        fix: None,
        requires: &[],
        uses: &[],
        options: &[],
    };

    pub struct EveryArray(pub Vec<SyntaxKind>);

    impl Rule for EveryArray {
        fn meta(&self) -> &'static RuleMeta {
            &META
        }
        fn kinds(&self) -> &[SyntaxKind] {
            &self.0
        }
        fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
            ctx.report(TextRange::empty(node.text_range().start()), "an array");
        }
    }

    static SPREAD: RuleMeta = RuleMeta {
        id: "spread-arrays",
        ..META_LAYOUT
    };
    const META_LAYOUT: RuleMeta = RuleMeta {
        id: "",
        category: Category::Style,
        description: "Lays every array out one item per line.",
        example: "",
        default_level: core_config::Level::Warn,
        fix: None,
        requires: &[],
        uses: &[],
        options: &[],
    };

    /// A layout-only rule.
    pub struct SpreadArrays(pub Vec<SyntaxKind>);

    impl Rule for SpreadArrays {
        fn meta(&self) -> &'static RuleMeta {
            &SPREAD
        }
        fn layout_role(&self) -> LayoutRole {
            LayoutRole::Only
        }
        fn kinds(&self) -> &[SyntaxKind] {
            &self.0
        }
        fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
            ctx.attach(node, Directive::BreakAll);
        }
    }

    pub fn layout_set(rule: &'static SpreadArrays) -> Extensions {
        Extensions {
            rules: vec![ExtensionRule {
                language: "json",
                rule,
            }],
            files: Vec::new(),
        }
    }

    pub fn set(rule: &'static EveryArray) -> Extensions {
        Extensions {
            rules: vec![ExtensionRule {
                language: "json",
                rule,
            }],
            files: Vec::new(),
        }
    }
}

#[test]
fn extension_rules_run_with_the_providers_and_reload_only_when_they_change() {
    let mut h = Harness::standard();
    let f = h.file("a.json", "[1]");
    let array = grammars::json()
        .kind_by_name("Array")
        .expect("an Array kind");
    let rule: &'static extension::EveryArray =
        Box::leak(Box::new(extension::EveryArray(vec![array])));
    let fired = |h: &Harness| {
        lint(&h.db, f)
            .as_ref()
            .unwrap()
            .diagnostics
            .iter()
            .filter(|d| d.rule == "every-array")
            .count()
    };
    assert_eq!(fired(&h), 0);
    h.executed();

    h.db.set_extensions(extension::set(rule));
    assert_eq!(fired(&h), 1);
    assert_eq!(Harness::count(&h.executed(), "lint"), 1);

    // The same rule object again: nothing is invalidated.
    h.db.set_extensions(extension::set(rule));
    assert_eq!(fired(&h), 1);
    assert!(h.executed().is_empty());

    // The config's level reaches it like any rule's.
    h.db.set_config(ConfigChain::parse(
        "lint:\n  rules:\n    every-array: \"off\"\n",
    ));
    assert_eq!(fired(&h), 0);
}

#[test]
fn format_depends_on_the_layout_rules_and_not_on_the_rest_of_lint() {
    let mut h = Harness::standard();
    let f = h.file(
        "a.json",
        "{\"a\": [1, 2]}
",
    );
    let array = grammars::json()
        .kind_by_name("Array")
        .expect("an Array kind");
    let rule: &'static extension::SpreadArrays =
        Box::leak(Box::new(extension::SpreadArrays(vec![array])));
    assert_eq!(
        h.format_ok(f),
        "{ \"a\": [1, 2] }
"
    );

    h.db.set_extensions(extension::layout_set(rule));
    assert_eq!(
        h.format_ok(f),
        "{
  \"a\": [
    1,
    2
  ]
}
"
    );
    h.executed();

    // A rule level that is no layout rule's: the layout is re-read but unchanged, so `format`
    // does not run again.
    h.db.set_config(ConfigChain::parse(
        "lint:
  rules:
    sorted-keys: warn
",
    ));
    h.format_ok(f);
    let names = h.executed();
    assert_eq!(Harness::count(&names, "format"), 0, "{names:?}");
    assert_eq!(Harness::count(&names, "lint"), 0, "{names:?}");

    // The layout rule's own level does.
    h.db.set_config(ConfigChain::parse(
        "lint:
  rules:
    spread-arrays: \"off\"
",
    ));
    assert_eq!(
        h.format_ok(f),
        "{ \"a\": [1, 2] }
"
    );
    let names = h.executed();
    assert_eq!(Harness::count(&names, "format"), 1, "{names:?}");
}
