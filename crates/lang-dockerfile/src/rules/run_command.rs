//! The `run-command` rule family: a check on the commands of a `RUN` (see `shell.rs`). A rule
//! reports the instruction when one of its commands is selected by `program` and `args` and breaks
//! the check; with no check param, the selected command itself is reported. Params:
//! - `program`: regex the whole command name matches; `args`: words of which one must be among the
//!   command's arguments (the subcommand, `install`). Both select the commands.
//! - `needs`: flags, one of which the command must have (`-y --yes -q*2`: `q` twice).
//! - `pinned` / `unpinned`: every package (the arguments after the subcommand that are not flags)
//!   must match `pinned` and must not match `unpinned`; `skip-flag-args`: flags whose value is
//!   not a package.
//! - `unless`: words (flags with a dash) that exempt the command: `-r .`.
//! - `cleaned-by`: alternatives `a b c | d e`, each a command name with arguments it must have;
//!   the instruction needs one of them after the selected command.
//! - `cache-dir`: a `--mount` of type cache or tmpfs on that directory exempts the instruction.

use crate::generated::kind as k;
use crate::shell::{Cmd, commands};
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::SyntaxNode;
use regex::Regex;

pub const FAMILY: Family = Family {
    name: "run-command",
    build,
};

struct RunCommand {
    meta: &'static RuleMeta,
    message: String,
    program: Regex,
    args: Vec<String>,
    needs: Vec<(String, String, usize)>,
    pinned: Option<Regex>,
    unpinned: Option<Regex>,
    skip_flag_args: Vec<String>,
    unless: Vec<String>,
    cleaned_by: Vec<Vec<String>>,
    cache_dir: Option<String>,
}

fn words(p: &mut Params, key: &str) -> Vec<String> {
    p.optional(key)
        .map(|v| v.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

fn regex(p: &mut Params, key: &str) -> Result<Option<Regex>, String> {
    p.optional(key)
        .map(|v| Regex::new(&v).map_err(|e| format!("param `{key}`: {e}")))
        .transpose()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let program = p.text("program")?;
    let rule = RunCommand {
        meta,
        message: p.text("message")?,
        program: Regex::new(&format!("^(?:{program})$"))
            .map_err(|e| format!("param `program`: {e}"))?,
        args: words(&mut p, "args"),
        needs: words(&mut p, "needs")
            .iter()
            .map(|f| {
                let (name, times) = f.split_once('*').unwrap_or((f, "1"));
                (
                    f.clone(),
                    name.trim_start_matches('-').to_string(),
                    times.parse().unwrap_or(1),
                )
            })
            .collect(),
        pinned: regex(&mut p, "pinned")?,
        unpinned: regex(&mut p, "unpinned")?,
        skip_flag_args: words(&mut p, "skip-flag-args"),
        unless: words(&mut p, "unless"),
        cleaned_by: p
            .optional("cleaned-by")
            .map(|v| {
                v.split('|')
                    .map(|a| a.split_whitespace().map(str::to_string).collect())
                    .collect()
            })
            .unwrap_or_default(),
        cache_dir: p.optional("cache-dir"),
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl RunCommand {
    fn selected(&self, c: &Cmd) -> bool {
        self.program.is_match(&c.name)
            && (self.args.is_empty() || c.args.iter().any(|a| self.args.contains(a)))
    }

    fn exempt(&self, c: &Cmd) -> bool {
        self.unless.iter().any(|u| match u.strip_prefix('-') {
            Some(flag) => c.has_flag(flag.trim_start_matches('-'), 1),
            None => c.args.contains(u),
        })
    }

    fn broken(&self, c: &Cmd) -> bool {
        if self.exempt(c) {
            return false;
        }
        if !self.needs.is_empty() {
            // A spec also matches the literal argument (`-nv`, `-q=2`).
            return !self
                .needs
                .iter()
                .any(|(raw, f, n)| c.has_flag(f, *n) || c.args.contains(raw));
        }
        if self.pinned.is_some() || self.unpinned.is_some() {
            let positionals = c.positionals(&self.skip_flag_args);
            let after = positionals
                .iter()
                .position(|p| self.args.iter().any(|a| a == p));
            return positionals[after.map_or(0, |i| i + 1)..].iter().any(|p| {
                !self.pinned.as_ref().is_none_or(|r| r.is_match(p))
                    || self.unpinned.as_ref().is_some_and(|r| r.is_match(p))
            });
        }
        self.cleaned_by.is_empty()
    }

    fn cleaned(&self, cmds: &[Cmd], from: usize) -> bool {
        self.cleaned_by.iter().any(|alt| {
            cmds[from..].iter().any(|c| {
                alt.first().is_some_and(|n| *n == c.name)
                    && alt[1..].iter().all(|a| c.args.contains(a))
            })
        })
    }
}

/// The commands of the instruction's own `RUN` (not a nested `ONBUILD`'s), and whether a `--mount`
/// of type cache or tmpfs targets `dir`.
fn run_commands(node: &SyntaxNode, dir: Option<&str>) -> Option<(Vec<Cmd>, bool)> {
    let mut run = false;
    let mut cmds = Vec::new();
    let mut json = Vec::new();
    let mut mounted = false;
    for t in node.children_with_tokens().filter_map(|e| e.into_token()) {
        match t.kind() {
            k::KEYWORD if !run => run = t.text().eq_ignore_ascii_case("run"),
            k::SHELL_COMMAND => cmds = commands(t.text()),
            // The script of `RUN <<EOT`: between the opening and the delimiter line.
            k::HEREDOC => {
                let body = t.text().split_once('\n').map_or("", |(_, rest)| rest);
                cmds.extend(commands(body.rsplit_once('\n').map_or("", |(b, _)| b)));
            }
            k::JSON_STRING => json.push(t.text().trim_matches('"').to_string()),
            k::WORD => {
                let w = t.text();
                mounted |= dir.is_some_and(|d| {
                    w.starts_with("--mount=")
                        && (w.contains("cache") || w.contains("tmpfs"))
                        && w.contains(d)
                });
            }
            _ => {}
        }
    }
    if !json.is_empty() {
        cmds = vec![Cmd {
            name: json.remove(0),
            args: json,
        }];
    }
    run.then_some((cmds, mounted))
}

impl Rule for RunCommand {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[core_syntax::SyntaxKind] {
        &[k::INSTRUCTION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some((cmds, mounted)) = run_commands(node, self.cache_dir.as_deref()) else {
            return;
        };
        let hit = cmds.iter().enumerate().any(|(i, c)| {
            self.selected(c)
                && (self.broken(c) || (!self.cleaned_by.is_empty() && !self.cleaned(&cmds, i + 1)))
        });
        if hit && !mounted {
            ctx.report(
                significant_range(node, ctx.file.grammar),
                self.message.clone(),
            );
        }
    }
}
