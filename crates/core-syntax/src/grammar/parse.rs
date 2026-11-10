use super::*;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("grammar line {line}: {message}")]
pub struct GrammarError {
    pub line: usize,
    pub message: String,
}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, GrammarError> {
    Err(GrammarError {
        line,
        message: message.into(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Lexeme {
    Ident(String),
    Str(String),
    Tag(String),
    Int(u32),
    Sym(char),
}

struct Decl {
    line: usize,
    lexemes: Vec<Lexeme>,
}

fn lex_line(line_no: usize, text: &str, out: &mut Vec<Lexeme>) -> Result<(), GrammarError> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '/' if chars.get(i + 1) == Some(&'/') => break,
            '"' => {
                let mut s = String::new();
                i += 1;
                loop {
                    match chars.get(i) {
                        None => return err(line_no, "unterminated string"),
                        Some('"') => break,
                        Some('\\') => {
                            i += 1;
                            match chars.get(i) {
                                Some(&e @ ('"' | '\\')) => s.push(e),
                                _ => return err(line_no, "bad escape (use \\\" or \\\\)"),
                            }
                        }
                        Some(&ch) => s.push(ch),
                    }
                    i += 1;
                }
                i += 1;
                out.push(Lexeme::Str(s));
            }
            '#' => {
                let start = i + 1;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
                    i += 1;
                }
                if i == start {
                    return err(line_no, "empty tag");
                }
                out.push(Lexeme::Tag(chars[start..i].iter().collect()));
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                match s.parse() {
                    Ok(n) => out.push(Lexeme::Int(n)),
                    Err(_) => return err(line_no, "number too large"),
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    i += 1;
                }
                out.push(Lexeme::Ident(chars[start..i].iter().collect()));
            }
            '=' | ':' | '?' | '*' => {
                out.push(Lexeme::Sym(c));
                i += 1;
            }
            other => return err(line_no, format!("unexpected character {other:?}")),
        }
    }
    Ok(())
}

fn split_decls(source: &str) -> Result<Vec<Decl>, GrammarError> {
    let mut decls: Vec<Decl> = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line_no = i + 1;
        let mut lexemes = Vec::new();
        lex_line(line_no, raw, &mut lexemes)?;
        if lexemes.is_empty() {
            continue;
        }
        if raw.starts_with(char::is_whitespace) {
            match decls.last_mut() {
                Some(d) => d.lexemes.append(&mut lexemes),
                None => return err(line_no, "continuation line before any declaration"),
            }
        } else {
            decls.push(Decl {
                line: line_no,
                lexemes,
            });
        }
    }
    Ok(decls)
}

fn content_hash(decls: &[Decl]) -> String {
    let mut h = Sha256::new();
    for d in decls {
        for l in &d.lexemes {
            let (tag, text) = match l {
                Lexeme::Ident(s) => (b'i', s.clone()),
                Lexeme::Str(s) => (b's', s.clone()),
                Lexeme::Tag(s) => (b't', s.clone()),
                Lexeme::Int(n) => (b'n', n.to_string()),
                Lexeme::Sym(c) => (b'y', c.to_string()),
            };
            h.update([tag]);
            h.update(text.as_bytes());
            h.update([0]);
        }
        h.update(b"\n");
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Cursor over one declaration's lexemes.
struct Cursor<'a> {
    line: usize,
    rest: &'a [Lexeme],
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&'a Lexeme> {
        self.rest.first()
    }
    fn next(&mut self) -> Option<&'a Lexeme> {
        let l = self.rest.first();
        if l.is_some() {
            self.rest = &self.rest[1..];
        }
        l
    }
    fn ident(&mut self, what: &str) -> Result<String, GrammarError> {
        match self.next() {
            Some(Lexeme::Ident(s)) => Ok(s.clone()),
            _ => err(self.line, format!("expected {what}")),
        }
    }
    fn string(&mut self, what: &str) -> Result<String, GrammarError> {
        match self.next() {
            Some(Lexeme::Str(s)) => Ok(s.clone()),
            _ => err(self.line, format!("expected {what}")),
        }
    }
    fn tags(&mut self) -> Vec<String> {
        let mut tags = Vec::new();
        while let Some(Lexeme::Tag(t)) = self.peek() {
            tags.push(t.clone());
            self.next();
        }
        tags
    }
}

/// An item before names are resolved to kinds.
struct RawItem {
    label: Option<String>,
    target: RawTarget,
    cardinality: Cardinality,
}
enum RawTarget {
    Name(String),
    Tag(String),
}

struct RawKind {
    line: usize,
    name: String,
    is_token: bool,
    text: Option<String>,
    tags: Vec<String>,
    body: Vec<RawItem>,
}

fn parse_body(c: &mut Cursor) -> Result<Vec<RawItem>, GrammarError> {
    let mut items = Vec::new();
    while let Some(l) = c.next() {
        let (label, target) = match l {
            Lexeme::Tag(t) => (None, RawTarget::Tag(t.clone())),
            Lexeme::Ident(first) => {
                if c.peek() == Some(&Lexeme::Sym(':')) {
                    c.next();
                    let target = match c.next() {
                        Some(Lexeme::Ident(n)) => RawTarget::Name(n.clone()),
                        Some(Lexeme::Tag(t)) => RawTarget::Tag(t.clone()),
                        _ => return err(c.line, "expected a kind or #tag after the label"),
                    };
                    (Some(first.clone()), target)
                } else {
                    (None, RawTarget::Name(first.clone()))
                }
            }
            _ => return err(c.line, "expected a body item"),
        };
        let cardinality = match c.peek() {
            Some(Lexeme::Sym('?')) => {
                c.next();
                Cardinality::Optional
            }
            Some(Lexeme::Sym('*')) => {
                c.next();
                Cardinality::Many
            }
            _ => Cardinality::One,
        };
        items.push(RawItem {
            label,
            target,
            cardinality,
        });
    }
    Ok(items)
}

pub(super) fn parse(source: &str) -> Result<Grammar, GrammarError> {
    let decls = split_decls(source)?;
    let hash = content_hash(&decls);

    let mut language: Option<(String, LanguageId, String)> = None;
    let mut comments = CommentSyntax::default();
    let mut suppressions = Vec::new();
    let mut raw: Vec<RawKind> = Vec::new();

    for d in &decls {
        let mut c = Cursor {
            line: d.line,
            rest: &d.lexemes,
        };
        let keyword = c.ident("a declaration keyword")?;
        match keyword.as_str() {
            "language" => {
                let name = c.ident("language name")?;
                let id = match c.next() {
                    Some(Lexeme::Int(n)) if (1..=LanguageId::MAX as u32).contains(n) => *n as u8,
                    _ => return err(d.line, "language id must be 1..=31"),
                };
                let title = match c.peek() {
                    Some(Lexeme::Str(_)) => c.string("display name")?,
                    _ => name.clone(),
                };
                if language
                    .replace((name, LanguageId::new(id), title))
                    .is_some()
                {
                    return err(d.line, "duplicate language declaration");
                }
            }
            "comments" => {
                while let Some(l) = c.next() {
                    match l {
                        Lexeme::Ident(k) if k == "line" => {
                            comments.line = Some(c.string("line comment prefix")?)
                        }
                        Lexeme::Ident(k) if k == "block" => {
                            comments.block = Some((
                                c.string("block comment start")?,
                                c.string("block comment end")?,
                            ))
                        }
                        _ => return err(d.line, "expected `line` or `block`"),
                    }
                }
            }
            "suppress" => {
                let role = c.ident("suppression role")?;
                let marker = c.string("suppression marker")?;
                suppressions.push(Suppression { role, marker });
            }
            "token" | "node" => {
                let is_token = keyword == "token";
                let name = c.ident("a kind name")?;
                let mut text = None;
                if is_token && let Some(Lexeme::Str(s)) = c.peek() {
                    text = Some(s.clone());
                    c.next();
                }
                let tags = c.tags();
                let mut body = Vec::new();
                if c.peek() == Some(&Lexeme::Sym('=')) {
                    if is_token {
                        return err(d.line, "tokens have no body");
                    }
                    c.next();
                    body = parse_body(&mut c)?;
                }
                if c.peek().is_some() {
                    return err(d.line, "unexpected trailing input");
                }
                raw.push(RawKind {
                    line: d.line,
                    name,
                    is_token,
                    text,
                    tags,
                    body,
                });
            }
            other => return err(d.line, format!("unknown declaration `{other}`")),
        }
    }

    let Some((language, language_id, title)) = language else {
        return err(1, "missing `language` declaration");
    };
    resolve(
        language,
        language_id,
        title,
        comments,
        suppressions,
        raw,
        hash,
    )
}

fn resolve(
    language: String,
    language_id: LanguageId,
    title: String,
    comments: CommentSyntax,
    suppressions: Vec<Suppression>,
    raw: Vec<RawKind>,
    hash: String,
) -> Result<Grammar, GrammarError> {
    // Pass 1: assign ids.
    let mut by_name: HashMap<String, SyntaxKind> = HashMap::new();
    let (mut n_nodes, mut n_tokens) = (0u32, 0u32);
    let mut kinds = Vec::new();
    for r in &raw {
        let counter = if r.is_token {
            &mut n_tokens
        } else {
            &mut n_nodes
        };
        if *counter >= SyntaxKind::MAX_INDEX {
            return err(r.line, "too many kinds of this class (max 8388608)");
        }
        let kind = SyntaxKind::new(language_id, r.is_token, *counter);
        *counter += 1;
        if by_name.insert(r.name.clone(), kind).is_some() {
            return err(r.line, format!("duplicate kind `{}`", r.name));
        }
        kinds.push(kind);
    }

    // Tag membership: tag -> Some(is_token) when its members are uniform, None when mixed.
    // Mixed tags are fine as categories but cannot be used as a field type.
    let mut tag_class: HashMap<&str, Option<bool>> = HashMap::new();
    for r in &raw {
        for t in &r.tags {
            let class = tag_class.entry(t).or_insert(Some(r.is_token));
            if *class != Some(r.is_token) {
                *class = None;
            }
        }
    }

    // Pass 2: resolve bodies.
    let mut nodes = Vec::new();
    let mut tokens = Vec::new();
    for (r, kind) in raw.iter().zip(kinds) {
        let mut body = Vec::new();
        let mut labels = HashSet::new();
        for it in &r.body {
            let target = match &it.target {
                RawTarget::Name(n) => match by_name.get(n) {
                    Some(k) => Target::Kind(*k),
                    None => return err(r.line, format!("unknown kind `{n}`")),
                },
                RawTarget::Tag(t) => match tag_class.get(t.as_str()) {
                    Some(Some(_)) => Target::Tag(t.clone()),
                    Some(None) => {
                        return err(r.line, format!("tag #{t} is used on both nodes and tokens"));
                    }
                    None => return err(r.line, format!("no kind carries tag #{t}")),
                },
            };
            if let Some(l) = &it.label
                && !labels.insert(l.clone())
            {
                return err(r.line, format!("duplicate label `{l}`"));
            }
            body.push(Item {
                label: it.label.clone(),
                target,
                cardinality: it.cardinality.clone(),
            });
        }
        let def = KindDef {
            name: r.name.clone(),
            kind,
            text: r.text.clone(),
            tags: r.tags.clone(),
            body,
        };
        if r.is_token {
            tokens.push(def)
        } else {
            nodes.push(def)
        }
    }

    let mut grammar = Grammar {
        language,
        language_id,
        title,
        comments,
        suppressions,
        token_flags: super::hot_flags(&tokens),
        node_flags: super::hot_flags(&nodes),
        nodes,
        tokens,
        by_name,
        hash,
        bodies: Vec::new(),
        skip: Vec::new(),
        tag_nodes: HashMap::new(),
    };
    grammar.resolve();
    Ok(grammar)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
// sample
language mini 7
comments line "//" block "/*" "*/"
suppress disable-next-line "sudsy-disable-next-line"
token Space #trivia
token Comma "," #separator
token Str #literal
node Pair #key-value = key:Str Comma value:#value
node Array #value = items:#value* // trailing comment
node Lit #value = value:#literal
node Long =
   a:Str
   b:Str?
"#;

    #[test]
    fn loads_a_grammar() {
        let g = Grammar::parse(SAMPLE).unwrap();
        assert_eq!(g.language, "mini");
        assert_eq!(g.language_id, LanguageId::new(7));
        assert_eq!(g.comments.line.as_deref(), Some("//"));
        assert_eq!(g.comments.block, Some(("/*".into(), "*/".into())));
        assert_eq!(g.suppressions.len(), 1);
        assert_eq!(g.tokens().len(), 3);
        assert_eq!(g.nodes().len(), 4);
        let comma = g.kind_by_name("Comma").unwrap();
        assert!(comma.is_token());
        assert_eq!(g.def(comma).unwrap().text.as_deref(), Some(","));
        assert!(g.has_tag(comma, "separator"));
        let pair = g.def(g.kind_by_name("Pair").unwrap()).unwrap();
        assert_eq!(pair.fields().count(), 2);
        assert_eq!(
            g.def(g.kind_by_name("Long").unwrap()).unwrap().body.len(),
            2
        );
        assert!(g.is_trivia(g.kind_by_name("Space").unwrap()));
        assert_eq!(g.kinds_with_tag("value").count(), 2);
    }

    #[test]
    fn hash_ignores_comments_and_layout_but_not_order() {
        let a = Grammar::parse(SAMPLE).unwrap();
        let b = Grammar::parse(
            &SAMPLE
                .replace("// sample", "// other\n\n")
                .replace("  ", " "),
        )
        .unwrap();
        assert_eq!(a.hash(), b.hash());
        let swapped = SAMPLE.replace(
            "token Space #trivia\ntoken Comma \",\" #separator",
            "token Comma \",\" #separator\ntoken Space #trivia",
        );
        assert_ne!(a.hash(), Grammar::parse(&swapped).unwrap().hash());
    }

    #[test]
    fn rejects_bad_grammars() {
        let cases = [
            ("node A", "missing `language`"),
            ("language x 1\nnode A\nnode A", "duplicate kind"),
            ("language x 1\nnode A = b:B", "unknown kind"),
            ("language x 1\nnode A = b:#nope", "no kind carries"),
            ("language x 1\ntoken T = A", "tokens have no body"),
            ("language x 0", "language id"),
            (
                "language x 1\ntoken T #a\nnode N #a = x:#a",
                "both nodes and tokens",
            ),
            ("language x 1\ntoken T\nnode N = a:T a:T", "duplicate label"),
            ("language x 1\nnode N = \"s\"", "expected a body item"),
            ("  node N", "continuation"),
        ];
        for (src, msg) in cases {
            let e = Grammar::parse(src).unwrap_err();
            assert!(e.message.contains(msg), "{src:?}: {e}");
        }
    }
}
