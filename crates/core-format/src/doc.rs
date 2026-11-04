//! The layout document: what formatting code produces and the printer consumes.
//!
//! The primitives are those of docs/formatting/engine.md (after Wadler and Prettier):
//! `text`, `line`, `softline`, `hardline`, `group`, `indent`, `align`, `if_break`, `fill`,
//! `conditional_group`, `line_suffix` and `break_parent`.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub enum Doc {
    Nil,
    /// Literal text. It may contain newlines (verbatim code); the printer then restarts the
    /// column count after the last one and does not re-indent the following lines.
    Text(Cow<'static, str>),
    /// Text of one line whose trailing blanks are significant (block scalar content): the
    /// printer does not trim them at the next line break.
    Verbatim(String),
    /// Text that starts a line at column 0 whatever the indentation (a preprocessor directive, or
    /// disabled code): the printer takes back the indentation of a blank line, or first starts a
    /// new line if the current one has content. It breaks the enclosing groups.
    LineStart(String),
    /// Contents that start their own line indented this many levels less than the current
    /// indentation (all of it with `usize::MAX`): the printer takes back the indentation of a
    /// blank line, or first starts a new line. It breaks the enclosing groups.
    Outdent(usize, Box<Doc>),
    /// A hard line break, unless the current line is empty. Starts an own-line comment that
    /// follows text on the line the formatter chose. It breaks the enclosing groups.
    NewlineIfNeeded,
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
    /// Contents indented one level when the group with this id was printed broken (the group
    /// decides the indentation of a later part of the line).
    IndentIfBreak(GroupId, Box<Doc>),
    /// A subtree that is cloned by reference count: layouts that try several candidates share
    /// the documents the candidates have in common instead of copying them (see [`shared`]).
    Shared(Arc<SharedDoc>),
}

/// Names a group so that [`Doc::IndentIfBreak`] can ask whether it was printed broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroupId(u32);

pub fn new_group_id() -> GroupId {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    GroupId(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The contents of [`Doc::Shared`], with whether they force a break (decided once, when shared).
#[derive(Debug, PartialEq)]
pub struct SharedDoc {
    pub doc: Doc,
    pub forced: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub contents: Doc,
    /// Alternative layouts of a conditional group, tried in order after `contents` (which is
    /// the first candidate). Empty for an ordinary group.
    pub states: Vec<Doc>,
    pub broken: bool,
    pub id: Option<GroupId>,
}

impl Doc {
    pub fn is_nil(&self) -> bool {
        matches!(self, Doc::Nil)
    }

    /// Marks groups that contain a forced break as broken. The printer does this itself; call it
    /// directly only to inspect a document.
    pub fn propagate_breaks(&mut self) -> bool {
        match self {
            Doc::Nil | Doc::Text(_) | Doc::Verbatim(_) | Doc::Line | Doc::SoftLine => false,
            Doc::HardLine | Doc::BreakParent | Doc::LineStart(_) | Doc::NewlineIfNeeded => true,
            Doc::Shared(s) => s.forced,
            _ => crate::deep(|| self.propagate_breaks_in()),
        }
    }

    /// Drops the document without recursing: dropping a deep one the usual way uses a stack
    /// frame per level.
    pub fn dispose(self) {
        let mut pending = vec![self];
        while let Some(doc) = pending.pop() {
            match doc {
                Doc::Outdent(_, d)
                | Doc::Indent(d)
                | Doc::Align(_, d)
                | Doc::LineSuffix(d)
                | Doc::IndentIfBreak(_, d) => pending.push(*d),
                Doc::IfBreak { broken, flat } => pending.extend([*broken, *flat]),
                Doc::Concat(parts) | Doc::Fill(parts) => pending.extend(parts),
                Doc::Group(g) => {
                    let g = *g;
                    pending.push(g.contents);
                    pending.extend(g.states);
                }
                Doc::Shared(s) => {
                    if let Ok(s) = Arc::try_unwrap(s) {
                        pending.push(s.doc);
                    }
                }
                _ => {}
            }
        }
    }

    fn propagate_breaks_in(&mut self) -> bool {
        match self {
            Doc::HardLine | Doc::BreakParent | Doc::LineStart(_) | Doc::NewlineIfNeeded => true,
            Doc::Outdent(_, d) => {
                d.propagate_breaks();
                true
            }
            Doc::Nil | Doc::Text(_) | Doc::Verbatim(_) | Doc::Line | Doc::SoftLine => false,
            Doc::Shared(s) => s.forced,
            Doc::IndentIfBreak(_, d) => d.propagate_breaks(),
            Doc::Concat(parts) | Doc::Fill(parts) => parts
                .iter_mut()
                .fold(false, |forced, d| d.propagate_breaks() | forced),
            Doc::Indent(d) | Doc::Align(_, d) | Doc::LineSuffix(d) => d.propagate_breaks(),
            Doc::IfBreak { broken, flat } => broken.propagate_breaks() | flat.propagate_breaks(),
            Doc::Group(g) => {
                let forced = g.contents.propagate_breaks();
                let mut every_candidate = !g.states.is_empty();
                for s in &mut g.states {
                    every_candidate &= s.propagate_breaks();
                }
                // A conditional group picks among its candidates, so a forced break inside one
                // does not force the group itself. When every candidate has one, whichever is
                // chosen breaks the parent.
                if forced && g.states.is_empty() {
                    g.broken = true;
                }
                g.broken || every_candidate
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

/// See [`Doc::NewlineIfNeeded`].
pub fn newline_if_needed() -> Doc {
    Doc::NewlineIfNeeded
}

/// See [`Doc::LineStart`].
pub fn line_start(s: impl Into<String>) -> Doc {
    Doc::LineStart(s.into())
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
        id: None,
    }))
}

/// A group other documents can ask about with [`indent_if_break`].
pub fn group_with_id(id: GroupId, contents: Doc) -> Doc {
    Doc::Group(Box::new(Group {
        contents,
        states: Vec::new(),
        broken: false,
        id: Some(id),
    }))
}

/// `contents`, indented when the group `id` (printed before it) broke.
pub fn indent_if_break(id: GroupId, contents: Doc) -> Doc {
    Doc::IndentIfBreak(id, Box::new(contents))
}

/// Makes `d` cheap to clone. Layouts that offer several candidates (`conditional_group`) build
/// each from the same parts; without sharing, every nesting level would copy its parts once more
/// per candidate.
pub fn shared(mut d: Doc) -> Doc {
    if matches!(d, Doc::Nil | Doc::Text(_) | Doc::Shared(_)) {
        return d;
    }
    let forced = d.propagate_breaks();
    Doc::Shared(Arc::new(SharedDoc { doc: d, forced }))
}

/// A group that is broken regardless of whether it fits.
pub fn group_broken(contents: Doc) -> Doc {
    Doc::Group(Box::new(Group {
        contents,
        states: Vec::new(),
        broken: true,
        id: None,
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
    let states: Vec<Doc> = candidates.into_iter().map(shared).collect();
    Doc::Group(Box::new(Group {
        contents: states[0].clone(),
        states,
        broken: false,
        id: None,
    }))
}

pub fn indent(d: Doc) -> Doc {
    Doc::Indent(Box::new(d))
}

pub fn outdent(levels: usize, d: Doc) -> Doc {
    Doc::Outdent(levels, Box::new(d))
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
