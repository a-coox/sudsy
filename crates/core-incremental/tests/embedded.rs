//! Code of one language inside another: linted with its own rules (positions, suppressions and
//! config in the host file), formatted by its own formatter when the host asks.

use core_config::{ConfigChain, EditorConfig};
use core_incremental::{Database, File, format, lint};
use provider_api::LanguageProvider;
use std::path::PathBuf;

fn db(config: &str) -> Database {
    let providers: Vec<Box<dyn LanguageProvider>> = vec![
        Box::new(lang_html::provider()),
        Box::new(lang_json::JsonProvider::new()),
        Box::new(lang_js::JsProvider::new()),
        Box::new(lang_css::CssProvider::new(lang_css::Dialect::Css)),
        Box::new(lang_css::CssProvider::new(lang_css::Dialect::Scss)),
        Box::new(lang_yaml::YamlProvider::new()),
        Box::new(lang_markdown::provider()),
        Box::new(lang_dockerfile::provider()),
        Box::new(lang_shell::provider()),
    ];
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(config));
    db
}

fn file(db: &Database, path: &str, text: &str) -> File {
    File::new(
        db,
        PathBuf::from(path),
        1,
        text.into(),
        false,
        EditorConfig::default(),
    )
}

/// `(rule, text at the start of its range)` of the file's diagnostics.
fn found(db: &Database, path: &str, text: &str) -> Vec<(String, String)> {
    let linted = lint(db, file(db, path, text)).as_ref().unwrap();
    let at = |d: &core_rules::Diagnostic| {
        text[usize::from(d.range.start())..]
            .chars()
            .take(4)
            .collect()
    };
    linted
        .diagnostics
        .iter()
        .map(|d| (d.rule.clone(), at(d)))
        .collect()
}

fn formatted(db: &Database, path: &str, text: &str) -> String {
    format(db, file(db, path, text)).as_ref().unwrap().clone()
}

#[test]
fn run_is_linted_as_shell_at_host_positions() {
    let dockerfile = "dockerfile:\n  lint:\n    rules:\n      use-workdir-not-cd: off\n";
    let db = db(dockerfile);
    let text = "FROM a:1\nRUN echo hi && \\\n  # why\n  cd /x; make\n";
    assert_eq!(
        found(&db, "Dockerfile", text),
        [("cd-without-exit".into(), "cd /".into())]
    );
    // The body of a heredoc is shell too, for the shell's rules and the Dockerfile's.
    let text = "FROM a:1\nRUN <<EOT\nsudo make\ncd /x\nEOT\n";
    assert_eq!(
        found(&db, "Dockerfile", text),
        [
            ("no-sudo".into(), "RUN ".into()),
            ("cd-without-exit".into(), "cd /".into())
        ]
    );
    // The Dockerfile's suppression comment silences the shell's rule.
    let text = "FROM a:1\n# sudsy-disable-next-line cd-without-exit\nRUN cd /x; make\n";
    assert!(found(&db, "Dockerfile", text).is_empty());
    // The language's own config section applies.
    let db = self::db(&format!(
        "{dockerfile}shell:\n  lint:\n    rules:\n      cd-without-exit: off\n"
    ));
    assert!(found(&db, "Dockerfile", "FROM a:1\nRUN cd /x; make\n").is_empty());
}

#[test]
fn shell_form_cmd_and_backtick_continuations_are_shell() {
    let db = db("dockerfile:\n  lint:\n    rules:\n      use-workdir-not-cd: off\n");
    for text in [
        "FROM a:1\nCMD cd /x; make\n",
        "FROM a:1\nENTRYPOINT cd /x; make\n",
        "FROM a:1\nHEALTHCHECK CMD cd /x; make\n",
        "# escape=`\nFROM a:1\nCMD echo a `\n  && cd /x; make\n",
    ] {
        let rules = found(&db, "Dockerfile", text);
        assert!(
            rules.iter().any(|(r, _)| r == "cd-without-exit"),
            "{text}: {rules:?}"
        );
    }
    // The exec form is not shell; a backtick continuation is not a substitution.
    for text in [
        "FROM a:1\nCMD [\"cd\", \"/x\"]\n",
        "# escape=`\nFROM a:1\nRUN echo a `\n  && echo b `\n  && echo c\n",
    ] {
        let rules = found(&db, "Dockerfile", text);
        assert!(
            rules.iter().all(|(r, _)| r != "cd-without-exit"),
            "{rules:?}"
        );
    }
}

#[test]
fn fences_are_linted_with_their_language() {
    let db = db("");
    let text = "# T\n\n- item\n\n  ```json\n  {\"a\": 1, \"a\": 2}\n  ```\n\n```text\n{\"a\": 1, \"a\": 2}\n```\n";
    let found = found(&db, "a.md", text);
    assert_eq!(
        found,
        [("no-duplicate-keys".into(), "\"a\":".into())],
        "{found:?}"
    );
}

#[test]
fn fences_are_formatted_by_their_language_unless_off() {
    let md = "- item\n\n  ```json\n  {\"a\":1,\"b\":[1,2]}\n  ```\n\n> ```yaml\n> a:   1\n> ```\n\n```json\n{\"a\":\n```\n\n```ruby\nx   =  1\n```\n";
    let want = "- item\n\n  ```json\n  { \"a\": 1, \"b\": [1, 2] }\n  ```\n\n> ```yaml\n> a: 1\n> ```\n\n```json\n{\"a\":\n```\n\n```ruby\nx   =  1\n```\n";
    let once = formatted(&db(""), "a.md", md);
    assert_eq!(once, want);
    assert_eq!(formatted(&db(""), "a.md", &once), once);
    let off = "markdown:\n  format:\n    embedded_language_formatting: off\n";
    assert_eq!(
        formatted(&db(off), "a.md", md),
        formatted(&db(off), "a.md", &formatted(&db(off), "a.md", md))
    );
    assert!(formatted(&db(off), "a.md", md).contains("{\"a\":1,\"b\":[1,2]}"));
}

#[test]
fn js_ts_and_css_fences_are_formatted_and_linted() {
    let md = "```javascript\nlet a={b:1}\n```\n\n```tsx\nconst x:number=1\n```\n\n```scss\na{b:c}\n```\n";
    let want = "```javascript\nlet a = { b: 1 };\n```\n\n```tsx\nconst x: number = 1;\n```\n\n```scss\na {\n  b: c;\n}\n```\n";
    assert_eq!(formatted(&db(""), "a.md", md), want);
    let on = "js:\n  lint:\n    rules:\n      no-debugger: error\n";
    let linted = found(&db(on), "a.md", "```ts\ndebugger;\n```\n");
    assert_eq!(linted, [("no-debugger".into(), "debu".into())]);
}

#[test]
fn html_script_style_and_attributes_are_formatted_by_their_language_unless_off() {
    let html = "<div style=\"color:red;margin:0\" onclick=\"f( 1,2 )\"></div>
<script>
const a = {b:1}
</script>
<style>
a{color:red}
</style>
<script type=\"text/x\">  a  </script>
<script>if (</script>
";
    let want = "<div style=\"color: red; margin: 0\" onclick=\"f(1, 2)\"></div>
<script>
  const a = { b: 1 };
</script>
<style>
  a {
    color: red;
  }
</style>
<script type=\"text/x\">
  a
</script>
<script>
  if (
</script>
";
    let once = formatted(&db(""), "a.html", html);
    assert_eq!(once, want);
    assert_eq!(formatted(&db(""), "a.html", &once), once);
    let off = "html:
  format:
    embedded_language_formatting: off
";
    assert!(formatted(&db(off), "a.html", html).contains("const a = {b:1}"));
}

#[test]
fn vue_templates_scripts_and_custom_blocks_are_formatted_by_their_language() {
    let mut providers: Vec<Box<dyn LanguageProvider>> = vec![Box::new(lang_html::vue_provider())];
    providers.extend([
        Box::new(lang_js::JsProvider::new()) as Box<dyn LanguageProvider>,
        Box::new(lang_json::JsonProvider::new()),
    ]);
    let mut db = Database::new(providers);
    db.set_config(ConfigChain::parse(
        "vue:\n  format:\n    indent_script_and_style: false\n",
    ));
    let vue = "<template><Foo :a=\"{b:1}\" @click=\"x=1\" v-for=\"(a,i) in l\">{{a|b}} {{ a+b }}</Foo></template>\n<script>\nlet a={}\n</script>\n<i18n lang=\"json\">{\"a\":1}</i18n>\n";
    let want = "<template>\n  <Foo :a=\"{ b: 1 }\" @click=\"x = 1\" v-for=\"(a, i) in l\"\n    >{{ a | b }} {{ a + b }}</Foo\n  >\n</template>\n<script>\nlet a = {};\n</script>\n<i18n lang=\"json\">\n{ \"a\": 1 }\n</i18n>\n";
    let once = formatted(&db, "a.vue", vue);
    assert_eq!(once, want);
    assert_eq!(formatted(&db, "a.vue", &once), once);
}
