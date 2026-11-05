//! The signature `extra_check` compares: structure and text, without layout.

use super::*;

/// What the formatter must not change, for the whole tree.
pub(super) fn signature(tree: &SyntaxNode) -> Vec<String> {
    let src = tree.text().to_string();
    let mut out = Vec::new();
    Render { src: &src, tab: 2 }.sig(&SyntaxElement::Node(tree.clone()), &mut out);
    out
}

impl Render<'_> {
    /// What formatting must not change, one entry per element: structure, levels, list kinds,
    /// prose with escapes undone and blanks collapsed, and raw text without container prefixes.
    pub(super) fn sig(&self, elem: &SyntaxElement, out: &mut Vec<String>) {
        let all: Vec<SyntaxElement> = match elem {
            SyntaxElement::Node(n) => n.descendants_with_tokens().collect(),
            SyntaxElement::Token(_) => vec![elem.clone()],
        };
        for e in all {
            match e {
                SyntaxElement::Node(n) => {
                    let kind = n.kind();
                    out.push(if kind == k::HEADING {
                        format!("H{}", heading_level(self.src, &n))
                    } else if kind == k::LIST {
                        let first = significant(&n).find(|t| t.kind() == k::LIST_MARKER);
                        let marker = first.map_or(String::new(), |t| t.text().trim().to_string());
                        let number = marker.trim_end_matches(['.', ')']).parse::<u64>().ok();
                        format!("L{}", number.map_or(String::new(), |n| n.to_string()))
                    } else {
                        format!("{:?}", kind.0)
                    });
                }
                SyntaxElement::Token(t) => self.sig_token(&t, out),
            }
        }
    }

    pub(super) fn sig_token(&self, t: &SyntaxToken, out: &mut Vec<String>) {
        let kind = t.kind();
        let text = t.text().replace('\r', "");
        let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
        if kind == k::TEXT {
            let in_heading = t
                .parent()
                .and_then(|p| p.parent())
                .is_some_and(|h| h.kind() == k::HEADING);
            let text = if in_heading {
                without_closing(&text).to_string()
            } else {
                text
            };
            out.push(words(&text.replace("\\*", "*").replace("\\_", "_")));
        } else if kind == k::TASK_MARKER {
            out.push(text.to_lowercase());
        } else if kind == k::THEMATIC_BREAK {
            out.push("---".into());
        } else if kind == k::INDENTED_CODE {
            out.push(
                self.indented(t)
                    .iter()
                    .map(|l| l.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        } else if [k::HTML_BLOCK, k::HTML_COMMENT, k::CODE_TEXT, k::CODE_SPAN].contains(&kind) {
            let lines = raw_lines(&text, &self.chain_of_token(t));
            out.push(
                lines
                    .iter()
                    .map(|l| l.trim_end())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        } else if kind == k::TABLE_DELIMITER {
            out.push(
                text.split('|')
                    .filter(|c| !c.trim().is_empty())
                    .map(|c| format!("{:?}", (c.trim().starts_with(':'), c.trim().ends_with(':'))))
                    .collect(),
            );
        } else if kind == k::LINK_DEF || kind == k::INFO {
            out.push(words(&text));
        } else if !is_trivia(kind)
            && ![
                k::EMPHASIS_DELIM,
                k::LIST_MARKER,
                k::QUOTE_MARKER,
                k::PIPE,
                k::FENCE,
            ]
            .contains(&kind)
        {
            out.push(text);
        }
    }
}
