//! Statement-level lexical rules: `no-empty-statement` and `no-comment-before-block`.

use super::kind;
use core_rules::rule;
use core_rules::tree::{is_code, significant_range};
use core_rules::{Edit, RuleContext};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange};

rule! {
    pub NoEmptyStatement {
        id: "no-empty-statement",
        category: Suspicious,
        default: Off,
        fix: Safe,
        description: "A lone `;` is a statement that does nothing. A statement labelled `label: ;` is \
                  allowed. The fix removes the statement (its whole line when it is alone on \
                  it), or turns an empty loop or `if` body into `{ }`; it is not offered for the \
                  only statement of a switch section. Overlaps compiler CS0642 for `if (x);`.",
        example: "int y = 1;;   // delete the second ;",
        kinds: &[kind::EMPTY_STATEMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(parent) = node.parent() else { return };
        let range = significant_range(node, ctx.file.grammar);
        let edit = match parent.kind() {
            kind::LABELED_STATEMENT => return,
            kind::BLOCK => Some(remove_edit(ctx.file.text, range)),
            kind::SWITCH_SECTION => {
                let statements = parent.children().filter(|c| {
                    !matches!(
                        c.kind(),
                        kind::CASE_SWITCH_LABEL
                            | kind::CASE_PATTERN_SWITCH_LABEL
                            | kind::DEFAULT_SWITCH_LABEL
                    )
                });
                (statements.count() > 1).then(|| remove_edit(ctx.file.text, range))
            }
            kind::GLOBAL_STATEMENT => None,
            _ => Some(Edit::replace(range, "{ }")),
        };
        let fix = edit.map(|edit| ("Remove the empty statement", vec![edit]));
        ctx.report_fix(range, "empty statement", fix);
    }
}

/// Deletes `range`; its whole line too when nothing else is on the line.
fn remove_edit(text: &str, range: TextRange) -> Edit {
    let (start, end) = (usize::from(range.start()), usize::from(range.end()));
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i + 1);
    if text[line_start..start].trim().is_empty() && text[end..line_end].trim().is_empty() {
        let range = TextRange::new((line_start as u32).into(), (line_end as u32).into());
        return Edit::delete(range);
    }
    Edit::delete(range)
}

rule! {
    pub NoCommentBeforeBlock {
        id: "no-comment-before-block",
        category: Style,
        default: Off,
        description: "A comment between the head of an `if`, `else`, `while`, `do`, `for`, `foreach`, \
                  `lock`, `try`, `catch`, `finally`, `switch`, `checked` or `unchecked` and its \
                  `{`. Put it inside the block or above the statement.",
        example: "if (x) // why\n{                  // move the comment above the if",
        kinds: &[
            kind::IF_STATEMENT,
            kind::ELSE_CLAUSE,
            kind::WHILE_STATEMENT,
            kind::DO_STATEMENT,
            kind::FOR_STATEMENT,
            kind::FOR_EACH_STATEMENT,
            kind::FOR_EACH_VARIABLE_STATEMENT,
            kind::LOCK_STATEMENT,
            kind::TRY_STATEMENT,
            kind::CATCH_CLAUSE,
            kind::FINALLY_CLAUSE,
            kind::SWITCH_STATEMENT,
            kind::CHECKED_STATEMENT,
            kind::UNCHECKED_STATEMENT,
        ],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        // The `{` of the statement's own block: a direct child, or the block's first token.
        let open = if node.kind() == kind::SWITCH_STATEMENT {
            node.token(kind::OPEN_BRACE_TOKEN)
        } else {
            node.child(kind::BLOCK)
                .and_then(|b| {
                    b.token(kind::OPEN_BRACE_TOKEN)
                })
        };
        let grammar = ctx.file.grammar;
        let mut previous = open.and_then(|t| t.prev_token());
        while let Some(t) = previous {
            if is_code(&t, grammar) {
                break;
            }
            if grammar.has_tag(t.kind(), "comment") {
                ctx.report(
                    t.text_range(),
                    "no comment between the statement and its block",
                );
            }
            previous = t.prev_token();
        }
    }
}
