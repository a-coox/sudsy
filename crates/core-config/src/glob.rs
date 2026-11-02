//! Glob patterns in `sudsy.yaml` (`overrides[].files`, `include`, `exclude`).
//!
//! Paths are relative to the directory of the config file and use `/`. `*` does not cross a `/`,
//! `**` does. A pattern without a `/` matches at any depth (`*.g.cs`), a leading `/` anchors it at
//! the config file's directory, and a pattern that names a directory matches everything below it.

use crate::error::ConfigError;
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde_saphyr::Spanned;

#[derive(Clone, Debug)]
pub struct Matcher {
    set: GlobSet,
    empty: bool,
}

impl PartialEq for Matcher {
    fn eq(&self, other: &Self) -> bool {
        self.empty == other.empty && self.set.len() == other.set.len()
    }
}

fn build(pattern: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
}

/// The globs one pattern stands for: the pattern itself and everything below a directory of that
/// name.
fn expand(pattern: &str) -> Vec<String> {
    let p = pattern.strip_prefix("./").unwrap_or(pattern);
    let (anchored, p) = match p.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (p.contains('/') && !p.starts_with("**/"), p),
    };
    let p = p.trim_end_matches('/');
    let base = if anchored || p.starts_with("**/") {
        p.to_string()
    } else {
        format!("**/{p}")
    };
    vec![base.clone(), format!("{base}/**")]
}

impl Matcher {
    /// Checks every pattern and reports the first bad one at its position.
    pub fn new(patterns: &[Spanned<String>]) -> Result<Matcher, ConfigError> {
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            for glob in expand(&pattern.value) {
                let glob = build(&glob).map_err(|e| {
                    ConfigError::at(
                        pattern.referenced.line(),
                        pattern.referenced.column(),
                        format!("invalid glob `{}`: {}", pattern.value, e.kind()),
                    )
                })?;
                builder.add(glob);
            }
        }
        let set = builder
            .build()
            .map_err(|e| ConfigError::new(format!("invalid glob: {e}")))?;
        Ok(Matcher {
            set,
            empty: patterns.is_empty(),
        })
    }

    pub fn from_strs(patterns: &[&str]) -> Result<Matcher, ConfigError> {
        let spanned: Vec<Spanned<String>> = patterns
            .iter()
            .map(|p| {
                let at = serde_saphyr::Location::UNKNOWN;
                Spanned::new((*p).to_string(), at, at)
            })
            .collect();
        Matcher::new(&spanned)
    }

    /// Does the pattern list match `path` (relative, `/` separated)? An empty list matches
    /// nothing.
    pub fn is_match(&self, path: &str) -> bool {
        !self.empty && self.set.is_match(path)
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(patterns: &[&str]) -> Matcher {
        Matcher::from_strs(patterns).unwrap()
    }

    #[test]
    fn star_stops_at_slashes_and_double_star_does_not() {
        assert!(m(&["tests/**"]).is_match("tests/a/b.cs"));
        assert!(!m(&["tests/**"]).is_match("src/tests/a.cs"));
        assert!(m(&["src/*.cs"]).is_match("src/a.cs"));
        assert!(!m(&["src/*.cs"]).is_match("src/x/a.cs"));
        assert!(m(&["**/*.cs"]).is_match("a.cs"));
    }

    #[test]
    fn patterns_without_a_slash_match_at_any_depth_and_directories_match_below() {
        assert!(m(&["*.g.cs"]).is_match("src/a/b.g.cs"));
        assert!(m(&["bin"]).is_match("src/bin/x.dll"));
        assert!(m(&["build/"]).is_match("build/out.json"));
        assert!(!m(&["/build"]).is_match("src/build/out.json"));
        assert!(m(&["/build"]).is_match("build/out.json"));
        assert!(!m(&[]).is_match("anything"));
    }

    #[test]
    fn bad_globs_are_errors() {
        assert!(Matcher::from_strs(&["[unclosed"]).is_err());
    }
}
