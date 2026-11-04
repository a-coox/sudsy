//! Checks that the rules a config names exist.

use crate::chain::ConfigChain;
use crate::error::{ConfigError, unknown_name};
use crate::model::LintSection;

impl ConfigChain {
    /// Every rule id in a `lint` section of the config files (top level, language sections and
    /// overrides) must be one of `known`, the ids of the rules of all languages and of the
    /// project's pattern and plugin rules (`plugin/rule`).
    pub fn validate_rule_ids(&self, known: &[&str]) -> Result<(), ConfigError> {
        for loaded in &self.files {
            let c = &loaded.config;
            let sections = std::iter::once(&c.lint)
                .chain(c.overrides.iter().map(|o| &o.lint))
                .chain(c.all_sections().map(|(_, s)| &s.value.lint));
            for lint in sections {
                check_section(lint, known).map_err(|e| loaded.attribute(e))?;
            }
        }
        Ok(())
    }
}

fn check_section(lint: &LintSection, known: &[&str]) -> Result<(), ConfigError> {
    for (id, rule) in &lint.rules {
        if !known.contains(&id.as_str()) {
            return Err(ConfigError::at(
                rule.referenced.line(),
                rule.referenced.column(),
                unknown_name("rule", id, known.iter().copied()),
            ));
        }
    }
    Ok(())
}
