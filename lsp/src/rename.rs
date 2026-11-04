use std::collections::HashMap;

use tower_lsp::lsp_types::{PrepareRenameResponse, TextEdit, Url, WorkspaceEdit};

use crate::context::LineIndex;
use crate::index::{IndexedFile, ProjectIndex};
use crate::parse::FileKind;
use crate::references;
use crate::resolve::{self, Symbol, SymbolKind};

/// Validate a rename at `offset`.
///
/// `None` means the cursor is not on an assembly symbol. `Some(Err(..))` means
/// the symbol is not safely renameable; the message is shown to the user.
pub fn prepare(
    project: &ProjectIndex,
    file: &IndexedFile,
    offset: usize,
) -> Option<Result<PrepareRenameResponse, String>> {
    let symbol = resolve::symbol_at(file, offset)?;
    Some(prepare_symbol(project, symbol))
}

/// Rename the assembly at `offset` and every name-form reference.
pub fn rename(
    project: &ProjectIndex,
    file: &IndexedFile,
    offset: usize,
    new_name: &str,
) -> Result<WorkspaceEdit, String> {
    let symbol = resolve::symbol_at(file, offset)
        .ok_or_else(|| "No assembly name or reference at the cursor.".to_string())?;
    let target = editable_target(project, symbol)?;
    validate_name(new_name)?;

    let old_name = target
        .name()
        .ok_or_else(|| "The assembly definition has no name.".to_string())?;
    let mut changes = HashMap::new();

    if new_name != old_name {
        ensure_current(target)?;
        let name_range = target
            .name_span()
            .ok_or_else(|| "The assembly definition has no name range.".to_string())?;
        add_edit(&mut changes, project, target, name_range, new_name)?;

        for site in references::references_to(project, &[target], false) {
            if site.file.external {
                continue;
            }
            if resolve::guid_reference(site.value).is_some() {
                continue;
            }
            ensure_current(site.file)?;
            add_edit(&mut changes, project, site.file, site.range, new_name)?;
        }
    }

    for edits in changes.values_mut() {
        edits.sort_by_key(|edit| {
            (
                edit.range.start.line,
                edit.range.start.character,
                edit.range.end.line,
                edit.range.end.character,
            )
        });
    }

    Ok(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

fn prepare_symbol(
    project: &ProjectIndex,
    symbol: Symbol<'_>,
) -> Result<PrepareRenameResponse, String> {
    let target = editable_target(project, symbol)?;
    let current = target
        .name()
        .ok_or_else(|| "The assembly definition has no name.".to_string())?;
    let line_index = LineIndex::new(&symbol.file.data_text);
    Ok(PrepareRenameResponse::RangeWithPlaceholder {
        range: line_index.range(&symbol.file.data_text, symbol.range.start, symbol.range.end),
        placeholder: current.to_string(),
    })
}

fn editable_target<'a>(
    project: &'a ProjectIndex,
    symbol: Symbol<'a>,
) -> Result<&'a IndexedFile, String> {
    if symbol.kind == SymbolKind::Reference && resolve::guid_reference(symbol.value).is_some() {
        return Err(
            "Rename the assembly from its declaration or a name reference; GUID references do not contain the name."
                .to_string(),
        );
    }

    let targets = resolve::targets(project, symbol);
    let target = match targets.as_slice() {
        [] => {
            return Err(format!(
                "Reference `{}` does not resolve to an assembly.",
                symbol.value
            ))
        }
        [target] => *target,
        _ => {
            return Err(format!(
                "`{}` matches {} assemblies; rename is ambiguous.",
                symbol.value,
                targets.len()
            ))
        }
    };

    if target.external {
        return Err(
            "Package assemblies are read-only; rename the assembly in its source package."
                .to_string(),
        );
    }
    if target.kind != FileKind::Asmdef {
        return Err("Only assembly definitions can be renamed.".to_string());
    }

    let name = target
        .name()
        .ok_or_else(|| "The assembly definition has no name.".to_string())?;
    if project.find_by_name(name).len() > 1 {
        return Err(format!(
            "Assembly name `{name}` is used by multiple definitions; rename is ambiguous."
        ));
    }
    Ok(target)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Assembly name must not be empty.".to_string());
    }
    if name.trim() != name {
        return Err("Assembly name must not start or end with whitespace.".to_string());
    }
    if resolve::guid_reference(name).is_some() {
        return Err("Assembly name must not start with `GUID:`.".to_string());
    }
    if name
        .chars()
        .any(|character| matches!(character, '"' | '\\') || character.is_control())
    {
        return Err(
            "Assembly name must not contain quotes, backslashes, or control characters."
                .to_string(),
        );
    }
    Ok(())
}

fn ensure_current(file: &IndexedFile) -> Result<(), String> {
    if file.text != file.data_text {
        return Err(format!(
            "`{}` is not valid JSON; fix it before renaming.",
            file.relative_path
        ));
    }
    Ok(())
}

fn add_edit(
    changes: &mut HashMap<Url, Vec<TextEdit>>,
    project: &ProjectIndex,
    file: &IndexedFile,
    range: crate::parse::TextRange,
    new_text: &str,
) -> Result<(), String> {
    let display_path = project.display_root.join(&file.relative_path);
    let uri = Url::from_file_path(display_path)
        .map_err(|_| format!("Could not build a file URL for {}", file.path.display()))?;
    let line_index = LineIndex::new(&file.data_text);
    changes.entry(uri).or_default().push(TextEdit {
        range: line_index.range(&file.data_text, range.start, range.end),
        new_text: new_text.to_string(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use std::sync::Arc;

    fn shared() -> IndexedFile {
        indexed(
            "Assets/Shared.asmdef",
            r#"{ "name": "Shared" }"#,
            false,
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        )
    }

    fn game() -> IndexedFile {
        file(
            "Assets/Game.asmdef",
            r#"{ "name": "Game", "references": ["Shared", "GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"] }"#,
        )
    }

    fn consumer() -> IndexedFile {
        file(
            "Assets/Consumer.asmdef",
            r#"{ "name": "Consumer", "references": ["Shared"] }"#,
        )
    }

    fn asmref() -> IndexedFile {
        file("Assets/Refs/Shared.asmref", r#"{ "reference": "Shared" }"#)
    }

    fn edits_for(edit: &WorkspaceEdit, project: &ProjectIndex, relative: &str) -> Vec<TextEdit> {
        let url = Url::from_file_path(project.display_root.join(relative)).unwrap();
        edit.changes
            .as_ref()
            .and_then(|changes| changes.get(&url))
            .cloned()
            .unwrap_or_default()
    }

    fn edit_text_at(text: &str, edit: &TextEdit) -> String {
        let line_index = LineIndex::new(text);
        let start = line_index.position_to_offset(text, edit.range.start);
        let end = line_index.position_to_offset(text, edit.range.end);
        text[start..end].to_string()
    }

    #[test]
    fn prepares_a_rename_on_a_declaration() {
        let text = r#"{ "name": "Shared" }"#;
        let project = project(vec![shared()]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let offset = offset_of(text, "Shared") + 1;
        let Some(Ok(PrepareRenameResponse::RangeWithPlaceholder { range, placeholder })) =
            prepare(&project, file, offset)
        else {
            panic!("expected a rename placeholder");
        };
        assert_eq!(placeholder, "Shared");
        let line_index = LineIndex::new(&file.data_text);
        let start = line_index.position_to_offset(&file.data_text, range.start);
        let end = line_index.position_to_offset(&file.data_text, range.end);
        assert_eq!(&file.data_text[start..end], "Shared");
    }

    #[test]
    fn prepares_a_rename_on_a_name_reference() {
        let text = r#"{ "name": "Consumer", "references": ["Shared"] }"#;
        let project = project(vec![shared(), consumer()]);
        let file = project
            .file(&root().join("Assets/Consumer.asmdef"))
            .expect("consumer");
        let offset = offset_of(text, "Shared") + 1;
        let Some(Ok(PrepareRenameResponse::RangeWithPlaceholder { range, placeholder })) =
            prepare(&project, file, offset)
        else {
            panic!("expected a rename placeholder");
        };
        assert_eq!(placeholder, "Shared");
        let line_index = LineIndex::new(&file.data_text);
        let start = line_index.position_to_offset(&file.data_text, range.start);
        let end = line_index.position_to_offset(&file.data_text, range.end);
        assert_eq!(&file.data_text[start..end], "Shared");
    }

    #[test]
    fn renames_the_declaration_and_name_references() {
        let project = project(vec![shared(), game(), consumer(), asmref()]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let edit = rename(&project, file, 12, "Core").expect("rename");

        let shared_edits = edits_for(&edit, &project, "Assets/Shared.asmdef");
        assert_eq!(shared_edits.len(), 1);
        assert_eq!(shared_edits[0].new_text, "Core");

        let game_edits = edits_for(&edit, &project, "Assets/Game.asmdef");
        assert_eq!(
            game_edits.len(),
            1,
            "the GUID reference must not be edited: {game_edits:?}"
        );
        assert_eq!(game_edits[0].new_text, "Core");

        let consumer_edits = edits_for(&edit, &project, "Assets/Consumer.asmdef");
        assert_eq!(consumer_edits.len(), 1);
        assert_eq!(consumer_edits[0].new_text, "Core");

        let asmref_edits = edits_for(&edit, &project, "Assets/Refs/Shared.asmref");
        assert_eq!(asmref_edits.len(), 1);
        assert_eq!(asmref_edits[0].new_text, "Core");

        let text = r#"{ "name": "Shared" }"#;
        assert_eq!(edit_text_at(text, &shared_edits[0]), "Shared");
    }

    #[test]
    fn renames_the_target_from_a_reference() {
        let text = r#"{ "name": "Consumer", "references": ["Shared"] }"#;
        let project = project(vec![shared(), consumer()]);
        let file = project
            .file(&root().join("Assets/Consumer.asmdef"))
            .expect("consumer");
        let offset = offset_of(text, "Shared") + 1;
        let edit = rename(&project, file, offset, "Core").expect("rename");
        assert_eq!(edits_for(&edit, &project, "Assets/Shared.asmdef").len(), 1);
        assert_eq!(
            edits_for(&edit, &project, "Assets/Consumer.asmdef").len(),
            1
        );
    }

    #[test]
    fn does_not_edit_external_references() {
        let external = indexed(
            "Packages/com.example/Package.asmdef",
            r#"{ "name": "Package", "references": ["Shared"] }"#,
            true,
            None,
        );
        let project = project(vec![shared(), external]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let edit = rename(&project, file, 12, "Core").expect("rename");
        let external_url = Url::from_file_path(
            project
                .display_root
                .join("Packages/com.example/Package.asmdef"),
        )
        .unwrap();
        assert!(
            !edit.changes.as_ref().unwrap().contains_key(&external_url),
            "package files must never be edited"
        );
    }

    #[test]
    fn refuses_package_assemblies() {
        let external = indexed(
            "Packages/com.example/Package.asmdef",
            r#"{ "name": "Package" }"#,
            true,
            None,
        );
        let text = r#"{ "name": "Package" }"#;
        let project = project(vec![external]);
        let file = project
            .file(&root().join("Packages/com.example/Package.asmdef"))
            .expect("package");
        let offset = offset_of(text, "Package") + 1;
        let error = rename(&project, file, offset, "Renamed").unwrap_err();
        assert!(error.contains("read-only"), "{error}");
        assert!(prepare(&project, file, offset)
            .expect("symbol")
            .unwrap_err()
            .contains("read-only"));
    }

    #[test]
    fn refuses_guid_references() {
        let project = project(vec![shared(), game()]);
        let file = project
            .file(&root().join("Assets/Game.asmdef"))
            .expect("game");
        let offset = offset_of(file.text.as_ref(), "GUID:") + 2;
        let error = rename(&project, file, offset, "Core").unwrap_err();
        assert!(error.contains("GUID"), "{error}");
        assert!(prepare(&project, file, offset)
            .expect("symbol")
            .unwrap_err()
            .contains("GUID"));
    }

    #[test]
    fn refuses_ambiguous_duplicate_names() {
        let a = file("Assets/A.asmdef", r#"{ "name": "Duplicate" }"#);
        let b = file("Assets/B.asmdef", r#"{ "name": "Duplicate" }"#);
        let consumer_text = r#"{ "name": "Consumer", "references": ["Duplicate"] }"#;
        let consumer = file("Assets/Consumer.asmdef", consumer_text);
        let project = project(vec![a, b, consumer]);

        let a = project.file(&root().join("Assets/A.asmdef")).expect("a");
        let error = rename(&project, a, 12, "Renamed").unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");

        let consumer = project
            .file(&root().join("Assets/Consumer.asmdef"))
            .expect("consumer");
        let offset = offset_of(consumer_text, "Duplicate") + 1;
        let error = rename(&project, consumer, offset, "Renamed").unwrap_err();
        assert!(error.contains("ambiguous"), "{error}");
    }

    #[test]
    fn rejects_invalid_names() {
        let project = project(vec![shared()]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        for name in [
            "",
            " Shared",
            "Shared ",
            "Ga\"me",
            "Ga\\me",
            "GUID:Game",
            "Ga\nme",
        ] {
            assert!(
                rename(&project, file, 12, name).is_err(),
                "expected {name:?} to be rejected"
            );
        }
        assert!(rename(&project, file, 12, "Game.Core-2_1").is_ok());
    }

    #[test]
    fn returns_empty_changes_for_the_same_name() {
        let project = project(vec![shared()]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let edit = rename(&project, file, 12, "Shared").expect("rename");
        assert!(edit.changes.unwrap_or_default().is_empty());
    }

    #[test]
    fn refuses_a_document_with_stale_spans() {
        let mut stale = shared();
        stale.text = Arc::from("{ \"name\": }");
        let project = project(vec![stale]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let error = rename(&project, file, 12, "Core").unwrap_err();
        assert!(error.contains("not valid JSON"), "{error}");
    }
}
