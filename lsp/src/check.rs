use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString};

use crate::diagnostics;
use crate::index;

pub fn run(project_path: &Path) -> Result<bool> {
    let roots = vec![project_path.to_path_buf()];
    let index = index::build(&roots, &HashMap::new());

    if index.projects.is_empty() {
        println!("No Unity project found under {}", project_path.display());
        return Ok(true);
    }

    for project in &index.projects {
        let project_assemblies = project.assemblies().filter(|file| !file.external).count();
        let package_assemblies = project.assemblies().filter(|file| file.external).count();
        println!("Unity project: {}", project.root.display());
        println!("  assemblies: {project_assemblies} project, {package_assemblies} package");
        println!("  packages: {}", project.packages.len());
        println!("  DLLs: {}", project.dlls.len());
    }

    let diagnostics = diagnostics::compute(&index);
    let mut urls: Vec<_> = diagnostics.keys().cloned().collect();
    urls.sort();

    let mut count = 0;
    let mut has_errors = false;
    for url in urls {
        let Some(path) = url.to_file_path().ok() else {
            continue;
        };
        let Some(file_diagnostics) = diagnostics.get(&url) else {
            continue;
        };
        for diagnostic in file_diagnostics {
            count += 1;
            if diagnostic.severity == Some(DiagnosticSeverity::ERROR) {
                has_errors = true;
            }
            let code = match &diagnostic.code {
                Some(NumberOrString::String(code)) => code.as_str(),
                Some(NumberOrString::Number(code)) => {
                    print!(
                        "{}:{}:{}: ",
                        path.display(),
                        diagnostic.range.start.line + 1,
                        diagnostic.range.start.character + 1
                    );
                    println!("[{code}] {}", diagnostic.message);
                    continue;
                }
                None => "",
            };
            let severity = match diagnostic.severity {
                Some(DiagnosticSeverity::ERROR) => "error",
                Some(DiagnosticSeverity::WARNING) => "warning",
                Some(DiagnosticSeverity::INFORMATION) => "info",
                Some(DiagnosticSeverity::HINT) => "hint",
                _ => "diagnostic",
            };
            println!(
                "{}:{}:{}: {severity} [{code}] {}",
                path.display(),
                diagnostic.range.start.line + 1,
                diagnostic.range.start.character + 1,
                diagnostic.message
            );
        }
    }

    println!("Diagnostics: {count}");
    Ok(has_errors)
}
