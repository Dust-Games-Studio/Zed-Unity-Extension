use std::collections::HashSet;
use std::path::Path;

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind};

use crate::context::LineIndex;
use crate::index::{IndexedFile, PackageInfo, ProjectIndex};
use crate::references;
use crate::resolve;

/// Hover information for the assembly declaration or reference at `offset`.
pub fn hover(project: &ProjectIndex, file: &IndexedFile, offset: usize) -> Option<Hover> {
    let symbol = resolve::symbol_at(file, offset)?;
    let line_index = LineIndex::new(&symbol.file.data_text);
    let range = line_index.range(&symbol.file.data_text, symbol.range.start, symbol.range.end);

    let targets = resolve::targets(project, symbol);
    let value = if targets.is_empty() {
        unresolved_markdown(project, symbol)
    } else {
        targets
            .iter()
            .map(|target| assembly_markdown(project, target))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        range: Some(range),
    })
}

fn assembly_markdown(project: &ProjectIndex, target: &IndexedFile) -> String {
    let name = target.name().unwrap_or("<unnamed>");
    let mut lines = vec![
        format!("**{name}**"),
        String::new(),
        "Assembly definition.".to_string(),
        String::new(),
        format!("- Path: `{}`", target.relative_path),
    ];

    match &target.guid {
        Some(guid) => lines.push(format!("- GUID: `{guid}`")),
        None => lines.push("- GUID: none (no `.meta` file)".to_string()),
    }
    if let Some(namespace) = target.root_namespace() {
        lines.push(format!("- Root namespace: `{namespace}`"));
    }
    lines.push(format!("- Origin: {}", origin(project, target)));

    let sites = references::references_to(project, &[target], false);
    let files: HashSet<&Path> = sites.iter().map(|site| site.file.path.as_path()).collect();
    lines.push(format!(
        "- Referenced by: {} reference(s) in {} file(s)",
        sites.len(),
        files.len()
    ));

    lines.join("\n")
}

fn origin(project: &ProjectIndex, file: &IndexedFile) -> String {
    if !file.external {
        return "Project".to_string();
    }

    let Some(package) = package_for(project, file) else {
        return "Package (not listed in the manifest)".to_string();
    };

    let mut origin = format!("Package `{}`", package.name);
    if let Some(version) = &package.version {
        origin.push_str(&format!(" {version}"));
    }
    if let Some(source) = &package.source {
        origin.push_str(&format!(" ({source})"));
    }
    origin
}

fn package_for<'a>(project: &'a ProjectIndex, file: &IndexedFile) -> Option<&'a PackageInfo> {
    let relative = file.relative_path.as_str();
    let folder = if let Some(rest) = relative.strip_prefix("Packages/") {
        rest.split('/').next()?
    } else {
        let rest = relative.strip_prefix("Library/PackageCache/")?;
        rest.split('/').next()?.split('@').next()?
    };

    project
        .packages
        .iter()
        .find(|package| package.name == folder)
}

fn unresolved_markdown(project: &ProjectIndex, symbol: resolve::Symbol<'_>) -> String {
    let mut lines = vec![
        format!("**`{}`** — unresolved assembly reference", symbol.value),
        String::new(),
    ];

    if let Some(guid) = resolve::guid_reference(symbol.value) {
        lines.push(format!(
            "No assembly definition with GUID `{guid}` was found in the project or its packages."
        ));
    } else {
        lines.push(format!(
            "No assembly definition named `{}` was found in the project or its packages.",
            symbol.value
        ));
    }

    let suggestions = resolve::nearest_assemblies(project, symbol.value, 3);
    if !suggestions.is_empty() {
        lines.push(String::new());
        lines.push("Did you mean:".to_string());
        for suggestion in suggestions {
            lines.push(format!("- `{}`", suggestion.name().unwrap_or_default()));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn shared() -> IndexedFile {
        indexed(
            "Assets/Shared.asmdef",
            r#"{ "name": "Shared", "rootNamespace": "Demo" }"#,
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

    fn text_of(hover: &Hover) -> &str {
        match &hover.contents {
            HoverContents::Markup(MarkupContent { value, .. }) => value,
            other => panic!("unexpected hover contents: {other:?}"),
        }
    }

    fn hover_text(project: &ProjectIndex, file: &IndexedFile, offset: usize) -> String {
        let hover = hover(project, file, offset).expect("hover");
        text_of(&hover).to_string()
    }

    #[test]
    fn shows_assembly_details_and_reference_count() {
        let project = project(vec![shared(), consumer(), asmref()]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        let offset = offset_of(file.text.as_ref(), "Shared") + 1;
        let hover = hover(&project, file, offset).expect("hover");
        let text = text_of(&hover);

        assert!(text.contains("**Shared**"), "{text}");
        assert!(text.contains("Assets/Shared.asmdef"), "{text}");
        assert!(text.contains("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"), "{text}");
        assert!(text.contains("Project"), "{text}");
        assert!(text.contains("Demo"), "{text}");
        assert!(
            text.contains("Referenced by: 2 reference(s) in 2 file(s)"),
            "{text}"
        );
    }

    #[test]
    fn shows_package_origin() {
        let project = project_with(
            vec![
                shared(),
                indexed(
                    "Library/PackageCache/com.unity.inputsystem@1.20.1/InputSystem/Unity.InputSystem.asmdef",
                    r#"{ "name": "Unity.InputSystem" }"#,
                    true,
                    Some("cccccccccccccccccccccccccccccccc"),
                ),
            ],
            Vec::new(),
            vec![package(
                "com.unity.inputsystem",
                Some("1.20.1"),
                Some("registry"),
            )],
        );
        let file = project
            .file(
                &root().join(
                    "Library/PackageCache/com.unity.inputsystem@1.20.1/InputSystem/Unity.InputSystem.asmdef",
                ),
            )
            .expect("input system");
        let offset = offset_of(file.text.as_ref(), "Unity.InputSystem") + 1;
        let text = hover_text(&project, file, offset);

        assert!(text.contains("Package `com.unity.inputsystem`"), "{text}");
        assert!(text.contains("1.20.1"), "{text}");
        assert!(text.contains("registry"), "{text}");
    }

    #[test]
    fn shows_the_target_for_a_reference() {
        let project = project(vec![shared(), consumer()]);
        let file = project
            .file(&root().join("Assets/Consumer.asmdef"))
            .expect("consumer");
        let offset = offset_of(file.text.as_ref(), "Shared") + 1;
        let text = hover_text(&project, file, offset);
        assert!(text.contains("**Shared**"), "{text}");
        assert!(text.contains("Assets/Shared.asmdef"), "{text}");
    }

    #[test]
    fn hovers_guid_references_and_asmrefs() {
        let project = project(vec![shared(), game(), asmref()]);
        let game = project
            .file(&root().join("Assets/Game.asmdef"))
            .expect("game");
        let offset = offset_of(game.text.as_ref(), "GUID:") + 2;
        let text = hover_text(&project, game, offset);
        assert!(text.contains("**Shared**"), "{text}");

        let asmref = project
            .file(&root().join("Assets/Refs/Shared.asmref"))
            .expect("asmref");
        let offset = offset_of(asmref.text.as_ref(), "Shared") + 1;
        let text = hover_text(&project, asmref, offset);
        assert!(text.contains("**Shared**"), "{text}");
    }

    #[test]
    fn suggests_names_for_unresolved_references() {
        let unresolved = file(
            "Assets/Broken.asmdef",
            r#"{ "name": "Broken", "references": ["Shard"] }"#,
        );
        let project = project(vec![shared(), unresolved]);
        let file = project
            .file(&root().join("Assets/Broken.asmdef"))
            .expect("broken");
        let offset = offset_of(file.text.as_ref(), "Shard") + 1;
        let text = hover_text(&project, file, offset);

        assert!(text.contains("unresolved assembly reference"), "{text}");
        assert!(text.contains("Shard"), "{text}");
        assert!(text.contains("Shared"), "{text}");
    }

    #[test]
    fn reports_a_missing_guid() {
        let project = project(vec![file(
            "Assets/NoGuid.asmdef",
            r#"{ "name": "NoGuid" }"#,
        )]);
        let file = project
            .file(&root().join("Assets/NoGuid.asmdef"))
            .expect("no guid");
        let offset = offset_of(file.text.as_ref(), "NoGuid") + 1;
        let text = hover_text(&project, file, offset);
        assert!(text.contains("no `.meta` file"), "{text}");
    }

    #[test]
    fn returns_none_outside_symbols() {
        let project = project(vec![shared()]);
        let file = project
            .file(&root().join("Assets/Shared.asmdef"))
            .expect("shared");
        assert!(hover(&project, file, 0).is_none());
    }
}
