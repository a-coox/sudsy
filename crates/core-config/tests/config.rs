use core_config::{
    ConfigChain, ConfigError, FileInput, Level, OptionDef, OptionSchema, OptionValue, Preset,
    Source, discover, load_file, parse_str, shared,
};
use std::path::Path;

const PLAN_EXAMPLE: &str = r#"
extends:
  - preset:csharp/microsoft
  - preset:yaml/default

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
    assert_eq!(c.extends[0].value, "preset:csharp/microsoft");
    assert_eq!(c.format["line_width"].value, OptionValue::Int(100));
    assert_eq!(c.lint.rules["max-line-length"].value.level, Level::Warn);
    assert_eq!(c.plugins.wasm[0].value, "rules/team-rules.wasm");
    assert_eq!(
        c.language_section("csharp").unwrap().format["line_width"].value,
        OptionValue::Int(120)
    );
    assert_eq!(
        c.language_section("csharp").unwrap().format["brace_newline"].value,
        OptionValue::String("all".into())
    );
    assert_eq!(
        c.language_section("csharp").unwrap().lint.rules["max-line-length"]
            .value
            .level,
        Level::Off
    );
    let plugin = &c.language_section("csharp").unwrap().lint.rules["team/no-direct-datetime"].value;
    assert_eq!(plugin.level, Level::Error);
    assert_eq!(
        plugin.options["allowed_in"],
        OptionValue::List(vec![OptionValue::String("**/SystemClock.cs".into())])
    );
    assert_eq!(c.overrides[0].files.value[0].value, "tests/**");
    assert_eq!(
        c.overrides[0]
            .language_section("csharp")
            .unwrap()
            .lint
            .rules["async-suffix"]
            .value
            .level,
        Level::Off
    );
}

#[test]
fn every_top_level_key_parses() {
    let c = parse_str(
        "extends: [base.yaml]\ninclude: [\"src/**\"]\nexclude: [\"**/*.g.cs\"]\nanalysis: deep\n",
    )
    .unwrap();
    assert_eq!(c.include[0].value, "src/**");
    assert_eq!(c.exclude[0].value, "**/*.g.cs");
    assert_eq!(c.depth(), core_config::Depth::Deep);
    assert_eq!(parse_str("").unwrap().depth(), core_config::Depth::Standard);
}

#[test]
fn rule_options_are_stored_as_data() {
    let c = parse_str(
        "lint:\n  rules:\n    x:\n      level: warn\n      options:\n        n: 3\n        nested:\n          a: [1, two]\n",
    )
    .unwrap();
    let options = &c.lint.rules["x"].value.options;
    assert_eq!(options["n"], OptionValue::Int(3));
    assert!(matches!(options["nested"], OptionValue::Map(_)));
}

#[test]
fn yaml_1_2_keeps_yes_no_on_off_as_strings() {
    let c =
        parse_str("format:\n  a: yes\n  b: no\n  c: on\n  d: off\n  e: true\n  f: [yes, off]\n")
            .unwrap();
    let s = |v: &str| OptionValue::String(v.into());
    assert_eq!(c.format["a"].value, s("yes"));
    assert_eq!(c.format["b"].value, s("no"));
    assert_eq!(c.format["c"].value, s("on"));
    assert_eq!(c.format["d"].value, s("off"));
    assert_eq!(c.format["e"].value, OptionValue::Bool(true));
    assert_eq!(
        c.format["f"].value,
        OptionValue::List(vec![s("yes"), s("off")])
    );
    // An unquoted `off` is a valid rule level precisely because it is a string.
    let c = parse_str("lint:\n  rules:\n    x: off\n").unwrap();
    assert_eq!(c.lint.rules["x"].value.level, Level::Off);
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

    let e = error_of("exclude:\n  - \"[unclosed\"\n");
    assert_eq!(e.line, Some(2), "{e}");
    assert!(e.message.contains("glob"), "{e}");

    let e = error_of("lint:\n  rules:\n    Not_A_Rule: warn\n");
    assert_eq!(e.line, Some(3), "{e}");
    assert!(e.message.contains("rule id"), "{e}");

    let e = error_of("extends:\n  - preset:csharp\n");
    assert_eq!(e.line, Some(2), "{e}");
    assert!(e.message.contains("preset name"), "{e}");

    let e = error_of("analysis: thorough\n");
    assert_eq!(e.line, Some(1), "{e}");
}

#[test]
fn unknown_keys_get_a_suggestion() {
    let e = error_of("formt:\n  line_width: 100\n");
    assert!(e.message.contains("did you mean `format`"), "{e}");
    assert_eq!((e.line, e.column), (Some(1), Some(1)), "{e}");
    let e = error_of("csharp:\n  lnt: {}\n");
    assert!(e.message.contains("did you mean `lint`"), "{e}");
    assert_eq!(e.line, Some(2), "{e}");
    // Nothing close: the known keys are listed, without a guess.
    let e = error_of("zzzzzz: 1\n");
    assert!(!e.message.contains("did you mean"), "{e}");
    assert!(e.message.contains("known keys"), "{e}");
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

// ---------------------------------------------------------------- chains of files

fn write(dir: &Path, name: &str, text: &str) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn extends_loads_files_relative_to_the_extending_file() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    write(d, "base/shared.yaml", "format:\n  line_width: 90\n");
    write(
        d,
        "sudsy.yaml",
        "extends:\n  - preset:json/narrow\n  - base/shared.yaml\nformat:\n  indent_size: 3\n",
    );
    let chain = ConfigChain::load(&d.join("sudsy.yaml")).unwrap();
    assert_eq!(chain.files.len(), 2);
    assert!(
        chain.files[0]
            .file
            .as_ref()
            .unwrap()
            .ends_with("shared.yaml")
    );
    assert_eq!(chain.presets.len(), 1);
    let resolved = chain
        .format_options(&FileInput::language("json"), &schema())
        .unwrap();
    // The extending file's `format` beats the base's; the base's beats the preset.
    assert_eq!(resolved.options.int("indent_size"), 3);
    assert_eq!(resolved.options.int("line_width"), 90);
    assert!(
        matches!(&resolved.sources["line_width"], Source::Config { section, .. } if section == "format")
    );
}

#[test]
fn extends_errors_point_at_the_entry() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    write(d, "sudsy.yaml", "extends:\n  - missing.yaml\n");
    let e = ConfigChain::load(&d.join("sudsy.yaml")).unwrap_err();
    assert_eq!(e.line, Some(2), "{e}");
    assert!(e.message.contains("not found"), "{e}");
    assert!(e.path.is_some());

    write(d, "a.yaml", "extends: [b.yaml]\n");
    write(d, "b.yaml", "extends: [a.yaml]\n");
    let e = ConfigChain::load(&d.join("a.yaml")).unwrap_err();
    assert!(e.message.contains("extends itself"), "{e}");

    write(d, "bad.yaml", "format: {}\nbogus: 1\n");
    write(d, "top.yaml", "extends: [bad.yaml]\n");
    let e = ConfigChain::load(&d.join("top.yaml")).unwrap_err();
    assert!(e.path.unwrap().ends_with("bad.yaml"));
}

// ---------------------------------------------------------------- resolution

const FULL: &str = "extends: [preset:json/narrow]
lint:
  rules:
    r1: { level: warn, options: { n: 2 } }
json:
  format:
    mode: c
overrides:
  - files: [\"tests/**\"]
    lint:
      rules:
        r1: error
    format:
      line_width: 40
";

fn schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(80),
        shared::indent_size(2),
        OptionDef::choice("mode", &["a", "b", "c"], "a", "A mode."),
    ])
    .with_presets(vec![
        Preset::new(
            "json/narrow",
            "format:
  line_width: 60
",
        ),
        Preset::new("json/full", FULL),
        Preset::new(
            "json/modal",
            "json:
  format:
    mode: b
",
        ),
        Preset::new(
            "json/data",
            "extends: [preset:json/narrow]
json:
  format:
    line_width: 70
    mode: c
",
        ),
        Preset::new(
            "json/undeclared",
            "json:
  format:
    no_such_option: true
",
        ),
        Preset::new(
            "json/loop-a",
            "extends: [preset:json/loop-b]
",
        ),
        Preset::new(
            "json/loop-b",
            "extends: [preset:json/loop-a]
",
        ),
    ])
}

fn resolve(config: &str, path: Option<&Path>) -> core_config::Resolved {
    let chain = ConfigChain::parse(config).unwrap();
    let mut input = FileInput::language("json");
    if let Some(p) = path {
        input = input.with_path(p);
    }
    chain.format_options(&input, &schema()).unwrap()
}

#[test]
fn language_section_beats_shared_beats_preset_beats_default() {
    let r = resolve(
        "extends: [preset:json/narrow, preset:json/modal]\nformat:\n  line_width: 100\n  indent_size: 4\n",
        None,
    );
    assert_eq!(r.options.int("line_width"), 100);
    assert_eq!(r.options.int("indent_size"), 4);
    assert_eq!(r.options.str("mode"), "b");
    assert_eq!(r.sources["mode"], Source::Preset("json/modal".into()));

    // The language section is `x:` for a language called x; csharp/json/yaml are the real ones.
    let chain =
        ConfigChain::parse("format:\n  line_width: 100\njson:\n  format:\n    line_width: 90\n")
            .unwrap();
    let line_width = |language| {
        chain
            .format_options(&FileInput::language(language), &schema())
            .unwrap()
            .options
            .int("line_width")
    };
    assert_eq!(line_width("json"), 90);
    assert_eq!(line_width("yaml"), 100);
}

#[test]
fn presets_of_other_languages_are_ignored_and_the_later_preset_wins() {
    let r = resolve("extends: [preset:csharp/other, preset:json/narrow]\n", None);
    assert_eq!(r.options.int("line_width"), 60);
}

#[test]
fn overrides_apply_by_glob_and_beat_the_language_section() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "sudsy.yaml",
        "format:\n  line_width: 100\njson:\n  format:\n    line_width: 90\noverrides:\n  - files: [\"tests/**\"]\n    format:\n      line_width: 50\n  - files: [\"**/*.snap.json\"]\n    json:\n      format:\n        indent_size: 8\n",
    );
    let chain = ConfigChain::load(&dir.path().join("sudsy.yaml")).unwrap();
    let at = |language: &str, rel: &str| {
        let path = dir.path().join(rel);
        chain
            .format_options(&FileInput::language(language).with_path(&path), &schema())
            .unwrap()
    };
    // Shared option in an override beats the language section.
    assert_eq!(at("json", "tests/a.json").options.int("line_width"), 50);
    assert_eq!(at("json", "src/a.json").options.int("line_width"), 90);
    assert_eq!(at("json", "src/a.snap.json").options.int("indent_size"), 8);
    // A language section in an override only applies to that language.
    assert_eq!(at("yaml", "src/a.snap.json").options.int("indent_size"), 2);
    let source = at("json", "tests/a.json").sources["line_width"].to_string();
    assert!(source.contains("overrides[0].format"), "{source}");
}

#[test]
fn editorconfig_sits_below_the_config_chain_and_above_defaults() {
    let dir = Path::new("/repo");
    let ec = core_config::EditorConfig::from_text(
        dir,
        "[*]\nindent_size = 7\nmax_line_length = 77\n",
        &dir.join("a.x"),
    );
    let chain = ConfigChain::parse("extends: [preset:json/narrow]\n").unwrap();
    let r = chain
        .format_options(
            &FileInput::language("json").with_editorconfig(&ec),
            &schema(),
        )
        .unwrap();
    assert_eq!(r.options.int("indent_size"), 7);
    assert!(matches!(
        r.sources["indent_size"],
        Source::EditorConfig { .. }
    ));
    // The preset beats .editorconfig.
    assert_eq!(r.options.int("line_width"), 60);
    // A value the option rejects is ignored.
    let ec =
        core_config::EditorConfig::from_text(dir, "[*]\nmax_line_length = 5\n", &dir.join("a.x"));
    let r = ConfigChain::default()
        .format_options(
            &FileInput::language("json").with_editorconfig(&ec),
            &schema(),
        )
        .unwrap();
    assert_eq!(r.options.int("line_width"), 80);
}

#[test]
fn validation_checks_presets_and_option_names_and_values() {
    let schemas = [("json", &schema())];
    let check = |config: &str| ConfigChain::parse(config).unwrap().validate(&schemas);
    assert!(check("format: {}\n").is_ok());
    let e = check("json:\n  format:\n    mode: z\n").unwrap_err();
    assert!(e.message.contains("one of: a, b, c"), "{e}");
    assert_eq!(e.line, Some(3));
    let e = check("extends: [preset:json/nope]\n").unwrap_err();
    assert!(e.message.contains("json/narrow"), "{e}");
    assert_eq!(e.line, Some(1));
    let e = check("extends: [preset:zzz/nope]\n").unwrap_err();
    assert!(e.message.contains("preset language"), "{e}");
    let e = check("format:\n  mode: a\n").unwrap_err();
    assert!(
        e.message.contains("not a shared option") || e.message.contains("unknown shared"),
        "{e}"
    );
    assert_eq!(e.line, Some(2));
    let e = check("format:\n  line_width: 5\n").unwrap_err();
    assert!(e.message.contains("between 20 and 1000"), "{e}");
    assert_eq!(e.line, Some(2));
    let e = check("overrides:\n  - files: [a]\n    format:\n      line_widht: 90\n").unwrap_err();
    assert!(e.message.contains("shared option"), "{e}");
    assert_eq!(e.line, Some(4));
    assert!(check("format:\n  line_width: 100\n  final_newline: false\n").is_ok());
}

#[test]
fn rules_merge_by_tier_and_keep_options_when_only_the_level_changes() {
    let chain = ConfigChain::parse(
        "lint:\n  rules:\n    a: warn\n    b:\n      level: error\n      options: { n: 1 }\nx:\n  lint: {}\n",
    );
    // `x` is not a language: the config is rejected when checked against the providers, not
    // silently ignored.
    let e = chain
        .unwrap()
        .validate(&[("csharp", &schema())])
        .unwrap_err();
    assert!(e.message.contains("unknown language section `x`"), "{e}");
    assert_eq!(e.line, Some(8), "{e}");

    let chain = ConfigChain::parse(
        "lint:\n  rules:\n    a: warn\n    b:\n      level: error\n      options: { n: 1 }\ncsharp:\n  lint:\n    rules:\n      a: \"off\"\n      b: warn\noverrides:\n  - files: [\"tests/**\"]\n    csharp:\n      lint:\n        rules:\n          a: error\n",
    )
    .unwrap();
    let rules = chain
        .rules(&FileInput::language("csharp"), &schema())
        .unwrap();
    assert_eq!(rules["a"].level, Level::Off);
    assert_eq!(rules["b"].level, Level::Warn);
    assert_eq!(rules["b"].options["n"], OptionValue::Int(1));
    assert!(rules["a"].source.to_string().contains("csharp.lint.rules"));
    let json = chain
        .rules(&FileInput::language("json"), &schema())
        .unwrap();
    assert_eq!(json["a"].level, Level::Warn);
    let path = Path::new("tests/x.cs");
    let in_tests = chain
        .rules(&FileInput::language("csharp").with_path(path), &schema())
        .unwrap();
    // Relative paths without a config directory are matched as written.
    assert_eq!(in_tests["a"].level, Level::Error);
}

#[test]
fn include_and_exclude_filter_paths() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "sudsy.yaml",
        "include: [\"src/**\", \"*.json\"]\nexclude: [\"**/*.g.cs\", \"src/gen\"]\n",
    );
    let chain = ConfigChain::load(&dir.path().join("sudsy.yaml")).unwrap();
    let excluded = |rel: &str| chain.excludes(&dir.path().join(rel));
    assert!(!excluded("src/a.cs"));
    assert!(excluded("src/a.g.cs"));
    assert!(excluded("src/gen/a.cs"));
    assert!(excluded("docs/a.cs"));
    assert!(!excluded("package.json"));
    assert!(!excluded("deep/dir/x.json"));
}

#[test]
fn a_preset_is_a_config_file_of_declared_options() {
    let chain = ConfigChain::parse(
        "extends: [preset:json/data]
",
    )
    .unwrap();
    let r = chain
        .format_options(&FileInput::language("json"), &schema())
        .unwrap();
    assert_eq!(
        (r.options.int("line_width"), r.options.str("mode")),
        (70, "c")
    );
    // Values the provider does not declare are an error, not a hidden setting.
    for (name, needle) in [
        ("undeclared", "no_such_option"),
        ("loop-a", "extends itself"),
    ] {
        let chain = ConfigChain::parse(&format!(
            "extends: [preset:json/{name}]
"
        ))
        .unwrap();
        let e = chain
            .format_options(&FileInput::language("json"), &schema())
            .unwrap_err();
        assert!(e.message.contains(needle), "{e}");
    }
    // Presets have no versions.
    assert!(
        ConfigChain::parse(
            "extends: [preset:json/data@1]
"
        )
        .is_err()
    );
}

#[test]
fn a_preset_resolves_exactly_like_the_same_text_in_sudsy_yaml() {
    let preset = ConfigChain::parse(
        "extends: [preset:json/full]
",
    )
    .unwrap();
    let by_hand = ConfigChain::parse(&FULL.replace(
        "extends: [preset:json/narrow]",
        "format:
  line_width: 60",
    ))
    .unwrap();
    for path in ["a.json", "tests/a.json"] {
        let path = Path::new(path);
        let input = FileInput::language("json").with_path(path);
        let (a, b) = (
            preset.format_options(&input, &schema()).unwrap(),
            by_hand.format_options(&input, &schema()).unwrap(),
        );
        assert_eq!(a.options, b.options);
        let (ra, rb) = (
            preset.rules(&input, &schema()).unwrap(),
            by_hand.rules(&input, &schema()).unwrap(),
        );
        assert_eq!(ra.len(), 1);
        for (id, rule) in &ra {
            assert_eq!((rule.level, &rule.options), (rb[id].level, &rb[id].options));
        }
        assert_eq!(
            a.options.int("line_width"),
            if path.starts_with("tests") { 40 } else { 60 }
        );
        assert_eq!(a.options.str("mode"), "c");
    }
    let input = FileInput::language("json").with_path(Path::new("tests/a.json"));
    let rules = preset.rules(&input, &schema()).unwrap();
    assert_eq!(rules["r1"].level, Level::Error);
    assert_eq!(rules["r1"].options["n"], OptionValue::Int(2));
    assert_eq!(rules["r1"].source.to_string(), "preset json/full");
}

#[test]
fn rule_ids_are_checked_against_the_known_rules() {
    let known = ["no-empty-catch", "sorted-keys", "team/own-rule"];
    let ok = ConfigChain::parse(
        "lint:\n  rules:\n    sorted-keys: warn\n    team/own-rule: error\ncsharp:\n  lint:\n    rules:\n      no-empty-catch: error\n",
    )
    .unwrap();
    ok.validate_rule_ids(&known).unwrap();

    // A plugin rule is known once its plugin is loaded; until then it is as unknown as any id.
    let unloaded = ConfigChain::parse(
        "lint:
  rules:
    team/other: error
",
    )
    .unwrap();
    assert!(unloaded.validate_rule_ids(&known).is_err());

    // The error points at the value of the unknown rule, in every kind of section.
    for (text, line) in [
        ("lint:\n  rules:\n    sorted-kyes: warn\n", 3),
        ("yaml:\n  lint:\n    rules:\n      sorted-kyes: warn\n", 4),
        (
            "overrides:\n  - files: [\"a\"]\n    json:\n      lint:\n        rules:\n          sorted-kyes: warn\n",
            6,
        ),
    ] {
        let e = ConfigChain::parse(text)
            .unwrap()
            .validate_rule_ids(&known)
            .unwrap_err();
        assert_eq!(e.line, Some(line), "{text}");
        assert!(
            e.message.contains("unknown rule `sorted-kyes`")
                && e.message.contains("did you mean `sorted-keys`"),
            "{}",
            e.message
        );
    }
}

#[test]
fn the_command_line_depth_replaces_the_configured_one() {
    let chain = ConfigChain::parse("analysis: fast\n").unwrap();
    assert_eq!(chain.depth(), core_config::Depth::Fast);
    assert_eq!(
        chain.with_depth(core_config::Depth::Deep).depth(),
        core_config::Depth::Deep
    );
    // Also without any config file.
    let none = ConfigChain::default().with_depth(core_config::Depth::Fast);
    assert_eq!(none.depth(), core_config::Depth::Fast);
}

#[test]
fn extends_diamond_reads_each_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    // 20 levels, each extending the next one 20 times: exponential without a visited set.
    for i in 0..20 {
        let next = format!("l{}.yaml", i + 1);
        write(
            d,
            &format!("l{i}.yaml"),
            &format!(
                "extends: [{}]
",
                vec![next; 20].join(", ")
            ),
        );
    }
    write(
        d,
        "l20.yaml",
        "format:
  line_width: 90
",
    );
    // Depth is still capped, so shorten to what the cap allows.
    let e = ConfigChain::load(&d.join("l0.yaml")).unwrap_err();
    assert!(e.message.contains("extend each other"), "{e}");
}

#[test]
fn extends_wide_diamond_dedupes() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    write(
        d,
        "leaf.yaml",
        "format:
  line_width: 90
",
    );
    write(
        d,
        "a.yaml",
        "extends: [leaf.yaml, leaf.yaml]
",
    );
    write(
        d,
        "b.yaml",
        "extends: [leaf.yaml]
",
    );
    write(
        d,
        "sudsy.yaml",
        "extends: [a.yaml, b.yaml]
",
    );
    assert_eq!(
        ConfigChain::load(&d.join("sudsy.yaml"))
            .unwrap()
            .files
            .len(),
        4
    );
}

#[test]
fn extends_total_file_count_is_capped() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let names: Vec<String> = (0..70).map(|i| format!("f{i}.yaml")).collect();
    for n in &names {
        write(d, n, "");
    }
    write(
        d,
        "sudsy.yaml",
        &format!(
            "extends: [{}]
",
            names.join(", ")
        ),
    );
    let e = ConfigChain::load(&d.join("sudsy.yaml")).unwrap_err();
    assert!(
        e.message.contains("config files in one extends chain"),
        "{e}"
    );
}

#[test]
fn oversized_config_is_refused_and_outside_errors_withhold_text() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    write(d, "big.yaml", &"#".repeat(2 << 20));
    let e = ConfigChain::load(&d.join("big.yaml")).unwrap_err();
    assert!(e.message.contains("limit"), "{e}");
    write(
        d,
        "outside/shared.yaml",
        "secretkey: 1
",
    );
    write(
        d,
        "proj/sudsy.yaml",
        "extends: [../outside/shared.yaml]
",
    );
    let e = ConfigChain::load(&d.join("proj/sudsy.yaml")).unwrap_err();
    assert!(
        !e.message.contains("secretkey") && e.message.contains("withheld"),
        "{e}"
    );
}
