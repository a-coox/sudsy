//! The Rust rules of XML; syntax rules that fit a pattern are files in `rules/`.

use crate::generated::kind;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use rowan::{GreenNodeData, GreenTokenData, NodeOrToken};

pub static RULES: &[&dyn Rule] = &[&NoDuplicateInclude];

type Green<'a> = NodeOrToken<&'a GreenNodeData, &'a GreenTokenData>;

/// The children of a green node with their start offsets.
fn children(node: &GreenNodeData, start: u32) -> impl Iterator<Item = (u32, Green<'_>)> {
    let mut at = start;
    node.children().map(move |c| {
        let len = match c {
            NodeOrToken::Node(n) => n.text_len(),
            NodeOrToken::Token(t) => t.text_len(),
        };
        at += u32::from(len);
        (at - u32::from(len), c)
    })
}

fn token_of(kind: SyntaxKind, c: Green<'_>) -> Option<&GreenTokenData> {
    c.into_token().filter(|t| SyntaxKind(t.kind().0) == kind)
}

/// `(element name, Include value)` and the range of the value, for a child element with an
/// `Include` attribute. Read from the green tree: a red node per token and attribute of every
/// element cost most of the lint.
fn include(element: &GreenNodeData, start: u32) -> Option<((&str, &str), TextRange)> {
    let (at, tag) = children(element, start).find_map(|(at, c)| Some((at, c.into_node()?)))?;
    let name = children(tag, at)
        .find_map(|(_, c)| token_of(kind::NAME, c))?
        .text();
    children(tag, at).find_map(|(at, c)| {
        let attribute = c
            .into_node()
            .filter(|a| SyntaxKind(a.kind().0) == kind::ATTRIBUTE)?;
        let key = children(attribute, at).find_map(|(_, c)| token_of(kind::NAME, c))?;
        let (at, value) = children(attribute, at)
            .find_map(|(at, c)| Some((at, token_of(kind::ATT_VALUE, c)?)))?;
        let text = value.text();
        (key.text() == "Include").then(|| {
            let range = TextRange::at(at.into(), value.text_len());
            ((name, &text[1..text.len() - 1]), range)
        })
    })
}

rule! {
    pub NoDuplicateInclude {
        id: "no-duplicate-include",
        category: Correctness,
        default: Warn,
        description: "An element of a project file (`PackageReference`, `ProjectReference`, \
                  `Compile`, ...) has the same `Include` as an earlier element of the same name in \
                  the same parent. The duplicate is a restore warning or a build error.",
        example: "<ItemGroup><Compile Include=\"A.cs\" /><Compile Include=\"A.cs\" /></ItemGroup>   // reported at the second",
        kinds: &[kind::ELEMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut seen = core_rules::unique::Repeats::default();
        let green = node.green();
        for (at, child) in children(&green, node.text_range().start().into()) {
            let NodeOrToken::Node(child) = child else {
                continue;
            };
            if SyntaxKind(child.kind().0) != kind::ELEMENT {
                continue;
            }
            let Some(((name, value), range)) = include(child, at) else {
                continue;
            };
            let message = || format!("`{name}` `{value}` is already included");
            seen.check(ctx, (name, value), range, message, "first included here");
        }
    }
}
