use std::collections::HashSet;
use std::path::Path;

use tower_lsp::lsp_types::{Location, Url};

use crate::context::LineIndex;
use crate::index::{IndexedFile, ProjectIndex};
use crate::parse::{FileKind, TextRange};
use crate::resolve;

/// One textual occurrence of a reference.
#[derive(Debug, Clone, Copy)]
pub struct ReferenceSite<'a> {
    pub file: &'a IndexedFile,
    pub range: TextRange,
    /// The reference text, used to distinguish name and GUID forms.
    pub value: &'a str,
}

/// All references to `targets` across the project.
pub fn references_to<'a>(
    project: &'a ProjectIndex,
    targets: &[&'a IndexedFile],
    include_declaration: bool,
) -> Vec<ReferenceSite<'a>> {
    if targets.is_empty() {
        return Vec::new();
    }

    let names: HashSet<&str> = targets.iter().filter_map(|file| file.name()).collect();
    let guids: Vec<&str> = targets
        .iter()
        .filter_map(|file| file.guid.as_deref())
        .collect();
    let paths: HashSet<&Path> = targets.iter().map(|file| file.path.as_path()).collect();

    let is_target = |value: &str| -> bool {
        if let Some(guid) = resolve::guid_reference(value) {
            guids
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(guid))
        } else {
            names.contains(value)
        }
    };

    let mut sites = Vec::new();
    for file in &project.files {
        if include_declaration
            && file.kind == FileKind::Asmdef
            && paths.contains(file.path.as_path())
        {
            if let Some(name) = file.asmdef.as_ref().and_then(|asmdef| asmdef.name.as_ref()) {
                sites.push(ReferenceSite {
                    file,
                    range: name.range,
                    value: &name.value,
                });
            }
        }

        match file.kind {
            FileKind::Asmdef => {
                for reference in file.references() {
                    if is_target(&reference.value) {
                        sites.push(ReferenceSite {
                            file,
                            range: reference.range,
                            value: &reference.value,
                        });
                    }
                }
            }
            FileKind::Asmref => {
                if let Some(reference) = file.asmref_reference() {
                    if is_target(&reference.value) {
                        sites.push(ReferenceSite {
                            file,
                            range: reference.range,
                            value: &reference.value,
                        });
                    }
                }
            }
        }
    }

    sites.sort_by(|left, right| {
        (left.file.path.as_path(), left.range.start, left.range.end).cmp(&(
            right.file.path.as_path(),
            right.range.start,
            right.range.end,
        ))
    });
    sites.dedup_by(|left, right| left.file.path == right.file.path && left.range == right.range);
    sites
}

/// LSP locations for the symbol at `offset`, using the client-facing root.
pub fn locations(
    project: &ProjectIndex,
    file: &IndexedFile,
    offset: usize,
    include_declaration: bool,
) -> Vec<Location> {
    let Some(symbol) = resolve::symbol_at(file, offset) else {
        return Vec::new();
    };
    let targets = resolve::targets(project, symbol);
    if targets.is_empty() {
        return Vec::new();
    }

    references_to(project, &targets, include_declaration)
        .into_iter()
        .filter_map(|site| {
            let display_path = project.display_root.join(&site.file.relative_path);
            let uri = Url::from_file_path(display_path).ok()?;
            let line_index = LineIndex::new(&site.file.data_text);
            Some(Location {
                uri,
                range: line_index.range(&site.file.data_text, site.range.start, site.range.end),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn game() -> IndexedFile {
        file(
            "Assets/Game.asmdef",
            r#"{ "name": "Game", "references": ["Shared", "GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"] }"#,
        )
    }

    fn shared() -> IndexedFile {
        indexed(
            "Assets/Shared.asmdef",
            r#"{ "name": "Shared" }"#,
            false,
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
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

    #[test]
    fn collects_name_and_guid_references() {
        let project = project(vec![game(), shared(), consumer()]);
        let shared = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let sites = references_to(&project, &[shared], false);
        assert_eq!(sites.len(), 3);
        assert!(sites
            .iter()
            .any(|site| site.file.relative_path == "Assets/Game.asmdef"));
        assert!(sites
            .iter()
            .any(|site| site.file.relative_path == "Assets/Consumer.asmdef"));
    }

    #[test]
    fn includes_asmref_references() {
        let project = project(vec![shared(), asmref()]);
        let shared = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let sites = references_to(&project, &[shared], false);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].file.relative_path, "Assets/Refs/Shared.asmref");
    }

    #[test]
    fn respects_include_declaration() {
        let project = project(vec![shared(), consumer()]);
        let shared = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");

        assert_eq!(references_to(&project, &[shared], false).len(), 1);
        let with_declaration = references_to(&project, &[shared], true);
        assert_eq!(with_declaration.len(), 2);
        assert!(with_declaration
            .iter()
            .any(|site| site.file.relative_path == "Assets/Shared.asmdef"));
    }

    #[test]
    fn collapses_duplicate_targets() {
        let project = project(vec![shared(), consumer()]);
        let shared = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let sites = references_to(&project, &[shared, shared], true);
        assert_eq!(sites.len(), 2);
    }

    #[test]
    fn does_not_match_other_guids_or_names() {
        let other = indexed(
            "Assets/Other.asmdef",
            r#"{ "name": "Other" }"#,
            false,
            Some("dddddddddddddddddddddddddddddddd"),
        );
        let project = project_with(
            vec![game(), shared(), other.clone()],
            Vec::new(),
            Vec::new(),
        );
        let sites = references_to(&project, &[&other], false);
        assert!(sites.is_empty());
    }

    #[test]
    fn finds_locations_through_a_reference() {
        let text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let project = project(vec![game(), shared()]);
        let game = project
            .file(&root().join("Assets/Game.asmdef"))
            .expect("game");
        let offset = offset_of(text, "Shared") + 1;
        let locations = locations(&project, game, offset, true);
        assert_eq!(locations.len(), 3);
        assert!(locations
            .iter()
            .any(|location| location.uri.as_str().ends_with("Shared.asmdef")));
        assert!(locations
            .iter()
            .any(|location| location.uri.as_str().ends_with("Game.asmdef")));
    }

    #[test]
    fn locations_use_the_client_facing_root() {
        let text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let mut project = project(vec![game(), shared()]);
        project.display_root = root().join("linked");
        let game = project
            .file(&root().join("Assets/Game.asmdef"))
            .expect("game");
        let offset = offset_of(text, "Shared") + 1;
        let locations = locations(&project, game, offset, false);
        assert_eq!(locations.len(), 2);
        assert!(locations
            .iter()
            .all(|location| location.uri.as_str().contains("/linked/")));
    }

    #[test]
    fn returns_nothing_outside_a_reference() {
        let project = project(vec![game()]);
        let game = project
            .file(&root().join("Assets/Game.asmdef"))
            .expect("game");
        assert!(locations(&project, game, 0, true).is_empty());
    }
}
