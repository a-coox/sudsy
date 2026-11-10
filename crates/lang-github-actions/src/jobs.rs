//! Rules on the `jobs` of a workflow: ids, `needs`, `runs-on` and `permissions`.

use crate::view::{Entry, entries, get, items, root_mapping, scalar, scalar_list};
use core_rules::rule;
use core_rules::{Check, FileContext, Opt, OptionDecl, RuleContext};
use core_syntax::{SyntaxNode, TextRange};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::sync::LazyLock;

/// The `jobs` mapping of a workflow; empty for any other file (an `action.yml`).
fn jobs(file: &FileContext<'_>) -> Vec<Entry> {
    root_mapping(file.root)
        .and_then(|root| get(&root, file.text, "jobs"))
        .and_then(|e| e.value)
        .map(|v| entries(&v, file.text))
        .unwrap_or_default()
}

rule! {
    pub InvalidJobId {
        id: "invalid-job-id",
        category: Correctness,
        default: Error,
        description: "A job id must start with a letter or `_` and contain only letters, digits, `-` and `_`.",
        example: "jobs:\n  build app:   # reported\n    runs-on: ubuntu-latest",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for job in jobs(file) {
            let mut chars = job.key.chars();
            let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            if !valid {
                ctx.report(job.key_range, format!("invalid job id `{}`", job.key));
            }
        }
    }
}

rule! {
    pub DuplicateIds {
        id: "duplicate-ids",
        category: Correctness,
        default: Error,
        description: "Job ids that differ only in case (ids are case-insensitive; exact repeats are \
                  `no-duplicate-keys`), and step ids repeated within a job.",
        example: "jobs:\n  Build: {}\n  build: {}   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let mut seen: HashMap<String, String> = HashMap::new();
        for job in jobs(file) {
            let lower = job.key.to_lowercase();
            match seen.get(&lower) {
                Some(first) if *first != job.key => {
                    let message = format!("job id `{}` repeats `{first}` (ids are case-insensitive)", job.key);
                    ctx.report(job.key_range, message);
                }
                Some(_) => {}
                None => {
                    seen.insert(lower, job.key.clone());
                }
            }
            let steps = job
                .value
                .and_then(|v| get(&v, file.text, "steps"))
                .and_then(|s| s.value);
            let mut ids: HashMap<String, ()> = HashMap::new();
            for step in steps.iter().flat_map(crate::view::items) {
                let Some((id, range)) = get(&step, file.text, "id")
                    .and_then(|e| e.value)
                    .and_then(|v| scalar(&v, file.text))
                else {
                    continue;
                };
                if ids.insert(id.to_lowercase(), ()).is_some() {
                    ctx.report(range, format!("step id `{id}` repeats within job `{}`", job.key));
                }
            }
        }
    }
}

/// `(job, needs entries)` of every job.
fn needs(file: &FileContext<'_>) -> Vec<(String, Vec<(String, TextRange)>)> {
    jobs(file)
        .into_iter()
        .map(|job| {
            let list = job
                .value
                .and_then(|v| get(&v, file.text, "needs"))
                .and_then(|e| e.value)
                .map(|v| scalar_list(&v, file.text))
                .unwrap_or_default();
            (job.key, list)
        })
        .collect()
}

rule! {
    pub UnknownNeeds {
        id: "unknown-needs",
        category: Correctness,
        default: Error,
        description: "`needs` names a job that the workflow does not define.",
        example: "jobs:\n  test:\n    needs: build   # reported\n    runs-on: ubuntu-latest",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let all = needs(file);
        for (job, list) in &all {
            for (name, range) in list {
                if !all.iter().any(|(j, _)| j.eq_ignore_ascii_case(name)) {
                    ctx.report(*range, format!("job `{job}` needs unknown job `{name}`"));
                }
            }
        }
    }
}

rule! {
    pub CyclicNeeds {
        id: "cyclic-needs",
        category: Correctness,
        default: Error,
        description: "The `needs` of the jobs form a cycle, so no job of it can start.",
        example: "jobs:\n  a:\n    needs: b   # reported\n  b:\n    needs: a   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let all = needs(file);
        let keys = jobs(file);
        let index = |name: &str| all.iter().position(|(j, _)| j.eq_ignore_ascii_case(name));
        // Whether `to` is reached from `from` through one or more `needs`.
        let reaches = |from: usize, to: usize| {
            let mut stack: Vec<usize> = all[from].1.iter().filter_map(|(n, _)| index(n)).collect();
            let mut seen = vec![false; all.len()];
            while let Some(at) = stack.pop() {
                if at == to {
                    return true;
                }
                if !std::mem::replace(&mut seen[at], true) {
                    stack.extend(all[at].1.iter().filter_map(|(n, _)| index(n)));
                }
            }
            false
        };
        // One report for each cycle, at its first job.
        let mut done = vec![false; all.len()];
        for (start, (job, _)) in all.iter().enumerate() {
            if done[start] || !reaches(start, start) {
                continue;
            }
            for (other, d) in done.iter_mut().enumerate() {
                *d |= other == start || (reaches(start, other) && reaches(other, start));
            }
            ctx.report(keys[start].key_range, format!("job `{job}` is part of a cycle of `needs`"));
        }
    }
}

static MATRIX_REF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\$\{\{\s*matrix\.([\w-]+)\s*\}\}$").expect("a valid pattern"));

/// The scalars of the job's `strategy.matrix.<name>` and of that key in its `include` entries.
fn matrix_values(job: &SyntaxNode, text: &str, name: &str) -> Vec<(String, TextRange)> {
    let Some(matrix) = get(job, text, "strategy")
        .and_then(|s| s.value)
        .and_then(|s| get(&s, text, "matrix"))
        .and_then(|m| m.value)
    else {
        return Vec::new();
    };
    let mut out: Vec<_> = get(&matrix, text, name)
        .and_then(|e| e.value)
        .map(|v| scalar_list(&v, text))
        .unwrap_or_default();
    let include = get(&matrix, text, "include").and_then(|e| e.value);
    for entry in include.iter().flat_map(items) {
        out.extend(
            get(&entry, text, name)
                .and_then(|e| e.value)
                .and_then(|v| scalar(&v, text)),
        );
    }
    out
}

/// The GitHub-hosted images and the labels every self-hosted runner has.
const HOSTED: &[&str] = &[
    r"ubuntu-slim",
    r"ubuntu-latest(-(4|8|16)-cores)?",
    r"ubuntu-(24|22)\.04(-arm)?",
    r"windows-latest(-8-cores)?",
    r"windows-2025(-vs2026)?",
    r"windows-2022",
    r"windows-11-arm",
    r"macos-latest(-(xlarge|large))?",
    r"macos-(26|15)(-(intel|xlarge|large))?",
    r"macos-14(-(xlarge|large))?",
    "self-hosted",
    "linux",
    "macos",
    "windows",
    "x64",
    "arm",
    "arm64",
];

rule! {
    pub InvalidRunsOn {
        id: "invalid-runs-on",
        category: Suspicious,
        default: Warn,
        description: "A `runs-on` label that is not one of the known runner labels. A job that also \
                  has `self-hosted` is not checked; `${{ matrix.name }}` stands for the matrix's values, other \
                  expressions are skipped.",
        example: "jobs:\n  build:\n    runs-on: ubuntu-lates   # reported",
        options: [OptionDecl {
            name: "labels",
            default: Opt::List(HOSTED),
            check: Check::Regex,
            description: "Regular expressions; a label is valid when one matches all of it, ignoring case.",
        }],
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let known: Vec<_> = ctx
            .options
            .list("labels")
            .iter()
            .filter_map(|p| RegexBuilder::new(&format!("^(?:{p})$")).case_insensitive(true).build().ok())
            .collect();
        for job in jobs(file) {
            let Some(job_node) = job.value else { continue };
            let Some(runs_on) = get(&job_node, file.text, "runs-on") else {
                continue;
            };
            let Some(runs_on) = runs_on.value else {
                ctx.report(runs_on.key_range, "`runs-on` has no label");
                continue;
            };
            // `runs-on: { group: g, labels: [a] }` names labels too.
            let labels = match get(&runs_on, file.text, "labels").and_then(|e| e.value) {
                Some(v) => scalar_list(&v, file.text),
                None => scalar_list(&runs_on, file.text),
            };
            // `${{ matrix.os }}` stands for the values the matrix gives `os`.
            let labels: Vec<_> = labels
                .into_iter()
                .flat_map(|(label, range)| match MATRIX_REF.captures(&label) {
                    Some(name) => matrix_values(&job_node, file.text, &name[1]),
                    None => vec![(label, range)],
                })
                .collect();
            if labels.iter().any(|(l, _)| l.eq_ignore_ascii_case("self-hosted")) {
                continue;
            }
            for (label, range) in labels {
                if !label.contains("${{") && !known.iter().any(|k| k.is_match(&label)) {
                    ctx.report(range, format!("unknown runner label `{label}`"));
                }
            }
        }
    }
}

rule! {
    pub MissingPermissions {
        id: "missing-permissions",
        category: Suspicious,
        default: Off,
        description: "A job without `permissions`, in a workflow without top-level `permissions`: \
                  it gets the repository's default token permissions, often write access.",
        example: "jobs:\n  build:   # reported\n    runs-on: ubuntu-latest",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(root) = root_mapping(file.root) else { return };
        if get(&root, file.text, "permissions").is_some() {
            return;
        }
        for job in jobs(file) {
            if !job.value.is_some_and(|v| get(&v, file.text, "permissions").is_some()) {
                ctx.report(job.key_range, format!("job `{}` has no `permissions`", job.key));
            }
        }
    }
}
