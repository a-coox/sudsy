use std::fs;

#[test]
fn scaffolds_a_pattern_rule_and_a_registered_rust_rule() {
    let root = std::env::temp_dir().join(format!("sudsy-new-rule-{}", std::process::id()));
    let rules = root.join("crates/lang-csharp/src/rules");
    fs::create_dir_all(&rules).unwrap();
    fs::write(
        rules.join("mod.rs"),
        "mod a;\nmod z;\n\npub static RULES: &[&dyn Rule] = &[\n    &a::A,\n];\n",
    )
    .unwrap();

    let said = xtask::new_rule(&root, "csharp", "no-foo", false).unwrap();
    assert!(said.contains("rule: no-foo"));
    let yaml = fs::read_to_string(root.join("crates/lang-csharp/rules/no-foo.yaml")).unwrap();
    assert!(yaml.starts_with("id: no-foo\nlanguage: csharp\n"));
    let case = fs::read_to_string(root.join("tests/rules/csharp/no-foo/starter.cs")).unwrap();
    assert!(case.contains("// ^ no-foo") || case.contains("^ no-foo"));

    xtask::new_rule(&root, "csharp", "no-bar", true).unwrap();
    assert_eq!(
        fs::read_to_string(rules.join("mod.rs")).unwrap(),
        "mod a;\nmod no_bar;\nmod z;\n\npub static RULES: &[&dyn Rule] = &[\n    &a::A,\n    &no_bar::NoBar,\n];\n"
    );
    assert!(
        fs::read_to_string(rules.join("no_bar.rs"))
            .unwrap()
            .contains("pub NoBar {")
    );

    // The data languages: Rust rules, listed in their registry (a one-line list is expanded).
    let yaml = root.join("crates/lang-yaml/src");
    fs::create_dir_all(&yaml).unwrap();
    fs::write(
        yaml.join("rules.rs"),
        "use core_rules::Rule;\n\npub static RULES: &[&dyn Rule] = &[&a::A];\n",
    )
    .unwrap();
    xtask::new_rule(&root, "toml", "no-baz", false).unwrap();
    let toml = fs::read_to_string(root.join("crates/lang-toml/rules/no-baz.yaml")).unwrap();
    assert!(
        toml.contains(
            "kind: InlineTable
  regex:"
        ),
        "{toml}"
    );
    xtask::new_rule(&root, "yaml", "no-foo", true).unwrap();
    assert_eq!(
        fs::read_to_string(yaml.join("rules.rs")).unwrap(),
        "use core_rules::Rule;\n\nmod no_foo;\n\npub static RULES: &[&dyn Rule] = &[\n    &a::A,\n    &no_foo::NoFoo,\n];\n"
    );
    assert!(yaml.join("rules/no_foo.rs").exists());
    let case = fs::read_to_string(root.join("tests/rules/yaml/no-foo/starter.yaml")).unwrap();
    assert_eq!(case, "a: {}\n#  ^^ no-foo\n");
    let json = root.join("crates/lang-json/src");
    fs::create_dir_all(&json).unwrap();
    fs::write(
        json.join("rules.rs"),
        "pub static RULES: &[&dyn Rule] = &[];\n",
    )
    .unwrap();
    xtask::new_rule(&root, "json", "no-foo", true).unwrap();
    assert!(
        fs::read_to_string(json.join("rules.rs"))
            .unwrap()
            .ends_with(
                "mod no_foo;\n\npub static RULES: &[&dyn Rule] = &[\n    &no_foo::NoFoo,\n];\n"
            )
    );

    assert!(xtask::new_rule(&root, "csharp", "no-foo", false).is_err());
    assert!(xtask::new_rule(&root, "csharp", "No_Foo", false).is_err());
    assert!(xtask::new_rule(&root, "cobol", "no-foo", false).is_err());
    fs::remove_dir_all(&root).unwrap();
}
