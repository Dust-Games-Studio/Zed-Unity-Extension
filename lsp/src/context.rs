use tower_lsp::lsp_types::{Position, Range};

/// Maps byte offsets in a UTF-8 document to LSP UTF-16 positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (offset, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { line_starts }
    }

    pub fn offset_to_position(&self, text: &str, offset: usize) -> Position {
        let offset = offset.min(text.len());
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        };
        let line_start = self.line_starts[line];
        let character = text[line_start..offset]
            .chars()
            .map(|character| character.len_utf16() as u32)
            .sum();
        Position::new(line as u32, character)
    }

    pub fn position_to_offset(&self, text: &str, position: Position) -> usize {
        let line = position.line as usize;
        let Some(&line_start) = self.line_starts.get(line) else {
            return text.len();
        };
        let line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(text.len());
        let mut utf16 = 0;
        for (offset, character) in text[line_start..line_end].char_indices() {
            if utf16 >= position.character {
                return line_start + offset;
            }
            utf16 += character.len_utf16() as u32;
        }
        line_end
    }

    pub fn range(&self, text: &str, start: usize, end: usize) -> Range {
        Range::new(
            self.offset_to_position(text, start),
            self.offset_to_position(text, end),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_ascii_offsets() {
        let text = "one\ntwo\nthree";
        let index = LineIndex::new(text);
        let offset = text.find("three").unwrap();
        assert_eq!(index.offset_to_position(text, offset), Position::new(2, 0));
        assert_eq!(
            index.position_to_offset(text, Position::new(2, 3)),
            offset + 3
        );
        assert_eq!(
            index.range(text, offset, offset + 5),
            Range::new(Position::new(2, 0), Position::new(2, 5))
        );
    }

    #[test]
    fn converts_utf16_columns() {
        let text = "a😀b\nsecond";
        let index = LineIndex::new(text);
        let after_emoji = "a😀".len();
        assert_eq!(
            index.offset_to_position(text, after_emoji),
            Position::new(0, 3)
        );
        assert_eq!(
            index.position_to_offset(text, Position::new(0, 3)),
            after_emoji
        );
    }

    #[test]
    fn clamps_out_of_range_positions() {
        let text = "short";
        let index = LineIndex::new(text);
        assert_eq!(
            index.position_to_offset(text, Position::new(99, 99)),
            text.len()
        );
        assert_eq!(
            index.offset_to_position(text, 999),
            Position::new(0, 5)
        );
    }
}
