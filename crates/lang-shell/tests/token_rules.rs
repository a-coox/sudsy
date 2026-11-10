//! The `token:` form of pattern rules (docs/rules/patterns.md): kind, regex, not-regex,
//! skip-first and needs.

use provider_api::testing::{lint_with, pattern_error, pattern_rules};

const RULE: &str = "id: t/words
language: shell
message: m
rule: { kind: Command }
token: { kind: [Word], needs: '$', regex: '^[c$]', not-regex: '^\\$', skip-first: Word }
";

#[test]
fn reports_the_matching_tokens_of_a_node() {
    let p = lang_shell::provider();
    let ext = pattern_rules(&p, RULE);
    // `cat` is the skipped name, `$a` is excluded by not-regex, `cz` has no `$` in its command.
    let text = "cat $a cx cy\ncat cz\n";
    let found = lint_with(&p, &ext, "a.sh", text, "t/words: warn");
    let words: Vec<&str> = found
        .iter()
        .map(|d| &text[usize::from(d.range.start())..usize::from(d.range.end())])
        .collect();
    assert_eq!(words, ["cx", "cy"]);
}

#[test]
fn a_token_rule_cannot_have_layout() {
    let p = lang_shell::provider();
    let source = format!("{RULE}layout: [break-all]\n");
    let error = pattern_error(&p, &source).expect("refused");
    assert!(error.contains("`token`"), "{error}");
}
