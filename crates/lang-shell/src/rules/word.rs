//! The `word` rule family: a regex on the words of a command (patterns reach nodes, not tokens).
//! Params: `regex` (what a token's text matches), `tokens` (token kinds, default `Word`; `Assign`
//! and `Test` are the other kinds of a command's parts) and `in` (node kinds whose own tokens are
//! read, default `Command`; also `For`), `args` (flag: the command's name, its first word, is not read) and
//! `skip` (regex: tokens matching it are not read) and `needs` (characters: a command without one of
//! them is not read at all, a cheap test before the regex). Each matching token is reported.

use crate::generated::kind as k;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode};
use regex::Regex;

pub const FAMILY: Family = Family {
    name: "word",
    build,
};

struct Word {
    meta: &'static RuleMeta,
    message: String,
    regex: Regex,
    skip: Option<Regex>,
    args: bool,
    needs: Vec<char>,
    tokens: Vec<SyntaxKind>,
    parents: Vec<SyntaxKind>,
}

fn kinds(p: &mut Params, key: &str, default: &str) -> Result<Vec<SyntaxKind>, String> {
    let text = p.optional(key).unwrap_or_else(|| default.to_string());
    text.split_whitespace()
        .map(|n| match n {
            "Word" => Ok(k::WORD),
            "Assign" => Ok(k::ASSIGN),
            "Test" => Ok(k::TEST),
            "Command" => Ok(k::COMMAND),
            "For" => Ok(k::FOR),
            _ => Err(format!("param `{key}`: unknown kind `{n}`")),
        })
        .collect()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = Word {
        meta,
        message: p.text("message")?,
        regex: Regex::new(&p.text("regex")?).map_err(|e| format!("param `regex`: {e}"))?,
        skip: p
            .optional("skip")
            .map(|r| Regex::new(&r).map_err(|e| format!("param `skip`: {e}")))
            .transpose()?,
        args: p.flag("args")?,
        needs: p.optional("needs").unwrap_or_default().chars().collect(),
        tokens: kinds(&mut p, "tokens", "Word")?,
        parents: kinds(&mut p, "in", "Command")?,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Word {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &self.parents
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if !self.needs.is_empty() {
            let r = node.text_range();
            let text = &ctx.file.text[usize::from(r.start())..usize::from(r.end())];
            if !self.needs.iter().any(|&c| text.contains(c)) {
                return;
            }
        }
        let mut name = self.args;
        for t in node.children_with_tokens().filter_map(|e| e.into_token()) {
            if name && t.kind() == k::WORD {
                name = false;
                continue;
            }
            if self.tokens.contains(&t.kind())
                && self.regex.is_match(t.text())
                && !self.skip.as_ref().is_some_and(|s| s.is_match(t.text()))
            {
                ctx.report(t.text_range(), self.message.clone());
            }
        }
    }
}
