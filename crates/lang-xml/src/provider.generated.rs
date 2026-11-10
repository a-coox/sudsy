// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("xml", tree_sitter_xml::LANGUAGE_XML), grammar: grammars::XML, mapping: &mapping::MAPPING, extensions: &["xml", "csproj", "vbproj", "fsproj", "props", "targets", "slnx", "config", "resx", "nuspec", "xaml", "axaml", "manifest", "ruleset", "proj", "projitems", "shproj", "vcxproj", "wixproj", "wxs", "runsettings", "vsixmanifest", "pubxml", "xsd", "xslt", "xsl", "svg", "plist"], file_names: &[], schema: option_schema(), formatter: &XmlFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: Some(validate::validate) })
}
