//! XML: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`), `validate.rs`
//! checks well-formedness beyond the grammar, formatting is in `format.rs`.

#[rustfmt::skip]
pub mod generated;
mod format;
mod mapping;
mod rules;
mod validate;

pub use format::{XmlFormatter, option_schema};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

/// File extensions of the XML dialects .NET repositories hold.
const EXTENSIONS: &[&str] = &[
    "xml",
    "csproj",
    "vbproj",
    "fsproj",
    "props",
    "targets",
    "slnx",
    "config",
    "resx",
    "nuspec",
    "xaml",
    "axaml",
    "manifest",
    "ruleset",
    "proj",
    "projitems",
    "shproj",
    "vcxproj",
    "wixproj",
    "wxs",
    "runsettings",
    "vsixmanifest",
    "pubxml",
    "xsd",
    "xslt",
    "xsl",
    "svg",
    "plist",
];

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec {
        language: tree_sitter_xml::LANGUAGE_XML.into(),
        grammar: grammars::xml(),
        mapping: &mapping::MAPPING,
        extensions: EXTENSIONS,
        schema: option_schema(),
        formatter: &XmlFormatter,
        hooks: &core_rules::DEFAULT_HOOKS,
        rules: rules::RULES,
        pattern_files: provider_api::pattern_files!(),
        validate: Some(validate::validate),
    })
}
