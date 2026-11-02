//! The layout document: what formatting code produces and the printer consumes.
//!
//! The primitives are those of plan.md "Formatting engine" (after Wadler and Prettier):
//! `text`, `line`, `softline`, `hardline`, `group`, `indent`, `align`, `if_break`, `fill`,
//! `conditional_group`, `line_suffix` and `break_parent`.

use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub enum Doc {
    Nil,
    /// Literal text. It may contain newlines (verbatim code); the printer then restarts the
    /// column count after the last one and does not re-indent the following lines.
    Text(Cow<'static, str>),
    /// Text of one line whose trailing blanks are significant (block scalar content): the
    /// printer does not trim them at the next line break.
    Verbatim(String),
    /// A space when the enclosing group fits on one line, else a newline.
    Line,
    /// Nothing when the enclosing group fits on one line, else a newline.
    SoftLine,
    /// Always a newline; forces every enclosing group to break.
    HardLine,
    Concat(Vec<Doc>),
    Group(Box<Group>),
    Indent(Box<Doc>),
    /// Indent by this many extra spaces (not levels).
    Align(usize, Box<Doc>),
    IfBreak {
        broken: Box<Doc>,
        flat: Box<Doc>,
    },
    /// Alternating `content, separator, content, ...`; separators break only where the next
    /// content no longer fits.
    Fill(Vec<Doc>),
    /// Printed at the end of the current line (trailing comments).
    LineSuffix(Box<Doc>),
    /// Forces every enclosing group to break.
    BreakParent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub contents: Doc,
    /// Alternative layouts of a conditional group, tried in order after `contents` (which is
    /// the first candidate). Empty for an ordinary group.
    pub states: Vec<Doc>,
    pub broken: bool,
}

impl Doc {
    pub fn is_nil(&self) -> bool {
        matches!(self, Doc::Nil)
    }

    /// Marks groups that contain a forced break as broken. The printer does this itself; call it
    /// directly only to inspect a document.
    pub fn propagate_breaks(&mut self) -> bool {
        match self {
            Doc::HardLine | Doc::BreakParent => true,
            Doc::Nil | Doc::Text(_) | Doc::Verbatim(_) | Doc::Line | Doc::SoftLine => false,
            Doc::Concat(parts) | Doc::Fill(parts) => parts
                .iter_mut()
                .fold(false, |forced, d| d.propagate_breaks() | forced),
            Doc::Indent(d) | Doc::Align(_, d) | Doc::LineSuffix(d) => d.propagate_breaks(),
            Doc::IfBreak { broken, flat } => broken.propagate_breaks() | flat.propagate_breaks(),
            Doc::Group(g) => {
                let mut forced = g.contents.propagate_breaks();
                for s in &mut g.states {
                    forced |= s.propagate_breaks();
                }
                // A conditional group picks among its candidates, so a forced break inside one
                // does not force the group itself (and does not leak to the parent).
                if forced && g.states.is_empty() {
                    g.broken = true;
                }
                g.broken
            }
        }
    }
}

impl From<&str> for Doc {
    fn from(s: &str) -> Doc {
        text(s)
    }
}

impl From<String> for Doc {
    fn from(s: String) -> Doc {
        Doc::Text(Cow::Owned(s))
    }
}

pub fn nil() -> Doc {
    Doc::Nil
}

pub fn text(s: impl Into<String>) -> Doc {
    let s = s.into();
    if s.is_empty() {
        Doc::Nil
    } else {
        Doc::Text(Cow::Owned(s))
    }
}

/// Text that is a string literal: no allocation, however often the document clones it.
pub fn lit(s: &'static str) -> Doc {
    if s.is_empty() {
        Doc::Nil
    } else {
        Doc::Text(Cow::Borrowed(s))
    }
}

/// Text copied from a source: a single ASCII character (most punctuation) is shared, anything
/// else is copied.
pub fn snippet(s: &str) -> Doc {
    static ASCII: [u8; 128] = {
        let mut table = [0u8; 128];
        let mut i = 0;
        while i < 128 {
            table[i] = i as u8;
            i += 1;
        }
        table
    };
    match s.as_bytes() {
        [] => Doc::Nil,
        &[b] if b < 128 => match std::str::from_utf8(&ASCII[b as usize..b as usize + 1]) {
            Ok(shared) => Doc::Text(Cow::Borrowed(shared)),
            Err(_) => text(s),
        },
        _ => text(s),
    }
}

/// See [`Doc::Verbatim`].
pub fn verbatim(s: impl Into<String>) -> Doc {
    let s = s.into();
    if s.is_empty() {
        Doc::Nil
    } else {
        Doc::Verbatim(s)
    }
}

pub fn line() -> Doc {
    Doc::Line
}

pub fn softline() -> Doc {
    Doc::SoftLine
}

pub fn hardline() -> Doc {
    Doc::HardLine
}

pub fn break_parent() -> Doc {
    Doc::BreakParent
}

pub fn concat(parts: Vec<Doc>) -> Doc {
    Doc::Concat(parts)
}

pub fn group(contents: Doc) -> Doc {
    Doc::Group(Box::new(Group {
        contents,
        states: Vec::new(),
        broken: false,
    }))
}

/// A group that is broken regardless of whether it fits.
pub fn group_broken(contents: Doc) -> Doc {
    Doc::Group(Box::new(Group {
        contents,
        states: Vec::new(),
        broken: true,
    }))
}

/// Tries each candidate in order and keeps the first that fits (the last if none does). A
/// candidate is tried flat, but may contain its own forced breaks, which is how a candidate such
/// as "hug the last argument" is expressed.
pub fn conditional_group(candidates: Vec<Doc>) -> Doc {
    assert!(
        !candidates.is_empty(),
        "conditional_group needs a candidate"
    );
    let contents = candidates[0].clone();
    Doc::Group(Box::new(Group {
        contents,
        states: candidates,
        broken: false,
    }))
}

pub fn indent(d: Doc) -> Doc {
    Doc::Indent(Box::new(d))
}

pub fn align(spaces: usize, d: Doc) -> Doc {
    Doc::Align(spaces, Box::new(d))
}

pub fn if_break(broken: Doc, flat: Doc) -> Doc {
    Doc::IfBreak {
        broken: Box::new(broken),
        flat: Box::new(flat),
    }
}

pub fn fill(parts: Vec<Doc>) -> Doc {
    Doc::Fill(parts)
}

pub fn line_suffix(d: Doc) -> Doc {
    Doc::LineSuffix(Box::new(d))
}

/// `parts` separated by `sep`.
pub fn join(sep: &Doc, parts: impl IntoIterator<Item = Doc>) -> Doc {
    let mut out = Vec::new();
    for (i, p) in parts.into_iter().enumerate() {
        if i > 0 {
            out.push(sep.clone());
        }
        out.push(p);
    }
    Doc::Concat(out)
}
