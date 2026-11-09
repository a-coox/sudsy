//! The shell of `RUN`: its command and the body of its heredoc are Shell code (the other
//! instructions are not: `CMD` and `ENTRYPOINT` have their own conventions).

use crate::generated::kind as k;
use core_syntax::{Embedded, SyntaxNode, TextRange, TextSize};

pub fn find(root: &SyntaxNode, text: &str) -> Vec<Embedded> {
    let mut out = Vec::new();
    for instruction in root.descendants().filter(|n| n.kind() == k::INSTRUCTION) {
        let mut tokens = instruction
            .children_with_tokens()
            .filter_map(|e| e.into_token());
        if !tokens
            .next()
            .is_some_and(|t| t.kind() == k::KEYWORD && t.text().eq_ignore_ascii_case("run"))
        {
            continue;
        }
        for t in tokens {
            let range = t.text_range();
            let at =
                |i: usize| range.start() + TextSize::try_from(i).expect("a file is under 4 GiB");
            let body = match t.kind() {
                // The first line of the heredoc holds what follows `<<EOT`, the last its delimiter.
                k::HEREDOC => match (t.text().find('\n'), t.text().rfind('\n')) {
                    (Some(a), Some(b)) if a < b => TextRange::new(at(a + 1), at(b)),
                    _ => continue,
                },
                k::SHELL_COMMAND if !t.text().starts_with("<<") => range,
                _ => continue,
            };
            // Docker drops the comment lines inside a continued instruction before the shell sees it.
            let mut carved = Vec::new();
            let mut from = usize::from(body.start());
            for line in text[body].split_inclusive('\n') {
                if t.kind() == k::SHELL_COMMAND
                    && from > usize::from(body.start())
                    && line.trim_start().starts_with('#')
                {
                    carved.push(TextRange::new(
                        TextSize::try_from(from).expect("a file is under 4 GiB"),
                        TextSize::try_from(from + line.len()).expect("a file is under 4 GiB"),
                    ));
                }
                from += line.len();
            }
            out.push(Embedded {
                language: "shell".into(),
                range: body,
                carved,
                format: false,
            });
        }
    }
    out
}
