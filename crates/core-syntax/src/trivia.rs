//! The trivia attachment rule, shared by every language.
//!
//! Between two consecutive non-trivia tokens lies a run of trivia tokens.
//! * **Trailing** trivia of the first token: the run up to and including the
//!   first trivia token that ends with a line break; if no token ends with a
//!   line break, the whole run.
//! * **Leading** trivia of the second token: the rest.
//!
//! Before the first token everything is leading. A tree encodes attachment by
//! structure: trailing trivia are siblings directly after their token, leading
//! trivia are siblings directly before theirs. Providers must produce trees
//! that satisfy [`check_attachment`]; the C# worker re-attaches Roslyn trivia
//! to match.

use crate::kind::SyntaxKind;
use crate::tree::{SyntaxNode, SyntaxToken};

pub fn is_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

fn ends_line(token: &SyntaxToken) -> bool {
    token.text().chars().next_back().is_some_and(is_line_break)
}

/// Verifies the attachment rule. `is_trivia` classifies token kinds.
pub fn check_attachment(
    root: &SyntaxNode,
    is_trivia: impl Fn(SyntaxKind) -> bool,
) -> Result<(), String> {
    let tokens: Vec<SyntaxToken> = root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .collect();
    let mut prev_real: Option<SyntaxToken> = None;
    let mut run: Vec<SyntaxToken> = Vec::new();
    let check_run =
        |prev: &Option<SyntaxToken>, run: &mut Vec<SyntaxToken>, next: Option<&SyntaxToken>| {
            let cut = match prev {
                None => 0,
                Some(_) => run.iter().position(ends_line).map_or(run.len(), |i| i + 1),
            };
            for (i, t) in run.iter().enumerate() {
                let (owner, side) = if i < cut {
                    (prev, "trailing")
                } else {
                    (&next.cloned(), "leading")
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
        };
    for t in tokens {
        if is_trivia(t.kind()) {
            run.push(t);
        } else {
            check_run(&prev_real, &mut run, Some(&t))?;
            prev_real = Some(t);
        }
    }
    check_run(&prev_real, &mut run, None)
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

    fn print(n: &SyntaxNode) -> String {
        crate::print(n)
    }
}
