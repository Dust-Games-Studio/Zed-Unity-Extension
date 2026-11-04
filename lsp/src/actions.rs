use std::collections::HashMap;

use tower_lsp::lsp_types::{CodeAction, CodeActionKind, TextEdit, Url, WorkspaceEdit};

use crate::context::LineIndex;
use crate::index::{IndexedFile, ProjectIndex};
use crate::parse::{FileKind, SpannedString, TextRange};
use crate::resolve;

/// Code actions for assembly references intersecting `range`.
pub fn code_actions(
    project: &ProjectIndex,
    file: &IndexedFile,
    range: TextRange,
) -> Vec<CodeAction> {
    if file.text != file.data_text {
        return Vec::new();
    }

    let text = file.text.as_ref();
    let references: Vec<&SpannedString> = match file.kind {
        FileKind::Asmdef => file.references().iter().collect(),
        FileKind::Asmref => file.asmref_reference().into_iter().collect(),
    };

    let mut actions = Vec::new();
    for reference in references {
        if !intersects(reference.range, range) {
            continue;
        }

        let targets = resolve::resolve_value(project, &reference.value);
        if targets.is_empty() {
            if file.kind == FileKind::Asmdef {
                let removal = removal_range(text, reference.range);
                if let Some(edit) = edit_for(project, file, text, removal, "") {
                    actions.push(action(
                        format!("Remove unresolved reference `{}`", reference.value),
                        edit,
                        false,
                    ));
                }
            }

            for (index, target) in resolve::nearest_assemblies(project, &reference.value, 3)
                .into_iter()
                .enumerate()
            {
                let Some(name) = target.name() else {
                    continue;
                };
                if let Some(edit) = edit_for(project, file, text, reference.range, name) {
                    actions.push(action(
                        format!("Add reference to `{name}`"),
                        edit,
                        index == 0,
                    ));
                }
            }
            continue;
        }

        if let Some(guid) = resolve::guid_reference(&reference.value) {
            let Some(target) = targets.first() else {
                continue;
            };
            if targets.len() != 1 {
                continue;
            }
            let Some(name) = target.name() else {
                continue;
            };
            if let Some(edit) = edit_for(project, file, text, reference.range, name) {
                actions.push(action(
                    format!("Convert GUID reference `{guid}` to assembly name `{name}`"),
                    edit,
                    false,
                ));
            }
        } else if targets.len() == 1 {
            let Some(guid) = targets[0].guid.as_deref() else {
                continue;
            };
            let replacement = format!("GUID:{guid}");
            if let Some(edit) = edit_for(project, file, text, reference.range, &replacement) {
                actions.push(action(
                    format!("Convert assembly reference to `{replacement}`"),
                    edit,
                    false,
                ));
            }
        }
    }

    actions
}

/// The range to delete when removing an array element, including one adjacent comma.
fn removal_range(text: &str, content: TextRange) -> TextRange {
    let bytes = text.as_bytes();
    let mut start = content.start;
    let mut end = content.end;

    if start > 0 && bytes.get(start - 1) == Some(&b'"') {
        start -= 1;
    }
    if bytes.get(end) == Some(&b'"') {
        end += 1;
    }

    let mut after = end;
    while after < bytes.len() && bytes[after].is_ascii_whitespace() {
        after += 1;
    }
    if bytes.get(after) == Some(&b',') {
        let mut removal_end = after + 1;
        while removal_end < bytes.len() && bytes[removal_end].is_ascii_whitespace() {
            removal_end += 1;
        }
        return TextRange::new(start, removal_end);
    }

    let mut before = start;
    while before > 0 && bytes[before - 1].is_ascii_whitespace() {
        before -= 1;
    }
    if before > 0 && bytes[before - 1] == b',' {
        return TextRange::new(before - 1, end);
    }

    TextRange::new(start, end)
}

fn edit_for(
    project: &ProjectIndex,
    file: &IndexedFile,
    text: &str,
    range: TextRange,
    new_text: &str,
) -> Option<WorkspaceEdit> {
    let display_path = project.display_root.join(&file.relative_path);
    let uri = Url::from_file_path(display_path).ok()?;
    let line_index = LineIndex::new(text);
    let edit = TextEdit {
        range: line_index.range(text, range.start, range.end),
        new_text: new_text.to_string(),
    };
    Some(WorkspaceEdit {
        changes: Some(HashMap::from([(uri, vec![edit])])),
        document_changes: None,
        change_annotations: None,
    })
}

fn action(title: String, edit: WorkspaceEdit, preferred: bool) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: Some(preferred),
        disabled: None,
        data: None,
    }
}

fn intersects(reference: TextRange, range: TextRange) -> bool {
    reference.start.saturating_sub(1) <= range.end && range.start <= reference.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn shared() -> IndexedFile {
        indexed(
            "Assets/Shared.asmdef",
            r#"{ "name": "Shared" }"#,
            false,
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        )
    }

    fn broken(text: &str) -> IndexedFile {
        file("Assets/Broken.asmdef", text)
    }

    fn project_with_broken(text: &str) -> ProjectIndex {
        project(vec![shared(), broken(text)])
    }

    fn actions_on(text: &str, needle: &str) -> Vec<CodeAction> {
        let project = project_with_broken(text);
        let file = project
            .file(&root().join("Assets/Broken.asmdef"))
            .expect("broken");
        let offset = offset_of(text, needle) + 1;
        code_actions(&project, file, TextRange::new(offset, offset))
    }

    fn actions_at(text: &str, range: TextRange) -> Vec<CodeAction> {
        let project = project_with_broken(text);
        let file = project
            .file(&root().join("Assets/Broken.asmdef"))
            .expect("broken");
        code_actions(&project, file, range)
    }

    fn apply_action(text: &str, action: &CodeAction) -> String {
        let edit = action.edit.as_ref().expect("edit");
        let changes = edit.changes.as_ref().expect("changes");
        let edits = changes.values().next().expect("one file");
        let line_index = LineIndex::new(text);
        let mut spans: Vec<(usize, usize, &str)> = edits
            .iter()
            .map(|edit| {
                (
                    line_index.position_to_offset(text, edit.range.start),
                    line_index.position_to_offset(text, edit.range.end),
                    edit.new_text.as_str(),
                )
            })
            .collect();
        spans.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));

        let mut result = text.to_string();
        for (start, end, new_text) in spans {
            result.replace_range(start..end, new_text);
        }
        result
    }

    fn titles(actions: &[CodeAction]) -> Vec<&str> {
        actions.iter().map(|action| action.title.as_str()).collect()
    }

    fn find<'a>(actions: &'a [CodeAction], needle: &str) -> &'a CodeAction {
        actions
            .iter()
            .find(|action| action.title.contains(needle))
            .unwrap_or_else(|| {
                panic!(
                    "missing action containing {needle:?}: {:?}",
                    titles(actions)
                )
            })
    }

    #[test]
    fn removes_a_middle_reference_with_its_comma() {
        let text = r#"{"name":"Broken","references":["Missing","Shared"]}"#;
        let actions = actions_on(text, "Missing");
        let action = find(&actions, "Remove unresolved reference");
        assert_eq!(
            apply_action(text, action),
            r#"{"name":"Broken","references":["Shared"]}"#
        );
    }

    #[test]
    fn removes_a_last_reference_with_the_preceding_comma() {
        let text = r#"{"name":"Broken","references":["Shared","Missing"]}"#;
        let actions = actions_on(text, "Missing");
        let action = find(&actions, "Remove unresolved reference");
        assert_eq!(
            apply_action(text, action),
            r#"{"name":"Broken","references":["Shared"]}"#
        );
    }

    #[test]
    fn removes_a_single_reference_to_an_empty_array() {
        let text = r#"{"name":"Broken","references":["Missing"]}"#;
        let actions = actions_on(text, "Missing");
        let action = find(&actions, "Remove unresolved reference");
        assert_eq!(
            apply_action(text, action),
            r#"{"name":"Broken","references":[]}"#
        );
    }

    #[test]
    fn removes_a_reference_in_a_multiline_array() {
        let text = "{\n  \"name\": \"Broken\",\n  \"references\": [\n    \"Missing\",\n    \"Shared\"\n  ]\n}";
        let actions = actions_on(text, "Missing");
        let action = find(&actions, "Remove unresolved reference");
        let result = apply_action(text, action);
        assert!(result.contains("\"Shared\""), "{result}");
        assert!(!result.contains("Missing"), "{result}");
        assert!(
            serde_json::from_str::<serde_json::Value>(&result).is_ok(),
            "{result}"
        );
    }

    #[test]
    fn suggests_the_nearest_assembly_for_a_typo() {
        let text = r#"{"name":"Broken","references":["Shard"]}"#;
        let actions = actions_on(text, "Shard");
        let action = find(&actions, "Add reference to `Shared`");
        assert_eq!(
            apply_action(text, action),
            r#"{"name":"Broken","references":["Shared"]}"#
        );
    }

    #[test]
    fn converts_a_guid_reference_to_a_name() {
        let text = r#"{"name":"Broken","references":["GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]}"#;
        let actions = actions_on(text, "GUID:");
        let action = find(&actions, "Convert GUID reference");
        assert_eq!(
            apply_action(text, action),
            r#"{"name":"Broken","references":["Shared"]}"#
        );
    }

    #[test]
    fn converts_a_name_reference_to_a_guid() {
        let text = r#"{"name":"Broken","references":["Shared"]}"#;
        let actions = actions_on(text, "Shared");
        let action = find(&actions, "Convert assembly reference to");
        assert_eq!(
            apply_action(text, action),
            r#"{"name":"Broken","references":["GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]}"#
        );
    }

    #[test]
    fn does_not_convert_unresolved_or_guidless_references() {
        let unresolved = actions_on(r#"{"name":"Broken","references":["Missing"]}"#, "Missing");
        assert!(!titles(&unresolved)
            .iter()
            .any(|title| title.contains("Convert")));

        let no_guid = file("Assets/NoGuid.asmdef", r#"{ "name": "NoGuid" }"#);
        let project = project(vec![
            no_guid,
            broken(r#"{"name":"Broken","references":["NoGuid"]}"#),
        ]);
        let file = project
            .file(&root().join("Assets/Broken.asmdef"))
            .expect("broken");
        let text = file.text.as_ref();
        let offset = offset_of(text, "NoGuid") + 1;
        let actions = code_actions(&project, file, TextRange::new(offset, offset));
        assert!(!titles(&actions)
            .iter()
            .any(|title| title.contains("Convert")));
    }

    #[test]
    fn does_not_remove_an_asmref_reference() {
        let text = r#"{ "reference": "Shard" }"#;
        let project = project(vec![shared(), file("Assets/Refs/Broken.asmref", text)]);
        let file = project
            .file(&root().join("Assets/Refs/Broken.asmref"))
            .expect("asmref");
        let offset = offset_of(text, "Shard") + 1;
        let actions = code_actions(&project, file, TextRange::new(offset, offset));
        assert!(!titles(&actions)
            .iter()
            .any(|title| title.contains("Remove")));
        assert!(titles(&actions)
            .iter()
            .any(|title| title.contains("Add reference to")));
    }

    #[test]
    fn converts_an_asmref_reference() {
        let text = r#"{ "reference": "Shared" }"#;
        let project = project(vec![shared(), file("Assets/Refs/Ref.asmref", text)]);
        let file = project
            .file(&root().join("Assets/Refs/Ref.asmref"))
            .expect("asmref");
        let offset = offset_of(text, "Shared") + 1;
        let actions = code_actions(&project, file, TextRange::new(offset, offset));
        let action = find(&actions, "Convert assembly reference to");
        assert_eq!(
            apply_action(text, action),
            r#"{ "reference": "GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" }"#
        );
    }

    #[test]
    fn only_offers_actions_for_the_requested_range() {
        let text = r#"{"name":"Broken","references":["Missing"]}"#;
        assert!(actions_at(text, TextRange::new(0, 1)).is_empty());
    }

    #[test]
    fn offers_actions_when_the_cursor_is_on_the_opening_quote() {
        let text = r#"{"name":"Broken","references":["Missing"]}"#;
        let opening_quote = offset_of(text, "\"Missing\"");
        let actions = actions_at(text, TextRange::new(opening_quote, opening_quote));
        assert!(titles(&actions)
            .iter()
            .any(|title| title.contains("Remove unresolved reference")));
    }

    #[test]
    fn returns_no_actions_for_a_stale_document() {
        let mut stale = broken(r#"{"name":"Broken","references":["Missing"]}"#);
        stale.text = std::sync::Arc::from("{ \"name\": }");
        let project = project(vec![shared(), stale]);
        let file = project
            .file(&root().join("Assets/Broken.asmdef"))
            .expect("broken");
        let offset = offset_of(file.data_text.as_ref(), "Missing") + 1;
        assert!(code_actions(&project, file, TextRange::new(offset, offset)).is_empty());
    }
}
