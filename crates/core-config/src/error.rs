use std::fmt;
use std::path::PathBuf;

/// A problem in a config file (or an `.editorconfig`), with its position when known (1-based).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub struct ConfigError {
    pub path: Option<PathBuf>,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub message: String,
}

impl ConfigError {
    pub fn new(message: impl Into<String>) -> Self {
        ConfigError {
            path: None,
            line: None,
            column: None,
            message: message.into(),
        }
    }

    pub(crate) fn at(line: u64, column: u64, message: impl Into<String>) -> Self {
        let known = line > 0;
        ConfigError {
            path: None,
            line: known.then_some(line),
            column: known.then_some(column),
            message: message.into(),
        }
    }

    pub(crate) fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Fills in the path unless the error already has one.
    pub(crate) fn in_file(mut self, path: Option<&PathBuf>) -> Self {
        if self.path.is_none() {
            self.path = path.cloned();
        }
        self
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(p) = &self.path {
            write!(f, "{}", p.display())?;
            if let (Some(l), Some(c)) = (self.line, self.column) {
                write!(f, ":{l}:{c}")?;
            }
            write!(f, ": ")?;
        } else if let (Some(l), Some(c)) = (self.line, self.column) {
            write!(f, "{l}:{c}: ")?;
        }
        write!(f, "{}", self.message)
    }
}

/// The candidate closest to `name`, if one is close enough to be a likely typo.
pub fn suggest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let limit = (name.chars().count() / 3).clamp(1, 3);
    candidates
        .into_iter()
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, c)| c)
}

/// `unknown option `x`; did you mean `y`? known options: a, b` (the suggestion only when close).
pub fn unknown_name<'a>(
    what: &str,
    name: &str,
    candidates: impl IntoIterator<Item = &'a str> + Clone,
) -> String {
    let mut out = format!("unknown {what} `{name}`");
    if let Some(s) = suggest(name, candidates.clone()) {
        out.push_str(&format!("; did you mean `{s}`?"));
    }
    let known: Vec<&str> = candidates.into_iter().collect();
    if !known.is_empty() {
        out.push_str(&format!(" (known: {})", known.join(", ")));
    }
    out
}

/// Levenshtein distance over chars.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_close_names_only() {
        let names = ["line_width", "indent_size", "final_newline"];
        assert_eq!(suggest("line_widht", names), Some("line_width"));
        assert_eq!(suggest("indent_sze", names), Some("indent_size"));
        assert_eq!(suggest("zzz", names), None);
        assert_eq!(suggest("brace_newline", names), None);
    }
}
