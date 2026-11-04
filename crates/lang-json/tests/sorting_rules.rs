//! Option validation of the shared ordering and key rules (per-option behavior is in the rule
//! fixtures).

use lang_json::JsonProvider;
use provider_api::testing::fix;

fn fixed(rule: &str, options: &str) -> String {
    let rules = format!("{rule}:\n  level: warn\n  options:\n    {options}");
    fix(&JsonProvider::new(), "a.json", "{}", &rules, true)
}

#[test]
#[should_panic(expected = "option `direction` must be one of `asc`, `desc`, not `up`")]
fn invalid_direction_is_a_config_error() {
    fixed("sorted-keys", "direction: up");
}

#[test]
#[should_panic(expected = "option `report` must be one of `moved`, `misplaced`, not `all`")]
fn invalid_report_is_a_config_error() {
    fixed("sorted-keys", "report: all");
}

#[test]
#[should_panic(expected = "rule `key-name-casing`")]
fn invalid_casing_is_a_config_error() {
    fixed("key-name-casing", "allowed: [Camel]");
}

#[test]
#[should_panic(expected = "option `ignored_keys` has an invalid regular expression `(`")]
fn invalid_ignored_keys_regex_is_a_config_error() {
    fixed("sorted-keys", "ignored_keys: [\"(\"]");
}
