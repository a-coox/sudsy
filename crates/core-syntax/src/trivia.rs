//! The trivia attachment rule, shared by every language.
//!
//! Between two consecutive non-trivia tokens lies a run of trivia elements.
//! * **Trailing** trivia of the first token: the run up to and including the
//!   first trivia element that ends with a line break; if no element ends with
//!   a line break, the whole run.
//! * **Leading** trivia of the second token: the rest.
//!
//! Before the first token everything is leading. A tree encodes attachment by
//! structure: trailing trivia are siblings directly after their token, leading
//! trivia are siblings directly before theirs. Providers must produce trees
//! that satisfy [`check_attachment`]; the C# worker re-attaches Roslyn trivia
//! to match.
//!
//! A trivia element is a trivia token, or a node of a trivia kind (a C# doc
//! comment or directive). Such a structured trivia is one element of the run;
//! its own tokens follow the same rule inside it, as if it were a tree of its
//! own.

use crate::kind::SyntaxKind;
use crate::tree::{SyntaxElement, SyntaxNode};
use rowan::WalkEvent;
use std::ops::Range;

/// The owner of each trivia token of a flat token list (hand-written parsers): `real` holds the
/// indices of the non-trivia tokens, `leading[i]` and `trailing[i]` the index ranges of the trivia
/// tokens that `real[i]` owns, by the attachment rule above.
pub struct Attached {
    pub real: Vec<usize>,
    pub leading: Vec<Range<usize>>,
    pub trailing: Vec<Range<usize>>,
}

/// Applies the attachment rule to `n` tokens; `text(i)` is the text of token `i`.
pub fn attach<'a>(
    n: usize,
    is_trivia: impl Fn(usize) -> bool,
    text: impl Fn(usize) -> &'a str,
) -> Attached {
    let real: Vec<usize> = (0..n).filter(|&i| !is_trivia(i)).collect();
    let mut leading = Vec::with_capacity(real.len());
    let mut trailing: Vec<Range<usize>> = Vec::with_capacity(real.len());
    let mut prev_end = 0; // index after the previous real token
    for &r in &real {
        // Trivia tokens prev_end..r: the rest of the previous token's line is trailing.
        let run = prev_end..r;
        let cut = if prev_end == 0 {
            run.start
        } else {
            let ends_line = |i: usize| text(i).chars().next_back().is_some_and(is_line_break);
            run.clone()
                .find(|&i| ends_line(i))
                .map_or(run.end, |i| i + 1)
        };
        if let Some(last) = trailing.last_mut() {
            *last = run.start..cut;
        }
        leading.push(cut..r);
        trailing.push(r + 1..r + 1);
        prev_end = r + 1;
    }
    Attached {
        real,
        leading,
        trailing,
    }
}

pub fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

fn ends_line(element: &SyntaxElement) -> bool {
    match element {
        SyntaxElement::Token(t) => t.text().chars().next_back().is_some_and(is_line_break),
        SyntaxElement::Node(n) => {
            let text = n.text();
            let len = text.len();
            len > rowan::TextSize::from(0)
                && text
                    .char_at(len - rowan::TextSize::from(1))
                    .is_some_and(is_line_break)
        }
    }
}

/// Verifies the attachment rule. `is_trivia` classifies token and node kinds.
pub fn check_attachment(
    root: &SyntaxNode,
    is_trivia: impl Fn(SyntaxKind) -> bool,
) -> Result<(), String> {
    check_scope(root, &is_trivia)
}

fn check_scope(scope: &SyntaxNode, is_trivia: &impl Fn(SyntaxKind) -> bool) -> Result<(), String> {
    let mut prev_real: Option<SyntaxElement> = None;
    let mut run: Vec<SyntaxElement> = Vec::new();
    let mut preorder = scope.preorder_with_tokens();
    while let Some(event) = preorder.next() {
        let WalkEvent::Enter(element) = event else {
            continue;
        };
        match &element {
            SyntaxElement::Node(n) if n == scope => continue,
            SyntaxElement::Node(n) if is_trivia(n.kind()) => {
                preorder.skip_subtree();
                check_scope(n, is_trivia)?;
                run.push(element);
            }
            SyntaxElement::Node(_) => continue,
            SyntaxElement::Token(t) if is_trivia(t.kind()) => run.push(element),
            SyntaxElement::Token(_) => {
                check_run(prev_real.as_ref(), &mut run, Some(&element))?;
                prev_real = Some(element);
            }
        }
    }
    check_run(prev_real.as_ref(), &mut run, None)
}

fn check_run(
    prev: Option<&SyntaxElement>,
    run: &mut Vec<SyntaxElement>,
    next: Option<&SyntaxElement>,
) -> Result<(), String> {
    let cut = match prev {
        None => 0,
        Some(_) => run.iter().position(ends_line).map_or(run.len(), |i| i + 1),
    };
    for (i, t) in run.iter().enumerate() {
        let (owner, side) = if i < cut {
            (prev, "trailing")
        } else {
            (next, "leading")
        };
        if let Some(owner) = owner
            && owner.parent() != t.parent()
        {
            return Err(format!(
                "{side} trivia at {:?} is not a sibling of its token at {:?}",
                t.text_range(),
                owner.text_range()
            ));
        }
    }
    run.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::LanguageId;
    use crate::tree::TreeBuilder;

    const ROOT: SyntaxKind = SyntaxKind::new(LanguageId::JSON, false, 0);
    const INNER: SyntaxKind = SyntaxKind::new(LanguageId::JSON, false, 1);
    const WORD: SyntaxKind = SyntaxKind::new(LanguageId::JSON, true, 0);
    const WS: SyntaxKind = SyntaxKind::new(LanguageId::JSON, true, 1);
    const NL: SyntaxKind = SyntaxKind::new(LanguageId::JSON, true, 2);

    fn build(trailing_inside: bool) -> SyntaxNode {
        // "a \n  b" : a [trailing: ' ', '\n'] ; b [leading: '  ']
        let mut b = TreeBuilder::new();
        b.start_node(ROOT);
        b.start_node(INNER);
        b.token(WORD, "a");
        if trailing_inside {
            b.token(WS, " ");
            b.token(NL, "\n");
        }
        b.finish_node();
        if !trailing_inside {
            b.token(WS, " ");
            b.token(NL, "\n");
        }
        b.token(WS, "  ");
        b.token(WORD, "b");
        b.finish_node();
        SyntaxNode::new_root(b.finish())
    }

    #[test]
    fn accepts_and_rejects() {
        let triv = |k: SyntaxKind| k == WS || k == NL;
        assert_eq!(print(&build(true)), "a \n  b");
        assert!(check_attachment(&build(true), triv).is_ok());
        assert!(check_attachment(&build(false), triv).is_err());
    }

    #[test]
    fn a_structured_trivia_is_one_element_and_follows_the_rule_inside() {
        // `a /*..*/\n b`, where the comment is a node holding two tokens of its own.
        const DOC: SyntaxKind = SyntaxKind::new(LanguageId::JSON, false, 2);
        const TOK: SyntaxKind = SyntaxKind::new(LanguageId::JSON, true, 3);
        let triv = |k: SyntaxKind| k == WS || k == NL || k == DOC;
        let build = |inner_trailing_outside: bool| {
            let mut b = TreeBuilder::new();
            b.start_node(ROOT);
            b.token(WORD, "a");
            b.token(WS, " ");
            b.start_node(DOC);
            b.start_node(INNER);
            b.token(TOK, "x");
            if !inner_trailing_outside {
                b.token(WS, " ");
            }
            b.finish_node();
            if inner_trailing_outside {
                b.token(WS, " ");
            }
            b.token(TOK, "y");
            b.token(NL, "\n");
            b.finish_node();
            b.token(WORD, "b");
            b.finish_node();
            SyntaxNode::new_root(b.finish())
        };
        assert!(check_attachment(&build(false), triv).is_ok());
        assert!(check_attachment(&build(true), triv).is_err());
    }

    fn print(n: &SyntaxNode) -> String {
        crate::print(n)
    }
}
