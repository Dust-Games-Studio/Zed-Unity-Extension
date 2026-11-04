use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Url};

use crate::context::LineIndex;
use crate::index::{IndexedFile, ProjectIndex, WorkspaceIndex};
use crate::parse::{FileKind, RawDiagnostic};

pub fn compute(index: &WorkspaceIndex) -> HashMap<Url, Vec<Diagnostic>> {
    let mut result = HashMap::new();

    for project in &index.projects {
        let raw = project_diagnostics(project);
        for (path, raw_diagnostics) in raw {
            if raw_diagnostics.is_empty() {
                continue;
            }
            let Some(file) = project.file(&path) else {
                continue;
            };
            let Ok(url) = Url::from_file_path(&path) else {
                continue;
            };
            result.insert(
                url,
                raw_diagnostics
                    .into_iter()
                    .map(|diagnostic| to_lsp(file, diagnostic))
                    .collect(),
            );
        }
    }

    result
}

fn project_diagnostics(project: &ProjectIndex) -> HashMap<PathBuf, Vec<RawDiagnostic>> {
    let mut diagnostics: HashMap<PathBuf, Vec<RawDiagnostic>> = project
        .project_files()
        .map(|file| (file.path.clone(), file.diagnostics.clone()))
        .collect();

    let mut all_by_name: HashMap<String, Vec<&IndexedFile>> = HashMap::new();
    let mut all_by_guid: HashMap<String, Vec<&IndexedFile>> = HashMap::new();
    for file in project.assemblies() {
        if let Some(name) = file.name() {
            all_by_name.entry(name.to_string()).or_default().push(file);
        }
        if let Some(guid) = &file.guid {
            all_by_guid
                .entry(guid.to_ascii_lowercase())
                .or_default()
                .push(file);
        }
    }

    for file in project.project_files() {
        let Some(entry) = diagnostics.get_mut(&file.path) else {
            continue;
        };
        match file.kind {
            FileKind::Asmdef => {
                for reference in file.references() {
                    if resolve(&reference.value, &all_by_name, &all_by_guid).is_empty() {
                        entry.push(RawDiagnostic::error(
                            reference.range,
                            "asmdef.unresolved-reference",
                            format!(
                                "Reference `{}` does not match any assembly definition in this project or its packages.",
                                reference.value
                            ),
                        ));
                    }
                }
            }
            FileKind::Asmref => {
                if let Some(reference) = file.asmref_reference() {
                    if !reference.value.is_empty()
                        && resolve(&reference.value, &all_by_name, &all_by_guid).is_empty()
                    {
                        entry.push(RawDiagnostic::error(
                            reference.range,
                            "asmref.unresolved-reference",
                            format!(
                                "Assembly reference `{}` does not match any assembly definition in this project or its packages.",
                                reference.value
                            ),
                        ));
                    }
                }
            }
        }
    }

    add_duplicate_diagnostics(project, &mut diagnostics);
    add_cycle_diagnostics(project, &mut diagnostics);
    diagnostics
}

fn resolve<'a>(
    reference: &str,
    by_name: &'a HashMap<String, Vec<&'a IndexedFile>>,
    by_guid: &'a HashMap<String, Vec<&'a IndexedFile>>,
) -> Vec<&'a IndexedFile> {
    if let Some(guid) = reference.strip_prefix("GUID:") {
        by_guid
            .get(&guid.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    } else {
        by_name.get(reference).cloned().unwrap_or_default()
    }
}

fn add_duplicate_diagnostics(
    project: &ProjectIndex,
    diagnostics: &mut HashMap<PathBuf, Vec<RawDiagnostic>>,
) {
    let mut by_name: HashMap<&str, Vec<&IndexedFile>> = HashMap::new();
    for file in project
        .project_files()
        .filter(|file| file.kind == FileKind::Asmdef)
    {
        if let Some(name) = file.name() {
            by_name.entry(name).or_default().push(file);
        }
    }

    for (name, files) in by_name {
        if files.len() < 2 {
            continue;
        }
        for file in files {
            let Some(range) = file.name_span() else {
                continue;
            };
            if let Some(entry) = diagnostics.get_mut(&file.path) {
                entry.push(RawDiagnostic::error(
                    range,
                    "asmdef.duplicate-name",
                    format!("Assembly name `{name}` is used by multiple assembly definitions."),
                ));
            }
        }
    }
}

fn add_cycle_diagnostics(
    project: &ProjectIndex,
    diagnostics: &mut HashMap<PathBuf, Vec<RawDiagnostic>>,
) {
    let nodes: Vec<&IndexedFile> = project
        .project_files()
        .filter(|file| file.kind == FileKind::Asmdef && file.name().is_some())
        .collect();
    if nodes.is_empty() {
        return;
    }

    let mut by_path = HashMap::<&Path, usize>::new();
    let mut by_name = HashMap::<String, Vec<usize>>::new();
    let mut by_guid = HashMap::<String, Vec<usize>>::new();
    for (index, file) in nodes.iter().enumerate() {
        by_path.insert(file.path.as_path(), index);
        if let Some(name) = file.name() {
            by_name.entry(name.to_string()).or_default().push(index);
        }
        if let Some(guid) = &file.guid {
            by_guid
                .entry(guid.to_ascii_lowercase())
                .or_default()
                .push(index);
        }
    }

    let adjacency: Vec<Vec<usize>> = nodes
        .iter()
        .map(|file| {
            let mut targets = HashSet::new();
            for reference in file.references() {
                if let Some(guid) = reference.value.strip_prefix("GUID:") {
                    if let Some(indices) = by_guid.get(&guid.to_ascii_lowercase()) {
                        targets.extend(indices.iter().copied());
                    }
                } else if let Some(indices) = by_name.get(&reference.value) {
                    targets.extend(indices.iter().copied());
                }
            }
            let mut targets: Vec<_> = targets.into_iter().collect();
            targets.sort_unstable();
            targets
        })
        .collect();

    for component in strongly_connected_components(&adjacency) {
        let self_loop = component.len() == 1 && adjacency[component[0]].contains(&component[0]);
        if component.len() < 2 && !self_loop {
            continue;
        }

        let mut names: Vec<String> = component
            .iter()
            .filter_map(|index| nodes[*index].name().map(str::to_string))
            .collect();
        names.sort();
        if names.is_empty() {
            continue;
        }
        let message = format!(
            "Assembly reference cycle: {} -> {}.",
            names.join(" -> "),
            names[0]
        );

        for index in &component {
            let file = nodes[*index];
            let Some(range) = file.name_span() else {
                continue;
            };
            if let Some(entry) = diagnostics.get_mut(&file.path) {
                entry.push(RawDiagnostic::error(
                    range,
                    "asmdef.reference-cycle",
                    message.clone(),
                ));
            }
        }
    }
}

fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct Tarjan<'a> {
        adjacency: &'a [Vec<usize>],
        next_index: usize,
        indices: Vec<Option<usize>>,
        low_links: Vec<usize>,
        stack: Vec<usize>,
        on_stack: Vec<bool>,
        components: Vec<Vec<usize>>,
    }

    impl Tarjan<'_> {
        fn visit(&mut self, node: usize) {
            let index = self.next_index;
            self.next_index += 1;
            self.indices[node] = Some(index);
            self.low_links[node] = index;
            self.stack.push(node);
            self.on_stack[node] = true;

            for &next in &self.adjacency[node] {
                if self.indices[next].is_none() {
                    self.visit(next);
                    self.low_links[node] = self.low_links[node].min(self.low_links[next]);
                } else if self.on_stack[next] {
                    self.low_links[node] =
                        self.low_links[node].min(self.indices[next].unwrap_or(index));
                }
            }

            if self.low_links[node] == index {
                let mut component = Vec::new();
                loop {
                    let member = self.stack.pop().expect("tarjan stack is non-empty");
                    self.on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                self.components.push(component);
            }
        }
    }

    let mut tarjan = Tarjan {
        adjacency,
        next_index: 0,
        indices: vec![None; adjacency.len()],
        low_links: vec![0; adjacency.len()],
        stack: Vec::new(),
        on_stack: vec![false; adjacency.len()],
        components: Vec::new(),
    };

    for node in 0..adjacency.len() {
        if tarjan.indices[node].is_none() {
            tarjan.visit(node);
        }
    }
    tarjan.components
}

fn to_lsp(file: &IndexedFile, raw: RawDiagnostic) -> Diagnostic {
    let line_index = LineIndex::new(&file.text);
    Diagnostic {
        range: line_index.range(&file.text, raw.range.start, raw.range.end),
        severity: Some(match raw.severity {
            DiagnosticSeverity::ERROR => DiagnosticSeverity::ERROR,
            DiagnosticSeverity::WARNING => DiagnosticSeverity::WARNING,
            _ => DiagnosticSeverity::INFORMATION,
        }),
        code: Some(NumberOrString::String(raw.code.to_string())),
        source: Some("unity-asmdef".to_string()),
        message: raw.message,
        ..Diagnostic::default()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::index::{ProjectIndex, WorkspaceIndex};
    use crate::parse;

    fn file(path: &str, text: &str) -> IndexedFile {
        let parsed = parse::parse_file(Path::new(path), text);
        IndexedFile {
            kind: parsed.kind,
            path: PathBuf::from(path),
            relative_path: path.to_string(),
            external: false,
            text: Arc::from(text),
            data_text: Arc::from(text),
            diagnostics: parsed.diagnostics,
            asmdef: parsed.asmdef,
            asmref: parsed.asmref,
            guid: None,
        }
    }

    fn external(path: &str, text: &str) -> IndexedFile {
        let mut file = file(path, text);
        file.external = true;
        file
    }

    fn workspace(files: Vec<IndexedFile>) -> WorkspaceIndex {
        WorkspaceIndex {
            projects: vec![ProjectIndex::new(
                PathBuf::from("/project"),
                files,
                Vec::new(),
                Vec::new(),
            )],
        }
    }

    fn codes(index: &WorkspaceIndex, path: &str) -> Vec<String> {
        let url = Url::from_file_path(Path::new(path)).unwrap();
        compute(index)
            .get(&url)
            .map(|diagnostics| {
                diagnostics
                    .iter()
                    .filter_map(|diagnostic| match &diagnostic.code {
                        Some(NumberOrString::String(code)) => Some(code.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn reports_unresolved_name_and_guid_references() {
        let index = workspace(vec![file(
            "/project/Assets/Game.asmdef",
            r#"{ "name": "Game", "references": ["Missing", "GUID:ffffffffffffffffffffffffffffffff"] }"#,
        )]);
        let codes = codes(&index, "/project/Assets/Game.asmdef");
        assert_eq!(
            codes
                .iter()
                .filter(|code| *code == "asmdef.unresolved-reference")
                .count(),
            2
        );
    }

    #[test]
    fn resolves_project_and_external_references() {
        let index = workspace(vec![
            file(
                "/project/Assets/Game.asmdef",
                r#"{ "name": "Game", "references": ["Shared", "Unity.InputSystem"] }"#,
            ),
            file("/project/Assets/Shared.asmdef", r#"{ "name": "Shared" }"#),
            external(
                "/project/Library/PackageCache/com.unity.inputsystem@abc/Unity.InputSystem.asmdef",
                r#"{ "name": "Unity.InputSystem" }"#,
            ),
        ]);
        assert!(codes(&index, "/project/Assets/Game.asmdef").is_empty());
    }

    #[test]
    fn reports_duplicate_names_in_project_assignments_only() {
        let index = workspace(vec![
            file("/project/Assets/A.asmdef", r#"{ "name": "Duplicate" }"#),
            file("/project/Assets/B.asmdef", r#"{ "name": "Duplicate" }"#),
            external(
                "/project/Assets/Package.asmdef",
                r#"{ "name": "Duplicate" }"#,
            ),
        ]);
        assert!(codes(&index, "/project/Assets/A.asmdef")
            .contains(&"asmdef.duplicate-name".to_string()));
        assert!(codes(&index, "/project/Assets/B.asmdef")
            .contains(&"asmdef.duplicate-name".to_string()));
        assert!(codes(&index, "/project/Assets/Package.asmdef").is_empty());
    }

    #[test]
    fn reports_reference_cycles() {
        let index = workspace(vec![
            file(
                "/project/Assets/A.asmdef",
                r#"{ "name": "A", "references": ["B"] }"#,
            ),
            file(
                "/project/Assets/B.asmdef",
                r#"{ "name": "B", "references": ["A"] }"#,
            ),
        ]);
        let a = codes(&index, "/project/Assets/A.asmdef");
        let b = codes(&index, "/project/Assets/B.asmdef");
        assert!(a.contains(&"asmdef.reference-cycle".to_string()));
        assert!(b.contains(&"asmdef.reference-cycle".to_string()));
    }

    #[test]
    fn reports_missing_asmref_targets() {
        let index = workspace(vec![
            file("/project/Assets/Game.asmdef", r#"{ "name": "Game" }"#),
            file(
                "/project/Assets/Ref.asmref",
                r#"{ "reference": "Missing" }"#,
            ),
        ]);
        assert_eq!(
            codes(&index, "/project/Assets/Ref.asmref"),
            vec!["asmref.unresolved-reference".to_string()]
        );
    }

    #[test]
    fn keeps_parse_and_schema_diagnostics() {
        let index = workspace(vec![
            file("/project/Assets/Broken.asmdef", r#"{ "name": }"#),
            file("/project/Assets/NoName.asmdef", r#"{ "references": [] }"#),
        ]);
        assert!(codes(&index, "/project/Assets/Broken.asmdef")
            .contains(&"asmdef.parse-error".to_string()));
        assert!(codes(&index, "/project/Assets/NoName.asmdef")
            .contains(&"asmdef.missing-name".to_string()));
    }

    #[test]
    fn internal_package_diagnostics_are_suppressed() {
        let index = workspace(vec![external(
            "/project/Library/PackageCache/pkg@abc/Package.asmdef",
            r#"{ "name": }"#,
        )]);
        assert!(codes(
            &index,
            "/project/Library/PackageCache/pkg@abc/Package.asmdef"
        )
        .is_empty());
    }
}
