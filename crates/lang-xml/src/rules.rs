//! The Rust rules of XML; syntax rules that fit a pattern are files in `rules/`.

use crate::generated::{Attribute, kind};
use core_rules::{Rule, RuleContext, rule};
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;
use core_syntax::ast::AstNode;

pub static RULES: &[&dyn Rule] = &[&NoDuplicateInclude];

/// `(element name, Include value)` and the range of the value, for a child element with an
/// `Include` attribute.
fn include(child: &SyntaxNode) -> Option<(String, String, core_syntax::TextRange)> {
    let tag = child.children().next()?;
    let name = tag.token(kind::NAME)?;
    tag.children().find_map(|a| {
        let a = Attribute::cast(a)?;
        let (key, value) = (a.key()?, a.value()?);
        let text = value.text();
        (key.text() == "Include").then(|| {
            (
                name.text().into(),
                text[1..text.len() - 1].into(),
                value.text_range(),
            )
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
        for child in node.children().filter(|c| c.kind() == kind::ELEMENT) {
            let Some((name, value, range)) = include(&child) else {
                continue;
            };
            let message = || format!("`{name}` `{value}` is already included");
            seen.check(ctx, (name.clone(), value.clone()), range, message, "first included here");
        }
    }
}
