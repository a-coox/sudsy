//! Code of other languages inside the markup: the content of `<script>` and `<style>` and the
//! values of `style`, `srcset` and event-handler attributes. The engine formats the code
//! (`Opts::embed`); this module lays the lines out. Code that does not parse stays as written.

use super::{P, dedent, s, trim_preserve_indentation, unescape_quotes};
use crate::format::model::{Dialect, N};
use core_format::EmbedMode;
use core_format::context::verbatim_text;
use core_format::doc::{self, Doc};

fn attr<'a>(n: &'a N, name: &str) -> Option<&'a str> {
    (n.attrs.iter())
        .find(|a| a.name.eq_ignore_ascii_case(name))
        .and_then(|a| a.value)
}

/// The language of the content of a `<script>` (Prettier's `inferScriptParser`).
fn script_language(n: &N) -> Option<&'static str> {
    if let Some(lang) = attr(n, "lang") {
        return match lang {
            "ts" => Some("ts"),
            "tsx" => Some("tsx"),
            "" | "js" | "jsx" => Some("js"),
            _ => None,
        };
    }
    match attr(n, "type").map(str::to_ascii_lowercase).as_deref() {
        None
        | Some(
            ""
            | "module"
            | "text/javascript"
            | "text/babel"
            | "text/jsx"
            | "application/javascript",
        ) => Some("js"),
        Some("application/x-typescript") => Some("ts"),
        Some("text/markdown") => Some("markdown"),
        Some("text/html") => Some("html"),
        Some(t) if t.ends_with("json") || t.ends_with("importmap") || t == "speculationrules" => {
            Some("json")
        }
        _ => None,
    }
}

fn style_language(n: &N) -> Option<&'static str> {
    match attr(n, "lang") {
        None | Some("css" | "postcss") => Some("css"),
        Some("scss") => Some("scss"),
        Some("less") => Some("less"),
        _ => None,
    }
}

/// The lines of `code` one per line, except the lines that start inside a template literal: their
/// indentation is part of the string and stays.
fn lines(code: &str) -> Doc {
    let (mut chunks, mut inside) = (Vec::<String>::new(), false);
    for line in code.split('\n') {
        match chunks.last_mut() {
            Some(last) if inside => {
                last.push('\n');
                last.push_str(line);
            }
            _ => chunks.push(line.to_string()),
        }
        let mut escaped = false;
        for c in line.chars() {
            match c {
                '\\' => escaped = !escaped,
                '`' if !escaped => inside = !inside,
                _ => escaped = false,
            }
        }
    }
    doc::join(&doc::hardline(), chunks.iter().map(|c| verbatim_text(c)))
}

/// `name="` body `"`: the body stays on the line when it fits, else it is indented on lines of
/// its own.
fn expand(name: &str, body: Doc) -> Doc {
    doc::concat(vec![
        s(format!("{name}=\"")),
        doc::group(doc::concat(vec![
            doc::indent(doc::concat(vec![doc::softline(), body])),
            doc::softline(),
        ])),
        s("\""),
    ])
}

impl P<'_> {
    /// The formatted content of a `<script>` or `<style>`, when its language is known.
    pub(super) fn embedded_content(&self, id: usize) -> Option<Doc> {
        let (n, embed) = (self.n(id), self.o.embed?);
        let p = self.n(n.parent);
        let language = match p.name.as_ref() {
            "script" => script_language(p)?,
            "style" => style_language(p)?,
            _ => return None,
        };
        let levels = self.depth(n.parent) + usize::from(!self.unindented(n.parent));
        let width = (self.o.width).saturating_sub(levels * self.o.tab);
        let code = dedent(trim_preserve_indentation(&n.value));
        let out = embed(language, EmbedMode::Program, &code, width, false)?;
        (!out.is_empty()).then(|| lines(&out))
    }

    /// The attribute `name="value"` of element `id` with its value formatted, when it has a
    /// language.
    pub(super) fn embedded_attribute(&self, id: usize, name: &str, value: &str) -> Option<Doc> {
        let lower = name.to_ascii_lowercase();
        let value = unescape_quotes(value);
        if lower == "style" && value.trim().is_empty() {
            return Some(s(format!("{name}=\"\"")));
        }
        let iframe_allow = lower == "allow" && self.n(id).name == "iframe";
        if iframe_allow
            && value
                .trim_matches(|c: char| c.is_whitespace() || c == ';')
                .is_empty()
        {
            return Some(s(format!("{name}=\"\"")));
        }
        if value.trim().is_empty() || value.contains("{{") {
            return None;
        }
        if iframe_allow {
            let parts = value
                .split(';')
                .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "));
            let parts: Vec<String> = parts.filter(|p| !p.is_empty()).collect();
            let last = parts.len().checked_sub(1)?;
            let body = parts.iter().enumerate().map(|(i, p)| {
                if i == last {
                    doc::concat(vec![s(p), doc::if_break(s(";"), doc::nil())])
                } else {
                    s(format!("{p};"))
                }
            });
            return Some(expand(name, doc::join(&doc::line(), body)));
        }
        if lower == "srcset" && matches!(self.n(id).name.as_ref(), "img" | "source") {
            return srcset(name, &value);
        }
        if self.o.dialect == Dialect::Vue
            && let Some(d) = self.vue_attribute(id, name, &lower, &value)
        {
            return Some(d);
        }
        let embed = self.o.embed?;
        let indent = (self.depth(id) + 1) * self.o.tab;
        let width = self.o.width.saturating_sub(indent + self.o.tab);
        let (mode, language) = match lower.as_str() {
            "style" => (EmbedMode::Declarations, "css"),
            l if l.len() > 2
                && l.starts_with("on")
                && l.bytes().all(|b| b.is_ascii_lowercase()) =>
            {
                (EmbedMode::Program, "js")
            }
            _ => return None,
        };
        // A handler is an expression when it parses as one, else statements.
        let first = (mode == EmbedMode::Program)
            .then(|| {
                // `f();` is the expression `f()` too.
                let one = value.trim_end().trim_end_matches(';');
                embed(language, EmbedMode::Expression, one, width, true)
            })
            .flatten();
        let (mode, out) = match first {
            Some(out) => (EmbedMode::Expression, out),
            None => (mode, embed(language, mode, &value, width, true)?),
        };
        if out.is_empty() {
            return None;
        }
        let out = out.replace('"', "&quot;");
        let rows: Vec<&str> = out.split('\n').collect();
        if mode == EmbedMode::Declarations
            && rows.iter().all(|r| r.ends_with(';') && !r.starts_with(' '))
        {
            let last = rows.len() - 1;
            let decls = rows.iter().enumerate().map(|(i, r)| {
                if i == last {
                    doc::concat(vec![
                        s(&r[..r.len() - 1]),
                        doc::if_break(s(";"), doc::nil()),
                    ])
                } else {
                    s(*r)
                }
            });
            return Some(expand(name, doc::join(&doc::line(), decls)));
        }
        Some(expand(name, lines(&out)))
    }
}

impl P<'_> {
    /// The language of the expressions of a Vue file: TypeScript when its script is.
    fn expression_language(&self) -> &'static str {
        let ts = self.t.n[0].kids.iter().any(|&c| {
            let n = self.n(c);
            n.name == "script" && matches!(attr(n, "lang"), Some("ts" | "tsx"))
        });
        if ts { "ts" } else { "js" }
    }

    /// `{{ expression }}`: the expression on the line when it fits, else on lines of its own.
    pub(super) fn interpolation(&self, id: usize) -> Doc {
        let n = self.n(id);
        let code = n.value.trim();
        let width = self
            .o
            .width
            .saturating_sub((self.depth(id) + 1) * self.o.tab);
        let out = self.o.embed.and_then(|e| {
            e(
                self.expression_language(),
                EmbedMode::Expression,
                code,
                width,
                false,
            )
        });
        let borrowed = n.next.is_some_and(|x| self.borrow_prev_close_end(x));
        doc::concat(vec![
            s(self.open_start(id)),
            doc::indent(doc::concat(vec![
                doc::line(),
                lines(out.as_deref().unwrap_or(code)),
            ])),
            if borrowed { s(" ") } else { doc::line() },
            s(self.close_end(id)),
        ])
    }

    /// A Vue directive (`:a`, `@a`, `#a`, `v-*`) whose value is an expression.
    fn vue_attribute(&self, id: usize, name: &str, lower: &str, value: &str) -> Option<Doc> {
        let embed = self.o.embed?;
        let key = lower.split([':', '.', '[']).next().unwrap_or("");
        let language = self.expression_language();
        let indent = (self.depth(id) + 1) * self.o.tab;
        if !(lower.starts_with([':', '@', '#', '.'])
            || key.starts_with("v-")
            || key == "slot-scope")
        {
            return None;
        }
        // The formatted value for a width, and whether it is statements.
        let run = |width: usize| -> Option<(String, bool)> {
            let expr = |code: &str| embed(language, EmbedMode::Expression, code, width, true);
            if key == "v-for" {
                return v_for(value, expr).map(|o| (o, false));
            }
            if lower.starts_with('@') || key == "v-on" {
                let one = value.trim_end().trim_end_matches(';');
                return expr(one).map(|o| (o, false)).or_else(|| {
                    embed(language, EmbedMode::Program, value, width, true).map(|o| (o, true))
                });
            }
            expr(value).map(|o| (o, false))
        };
        let fix = |o: String| o.replace('"', "&quot;");
        let (flat, statements) = run(self.o.width.saturating_sub(indent + name.len() + 2))?;
        // A string or template literal stays on the line however long it is.
        let quote = flat
            .chars()
            .next()
            .filter(|c| matches!(c, '\'' | '"' | '`'));
        let literal = quote.is_some_and(|q| {
            flat.len() > 1 && flat.ends_with(q) && !flat[1..flat.len() - 1].contains(q)
        });
        if literal {
            return Some(doc::concat(vec![
                s(format!("{name}=\"")),
                lines(&fix(flat)),
                s("\""),
            ]));
        }
        if !flat.contains('\n') {
            return Some(expand(name, s(fix(flat))));
        }
        let first = flat.lines().next().unwrap_or("");
        let hug = !statements
            && (flat.starts_with(['{', '['])
                || flat.starts_with("function")
                || (first.contains("=>") && first.ends_with('{')));
        if hug {
            return Some(doc::concat(vec![
                s(format!("{name}=\"")),
                lines(&fix(flat)),
                s("\""),
            ]));
        }
        let (broken, _) = run(self.o.width.saturating_sub(indent + self.o.tab))?;
        Some(expand(name, lines(&fix(broken))))
    }
}

impl P<'_> {
    /// A root block of a Vue file that is not markup: formatted by its `lang` or `type`, else kept.
    pub(super) fn custom_block_doc(&self, id: usize) -> Doc {
        let n = self.n(id);
        let raw = verbatim_text(&self.t.src[n.start..n.end]);
        let language = match (attr(n, "lang"), attr(n, "type")) {
            (Some("json"), _) | (None, Some("application/json")) => "json",
            (Some("yaml" | "yml"), _) => "yaml",
            (Some("graphql" | "gql"), _) => "graphql",
            (Some("md" | "markdown"), _) => "markdown",
            (Some("js" | "javascript"), _) => "js",
            (Some("ts" | "typescript"), _) => "ts",
            _ => return raw,
        };
        let (Some(embed), Some(close), true) = (self.o.embed, n.close_start, n.name != "template")
        else {
            return raw;
        };
        let code = dedent(trim_preserve_indentation(&self.t.src[n.tag_end..close]));
        match embed(language, EmbedMode::Program, &code, self.o.width, false) {
            Some(out) if !out.is_empty() => doc::concat(vec![
                verbatim_text(&self.t.src[n.start..n.tag_end]),
                doc::hardline(),
                lines(&out),
                doc::hardline(),
                s(format!("</{}>", n.name)),
            ]),
            _ => raw,
        }
    }
}

/// `item in items`, `(item, index) of list`: the loop variables as written (parenthesized when
/// there are several) and the collection formatted by `expr`.
fn v_for(value: &str, expr: impl Fn(&str) -> Option<String>) -> Option<String> {
    let (at, kw) = [" in ", " of "]
        .iter()
        .filter_map(|k| value.find(k).map(|i| (i, *k)))
        .min()?;
    let left = value[..at].trim();
    let inner = left
        .strip_prefix('(')
        .and_then(|l| l.strip_suffix(')'))
        .unwrap_or(left);
    let left = if inner.contains(['{', '[']) {
        left.to_string()
    } else {
        let names: Vec<&str> = inner.split(',').map(str::trim).collect();
        if left.starts_with('(') || names.len() > 1 {
            format!("({})", names.join(", "))
        } else {
            names[0].to_string()
        }
    };
    let right = value[at + kw.len()..].trim();
    Some(format!("{left}{kw}{}", expr(right)?))
}

/// `a.png 1x, b.png 2x`: the candidates one per line, descriptors aligned, when too wide.
fn srcset(name: &str, value: &str) -> Option<Doc> {
    if value.contains("data:") {
        return None;
    }
    // The candidates as the HTML spec reads them: a URL runs to a blank, so it may hold commas.
    let mut items = Vec::new();
    let mut rest = value.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    while !rest.is_empty() {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (url, after) = rest.split_at(end);
        let desc = if url.ends_with(',') {
            rest = after;
            ""
        } else {
            let (d, more) = after.split_once(',').unwrap_or((after, ""));
            rest = more;
            d.trim()
        };
        let unit = desc.chars().last();
        let number = &desc[..desc.len().saturating_sub(1)];
        if !desc.is_empty()
            && !(matches!(unit, Some('w' | 'x' | 'h'))
                && number.parse::<f64>().is_ok_and(|n| n >= 0.0))
        {
            return None;
        }
        items.push((url.trim_end_matches(','), desc));
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    }
    let left = |d: &str| d.find('.').unwrap_or(d.len().saturating_sub(1));
    let max_url = items.iter().map(|(u, _)| u.len()).max()?;
    let max_left = items.iter().map(|(_, d)| left(d)).max()?;
    let parts = items.iter().map(|&(url, desc)| {
        if desc.is_empty() {
            return s(url);
        }
        let pad = max_url - url.len() + 1 + max_left - left(desc);
        doc::concat(vec![
            s(url),
            doc::if_break(s(" ".repeat(pad)), s(" ")),
            s(desc),
        ])
    });
    let sep = doc::concat(vec![s(","), doc::line()]);
    Some(expand(name, doc::join(&sep, parts)))
}
