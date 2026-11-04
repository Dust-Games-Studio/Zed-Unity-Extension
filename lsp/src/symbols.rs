use tower_lsp::lsp_types::{Location, SymbolInformation, SymbolKind, Url};

use crate::context::LineIndex;
use crate::index::WorkspaceIndex;

/// Upper bound on one `workspace/symbol` response.
pub const LIMIT: usize = 1000;

/// Assembly symbols matching `query`, project assemblies before packages.
#[allow(deprecated)]
pub fn workspace_symbols(index: &WorkspaceIndex, query: &str) -> Vec<SymbolInformation> {
    let needle = query.trim().to_ascii_lowercase();
    let mut matches: Vec<(bool, String, String, SymbolInformation)> = Vec::new();

    for project in &index.projects {
        for file in project.assemblies() {
            let Some(name) = file.name() else {
                continue;
            };
            if !needle.is_empty()
                && !name.to_ascii_lowercase().contains(&needle)
                && !file.relative_path.to_ascii_lowercase().contains(&needle)
            {
                continue;
            }

            let display_path = project.display_root.join(&file.relative_path);
            let Ok(uri) = Url::from_file_path(display_path) else {
                continue;
            };
            let line_index = LineIndex::new(&file.data_text);
            let range = file
                .name_span()
                .map(|span| line_index.range(&file.data_text, span.start, span.end))
                .unwrap_or_default();
            let container = if file.external {
                "Unity packages"
            } else {
                "Unity project assemblies"
            };

            matches.push((
                file.external,
                name.to_string(),
                file.relative_path.clone(),
                SymbolInformation {
                    name: name.to_string(),
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    location: Location { uri, range },
                    container_name: Some(container.to_string()),
                },
            ));
        }
    }

    matches.sort_by(|left, right| {
        (left.0, left.1.as_str(), left.2.as_str()).cmp(&(
            right.0,
            right.1.as_str(),
            right.2.as_str(),
        ))
    });
    matches
        .into_iter()
        .take(LIMIT)
        .map(|(_, _, _, symbol)| symbol)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn shared() -> crate::index::IndexedFile {
        indexed(
            "Assets/Shared.asmdef",
            r#"{ "name": "Shared" }"#,
            false,
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        )
    }

    fn input_system() -> crate::index::IndexedFile {
        indexed(
            "Library/PackageCache/com.unity.inputsystem@abc/Unity.InputSystem.asmdef",
            r#"{ "name": "Unity.InputSystem" }"#,
            true,
            None,
        )
    }

    fn workspace(files: Vec<crate::index::IndexedFile>) -> WorkspaceIndex {
        WorkspaceIndex {
            projects: vec![project(files)],
        }
    }

    #[test]
    fn returns_project_and_package_assemblies() {
        let index = workspace(vec![shared(), input_system()]);
        let symbols = workspace_symbols(&index, "");
        assert_eq!(symbols.len(), 2);
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "Shared" && symbol.kind == SymbolKind::MODULE));
        assert!(symbols.iter().any(|symbol| {
            symbol.name == "Unity.InputSystem"
                && symbol.container_name.as_deref() == Some("Unity packages")
        }));
        assert!(symbols.iter().any(|symbol| {
            symbol.container_name.as_deref() == Some("Unity project assemblies")
        }));
        assert!(symbols
            .iter()
            .all(|symbol| symbol.location.uri.as_str().ends_with(".asmdef")));
    }

    #[test]
    fn filters_names_case_insensitively() {
        let index = workspace(vec![shared(), input_system()]);
        let symbols = workspace_symbols(&index, "SHA");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Shared");

        let symbols = workspace_symbols(&index, "inputs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Unity.InputSystem");
    }

    #[test]
    fn returns_empty_for_no_match() {
        let index = workspace(vec![shared()]);
        assert!(workspace_symbols(&index, "Missing").is_empty());
    }

    #[test]
    fn skips_assemblies_without_names() {
        let index = workspace(vec![
            shared(),
            file("Assets/Broken.asmdef", r#"{ "references": [] }"#),
        ]);
        let symbols = workspace_symbols(&index, "");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Shared");
    }

    #[test]
    fn sorts_project_assemblies_before_packages() {
        let index = workspace(vec![
            input_system(),
            indexed("Assets/Alpha.asmdef", r#"{ "name": "Alpha" }"#, false, None),
        ]);
        let symbols = workspace_symbols(&index, "");
        assert_eq!(symbols[0].name, "Alpha");
        assert_eq!(symbols[1].name, "Unity.InputSystem");
    }

    #[test]
    fn uses_the_client_facing_root() {
        let mut project = project(vec![shared()]);
        project.display_root = root().join("linked");
        let index = WorkspaceIndex {
            projects: vec![project],
        };
        let symbols = workspace_symbols(&index, "Shared");
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].location.uri.as_str().contains("/linked/"));
    }
}
