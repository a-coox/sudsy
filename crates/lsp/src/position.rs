//! Byte offsets of a text to LSP positions (line and UTF-16 column) and back, and the edits made
//! of them.

use core_rules::LineIndex;
use lsp_types::{Position, Range, TextEdit};

/// The line starts of one text; converts between its byte offsets and LSP positions.
pub struct Positions<'a> {
    text: &'a str,
    lines: LineIndex,
}

impl<'a> Positions<'a> {
    pub fn new(text: &'a str) -> Self {
        Positions {
            text,
            lines: LineIndex::new(text),
        }
    }

    pub fn position(&self, offset: usize) -> Position {
        let offset = offset.min(self.text.len());
        let line = self.lines.line((offset as u32).into());
        let (start, _) = self.lines.line_range(self.text, line);
        let column = self.text[start..offset].encode_utf16().count();
        Position::new(line as u32, column as u32)
    }

    pub fn range(&self, start: usize, end: usize) -> Range {
        Range::new(self.position(start), self.position(end))
    }

    /// The byte offset of a position; a column past the end of the line is the end of the line,
    /// a line past the end of the text is the end of the text.
    pub fn offset(&self, position: Position) -> usize {
        let line = position.line as usize;
        if line >= self.lines.line_count() {
            return self.text.len();
        }
        let (start, end) = self.lines.line_range(self.text, line);
        let mut units = 0;
        for (i, c) in self.text[start..end].char_indices() {
            if units >= position.character {
                return start + i;
            }
            units += c.len_utf16() as u32;
        }
        end
    }
}

/// Applies one change event: `None` replaces the whole text.
pub fn apply_change(text: &mut String, range: Option<Range>, replacement: &str) {
    match range {
        None => *text = replacement.to_string(),
        Some(range) => {
            let positions = Positions::new(text);
            let (start, end) = (positions.offset(range.start), positions.offset(range.end));
            text.replace_range(start..end.max(start), replacement);
        }
    }
}

/// The one edit that turns `old` into `new`: the text between their common prefix and suffix.
/// `None` when they are equal. A single small edit keeps the cursor and folds of an editor where
/// they are, which replacing the whole document would not.
pub fn minimal_edit(old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(prefix) || !new.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    while !old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix) {
        suffix -= 1;
    }
    let positions = Positions::new(old);
    Some(TextEdit::new(
        positions.range(prefix, old.len() - suffix),
        new[prefix..new.len() - suffix].to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_count_utf16_units_and_all_line_breaks() {
        // "é" is one unit, the emoji two; \r\n is one break, a lone \r another.
        let text = "aé😀b\r\nc\rd";
        let p = Positions::new(text);
        let after_emoji = text.find('b').unwrap();
        assert_eq!(p.position(after_emoji), Position::new(0, 4));
        assert_eq!(p.offset(Position::new(0, 4)), after_emoji);
        assert_eq!(p.position(text.find('c').unwrap()), Position::new(1, 0));
        assert_eq!(p.position(text.find('d').unwrap()), Position::new(2, 0));
        // Past the end of a line, and of the text.
        assert_eq!(
            p.offset(Position::new(1, 99)),
            text.find("c\r").unwrap() + 1
        );
        assert_eq!(p.offset(Position::new(9, 0)), text.len());
        // The middle of a surrogate pair resolves to the end of the pair.
        assert_eq!(p.offset(Position::new(0, 3)), after_emoji);
    }

    #[test]
    fn changes_apply_with_ranges_or_whole() {
        let mut text = "ab\ncd".to_string();
        apply_change(
            &mut text,
            Some(Range::new(Position::new(0, 1), Position::new(1, 1))),
            "X",
        );
        assert_eq!(text, "aXd");
        apply_change(&mut text, None, "new");
        assert_eq!(text, "new");
    }

    #[test]
    fn minimal_edit_trims_prefix_and_suffix_on_char_boundaries() {
        assert!(minimal_edit("same", "same").is_none());
        let edit = minimal_edit("a = 1;\nb = 2;\n", "a = 1;\nb  =  2;\n").unwrap();
        assert_eq!(edit.range.start, Position::new(1, 2));
        assert_eq!(edit.new_text, " = ");
        // Characters that share leading bytes are not split.
        let edit = minimal_edit("é", "è").unwrap();
        assert_eq!(edit.new_text, "è");
        assert_eq!(
            edit.range,
            Range::new(Position::new(0, 0), Position::new(0, 1))
        );
    }
}
