use crate::generated::kind;
use core_rules::{RuleContext, rule};
use core_syntax::SyntaxNode;

rule! {
    pub NoTypenamePrefix {
        id: "no-typename-prefix",
        category: Style,
        default: Warn,
        description: "A field of an object or interface type starts with the name of the type \
                  (`type User { userId: ID }`); the type already says it (case ignored). No fix: \
                  renaming a field changes the schema.",
        example: "type User {\n  userId: ID   # reported\n}",
        kinds: &[kind::FIELD_DEF],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(owner) = node.parent().and_then(|body| body.parent()) else {
            return;
        };
        if !matches!(owner.kind(), kind::OBJECT_TYPE | kind::INTERFACE_TYPE) {
            return;
        }
        let (Some(type_name), Some(field)) = (super::name_of(&owner), super::name_of(node)) else {
            return;
        };
        let type_name = type_name.text().to_lowercase();
        if field.text().to_lowercase().starts_with(&type_name) {
            ctx.report(
                field.text_range(),
                format!("field `{}` starts with the type name `{type_name}`", field.text()),
            );
        }
    }
}
