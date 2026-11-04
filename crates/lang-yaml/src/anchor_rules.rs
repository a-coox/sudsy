//! Rules on anchors and aliases: `no-undefined-alias` (correctness) and the opt-in
//! `no-duplicate-anchors` and `no-unused-anchors`. Anchors do not cross document markers.

use crate::generated::kind;
use core_config::Level;
use core_rules::{Category, Edit, FileContext, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::{TextRange, TextSize};
use std::collections::{HashMap, HashSet};

pub struct NoUndefinedAlias;
pub struct NoDuplicateAnchors;
pub struct NoUnusedAnchors;

/// An `&name` or `*name` token of the file.
struct Event<'a> {
    anchor: bool,
    name: &'a str,
    range: TextRange,
    /// The start of the document it is in; names are looked up per document.
    document: Option<TextSize>,
}

/// The anchors and aliases of the file in order. Looks only at the `&` and `*` bytes of the text
/// and asks the tree what they are, so a file without anchors costs one scan and no tree walk.
fn events<'a>(file: &FileContext<'a>) -> Vec<Event<'a>> {
    let mut out = Vec::new();
    for at in memchr::memchr2_iter(b'&', b'*', file.text.as_bytes()) {
        let Some(token) = file
            .root
            .token_at_offset(TextSize::new(at as u32))
            .right_biased()
        else {
            continue;
        };
        let (kind, range) = (token.kind(), token.text_range());
        if !matches!(kind, kind::ANCHOR | kind::ALIAS) || usize::from(range.start()) != at {
            continue;
        }
        out.push(Event {
            anchor: kind == kind::ANCHOR,
            name: &file.text[at + 1..usize::from(range.end())],
            range,
            document: token
                .parent_ancestors()
                .find(|n| n.kind() == kind::DOCUMENT)
                .map(|n| n.text_range().start()),
        });
    }
    out
}

static NO_UNDEFINED_ALIAS: RuleMeta = RuleMeta {
    id: "no-undefined-alias",
    category: Category::Correctness,
    description: "An alias names an anchor that is not defined earlier in the same document.",
    example: "a: *nope   # no `&nope` before it",
    default_level: Level::Error,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for NoUndefinedAlias {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNDEFINED_ALIAS
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut anchors: HashSet<&str> = HashSet::new();
        let mut document = None;
        for e in events(file) {
            if e.document != document {
                document = e.document;
                anchors.clear();
            }
            if e.anchor {
                anchors.insert(e.name);
            } else if !anchors.contains(e.name) {
                ctx.report(e.range, format!("undefined alias `{}`", e.name));
            }
        }
    }
}

static NO_DUPLICATE_ANCHORS: RuleMeta = RuleMeta {
    id: "no-duplicate-anchors",
    category: Category::Suspicious,
    description: "An anchor name is defined again in the same document; an alias refers to the \
                  latest definition above it, which is easy to misread. yamllint \
                  `forbid-duplicated-anchors`.",
    example: "- &a 1\n- &a 2   # reported at the second `&a`",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for NoDuplicateAnchors {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_ANCHORS
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut first: HashMap<&str, TextRange> = HashMap::new();
        let mut document = None;
        for e in events(file).into_iter().filter(|e| e.anchor) {
            if e.document != document {
                document = e.document;
                first.clear();
            }
            match first.get(e.name) {
                Some(&at) => {
                    ctx.report(e.range, format!("duplicate anchor `{}`", e.name))
                        .related(at, "first defined here");
                }
                None => {
                    first.insert(e.name, e.range);
                }
            }
        }
    }
}

static NO_UNUSED_ANCHORS: RuleMeta = RuleMeta {
    id: "no-unused-anchors",
    category: Category::Cleanup,
    description: "An anchor that no alias refers to. The fix removes the anchor, which changes \
                  nothing about the data. yamllint `forbid-unused-anchors`.",
    example: "a: &x 1   # becomes a: 1 when no `*x` follows",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for NoUnusedAnchors {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNUSED_ANCHORS
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let events = events(file);
        let mut used = vec![false; events.len()];
        // An alias refers to the latest anchor of its name above it.
        let mut latest: HashMap<&str, usize> = HashMap::new();
        let mut document = None;
        for (i, e) in events.iter().enumerate() {
            if e.document != document {
                document = e.document;
                latest.clear();
            }
            if e.anchor {
                latest.insert(e.name, i);
            } else if let Some(&anchor) = latest.get(e.name) {
                used[anchor] = true;
            }
        }
        for (e, _) in events.iter().zip(&used).filter(|(e, u)| e.anchor && !**u) {
            ctx.report(e.range, format!("unused anchor `{}`", e.name))
                .fix(Fix::new(
                    "Remove the anchor",
                    FixKind::Safe,
                    vec![Edit::delete(removal(file.text, e.range))],
                ));
        }
    }
}

/// The anchor and the blanks that separated it from its neighbour: the ones after it, or, at
/// the end of a line, the ones before it.
fn removal(text: &str, anchor: TextRange) -> TextRange {
    let blank = |c: char| c == ' ' || c == '\t';
    let (start, end) = (usize::from(anchor.start()), usize::from(anchor.end()));
    let after = text[end..].len() - text[end..].trim_start_matches(blank).len();
    let (from, to) = if text[end + after..].starts_with(['\n', '\r']) || end + after == text.len() {
        let before = text[..start].len() - text[..start].trim_end_matches(blank).len();
        (start - before, end)
    } else {
        (start, end + after)
    };
    TextRange::new(TextSize::new(from as u32), TextSize::new(to as u32))
}
