//! Code of another language inside a file (a Dockerfile's `RUN`, a Markdown fence), as the host's
//! provider reports it. The engine parses the region with the other language's provider.

use crate::TextRange;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Embedded {
    /// Names the language: its name or a file extension (a fence's info word).
    pub language: String,
    /// Where the code is, in host offsets.
    pub range: TextRange,
    /// The parts of `range` that belong to the host, in order: the container prefix of each fence
    /// line, the comment lines inside a continued `RUN`.
    pub carved: Vec<TextRange>,
    /// Whether the host lets the language's formatter rewrite the code.
    pub format: bool,
}

impl Embedded {
    /// The region's text with the carved parts blanked: as long as the region, so an offset in
    /// the result is an offset from `range.start()`.
    pub fn blanked(&self, text: &str) -> String {
        let mut out = text[self.range].to_string();
        for c in &self.carved {
            let at = usize::from(c.start() - self.range.start())
                ..usize::from(c.end() - self.range.start());
            out.replace_range(at.clone(), &" ".repeat(at.len()));
        }
        out
    }

    /// The region's text without the carved parts: the code as it reads outside the host.
    pub fn stripped(&self, text: &str) -> String {
        let (mut out, mut from) = (String::new(), self.range.start());
        for c in &self.carved {
            out.push_str(&text[TextRange::new(from, c.start())]);
            from = c.end();
        }
        out + &text[TextRange::new(from, self.range.end())]
    }
}
