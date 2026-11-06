//! Rules about the build stages of a file: what each FROM starts and what refers to it.

use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta, rule};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

/// A top-level instruction: its upper-case name and its arguments as written (no comments or
/// line continuations).
struct Inst {
    name: String,
    range: TextRange,
    words: Vec<String>,
}

fn instructions(doc: &SyntaxNode, ctx: &RuleContext<'_>) -> Vec<Inst> {
    doc.children()
        .filter(|n| n.kind() == k::INSTRUCTION)
        .map(|n| {
            // Tokens with no blank between them are one word (`--platform=` and `$ARCH`).
            let mut words: Vec<String> = Vec::new();
            let mut end = None;
            for t in n.children_with_tokens().filter_map(|e| e.into_token()) {
                if matches!(
                    t.kind(),
                    k::WHITESPACE | k::NEWLINE | k::COMMENT | k::LINE_CONTINUATION
                ) {
                    end = None;
                    continue;
                }
                match (end, words.last_mut()) {
                    (Some(e), Some(w)) if e == t.text_range().start() => w.push_str(t.text()),
                    _ => words.push(t.text().to_string()),
                }
                end = Some(t.text_range().end());
            }
            let mut words = words.into_iter();
            let name = words.next().unwrap_or_default().to_ascii_uppercase();
            Inst {
                name,
                range: significant_range(&n, ctx.file.grammar),
                words: words.collect(),
            }
        })
        .collect()
}

impl Inst {
    /// `--name=value` of the instruction.
    fn option(&self, name: &str) -> Option<&str> {
        self.words
            .iter()
            .find_map(|w| w.strip_prefix("--")?.strip_prefix(name)?.strip_prefix('='))
    }

    /// The image and the stage name of a FROM.
    fn from(&self) -> (&str, Option<&str>) {
        let mut rest = self.words.iter().filter(|w| !w.starts_with("--"));
        let image = rest.next().map_or("", String::as_str);
        let alias = match (rest.next(), rest.next()) {
            (Some(a), Some(n)) if a.eq_ignore_ascii_case("as") => Some(n.as_str()),
            _ => None,
        };
        (image, alias)
    }
}

rule! {
    pub UntaggedBaseImage {
        id: "untagged-base-image",
        category: Suspicious,
        default: Warn,
        description: "A FROM image with neither a tag nor a digest, which builds on whatever `latest` is. \
                  `scratch`, a variable and an earlier stage's name are fine.",
        example: "FROM alpine   # FROM alpine:3.20",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut stages: Vec<String> = Vec::new();
        for i in instructions(doc, ctx).iter().filter(|i| i.name == "FROM") {
            let (image, alias) = i.from();
            let tagged = image.contains(':') || image.contains('@');
            if !tagged && image != "scratch" && !image.starts_with('$') && !stages.iter().any(|s| s == image) {
                ctx.report(i.range, format!("`{image}` has no tag"));
            }
            stages.extend(alias.map(str::to_string));
        }
    }
}

rule! {
    pub LastUserNotRoot {
        id: "last-user-not-root",
        category: Correctness,
        default: Warn,
        description: "A stage whose last USER is root (`root`, `0`, `root:group`, `0:group`).",
        example: "USER root   # last USER of the stage",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut root = None;
        let flush = |root: &mut Option<TextRange>, ctx: &mut RuleContext<'_>| {
            if let Some(r) = root.take() {
                ctx.report(r, "the last USER of the stage is root");
            }
        };
        let mut seen_from = false;
        for i in instructions(doc, ctx) {
            match i.name.as_str() {
                "FROM" => {
                    flush(&mut root, ctx);
                    seen_from = true;
                }
                "USER" if seen_from => {
                    let user = i.words.first().map_or("", String::as_str);
                    let name = user.split(':').next().unwrap_or("");
                    root = (name == "root" || name == "0").then_some(i.range);
                }
                _ => {}
            }
        }
        flush(&mut root, ctx);
    }
}

rule! {
    pub DuplicateStageName {
        id: "duplicate-stage-name",
        category: Correctness,
        default: Error,
        description: "Two FROM instructions name their stage alike.",
        example: "FROM a AS x\nFROM b AS x   # reported",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut seen: Vec<&str> = Vec::new();
        let insts = instructions(doc, ctx);
        for i in insts.iter().filter(|i| i.name == "FROM") {
            if let Some(name) = i.from().1 {
                if seen.contains(&name) {
                    ctx.report(i.range, format!("the stage name `{name}` is already used"));
                }
                seen.push(name);
            }
        }
    }
}

rule! {
    pub CopyFromUnknownStage {
        id: "copy-from-unknown-stage",
        category: Correctness,
        default: Warn,
        description: "A `COPY --from` that names no earlier stage: not a stage name, not the index of an \
                  earlier stage, and not an image (no `:`).",
        example: "COPY --from=build /a /b   # no stage `build` before this line",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut names: Vec<String> = Vec::new();
        let mut count = 0usize;
        for i in instructions(doc, ctx) {
            if i.name == "FROM" {
                count += 1;
                names.extend(i.from().1.map(str::to_string));
            } else if i.name == "COPY" {
                let Some(from) = i.option("from") else { continue };
                let known = from.contains(':')
                    || names.iter().any(|n| n == from)
                    || from.parse::<usize>().is_ok_and(|n| n < count);
                if !known {
                    ctx.report(i.range, format!("`--from={from}` is not an earlier stage"));
                }
            }
        }
    }
}

rule! {
    pub CopyFromOwnStage {
        id: "copy-from-own-stage",
        category: Correctness,
        default: Error,
        description: "A `COPY --from` that names the stage it is in.",
        example: "FROM a AS x\nCOPY --from=x /a /b   # reported",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut own: Option<String> = None;
        for i in instructions(doc, ctx) {
            if i.name == "FROM" {
                own = i.from().1.map(str::to_string);
            } else if i.name == "COPY" && i.option("from").is_some_and(|f| Some(f) == own.as_deref()) {
                ctx.report(i.range, "a stage cannot copy from itself");
            }
        }
    }
}

rule! {
    pub FromFirst {
        id: "from-first",
        category: Correctness,
        default: Error,
        description: "An instruction other than ARG before the first FROM.",
        example: "RUN echo hi\nFROM alpine:3   # the RUN comes first",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        for i in instructions(doc, ctx) {
            match i.name.as_str() {
                "FROM" => return,
                "ARG" => {}
                _ => {
                    ctx.report(i.range, "the file must begin with FROM or ARG");
                }
            }
        }
    }
}

pub const ONCE_PER_STAGE: Family = Family {
    name: "once-per-stage",
    build,
};

/// `instruction`: the instruction that may appear once per stage (CMD, ENTRYPOINT, HEALTHCHECK);
/// each further one is reported.
struct OncePerStage {
    meta: &'static RuleMeta,
    message: String,
    instruction: String,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let rule = OncePerStage {
        meta,
        message: p.text("message")?,
        instruction: p.text("instruction")?.to_ascii_uppercase(),
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for OncePerStage {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[k::DOCUMENT]
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut seen = false;
        for i in instructions(doc, ctx) {
            if i.name == "FROM" {
                seen = false;
            } else if i.name == self.instruction {
                if seen {
                    ctx.report(i.range, self.message.clone());
                }
                seen = true;
            }
        }
    }
}
