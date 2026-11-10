//! The `run-command` rule family: a check on the commands of a `RUN`, read from its Shell tree
//! (see `embedded.rs`). A rule
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
use core_rules::tree::significant_range;
use core_rules::{EmbeddedTree, Family, Params, Re, Rule, RuleContext, RuleMeta};
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;
use core_syntax::grammars::SHELL;
use std::cell::RefCell;
use std::rc::Rc;

pub const FAMILY: Family = Family {
    name: "run-command",
    build,
};

/// A simple command of a script: its name and arguments, unquoted.
struct Cmd {
    name: String,
    args: Vec<String>,
}

impl Cmd {
    /// The names of the flags: `--long[=v]` is `long`, `-abc` is `a`, `b`, `c` (`--` and `-` are
    /// arguments).
    fn flags(&self) -> Vec<String> {
        self.args.iter().flat_map(|a| flag_names(a)).collect()
    }

    fn has_flag(&self, name: &str, times: usize) -> bool {
        self.flags().iter().filter(|f| *f == name).count() >= times
    }

    /// The arguments that are not flags, without the value of a flag in `skip` (the next word,
    /// unless the flag has `=value`).
    fn positionals(&self, skip: &[String]) -> Vec<&str> {
        let mut out = Vec::new();
        let mut args = self.args.iter();
        while let Some(a) = args.next() {
            let flags = flag_names(a);
            if flags.is_empty() {
                out.push(a.as_str());
            } else if !a.contains('=')
                && (skip.iter().any(|s| s == "*") || flags.iter().any(|f| skip.contains(f)))
            {
                args.next();
            }
        }
        out
    }
}

fn flag_names(arg: &str) -> Vec<String> {
    if arg == "--" || arg == "-" {
        Vec::new()
    } else if let Some(long) = arg.strip_prefix("--") {
        vec![long.split('=').next().unwrap_or("").to_string()]
    } else if let Some(short) = arg.strip_prefix('-') {
        short.chars().map(String::from).collect()
    } else {
        Vec::new()
    }
}

/// A word without its quotes and escapes.
fn unquote(word: &str) -> String {
    let (mut out, mut quote, mut chars) = (String::new(), None, word.chars());
    while let Some(c) = chars.next() {
        match (c, quote) {
            ('\\', q) if q != Some('\'') => out.extend(chars.next()),
            ('\'' | '"', None) => quote = Some(c),
            (c, Some(q)) if c == q => quote = None,
            _ => out.push(c),
        }
    }
    out
}

/// The commands of a shell tree, in order: the words of each `Command` node (a word is a token,
/// a string, a concatenation or a substitution; assignments and redirections are not words). A
/// command inside `$(...)` is a `Command` node of its own, found like the others.
fn shell_commands(root: &SyntaxNode) -> Vec<Cmd> {
    let shell = SHELL.get();
    let kind = |n| shell.kind_by_name(n);
    let command = kind("Command");
    let words = ["Word", "String", "Concat", "Subst"].map(kind);
    let mut out = Vec::new();
    for node in root.descendants().filter(|n| Some(n.kind()) == command) {
        let mut words = (node.children_with_tokens())
            .filter(|e| words.contains(&Some(e.kind())))
            .map(|e| unquote(&e.to_string()));
        if let Some(name) = words.next() {
            out.push(Cmd {
                name,
                args: words.collect(),
            });
        }
    }
    out
}

struct RunCommand {
    meta: &'static RuleMeta,
    message: String,
    program: Re,
    args: Vec<String>,
    needs: Vec<(String, String, usize)>,
    pinned: Option<Re>,
    unpinned: Option<Re>,
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

fn regex(p: &mut Params, key: &str) -> Result<Option<Re>, String> {
    p.optional(key)
        .map(|v| Re::lazy(&v).map_err(|e| format!("param `{key}`: {e}")))
        .transpose()
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let program = p.text("program")?;
    let rule = RunCommand {
        meta,
        message: p.text("message")?,
        program: Re::lazy(&format!("^(?:{program})$"))
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

thread_local! {
    /// The commands of the last instruction read: every rule asks about the same one in turn.
    static LAST: RefCell<Option<(SyntaxNode, Rc<Vec<Cmd>>)>> = const { RefCell::new(None) };
}

/// The commands of the instruction's own `RUN` (not a nested `ONBUILD`'s).
fn run_commands(node: &SyntaxNode, embedded: &[EmbeddedTree]) -> Option<Rc<Vec<Cmd>>> {
    let mut tokens = node.tokens();
    let keyword = tokens.find(|t| t.kind() == k::KEYWORD)?;
    if !keyword.text().eq_ignore_ascii_case("run") {
        return None;
    }
    LAST.with_borrow_mut(|last| {
        if let Some((n, cmds)) = last
            && n == node
        {
            return Some(cmds.clone());
        }
        let cmds = Rc::new(read_commands(node, embedded));
        *last = Some((node.clone(), cmds.clone()));
        Some(cmds)
    })
}

fn read_commands(node: &SyntaxNode, embedded: &[EmbeddedTree]) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    let mut json = Vec::new();
    for t in node.tokens() {
        match t.kind() {
            // The script of `RUN cmd` or `RUN <<EOT`, parsed as shell.
            k::SHELL_COMMAND | k::HEREDOC => {
                for e in embedded
                    .iter()
                    .filter(|e| e.language == "shell" && t.text_range().contains_range(e.range))
                {
                    cmds.extend(shell_commands(&e.root));
                }
            }
            k::JSON_STRING => json.push(t.text().trim_matches('"').to_string()),
            _ => {}
        }
    }
    if !json.is_empty() {
        cmds = vec![Cmd {
            name: json.remove(0),
            args: json,
        }];
    }
    cmds
}

/// Whether a `--mount` of type cache or tmpfs targets `dir`.
fn mounted(node: &SyntaxNode, dir: &str) -> bool {
    node.tokens().filter(|t| t.kind() == k::WORD).any(|t| {
        let w = t.text();
        w.starts_with("--mount=") && (w.contains("cache") || w.contains("tmpfs")) && w.contains(dir)
    })
}

impl Rule for RunCommand {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[core_syntax::SyntaxKind] {
        &[k::INSTRUCTION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(cmds) = run_commands(node, ctx.file.embedded) else {
            return;
        };
        let hit = cmds.iter().enumerate().any(|(i, c)| {
            self.selected(c)
                && (self.broken(c) || (!self.cleaned_by.is_empty() && !self.cleaned(&cmds, i + 1)))
        });
        if hit && !self.cache_dir.as_deref().is_some_and(|d| mounted(node, d)) {
            ctx.report(
                significant_range(node, ctx.file.grammar),
                self.message.clone(),
            );
        }
    }
}
