#![cfg(test)]

use std::path::PathBuf;
use std::sync::Arc;

use crate::index::{DllFile, IndexedFile, PackageInfo, ProjectIndex};
use crate::parse;

pub fn root() -> PathBuf {
    std::env::current_dir().unwrap().join("project")
}

pub fn file(relative: &str, text: &str) -> IndexedFile {
    indexed(relative, text, false, None)
}

pub fn indexed(relative: &str, text: &str, external: bool, guid: Option<&str>) -> IndexedFile {
    let path = root().join(relative);
    let parsed = parse::parse_file(&path, text);
    IndexedFile {
        kind: parsed.kind,
        path,
        relative_path: relative.to_string(),
        external,
        text: Arc::from(text),
        data_text: Arc::from(text),
        diagnostics: parsed.diagnostics,
        asmdef: parsed.asmdef,
        asmref: parsed.asmref,
        guid: guid.map(str::to_string),
    }
}

pub fn project(files: Vec<IndexedFile>) -> ProjectIndex {
    ProjectIndex::new(root(), files, Vec::new(), Vec::new())
}

pub fn project_with(
    files: Vec<IndexedFile>,
    dlls: Vec<DllFile>,
    packages: Vec<PackageInfo>,
) -> ProjectIndex {
    ProjectIndex::new(root(), files, dlls, packages)
}

pub fn package(name: &str, version: Option<&str>, source: Option<&str>) -> PackageInfo {
    PackageInfo {
        name: name.to_string(),
        version: version.map(str::to_string),
        source: source.map(str::to_string),
    }
}

pub fn offset_of(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("missing {needle:?} in {text:?}"))
}
