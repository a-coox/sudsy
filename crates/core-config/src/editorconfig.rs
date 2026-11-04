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
//! | `end_of_line` (`lf`, `crlf`; `cr` ignored) | `end_of_line` |
//! | `csharp_new_line_before_open_brace` (`all`, `none`, or a list of elements) | `brace_newline`, `brace_same_line`, `wrapped_brace_line` (C#) |
//! | `csharp_new_line_before_else`, `_catch`, `_finally` (`true`, `false`) | `else_line`, `catch_line`, `finally_line` (C#) |
//! | `csharp_indent_labels` (`flush_left`, `one_less_than_current`, `no_change`) | `label_indent` (C#) |
//!
//! The other `csharp_*` / `dotnet_*` formatting keys are not mapped. One that a file's sections set
//! to something other than the behavior Sudsy has (a fixed one: `csharp_space_after_cast = true`)
//! is reported in [`EditorConfig::ignored`]; code-style keys (`dotnet_style_*`, `csharp_style_*`,
//! `dotnet_diagnostic.*`, naming rules) belong to the SDK and are silent.
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

/// A formatting key of a section that applies to the file, which Sudsy does not honor.
#[derive(Clone, Debug, PartialEq)]
pub struct IgnoredKey {
    pub key: String,
    pub value: String,
    pub file: PathBuf,
    pub line: usize,
}

/// The mapped values that apply to one file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditorConfig {
    pub values: Vec<EditorConfigValue>,
    pub ignored: Vec<IgnoredKey>,
}

/// The elements `csharp_new_line_before_open_brace` lists, as `brace_same_line` names them.
const BRACE_ELEMENTS: [&str; 9] = [
    "accessors",
    "anonymous_methods",
    "anonymous_types",
    "control_blocks",
    "lambdas",
    "methods",
    "object_collection_array_initializers",
    "properties",
    "types",
];

/// Formatting keys without an option and the value of the behavior Sudsy has instead. A section
/// that sets one to another value is reported as ignored.
const FIXED: [(&str, &str); 29] = [
    ("csharp_indent_case_contents", "true"),
    ("csharp_indent_switch_labels", "true"),
    ("csharp_indent_block_contents", "true"),
    ("csharp_indent_braces", "false"),
    (
        "csharp_new_line_before_members_in_object_initializers",
        "true",
    ),
    ("csharp_new_line_before_members_in_anonymous_types", "true"),
    ("csharp_new_line_between_query_expression_clauses", "true"),
    ("csharp_space_after_cast", "false"),
    (
        "csharp_space_after_keywords_in_control_flow_statements",
        "true",
    ),
    ("csharp_space_between_parentheses", "false"),
    ("csharp_space_before_colon_in_inheritance_clause", "true"),
    ("csharp_space_after_colon_in_inheritance_clause", "true"),
    ("csharp_space_around_binary_operators", "before_and_after"),
    (
        "csharp_space_between_method_declaration_parameter_list_parentheses",
        "false",
    ),
    (
        "csharp_space_between_method_declaration_empty_parameter_list_parentheses",
        "false",
    ),
    (
        "csharp_space_between_method_declaration_name_and_open_parenthesis",
        "false",
    ),
    (
        "csharp_space_between_method_call_parameter_list_parentheses",
        "false",
    ),
    (
        "csharp_space_between_method_call_empty_parameter_list_parentheses",
        "false",
    ),
    (
        "csharp_space_between_method_call_name_and_opening_parenthesis",
        "false",
    ),
    ("csharp_space_after_comma", "true"),
    ("csharp_space_before_comma", "false"),
    ("csharp_space_after_dot", "false"),
    ("csharp_space_before_dot", "false"),
    ("csharp_space_after_semicolon_in_for_statement", "true"),
    ("csharp_space_before_semicolon_in_for_statement", "false"),
    ("csharp_space_around_declaration_statements", "false"),
    ("csharp_space_before_open_square_brackets", "false"),
    ("csharp_space_between_empty_square_brackets", "false"),
    ("csharp_space_between_square_brackets", "false"),
];

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

/// `csharp_new_line_before_open_brace`: `all`, `none`, or the elements that get a new line.
fn map_braces(props: &Props, push: &mut impl FnMut(&'static str, &str, OptionValue)) {
    const KEY: &str = "csharp_new_line_before_open_brace";
    let Some((v, ..)) = props.get(KEY) else {
        return;
    };
    let string = |s: &str| OptionValue::String(s.into());
    match v.as_str() {
        "all" => {
            push("brace_newline", KEY, string("all"));
            push("brace_same_line", KEY, OptionValue::List(Vec::new()));
            push("wrapped_brace_line", KEY, string("creation"));
        }
        "none" => {
            push("brace_newline", KEY, string("none"));
            push("wrapped_brace_line", KEY, string("none"));
        }
        list => {
            let listed: Vec<&str> = list.split(',').map(str::trim).collect();
            let same_line = BRACE_ELEMENTS
                .into_iter()
                .filter(|e| !listed.contains(e))
                .map(string)
                .collect();
            push("brace_newline", KEY, string("all"));
            push("brace_same_line", KEY, OptionValue::List(same_line));
        }
    }
}

/// The formatting keys of the file's sections that Sudsy has no option for and does not follow.
fn ignored(props: &Props) -> Vec<IgnoredKey> {
    FIXED
        .iter()
        .filter_map(|(key, fixed)| {
            let (value, file, line) = props.get(*key)?;
            (value != fixed).then(|| IgnoredKey {
                key: (*key).into(),
                value: value.clone(),
                file: file.clone(),
                line: *line,
            })
        })
        .collect()
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
    if let Some((v, ..)) = props.get("end_of_line")
        && matches!(v.as_str(), "lf" | "crlf")
    {
        push("end_of_line", "end_of_line", OptionValue::String(v.clone()));
    }
    map_braces(props, &mut push);
    for (key, option) in [
        ("csharp_new_line_before_else", "else_line"),
        ("csharp_new_line_before_catch", "catch_line"),
        ("csharp_new_line_before_finally", "finally_line"),
    ] {
        if let Some((v, ..)) = props.get(key)
            && matches!(v.as_str(), "true" | "false")
        {
            let line = if v == "true" { "new_line" } else { "same_line" };
            push(option, key, OptionValue::String(line.into()));
        }
    }
    if let Some((v, ..)) = props.get("csharp_indent_labels") {
        let indent = match v.as_str() {
            "flush_left" => Some("flush_left"),
            "one_less_than_current" => Some("one_less"),
            "no_change" => Some("no_change"),
            _ => None,
        };
        if let Some(indent) = indent {
            push(
                "label_indent",
                "csharp_indent_labels",
                OptionValue::String(indent.into()),
            );
        }
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
        let props = properties(files, file);
        EditorConfig {
            values: map(&props),
            ignored: ignored(&props),
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
                ("wrapped_brace_line", OptionValue::String("none".into())),
            ]
        );
        assert_eq!(values(text, "/repo/a/b.json").len(), 4);
    }

    fn word_list(items: &[&str]) -> OptionValue {
        OptionValue::List(
            items
                .iter()
                .map(|i| OptionValue::String((*i).into()))
                .collect(),
        )
    }

    #[test]
    fn a_list_of_elements_puts_the_others_on_the_line_of_their_head() {
        let got = values(
            "[*.cs]\ncsharp_new_line_before_open_brace = Types, methods\n",
            "/repo/A.cs",
        );
        assert_eq!(
            got,
            [
                ("brace_newline", OptionValue::String("all".into())),
                (
                    "brace_same_line",
                    word_list(&[
                        "accessors",
                        "anonymous_methods",
                        "anonymous_types",
                        "control_blocks",
                        "lambdas",
                        "object_collection_array_initializers",
                        "properties",
                    ])
                ),
            ]
        );
    }

    #[test]
    fn clause_keywords_and_labels() {
        let text = "[*.cs]\ncsharp_new_line_before_else = false\ncsharp_new_line_before_catch = true\ncsharp_new_line_before_finally = maybe\ncsharp_indent_labels = one_less_than_current\n";
        assert_eq!(
            values(text, "/repo/A.cs"),
            [
                ("else_line", OptionValue::String("same_line".into())),
                ("catch_line", OptionValue::String("new_line".into())),
                ("label_indent", OptionValue::String("one_less".into())),
            ]
        );
    }

    #[test]
    fn unmapped_formatting_keys_are_reported_when_they_differ_from_sudsy() {
        let text = "[*.cs]\ncsharp_space_after_cast = true\ncsharp_space_after_comma = true\ndotnet_style_qualification_for_field = true\ncsharp_style_var_for_built_in_types = true\n[*.json]\ncsharp_space_before_comma = true\n";
        let ec = EditorConfig::from_text(Path::new("/repo"), text, Path::new("/repo/A.cs"));
        let keys: Vec<_> = ec.ignored.iter().map(|i| (&*i.key, i.line)).collect();
        assert_eq!(keys, [("csharp_space_after_cast", 2)]);
    }

    #[test]
    fn end_of_line_maps_lf_and_crlf_only() {
        let at = |v: &str| values(&format!("[*]\nend_of_line = {v}\n"), "/repo/a.json");
        assert_eq!(
            at("crlf"),
            [("end_of_line", OptionValue::String("crlf".into()))]
        );
        assert_eq!(
            at("lf"),
            [("end_of_line", OptionValue::String("lf".into()))]
        );
        assert!(at("cr").is_empty());
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
