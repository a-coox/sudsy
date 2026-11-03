//! The harness the built-in rules are tested with, for rules written against this SDK: run the
//! rule natively against a real language provider (C# needs the worker, found as the CLI finds it),
//! check expected diagnostics written under the lines they point at, and run the fix loop.
//!
//! ```ignore
//! let ext = plugin_sdk::testing::extensions("csharp", &NoDirectDateTime);
//! let p = plugin_sdk::testing::provider("csharp");
//! plugin_sdk::testing::expect(&*p, &ext, "a.cs", "var x = DateTime.Now;\n//      ^^^^^^^^^^^^ no-direct-datetime\n", "");
//! ```

use core_rules::{ExtensionRule, Extensions, Rule};
use provider_api::LanguageProvider;

pub use provider_api::testing::{expect, fix, fix_with, lint, lint_with, render};

/// The shipped provider of `language` (`csharp`, `json` or `yaml`).
pub fn provider(language: &str) -> Box<dyn LanguageProvider> {
    languages::providers()
        .into_iter()
        .find(|p| p.grammar().language == language)
        .unwrap_or_else(|| panic!("no provider for `{language}`"))
}

/// A project that has just `rule` for `language`, under its own short id.
pub fn extensions(language: &'static str, rule: &'static dyn Rule) -> Extensions {
    Extensions {
        rules: vec![ExtensionRule { language, rule }],
        files: Vec::new(),
    }
}
