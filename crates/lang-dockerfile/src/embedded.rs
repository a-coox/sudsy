//! The shell of `RUN`, and of `CMD` and `ENTRYPOINT` in shell form (also under `ONBUILD` and
//! `HEALTHCHECK`): the command and the body of the heredoc are Shell code.

use crate::generated::kind as k;
use core_syntax::embedded::size;
use core_syntax::{Embedded, SyntaxNode, SyntaxNodeExt, TextRange};

pub fn find(root: &SyntaxNode, text: &str) -> Vec<Embedded> {
    let mut out = Vec::new();
    let backtick = escape_is_backtick(text);
    for instruction in root.descendants().filter(|n| n.kind() == k::INSTRUCTION) {
        let mut tokens = instruction.tokens();
        let Some(keyword) = tokens.next().filter(|t| t.kind() == k::KEYWORD) else {
            continue;
        };
        let run = keyword.text().eq_ignore_ascii_case("run");
        if !run
            && !["cmd", "entrypoint"]
                .iter()
                .any(|w| keyword.text().eq_ignore_ascii_case(w))
        {
            continue;
        }
        for t in tokens {
            let range = t.text_range();
            let at = |i: usize| range.start() + size(i);
            let body = match t.kind() {
                // The first line of the heredoc holds what follows `<<EOT`, the last its delimiter.
                k::HEREDOC => match (t.text().find('\n'), t.text().rfind('\n')) {
                    (Some(a), Some(b)) if a < b => TextRange::new(at(a + 1), at(b)),
                    _ => continue,
                },
                // The exec form of `CMD` and `ENTRYPOINT` is a JSON array, not shell.
                k::SHELL_COMMAND
                    if !t.text().starts_with("<<") && (run || !t.text().starts_with('[')) =>
                {
                    range
                }
                _ => continue,
            };
            // Docker drops the comment lines inside a continued instruction before the shell sees it.
            let mut carved = Vec::new();
            let mut from = usize::from(body.start());
            for line in text[body].split_inclusive('\n') {
                // A backtick continuation is not shell: blank it with its line break.
                let code = line.trim_end();
                if backtick && t.kind() == k::SHELL_COMMAND && code.ends_with('`') {
                    let at = |i: usize| size(from + i);
                    carved.push(TextRange::new(at(code.len() - 1), at(line.len())));
                }
                if t.kind() == k::SHELL_COMMAND
                    && from > usize::from(body.start())
                    && line.trim_start().starts_with('#')
                {
                    carved.push(TextRange::new(size(from), size(from + line.len())));
                }
                from += line.len();
            }
            out.push(Embedded::new("shell", body, carved));
        }
    }
    out
}

/// Whether the leading `# escape=` directive (parser directives come before anything else) is a backtick.
fn escape_is_backtick(text: &str) -> bool {
    text.lines()
        .map(|l| {
            l.trim()
                .strip_prefix('#')
                .map(|d| d.trim().replace(' ', ""))
        })
        .take_while(|d| d.as_ref().is_some_and(|d| d.contains('=')))
        .any(|d| d.is_some_and(|d| d.eq_ignore_ascii_case("escape=`")))
}
