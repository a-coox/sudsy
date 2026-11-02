#![allow(dead_code)]

use core_config::{OptionValue, ResolvedOptions};
use core_format::DirectiveTable;
use core_format::testing::Subject;
use core_syntax::{Grammar, SyntaxNode};
use lang_yaml::{YamlProvider, option_schema};
use provider_api::{LanguageProvider, SourceFile, format_file};

/// A YAML file formatted with some options.
pub struct YamlSubject {
    provider: YamlProvider,
    options: Vec<(String, OptionValue)>,
}

pub fn subject(options: &[(&str, OptionValue)]) -> YamlSubject {
    YamlSubject {
        provider: YamlProvider::new(),
        options: options
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

pub fn s(v: &str) -> OptionValue {
    OptionValue::String(v.into())
}

pub fn n(v: i64) -> OptionValue {
    OptionValue::Int(v)
}

impl YamlSubject {
    /// Options as the CLI resolves them: schema defaults, then the given values.
    pub fn resolved(&self) -> ResolvedOptions {
        let schema = option_schema();
        let mut options = schema.defaults();
        for (k, v) in &self.options {
            schema.apply(&mut options, k, v).unwrap();
        }
        options
    }
}

impl Subject for YamlSubject {
    fn grammar(&self) -> &Grammar {
        self.provider.grammar()
    }

    fn parse(&self, text: &str) -> Result<SyntaxNode, String> {
        let r = self
            .provider
            .parse(&SourceFile::new("a.yaml", 1, text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &self.provider,
            &SourceFile::new("a.yaml", 1, text),
            &self.resolved(),
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper; returns the output.
#[track_caller]
pub fn fmt(input: &str, options: &[(&str, OptionValue)]) -> String {
    core_format::testing::assert_formats(&subject(options), input)
}
