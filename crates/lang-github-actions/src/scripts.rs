//! The `run:` scripts of a file: where they are, which shell runs them (the embedded code, see
//! `embedded`), and the rules that read their text.

use crate::view::{entries, get, items, root_mapping, scalar};
use core_rules::rule;
use core_rules::{FileContext, RuleContext};
use core_syntax::embedded::size;
use core_syntax::{Embedded, SyntaxNode, SyntaxNodeExt, TextRange};
use lang_yaml::generated::kind;
use regex::Regex;
use std::sync::LazyLock;

pub struct Script {
    pub range: TextRange,
    /// Literal text that a shell can read as written: a single-line plain scalar or a `|` block.
    pub code: bool,
    pub language: Option<&'static str>,
}

static EXPRESSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\$\{\{(.*?)\}\}").expect("a valid pattern"));

/// `bash`, `sh`, `pwsh` and `powershell` (with options after the name) name a language we check.
fn language_of(shell: &str) -> Option<&'static str> {
    match shell.split_whitespace().next()? {
        "bash" | "sh" => Some("shell"),
        "pwsh" | "powershell" => Some("powershell"),
        _ => None,
    }
}

fn default_shell(map: &SyntaxNode, text: &str) -> Option<String> {
    let run = get(map, text, "defaults")?.value?;
    let shell = get(&get(&run, text, "run")?.value?, text, "shell")?.value?;
    scalar(&shell, text).map(|s| s.0)
}

/// Every `run` of the steps of the workflow's jobs and of a composite action.
pub fn scripts(root: &SyntaxNode, text: &str) -> Vec<Script> {
    let Some(file) = root_mapping(root) else {
        return Vec::new();
    };
    let file_shell = default_shell(&file, text);
    let mut out = Vec::new();
    let mut steps_of = |container: &SyntaxNode, shell: &Option<String>, windows: bool| {
        let Some(steps) = get(container, text, "steps").and_then(|e| e.value) else {
            return;
        };
        for step in items(&steps) {
            // The JavaScript of `actions/github-script` is text pasted from the same expressions.
            let uses = get(&step, text, "uses")
                .and_then(|e| e.value)
                .and_then(|v| scalar(&v, text));
            if uses.is_some_and(|u| u.0.starts_with("actions/github-script"))
                && let Some(js) = get(&step, text, "with")
                    .and_then(|e| e.value)
                    .and_then(|w| get(&w, text, "script"))
                    .and_then(|e| e.value)
            {
                out.extend(script(&js, text, None));
            }
            let Some(run) = get(&step, text, "run").and_then(|e| e.value) else {
                continue;
            };
            let own = get(&step, text, "shell")
                .and_then(|e| e.value)
                .and_then(|v| scalar(&v, text))
                .map(|s| s.0);
            let language = match own.or_else(|| shell.clone()) {
                Some(shell) => language_of(&shell),
                None if windows => Some("powershell"),
                None => Some("shell"),
            };
            out.extend(script(&run, text, language));
        }
    };
    if let Some(runs) = get(&file, text, "runs").and_then(|e| e.value) {
        steps_of(&runs, &file_shell, false);
    }
    let jobs = get(&file, text, "jobs").and_then(|e| e.value);
    for job in jobs.iter().flat_map(|j| entries(j, text)) {
        let Some(value) = job.value else { continue };
        let windows = get(&value, text, "runs-on")
            .and_then(|e| e.value)
            .is_some_and(|v| text[v.text_range()].to_lowercase().contains("windows"));
        let shell = default_shell(&value, text).or_else(|| file_shell.clone());
        steps_of(&value, &shell, windows);
    }
    out
}

fn script(value: &SyntaxNode, text: &str, language: Option<&'static str>) -> Option<Script> {
    let node = crate::view::inner(value)?;
    match node.kind() {
        kind::SCALAR => {
            let (_, range) = scalar(&node, text)?;
            let plain = node.children_with_tokens().any(|c| c.kind() == kind::PLAIN);
            Some(Script {
                range,
                code: plain && !text[range].contains('\n'),
                language,
            })
        }
        kind::BLOCK_SCALAR => {
            let token = |k| node.token(k);
            let (header, content) = (token(kind::BLOCK_HEADER)?, token(kind::BLOCK_CONTENT)?);
            Some(Script {
                range: content.text_range(),
                code: text[header.text_range()].starts_with('|'),
                language,
            })
        }
        _ => None,
    }
}

/// The regions of `run` code in a shell the checks know. The host parts inside them are the
/// block's indentation and the `${{ }}` expressions, which the shell never sees.
pub fn embedded(root: &SyntaxNode, text: &str) -> Vec<Embedded> {
    let mut out = Vec::new();
    for script in scripts(root, text).into_iter().filter(|s| s.code) {
        let Some(language) = script.language else {
            continue;
        };
        let body = &text[script.range];
        let start = usize::from(script.range.start());
        let at = |i: usize| size(start + i);
        let mut carved = Vec::new();
        // A block scalar's lines start with the indentation of its first line.
        let indent = body
            .lines()
            .find(|l| !l.trim().is_empty())
            .map_or(0, |l| l.len() - l.trim_start().len());
        if body.contains('\n') {
            let mut from = 0;
            for line in body.split_inclusive('\n') {
                let spaces = (line.len() - line.trim_start_matches(' ').len()).min(indent);
                if spaces > 0 {
                    carved.push(TextRange::new(at(from), at(from + spaces)));
                }
                from += line.len();
            }
        }
        carved.extend(
            EXPRESSION
                .find_iter(body)
                .map(|m| TextRange::new(at(m.start()), at(m.end()))),
        );
        out.push(Embedded::new(language, script.range, carved));
    }
    out
}

/// The text of every script, with the offset of its range in the file.
fn script_texts<'a>(file: &FileContext<'a>) -> impl Iterator<Item = (usize, &'a str)> {
    let text = file.text;
    scripts(file.root, text)
        .into_iter()
        .map(move |s| (usize::from(s.range.start()), &text[s.range]))
}

fn span(start: usize, len: usize) -> TextRange {
    TextRange::new(size(start), size(start + len))
}

static DEPRECATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"::(set-output|save-state|set-env|add-path)\b").expect("a valid pattern")
});

rule! {
    pub DeprecatedCommands {
        id: "deprecated-commands",
        category: Correctness,
        default: Error,
        description: "A script uses a workflow command that GitHub has disabled (`::set-output`, \
                  `::save-state`, `::set-env`, `::add-path`); write to `$GITHUB_OUTPUT`, \
                  `$GITHUB_STATE`, `$GITHUB_ENV` or `$GITHUB_PATH` instead.",
        example: "steps:\n  - run: echo \"::set-output name=v::1\"   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (start, body) in script_texts(file) {
            for m in DEPRECATED.find_iter(body) {
                ctx.report(
                    span(start + m.start(), m.len()),
                    format!("the workflow command `{}` is deprecated", m.as_str()),
                );
            }
        }
    }
}

/// Contexts that carry text an attacker controls (`*` is any one name or index).
const UNTRUSTED: &[&str] = &[
    "github.event.issue.title",
    "github.event.issue.body",
    "github.event.pull_request.title",
    "github.event.pull_request.body",
    "github.event.pull_request.head.ref",
    "github.event.pull_request.head.label",
    "github.event.pull_request.head.repo.default_branch",
    "github.event.discussion.title",
    "github.event.discussion.body",
    "github.event.comment.body",
    "github.event.review.body",
    "github.event.review_comment.body",
    "github.event.pages.*.page_name",
    "github.event.commits.*.message",
    "github.event.commits.*.author.email",
    "github.event.commits.*.author.name",
    "github.event.head_commit.message",
    "github.event.head_commit.author.email",
    "github.event.head_commit.author.name",
    "github.event.workflow_run.head_branch",
    "github.event.workflow_run.display_title",
    "github.event.workflow_run.head_commit.message",
    "github.event.workflow_run.head_commit.author.email",
    "github.event.workflow_run.head_commit.author.name",
    "github.head_ref",
];

static PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z_][\w-]*(?:\.[\w*-]+|\[\s*(?:'[^']*'|\d+|\*)\s*\])*")
        .expect("a valid pattern")
});

/// `a['b'][0]` as `a.b.*`, lowercase.
fn normalize(path: &str) -> String {
    let mut out = String::new();
    for part in path.split(['.', '[']) {
        let part = part.trim_end_matches(']').trim().trim_matches('\'');
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&if part.chars().all(|c| c.is_ascii_digit()) {
            "*".into()
        } else {
            part.to_lowercase()
        });
    }
    out
}

fn untrusted(path: &str) -> bool {
    let path = normalize(path);
    UNTRUSTED.iter().any(|pattern| {
        let (a, b): (Vec<_>, Vec<_>) = (pattern.split('.').collect(), path.split('.').collect());
        a.len() == b.len()
            && a.iter()
                .zip(&b)
                .all(|(x, y)| x == y || *x == "*" || *y == "*")
    })
}

rule! {
    pub ScriptInjection {
        id: "script-injection",
        category: Correctness,
        default: Warn,
        description: "An expression in a `run` script reads text that an attacker can choose (an \
                  issue or pull request title, a branch name, a commit message): it is pasted \
                  into the script before the shell runs, so it can inject commands. Pass it \
                  through an environment variable (`env:`) and read `\"$VAR\"` instead.",
        example: "steps:\n  - run: echo \"${{ github.event.issue.title }}\"   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (start, body) in script_texts(file) {
            for expression in EXPRESSION.captures_iter(body) {
                let inner = expression.get(1).expect("the group always takes part");
                for path in PATH.find_iter(inner.as_str()).filter(|p| untrusted(p.as_str())) {
                    ctx.report(
                        span(start + inner.start() + path.start(), path.len()),
                        format!("`{}` can be set by an attacker and is pasted into the script", path.as_str()),
                    );
                }
            }
        }
    }
}
