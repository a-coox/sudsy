//! The data model of a YAML stream, loaded from our own tree: the node graph of YAML 1.2 with
//! core-schema scalar resolution, tags, and aliases expanded. The formatter's safety check
//! compares the model before and after formatting.

use crate::generated::kind as k;
use crate::scalar::{self, block_header, block_value};
use crate::tree::{blank_lines_after, block_parent_indent, is_trivia, token_of};
use core_syntax::{SyntaxElement, SyntaxNode};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    Seq(Vec<Node>),
    Map(Vec<(Node, Node)>),
}

/// A value and its resolved tag (absent for the core schema's own implicit tags).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub tag: Option<String>,
    pub value: Value,
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b || a.is_nan() && b.is_nan(),
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Seq(a), Value::Seq(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            _ => false,
        }
    }
}

const CORE: &str = "tag:yaml.org,2002:";

/// Loads every document of a tree without syntax errors.
pub fn load_tree(root: &SyntaxNode) -> Result<Vec<Node>, String> {
    let mut docs = Vec::new();
    for doc in root.children().filter(|n| n.kind() == k::DOCUMENT) {
        // A lone `...` is not a document.
        let explicit = doc
            .children_with_tokens()
            .any(|e| matches!(&e, SyntaxElement::Token(t) if t.kind() == k::DOC_START));
        if !explicit && doc.children().next().is_none() {
            continue;
        }
        docs.push(Loader::new().document(&doc)?);
    }
    Ok(docs)
}

/// Parses and loads `src`.
pub fn load(src: &str) -> Result<Vec<Node>, String> {
    let parsed = crate::parse(src);
    if let Some(e) = parsed.errors.first() {
        return Err(format!("{} (byte {})", e.message, e.start));
    }
    load_tree(&parsed.tree)
}

struct Loader {
    handles: HashMap<String, String>,
    anchors: HashMap<String, Node>,
}

impl Loader {
    fn new() -> Self {
        Loader {
            handles: HashMap::new(),
            anchors: HashMap::new(),
        }
    }

    fn document(&mut self, doc: &SyntaxNode) -> Result<Node, String> {
        for t in doc.children_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() == k::DIRECTIVE {
                let words: Vec<&str> = t.text().split_whitespace().collect();
                if words[0] == "%TAG" {
                    self.handles
                        .insert(words[1].to_string(), words[2].to_string());
                }
            }
        }
        match doc.children().next() {
            Some(body) => self.node(&body, None),
            None => Ok(null()),
        }
    }

    fn resolve_tag(&self, text: &str) -> String {
        if let Some(v) = text.strip_prefix("!<").and_then(|t| t.strip_suffix('>')) {
            return v.to_string();
        }
        if text == "!" {
            return "!".into();
        }
        // `!!suffix`, `!handle!suffix` or `!suffix`.
        let rest = &text[1..];
        let (handle, suffix) = match rest.find('!') {
            Some(i) => (&text[..i + 2], &rest[i + 1..]),
            None => ("!", rest),
        };
        let prefix = match self.handles.get(handle) {
            Some(p) => p.as_str(),
            None if handle == "!!" => CORE,
            None => "!",
        };
        format!("{prefix}{suffix}")
    }

    /// The node for `n`; `tag` is a resolved tag that applies to it.
    fn node(&mut self, n: &SyntaxNode, tag: Option<String>) -> Result<Node, String> {
        match n.kind() {
            k::PROPERTY_NODE => self.property_node(n),
            k::ALIAS_NODE => {
                let t = token_of(n, k::ALIAS).ok_or("alias without a name")?;
                let name = &t.text()[1..];
                self.anchors
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("undefined alias `{name}`"))
            }
            k::SCALAR => self.scalar(n, tag),
            k::BLOCK_SCALAR => self.block_scalar(n, tag),
            k::BLOCK_MAPPING => {
                let mut pairs = Vec::new();
                for entry in n.children() {
                    pairs.push(self.pair(&entry)?);
                }
                Ok(collection(tag, Value::Map(pairs)))
            }
            k::FLOW_MAPPING => {
                let mut pairs = Vec::new();
                for entry in n.children() {
                    pairs.push(self.pair(&entry)?);
                }
                Ok(collection(tag, Value::Map(pairs)))
            }
            k::BLOCK_SEQUENCE => {
                let mut items = Vec::new();
                for entry in n.children() {
                    items.push(match entry.children().next() {
                        Some(v) => self.node(&v, None)?,
                        None => null(),
                    });
                }
                Ok(collection(tag, Value::Seq(items)))
            }
            k::FLOW_SEQUENCE => {
                let mut items = Vec::new();
                for item in n.children() {
                    items.push(if item.kind() == k::FLOW_PAIR {
                        let pair = self.pair(&item)?;
                        collection(None, Value::Map(vec![pair]))
                    } else {
                        self.node(&item, None)?
                    });
                }
                Ok(collection(tag, Value::Seq(items)))
            }
            other => Err(format!("unexpected node {other:?}")),
        }
    }

    /// A mapping entry: nodes before the `:` are the key, after it the value.
    fn pair(&mut self, entry: &SyntaxNode) -> Result<(Node, Node), String> {
        let (mut key, mut value, mut after_colon) = (None, None, false);
        let wanted = entry
            .children_with_tokens()
            .by_kind(|kind| kind == k::COLON || !kind.is_token());
        for e in wanted {
            match e {
                SyntaxElement::Token(t) if t.kind() == k::COLON => after_colon = true,
                SyntaxElement::Node(n) => {
                    let node = self.node(&n, None)?;
                    if after_colon {
                        value = Some(node);
                    } else {
                        key = Some(node);
                    }
                }
                SyntaxElement::Token(_) => {}
            }
        }
        Ok((key.unwrap_or_else(null), value.unwrap_or_else(null)))
    }

    fn property_node(&mut self, n: &SyntaxNode) -> Result<Node, String> {
        let mut tag = None;
        let mut anchor = None;
        for t in n.children_with_tokens().filter_map(|e| e.into_token()) {
            match t.kind() {
                k::TAG => tag = Some(self.resolve_tag(t.text())),
                k::ANCHOR => anchor = Some(t.text()[1..].to_string()),
                _ => {}
            }
        }
        let mut node = match n.children().next() {
            Some(content) => self.node(&content, tag.clone())?,
            None => self.empty_node(tag.as_deref()),
        };
        // Non-scalar nodes get the tag attached by `node`; keep custom tags on scalars too.
        if let Some(t) = &tag
            && node.tag.is_none()
            && !t.starts_with(CORE)
            && t != "!"
        {
            node.tag = Some(t.clone());
        }
        if let Some(a) = anchor {
            self.anchors.insert(a, node.clone());
        }
        Ok(node)
    }

    fn empty_node(&self, tag: Option<&str>) -> Node {
        match tag {
            Some(t) if t == format!("{CORE}str") => Node {
                tag: None,
                value: Value::Str(String::new()),
            },
            Some(t) if t.starts_with(CORE) || t == "!" => null(),
            Some(t) => Node {
                tag: Some(t.to_string()),
                value: Value::Null,
            },
            None => null(),
        }
    }

    fn scalar(&mut self, n: &SyntaxNode, tag: Option<String>) -> Result<Node, String> {
        let t = n
            .children_with_tokens()
            .by_kind(|kind| kind.is_token() && !is_trivia(kind))
            .find_map(|e| e.into_token())
            .ok_or("scalar without a token")?;
        let (text, plain) = match t.kind() {
            k::PLAIN => (scalar::plain_value(t.text()), true),
            k::SINGLE_QUOTED => (scalar::single_quoted_value(t.text()), false),
            _ => (
                scalar::double_quoted_value(t.text()).ok_or("invalid escape")?,
                false,
            ),
        };
        Ok(resolve_scalar(text, plain, tag.as_deref()))
    }

    fn block_scalar(&mut self, n: &SyntaxNode, tag: Option<String>) -> Result<Node, String> {
        let header = token_of(n, k::BLOCK_HEADER).ok_or("block scalar without a header")?;
        let header_info = block_header(header.text());
        let content_token = token_of(n, k::BLOCK_CONTENT);
        let content = content_token
            .as_ref()
            .map_or_else(String::new, |c| c.text().to_string());
        let blank_after = blank_lines_after(content_token.as_ref().unwrap_or(&header));
        let text = block_value(header_info, &content, block_parent_indent(n), blank_after);
        Ok(resolve_scalar(text, false, tag.as_deref()))
    }
}

fn null() -> Node {
    Node {
        tag: None,
        value: Value::Null,
    }
}

/// A collection with its tag; the core schema's `!!map` and `!!seq` are the defaults.
fn collection(tag: Option<String>, value: Value) -> Node {
    let tag = tag
        .filter(|t| !t.starts_with(&format!("{CORE}map")) && !t.starts_with(&format!("{CORE}seq")));
    Node { tag, value }
}

/// The core schema: plain scalars are resolved by their text, everything else is a string; an
/// explicit core tag forces the type.
fn resolve_scalar(text: String, plain: bool, tag: Option<&str>) -> Node {
    let plain_value = |text: String| -> Value { resolve_plain(&text).unwrap_or(Value::Str(text)) };
    let (tag_out, value) = match tag {
        None => (
            None,
            if plain {
                plain_value(text)
            } else {
                Value::Str(text)
            },
        ),
        Some("!") => (None, Value::Str(text)),
        Some(t) => match t.strip_prefix(CORE) {
            Some("str") => (None, Value::Str(text)),
            Some(kind @ ("int" | "float" | "bool" | "null")) => match resolve_plain(&text) {
                Some(Value::Int(i)) if kind == "float" => (None, Value::Float(i as f64)),
                Some(v) if kind_matches(kind, &v) => (None, v),
                _ => (Some(t.to_string()), Value::Str(text)),
            },
            Some(_) => (Some(t.to_string()), Value::Str(text)),
            None => (
                Some(t.to_string()),
                if plain {
                    plain_value(text)
                } else {
                    Value::Str(text)
                },
            ),
        },
    };
    Node {
        tag: tag_out,
        value,
    }
}

fn kind_matches(kind: &str, v: &Value) -> bool {
    matches!(
        (kind, v),
        ("int", Value::Int(_))
            | ("float", Value::Float(_) | Value::Int(_))
            | ("bool", Value::Bool(_))
            | ("null", Value::Null)
    )
}

/// YAML 1.2 core schema resolution of a plain scalar; `None` means a string.
pub fn resolve_plain(s: &str) -> Option<Value> {
    match s {
        "" | "~" | "null" | "Null" | "NULL" => return Some(Value::Null),
        "true" | "True" | "TRUE" => return Some(Value::Bool(true)),
        "false" | "False" | "FALSE" => return Some(Value::Bool(false)),
        ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => {
            return Some(Value::Float(f64::INFINITY));
        }
        "-.inf" | "-.Inf" | "-.INF" => return Some(Value::Float(f64::NEG_INFINITY)),
        ".nan" | ".NaN" | ".NAN" => return Some(Value::Float(f64::NAN)),
        _ => {}
    }
    if let Some(h) = s.strip_prefix("0x")
        && !h.is_empty()
        && h.bytes().all(|c| c.is_ascii_hexdigit())
    {
        return i128::from_str_radix(h, 16).ok().map(Value::Int);
    }
    if let Some(o) = s.strip_prefix("0o")
        && !o.is_empty()
        && o.bytes().all(|c| (b'0'..=b'7').contains(&c))
    {
        return i128::from_str_radix(o, 8).ok().map(Value::Int);
    }
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    if !body.is_empty() && body.bytes().all(|c| c.is_ascii_digit()) {
        return match s.parse::<i128>() {
            Ok(i) => Some(Value::Int(i)),
            Err(_) => s.parse::<f64>().ok().map(Value::Float),
        };
    }
    if is_core_float(body) {
        return s.parse::<f64>().ok().map(Value::Float);
    }
    None
}

/// `(\.[0-9]+ | [0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?`
fn is_core_float(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let s = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - s
    };
    let int_digits = digits(&mut i);
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac = digits(&mut i);
        if int_digits == 0 && frac == 0 {
            return false;
        }
    } else if int_digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return false;
        }
    }
    i == b.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> Value {
        load(src).unwrap().remove(0).value
    }

    #[test]
    fn core_schema_resolution() {
        assert_eq!(one("12"), Value::Int(12));
        assert_eq!(one("0o17"), Value::Int(15));
        assert_eq!(one("0xFF"), Value::Int(255));
        assert_eq!(one("007"), Value::Int(7));
        assert_eq!(one("1.5e3"), Value::Float(1500.0));
        assert_eq!(one(".5"), Value::Float(0.5));
        assert_eq!(one("1."), Value::Float(1.0));
        assert_eq!(one("true"), Value::Bool(true));
        assert_eq!(one("~"), Value::Null);
        // YAML 1.2: these are strings.
        for s in [
            "yes", "no", "on", "off", "y", "0b101", "1_000", "0o8", "1e", "+",
        ] {
            assert_eq!(one(s), Value::Str(s.into()), "{s}");
        }
    }

    #[test]
    fn tags_force_types() {
        assert_eq!(one("!!str 12"), Value::Str("12".into()));
        assert_eq!(one("!!float 1"), Value::Float(1.0));
        assert_eq!(one("'12'"), Value::Str("12".into()));
    }

    #[test]
    fn aliases_expand() {
        let docs = load("a: &x [1, 2]\nb: *x\n").unwrap();
        let Value::Map(pairs) = &docs[0].value else {
            panic!()
        };
        assert_eq!(pairs[0].1, pairs[1].1);
    }

    #[test]
    fn block_scalars_use_the_parent_indentation() {
        assert_eq!(
            one("- |1\n  a\n"),
            Value::Seq(vec![Node {
                tag: None,
                value: Value::Str(" a\n".into())
            }])
        );
    }
}
