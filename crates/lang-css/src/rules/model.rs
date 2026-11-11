//! What the rules read from the tree: names of properties and at-rules, the parts of selectors,
//! and the tokens and functions of a value.

use crate::generated::kind as k;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxToken, TextRange};

pub fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHITESPACE | k::NEWLINE | k::COMMENT | k::LINE_COMMENT
    )
}

/// The tokens below `node` that are not trivia, groups and functions included.
pub fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !is_trivia(t.kind()))
}

/// The text of the tokens of `node` without trivia, joined with nothing between.
pub fn plain(node: &SyntaxNode) -> String {
    tokens(node).map(|t| t.text().to_string()).collect()
}

/// The property of a declaration, lowercase; `None` for a variable of SCSS or Less (`$a`, `@a`).
pub fn property(decl: &SyntaxNode) -> Option<(String, TextRange)> {
    let p = decl.child(k::PROPERTY)?;
    let name = plain(&p);
    (!name.starts_with(['$', '@'])).then(|| {
        let range = p.text_range();
        let first = tokens(&p).next().map_or(range, |t| t.text_range());
        // Custom properties are case sensitive.
        (
            if is_custom(&name) {
                name
            } else {
                name.to_ascii_lowercase()
            },
            first,
        )
    })
}

pub fn is_custom(name: &str) -> bool {
    name.starts_with("--")
}

/// The name of an at-rule without `@`, lowercase.
pub fn at_name(at: &SyntaxNode) -> String {
    at.token(k::AT_KEYWORD)
        .map(|t| t.text()[1..].to_ascii_lowercase())
        .unwrap_or_default()
}

/// A name without its vendor prefix (`-webkit-box` is `box`).
pub fn unprefixed(name: &str) -> &str {
    match name.strip_prefix('-') {
        Some(rest) if !rest.starts_with('-') => rest.split_once('-').map_or(name, |(_, n)| n),
        _ => name,
    }
}

pub fn is_keyframes(at: &SyntaxNode) -> bool {
    unprefixed(&at_name(at)) == "keyframes"
}

/// Whether `node` is a direct child of the block of a `@keyframes`.
pub fn in_keyframes(node: &SyntaxNode) -> bool {
    node.parent()
        .and_then(|b| b.parent())
        .is_some_and(|a| a.kind() == k::AT_RULE && is_keyframes(&a))
}

/// The value node of a declaration.
pub fn value(decl: &SyntaxNode) -> Option<SyntaxNode> {
    decl.child(k::VALUE)
}

/// The statements of a block or stylesheet, with their kind.
pub fn items(container: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    let block = container.child(k::BLOCK);
    block
        .as_ref()
        .unwrap_or(container)
        .children()
        .collect::<Vec<_>>()
        .into_iter()
}

/// The name of a function node.
pub fn function_name(f: &SyntaxNode) -> Option<SyntaxToken> {
    f.token(k::IDENT)
}

/// The own children of a group that are not its brackets or trivia, as elements.
pub fn group_items(group: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut items: Vec<_> = group
        .children_with_tokens()
        .filter(|e| !is_trivia(e.kind()))
        .collect();
    if items.len() >= 2 {
        items.remove(0);
        items.pop();
    }
    items
}

/// The number part and the unit of a `Dimension` token text.
pub fn split_dimension(text: &str) -> (&str, &str) {
    let mut end = 0;
    let b = text.as_bytes();
    while end < b.len() {
        let c = b[end];
        let exp = (c == b'e' || c == b'E')
            && (b.get(end + 1).is_some_and(u8::is_ascii_digit)
                || b.get(end + 1).is_some_and(|n| *n == b'-' || *n == b'+')
                    && b.get(end + 2).is_some_and(u8::is_ascii_digit))
            || (end > 0 && matches!(b[end - 1], b'e' | b'E') && (c == b'-' || c == b'+'));
        if c.is_ascii_digit() || c == b'.' || (end == 0 && (c == b'-' || c == b'+')) || exp {
            end += 1;
        } else {
            break;
        }
    }
    text.split_at(end)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Class,
    Id,
    Attribute,
    PseudoClass,
    PseudoElement,
    Type,
    Universal,
    Nesting,
}

/// One piece of a compound selector: `.a` is `(Class, "a")`; an attribute is its name.
#[derive(Clone, Debug)]
pub struct Piece {
    pub part: Part,
    pub name: String,
    pub range: TextRange,
    /// Adds nothing to the specificity (`:where()` and what it holds; `:not` itself).
    pub weightless: bool,
}

/// A compound selector, with what the pseudo-class arguments inside it hold.
#[derive(Clone, Debug, Default)]
pub struct Compound {
    pub pieces: Vec<Piece>,
}

/// A complex selector: compounds with the combinators between them (`" "` for descendant).
#[derive(Clone, Debug, Default)]
pub struct Complex {
    pub compounds: Vec<Compound>,
    pub combinators: Vec<(String, TextRange)>,
    /// The text, whitespace collapsed.
    pub text: String,
    pub range: Option<TextRange>,
}

impl Complex {
    pub fn pieces(&self) -> impl Iterator<Item = &Piece> {
        self.compounds.iter().flat_map(|c| c.pieces.iter())
    }

    pub fn count(&self, part: Part) -> usize {
        self.pieces().filter(|p| p.part == part).count()
    }

    /// The (ids, classes and the like, types) of the selector.
    pub fn specificity(&self) -> (usize, usize, usize) {
        let n = |f: fn(Part) -> bool| self.pieces().filter(|p| !p.weightless && f(p.part)).count();
        (
            n(|p| p == Part::Id),
            n(|p| matches!(p, Part::Class | Part::Attribute | Part::PseudoClass)),
            n(|p| matches!(p, Part::Type | Part::PseudoElement)),
        )
    }
}

/// The complex selectors of a rule's prelude, split at the commas. Functional pseudo-classes
/// (`:not(.a)`) add what their arguments hold to the compound they are in.
pub fn selectors(prelude: &SyntaxNode) -> Vec<Complex> {
    let mut out = Vec::new();
    let mut scan = Scan::default();
    scan.run(prelude, &mut out);
    out
}

#[derive(Default)]
struct Scan {
    cur: Complex,
    compound: Compound,
    space: bool,
    /// What the last colons and dots announce.
    after: Option<Part>,
    colons: usize,
    text: String,
}

impl Scan {
    fn finish_compound(&mut self) {
        if !self.compound.pieces.is_empty() {
            self.cur.compounds.push(std::mem::take(&mut self.compound));
        }
    }

    fn finish_complex(&mut self, out: &mut Vec<Complex>) {
        self.finish_compound();
        if !self.cur.compounds.is_empty() {
            let mut done = std::mem::take(&mut self.cur);
            done.text = std::mem::take(&mut self.text).trim().to_string();
            out.push(done);
        }
        self.text.clear();
        self.space = false;
        self.after = None;
    }

    /// Starts a new compound when whitespace came before the next piece.
    fn piece(&mut self, part: Part, name: String, range: TextRange) {
        if self.space && !self.compound.pieces.is_empty() {
            self.finish_compound();
            self.cur.combinators.push((" ".into(), range));
        }
        self.space = false;
        self.cur.range = Some(self.cur.range.map_or(range, |r| r.cover(range)));
        self.compound.pieces.push(Piece {
            part,
            name,
            range,
            weightless: false,
        });
    }

    fn run(&mut self, node: &SyntaxNode, out: &mut Vec<Complex>) {
        for e in node.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) => self.token(&t, out),
                SyntaxElement::Node(n) if n.kind() == k::FUNCTION => {
                    let name = function_name(&n)
                        .map(|t| t.text().to_string())
                        .unwrap_or_default();
                    let range = n.text_range();
                    let part = match self.after.take() {
                        Some(p) => p,
                        None => Part::Type,
                    };
                    self.colons = 0;
                    self.text.push_str(&name);
                    let lower = name.to_ascii_lowercase();
                    self.piece(part, name, range);
                    let matching = part == Part::PseudoClass
                        && matches!(
                            lower.as_str(),
                            "is" | "not" | "has" | "where" | "matches" | "-webkit-any" | "-moz-any"
                        );
                    if let Some(own) = self.compound.pieces.last_mut() {
                        own.weightless = matching;
                    }
                    if let Some(args) = n.child(k::GROUP) {
                        self.text.push_str(&args.text().to_string());
                        let mut inner = Scan::default();
                        let mut found = Vec::new();
                        inner.run(&args, &mut found);
                        inner.finish_complex(&mut found);
                        self.compound.pieces.extend(found.iter().flat_map(|c| {
                            c.pieces().cloned().map(|mut p| {
                                p.weightless |= lower == "where";
                                p
                            })
                        }));
                    }
                }
                SyntaxElement::Node(n) => {
                    // `[attr=value]`, or a parenthesized group that is not an argument.
                    let text = n.text().to_string();
                    self.text.push_str(&text);
                    if n.first_token().is_some_and(|t| t.kind() == k::L_BRACKET) {
                        let name = tokens(&n)
                            .nth(1)
                            .map(|t| t.text().to_string())
                            .unwrap_or_default();
                        self.piece(Part::Attribute, name, n.text_range());
                    }
                }
            }
        }
        if node.kind() == k::PRELUDE {
            self.finish_complex(out);
        }
    }

    fn token(&mut self, t: &SyntaxToken, out: &mut Vec<Complex>) {
        let (kind, text) = (t.kind(), t.text());
        if is_trivia(kind) {
            self.space = true;
            if !self.text.ends_with(' ') {
                self.text.push(' ');
            }
            return;
        }
        if kind == k::COMMA {
            return self.finish_complex(out);
        }
        self.text.push_str(text);
        let range = t.text_range();
        match kind {
            k::COLON => {
                self.colons += 1;
                self.after = Some(if self.colons > 1 {
                    Part::PseudoElement
                } else {
                    Part::PseudoClass
                });
            }
            k::DELIM if matches!(text, ">" | "+" | "~") => {
                self.finish_compound();
                self.cur.combinators.push((text.into(), range));
                self.space = false;
            }
            k::DELIM if text == "." => self.after = Some(Part::Class),
            k::DELIM if text == "*" => self.piece(Part::Universal, "*".into(), range),
            k::DELIM if text == "&" => self.piece(Part::Nesting, "&".into(), range),
            k::HASH => self.piece(Part::Id, text[1..].to_string(), range),
            // A keyframe selector.
            k::PERCENTAGE => self.piece(Part::Type, text.to_string(), range),
            k::IDENT => {
                let part = self.after.take().unwrap_or(Part::Type);
                self.colons = 0;
                self.piece(part, text.to_string(), range);
            }
            _ => {}
        }
    }
}
