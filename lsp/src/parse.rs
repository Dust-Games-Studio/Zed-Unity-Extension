use std::path::Path;

use jsonc_parser::{
    ast::{Object, Value},
    common::{Range as JsonRange, Ranged},
    parse_to_ast,
    CollectOptions,
    ParseOptions,
};
use tower_lsp::lsp_types::DiagnosticSeverity;

use crate::platforms;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, offset: usize) -> bool {
        self.start <= offset && offset <= self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedString {
    pub value: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDefine {
    pub name: Option<SpannedString>,
    pub expression: Option<SpannedString>,
    pub define: Option<SpannedString>,
}

#[derive(Debug, Clone, Default)]
pub struct AsmdefData {
    pub name: Option<SpannedString>,
    pub root_namespace: Option<SpannedString>,
    pub references: Vec<SpannedString>,
    pub include_platforms: Vec<SpannedString>,
    pub exclude_platforms: Vec<SpannedString>,
    pub precompiled_references: Vec<SpannedString>,
    pub version_defines: Vec<VersionDefine>,
}

#[derive(Debug, Clone, Default)]
pub struct AsmrefData {
    pub reference: Option<SpannedString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Asmdef,
    Asmref,
}

#[derive(Debug, Clone)]
pub struct RawDiagnostic {
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub code: &'static str,
    pub message: String,
}

impl RawDiagnostic {
    pub fn error(range: TextRange, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::ERROR,
            code,
            message: message.into(),
        }
    }

    pub fn warning(range: TextRange, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            range,
            severity: DiagnosticSeverity::WARNING,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub kind: FileKind,
    pub asmdef: Option<AsmdefData>,
    pub asmref: Option<AsmrefData>,
    pub diagnostics: Vec<RawDiagnostic>,
    pub valid_json: bool,
}

impl ParsedFile {
    pub fn data_name(&self) -> Option<&SpannedString> {
        match self.kind {
            FileKind::Asmdef => self.asmdef.as_ref().and_then(|data| data.name.as_ref()),
            FileKind::Asmref => None,
        }
    }
}

pub fn parse_file(path: &Path, text: &str) -> ParsedFile {
    let kind = match path.extension().and_then(|extension| extension.to_str()) {
        Some("asmref") => FileKind::Asmref,
        _ => FileKind::Asmdef,
    };
    let mut diagnostics = Vec::new();

    let parsed = match parse_to_ast(text, &CollectOptions::default(), &strict_options()) {
        Ok(parsed) => parsed,
        Err(error) => {
            diagnostics.push(RawDiagnostic::error(
                raw_range(error.range()),
                "asmdef.parse-error",
                format!("Invalid JSON: {}", error.kind()),
            ));
            return ParsedFile {
                kind,
                asmdef: None,
                asmref: None,
                diagnostics,
                valid_json: false,
            };
        }
    };

    let Some(value) = parsed.value else {
        diagnostics.push(RawDiagnostic::error(
            TextRange::new(0, 0),
            "asmdef.parse-error",
            "JSON document is empty.",
        ));
        return ParsedFile {
            kind,
            asmdef: None,
            asmref: None,
            diagnostics,
            valid_json: false,
        };
    };

    let Some(object) = value.as_object() else {
        diagnostics.push(RawDiagnostic::error(
            raw_range(value.range()),
            "asmdef.root-not-object",
            "The root value must be a JSON object.",
        ));
        return ParsedFile {
            kind,
            asmdef: None,
            asmref: None,
            diagnostics,
            valid_json: true,
        };
    };

    let (asmdef, asmref) = match kind {
        FileKind::Asmdef => (
            Some(parse_asmdef_object(object, text, &mut diagnostics)),
            None,
        ),
        FileKind::Asmref => (
            None,
            Some(parse_asmref_object(object, text, &mut diagnostics)),
        ),
    };

    ParsedFile {
        kind,
        asmdef,
        asmref,
        diagnostics,
        valid_json: true,
    }
}

pub fn read_guid(meta_path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(meta_path).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("guid:") else {
            continue;
        };
        let guid = rest.trim().trim_matches('"').trim();
        if !guid.is_empty() {
            return Some(guid.to_string());
        }
    }
    None
}

fn strict_options() -> ParseOptions {
    ParseOptions {
        allow_comments: false,
        allow_loose_object_property_names: false,
        allow_trailing_commas: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
        allow_bare_decimal_point_numbers: false,
        allow_non_finite_numbers: false,
        allow_extended_string_escapes: false,
    }
}

fn raw_range(range: JsonRange) -> TextRange {
    TextRange::new(range.start, range.end)
}

fn content_range(range: JsonRange, text: &str) -> TextRange {
    let mut start = range.start;
    let mut end = range.end;
    if text.as_bytes().get(start) == Some(&b'"') {
        start += 1;
    }
    if end > start && text.as_bytes().get(end - 1) == Some(&b'"') {
        end -= 1;
    }
    TextRange::new(start, end)
}

fn spanned_string(literal: &jsonc_parser::ast::StringLit<'_>, text: &str) -> SpannedString {
    SpannedString {
        value: literal.value.to_string(),
        range: content_range(literal.range, text),
    }
}

fn missing_field(diagnostics: &mut Vec<RawDiagnostic>, range: JsonRange, code: &'static str, key: &str) {
    diagnostics.push(RawDiagnostic::error(
        TextRange::new(range.start, range.start),
        code,
        format!("Missing required field `{key}`."),
    ));
}

fn string_field(
    object: &Object<'_>,
    key: &str,
    text: &str,
    diagnostics: &mut Vec<RawDiagnostic>,
) -> Option<SpannedString> {
    let property = object.get(key)?;
    match &property.value {
        Value::StringLit(literal) => Some(spanned_string(literal, text)),
        other => {
            diagnostics.push(RawDiagnostic::error(
                raw_range(other.range()),
                "asmdef.invalid-field",
                format!("Field `{key}` must be a string."),
            ));
            None
        }
    }
}

fn string_array_field(
    object: &Object<'_>,
    key: &str,
    text: &str,
    diagnostics: &mut Vec<RawDiagnostic>,
) -> Vec<SpannedString> {
    let Some(property) = object.get(key) else {
        return Vec::new();
    };
    let Some(array) = property.value.as_array() else {
        diagnostics.push(RawDiagnostic::error(
            raw_range(property.value.range()),
            "asmdef.invalid-field",
            format!("Field `{key}` must be an array of strings."),
        ));
        return Vec::new();
    };

    let mut values = Vec::new();
    for element in &array.elements {
        match element {
            Value::StringLit(literal) => values.push(spanned_string(literal, text)),
            other => diagnostics.push(RawDiagnostic::error(
                raw_range(other.range()),
                "asmdef.invalid-field",
                format!("Each item in `{key}` must be a string."),
            )),
        }
    }
    values
}

fn boolean_field(object: &Object<'_>, key: &str, diagnostics: &mut Vec<RawDiagnostic>) {
    let Some(property) = object.get(key) else {
        return;
    };
    if property.value.as_boolean_lit().is_none() {
        diagnostics.push(RawDiagnostic::error(
            raw_range(property.value.range()),
            "asmdef.invalid-field",
            format!("Field `{key}` must be a boolean."),
        ));
    }
}

fn parse_asmdef_object(
    object: &Object<'_>,
    text: &str,
    diagnostics: &mut Vec<RawDiagnostic>,
) -> AsmdefData {
    let name = string_field(object, "name", text, diagnostics);
    if object.get("name").is_none() {
        missing_field(
            diagnostics,
            object.range,
            "asmdef.missing-name",
            "name",
        );
    } else if let Some(name) = &name {
        if name.value.is_empty() {
            diagnostics.push(RawDiagnostic::error(
                name.range,
                "asmdef.missing-name",
                "Assembly name must not be empty.",
            ));
        }
    }

    let root_namespace = string_field(object, "rootNamespace", text, diagnostics);
    let references = string_array_field(object, "references", text, diagnostics);
    let include_platforms = string_array_field(object, "includePlatforms", text, diagnostics);
    let exclude_platforms = string_array_field(object, "excludePlatforms", text, diagnostics);
    let precompiled_references =
        string_array_field(object, "precompiledReferences", text, diagnostics);
    let version_defines = version_defines_field(object, text, diagnostics);

    for key in [
        "allowUnsafeCode",
        "overrideReferences",
        "autoReferenced",
        "noEngineReferences",
    ] {
        boolean_field(object, key, diagnostics);
    }

    if !include_platforms.is_empty() && !exclude_platforms.is_empty() {
        diagnostics.push(RawDiagnostic::warning(
            include_platforms[0].range,
            "asmdef.platform-conflict",
            "Unity requires `includePlatforms` to be empty when `excludePlatforms` is not empty.",
        ));
    }

    for platform in include_platforms.iter().chain(exclude_platforms.iter()) {
        if !platforms::is_known(&platform.value) {
            diagnostics.push(RawDiagnostic::warning(
                platform.range,
                "asmdef.unknown-platform",
                format!("Unknown Unity assembly platform `{}`.", platform.value),
            ));
        }
    }

    AsmdefData {
        name,
        root_namespace,
        references,
        include_platforms,
        exclude_platforms,
        precompiled_references,
        version_defines,
    }
}

fn parse_asmref_object(
    object: &Object<'_>,
    text: &str,
    diagnostics: &mut Vec<RawDiagnostic>,
) -> AsmrefData {
    let reference = match string_field(object, "reference", text, diagnostics) {
        Some(reference) if reference.value.is_empty() => {
            diagnostics.push(RawDiagnostic::error(
                reference.range,
                "asmref.missing-reference",
                "Assembly reference must not be empty.",
            ));
            Some(reference)
        }
        Some(reference) => Some(reference),
        None => {
            if object.get("reference").is_none() {
                missing_field(
                    diagnostics,
                    object.range,
                    "asmref.missing-reference",
                    "reference",
                );
            }
            None
        }
    };
    AsmrefData { reference }
}

fn version_defines_field(
    object: &Object<'_>,
    text: &str,
    diagnostics: &mut Vec<RawDiagnostic>,
) -> Vec<VersionDefine> {
    let Some(property) = object.get("versionDefines") else {
        return Vec::new();
    };
    let Some(array) = property.value.as_array() else {
        diagnostics.push(RawDiagnostic::error(
            raw_range(property.value.range()),
            "asmdef.invalid-field",
            "Field `versionDefines` must be an array of objects.",
        ));
        return Vec::new();
    };

    let mut defines = Vec::new();
    for element in &array.elements {
        let Some(entry) = element.as_object() else {
            diagnostics.push(RawDiagnostic::error(
                raw_range(element.range()),
                "asmdef.invalid-version-define",
                "Each `versionDefines` entry must be an object.",
            ));
            continue;
        };

        let name = required_version_define_field(entry, "name", text, diagnostics);
        let expression = required_version_define_field(entry, "expression", text, diagnostics);
        let define = required_version_define_field(entry, "define", text, diagnostics);
        defines.push(VersionDefine {
            name,
            expression,
            define,
        });
    }
    defines
}

fn required_version_define_field(
    object: &Object<'_>,
    key: &str,
    text: &str,
    diagnostics: &mut Vec<RawDiagnostic>,
) -> Option<SpannedString> {
    match object.get(key) {
        Some(property) => match &property.value {
            Value::StringLit(literal) => {
                let value = spanned_string(literal, text);
                if value.value.is_empty() {
                    diagnostics.push(RawDiagnostic::error(
                        value.range,
                        "asmdef.invalid-version-define",
                        format!("`versionDefines.{key}` must not be empty."),
                    ));
                }
                Some(value)
            }
            other => {
                diagnostics.push(RawDiagnostic::error(
                    raw_range(other.range()),
                    "asmdef.invalid-version-define",
                    format!("`versionDefines.{key}` must be a string."),
                ));
                None
            }
        },
        None => {
            diagnostics.push(RawDiagnostic::error(
                TextRange::new(object.range.start, object.range.start),
                "asmdef.invalid-version-define",
                format!("Missing required field `versionDefines.{key}`."),
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_asmdef(text: &str) -> ParsedFile {
        parse_file(Path::new("Game.asmdef"), text)
    }

    fn parse_asmref(text: &str) -> ParsedFile {
        parse_file(Path::new("Game.asmref"), text)
    }

    fn codes(file: &ParsedFile) -> Vec<&'static str> {
        file.diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    #[test]
    fn extracts_asmdef_fields_and_content_spans() {
        let text = r#"{
            "name": "Game.Runtime",
            "rootNamespace": "Game",
            "references": ["GUID:abc", "Other"],
            "includePlatforms": ["Editor"],
            "precompiledReferences": ["Newtonsoft.Json.dll"],
            "versionDefines": [
                { "name": "com.unity.inputsystem", "expression": "1.0.0", "define": "HAS_INPUT" }
            ]
        }"#;
        let parsed = parse_asmdef(text);
        assert!(parsed.valid_json);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

        let data = parsed.asmdef.unwrap();
        assert_eq!(data.name.as_ref().unwrap().value, "Game.Runtime");
        assert_eq!(
            &text[data.name.as_ref().unwrap().range.start..data.name.as_ref().unwrap().range.end],
            "Game.Runtime"
        );
        assert_eq!(data.references.len(), 2);
        assert_eq!(data.references[0].value, "GUID:abc");
        assert_eq!(data.include_platforms[0].value, "Editor");
        assert_eq!(data.precompiled_references[0].value, "Newtonsoft.Json.dll");
        assert_eq!(data.version_defines[0].name.as_ref().unwrap().value, "com.unity.inputsystem");
        assert_eq!(data.version_defines[0].define.as_ref().unwrap().value, "HAS_INPUT");
    }

    #[test]
    fn extracts_asmref_reference() {
        let parsed = parse_asmref(r#"{ "reference": "Game.Runtime" }"#);
        assert!(parsed.valid_json);
        assert!(parsed.diagnostics.is_empty());
        assert_eq!(
            parsed.asmref.unwrap().reference.unwrap().value,
            "Game.Runtime"
        );
    }

    #[test]
    fn reports_malformed_json_with_a_range() {
        let parsed = parse_asmdef(r#"{ "name": }"#);
        assert!(!parsed.valid_json);
        assert_eq!(codes(&parsed), vec!["asmdef.parse-error"]);
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.severity, DiagnosticSeverity::ERROR);
        assert!(diagnostic.range.start <= diagnostic.range.end);
    }

    #[test]
    fn reports_missing_and_empty_names() {
        let missing = parse_asmdef(r#"{ "references": [] }"#);
        assert_eq!(codes(&missing), vec!["asmdef.missing-name"]);

        let empty = parse_asmdef(r#"{ "name": "" }"#);
        assert_eq!(codes(&empty), vec!["asmdef.missing-name"]);
    }

    #[test]
    fn reports_wrong_field_types() {
        let parsed = parse_asmdef(r#"{ "name": "Game", "references": "Other" }"#);
        assert_eq!(codes(&parsed), vec!["asmdef.invalid-field"]);
        assert!(parsed.diagnostics[0].message.contains("references"));
    }

    #[test]
    fn reports_invalid_version_defines() {
        let parsed = parse_asmdef(
            r#"{ "name": "Game", "versionDefines": [{ "name": "com.unity.test", "expression": "1.0.0" }] }"#,
        );
        assert_eq!(codes(&parsed), vec!["asmdef.invalid-version-define"]);
        assert!(parsed.diagnostics[0].message.contains("define"));
    }

    #[test]
    fn reports_platform_conflicts_and_unknown_platforms() {
        let parsed = parse_asmdef(
            r#"{ "name": "Game", "includePlatforms": ["Editor"], "excludePlatforms": ["Andriod"] }"#,
        );
        assert!(codes(&parsed).contains(&"asmdef.platform-conflict"));
        assert!(codes(&parsed).contains(&"asmdef.unknown-platform"));
    }

    #[test]
    fn reports_root_not_object() {
        let parsed = parse_asmdef(r#"["Game"]"#);
        assert_eq!(codes(&parsed), vec!["asmdef.root-not-object"]);
    }

    #[test]
    fn reads_guid_from_meta_file() {
        let temp = tempfile::tempdir().unwrap();
        let meta = temp.path().join("Game.asmdef.meta");
        std::fs::write(&meta, "fileFormatVersion: 2\nguid: 17b36165d09634a48bf5a0e4bb27f4bd\n").unwrap();
        assert_eq!(
            read_guid(&meta).as_deref(),
            Some("17b36165d09634a48bf5a0e4bb27f4bd")
        );
        assert_eq!(read_guid(&temp.path().join("missing.meta")), None);
    }
}
