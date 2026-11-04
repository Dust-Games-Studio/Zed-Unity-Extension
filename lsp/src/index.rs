use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use walkdir::WalkDir;

use crate::parse::{
    self, AsmdefData, AsmrefData, FileKind, RawDiagnostic, SpannedString, TextRange,
};
use crate::paths;

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub kind: FileKind,
    pub path: PathBuf,
    pub relative_path: String,
    pub external: bool,
    pub text: Arc<str>,
    pub diagnostics: Vec<RawDiagnostic>,
    pub asmdef: Option<AsmdefData>,
    pub asmref: Option<AsmrefData>,
    pub guid: Option<String>,
}

impl IndexedFile {
    pub fn name(&self) -> Option<&str> {
        self.asmdef
            .as_ref()?
            .name
            .as_ref()
            .map(|name| name.value.as_str())
    }

    pub fn name_span(&self) -> Option<TextRange> {
        self.asmdef.as_ref()?.name.as_ref().map(|name| name.range)
    }

    pub fn root_namespace(&self) -> Option<&str> {
        self.asmdef
            .as_ref()?
            .root_namespace
            .as_ref()
            .map(|namespace| namespace.value.as_str())
    }

    pub fn references(&self) -> &[SpannedString] {
        self.asmdef
            .as_ref()
            .map(|asmdef| asmdef.references.as_slice())
            .unwrap_or_default()
    }

    pub fn asmref_reference(&self) -> Option<&SpannedString> {
        self.asmref.as_ref()?.reference.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct DllFile {
    pub name: String,
    pub path: PathBuf,
    pub relative_path: String,
    pub external: bool,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectIndex {
    pub root: PathBuf,
    pub files: Vec<IndexedFile>,
    pub dlls: Vec<DllFile>,
    pub packages: Vec<PackageInfo>,
    name_index: HashMap<String, Vec<usize>>,
    guid_index: HashMap<String, Vec<usize>>,
}

impl ProjectIndex {
    #[allow(dead_code)]
    pub fn new(
        root: PathBuf,
        files: Vec<IndexedFile>,
        dlls: Vec<DllFile>,
        packages: Vec<PackageInfo>,
    ) -> Self {
        let mut project = Self {
            root,
            files,
            dlls,
            packages,
            name_index: HashMap::new(),
            guid_index: HashMap::new(),
        };
        project.rebuild_indexes();
        project
    }

    pub fn rebuild_indexes(&mut self) {
        self.name_index.clear();
        self.guid_index.clear();
        for (index, file) in self.files.iter().enumerate() {
            if let Some(name) = file.name() {
                self.name_index
                    .entry(name.to_string())
                    .or_default()
                    .push(index);
            }
            if let Some(guid) = &file.guid {
                self.guid_index.entry(guid.clone()).or_default().push(index);
            }
        }
    }

    pub fn find_by_name(&self, name: &str) -> Vec<&IndexedFile> {
        self.name_index
            .get(name)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|index| self.files.get(*index))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn find_by_guid(&self, guid: &str) -> Vec<&IndexedFile> {
        self.guid_index
            .get(guid)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|index| self.files.get(*index))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn file(&self, path: &Path) -> Option<&IndexedFile> {
        self.files.iter().find(|file| file.path == path)
    }

    pub fn project_files(&self) -> impl Iterator<Item = &IndexedFile> {
        self.files.iter().filter(|file| !file.external)
    }

    pub fn assemblies(&self) -> impl Iterator<Item = &IndexedFile> {
        self.files
            .iter()
            .filter(|file| file.kind == FileKind::Asmdef)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceIndex {
    pub projects: Vec<ProjectIndex>,
}

impl WorkspaceIndex {
    pub fn project_for_path(&self, path: &Path) -> Option<&ProjectIndex> {
        let path = normalize_path(path);
        self.projects
            .iter()
            .filter(|project| path.starts_with(&project.root))
            .max_by_key(|project| project.root.components().count())
    }

    pub fn project_for_path_mut(&mut self, path: &Path) -> Option<&mut ProjectIndex> {
        let path = normalize_path(path);
        let index = self
            .projects
            .iter()
            .enumerate()
            .filter(|(_, project)| path.starts_with(&project.root))
            .max_by_key(|(_, project)| project.root.components().count())
            .map(|(index, _)| index)?;
        self.projects.get_mut(index)
    }

    pub fn replace_file(&mut self, path: &Path, text: &str) -> bool {
        let path = normalize_path(path);
        let Some(project) = self.project_for_path_mut(&path) else {
            return false;
        };
        let project_root = project.root.clone();
        let parsed = parse::parse_file(&path, text);
        let guid = if parsed.kind == FileKind::Asmdef {
            parse::read_guid(&meta_path(&path))
        } else {
            None
        };
        let file = IndexedFile {
            kind: parsed.kind,
            relative_path: relative_path(&project_root, &path),
            external: is_external(&project_root, &path),
            text: Arc::from(text),
            diagnostics: parsed.diagnostics,
            asmdef: parsed.asmdef,
            asmref: parsed.asmref,
            guid,
            path: path.clone(),
        };

        if let Some(existing) = project.files.iter_mut().find(|file| file.path == path) {
            *existing = file;
        } else {
            project.files.push(file);
            project
                .files
                .sort_by(|left, right| left.path.cmp(&right.path));
        }
        project.rebuild_indexes();
        true
    }

    pub fn remove_file(&mut self, path: &Path) -> bool {
        let path = normalize_path(path);
        let Some(project) = self.project_for_path_mut(&path) else {
            return false;
        };
        let before = project.files.len();
        project.files.retain(|file| file.path != path);
        project.dlls.retain(|dll| dll.path != path);
        project.rebuild_indexes();
        before != project.files.len()
    }
}

pub fn build(
    workspace_roots: &[PathBuf],
    open_documents: &HashMap<PathBuf, String>,
) -> WorkspaceIndex {
    let mut projects = paths::discover_unity_projects(workspace_roots, 4);

    if projects.is_empty() {
        for root in workspace_roots {
            if root.join("Assets").is_dir() || root.join("Packages").is_dir() {
                projects.push(root.clone());
            }
        }
    }

    WorkspaceIndex {
        projects: projects
            .into_iter()
            .map(|root| build_project(&root, open_documents))
            .collect(),
    }
}

pub fn build_project(root: &Path, open_documents: &HashMap<PathBuf, String>) -> ProjectIndex {
    let root = normalize_path(root);
    let packages = read_packages(&root);
    let mut project = ProjectIndex {
        root: root.clone(),
        files: Vec::new(),
        dlls: Vec::new(),
        packages,
        name_index: HashMap::new(),
        guid_index: HashMap::new(),
    };

    let mut seen_files = HashSet::new();
    let mut seen_dlls = HashSet::new();

    scan_dir(
        &root.join("Assets"),
        &root,
        false,
        open_documents,
        &mut project,
        &mut seen_files,
        &mut seen_dlls,
    );
    scan_dir(
        &root.join("Packages"),
        &root,
        true,
        open_documents,
        &mut project,
        &mut seen_files,
        &mut seen_dlls,
    );

    for package_dir in package_cache_dirs(&root, &project.packages) {
        scan_dir(
            &package_dir,
            &root,
            true,
            open_documents,
            &mut project,
            &mut seen_files,
            &mut seen_dlls,
        );
    }

    project
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    project
        .dlls
        .sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    project.rebuild_indexes();
    project
}

fn scan_dir(
    dir: &Path,
    project_root: &Path,
    external: bool,
    open_documents: &HashMap<PathBuf, String>,
    project: &mut ProjectIndex,
    seen_files: &mut HashSet<PathBuf>,
    seen_dlls: &mut HashSet<PathBuf>,
) {
    if !dir.is_dir() {
        return;
    }

    let walker = WalkDir::new(dir).into_iter().filter_entry(|entry| {
        entry
            .file_name()
            .to_str()
            .map(|name| !paths::is_excluded_dir(name))
            .unwrap_or(false)
    });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = normalize_path(entry.path());

        if paths::is_asmdef_or_asmref(&path) {
            if !seen_files.insert(path.clone()) {
                continue;
            }
            let text = open_documents
                .get(&path)
                .cloned()
                .or_else(|| fs::read_to_string(&path).ok());
            let Some(text) = text else {
                log::warn!("failed to read {}", path.display());
                continue;
            };
            let parsed = parse::parse_file(&path, &text);
            let guid = if parsed.kind == FileKind::Asmdef {
                parse::read_guid(&meta_path(&path))
            } else {
                None
            };
            project.files.push(IndexedFile {
                kind: parsed.kind,
                relative_path: relative_path(project_root, &path),
                external,
                text: Arc::from(text),
                diagnostics: parsed.diagnostics,
                asmdef: parsed.asmdef,
                asmref: parsed.asmref,
                guid,
                path,
            });
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
            && seen_dlls.insert(path.clone())
        {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            project.dlls.push(DllFile {
                name,
                relative_path: relative_path(project_root, &path),
                external,
                path,
            });
        }
    }
}

fn read_packages(project_root: &Path) -> Vec<PackageInfo> {
    let mut packages = HashMap::<String, PackageInfo>::new();

    if let Some(manifest) = read_json(&project_root.join("Packages/manifest.json")) {
        if let Some(dependencies) = manifest.get("dependencies").and_then(Value::as_object) {
            for (name, value) in dependencies {
                let version = value.as_str().map(str::to_string).or_else(|| {
                    value
                        .get("version")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
                packages.insert(
                    name.clone(),
                    PackageInfo {
                        name: name.clone(),
                        version,
                        source: Some("manifest".to_string()),
                    },
                );
            }
        }
    }

    if let Some(lock) = read_json(&project_root.join("Packages/packages-lock.json")) {
        if let Some(dependencies) = lock.get("dependencies").and_then(Value::as_object) {
            for (name, value) in dependencies {
                let version = value
                    .get("version")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let source = value
                    .get("source")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                packages.insert(
                    name.clone(),
                    PackageInfo {
                        name: name.clone(),
                        version,
                        source,
                    },
                );
            }
        }
    }

    let mut packages: Vec<_> = packages.into_values().collect();
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    packages
}

fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            log::warn!("failed to parse {}: {error}", path.display());
            None
        }
    }
}

fn package_cache_dirs(project_root: &Path, packages: &[PackageInfo]) -> Vec<PathBuf> {
    let cache = project_root.join("Library/PackageCache");
    let Ok(entries) = fs::read_dir(&cache) else {
        return Vec::new();
    };
    let names: HashSet<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();

    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let package_name = name.split('@').next().unwrap_or(&name);
        if names.contains(package_name) {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    dirs
}

fn is_external(project_root: &Path, path: &Path) -> bool {
    path.starts_with(project_root.join("Packages"))
        || path.starts_with(project_root.join("Library/PackageCache"))
}

fn relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn meta_path(path: &Path) -> PathBuf {
    let mut meta = path.as_os_str().to_os_string();
    meta.push(".meta");
    PathBuf::from(meta)
}

pub fn normalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write(
            &root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 6000.6.6f1",
        );
        write(
            &root.join("Packages/manifest.json"),
            r#"{ "dependencies": { "com.unity.inputsystem": "1.20.1" } }"#,
        );
        write(
            &root.join("Packages/packages-lock.json"),
            r#"{ "dependencies": { "com.unity.inputsystem": { "version": "1.20.1", "source": "registry" } } }"#,
        );
        write(
            &root.join("Assets/Game/Game.asmdef"),
            r#"{ "name": "Game.Runtime", "references": ["Shared"] }"#,
        );
        write(
            &root.join("Assets/Game/Game.asmdef.meta"),
            "fileFormatVersion: 2\nguid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        );
        write(
            &root.join("Assets/Shared/Shared.asmdef"),
            r#"{ "name": "Shared" }"#,
        );
        write(
            &root.join("Assets/Shared/Shared.asmdef.meta"),
            "fileFormatVersion: 2\nguid: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n",
        );
        write(&root.join("Assets/Plugins/Newtonsoft.Json.dll"), "");
        write(
            &root.join("Library/PackageCache/com.unity.inputsystem@abc/InputSystem/Unity.InputSystem.asmdef"),
            r#"{ "name": "Unity.InputSystem" }"#,
        );
        write(
            &root.join("Library/PackageCache/com.unity.inputsystem@abc/InputSystem/Unity.InputSystem.asmdef.meta"),
            "fileFormatVersion: 2\nguid: cccccccccccccccccccccccccccccccc\n",
        );
        write(
            &root.join(
                "Library/PackageCache/com.unity.inputsystem@abc/InputSystem/Unity.InputSystem.dll",
            ),
            "",
        );
        write(
            &root.join("Library/PackageCache/not-a-dependency@abc/Ignored.asmdef"),
            r#"{ "name": "Ignored.Package" }"#,
        );
        temp
    }

    #[test]
    fn builds_project_and_package_index() {
        let temp = fixture();
        let index = build(&[temp.path().to_path_buf()], &HashMap::new());
        assert_eq!(index.projects.len(), 1);
        let project = &index.projects[0];

        let names: Vec<_> = project
            .assemblies()
            .map(|file| file.name().unwrap())
            .collect();
        assert!(names.contains(&"Game.Runtime"));
        assert!(names.contains(&"Shared"));
        assert!(names.contains(&"Unity.InputSystem"));
        assert!(!names.contains(&"Ignored.Package"));

        let input_system = project.find_by_name("Unity.InputSystem");
        assert_eq!(input_system.len(), 1);
        assert!(input_system[0].external);

        assert_eq!(
            project.find_by_name("Game.Runtime")[0].guid.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            project.find_by_guid("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")[0].name(),
            Some("Shared")
        );
        assert!(project
            .dlls
            .iter()
            .any(|dll| dll.name == "Newtonsoft.Json.dll"));
        assert!(project
            .dlls
            .iter()
            .any(|dll| dll.name == "Unity.InputSystem.dll"));
        assert!(project
            .packages
            .iter()
            .any(|package| package.name == "com.unity.inputsystem"));
    }

    #[test]
    fn finds_the_owning_project_and_uses_open_buffers() {
        let temp = fixture();
        let game = temp.path().join("Assets/Game/Game.asmdef");
        let mut open = HashMap::new();
        open.insert(
            normalize_path(&game),
            r#"{ "name": "Game.Edited" }"#.to_string(),
        );
        let index = build(&[temp.path().to_path_buf()], &open);
        assert_eq!(
            index.project_for_path(&game).unwrap().root,
            normalize_path(temp.path())
        );
        assert_eq!(
            index.projects[0].find_by_name("Game.Edited")[0].name(),
            Some("Game.Edited")
        );
        assert!(index.projects[0].find_by_name("Game.Runtime").is_empty());
    }

    #[test]
    fn replaces_and_removes_files() {
        let temp = fixture();
        let mut index = build(&[temp.path().to_path_buf()], &HashMap::new());
        let game = normalize_path(&temp.path().join("Assets/Game/Game.asmdef"));

        assert!(index.replace_file(&game, r#"{ "name": "Game.Renamed" }"#));
        assert!(index.projects[0].find_by_name("Game.Renamed").len() == 1);
        assert!(index.projects[0].find_by_name("Game.Runtime").is_empty());

        assert!(index.remove_file(&game));
        assert!(index.projects[0].find_by_name("Game.Renamed").is_empty());
    }

    #[test]
    fn committed_fixture_covers_lsp_cases() {
        let root = normalize_path(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/lsp-project"),
        );
        let index = build(std::slice::from_ref(&root), &HashMap::new());
        assert_eq!(index.projects.len(), 1);
        let project = &index.projects[0];

        for name in [
            "Game.Runtime",
            "Game.Editor",
            "Shared",
            "Unity.InputSystem",
            "Cycle.A",
            "Cycle.B",
            "Duplicate",
            "Missing.Ref",
            "Bom.Assembly",
        ] {
            assert!(
                !project.find_by_name(name).is_empty(),
                "fixture is missing {name}"
            );
        }
        assert!(project.find_by_name("Unity.InputSystem")[0].external);
        assert!(project
            .dlls
            .iter()
            .any(|dll| dll.name == "Newtonsoft.Json.dll"));
        assert!(project
            .packages
            .iter()
            .any(|package| package.name == "com.unity.inputsystem"));

        let broken = project
            .file(&normalize_path(&root.join("Assets/Broken/Broken.asmdef")))
            .expect("broken fixture");
        assert!(broken
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "asmdef.parse-error"));

        let diagnostics = crate::diagnostics::compute(&index);
        let codes_for = |path: &Path| -> Vec<String> {
            let url = tower_lsp::lsp_types::Url::from_file_path(path).unwrap();
            diagnostics
                .get(&url)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| match &item.code {
                            Some(tower_lsp::lsp_types::NumberOrString::String(code)) => {
                                Some(code.clone())
                            }
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };

        assert!(codes_for(&root.join("Assets/Cycles/CycleA.asmdef"))
            .contains(&"asmdef.reference-cycle".to_string()));
        assert!(codes_for(&root.join("Assets/Duplicates/Duplicate1.asmdef"))
            .contains(&"asmdef.duplicate-name".to_string()));
        assert!(codes_for(&root.join("Assets/MissingRef.asmdef"))
            .contains(&"asmdef.unresolved-reference".to_string()));
        assert!(codes_for(&root.join("Assets/Refs/ByGuid.asmref")).is_empty());
        assert!(codes_for(&root.join("Assets/Bom.asmdef")).is_empty());
    }

    #[test]
    fn falls_back_to_workspace_root_without_project_marker() {
        let temp = tempfile::tempdir().unwrap();
        write(
            &temp.path().join("Assets/Game.asmdef"),
            r#"{ "name": "Game" }"#,
        );
        let index = build(&[temp.path().to_path_buf()], &HashMap::new());
        assert_eq!(index.projects.len(), 1);
        assert_eq!(
            index.projects[0].find_by_name("Game")[0].name(),
            Some("Game")
        );
    }
}
