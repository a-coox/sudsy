//! The Rust rules of gitignore-syntax files (they read the patterns of a file in order); rules
//! that fit a pattern are files in `rules/`.

use crate::generated::kind;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange};

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicatePattern,
    &RedundantPattern,
    &NegationOfExcludedParent,
];

/// A pattern line: its text (the parts joined, trailing spaces dropped) and where it is.
struct Pat {
    text: String,
    range: TextRange,
}

impl Pat {
    fn negated(&self) -> bool {
        self.text.starts_with('!')
    }

    /// The path of the pattern without `!`, a leading `/` and a trailing `/`.
    fn path(&self) -> &str {
        let t = self.text.strip_prefix('!').unwrap_or(&self.text);
        let t = t.strip_prefix('/').unwrap_or(t);
        t.strip_suffix('/').unwrap_or(t)
    }
}

fn patterns(doc: &SyntaxNode) -> Vec<Pat> {
    doc.children()
        .filter(|e| e.kind() == kind::ENTRY)
        .filter_map(|e| {
            let toks: Vec<_> = e.tokens().filter(|t| t.kind() == kind::PATTERN).collect();
            let range = TextRange::new(
                toks.first()?.text_range().start(),
                toks.last()?.text_range().end(),
            );
            Some(Pat {
                text: toks.iter().map(|t| t.text()).collect(),
                range,
            })
        })
        .collect()
}

rule! {
    pub NoDuplicatePattern {
        id: "no-duplicate-pattern",
        category: Suspicious,
        default: Off,
        description: "The same pattern appears twice with no opposite pattern (`!x` for `x`) in \
                  between, so the second one changes nothing.",
        example: "*.log\nbuild/\n*.log   # reported: already listed above",
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let pats = patterns(node);
        // 1-based index of each text's last line, and of the last positive (`[0]`) and negated (`[1]`) line.
        let mut seen: std::collections::HashMap<&str, usize> = Default::default();
        let mut last = [0usize; 2];
        for (i, p) in pats.iter().enumerate() {
            let me = p.negated() as usize;
            if let Some(&j) = seen.get(p.text.as_str())
                && last[1 - me] <= j
            {
                ctx.report(p.range, format!("`{}` is already listed", p.text))
                    .related(pats[j - 1].range, "first listed here");
            }
            seen.insert(&p.text, i + 1);
            last[me] = i + 1;
        }
    }
}

rule! {
    pub RedundantPattern {
        id: "redundant-pattern",
        category: Suspicious,
        default: Off,
        description: "A name pattern such as `foo` already matches files and directories called \
                  `foo` at every level, so a later `foo/` or `/foo` adds nothing (no `!` pattern \
                  in between).",
        example: "build\n/build/   # reported: `build` covers it",
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let pats = patterns(node);
        for (i, p) in pats.iter().enumerate().filter(|(_, p)| !p.negated()) {
            // `foo/` and `/foo` only; `**/foo` differs in .dockerignore, where a bare name is the root's.
            let anchored = p.text.starts_with('/') || p.text.ends_with('/');
            if !anchored || p.path().is_empty() || p.path().contains('/') {
                continue;
            }
            let covered = pats[..i]
                .iter()
                .rposition(|q| q.text == p.path())
                .filter(|&j| !pats[j..i].iter().any(Pat::negated));
            if let Some(j) = covered {
                ctx.report(p.range, format!("`{}` is already covered by `{}`", p.text, pats[j].text))
                    .related(pats[j].range, "covered by this");
            }
        }
    }
}

rule! {
    pub NegationOfExcludedParent {
        id: "negation-of-excluded-parent",
        category: Correctness,
        default: Warn,
        description: "Git cannot re-include a file when a parent directory is excluded: `!a/b` \
                  after `a/` has no effect. Exclude the contents (`a/*`) instead. Not reported \
                  in `.dockerignore`, where the later pattern wins. Only literal paths are checked.",
        example: "node_modules/\n!node_modules/keep.js   # reported: node_modules/ is excluded",
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if ctx.file.path.extension().is_some_and(|e| e == "dockerignore")
            || ctx.file.path.file_name().is_some_and(|n| n == ".dockerignore")
        {
            return;
        }
        let pats = patterns(node);
        for (i, n) in pats.iter().enumerate().filter(|(_, p)| p.negated()) {
            let comps: Vec<&str> = n.path().split('/').collect();
            let parents = &comps[..comps.len() - 1];
            // An earlier literal pattern for a directory on the way to the file.
            let excluder = pats[..i].iter().rposition(|q| {
                let q_anchored = q.text.starts_with('/') || q.path().contains('/');
                let qc: Vec<&str> = q.path().split('/').collect();
                !q.negated()
                    && !q.path().contains(['*', '?', '[', '\\'])
                    && (0..parents.len()).any(|k| {
                        if q_anchored {
                            parents[..=k] == qc[..]
                        } else {
                            parents[k] == q.path()
                        }
                    })
            });
            let Some(j) = excluder else { continue };
            // A `!` pattern in between for the directory re-includes it.
            let q = pats[j].path();
            if !pats[j..i].iter().any(|r| r.negated() && r.path() == q) {
                ctx.report(n.range, format!("`{}` cannot re-include: `{}` excludes a parent directory", n.text, pats[j].text))
                    .related(pats[j].range, "parent excluded here");
            }
        }
    }
}
