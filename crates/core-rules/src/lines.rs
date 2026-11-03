//! Offsets to line and column.

use core_syntax::TextSize;

/// Where the lines of a text start.
pub struct LineIndex {
    starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let bytes = text.as_bytes();
        let mut starts = vec![0];
        for (i, &b) in bytes.iter().enumerate() {
            // `\r\n` is one break; a lone `\r` is a break too.
            if b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n')) {
                starts.push(i as u32 + 1);
            }
        }
        LineIndex { starts }
    }

    /// Zero-based line of a byte offset.
    pub fn line(&self, offset: TextSize) -> usize {
        self.starts
            .partition_point(|&s| s <= u32::from(offset))
            .saturating_sub(1)
    }

    /// One-based line and one-based column (in characters) of a byte offset.
    pub fn line_col(&self, text: &str, offset: TextSize) -> (usize, usize) {
        let line = self.line(offset);
        let start = self.starts[line] as usize;
        let end = (u32::from(offset) as usize).min(text.len());
        (line + 1, text[start..end].chars().count() + 1)
    }

    /// Byte range of a zero-based line, without its line break.
    pub fn line_range(&self, text: &str, line: usize) -> (usize, usize) {
        let start = self.starts[line] as usize;
        let end = self
            .starts
            .get(line + 1)
            .map_or(text.len(), |&e| e as usize);
        (
            start,
            text[start..end].trim_end_matches(['\n', '\r']).len() + start,
        )
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }
}
