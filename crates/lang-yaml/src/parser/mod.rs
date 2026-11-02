//! A lossless recursive-descent YAML 1.2 parser with error recovery.
//!
//! The parser follows the productions of the specification, with `n` the indentation of the
//! enclosing block collection. It does not build the tree itself: it reports events (`Start`,
//! real tokens, `Finish`) and [`builder`] adds the trivia between tokens and builds the tree. A
//! syntax error aborts the rest of the document: what is left of it becomes one `ERROR` node of
//! `Unknown` tokens, so every byte is still in the tree.
//!
//! * [`scan`]: characters, columns, trivia, properties and the scalar scanners.
//! * [`flow`]: flow nodes and collections. [`block`]: documents, block collections and block scalars.

mod block;
mod builder;
mod flow;
mod scan;

use crate::generated::kind as k;
use builder::{Ev, build};
use core_syntax::{SyntaxKind, SyntaxNode};
use provider_api::ParseError;

pub struct Parsed {
    pub tree: SyntaxNode,
    pub errors: Vec<ParseError>,
}

pub fn parse(src: &str) -> Parsed {
    let mut p = Parser::new(src);
    p.check_characters();
    p.stream();
    let (tree, errors) = build(src, &p.events, p.errors);
    Parsed { tree, errors }
}

/// Deeper nesting is an error: it bounds the recursion of the parser, the formatter and the
/// destructor of the tree.
pub const MAX_DEPTH: usize = 100;

/// A syntax error was recorded; unwind to the document.
pub(crate) struct Abort;
pub(crate) type R<T> = Result<T, Abort>;

pub(crate) struct Parser<'a> {
    pub(crate) src: &'a str,
    pub(crate) b: &'a [u8],
    pub(crate) pos: usize,
    /// End of the last token pushed.
    pub(crate) last_end: usize,
    pub(crate) events: Vec<Ev>,
    pub(crate) errors: Vec<ParseError>,
    /// Nodes started and not yet finished.
    pub(crate) open: usize,
    /// Depth of flow collections around the current position.
    pub(crate) in_flow: usize,
    /// Depth of nested nodes, bounded by [`MAX_DEPTH`].
    pub(crate) depth: usize,
    /// Smallest column of a continuation line of a flow scalar or flow collection (`n + 1`).
    pub(crate) min_col: usize,
    /// Tag handles declared by `%TAG` directives of the current document.
    pub(crate) handles: Vec<String>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src,
            b: src.as_bytes(),
            pos: 0,
            last_end: 0,
            events: Vec::new(),
            errors: Vec::new(),
            open: 0,
            in_flow: 0,
            depth: 0,
            min_col: 0,
            handles: Vec::new(),
        }
    }

    pub(crate) fn start(&mut self, kind: SyntaxKind) {
        self.events.push(Ev::Start(kind));
        self.open += 1;
    }

    pub(crate) fn finish(&mut self) {
        self.events.push(Ev::Finish);
        self.open -= 1;
    }

    /// Opens nodes before the events from `cp` on (outermost first).
    pub(crate) fn wrap_from(&mut self, cp: usize, kinds: &[SyntaxKind]) {
        for kind in kinds.iter().rev() {
            self.events.insert(cp, Ev::Start(*kind));
            self.open += 1;
        }
    }

    pub(crate) fn tok(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        self.events.push(Ev::Tok(kind, start, end));
        self.pos = end;
        self.last_end = end;
    }

    /// Enters one nesting level.
    pub(crate) fn descend(&mut self) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return self.fail("too-deep", "nesting is too deep");
        }
        Ok(())
    }

    pub(crate) fn error_at(&mut self, start: usize, len: usize, code: &str, message: &str) {
        self.errors.push(ParseError {
            start: start as u32,
            len: len.max(1) as u32,
            code: code.into(),
            message: message.into(),
        });
    }

    /// Records an error at the current position and unwinds.
    pub(crate) fn fail<T>(&mut self, code: &str, message: &str) -> R<T> {
        self.error_at(self.pos, 1, code, message);
        Err(Abort)
    }

    /// Characters outside YAML's printable set are errors wherever they are.
    fn check_characters(&mut self) {
        let mut bad = Vec::new();
        for (i, c) in self.src.char_indices() {
            let ok = matches!(c, '\t' | '\n' | '\r' | ' '..='~' | '\u{85}' | '\u{a0}'..='\u{d7ff}'
                | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}');
            if !ok {
                bad.push((i, c.len_utf8()));
            }
        }
        for (i, len) in bad {
            self.error_at(
                i,
                len,
                "invalid-character",
                "a character YAML does not allow",
            );
        }
    }

    /// Everything after the current position up to the next document marker becomes an `ERROR`
    /// node of `Unknown` tokens, after closing the nodes that were open inside the document.
    pub(crate) fn recover(&mut self, depth: usize) {
        while self.open > depth {
            self.finish();
        }
        self.pos = self.last_end;
        self.start(SyntaxKind::ERROR);
        loop {
            let look = self.skip_trivia();
            if look.eof || look.marker {
                break;
            }
            let s = self.pos;
            let mut e = s;
            while e < self.b.len() && !matches!(self.b[e], b' ' | b'\t' | b'\n' | b'\r') {
                e += 1;
            }
            while !self.src.is_char_boundary(e) {
                e += 1;
            }
            self.tok(k::UNKNOWN, s, e);
        }
        self.finish();
    }
}
