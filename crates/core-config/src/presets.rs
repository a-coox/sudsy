//! Presets (plan.md "Configuration and presets"): named, versioned sets of option values that
//! `extends: [preset:csharp/csharpier-compat@1]` applies below the shared and language sections.
//!
//! A provider declares its presets next to its option schema. A preset may also set internal
//! values: names the schema does not declare, which users cannot set and which only the
//! provider's formatting code reads (how a preset picks between layout variants).

use crate::model::OptionValue;

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

    pub(crate) fn unversioned(&self) -> &str {
        self.name.split('@').next().unwrap_or(self.name)
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
