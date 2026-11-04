//! What a plain scalar means: the implicit types of the YAML 1.1 schema (PyYAML's resolver, which
//! yamllint uses) and whether a string can be written plain without changing.

use regex::Regex;
use std::sync::LazyLock;

/// The implicit resolvers other than string: bool, float, int (with `0o17`), merge, null,
/// timestamp, value and yaml.
static NON_STRING: LazyLock<Regex> = LazyLock::new(|| {
    let alternatives = [
        "yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF",
        "[-+]?[0-9][0-9_]*\\.[0-9_]*(?:[eE][-+][0-9]+)?",
        "\\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?",
        "[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\\.[0-9_]*",
        "[-+]?\\.(?:inf|Inf|INF)",
        "\\.(?:nan|NaN|NAN)",
        "[-+]?0b[01_]+",
        "[-+]?0o?[0-7_]+",
        "[-+]?(?:0|[1-9][0-9_]*)",
        "[-+]?0x[0-9a-fA-F_]+",
        "[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+",
        "<<",
        "~|null|Null|NULL|",
        "[0-9]{4}-[0-9]{2}-[0-9]{2}",
        "[0-9]{4}-[0-9][0-9]?-[0-9][0-9]?(?:[Tt]|[ \\t]+)[0-9][0-9]?:[0-9]{2}:[0-9]{2}(?:\\.[0-9]*)?(?:[ \\t]*(?:Z|[-+][0-9][0-9]?(?::[0-9]{2})?))?",
        "=",
        "!|&|\\*",
    ];
    Regex::new(&format!("^(?:{})$", alternatives.join("|"))).expect("the pattern is valid")
});

/// Does the plain scalar `value` resolve to a string (rather than a bool, number, null, ...)?
pub fn resolves_to_str(value: &str) -> bool {
    !NON_STRING.is_match(value)
}

fn is_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Can `value` be a plain scalar as a block mapping value and read back as the same string?
pub fn plain_round_trips(value: &str) -> bool {
    let mut chars = value.chars();
    let (Some(first), Some(last)) = (chars.next(), value.chars().next_back()) else {
        return false;
    };
    if matches!(first, ' ' | '\t') || matches!(last, ' ' | '\t') {
        return false;
    }
    if value
        .chars()
        .any(|c| is_break(c) || c.is_control() && c != '\t')
    {
        return false;
    }
    if "-?:,[]{}#&*!|>'\"%@`".contains(first) {
        let next = chars.next();
        // `-a`, `?a` and `:a` are plain; the rest of the indicators start something else.
        if !("-?:".contains(first) && next.is_some_and(|n| !matches!(n, ' ' | '\t'))) {
            return false;
        }
    }
    !(last == ':'
        || value.contains(": ")
        || value.contains(":\t")
        || value.contains(" #")
        || value.contains("\t#"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicit_types() {
        for v in [
            "yes",
            "On",
            "12",
            "0755",
            "0o17",
            "1_000",
            "1.5",
            ".5",
            "-.inf",
            ".NaN",
            "~",
            "",
            "0x1F",
            "1:30",
            "2001-12-14",
            "2001-12-14t21:59:43.10-05:00",
            "<<",
            "=",
            "!",
        ] {
            assert!(!resolves_to_str(v), "{v}");
        }
        for v in ["yeah", "1e3", "1.2.3", "abc", "0b", "2001-12-1", "a: b"] {
            assert!(resolves_to_str(v), "{v}");
        }
    }

    #[test]
    fn plain_scalars_that_read_back() {
        for v in [
            "abc", "a b", "a:b", "a#b", "-a", "?a", ":a", "a, b", "x]", "é",
        ] {
            assert!(plain_round_trips(v), "{v}");
        }
        for v in [
            "", " a", "a ", "a: b", "a:", "a #b", "#a", "- a", "-", "?", "[a", "{a", ",a", "&a",
            "*a", "!a", "|a", ">a", "'a", "\"a", "%a", "@a", "`a", "a\nb",
        ] {
            assert!(!plain_round_trips(v), "{v:?}");
        }
    }
}
