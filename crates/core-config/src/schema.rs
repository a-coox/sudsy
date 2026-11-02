//! Option schemas: every provider declares its format options once (name, type, default,
//! allowed values, description). Validation, resolution, docs and tests derive from it.

use crate::error::unknown_name;
use crate::model::OptionValue;
use crate::presets::Preset;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionKind {
    Bool,
    Int {
        min: i64,
        max: i64,
    },
    Enum(&'static [&'static str]),
    /// A list of C# identifiers-like words (preprocessor symbols).
    Words,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OptionDef {
    pub name: &'static str,
    pub kind: OptionKind,
    pub default: OptionValue,
    pub description: &'static str,
}

impl OptionDef {
    pub fn boolean(name: &'static str, default: bool, description: &'static str) -> Self {
        OptionDef {
            name,
            kind: OptionKind::Bool,
            default: OptionValue::Bool(default),
            description,
        }
    }

    pub fn int(
        name: &'static str,
        default: i64,
        range: (i64, i64),
        description: &'static str,
    ) -> Self {
        OptionDef {
            name,
            kind: OptionKind::Int {
                min: range.0,
                max: range.1,
            },
            default: OptionValue::Int(default),
            description,
        }
    }

    /// A list of words (letters, digits, `_`), empty by default.
    pub fn words(name: &'static str, description: &'static str) -> Self {
        OptionDef {
            name,
            kind: OptionKind::Words,
            default: OptionValue::List(Vec::new()),
            description,
        }
    }

    pub fn choice(
        name: &'static str,
        values: &'static [&'static str],
        default: &'static str,
        description: &'static str,
    ) -> Self {
        debug_assert!(values.contains(&default));
        OptionDef {
            name,
            kind: OptionKind::Enum(values),
            default: OptionValue::String(default.into()),
            description,
        }
    }

    /// Every value worth testing: both booleans, all enum values, and the bounds and default of
    /// an integer range.
    pub fn sample_values(&self) -> Vec<OptionValue> {
        match &self.kind {
            OptionKind::Bool => vec![OptionValue::Bool(false), OptionValue::Bool(true)],
            OptionKind::Enum(values) => values
                .iter()
                .map(|v| OptionValue::String((*v).into()))
                .collect(),
            OptionKind::Words => vec![
                OptionValue::List(Vec::new()),
                OptionValue::List(vec![OptionValue::String("DEBUG".into())]),
            ],
            OptionKind::Int { min, max } => {
                let default = match self.default {
                    OptionValue::Int(d) => d,
                    _ => *min,
                };
                let mut v = vec![*min, default, (*max).min(default.saturating_mul(4))];
                v.sort_unstable();
                v.dedup();
                v.into_iter().map(OptionValue::Int).collect()
            }
        }
    }

    pub(crate) fn check(&self, value: &OptionValue) -> Result<(), String> {
        match (&self.kind, value) {
            (OptionKind::Bool, OptionValue::Bool(_)) => Ok(()),
            (OptionKind::Int { min, max }, OptionValue::Int(n)) if (min..=max).contains(&n) => {
                Ok(())
            }
            (OptionKind::Int { min, max }, OptionValue::Int(_)) => {
                Err(format!("must be between {min} and {max}"))
            }
            (OptionKind::Int { .. }, _) => Err("must be an integer".into()),
            (OptionKind::Bool, _) => Err("must be true or false".into()),
            (OptionKind::Enum(values), OptionValue::String(s)) if values.contains(&s.as_str()) => {
                Ok(())
            }
            (OptionKind::Words, OptionValue::List(items)) if items.iter().all(is_word) => Ok(()),
            (OptionKind::Words, _) => {
                Err("must be a list of words made of letters, digits and `_`".into())
            }
            (OptionKind::Enum(values), _) => Err(format!("must be one of: {}", values.join(", "))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("format option `{option}`: {message}")]
pub struct OptionError {
    pub option: String,
    pub message: String,
}

/// The options of one provider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OptionSchema {
    defs: Vec<OptionDef>,
    presets: Vec<Preset>,
}

impl OptionSchema {
    pub fn new(defs: Vec<OptionDef>) -> Self {
        OptionSchema {
            defs,
            presets: Vec::new(),
        }
    }

    /// The presets (`extends: preset:...`) of this provider.
    pub fn with_presets(mut self, presets: Vec<Preset>) -> Self {
        self.presets = presets;
        self
    }

    pub fn presets(&self) -> &[Preset] {
        &self.presets
    }

    pub fn defs(&self) -> &[OptionDef] {
        &self.defs
    }

    pub fn get(&self, name: &str) -> Option<&OptionDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    pub fn defaults(&self) -> ResolvedOptions {
        ResolvedOptions(
            self.defs
                .iter()
                .map(|d| (d.name.to_string(), d.default.clone()))
                .collect(),
        )
    }

    /// Checks `value` against the declaration of `name` and stores it.
    pub fn apply(
        &self,
        into: &mut ResolvedOptions,
        name: &str,
        value: &OptionValue,
    ) -> Result<(), OptionError> {
        let def = self.get(name).ok_or_else(|| OptionError {
            option: name.into(),
            message: unknown_name("option", name, self.defs.iter().map(|d| d.name)).replacen(
                &format!("unknown option `{name}`"),
                "unknown",
                1,
            ),
        })?;
        def.check(value).map_err(|message| OptionError {
            option: name.into(),
            message,
        })?;
        into.0.insert(name.into(), value.clone());
        Ok(())
    }
}

/// Validated option values for one file: every option of the schema has a value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedOptions(BTreeMap<String, OptionValue>);

impl ResolvedOptions {
    pub fn set(&mut self, name: &str, value: OptionValue) {
        self.0.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&OptionValue> {
        self.0.get(name)
    }

    pub fn bool(&self, name: &str) -> bool {
        match self.0.get(name) {
            Some(OptionValue::Bool(b)) => *b,
            other => panic!("option `{name}` is not a boolean: {other:?}"),
        }
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.0.get(name) {
            Some(OptionValue::Int(n)) => *n,
            other => panic!("option `{name}` is not an integer: {other:?}"),
        }
    }

    /// The strings of a list option; empty when the option is absent.
    pub fn strings(&self, name: &str) -> Vec<String> {
        match self.0.get(name) {
            Some(OptionValue::List(items)) => items
                .iter()
                .filter_map(|i| match i {
                    OptionValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn str(&self, name: &str) -> &str {
        match self.0.get(name) {
            Some(OptionValue::String(s)) => s,
            other => panic!("option `{name}` is not a string: {other:?}"),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &OptionValue)> {
        self.0.iter()
    }
}

/// Definitions of the options every language shares (the top-level `format:` section). A provider
/// includes the ones it supports, with its own default.
pub mod shared {
    use super::OptionDef;

    pub const LINE_WIDTH: &str = "line_width";
    pub const INDENT_SIZE: &str = "indent_size";
    pub const INDENT_STYLE: &str = "indent_style";
    pub const FINAL_NEWLINE: &str = "final_newline";
    pub const NAMES: [&str; 4] = [LINE_WIDTH, INDENT_SIZE, INDENT_STYLE, FINAL_NEWLINE];

    pub fn line_width(default: i64) -> OptionDef {
        OptionDef::int(
            LINE_WIDTH,
            default,
            (20, 1000),
            "Preferred maximum line width in columns.",
        )
    }

    pub fn indent_size(default: i64) -> OptionDef {
        OptionDef::int(
            INDENT_SIZE,
            default,
            (1, 16),
            "Columns per indentation level (also the width of a tab).",
        )
    }

    pub fn indent_style() -> OptionDef {
        OptionDef::choice(
            INDENT_STYLE,
            &["space", "tab"],
            "space",
            "Indent with spaces or tabs.",
        )
    }

    pub fn final_newline(default: bool) -> OptionDef {
        OptionDef::boolean(FINAL_NEWLINE, default, "End the file with a newline.")
    }
}
impl OptionSchema {
    /// The preset named `language/name`.
    pub fn find_preset(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

fn is_word(value: &OptionValue) -> bool {
    matches!(value, OptionValue::String(s)
        if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> OptionSchema {
        OptionSchema::new(vec![
            shared::line_width(80),
            shared::indent_size(2),
            OptionDef::choice("mode", &["a", "b"], "a", "A mode."),
        ])
    }

    #[test]
    fn defaults_and_checked_values() {
        let s = schema();
        let mut r = s.defaults();
        assert_eq!(r.int("line_width"), 80);
        s.apply(&mut r, "mode", &OptionValue::String("b".into()))
            .unwrap();
        assert_eq!(r.str("mode"), "b");
        for (name, value, needle) in [
            (
                "nope",
                OptionValue::Bool(true),
                "format option `nope`: unknown",
            ),
            (
                "line_widht",
                OptionValue::Int(90),
                "did you mean `line_width`",
            ),
            ("mode", OptionValue::String("c".into()), "one of: a, b"),
            ("line_width", OptionValue::Int(5), "between 20 and 1000"),
            ("line_width", OptionValue::Bool(true), "integer"),
        ] {
            let e = s.apply(&mut r, name, &value).unwrap_err();
            assert!(e.to_string().contains(needle), "{e}");
        }
    }
}
