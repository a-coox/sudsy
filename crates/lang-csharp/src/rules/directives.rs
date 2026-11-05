//! Preprocessor directives: `directive-keyword-space` (`no-regions` and `no-region-in-body` are
//! pattern files in `rules/`).

use super::kind;
use core_rules::rule;
use core_rules::{Edit, RuleContext};
use core_syntax::{SyntaxKind, SyntaxNode};

/// Every directive node kind.
static DIRECTIVES: &[SyntaxKind] = &[
    kind::IF_DIRECTIVE_TRIVIA,
    kind::ELIF_DIRECTIVE_TRIVIA,
    kind::ELSE_DIRECTIVE_TRIVIA,
    kind::END_IF_DIRECTIVE_TRIVIA,
    kind::REGION_DIRECTIVE_TRIVIA,
    kind::END_REGION_DIRECTIVE_TRIVIA,
    kind::ERROR_DIRECTIVE_TRIVIA,
    kind::WARNING_DIRECTIVE_TRIVIA,
    kind::BAD_DIRECTIVE_TRIVIA,
    kind::DEFINE_DIRECTIVE_TRIVIA,
    kind::UNDEF_DIRECTIVE_TRIVIA,
    kind::LINE_DIRECTIVE_TRIVIA,
    kind::LINE_SPAN_DIRECTIVE_TRIVIA,
    kind::PRAGMA_WARNING_DIRECTIVE_TRIVIA,
    kind::PRAGMA_CHECKSUM_DIRECTIVE_TRIVIA,
    kind::REFERENCE_DIRECTIVE_TRIVIA,
    kind::LOAD_DIRECTIVE_TRIVIA,
    kind::NULLABLE_DIRECTIVE_TRIVIA,
];

fn hash_of(node: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind::HASH_TOKEN)
}

rule! {
    pub DirectiveKeywordSpace {
        id: "directive-keyword-space",
        category: Style,
        default: Off,
        fix: Safe,
        description: "No space between `#` and the directive keyword (`#if`, not `# if`).",
        example: "# if DEBUG   // write: #if DEBUG",
        kinds: DIRECTIVES,
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(space) = hash_of(node).and_then(|h| h.next_token()) else {
            return;
        };
        if space.kind() != kind::WHITESPACE_TRIVIA {
            return;
        }
        let Some(keyword) = space.next_token() else {
            return;
        };
        ctx.report_fix(
            keyword.text_range(),
            "the directive keyword should follow `#` directly",
            (
                "Remove the space after `#`",
                vec![Edit::delete(space.text_range())],
            ),
        );
    }
}
