//! SDK for Sudsy rule plugins. A plugin is a Rust crate that implements
//! [`Rule`], the same trait the built-in rules implement, and exports it with
//! [`export_plugin!`]; it compiles to a Wasm component with
//! `cargo build --target wasm32-wasip2 --release` and the config lists the `.wasm` file under
//! `plugins: wasm:`.
//!
//! API v0 is UNSTABLE (`wit/rules.wit`, `rules@0.1`): it will change until v1.
//!
//! ```ignore
//! use plugin_sdk::*;
//!
//! struct NoDirectDateTime;
//! static META: RuleMeta = RuleMeta { id: "no-direct-datetime", /* ... */ };
//! impl Rule for NoDirectDateTime { /* meta, kinds, check ... */ }
//!
//! export_plugin!(name: "team", rules: [("csharp", &NoDirectDateTime)]);
//! ```
//!
//! The host reports the rule as `team/no-direct-datetime`. Rules see the syntax tree, the file's
//! semantic answers and flow graphs ([`flow`], [`core_semantics`]) and may use the pattern
//! matcher ([`patterns`]); they cannot touch files, the network, the clock or the environment.
//! `plugin_sdk::testing` (feature `testing`, native only) runs a rule against a real language
//! provider with the same harness the built-in rules use.

pub mod guest;
#[cfg(feature = "testing")]
pub mod testing;

pub use core_config::Level;
pub use core_flow as flow;
pub use core_patterns as patterns;
pub use core_rules::tree;
pub use core_rules::{
    BodyContext, Category, Edit, FileContext, Fix, FixKind, Layer, Opt, OptionDecl, ProjectContext,
    ProjectFacts, Rule, RuleContext, RuleMeta, RuleOptions,
};
pub use core_semantics;
pub use core_syntax::{self, SyntaxKind, SyntaxNode, TextRange, TextSize};

/// The kind named `name` in the grammar of `language` (`json`, `csharp`, `yaml`): a rule's
/// `kinds()` is built from these. Panics on a name the grammar does not have, which a test of
/// the rule finds at once.
pub fn kind_of(language: &str, name: &str) -> SyntaxKind {
    let grammar = match language {
        "json" => core_syntax::grammars::json(),
        "csharp" => core_syntax::grammars::csharp(),
        "yaml" => core_syntax::grammars::yaml(),
        other => panic!("no grammar for `{other}`"),
    };
    grammar
        .kind_by_name(name)
        .unwrap_or_else(|| panic!("the {language} grammar has no kind `{name}`"))
}

/// A rule a plugin exports and the language it runs on (`csharp`, `json`, `yaml`, or `*`).
pub struct PluginRule {
    pub language: &'static str,
    pub rule: &'static dyn Rule,
}

#[allow(unsafe_code, missing_docs, clippy::all)]
#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({
        path: "../../wit",
        world: "plugin",
        pub_export_macro: true,
        default_bindings_module: "plugin_sdk::bindings",
    });
}

/// Exports the plugin: its name (the prefix of its rule ids) and its rules. Only a build for
/// `wasm32` exports anything: a native build of the same crate (its tests, a built-in copy of the
/// rules) has nothing to export, and the component's export names are not valid native symbols.
#[macro_export]
macro_rules! export_plugin {
    (name: $name:expr, rules: [$(($language:expr, $rule:expr)),* $(,)?]) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            use $crate::bindings::exports::sudsy::rules::rules as w;

            static RULES: &[$crate::PluginRule] = &[
                $($crate::PluginRule { language: $language, rule: $rule }),*
            ];

            struct SudsyPlugin;

            impl w::Guest for SudsyPlugin {
                fn info() -> w::PluginInfo {
                    $crate::guest::info($name, RULES)
                }

                fn check(
                    rule: String,
                    options: Vec<(String, w::OptionValue)>,
                    file: w::File,
                ) -> Result<Vec<w::Diagnostic>, String> {
                    $crate::guest::check(RULES, &rule, options, file)
                }

                fn collect(
                    rule: String,
                    options: Vec<(String, w::OptionValue)>,
                    file: w::File,
                ) -> Result<Vec<w::Fact>, String> {
                    $crate::guest::collect(RULES, &rule, options, file)
                }

                fn finish(
                    rule: String,
                    options: Vec<(String, w::OptionValue)>,
                    facts: Vec<w::Fact>,
                ) -> Result<Vec<w::ProjectDiagnostic>, String> {
                    $crate::guest::finish(RULES, &rule, options, facts)
                }
            }

            $crate::bindings::export!(SudsyPlugin with_types_in $crate::bindings);
        };
    };
}
