use std::collections::HashSet;

use crate::index::{IndexedFile, ProjectIndex};
use crate::parse::{FileKind, TextRange};

/// The kind of symbol under the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// An assembly's `name` value in an `.asmdef`.
    Declaration,
    /// A `references` entry or an `.asmref` `reference` value.
    Reference,
}

/// An assembly declaration or reference found at a byte offset.
#[derive(Debug, Clone, Copy)]
pub struct Symbol<'a> {
    pub file: &'a IndexedFile,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub value: &'a str,
}

/// Find the assembly declaration or reference at `offset`.
pub fn symbol_at(file: &IndexedFile, offset: usize) -> Option<Symbol<'_>> {
    match file.kind {
        FileKind::Asmdef => {
            if let Some(name) = file.asmdef.as_ref().and_then(|asmdef| asmdef.name.as_ref()) {
                if contains(file, name.range, offset) {
                    return Some(Symbol {
                        file,
                        kind: SymbolKind::Declaration,
                        range: name.range,
                        value: &name.value,
                    });
                }
            }
            file.references()
                .iter()
                .find(|reference| contains(file, reference.range, offset))
                .map(|reference| Symbol {
                    file,
                    kind: SymbolKind::Reference,
                    range: reference.range,
                    value: &reference.value,
                })
        }
        FileKind::Asmref => file
            .asmref_reference()
            .filter(|reference| contains(file, reference.range, offset))
            .map(|reference| Symbol {
                file,
                kind: SymbolKind::Reference,
                range: reference.range,
                value: &reference.value,
            }),
    }
}

/// Content spans exclude the surrounding quotes; treat the opening quote as a hit too.
fn contains(file: &IndexedFile, range: TextRange, offset: usize) -> bool {
    range.contains(offset)
        || (offset.checked_add(1) == Some(range.start)
            && file.data_text.as_bytes().get(offset) == Some(&b'"'))
}

/// Resolve the assembly or assemblies a symbol points at.
pub fn targets<'a>(project: &'a ProjectIndex, symbol: Symbol<'a>) -> Vec<&'a IndexedFile> {
    match symbol.kind {
        SymbolKind::Declaration => vec![symbol.file],
        SymbolKind::Reference => resolve_value(project, symbol.value),
    }
}

/// The GUID part of a `GUID:` reference, accepting any casing of the prefix.
pub fn guid_reference(value: &str) -> Option<&str> {
    let prefix = value.get(..5)?;
    prefix.eq_ignore_ascii_case("GUID:").then(|| &value[5..])
}

/// Resolve a reference value by name or `GUID:` form.
pub fn resolve_value<'a>(project: &'a ProjectIndex, value: &str) -> Vec<&'a IndexedFile> {
    if let Some(guid) = guid_reference(value) {
        project.find_by_guid(guid)
    } else if value.is_empty() {
        Vec::new()
    } else {
        project.find_by_name(value)
    }
}

/// The closest assembly names to an unresolved reference, best match first.
///
/// Candidates are compared case-insensitively with Levenshtein distance. Names
/// more than roughly a third of the input away are discarded, project
/// assemblies sort before packages, and duplicate names are collapsed.
pub fn nearest_assemblies<'a>(
    project: &'a ProjectIndex,
    value: &str,
    limit: usize,
) -> Vec<&'a IndexedFile> {
    let value = value.trim();
    if value.is_empty() || guid_reference(value).is_some() || limit == 0 {
        return Vec::new();
    }

    let needle = value.to_ascii_lowercase();
    let threshold = (value.chars().count() / 3).max(2);
    let mut candidates: Vec<(usize, bool, String, &'a IndexedFile)> = project
        .assemblies()
        .filter_map(|file| {
            let name = file.name()?;
            if name == value {
                return None;
            }
            let distance = levenshtein(&needle, &name.to_ascii_lowercase());
            if distance > threshold {
                return None;
            }
            Some((distance, file.external, name.to_string(), file))
        })
        .collect();

    candidates.sort_by(|left, right| {
        (left.0, left.1, left.2.as_str()).cmp(&(right.0, right.1, right.2.as_str()))
    });
    let mut seen = HashSet::new();
    candidates.retain(|(_, _, name, _)| seen.insert(name.clone()));
    candidates.truncate(limit);
    candidates.into_iter().map(|(_, _, _, file)| file).collect()
}

/// Standard Levenshtein edit distance over Unicode scalar values.
pub fn levenshtein(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (row, left_char) in left.chars().enumerate() {
        current[0] = row + 1;
        for (column, right_char) in right.iter().enumerate() {
            let substitution = previous[column] + usize::from(left_char != *right_char);
            current[column + 1] = (previous[column + 1] + 1)
                .min(current[column] + 1)
                .min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right.len()]
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

    fn input_system() -> IndexedFile {
        indexed(
            "Library/PackageCache/com.unity.inputsystem@abc/Unity.InputSystem.asmdef",
            r#"{ "name": "Unity.InputSystem" }"#,
            true,
            Some("cccccccccccccccccccccccccccccccc"),
        )
    }

    #[test]
    fn finds_a_declaration_at_the_name_value() {
        let text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let file = file("Assets/Game.asmdef", text);
        let offset = offset_of(text, "Game") + 2;
        let symbol = symbol_at(&file, offset).expect("declaration");
        assert_eq!(symbol.kind, SymbolKind::Declaration);
        assert_eq!(symbol.value, "Game");
        assert_eq!(&text[symbol.range.start..symbol.range.end], "Game");
    }

    #[test]
    fn finds_a_reference_in_an_asmdef() {
        let text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let file = file("Assets/Game.asmdef", text);
        let offset = offset_of(text, "Shared") + 1;
        let symbol = symbol_at(&file, offset).expect("reference");
        assert_eq!(symbol.kind, SymbolKind::Reference);
        assert_eq!(symbol.value, "Shared");
    }

    #[test]
    fn finds_a_reference_in_an_asmref() {
        let text = r#"{ "reference": "Shared" }"#;
        let file = file("Assets/Ref.asmref", text);
        let offset = offset_of(text, "Shared") + 1;
        let symbol = symbol_at(&file, offset).expect("reference");
        assert_eq!(symbol.kind, SymbolKind::Reference);
        assert_eq!(symbol.value, "Shared");
    }

    #[test]
    fn returns_none_outside_symbols() {
        let text = r#"{ "name": "Game" }"#;
        let file = file("Assets/Game.asmdef", text);
        assert!(symbol_at(&file, 0).is_none());
        assert!(symbol_at(&file, text.len()).is_none());
    }

    #[test]
    fn finds_symbols_on_their_opening_quote() {
        let text = r#"{ "name": "Game", "references": ["Shared"] }"#;
        let file = file("Assets/Game.asmdef", text);

        let declaration_quote = offset_of(text, "\"Game\"");
        let symbol = symbol_at(&file, declaration_quote).expect("declaration quote");
        assert_eq!(symbol.kind, SymbolKind::Declaration);

        let reference_quote = offset_of(text, "\"Shared\"");
        let symbol = symbol_at(&file, reference_quote).expect("reference quote");
        assert_eq!(symbol.kind, SymbolKind::Reference);
        assert_eq!(symbol.value, "Shared");
    }

    #[test]
    fn resolves_names_and_guids_case_insensitively() {
        let project = project(vec![game(), shared(), input_system()]);
        assert_eq!(resolve_value(&project, "Shared")[0].name(), Some("Shared"));
        assert_eq!(
            resolve_value(&project, "GUID:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")[0].name(),
            Some("Shared")
        );
        assert_eq!(
            resolve_value(&project, "guid:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")[0].name(),
            Some("Shared")
        );
        assert!(resolve_value(&project, "Missing").is_empty());
        assert!(resolve_value(&project, "").is_empty());
    }

    #[test]
    fn returns_every_target_for_duplicate_names() {
        let project = project(vec![
            file("Assets/A.asmdef", r#"{ "name": "Duplicate" }"#),
            file("Assets/B.asmdef", r#"{ "name": "Duplicate" }"#),
        ]);
        assert_eq!(resolve_value(&project, "Duplicate").len(), 2);
    }

    #[test]
    fn targets_a_declaration_at_itself_and_a_reference_at_its_target() {
        let project = project(vec![game(), shared()]);
        let game = project
            .file(&root().join("Assets/Game.asmdef"))
            .expect("game");
        let declaration_offset = offset_of(&game.text, "Game") + 1;
        assert_eq!(
            targets(&project, symbol_at(game, declaration_offset).unwrap()).len(),
            1
        );

        let reference_offset = offset_of(&game.text, "Shared") + 1;
        assert_eq!(
            targets(&project, symbol_at(game, reference_offset).unwrap())[0].name(),
            Some("Shared")
        );
    }

    #[test]
    fn suggests_the_nearest_assembly_names() {
        let project = project(vec![game(), shared(), input_system()]);
        let shared = nearest_assemblies(&project, "Shard", 3);
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].name(), Some("Shared"));

        let input = nearest_assemblies(&project, "Unity.InputSytem", 3);
        assert_eq!(input.len(), 1);
        assert_eq!(input[0].name(), Some("Unity.InputSystem"));

        assert!(nearest_assemblies(&project, "CompletelyDifferent", 3).is_empty());
        assert!(nearest_assemblies(&project, "", 3).is_empty());
        assert!(nearest_assemblies(&project, "GUID:abc", 3).is_empty());
        assert!(nearest_assemblies(&project, "Shard", 0).is_empty());
    }

    #[test]
    fn collapses_duplicate_names_and_prefers_project_assemblies() {
        let project = project(vec![
            indexed(
                "Library/PackageCache/com.example@abc/Shared.asmdef",
                r#"{ "name": "Shared" }"#,
                true,
                None,
            ),
            shared(),
        ]);
        let suggested = nearest_assemblies(&project, "Shard", 3);
        assert_eq!(suggested.len(), 1);
        assert!(!suggested[0].external);
    }

    #[test]
    fn computes_levenshtein_distance() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("Shard", "Shared"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("same", "same"), 0);
    }
}
