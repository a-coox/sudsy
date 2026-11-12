//! The code of other languages inside a file, parsed with the providers of those languages.

use crate::provider::{LanguageProvider, SourceFile};
use core_rules::EmbeddedTree;
use core_syntax::{Embedded, SyntaxNode};
use std::path::Path;

/// A region of the host file and its parse (offsets from the start of the region).
pub struct EmbeddedParse<'a> {
    pub region: Embedded,
    pub provider: &'a dyn LanguageProvider,
    pub root: SyntaxNode,
    /// The text that was parsed: the region with the host's parts blanked.
    pub code: String,
}

/// A provider and the dialect its matcher selects.
pub type Resolved<'a> = Option<(&'a dyn LanguageProvider, Option<String>)>;

/// The provider `name` stands for (a language name, an extension or a file name).
pub fn resolve<'a>(
    providers: impl IntoIterator<Item = &'a dyn LanguageProvider>,
    name: &str,
) -> Resolved<'a> {
    let file = format!("x.{name}");
    providers
        .into_iter()
        .filter(|p| p.embeddable())
        .find_map(|p| {
            if p.language() == name {
                return Some((p, None));
            }
            let m = (p.file_matchers().iter())
                .find(|m| m.matches(Path::new(&file)) || m.matches(Path::new(name)))?;
            Some((p, m.dialect.clone()))
        })
}

/// The parses as the trees rules read (see `core_rules::FileContext::embedded`).
pub fn trees(parses: &[EmbeddedParse<'_>]) -> Vec<EmbeddedTree> {
    parses
        .iter()
        .map(|e| EmbeddedTree {
            range: e.region.range,
            language: e.provider.language().to_string(),
            root: e.root.clone(),
        })
        .collect()
}

/// Parses what `host` finds in `root`. A region whose language nobody handles or whose code has
/// syntax errors is left out: it stays as written.
pub fn parse_regions<'a>(
    host: &dyn LanguageProvider,
    root: &SyntaxNode,
    text: &str,
    resolve: &dyn Fn(&str) -> Resolved<'a>,
) -> Vec<EmbeddedParse<'a>> {
    let mut out = Vec::new();
    for region in host.embedded(root, text) {
        let Some((provider, dialect)) = resolve(&region.language) else {
            continue;
        };
        let code = region.blanked(text);
        let mut file = SourceFile::new(format!("embedded.{}", region.language), 1, code.clone());
        file.language_version = dialect;
        if let Ok(parsed) = provider.parse(&file)
            && parsed.errors.is_empty()
        {
            out.push(EmbeddedParse {
                region,
                provider,
                root: parsed.tree,
                code,
            });
        }
    }
    out
}
