//! The Rust rules of Shell; rules that fit a pattern are files in `rules/`.

use crate::generated::kind as k;
use core_rules::{Family, Rule, RuleContext, rule};
use core_syntax::{SyntaxElement, SyntaxNode};

mod word;

pub static RULES: &[&dyn Rule] = &[&CdWithoutExit];

pub static FAMILIES: &[Family] = &[word::FAMILY];

/// A flag word that turns on `errexit`: `-e`, `-eu`, not `--` options.
fn is_errexit_flag(w: &str) -> bool {
    w.starts_with('-') && !w.starts_with("--") && w.contains('e')
}

/// `set -e` / `set -o errexit` on any line, or `-e` on the shebang. shortcut: not scoped to the
/// code after it, as shellcheck does not either.
fn has_errexit(text: &str) -> bool {
    text.lines().enumerate().any(|(i, l)| {
        let mut w = l.split_whitespace();
        match (i, w.next()) {
            (0, Some(first)) if first.starts_with("#!") => w.any(is_errexit_flag),
            (_, Some("set")) => w.any(|w| is_errexit_flag(w) || w == "errexit"),
            _ => false,
        }
    })
}

/// The element before `node`, trivia skipped.
fn previous(node: &SyntaxNode) -> Option<SyntaxElement> {
    let mut e = node.prev_sibling_or_token();
    while let Some(x) = e {
        if !matches!(x.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT) {
            return Some(x);
        }
        e = x.prev_sibling_or_token();
    }
    None
}

/// Whether the exit status of `node` is already tested: the condition of `if`/`while`, a
/// non-last element of `&&`/`||`, a pipeline element or a negated command.
fn checked(node: &SyntaxNode) -> bool {
    match node.parent().map(|p| p.kind()) {
        // The last element of a list is unchecked (`a && cd b`).
        Some(k::LIST) => node.next_sibling().is_some(),
        Some(k::PIPELINE | k::NEGATED | k::REDIRECTED) => true,
        Some(k::IF | k::ELIF | k::WHILE) => matches!(
            previous(node),
            Some(SyntaxElement::Token(t))
                if t.kind() == k::KEYWORD && matches!(t.text(), "if" | "elif" | "while" | "until")
        ),
        _ => false,
    }
}

rule! {
    pub CdWithoutExit {
        id: "cd-without-exit",
        category: Suspicious,
        default: Warn,
        description: "`cd`, `pushd` or `popd` whose failure is not handled (`cd dir || exit`). Not reported with `set -e`, for `cd ..` or `cd /`, or for `popd -n`.",
        example: "cd build",
        kinds: &[k::COMMAND],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut words = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::WORD);
        let Some(name) = words.next().filter(|t| matches!(t.text(), "cd" | "pushd" | "popd")) else {
            return;
        };
        let args: Vec<_> = words.collect();
        let safe = name.text() == "cd"
            && !args.is_empty()
            && args.iter().all(|a| a.text().chars().all(|c| c == '.' || c == '/'));
        let skip_n = name.text() != "cd" && args.iter().any(|a| a.text() == "-n");
        if safe || skip_n || checked(node) || has_errexit(ctx.file.text) {
            return;
        }
        ctx.report(name.text_range(), "use `cd ... || exit` in case `cd` fails");
    }
}
