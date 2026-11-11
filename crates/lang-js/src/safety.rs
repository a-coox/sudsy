//! The meaning check of the output (debug builds and tests). The generic check has already
//! re-parsed the output with oxc and compared nodes and tokens, leaving out trivia, `;` and the
//! text of literals (tagged `#restyle`); here the literals of both trees are compared by value:
//! quotes and quote escapes of strings, the spelling of numbers, the order of regular expression
//! flags may differ, nothing else.

use crate::format::same_value;
use crate::generated::kind as k;
use core_syntax::SyntaxNode;

pub(crate) fn check(before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
    let literals = |tree: &SyntaxNode| {
        tree.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| matches!(t.kind(), k::STRING | k::NUMBER | k::BIG_INT | k::REG_EXP))
            .collect::<Vec<_>>()
    };
    let (a, b) = (literals(before), literals(after));
    if a.len() != b.len() {
        return Err("the output has a different number of literals".into());
    }
    for (x, y) in a.iter().zip(&b) {
        if x.kind() != y.kind() || !same_value(x.kind(), x.text(), y.text()) {
            return Err(format!("the literal `{}` became `{}`", x.text(), y.text()));
        }
    }
    Ok(())
}
