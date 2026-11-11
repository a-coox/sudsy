//! The Rust rules of Shell; rules that fit a pattern are files in `rules/`.

use crate::generated::kind as k;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxNodeExt};

pub static RULES: &[&dyn Rule] = &[&CdWithoutExit, &UselessEcho];

/// A flag word that turns on `errexit`: `-e`, `-eu`, not `--` options.
fn is_errexit_flag(w: &str) -> bool {
    w.starts_with('-') && !w.starts_with("--") && w.contains('e')
}

/// `set -e` / `set -o errexit` on any line, or `-e` on the shebang. shortcut: not scoped to the
/// code after it, as shellcheck does not either.
fn has_errexit(text: &str) -> bool {
    let line_at = |i: usize| text[i..].lines().next().unwrap_or("").split_whitespace();
    // A substring search for `set` finds the few candidate lines without splitting the whole file.
    (text.starts_with("#!") && line_at(0).any(is_errexit_flag))
        || text.match_indices("set").any(|(i, _)| {
            let mut w = line_at(text[..i].rfind('\n').map_or(0, |n| n + 1));
            w.next() == Some("set") && w.any(|w| is_errexit_flag(w) || w == "errexit")
        })
}

fn is_trivia(kind: core_syntax::SyntaxKind) -> bool {
    matches!(kind, k::WHITESPACE | k::NEWLINE | k::COMMENT)
}

/// The element before `node`, trivia skipped.
fn previous(node: &SyntaxNode) -> Option<SyntaxElement> {
    let mut e = node.prev_sibling_or_token();
    while let Some(x) = e {
        if !is_trivia(x.kind()) {
            return Some(x);
        }
        e = x.prev_sibling_or_token();
    }
    None
}

/// Whether the exit status of `node` is already tested: the condition of `if`/`while`, a
/// non-last element of `&&`/`||`. A pipeline, negation or redirection takes its parent's answer.
fn checked(node: &SyntaxNode) -> bool {
    match node.parent().map(|p| p.kind()) {
        // The last element of a list is unchecked (`a && cd b`).
        Some(k::LIST) => node.next_sibling().is_some(),
        Some(k::PIPELINE | k::NEGATED | k::REDIRECTED) => checked(&node.parent().unwrap()),
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
            .tokens()
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

/// Is `arg` a substitution, or a string that holds nothing else?
fn only_substitution(arg: &SyntaxElement) -> bool {
    match arg {
        SyntaxElement::Node(n) if n.kind() == k::SUBST => {
            !n.text().to_string().starts_with(['<', '>'])
        }
        SyntaxElement::Node(n) if n.kind() == k::STRING => {
            n.children().count() == 1
                && (n.children_with_tokens())
                    .filter_map(|e| e.into_token())
                    .all(|t| is_trivia(t.kind()) || t.text() == "\"")
                && n.first_child().is_some_and(|c| c.kind() == k::SUBST)
        }
        _ => false,
    }
}

rule! {
    pub UselessEcho {
        id: "useless-echo",
        category: Style,
        default: Off,
        description: "An `echo` whose only argument is a command substitution.",
        example: "echo $(date)",
        kinds: &[k::COMMAND],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut parts = node
            .children_with_tokens()
            .filter(|e| !is_trivia(e.kind()));
        if let (Some(SyntaxElement::Token(name)), Some(arg), None) =
            (parts.next(), parts.next(), parts.next())
            && name.text() == "echo"
            && only_substitution(&arg)
        {
            ctx.report(node.text_range(), "run the command itself instead of echoing its output");
        }
    }
}
