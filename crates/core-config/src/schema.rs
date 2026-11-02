//! Option schemas: every provider declares its format options once (name, type, default,
//! allowed values, description). Validation, resolution, docs and tests derive from it.

use crate::model::{LanguageSection, OptionValue, Options};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionKind {
    Bool,
    Int { min: i64, max: i64 },
    Enum(&'static [&'static str]),
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

    fn check(&self, value: &OptionValue) -> Result<(), String> {
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
}

impl OptionSchema {
    pub fn new(defs: Vec<OptionDef>) -> Self {
        OptionSchema { defs }
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
            message: format!(
                "unknown option; known options: {}",
                self.defs
                    .iter()
                    .map(|d| d.name)
                    .collect::<Vec<_>>()
                    .join(", ")
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

/// Format options for one file from the config: schema defaults, then the shared `format:`
/// section, then the language section. Shared keys a language does not declare are ignored; an
/// unknown key in the language section is an error. Presets, overrides and `.editorconfig` extend
/// this in M2.
pub fn resolve_format(
    shared_format: &Options,
    section: &LanguageSection,
    schema: &OptionSchema,
) -> Result<ResolvedOptions, OptionError> {
    let mut resolved = schema.defaults();
    for (name, value) in shared_format {
        if !shared::NAMES.contains(&name.as_str()) {
            return Err(OptionError {
                option: name.clone(),
                message: format!(
                    "not a shared option; put it under a language section (shared options: {})",
                    shared::NAMES.join(", ")
                ),
            });
        }
        if schema.get(name).is_some() {
            schema.apply(&mut resolved, name, value)?;
        }
    }
    for (name, value) in &section.format {
        schema.apply(&mut resolved, name, value)?;
    }
    Ok(resolved)
}

impl crate::Config {
    /// Format options for files of `language` (a grammar's language name). This is the one place
    /// M2 extends with presets, overrides and `.editorconfig`.
    pub fn format_options(
        &self,
        language: &str,
        schema: &OptionSchema,
    ) -> Result<ResolvedOptions, OptionError> {
        let none = LanguageSection::default();
        let section = self.language_section(language).unwrap_or(&none);
        resolve_format(&self.format, section, schema)
    }
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

    fn opts(pairs: &[(&str, OptionValue)]) -> Options {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn language_section_beats_shared_beats_default() {
        let shared = opts(&[
            ("line_width", OptionValue::Int(100)),
            ("indent_size", OptionValue::Int(4)),
        ]);
        let section = LanguageSection {
            format: opts(&[("line_width", OptionValue::Int(120))]),
            ..Default::default()
        };
        let r = resolve_format(&shared, &section, &schema()).unwrap();
        assert_eq!(r.int("line_width"), 120);
        assert_eq!(r.int("indent_size"), 4);
        assert_eq!(r.str("mode"), "a");
    }

    #[test]
    fn shared_option_the_language_lacks_is_ignored() {
        let shared = opts(&[("final_newline", OptionValue::Bool(false))]);
        assert!(resolve_format(&shared, &LanguageSection::default(), &schema()).is_ok());
    }

    #[test]
    fn rejects_unknown_and_invalid_values() {
        let bad_shared = opts(&[("mode", OptionValue::String("b".into()))]);
        let e = resolve_format(&bad_shared, &LanguageSection::default(), &schema()).unwrap_err();
        assert!(e.message.contains("not a shared option"));
        for (name, value, needle) in [
            ("nope", OptionValue::Bool(true), "unknown option"),
            ("mode", OptionValue::String("c".into()), "one of: a, b"),
            ("line_width", OptionValue::Int(5), "between 20 and 1000"),
            ("line_width", OptionValue::Bool(true), "integer"),
        ] {
            let section = LanguageSection {
                format: opts(&[(name, value)]),
                ..Default::default()
            };
            let e = resolve_format(&Options::new(), &section, &schema()).unwrap_err();
            assert!(e.to_string().contains(needle), "{e}");
        }
    }

    #[test]
    fn config_picks_the_section_of_the_language() {
        let config = crate::parse_str(
            "format:
  line_width: 100
json:
  format:
    line_width: 90
",
        )
        .unwrap();
        let line_width = |lang| {
            config
                .format_options(lang, &schema())
                .unwrap()
                .int("line_width")
        };
        assert_eq!(line_width("json"), 90);
        assert_eq!(line_width("yaml"), 100);
        assert_eq!(line_width("unknown"), 100);
    }
}
