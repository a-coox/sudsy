use super::kind;
use core_rules::FileContext;
use core_rules::rule;
use core_rules::tree::structured_trivia;
use core_rules::{Edit, RuleContext};
use core_syntax::TextRange;

rule! {
    pub DocCommentSpace {
        id: "doc-comment-space",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A documentation line (`///`) starts with a single space. More spaces are \
                  allowed to indent continuation lines, not on the first line of the comment; a \
                  tab or no space is always reported.",
        example: "///<summary>x</summary>   // write: /// <summary>x</summary>",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if !file.text.contains("///") {
            return;
        }
        for token in file
            .root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| {
                t.kind() == kind::DOCUMENTATION_COMMENT_EXTERIOR_TRIVIA
                    && t.text().trim_start() == "///"
            })
        {
            let at = token.text_range().end();
            let rest = &file.text[usize::from(at)..];
            let Some(first) = rest.chars().next() else {
                continue;
            };
            if first == '\n' || first == '\r' {
                continue;
            }
            let (len, replacement) = if first == ' ' {
                let spaces = rest.len() - rest.trim_start_matches(' ').len();
                let first_line = structured_trivia(&token, file.grammar)
                    .is_some_and(|n| n.text_range().start() == token.text_range().start());
                let blank = rest[spaces..].starts_with(['\n', '\r']) || spaces == rest.len();
                if spaces == 1 || !first_line || blank {
                    continue;
                }
                (spaces, " ")
            } else if first == '\t' {
                (1, " ")
            } else {
                (0, " ")
            };
            let range = TextRange::at(at, (len.max(1) as u32).into());
            let edit = Edit::replace(TextRange::at(at, (len as u32).into()), replacement);
            ctx.report_fix(
                range,
                "a documentation line should start with a single space",
                ("Use a single space", vec![edit]),
            );
        }
    }
}
