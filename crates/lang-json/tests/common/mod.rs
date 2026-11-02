#![allow(dead_code)]

use core_config::{OptionValue, ResolvedOptions};
use core_format::DirectiveTable;
use core_format::testing::Subject;
use core_syntax::{Grammar, SyntaxNode};
use lang_json::{JsonProvider, option_schema};
use provider_api::{LanguageProvider, SourceFile, format_file, resolve_dialect};
use std::path::Path;

/// A JSON file formatted with some options.
pub struct JsonSubject {
    provider: JsonProvider,
    path: String,
    options: Vec<(String, OptionValue)>,
}

pub fn subject(path: &str, options: &[(&str, OptionValue)]) -> JsonSubject {
    JsonSubject {
        provider: JsonProvider::new(),
        path: path.into(),
        options: options
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

pub fn s(v: &str) -> OptionValue {
    OptionValue::String(v.into())
}

impl JsonSubject {
    /// Options as the CLI resolves them: schema defaults, the given values, then the dialect.
    pub fn resolved(&self) -> (ResolvedOptions, Option<String>) {
        let schema = option_schema();
        let mut options = schema.defaults();
        for (k, v) in &self.options {
            schema.apply(&mut options, k, v).unwrap();
        }
        let matcher = provider_api::select_provider(
            &[Box::new(JsonProvider::new()) as Box<dyn LanguageProvider>],
            Path::new(&self.path),
        )
        .and_then(|(_, d)| d);
        let dialect = resolve_dialect(&mut options, matcher);
        (options, dialect)
    }

    fn source(&self, text: &str) -> SourceFile {
        let mut f = SourceFile::new(&self.path, 1, text);
        f.language_version = self.resolved().1;
        f
    }
}

impl Subject for JsonSubject {
    fn grammar(&self) -> &Grammar {
        self.provider.grammar()
    }

    fn parse(&self, text: &str) -> Result<SyntaxNode, String> {
        let r = self
            .provider
            .parse(&self.source(text))
            .map_err(|e| e.to_string())?;
        match r.errors.first() {
            Some(e) => Err(format!("{} (byte {})", e.message, e.start)),
            None => Ok(r.tree),
        }
    }

    fn format(&self, text: &str) -> Result<String, String> {
        format_file(
            &self.provider,
            &self.source(text),
            &self.resolved().0,
            &DirectiveTable::default(),
        )
        .map_err(|e| e.to_string())
    }
}

/// Formats with every check of the shared helper; returns the output.
#[track_caller]
pub fn fmt(input: &str, options: &[(&str, OptionValue)]) -> String {
    fmt_path("a.json", input, options)
}

#[track_caller]
pub fn fmt_path(path: &str, input: &str, options: &[(&str, OptionValue)]) -> String {
    core_format::testing::assert_formats(&subject(path, options), input)
}
