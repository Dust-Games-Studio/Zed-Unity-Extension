use std::collections::HashSet;
use std::path::Path;

use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionTextEdit, Documentation, MarkupContent,
    MarkupKind, TextEdit,
};

use crate::context::{self, CompletionContext, CompletionKind, LineIndex};
use crate::index::{IndexedFile, ProjectIndex};
use crate::parse::FileKind;
use crate::platforms;

const ASMDEF_KEYS: &[(&str, &str)] = &[
    ("name", "Assembly name"),
    ("rootNamespace", "Default namespace for scripts"),
    ("references", "Assembly definition references"),
    ("includePlatforms", "Platforms that include this assembly"),
    ("excludePlatforms", "Platforms that exclude this assembly"),
    ("allowUnsafeCode", "Allow unsafe C# code"),
    (
        "overrideReferences",
        "Override automatic precompiled references",
    ),
    ("precompiledReferences", "Precompiled DLL references"),
    (
        "autoReferenced",
        "Automatically referenced by predefined assemblies",
    ),
    (
        "defineConstraints",
        "Preprocessor symbols required to compile",
    ),
    ("versionDefines", "Package-version-based defines"),
    (
        "noEngineReferences",
        "Do not reference UnityEngine or UnityEditor",
    ),
];

const ASMREF_KEYS: &[(&str, &str)] = &[("reference", "Assembly to reference")];

const VERSION_DEFINE_KEYS: &[(&str, &str)] = &[
    ("name", "Package or module name"),
    ("expression", "Version expression"),
    ("define", "Symbol to define"),
];

pub fn completions(
    project: &ProjectIndex,
    file_path: &Path,
    text: &str,
    offset: usize,
) -> Vec<CompletionItem> {
    let Some(context) = context::completion_context(text, offset) else {
        return Vec::new();
    };
    let line_index = LineIndex::new(text);
    let file_kind = if file_path
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("asmref")
    {
        FileKind::Asmref
    } else {
        FileKind::Asmdef
    };

    match context.kind {
        CompletionKind::References => {
            assembly_items(project, file_path, &context, text, &line_index)
        }
        CompletionKind::IncludePlatforms | CompletionKind::ExcludePlatforms => {
            platform_items(&context, text, &line_index)
        }
        CompletionKind::VersionDefineName => package_items(project, &context, text, &line_index),
        CompletionKind::PrecompiledReferences => dll_items(project, &context, text, &line_index),
        CompletionKind::TopLevelKey => {
            let keys = if file_kind == FileKind::Asmref {
                ASMREF_KEYS
            } else {
                ASMDEF_KEYS
            };
            key_items(keys, &context, text, &line_index)
        }
        CompletionKind::VersionDefineKey => {
            key_items(VERSION_DEFINE_KEYS, &context, text, &line_index)
        }
    }
}

fn assembly_items(
    project: &ProjectIndex,
    current_path: &Path,
    context: &CompletionContext,
    text: &str,
    line_index: &LineIndex,
) -> Vec<CompletionItem> {
    let guid_mode = context
        .prefix
        .trim_start()
        .to_ascii_uppercase()
        .starts_with("GUID:");
    let include_guid_items = guid_mode || context.prefix.is_empty();

    let mut assemblies: Vec<&IndexedFile> = project
        .assemblies()
        .filter(|file| file.path != current_path && file.name().is_some())
        .collect();
    assemblies.sort_by(|left, right| {
        (left.external, left.name().unwrap_or_default())
            .cmp(&(right.external, right.name().unwrap_or_default()))
    });

    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for file in assemblies {
        let name = file.name().unwrap_or_default();
        if !seen.insert(name.to_string()) {
            continue;
        }

        let detail = if file.external {
            format!("Package assembly · {}", file.relative_path)
        } else {
            format!("Assembly · {}", file.relative_path)
        };
        let documentation = assembly_documentation(file);
        let sort_group = if file.external { "1" } else { "0" };

        if guid_mode {
            if let Some(guid) = &file.guid {
                items.push(make_item(
                    context,
                    text,
                    line_index,
                    format!("GUID:{guid}"),
                    CompletionItemKind::REFERENCE,
                    Some(format!("{name} · {}", file.relative_path)),
                    Some(format!("GUID:{guid}")),
                    Some(format!("2_{name}")),
                    documentation,
                ));
            }
            continue;
        }

        items.push(make_item(
            context,
            text,
            line_index,
            name.to_string(),
            CompletionItemKind::REFERENCE,
            Some(detail.clone()),
            Some(name.to_string()),
            Some(format!("{sort_group}_{name}")),
            documentation.clone(),
        ));

        if include_guid_items {
            if let Some(guid) = &file.guid {
                items.push(make_item(
                    context,
                    text,
                    line_index,
                    format!("GUID:{guid}"),
                    CompletionItemKind::REFERENCE,
                    Some(format!("{name} · {}", file.relative_path)),
                    Some(name.to_string()),
                    Some(format!("2_{name}")),
                    documentation,
                ));
            }
        }
    }

    items
}

fn platform_items(
    context: &CompletionContext,
    text: &str,
    line_index: &LineIndex,
) -> Vec<CompletionItem> {
    platforms::PLATFORMS
        .iter()
        .map(|platform| {
            make_item(
                context,
                text,
                line_index,
                (*platform).to_string(),
                CompletionItemKind::ENUM_MEMBER,
                Some("Unity assembly platform".to_string()),
                Some((*platform).to_string()),
                Some(format!("0_{platform}")),
                None,
            )
        })
        .collect()
}

fn package_items(
    project: &ProjectIndex,
    context: &CompletionContext,
    text: &str,
    line_index: &LineIndex,
) -> Vec<CompletionItem> {
    project
        .packages
        .iter()
        .map(|package| {
            let detail = match (&package.version, &package.source) {
                (Some(version), Some(source)) => format!("{version} · {source}"),
                (Some(version), None) => version.clone(),
                _ => "Unity package".to_string(),
            };
            make_item(
                context,
                text,
                line_index,
                package.name.clone(),
                CompletionItemKind::MODULE,
                Some(detail),
                Some(package.name.clone()),
                Some(format!("0_{}", package.name)),
                None,
            )
        })
        .collect()
}

fn dll_items(
    project: &ProjectIndex,
    context: &CompletionContext,
    text: &str,
    line_index: &LineIndex,
) -> Vec<CompletionItem> {
    let mut dlls: Vec<_> = project.dlls.iter().collect();
    dlls.sort_by(|left, right| {
        (left.external, left.name.as_str()).cmp(&(right.external, right.name.as_str()))
    });

    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for dll in dlls {
        if !seen.insert(dll.name.clone()) {
            continue;
        }
        items.push(make_item(
            context,
            text,
            line_index,
            dll.name.clone(),
            CompletionItemKind::FILE,
            Some(dll.relative_path.clone()),
            Some(dll.name.clone()),
            Some(format!("0_{}", dll.name)),
            None,
        ));
    }
    items
}

fn key_items(
    keys: &[(&str, &str)],
    context: &CompletionContext,
    text: &str,
    line_index: &LineIndex,
) -> Vec<CompletionItem> {
    keys.iter()
        .map(|(key, description)| {
            make_item(
                context,
                text,
                line_index,
                (*key).to_string(),
                CompletionItemKind::PROPERTY,
                Some((*description).to_string()),
                Some((*key).to_string()),
                Some(format!("0_{key}")),
                None,
            )
        })
        .collect()
}

fn assembly_documentation(file: &IndexedFile) -> Option<Documentation> {
    let name = file.name()?;
    let mut lines = vec![format!("**{name}**")];
    if let Some(namespace) = file.root_namespace() {
        lines.push(format!("Root namespace: `{namespace}`"));
    }
    if let Some(guid) = &file.guid {
        lines.push(format!("GUID: `{guid}`"));
    }
    lines.push(format!("Path: `{}`", file.relative_path));
    Some(Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: lines.join("\n\n"),
    }))
}

#[allow(clippy::too_many_arguments)]
fn make_item(
    context: &CompletionContext,
    text: &str,
    line_index: &LineIndex,
    label: String,
    kind: CompletionItemKind,
    detail: Option<String>,
    filter_text: Option<String>,
    sort_text: Option<String>,
    documentation: Option<Documentation>,
) -> CompletionItem {
    let new_text = if context.in_string {
        label.clone()
    } else {
        serde_json::to_string(&label).unwrap_or_else(|_| format!("\"{label}\""))
    };
    CompletionItem {
        label,
        kind: Some(kind),
        detail,
        documentation,
        filter_text,
        sort_text,
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: line_index.range(text, context.replace.start, context.replace.end),
            new_text,
        })),
        ..CompletionItem::default()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::index::{DllFile, PackageInfo};
    use crate::parse;

    fn file(path: &str, name: &str, guid: Option<&str>, external: bool) -> IndexedFile {
        let text = format!(r#"{{ "name": "{name}" }}"#);
        let parsed = parse::parse_file(Path::new("x.asmdef"), &text);
        IndexedFile {
            kind: FileKind::Asmdef,
            path: PathBuf::from(path),
            relative_path: path.to_string(),
            external,
            text: Arc::from(text),
            diagnostics: parsed.diagnostics,
            asmdef: parsed.asmdef,
            asmref: None,
            guid: guid.map(str::to_string),
        }
    }

    fn project() -> ProjectIndex {
        ProjectIndex::new(
            PathBuf::from("/project"),
            vec![
                file("/project/Assets/Game.asmdef", "Game", Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"), false),
                file("/project/Assets/Shared.asmdef", "Shared", Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"), false),
                file("/project/Library/PackageCache/com.unity.inputsystem@abc/Unity.InputSystem.asmdef", "Unity.InputSystem", Some("cccccccccccccccccccccccccccccccc"), true),
            ],
            vec![
                DllFile {
                    name: "Newtonsoft.Json.dll".to_string(),
                    path: PathBuf::from("/project/Assets/Plugins/Newtonsoft.Json.dll"),
                    relative_path: "Assets/Plugins/Newtonsoft.Json.dll".to_string(),
                    external: false,
                },
                DllFile {
                    name: "Unity.InputSystem.dll".to_string(),
                    path: PathBuf::from("/project/Library/PackageCache/com.unity.inputsystem@abc/Unity.InputSystem.dll"),
                    relative_path: "Library/PackageCache/com.unity.inputsystem@abc/Unity.InputSystem.dll".to_string(),
                    external: true,
                },
            ],
            vec![PackageInfo {
                name: "com.unity.inputsystem".to_string(),
                version: Some("1.20.1".to_string()),
                source: Some("registry".to_string()),
            }],
        )
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|item| item.label.as_str()).collect()
    }

    fn complete(text: &str, marker: &str) -> Vec<CompletionItem> {
        let offset = text.find(marker).unwrap() + marker.len();
        complete_at(text, offset)
    }

    fn complete_at_end(text: &str) -> Vec<CompletionItem> {
        complete_at(text, text.len())
    }

    fn complete_at(text: &str, offset: usize) -> Vec<CompletionItem> {
        completions(
            &project(),
            Path::new("/project/Assets/Game.asmdef"),
            text,
            offset,
        )
    }

    #[test]
    fn completes_assembly_names_and_excludes_self() {
        let items = complete(r#"{"name":"Game","references":[""]}"#, "[\"");
        let labels = labels(&items);
        assert!(labels.contains(&"Shared"));
        assert!(labels.contains(&"Unity.InputSystem"));
        assert!(!labels.contains(&"Game"));
    }

    #[test]
    fn completes_guid_forms_when_the_prefix_requests_them() {
        let items = complete(r#"{"references":["GUID:"]}"#, "GUID:");
        let labels = labels(&items);
        assert!(!labels.is_empty());
        assert!(labels.iter().all(|label| label.starts_with("GUID:")));
        assert!(labels.contains(&"GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
    }

    #[test]
    fn offers_guid_forms_for_an_empty_reference() {
        let items = complete(r#"{"references":[""]}"#, "[\"");
        let labels = labels(&items);
        assert!(labels.contains(&"GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
    }

    #[test]
    fn completes_platforms_packages_and_dlls() {
        let platform_items = complete(r#"{"includePlatforms":["Ed"]}"#, "Ed");
        assert!(labels(&platform_items).contains(&"Editor"));

        let package_items = complete(r#"{"versionDefines":[{"name":"com.uni"}]}"#, "com.uni");
        assert!(labels(&package_items).contains(&"com.unity.inputsystem"));

        let dll_items = complete(r#"{"precompiledReferences":["New"]}"#, "New");
        assert!(labels(&dll_items).contains(&"Newtonsoft.Json.dll"));
        assert!(labels(&dll_items).contains(&"Unity.InputSystem.dll"));
    }

    #[test]
    fn completes_schema_keys() {
        let items = complete_at_end("{\"");
        let key_labels = labels(&items);
        assert!(key_labels.contains(&"name"));
        assert!(key_labels.contains(&"references"));

        let version_keys = complete_at_end("{\"versionDefines\":[{\"");
        let version_key_labels = labels(&version_keys);
        assert!(version_key_labels.contains(&"name"));
        assert!(version_key_labels.contains(&"expression"));
        assert!(version_key_labels.contains(&"define"));
    }

    #[test]
    fn replaces_only_the_string_contents() {
        let text = r#"{"references":["Sha"]}"#;
        let offset = text.find("Sha").unwrap() + 3;
        let items = completions(
            &project(),
            Path::new("/project/Assets/Game.asmdef"),
            text,
            offset,
        );
        let shared = items.iter().find(|item| item.label == "Shared").unwrap();
        let Some(CompletionTextEdit::Edit(edit)) = &shared.text_edit else {
            panic!("missing text edit");
        };
        assert_eq!(edit.new_text, "Shared");
        assert_eq!(
            &text[edit.range.start.character as usize..edit.range.end.character as usize],
            "Sha"
        );
    }
}
