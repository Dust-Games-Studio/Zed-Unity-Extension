use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidChangeWatchedFilesParams, DidChangeWatchedFilesRegistrationOptions,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, FileSystemWatcher, GlobPattern, GotoDefinitionParams,
    GotoDefinitionResponse, InitializeParams, InitializeResult, InitializedParams, MessageType,
    OneOf, Registration, ServerCapabilities, ServerInfo, TextDocumentSyncCapability,
    TextDocumentSyncKind, TextDocumentSyncOptions, TextDocumentSyncSaveOptions, Url, WatchKind,
    WorkspaceFoldersServerCapabilities, WorkspaceServerCapabilities,
};
use tower_lsp::{Client, LanguageServer};

use crate::completion;
use crate::context::LineIndex;
use crate::definition;
use crate::diagnostics;
use crate::index::{self, WorkspaceIndex};
use crate::paths;
use crate::watcher::{self, WatcherHandle};

pub struct Backend {
    inner: Arc<Inner>,
}

struct Inner {
    client: Client,
    state: RwLock<State>,
    update_lock: Mutex<()>,
}

#[derive(Default)]
struct State {
    roots: Vec<PathBuf>,
    index: WorkspaceIndex,
    open_documents: HashMap<PathBuf, OpenDocument>,
    published: HashMap<Url, Vec<tower_lsp::lsp_types::Diagnostic>>,
    watcher: Option<WatcherHandle>,
    supports_watched_files: bool,
}

struct OpenDocument {
    text: String,
    version: i32,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            inner: Arc::new(Inner {
                client,
                state: RwLock::new(State::default()),
                update_lock: Mutex::new(()),
            }),
        }
    }
}

impl Inner {
    async fn open_document(&self, path: PathBuf, text: String, version: i32) -> bool {
        let path = index::normalize_path(&path);
        let mut added_root = false;
        {
            let mut state = self.state.write().await;
            if state.index.project_for_path(&path).is_none() {
                if let Some(root) = paths::project_root_for_file(&path) {
                    if !state.roots.contains(&root) {
                        state.roots.push(root);
                        added_root = true;
                    }
                }
            }
            state.open_documents.insert(
                path.clone(),
                OpenDocument {
                    text: text.clone(),
                    version,
                },
            );
        }

        if added_root {
            self.refresh_all().await;
        } else {
            self.apply_text(path, text).await;
        }
        added_root
    }

    async fn change_document(&self, path: PathBuf, text: String, version: i32) {
        let path = index::normalize_path(&path);
        {
            let mut state = self.state.write().await;
            state.open_documents.insert(
                path.clone(),
                OpenDocument {
                    text: text.clone(),
                    version,
                },
            );
        }
        self.apply_text(path, text).await;
    }

    async fn save_document(&self, path: PathBuf) {
        let path = index::normalize_path(&path);
        let text = {
            let state = self.state.read().await;
            state
                .open_documents
                .get(&path)
                .map(|document| document.text.clone())
        }
        .or_else(|| std::fs::read_to_string(&path).ok());

        if let Some(text) = text {
            self.apply_text(path, text).await;
        }
    }

    async fn close_document(&self, path: PathBuf) {
        let path = index::normalize_path(&path);
        let text = {
            let mut state = self.state.write().await;
            state.open_documents.remove(&path);
            std::fs::read_to_string(&path).ok()
        };

        let guard = self.update_lock.lock().await;
        let changed = {
            let mut state = self.state.write().await;
            match text {
                Some(text) => state.index.replace_file(&path, &text),
                None => state.index.remove_file(&path),
            }
        };
        if changed {
            self.publish_diagnostics().await;
            drop(guard);
        } else {
            drop(guard);
            self.refresh_all().await;
        }
    }

    async fn apply_text(&self, path: PathBuf, text: String) {
        let guard = self.update_lock.lock().await;
        let reindexed = {
            let mut state = self.state.write().await;
            state.index.replace_file(&path, &text)
        };
        if reindexed {
            self.publish_diagnostics().await;
            drop(guard);
        } else {
            drop(guard);
            self.refresh_all().await;
        }
    }

    async fn refresh_all(&self) {
        let _guard = self.update_lock.lock().await;
        let (roots, open_documents) = {
            let state = self.state.read().await;
            let open_documents: HashMap<PathBuf, String> = state
                .open_documents
                .iter()
                .map(|(path, document)| (path.clone(), document.text.clone()))
                .collect();
            (state.roots.clone(), open_documents)
        };

        let index = tokio::task::spawn_blocking(move || index::build(&roots, &HashMap::new()))
            .await
            .unwrap_or_default();

        {
            let mut state = self.state.write().await;
            state.index = index;
            // Overlay open buffers after the disk scan. `replace_file` retains the
            // last valid parse for a buffer that is temporarily invalid JSON.
            for (path, text) in open_documents {
                state.index.replace_file(&path, &text);
            }
        }
        self.publish_diagnostics().await;
    }

    async fn publish_diagnostics(&self) {
        let (changed, current) = {
            let state = self.state.read().await;
            let current = diagnostics::compute(&state.index);
            let mut changed = Vec::new();

            for (url, diagnostics) in &current {
                if state.published.get(url) != Some(diagnostics) {
                    let version = url
                        .to_file_path()
                        .ok()
                        .map(|path| index::normalize_path(&path))
                        .and_then(|path| {
                            state
                                .open_documents
                                .get(&path)
                                .map(|database| database.version)
                        });
                    changed.push((url.clone(), diagnostics.clone(), version));
                }
            }
            for url in state.published.keys() {
                if !current.contains_key(url) {
                    changed.push((url.clone(), Vec::new(), None));
                }
            }
            (changed, current)
        };

        for (url, diagnostics, version) in changed {
            self.client
                .publish_diagnostics(url, diagnostics, version)
                .await;
        }

        self.state.write().await.published = current;
    }

    async fn start_watcher(self: &Arc<Self>) {
        let directories = {
            let state = self.state.read().await;
            let mut directories = Vec::new();
            for project in &state.index.projects {
                for suffix in ["Assets", "Packages"] {
                    let directory = project.root.join(suffix);
                    if directory.is_dir() {
                        directories.push(directory);
                    }
                }
            }
            directories.sort();
            directories.dedup();
            directories
        };

        if directories.is_empty() {
            return;
        }

        let (handle, mut receiver) = match watcher::watch(directories) {
            Ok(watched) => watched,
            Err(error) => {
                log::warn!("file watcher could not start: {error}");
                return;
            }
        };

        {
            let mut state = self.state.write().await;
            state.watcher = Some(handle);
        }

        let inner = Arc::clone(self);
        tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                tokio::time::sleep(Duration::from_millis(250)).await;
                while receiver.try_recv().is_ok() {}
                inner.refresh_all().await;
            }
        });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let roots = workspace_roots(&params);
        let index = tokio::task::spawn_blocking({
            let roots = roots.clone();
            move || index::build(&roots, &HashMap::new())
        })
        .await
        .unwrap_or_default();

        let supports_watched_files = params
            .capabilities
            .workspace
            .and_then(|workspace| workspace.did_change_watched_files)
            .and_then(|watched| watched.dynamic_registration)
            .unwrap_or(false);
        {
            let mut state = self.inner.state.write().await;
            state.roots = roots;
            state.index = index;
            state.supports_watched_files = supports_watched_files;
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        will_save: Some(false),
                        will_save_wait_until: Some(false),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec!["\"".to_string()]),
                    ..CompletionOptions::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "unity-asmdef-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {
        self.inner.publish_diagnostics().await;
        self.inner.start_watcher().await;

        if self.inner.state.read().await.supports_watched_files {
            let client = self.inner.client.clone();
            tokio::spawn(async move {
                let registration = watched_files_registration();
                match tokio::time::timeout(
                    Duration::from_secs(2),
                    client.register_capability(vec![registration]),
                )
                .await
                {
                    Ok(Ok(())) => log::info!("registered workspace/didChangeWatchedFiles"),
                    Ok(Err(error)) => {
                        log::warn!("failed to register watched-file notifications: {error}")
                    }
                    Err(_) => log::warn!("timed out registering watched-file notifications"),
                }
            });
        }

        let summary = {
            let state = self.inner.state.read().await;
            let projects = state.index.projects.len();
            let assemblies: usize = state
                .index
                .projects
                .iter()
                .flat_map(|project| project.assemblies())
                .filter(|file| !file.external)
                .count();
            format!("Indexed {projects} Unity project(s), {assemblies} project assemblies.")
        };
        self.inner
            .client
            .log_message(MessageType::INFO, summary)
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        self.inner.state.write().await.watcher = None;
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if let Some(path) = uri_to_path(&params.text_document.uri) {
            let added_root = self
                .inner
                .open_document(
                    path,
                    params.text_document.text,
                    params.text_document.version,
                )
                .await;
            if added_root {
                self.inner.start_watcher().await;
            }
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(path) = uri_to_path(&params.text_document.uri) {
            if let Some(change) = params.content_changes.into_iter().last() {
                self.inner
                    .change_document(path, change.text, params.text_document.version)
                    .await;
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(path) = uri_to_path(&params.text_document.uri) {
            self.inner.save_document(path).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        if let Some(path) = uri_to_path(&params.text_document.uri) {
            self.inner.close_document(path).await;
        }
    }

    async fn did_change_watched_files(&self, _params: DidChangeWatchedFilesParams) {
        self.inner.refresh_all().await;
    }

    async fn did_change_workspace_folders(&self, params: DidChangeWorkspaceFoldersParams) {
        {
            let mut state = self.inner.state.write().await;
            for removed in params.event.removed {
                if let Ok(path) = removed.uri.to_file_path() {
                    let path = index::normalize_path(&path);
                    state.roots.retain(|root| root != &path);
                }
            }
            for added in params.event.added {
                if let Ok(path) = added.uri.to_file_path() {
                    let path = index::normalize_path(&path);
                    if !state.roots.contains(&path) {
                        state.roots.push(path);
                    }
                }
            }
        }
        self.inner.refresh_all().await;
        self.inner.start_watcher().await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let Some(path) = uri_to_path(&params.text_document_position.text_document.uri) else {
            return Ok(None);
        };
        let path = index::normalize_path(&path);
        let state = self.inner.state.read().await;
        let Some(project) = state.index.project_for_path(&path) else {
            return Ok(None);
        };
        let Some(text) = state
            .open_documents
            .get(&path)
            .map(|document| document.text.clone())
            .or_else(|| std::fs::read_to_string(&path).ok())
        else {
            return Ok(None);
        };

        let line_index = LineIndex::new(&text);
        let offset = line_index.position_to_offset(&text, params.text_document_position.position);
        let items = completion::completions(project, &path, &text, offset);
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let Some(path) = uri_to_path(&params.text_document_position_params.text_document.uri)
        else {
            return Ok(None);
        };
        let path = index::normalize_path(&path);
        let state = self.inner.state.read().await;
        let Some(project) = state.index.project_for_path(&path) else {
            return Ok(None);
        };
        let Some(file) = project.file(&path) else {
            return Ok(None);
        };
        let Some(text) = state
            .open_documents
            .get(&path)
            .map(|document| document.text.clone())
            .or_else(|| std::fs::read_to_string(&path).ok())
        else {
            return Ok(None);
        };

        let line_index = LineIndex::new(&text);
        let offset =
            line_index.position_to_offset(&text, params.text_document_position_params.position);
        let locations = definition::locations(project, file, offset);
        if locations.is_empty() {
            Ok(None)
        } else {
            Ok(Some(GotoDefinitionResponse::Array(locations)))
        }
    }
}

fn workspace_roots(params: &InitializeParams) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(folders) = &params.workspace_folders {
        for folder in folders {
            if let Ok(path) = folder.uri.to_file_path() {
                roots.push(path);
            }
        }
    }
    if roots.is_empty() {
        if let Some(uri) = &params.root_uri {
            if let Ok(path) = uri.to_file_path() {
                roots.push(path);
            }
        }
    }
    if roots.is_empty() {
        if let Ok(path) = std::env::current_dir() {
            roots.push(path);
        }
    }
    roots
}

fn uri_to_path(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

fn watched_files_registration() -> Registration {
    Registration {
        id: "unity-asmdef-watched-files".to_string(),
        method: "workspace/didChangeWatchedFiles".to_string(),
        register_options: Some(
            serde_json::to_value(DidChangeWatchedFilesRegistrationOptions {
                watchers: vec![FileSystemWatcher {
                    glob_pattern: GlobPattern::String("**/*.{asmdef,asmref,meta,json}".to_string()),
                    kind: Some(WatchKind::Create | WatchKind::Change | WatchKind::Delete),
                }],
            })
            .expect("watched-file registration options serialize"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_files_registration_targets_index_inputs() {
        let registration = watched_files_registration();
        assert_eq!(registration.method, "workspace/didChangeWatchedFiles");
        let options = registration.register_options.unwrap();
        assert_eq!(
            options["watchers"][0]["globPattern"],
            "**/*.{asmdef,asmref,meta,json}"
        );
    }
}
