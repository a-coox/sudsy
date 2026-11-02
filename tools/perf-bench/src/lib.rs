//! Inputs and setup shared by the benchmark binary and the scaling test.

pub mod inputs;

use provider_api::{LanguageProvider, SourceFile};

/// `text` as a source file for `p`, with the options `p` has by default.
pub fn file(
    p: &dyn LanguageProvider,
    name: &str,
    version: u64,
    text: &str,
) -> (SourceFile, core_config::ResolvedOptions) {
    let config = core_config::Config::default();
    let mut opts = config
        .format_options(&p.grammar().language, p.option_schema())
        .expect("default options");
    let mut f = SourceFile::new(name, version, text);
    f.language_version = provider_api::resolve_dialect(&mut opts, None);
    (f, opts)
}
