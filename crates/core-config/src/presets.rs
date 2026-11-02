//! Presets (plan.md "Configuration and presets"): named, versioned sets of option values that
//! `extends: [preset:csharp/google@1]` applies below the shared and language sections.
//!
//! A preset is plain data: option values the provider declares, in a YAML file shipped with the
//! provider ([`Preset::from_yaml`]). Formatting code never looks at which preset is in use, so a
//! `sudsy.yaml` that writes the same options by hand formats exactly the same.

use crate::model::OptionValue;

/// `extends` entries that name a preset start with this.
pub const PRESET_PREFIX: &str = "preset:";

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// `language/name@version`, e.g. `csharp/microsoft@1`.
    pub name: String,
    pub values: Vec<(String, OptionValue)>,
}

impl Preset {
    pub fn new(name: &'static str, values: Vec<(&'static str, OptionValue)>) -> Self {
        debug_assert!(name.contains('/') && name.contains('@'), "{name}");
        Preset {
            name: name.to_string(),
            values: values
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    /// The presets of one language from the text of its data file: a YAML mapping of
    /// `name@version` to a mapping of option names to values. The text is part of the program, so
    /// a malformed one is a bug and panics; the provider tests resolve every preset.
    pub fn from_yaml(language: &str, text: &str) -> Vec<Preset> {
        let mut options = serde_saphyr::Options::default();
        options.strict_booleans = true;
        let sets: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, OptionValue>,
        > = serde_saphyr::from_str_with_options(text, options)
            .unwrap_or_else(|e| panic!("{language} presets: {e}"));
        sets.into_iter()
            .map(|(name, values)| {
                let name = format!("{language}/{name}");
                debug_assert!(name.contains('@'), "{name}");
                Preset {
                    name,
                    values: values.into_iter().collect(),
                }
            })
            .collect()
    }

    pub(crate) fn unversioned(&self) -> &str {
        self.name.split('@').next().unwrap_or(&self.name)
    }

    pub(crate) fn version(&self) -> u32 {
        self.name
            .rsplit('@')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{OptionSchema, shared};

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

    #[test]
    fn versioned_and_bare_names() {
        let s = schema();
        assert_eq!(s.find_preset("x/narrow@1").unwrap().name, "x/narrow@1");
        assert_eq!(s.find_preset("x/narrow").unwrap().name, "x/narrow@2");
        assert_eq!(s.find_preset("x/plain").unwrap().name, "x/plain@1");
        assert!(s.find_preset("x/nope@1").is_none());
        assert!(s.find_preset("x/narrow@3").is_none());
    }
}
