//! Presets (plan.md "Configuration and presets"): named, versioned sets of option values that
//! `extends: [preset:csharp/csharpier-compat@1]` applies below the shared and language sections.
//!
//! A provider declares its presets next to its option schema. A preset may also set internal
//! values: names the schema does not declare, which users cannot set and which only the
//! provider's formatting code reads (how a preset picks between layout variants).

use crate::model::OptionValue;
use crate::schema::{OptionError, OptionSchema, ResolvedOptions};

/// `extends` entries that name a preset start with this.
pub const PRESET_PREFIX: &str = "preset:";

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// `language/name@version`, e.g. `csharp/microsoft@1`.
    pub name: &'static str,
    pub values: Vec<(&'static str, OptionValue)>,
}

impl Preset {
    pub fn new(name: &'static str, values: Vec<(&'static str, OptionValue)>) -> Self {
        debug_assert!(name.contains('/') && name.contains('@'), "{name}");
        Preset { name, values }
    }

    fn unversioned(&self) -> &str {
        self.name.split('@').next().unwrap_or(self.name)
    }

    fn version(&self) -> u32 {
        self.name
            .rsplit('@')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }
}

/// Applies the presets named by `extends` that belong to `language`, in order. Entries that are
/// not presets (config files, M3) and presets of other languages are skipped. A name without a
/// version means the newest version.
pub fn apply_presets(
    extends: &[&str],
    language: &str,
    schema: &OptionSchema,
    into: &mut ResolvedOptions,
) -> Result<(), OptionError> {
    for entry in extends {
        let Some(name) = entry.strip_prefix(PRESET_PREFIX) else {
            continue;
        };
        if name.split('/').next() != Some(language) {
            continue;
        }
        let preset = find(schema, name).ok_or_else(|| OptionError {
            option: "extends".into(),
            message: format!(
                "unknown preset `{name}`; known presets: {}",
                schema
                    .presets()
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })?;
        for (option, value) in &preset.values {
            if schema.get(option).is_some() {
                schema.apply(into, option, value)?;
            } else {
                into.set(option, value.clone());
            }
        }
    }
    Ok(())
}

fn find<'a>(schema: &'a OptionSchema, name: &str) -> Option<&'a Preset> {
    if name.contains('@') {
        return schema.presets().iter().find(|p| p.name == name);
    }
    schema
        .presets()
        .iter()
        .filter(|p| p.unversioned() == name)
        .max_by_key(|p| p.version())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::shared;

    fn schema() -> OptionSchema {
        OptionSchema::new(vec![shared::line_width(120)]).with_presets(vec![
            Preset::new("x/plain@1", vec![]),
            Preset::new(
                "x/narrow@1",
                vec![
                    ("line_width", OptionValue::Int(100)),
                    ("hidden", OptionValue::Bool(true)),
                ],
            ),
            Preset::new("x/narrow@2", vec![("line_width", OptionValue::Int(90))]),
        ])
    }

    fn apply(entry: &str) -> Result<ResolvedOptions, OptionError> {
        let s = schema();
        let mut r = s.defaults();
        apply_presets(&[entry], "x", &s, &mut r).map(|()| r)
    }

    #[test]
    fn versioned_and_bare_names() {
        let r = apply("preset:x/narrow@1").unwrap();
        assert_eq!(r.int("line_width"), 100);
        assert_eq!(r.get("hidden"), Some(&OptionValue::Bool(true)));
        assert_eq!(apply("preset:x/narrow").unwrap().int("line_width"), 90);
        assert_eq!(apply("preset:x/plain@1").unwrap().int("line_width"), 120);
    }

    #[test]
    fn other_languages_and_files_are_skipped_unknown_names_fail() {
        assert!(apply("preset:y/anything@1").is_ok());
        assert!(apply("base.yaml").is_ok());
        let e = apply("preset:x/nope@1").unwrap_err();
        assert!(e.message.contains("x/narrow@2"), "{e}");
    }
}
