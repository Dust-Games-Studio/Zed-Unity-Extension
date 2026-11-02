use std::path::{Path, PathBuf};

pub const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".idea",
    ".vs",
    "Build",
    "Builds",
    "Library",
    "Logs",
    "Temp",
    "TestResults",
    "UserSettings",
    "node_modules",
    "obj",
];

pub fn is_excluded_dir(name: &str) -> bool {
    EXCLUDED_DIRS
        .iter()
        .any(|excluded| name.eq_ignore_ascii_case(excluded))
}

pub fn is_unity_project(path: &Path) -> bool {
    path.join("ProjectSettings/ProjectVersion.txt").is_file()
        || path.join("Packages/manifest.json").is_file()
}

pub fn discover_unity_projects(workspace_roots: &[PathBuf], max_depth: usize) -> Vec<PathBuf> {
    let mut projects = Vec::new();

    for root in workspace_roots {
        if is_unity_project(root) {
            projects.push(root.clone());
            continue;
        }

        let walker = walkdir::WalkDir::new(root)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| !is_excluded_dir(name))
                    .unwrap_or(false)
            });

        for entry in walker.flatten() {
            if entry.file_type().is_dir() && is_unity_project(entry.path()) {
                projects.push(entry.path().to_path_buf());
            }
        }
    }

    projects.sort();
    projects.dedup();
    projects
}

pub fn project_root_for_file(file: &Path) -> Option<PathBuf> {
    let start = if file.is_dir() { file } else { file.parent()? };

    for ancestor in start.ancestors() {
        if is_unity_project(ancestor) {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

pub fn is_asmdef_or_asmref(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("asmdef" | "asmref")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn identifies_unity_projects() {
        let temp = tempfile::tempdir().unwrap();
        assert!(!is_unity_project(temp.path()));

        fs::create_dir_all(temp.path().join("ProjectSettings")).unwrap();
        fs::write(
            temp.path().join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 6000.6.6f1",
        )
        .unwrap();
        assert!(is_unity_project(temp.path()));

        let nested = temp.path().join("Packages");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("manifest.json"), "{}").unwrap();
        let file = temp.path().join("Assets/Game/Game.asmdef");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "{}").unwrap();
        assert_eq!(
            project_root_for_file(&file),
            Some(temp.path().to_path_buf())
        );
    }

    #[test]
    fn discovers_nested_projects_and_skips_generated_directories() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("Games/One");
        fs::create_dir_all(project.join("ProjectSettings")).unwrap();
        fs::write(project.join("ProjectSettings/ProjectVersion.txt"), "x").unwrap();

        let ignored = temp.path().join("Library/Nested");
        fs::create_dir_all(ignored.join("ProjectSettings")).unwrap();
        fs::write(ignored.join("ProjectSettings/ProjectVersion.txt"), "x").unwrap();

        let projects = discover_unity_projects(&[temp.path().to_path_buf()], 4);
        assert_eq!(projects, vec![project]);
    }

    #[test]
    fn filters_generated_directory_names() {
        assert!(is_excluded_dir("Library"));
        assert!(is_excluded_dir("library"));
        assert!(is_excluded_dir("obj"));
        assert!(!is_excluded_dir("Assets"));
        assert!(is_asmdef_or_asmref(Path::new("x.asmdef")));
        assert!(is_asmdef_or_asmref(Path::new("x.asmref")));
        assert!(!is_asmdef_or_asmref(Path::new("x.meta")));
    }
}
