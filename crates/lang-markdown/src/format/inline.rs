//! Inline content: paragraphs, emphasis, escapes.

use super::*;

impl Render<'_> {
    /// Prose lines: blanks collapsed, indentation dropped, a hard break (two trailing blanks) kept.
    pub(super) fn inline_lines(&self, n: &SyntaxNode) -> Vec<String> {
        let mut s = String::new();
        self.inline(n, false, &mut s);
        let mut lines: Vec<String> = s.split('\n').map(str::to_string).collect();
        let count = lines.len();
        let sentinel = |l: &str| l.contains(NEWLINE_IN_TOKEN);
        for (i, l) in lines.iter_mut().enumerate() {
            let hard = i + 1 < count && l.ends_with("  ") && !l.trim().is_empty();
            // Blanks inside a token that spans lines (a code span) are content: only the ends trim.
            *l = if sentinel(l) {
                l.trim_start().to_string()
            } else {
                l.trim().to_string()
            };
            if hard {
                l.push_str("  ");
            }
        }
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        while lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        lines
            .iter()
            .flat_map(|l| l.split(NEWLINE_IN_TOKEN))
            .map(str::to_string)
            .collect()
    }

    pub(super) fn inline(&self, n: &SyntaxNode, in_emph: bool, out: &mut String) {
        let kids: Vec<SyntaxElement> = n.children_with_tokens().collect();
        self.run(&kids, in_emph, out);
    }

    /// Inline elements as one run of text.
    pub(super) fn run(&self, kids: &[SyntaxElement], in_emph: bool, out: &mut String) {
        for (i, e) in kids.iter().enumerate() {
            match e {
                SyntaxElement::Token(t) => {
                    let kind = t.kind();
                    if kind == k::WHITESPACE {
                        // Next to a backtick the parser may have missed a code span: keep the blanks.
                        let tick = |x: Option<SyntaxToken>| {
                            x.is_some_and(|x| x.kind() == k::TEXT && x.text().contains('`'))
                        };
                        if tick(t.prev_token()) || tick(t.next_token()) {
                            out.push_str(t.text());
                            continue;
                        }
                        let hard = t.text().len() >= 2
                            && matches!(kids.get(i + 1), Some(SyntaxElement::Token(x)) if x.kind() == k::NEWLINE);
                        out.push_str(if hard { "  " } else { " " });
                    } else if kind == k::NEWLINE {
                        out.push('\n');
                    } else if kind == k::TEXT {
                        out.push_str(&self.escape(t));
                    } else if !is_trivia(kind) {
                        // A line break inside a token (code span) is kept, minus the container prefix.
                        let chain = t.parent().map(|p| self.chain(&p)).unwrap_or_default();
                        let text = raw_lines_all(t.text(), &chain);
                        out.push_str(&text.join(&NEWLINE_IN_TOKEN.to_string()));
                    }
                }
                SyntaxElement::Node(c) if c.kind() == k::EMPHASIS || c.kind() == k::STRONG => {
                    self.emphasis(c, in_emph, out)
                }
                SyntaxElement::Node(c) => self.inline(c, in_emph, out),
            }
        }
    }

    pub(super) fn emphasis(&self, n: &SyntaxNode, in_emph: bool, out: &mut String) {
        let strong = n.kind() == k::STRONG;
        let delims: Vec<SyntaxToken> = n
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::EMPHASIS_DELIM)
            .collect();
        let w = if strong { 2 } else { 1 };
        let (Some(open), Some(close)) =
            (delims.get(w - 1), delims.get(delims.len().wrapping_sub(w)))
        else {
            return self.inline(n, true, out);
        };
        let (a, b) = (start(&delims[0]), end(&delims[delims.len() - 1]));
        let inner_src = &self.src[end(open)..start(close).max(end(open))];
        let before = self.src[..a].chars().next_back();
        let after = self.src[b..].chars().next();
        let plain = |c: Option<char>| !word_char(c) && !matches!(c, Some('*' | '_'));
        let edge = |c: Option<char>| !matches!(c, Some('*' | '_'));
        // Labels of reference links are matched as written.
        let in_link = n
            .ancestors()
            .any(|a| a.kind() == k::LINK || a.kind() == k::IMAGE);
        let safe = !in_link
            && plain(before)
            && plain(after)
            && inner_src.chars().next().is_some_and(|c| !c.is_whitespace())
            && inner_src
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_whitespace())
            && edge(inner_src.chars().next())
            && edge(inner_src.chars().next_back())
            && !inner_src.contains(['_', '*']);
        let style = if strong {
            "**"
        } else if in_emph {
            "*"
        } else {
            "_"
        };
        let original: String = delims.iter().map(|t| t.text().to_string()).collect();
        let (o, c) = if safe {
            (style.to_string(), style.to_string())
        } else {
            let half = original.len() / 2;
            (original[..half].to_string(), original[half..].to_string())
        };
        // Trivia stands before the opening and after the closing delimiter as well.
        let kids: Vec<SyntaxElement> = n
            .children_with_tokens()
            .filter(|e| !matches!(e, SyntaxElement::Token(t) if t.kind() == k::EMPHASIS_DELIM))
            .collect();
        let (opens, closes) = (delims[0].text_range().start(), close.text_range().start());
        let head = kids
            .iter()
            .take_while(|e| e.text_range().end() <= opens)
            .count();
        let body = kids
            .iter()
            .take_while(|e| e.text_range().end() <= closes)
            .count();
        self.run(&kids[..head], in_emph, out);
        out.push_str(&o);
        self.run(&kids[head..body], !strong || in_emph, out);
        out.push_str(&c);
        self.run(&kids[body..], in_emph, out);
    }

    /// The text of a prose token with runs of blanks collapsed (kept next to a backtick, where
    /// the parser may have missed a code span). Escapes are the author's: Prettier 3.9 adds none.
    pub(super) fn escape(&self, t: &SyntaxToken) -> String {
        let text = t.text();
        if text.contains('`') {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        for (i, w) in text
            .split([' ', '\t'])
            .filter(|w| !w.is_empty())
            .enumerate()
        {
            if i > 0 || text.starts_with([' ', '\t']) {
                out.push(' ');
            }
            out.push_str(w);
        }
        if text.ends_with([' ', '\t']) && !out.is_empty() {
            out.push(' ');
        }
        out
    }
}
