//! What the shared key rules ask of GraphQL: the name of an entry is its name without the `$` of
//! a variable, and a field with an alias is known by the alias (its response name).

use crate::generated::kind;
use core_rules::RuleHooks;
use core_syntax::SyntaxElement;
use std::borrow::Cow;

pub struct GraphqlHooks;

impl RuleHooks for GraphqlHooks {
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let alias = key
            .as_token()
            .and_then(|t| t.parent())
            .filter(|p| p.kind() == kind::FIELD)
            .and_then(|field| field.children().find(|n| n.kind() == kind::ALIAS))
            .and_then(|alias| alias.first_token());
        let range = alias.map_or_else(|| key.text_range(), |t| t.text_range());
        let name = text[range].trim();
        Cow::Borrowed(name.strip_prefix('$').unwrap_or(name))
    }
}
