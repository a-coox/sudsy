//! The rules of JavaScript and TypeScript: pattern files in `rules/*.yaml`, and the `check`
//! family they name for ESLint's core rules that read the syntax tree. `cargo xtask new-rule js
//! <id> --rust` adds a module under `rules/` and lists it here.

mod dupes;
mod escapes;
mod flow;
mod misc;
mod ts;

use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::{Diagnostic, Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

pub static RULES: &[&dyn Rule] = &[];

pub static FAMILIES: &[Family] = &[FAMILY];

const FAMILY: Family = Family {
    name: "check",
    build,
};

pub(super) struct Check {
    meta: &'static RuleMeta,
    message: String,
    kinds: &'static [SyntaxKind],
    run: fn(&Check, &SyntaxNode, &mut RuleContext<'_>),
}

type Entry = (
    &'static str,
    &'static [SyntaxKind],
    fn(&Check, &SyntaxNode, &mut RuleContext<'_>),
);

const CHECKS: &[Entry] = &[
    ("dupe-keys", &[k::OBJECT_EXPRESSION], dupes::dupe_keys),
    (
        "dupe-class-members",
        &[k::CLASS_BODY],
        dupes::dupe_class_members,
    ),
    (
        "duplicate-case",
        &[k::SWITCH_STATEMENT],
        dupes::duplicate_case,
    ),
    ("dupe-else-if", &[k::IF_STATEMENT], dupes::dupe_else_if),
    (
        "empty",
        &[k::BLOCK_STATEMENT, k::SWITCH_STATEMENT],
        misc::empty,
    ),
    ("useless-catch", &[k::TRY_STATEMENT], misc::useless_catch),
    (
        "self-assign",
        &[k::ASSIGNMENT_EXPRESSION],
        misc::self_assign,
    ),
    (
        "cond-assign",
        &[
            k::IF_STATEMENT,
            k::WHILE_STATEMENT,
            k::DO_WHILE_STATEMENT,
            k::FOR_STATEMENT,
            k::CONDITIONAL_EXPRESSION,
        ],
        misc::cond_assign,
    ),
    (
        "use-isnan",
        &[k::BINARY_EXPRESSION, k::SWITCH_STATEMENT, k::SWITCH_CASE],
        misc::use_isnan,
    ),
    ("valid-typeof", &[k::BINARY_EXPRESSION], misc::valid_typeof),
    (
        "extra-boolean-cast",
        &[k::UNARY_EXPRESSION, k::CALL_EXPRESSION, k::NEW_EXPRESSION],
        misc::extra_boolean_cast,
    ),
    (
        "unsafe-finally",
        &[
            k::RETURN_STATEMENT,
            k::THROW_STATEMENT,
            k::BREAK_STATEMENT,
            k::CONTINUE_STATEMENT,
        ],
        flow::unsafe_finally,
    ),
    ("unreachable", &[k::PROGRAM], flow::unreachable),
    (
        "getter-return",
        &[k::METHOD_DEFINITION, k::OBJECT_PROPERTY],
        flow::getter_return,
    ),
    ("require-yield", &[k::FUNCTION], flow::require_yield),
    (
        "useless-escape",
        &[k::STRING_LITERAL, k::TEMPLATE_LITERAL, k::REG_EXP_LITERAL],
        escapes::useless_escape,
    ),
    (
        "regex-spaces",
        &[k::REG_EXP_LITERAL, k::NEW_EXPRESSION, k::CALL_EXPRESSION],
        escapes::regex_spaces,
    ),
    (
        "empty-character-class",
        &[k::REG_EXP_LITERAL],
        escapes::empty_character_class,
    ),
    (
        "irregular-whitespace",
        &[k::PROGRAM],
        escapes::irregular_whitespace,
    ),
    (
        "loss-of-precision",
        &[k::NUMERIC_LITERAL],
        escapes::loss_of_precision,
    ),
    ("ban-ts-comment", &[k::PROGRAM], ts::ban_ts_comment),
    (
        "extra-non-null",
        &[k::TS_NON_NULL_EXPRESSION],
        ts::extra_non_null,
    ),
    (
        "misused-new",
        &[
            k::METHOD_DEFINITION,
            k::TS_CONSTRUCT_SIGNATURE_DECLARATION,
            k::TS_METHOD_SIGNATURE,
        ],
        ts::misused_new,
    ),
    (
        "prefer-as-const",
        &[k::TS_LITERAL_TYPE],
        ts::prefer_as_const,
    ),
    (
        "inferrable-types",
        &[
            k::VARIABLE_DECLARATOR,
            k::FORMAL_PARAMETER,
            k::PROPERTY_DEFINITION,
            k::ACCESSOR_PROPERTY,
        ],
        ts::inferrable_types,
    ),
    ("array-type", &[k::TS_TYPE_REFERENCE], ts::array_type),
    (
        "triple-slash-reference",
        &[k::PROGRAM],
        ts::triple_slash_reference,
    ),
    (
        "adjacent-overloads",
        &[
            k::CLASS_BODY,
            k::PROGRAM,
            k::TS_MODULE_BLOCK,
            k::TS_INTERFACE_BODY,
            k::FUNCTION_BODY,
            k::BLOCK_STATEMENT,
            k::TS_TYPE_LITERAL,
        ],
        ts::adjacent_overloads,
    ),
];

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let name = p.text("check")?;
    let Some((_, kinds, run)) = CHECKS.iter().find(|(n, ..)| *n == name) else {
        return Err(format!("param `check`: unknown check `{name}`"));
    };
    let rule = Check {
        meta,
        message: p.text("message")?,
        kinds,
        run: *run,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Check {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        (self.run)(self, node, ctx);
    }
}

impl Check {
    /// Reports at `range`; the message's `{name}` is `name`.
    fn hit<'a>(
        &self,
        ctx: &'a mut RuleContext<'_>,
        range: TextRange,
        name: &str,
    ) -> &'a mut Diagnostic {
        ctx.report(range, self.message.replace("{name}", name))
    }
}

/// The span of `n` without its surrounding trivia.
fn span(ctx: &RuleContext<'_>, n: &SyntaxNode) -> TextRange {
    significant_range(n, ctx.file.grammar)
}

/// The own tokens of `n` that are code (not trivia).
fn own_code<'g>(n: &SyntaxNode, g: &'g Grammar) -> impl Iterator<Item = SyntaxToken> + 'g {
    (n.children_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
}

/// The first token of `n` that is code (the node's own first token may be leading trivia).
fn code_token(n: &SyntaxNode, g: &Grammar) -> Option<SyntaxToken> {
    (n.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| !g.is_trivia(t.kind()))
}

/// Does `n` have an own code token with this text?
fn has_own(n: &SyntaxNode, g: &Grammar, text: &str) -> bool {
    own_code(n, g).any(|t| t.text() == text)
}

/// The text of the code tokens of `n` joined: the node without its whitespace and comments.
fn code_text(n: &SyntaxNode, g: &Grammar) -> String {
    (n.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
        .map(|t| t.text().to_string())
        .collect()
}

/// The expression inside any number of parentheses.
fn un(mut n: SyntaxNode) -> SyntaxNode {
    while n.kind() == k::PARENTHESIZED_EXPRESSION {
        match n.children().next() {
            Some(inner) => n = inner,
            None => break,
        }
    }
    n
}

/// The operator of a unary, binary, logical or assignment expression.
fn operator(n: &SyntaxNode, g: &Grammar) -> String {
    let ops = |t: &SyntaxToken| {
        matches!(
            t.kind(),
            k::OPERATOR | k::KEYWORD | k::L_ANGLE | k::R_ANGLE | k::PIPE
        )
    };
    own_code(n, g)
        .find(ops)
        .map_or_else(String::new, |t| t.text().to_string())
}

/// The condition of an `if`, `while`, `do`, `for` or `?:`, as written (parentheses included).
fn test_of(n: &SyntaxNode) -> Option<SyntaxNode> {
    match n.kind() {
        k::DO_WHILE_STATEMENT => n.children().nth(1),
        k::FOR_STATEMENT => {
            let mut semicolons = 0;
            for e in n.children_with_tokens() {
                match e {
                    SyntaxElement::Token(t) if t.kind() == k::SEMICOLON => semicolons += 1,
                    SyntaxElement::Node(c) if semicolons == 1 => return Some(c),
                    _ => {}
                }
            }
            None
        }
        _ => n.children().next(),
    }
}
