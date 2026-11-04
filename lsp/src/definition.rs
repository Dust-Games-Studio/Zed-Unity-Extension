use tower_lsp::lsp_types::{Location, Url};

use crate::context::LineIndex;
use crate::index::{IndexedFile, ProjectIndex};
use crate::parse::{FileKind, TextRange};

pub fn locations(project: &ProjectIndex, file: &IndexedFile, offset: usize) -> Vec<Location> {
    let reference = match file.kind {
        FileKind::Asmdef => file
            .references()
            .iter()
            .find(|reference| reference.range.contains(offset)),
        FileKind::Asmref => file
            .asmref_reference()
            .filter(|reference| reference.range.contains(offset)),
    };
    let Some(reference) = reference else {
        return Vec::new();
    };

    let targets = if let Some(guid) = reference.value.strip_prefix("GUID:") {
        project.find_by_guid(&guid.to_ascii_lowercase())
    } else {
        project.find_by_name(&reference.value)
    };

    targets
        .into_iter()
        .filter_map(|target| {
            let display_path = project.display_root.join(&target.relative_path);
            let uri = Url::from_file_path(&display_path).ok()?;
            let span = target.name_span().unwrap_or(TextRange::new(0, 0));
            let line_index = LineIndex::new(&target.data_text);
            Some(Location {
                uri,
                range: line_index.range(&target.data_text, span.start, span.end),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::index::ProjectIndex;
    use crate::parse;

    fn test_project_root() -> PathBuf {
        std::env::current_dir().unwrap().join("project")
    }

    fn relative_test_path(path: &str) -> &str {
        path.strip_prefix("/project/").unwrap_or(path)
    }

    fn test_path(path: &str) -> PathBuf {
        test_project_root().join(relative_test_path(path))
    }

    fn file(path: &str, text: &str) -> IndexedFile {
        let relative_path = relative_test_path(path);
        let path = test_project_root().join(relative_path);
        let parsed = parse::parse_file(&path, text);
        IndexedFile {
            kind: parsed.kind,
            path,
            relative_path: relative_path.to_string(),
            external: false,
            text: Arc::from(text),
            data_text: Arc::from(text),
            diagnostics: parsed.diagnostics,
            asmdef: parsed.asmdef,
            asmref: parsed.asmref,
            guid: None,
        }
    }

    fn project(files: Vec<IndexedFile>) -> ProjectIndex {
        ProjectIndex::new(test_project_root(), files, Vec::new(), Vec::new())
    }

    #[test]
    fn goes_to_a_reference_by_name() {
        let game_text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let project = project(vec![
            file("/project/Assets/Game.asmdef", game_text),
            file("/project/Assets/Shared.asmdef", r#"{ "name": "Shared" }"#),
        ]);
        let game = project
            .file(&test_path("/project/Assets/Game.asmdef"))
            .unwrap();
        let offset = game_text.find("Shared").unwrap() + 2;
        let locations = locations(&project, game, offset);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].uri.as_str().ends_with("Shared.asmdef"));
    }

    #[test]
    fn returns_all_matches_for_duplicate_names() {
        let game_text = r#"{ "name": "Game", "references": ["Duplicate"] }"#;
        let project = project(vec![
            file("/project/Assets/Game.asmdef", game_text),
            file("/project/Assets/A.asmdef", r#"{ "name": "Duplicate" }"#),
            file("/project/Assets/B.asmdef", r#"{ "name": "Duplicate" }"#),
        ]);
        let game = project
            .file(&test_path("/project/Assets/Game.asmdef"))
            .unwrap();
        let offset = game_text.find("Duplicate").unwrap() + 2;
        assert_eq!(locations(&project, game, offset).len(), 2);
    }

    #[test]
    fn definitions_use_the_display_root() {
        let game_text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let mut project = project(vec![
            file("/project/Assets/Game.asmdef", game_text),
            file("/project/Assets/Shared.asmdef", r#"{ "name": "Shared" }"#),
        ]);
        project.display_root = test_project_root().join("linked");
        let game = project
            .file(&test_path("/project/Assets/Game.asmdef"))
            .unwrap();
        let offset = game_text.find("Shared").unwrap() + 2;
        let locations = locations(&project, game, offset);
        assert_eq!(locations.len(), 1);
        assert!(
            locations[0].uri.as_str().contains("/linked/"),
            "definition URI must use the display root: {}",
            locations[0].uri
        );
    }

    #[test]
    fn goes_to_an_asmref_target() {
        let ref_text = r#"{ "reference": "Shared" }"#;
        let project = project(vec![
            file("/project/Assets/Ref.asmref", ref_text),
            file("/project/Assets/Shared.asmdef", r#"{ "name": "Shared" }"#),
        ]);
        let reference = project
            .file(&test_path("/project/Assets/Ref.asmref"))
            .unwrap();
        let offset = ref_text.find("Shared").unwrap() + 2;
        let locations = locations(&project, reference, offset);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].uri.as_str().ends_with("Shared.asmdef"));
    }
}
