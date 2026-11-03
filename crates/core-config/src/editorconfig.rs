//! `.editorconfig` as a fallback source of format options.
//!
//! Files are found by walking up from the formatted file until one says `root = true`; a closer
//! file overrides a farther one, a later section overrides an earlier one. Section globs follow
//! the EditorConfig specification: `*` (no `/`), `**`, `?`, `[seq]`, `[!seq]`, `{a,b}` and
//! `{n..m}`; a section without a `/` matches at any depth, one with a `/` is relative to the
//! directory of its `.editorconfig`.
//!
//! Mapping (everything else in the file is another tool's business and ignored):
//!
//! | `.editorconfig` | Option |
//! | --- | --- |
//! | `indent_style` (`space`, `tab`) | `indent_style` |
//! | `indent_size` (a number; `tab` means `tab_width`) | `indent_size` |
//! | `tab_width` | `indent_size`, only for `indent_style = tab` without an `indent_size` |
//! | `max_line_length` (a number; `off` is ignored) | `line_width` |
//! | `insert_final_newline` | `final_newline` |
//! | `csharp_new_line_before_open_brace` (`all`, `none`; other lists are ignored) | `brace_newline` (C#) |
//!
//! A value that is not valid for the option (a width of 5, `max_line_length = off`) is ignored:
//! the file is shared with other tools, so it is not Sudsy's to reject. Options a language does not
//! declare are ignored for its files.

use crate::model::OptionValue;
use globset::GlobBuilder;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One value taken from an `.editorconfig`, mapped to the option it sets.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorConfigValue {
    pub option: &'static str,
    pub value: OptionValue,
    pub file: PathBuf,
    pub line: usize,
    /// The `.editorconfig` key the value came from.
    pub key: String,
}

/// The mapped values that apply to one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorConfig {
    pub values: Vec<EditorConfigValue>,
}

#[derive(Debug)]
struct Section {
    pattern: String,
    props: Vec<(String, String, usize)>,
}

#[derive(Debug, Default)]
struct Parsed {
    root: bool,
    sections: Vec<Section>,
}

fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            parsed.sections.push(Section {
                pattern: inner.to_string(),
                props: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
            continue;
        };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim().to_string());
        match parsed.sections.last_mut() {
            Some(section) => section.props.push((key, value, index + 1)),
            None if key == "root" => parsed.root = value.eq_ignore_ascii_case("true"),
            None => {}
        }
    }
    parsed
}

/// `{1..3}` becomes `{1,2,3}`; other braces are left to the glob engine.
fn expand_ranges(pattern: &str) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = &after[..close];
        let range = inner.split_once("..").and_then(|(a, b)| {
            let (a, b) = (a.parse::<i64>().ok()?, b.parse::<i64>().ok()?);
            (a <= b && b - a < 1000).then_some((a, b))
        });
        match range {
            Some((a, b)) => {
                let items: Vec<String> = (a..=b).map(|n| n.to_string()).collect();
                out.push_str(&format!("{{{}}}", items.join(",")));
            }
            None => out.push_str(&rest[open..open + close + 2]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// `src/**.cs`: `**` followed by more name text matches any depth, like `**/*`.
fn split_double_star(pattern: &str) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            out.push('*');
            match chars.peek() {
                None | Some('/') => {}
                Some(_) => out.push_str("/*"),
            }
        }
    }
    out
}

fn section_matches(pattern: &str, relative: &str) -> bool {
    let pattern = split_double_star(&expand_ranges(pattern));
    let glob = match pattern.strip_prefix('/') {
        Some(anchored) => anchored.to_string(),
        None if pattern.contains('/') => pattern,
        None => format!("**/{pattern}"),
    };
    GlobBuilder::new(&glob)
        .literal_separator(true)
        .build()
        .is_ok_and(|g| g.compile_matcher().is_match(relative))
}

type Props = BTreeMap<String, (String, PathBuf, usize)>;

/// The properties of `file` in the given files, farthest first.
fn properties(files: &[(PathBuf, Parsed)], file: &Path) -> Props {
    let mut props = Props::new();
    for (dir, parsed) in files {
        let Ok(relative) = file.strip_prefix(dir) else {
            continue;
        };
        let relative = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        for section in &parsed.sections {
            if !section_matches(&section.pattern, &relative) {
                continue;
            }
            for (key, value, line) in &section.props {
                if value.eq_ignore_ascii_case("unset") {
                    props.remove(key);
                } else {
                    props.insert(
                        key.clone(),
                        (value.to_ascii_lowercase(), dir.join(".editorconfig"), *line),
                    );
                }
            }
        }
    }
    props
}

fn map(props: &Props) -> Vec<EditorConfigValue> {
    let mut out = Vec::new();
    let mut push = |option: &'static str, key: &str, value: OptionValue| {
        let (_, file, line) = &props[key];
        out.push(EditorConfigValue {
            option,
            value,
            file: file.clone(),
            line: *line,
            key: key.to_string(),
        });
    };
    let number = |key: &str| props.get(key).and_then(|(v, ..)| v.parse::<i64>().ok());
    let style = props.get("indent_style").map(|(v, ..)| v.as_str());
    if let Some(style @ ("space" | "tab")) = style {
        push(
            "indent_style",
            "indent_style",
            OptionValue::String(style.into()),
        );
    }
    match props.get("indent_size").map(|(v, ..)| v.as_str()) {
        Some("tab") => {
            if let Some(n) = number("tab_width") {
                push("indent_size", "tab_width", OptionValue::Int(n));
            }
        }
        Some(_) => {
            if let Some(n) = number("indent_size") {
                push("indent_size", "indent_size", OptionValue::Int(n));
            }
        }
        None if style == Some("tab") => {
            if let Some(n) = number("tab_width") {
                push("indent_size", "tab_width", OptionValue::Int(n));
            }
        }
        None => {}
    }
    if let Some(n) = number("max_line_length") {
        push("line_width", "max_line_length", OptionValue::Int(n));
    }
    if let Some((v, ..)) = props.get("insert_final_newline")
        && matches!(v.as_str(), "true" | "false")
    {
        push(
            "final_newline",
            "insert_final_newline",
            OptionValue::Bool(v == "true"),
        );
    }
    if let Some((v, ..)) = props.get("csharp_new_line_before_open_brace")
        && matches!(v.as_str(), "all" | "none")
    {
        push(
            "brace_newline",
            "csharp_new_line_before_open_brace",
            OptionValue::String(v.clone()),
        );
    }
    out
}

impl EditorConfig {
    /// Reads the `.editorconfig` files above `file` (an absolute path).
    pub fn for_file(file: &Path) -> EditorConfig {
        let mut found: Vec<(PathBuf, Parsed)> = Vec::new();
        for dir in file.ancestors().skip(1) {
            let Ok(text) = std::fs::read_to_string(dir.join(".editorconfig")) else {
                continue;
            };
            let parsed = parse(&text);
            let root = parsed.root;
            found.push((dir.to_path_buf(), parsed));
            if root {
                break;
            }
        }
        found.reverse();
        EditorConfig::from_files(&found, file)
    }

    fn from_files(files: &[(PathBuf, Parsed)], file: &Path) -> EditorConfig {
        EditorConfig {
            values: map(&properties(files, file)),
        }
    }

    /// For tests and tools that already hold the text of a single `.editorconfig` in `dir`.
    pub fn from_text(dir: &Path, text: &str, file: &Path) -> EditorConfig {
        EditorConfig::from_files(&[(dir.to_path_buf(), parse(text))], file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(text: &str, file: &str) -> Vec<(&'static str, OptionValue)> {
        EditorConfig::from_text(Path::new("/repo"), text, Path::new(file))
            .values
            .into_iter()
            .map(|v| (v.option, v.value))
            .collect()
    }

    #[test]
    fn maps_the_documented_keys() {
        let text = "root = true\n[*]\nindent_style = space\nindent_size = 3\nmax_line_length = 90\ninsert_final_newline = false\n[*.cs]\ncsharp_new_line_before_open_brace = none\n";
        assert_eq!(
            values(text, "/repo/a/B.cs"),
            [
                ("indent_style", OptionValue::String("space".into())),
                ("indent_size", OptionValue::Int(3)),
                ("line_width", OptionValue::Int(90)),
                ("final_newline", OptionValue::Bool(false)),
                ("brace_newline", OptionValue::String("none".into())),
            ]
        );
        assert_eq!(values(text, "/repo/a/b.json").len(), 4);
    }

    #[test]
    fn tab_width_stands_in_for_indent_size() {
        assert_eq!(
            values("[*]\nindent_style = tab\ntab_width = 8\n", "/repo/a.json"),
            [
                ("indent_style", OptionValue::String("tab".into())),
                ("indent_size", OptionValue::Int(8)),
            ]
        );
        assert_eq!(
            values("[*]\nindent_size = tab\ntab_width = 6\n", "/repo/a.json"),
            [("indent_size", OptionValue::Int(6))]
        );
    }

    #[test]
    fn later_sections_win_unset_removes_and_off_is_ignored() {
        let text = "[*]\nindent_size = 2\nmax_line_length = off\n[*.json]\nindent_size = 4\n[a.json]\nindent_size = unset\n";
        assert_eq!(
            values(text, "/repo/x.json"),
            [("indent_size", OptionValue::Int(4))]
        );
        assert!(values(text, "/repo/a.json").is_empty());
        assert_eq!(
            values(text, "/repo/x.yaml"),
            [("indent_size", OptionValue::Int(2))]
        );
    }

    #[test]
    fn section_globs() {
        assert!(section_matches("*.{cs,json}", "src/a.cs"));
        assert!(section_matches("*.cs", "src/deep/a.cs"));
        assert!(!section_matches("src/*.cs", "src/deep/a.cs"));
        assert!(section_matches("src/**.cs", "src/deep/a.cs"));
        assert!(section_matches("/src/*.cs", "src/a.cs"));
        assert!(section_matches("file{1..3}.json", "file2.json"));
        assert!(!section_matches("file{1..3}.json", "file4.json"));
        assert!(section_matches("[ab].json", "a.json"));
        assert!(section_matches("[!ab].json", "c.json"));
    }

    #[test]
    fn closer_files_override_and_root_stops_the_walk() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let nested = dir.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            dir.join(".editorconfig"),
            "[*]\nindent_size = 2\nmax_line_length = 70\n",
        )
        .unwrap();
        std::fs::write(dir.join("a/.editorconfig"), "[*]\nindent_size = 4\n").unwrap();
        let file = nested.join("x.json");
        let v: Vec<_> = EditorConfig::for_file(&file)
            .values
            .into_iter()
            .map(|v| (v.option, v.value))
            .collect();
        assert_eq!(
            v,
            [
                ("indent_size", OptionValue::Int(4)),
                ("line_width", OptionValue::Int(70))
            ]
        );
        std::fs::write(
            dir.join("a/.editorconfig"),
            "root = true\n[*]\nindent_size = 4\n",
        )
        .unwrap();
        assert_eq!(EditorConfig::for_file(&file).values.len(), 1);
    }
}
