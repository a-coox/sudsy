use core_config::{ConfigError, Level, OptionValue, discover, load_file, parse_str};

const PLAN_EXAMPLE: &str = r#"
extends:
  - preset:csharp/microsoft@1
  - preset:yaml/default@1

format:
  line_width: 100

lint:
  rules:
    max-line-length: warn

plugins:
  patterns: ["rules/*.yaml"]
  wasm: ["rules/team-rules.wasm"]

csharp:
  format:
    line_width: 120
    brace_newline: all
    trailing_commas: multiline
    member_chain_min_calls: 3
  lint:
    rules:
      no-empty-catch: error
      max-line-length: "off"
      team/no-direct-datetime:
        level: error
        options:
          allowed_in: ["**/SystemClock.cs"]

yaml:
  lint:
    rules:
      ambiguous-booleans: warn

overrides:
  - files: ["tests/**"]
    csharp:
      lint:
        rules:
          async-suffix: "off"
"#;

#[test]
fn parses_the_structure_from_the_plan() {
    let c = parse_str(PLAN_EXAMPLE).unwrap();
    assert_eq!(c.extends.len(), 2);
    assert_eq!(c.extends[0].value, "preset:csharp/microsoft@1");
    assert_eq!(c.format["line_width"], OptionValue::Int(100));
    assert_eq!(c.lint.rules["max-line-length"].level, Level::Warn);
    assert_eq!(c.plugins.wasm, ["rules/team-rules.wasm"]);
    assert_eq!(c.csharp.format["line_width"], OptionValue::Int(120));
    assert_eq!(
        c.csharp.format["brace_newline"],
        OptionValue::String("all".into())
    );
    assert_eq!(c.csharp.lint.rules["max-line-length"].level, Level::Off);
    let plugin = &c.csharp.lint.rules["team/no-direct-datetime"];
    assert_eq!(plugin.level, Level::Error);
    assert_eq!(
        plugin.options["allowed_in"],
        OptionValue::List(vec![OptionValue::String("**/SystemClock.cs".into())])
    );
    assert_eq!(c.overrides[0].files.value, ["tests/**"]);
    assert_eq!(
        c.overrides[0].csharp.lint.rules["async-suffix"].level,
        Level::Off
    );
}

#[test]
fn yaml_1_2_keeps_yes_no_on_off_as_strings() {
    let c =
        parse_str("format:\n  a: yes\n  b: no\n  c: on\n  d: off\n  e: true\n  f: [yes, off]\n")
            .unwrap();
    let s = |v: &str| OptionValue::String(v.into());
    assert_eq!(c.format["a"], s("yes"));
    assert_eq!(c.format["b"], s("no"));
    assert_eq!(c.format["c"], s("on"));
    assert_eq!(c.format["d"], s("off"));
    assert_eq!(c.format["e"], OptionValue::Bool(true));
    assert_eq!(c.format["f"], OptionValue::List(vec![s("yes"), s("off")]));
    // An unquoted `off` is a valid rule level precisely because it is a string.
    let c = parse_str("lint:\n  rules:\n    x: off\n").unwrap();
    assert_eq!(c.lint.rules["x"].level, Level::Off);
}

#[test]
fn empty_or_comment_only_files_are_the_default_config() {
    assert_eq!(parse_str("").unwrap(), Default::default());
    assert_eq!(parse_str("# nothing yet\n\n").unwrap(), Default::default());
}

fn error_of(src: &str) -> ConfigError {
    parse_str(src).unwrap_err()
}

#[test]
fn errors_carry_locations() {
    let e = error_of("format:\n  line_width: 100\nfrmat: {}\n");
    assert_eq!(e.line, Some(3), "{e}");
    assert!(e.message.contains("frmat"), "{e}");

    let e = error_of("lint:\n  rules:\n    no-empty-catch: loud\n");
    assert_eq!(e.line, Some(3), "{e}");
    assert!(e.message.contains("loud"), "{e}");

    let e = error_of("lint:\n  rules:\n    x:\n      level: warn\n      extra: 1\n");
    assert_eq!(e.line, Some(5), "{e}");

    let e = error_of("csharp:\n  lint:\n    rules:\n      x:\n        options: {}\n");
    assert!(e.message.contains("level"), "{e}");

    let e = error_of("overrides:\n  - files: []\n");
    assert_eq!((e.line, e.column), (Some(2), Some(12)), "{e}");

    let e = error_of("extends:\n  - \"\"\n");
    assert_eq!(e.line, Some(2), "{e}");

    let e = error_of("format: [unclosed\n");
    assert!(e.line.is_some(), "{e}");
}

#[test]
fn display_includes_path_and_position() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sudsy.yaml");
    std::fs::write(&path, "bogus: 1\n").unwrap();
    let e = load_file(&path).unwrap_err();
    assert!(
        e.to_string().starts_with(&format!("{}:1:", path.display())),
        "{e}"
    );
}

#[test]
fn discovery_walks_up_and_rejects_ambiguity() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(dir.path().join("sudsy.yml"), "").unwrap();
    assert_eq!(
        discover(&nested).unwrap(),
        Some(dir.path().join("sudsy.yml"))
    );
    std::fs::write(dir.path().join("sudsy.yaml"), "").unwrap();
    assert!(discover(&nested).unwrap_err().to_string().contains("both"));
}

#[test]
fn duplicate_keys_are_rejected() {
    let e = parse_str("format:\n  a: 1\n  a: 2\n").unwrap_err();
    assert_eq!(e.line, Some(3), "{e}");
}
