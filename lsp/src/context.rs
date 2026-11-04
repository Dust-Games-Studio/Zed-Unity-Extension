use tower_lsp::lsp_types::{Position, Range};

use crate::parse::TextRange;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    References,
    IncludePlatforms,
    ExcludePlatforms,
    PrecompiledReferences,
    VersionDefineName,
    TopLevelKey,
    VersionDefineKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub kind: CompletionKind,
    pub prefix: String,
    pub replace: TextRange,
    pub in_string: bool,
}

#[derive(Debug, Clone)]
struct ObjectFrame {
    path: Vec<String>,
    expecting_key: bool,
    pending_key: Option<String>,
}

#[derive(Debug, Clone)]
struct ArrayFrame {
    path: Vec<String>,
    expecting_value: bool,
}

#[derive(Debug, Clone)]
enum Frame {
    Object(ObjectFrame),
    Array(ArrayFrame),
}

pub fn completion_context(text: &str, offset: usize) -> Option<CompletionContext> {
    let offset = clamp_to_char_boundary(text, offset);
    let bytes = text.as_bytes();
    let mut stack: Vec<Frame> = Vec::new();
    let mut index = 0;

    while index < offset {
        match bytes[index] {
            b'"' => {
                let (end, terminated) = scan_string(bytes, index);
                let cursor_inside = index < offset
                    && if terminated {
                        offset < end
                    } else {
                        offset <= text.len()
                    };

                if cursor_inside {
                    let content_start = index + 1;
                    let content_end = if terminated {
                        end.saturating_sub(1)
                    } else {
                        offset
                    };
                    let prefix = text
                        .get(content_start..offset)
                        .unwrap_or_default()
                        .to_string();
                    let kind = match stack.last() {
                        Some(frame) if frame.expecting_key() => key_kind(frame)?,
                        Some(frame) => value_kind(frame)?,
                        None => return None,
                    };
                    return Some(CompletionContext {
                        kind,
                        prefix,
                        replace: TextRange::new(content_start, content_end),
                        in_string: true,
                    });
                }

                let value = decode_string(text, index, end, terminated);
                consume_string(&mut stack, value);
                index = end.max(index + 1);
            }
            b'{' => {
                let path = take_child_path(&mut stack);
                stack.push(Frame::Object(ObjectFrame {
                    path,
                    expecting_key: true,
                    pending_key: None,
                }));
                index += 1;
            }
            b'[' => {
                let path = take_child_path(&mut stack);
                stack.push(Frame::Array(ArrayFrame {
                    path,
                    expecting_value: true,
                }));
                index += 1;
            }
            b'}' | b']' => {
                stack.pop();
                index += 1;
            }
            b',' => {
                match stack.last_mut() {
                    Some(Frame::Object(object)) => {
                        object.expecting_key = true;
                        object.pending_key = None;
                    }
                    Some(Frame::Array(array)) => array.expecting_value = true,
                    None => {}
                }
                index += 1;
            }
            b':' | b' ' | b'\t' | b'\r' | b'\n' => {
                index += 1;
            }
            _ => {
                let end = scan_scalar(bytes, index);
                consume_scalar(&mut stack);
                index = end.max(index + 1);
            }
        }
    }

    let frame = stack.last()?;
    match frame {
        Frame::Object(object) if object.expecting_key => Some(CompletionContext {
            kind: key_kind(frame)?,
            prefix: String::new(),
            replace: TextRange::new(offset, offset),
            in_string: false,
        }),
        Frame::Object(object) => {
            let key = object.pending_key.as_deref()?;
            let kind = value_kind_for_object(&object.path, key)?;
            Some(CompletionContext {
                kind,
                prefix: String::new(),
                replace: TextRange::new(offset, offset),
                in_string: false,
            })
        }
        Frame::Array(array) if array.expecting_value => Some(CompletionContext {
            kind: value_kind_for_array(&array.path)?,
            prefix: String::new(),
            replace: TextRange::new(offset, offset),
            in_string: false,
        }),
        _ => None,
    }
}

impl Frame {
    fn expecting_key(&self) -> bool {
        matches!(self, Frame::Object(object) if object.expecting_key)
    }
}

fn key_kind(frame: &Frame) -> Option<CompletionKind> {
    let Frame::Object(object) = frame else {
        return None;
    };
    if object.path.is_empty() {
        Some(CompletionKind::TopLevelKey)
    } else if object.path.len() == 1 && object.path[0] == "versionDefines" {
        Some(CompletionKind::VersionDefineKey)
    } else {
        None
    }
}

fn value_kind(frame: &Frame) -> Option<CompletionKind> {
    match frame {
        Frame::Object(object) => value_kind_for_object(
            &object.path,
            object.pending_key.as_deref().unwrap_or_default(),
        ),
        Frame::Array(array) => value_kind_for_array(&array.path),
    }
}

fn value_kind_for_object(path: &[String], key: &str) -> Option<CompletionKind> {
    if path.is_empty() && key == "reference" {
        Some(CompletionKind::References)
    } else if path.len() == 1 && path[0] == "versionDefines" && key == "name" {
        Some(CompletionKind::VersionDefineName)
    } else {
        None
    }
}

fn value_kind_for_array(path: &[String]) -> Option<CompletionKind> {
    match path.last().map(String::as_str) {
        Some("references") => Some(CompletionKind::References),
        Some("includePlatforms") => Some(CompletionKind::IncludePlatforms),
        Some("excludePlatforms") => Some(CompletionKind::ExcludePlatforms),
        Some("precompiledReferences") => Some(CompletionKind::PrecompiledReferences),
        _ => None,
    }
}

fn take_child_path(stack: &mut [Frame]) -> Vec<String> {
    let Some(parent) = stack.last_mut() else {
        return Vec::new();
    };
    match parent {
        Frame::Object(object) => {
            let mut path = object.path.clone();
            if let Some(key) = object.pending_key.take() {
                path.push(key);
            }
            path
        }
        Frame::Array(array) => {
            array.expecting_value = false;
            array.path.clone()
        }
    }
}

fn consume_string(stack: &mut [Frame], value: String) {
    let Some(parent) = stack.last_mut() else {
        return;
    };
    match parent {
        Frame::Object(object) if object.expecting_key => {
            object.pending_key = Some(value);
            object.expecting_key = false;
        }
        Frame::Object(object) => {
            object.pending_key = None;
        }
        Frame::Array(array) => array.expecting_value = false,
    }
}

fn consume_scalar(stack: &mut [Frame]) {
    let Some(parent) = stack.last_mut() else {
        return;
    };
    match parent {
        Frame::Object(object) => object.pending_key = None,
        Frame::Array(array) => array.expecting_value = false,
    }
}

fn scan_string(bytes: &[u8], start: usize) -> (usize, bool) {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = (index + 2).min(bytes.len()),
            b'"' => return (index + 1, true),
            _ => index += 1,
        }
    }
    (bytes.len(), false)
}

fn scan_scalar(bytes: &[u8], start: usize) -> usize {
    let mut index = start;
    while index < bytes.len()
        && !matches!(
            bytes[index],
            b',' | b']' | b'}' | b' ' | b'\t' | b'\r' | b'\n'
        )
    {
        index += 1;
    }
    index
}

fn decode_string(text: &str, start: usize, end: usize, terminated: bool) -> String {
    if terminated {
        serde_json::from_str(&text[start..end])
            .unwrap_or_else(|_| text[start + 1..end.saturating_sub(1)].to_string())
    } else {
        let raw = &text[start + 1..];
        serde_json::from_str(&format!("\"{raw}\"")).unwrap_or_else(|_| raw.to_string())
    }
}

fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
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
        assert_eq!(index.offset_to_position(text, 999), Position::new(0, 5));
    }

    fn context_at(text: &str, cursor_text: &str) -> Option<CompletionContext> {
        let offset = text.find(cursor_text).unwrap() + cursor_text.len();
        completion_context(text, offset)
    }

    #[test]
    fn completes_reference_strings_and_unclosed_references() {
        let closed = context_at(r#"{"references":["Sha"]}"#, "Sha").unwrap();
        assert_eq!(closed.kind, CompletionKind::References);
        assert_eq!(closed.prefix, "Sha");
        assert!(closed.in_string);
        assert_eq!(
            &r#"{"references":["Sha"]}"#[closed.replace.start..closed.replace.end],
            "Sha"
        );

        let unclosed = context_at(r#"{"references":["Sha"#, "Sha").unwrap();
        assert_eq!(unclosed.kind, CompletionKind::References);
        assert_eq!(unclosed.prefix, "Sha");
        assert!(unclosed.in_string);
    }

    #[test]
    fn completes_platforms_version_defines_and_precompiled_references() {
        assert_eq!(
            context_at(r#"{"includePlatforms":["Ed"]}"#, "Ed")
                .unwrap()
                .kind,
            CompletionKind::IncludePlatforms
        );
        assert_eq!(
            context_at(r#"{"excludePlatforms":["i"]}"#, "i")
                .unwrap()
                .kind,
            CompletionKind::ExcludePlatforms
        );
        assert_eq!(
            context_at(r#"{"precompiledReferences":["New"]}"#, "New")
                .unwrap()
                .kind,
            CompletionKind::PrecompiledReferences
        );
        assert_eq!(
            context_at(r#"{"versionDefines":[{"name":"com.uni"}]}"#, "com.uni")
                .unwrap()
                .kind,
            CompletionKind::VersionDefineName
        );
    }

    #[test]
    fn completes_asmref_reference_values() {
        let context = context_at(r#"{"reference":"Sha"}"#, "Sha").unwrap();
        assert_eq!(context.kind, CompletionKind::References);
        assert_eq!(context.prefix, "Sha");
    }

    #[test]
    fn completes_keys_in_the_right_object() {
        let top = context_at(r#"{"na"#, "na").unwrap();
        assert_eq!(top.kind, CompletionKind::TopLevelKey);
        assert_eq!(top.prefix, "na");

        let after_comma_text = r#"{"name":"Game","#;
        let after_comma = completion_context(after_comma_text, after_comma_text.len()).unwrap();
        assert_eq!(after_comma.kind, CompletionKind::TopLevelKey);
        assert!(!after_comma.in_string);

        let nested_text = r#"{"versionDefines":[{"#;
        let nested = completion_context(nested_text, nested_text.len()).unwrap();
        assert_eq!(nested.kind, CompletionKind::VersionDefineKey);
    }

    #[test]
    fn does_not_complete_unrelated_values() {
        assert_eq!(context_at(r#"{"name":"Ga"}"#, "Ga"), None);
        assert_eq!(context_at(r#"{"rootNamespace":"Ga"}"#, "Ga"), None);
    }
}
