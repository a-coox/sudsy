//! Preprocessor directives: `directive-keyword-space` (`no-regions` and `no-region-in-body` are
//! pattern files in `rules/`).

use super::kind;
use core_config::Level;
use core_rules::{Category, Edit, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode};

pub struct DirectiveKeywordSpace;

static DIRECTIVE_KEYWORD_SPACE: RuleMeta = RuleMeta {
    id: "directive-keyword-space",
    category: Category::Style,
    description: "No space between `#` and the directive keyword (`#if`, not `# if`).",
    example: "# if DEBUG   // write: #if DEBUG",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

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

impl Rule for DirectiveKeywordSpace {
    fn meta(&self) -> &'static RuleMeta {
        &DIRECTIVE_KEYWORD_SPACE
    }

    fn kinds(&self) -> &[SyntaxKind] {
        DIRECTIVES
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
        ctx.report(
            keyword.text_range(),
            "the directive keyword should follow `#` directly",
        )
        .fix(Fix::new(
            "Remove the space after `#`",
            FixKind::Safe,
            vec![Edit::delete(space.text_range())],
        ));
    }
}
