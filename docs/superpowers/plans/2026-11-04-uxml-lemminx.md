# Unity UXML via LemMinX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `0.3.0` with LemMinX-backed tag, attribute, and enum completion for Unity UXML, completion-only by default, with pinned and verified LemMinX downloads.

**Architecture:** The existing WASM extension gains a second language-server registration. `language_server_command` dispatches by server ID; `unity-uxml` resolves a configured LemMinX, a `lemminx` on `PATH`, or downloads and verifies LemMinX `0.29.3`. `language_server_initialization_options` computes a file association to `<worktree>/UIElementsSchema/UIElements.xsd` and deep-merges user settings, with validation off by default and forced on only when the schema is missing.

**Tech Stack:** Rust 2021, Zed extension API `0.7.0`, `serde_json` (re-exported by `zed_extension_api`), SHA-256 via `sha2`, LemMinX `0.29.3`, TOML manifest.

**Spec:** `docs/superpowers/specs/2026-11-04-uxml-lemminx-design.md`

## Global Constraints

- Extension version becomes `0.3.0` in `Cargo.toml` and `extension.toml`; `lsp/Cargo.toml` stays `0.2.1`; `LSP_TAG` stays `unity-asmdef-lsp-v0.2.1`.
- LemMinX tag is exactly `0.29.3` from `redhat-developer/vscode-xml`.
- Pinned SHA-256 values, verbatim:
  - `lemminx-linux-x86_64`: `84cbbe3c31eb158799327d5f68a0dc876e81c431da596747b92eecffe956b3f2`
  - `lemminx-linux-aarch_64`: `e706d10fbda4fa354952edb604800d067518401521d4d1e491b8bee7cf4cbcdb`
  - `lemminx-osx-x86_64`: `e13b4c135f56f906a7450bc9f64f49718bf743c5c4df8003a5610e1fb8772a55`
  - `lemminx-osx-aarch_64`: `c5e9e2aee768eb69d041e9541cd0568a432ebd51c7fe9c1d7b7a1130b759475a`
  - `lemminx-win32`: `25f571c0f07d0ad76be60a86c1b864d46cf7bf94773dc21e5d5da646478aac79`
- Language server ID is `unity-uxml`; language is `Unity UXML`; language ID is `xml`.
- Validation default is disabled when the schema exists, enabled when it is missing. Never bundle or generate a schema.
- File association default is `[{"pattern":"**/*.uxml","systemId":"<root>/UIElementsSchema/UIElements.xsd"}]`, emitted even when the file is missing.
- User settings live under `lsp.unity-uxml.settings` and use the `{"xml":{...}}` shape.
- Do not modify `.github/workflows/release-lsp.yml`.
- Do not change the asmdef server's behavior or release version.
- Tests must be written first and observed failing before implementation.

## Review Focus

- A worktree with no `UIElementsSchema/UIElements.xsd`: the server must start, return no Unity completions, and publish a diagnostic that names the missing file rather than panicking or exiting.
- A user enabling validation or overriding `fileAssociations`: the computed defaults must survive the merge, and explicit arrays must replace rather than append.
- Unsupported platforms (32-bit x86) and missing release assets: the extension must return an actionable error, never panic or launch an unverified binary.
- A corrupted or partial LemMinX download: verification must fail, the download directory must be removed, and no binary may be launched.
- Unity generating the schema after the server starts: completion must recover through the watched-file event, or the documentation must tell the user to restart.

---

### Task 1: LemMinX release metadata

**Files:**
- Modify: `src/lib.rs`
- Test: `src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `const UXML_LSP_NAME: &str`, `const LEM_MINX_REPO: &str`, `const LEM_MINX_TAG: &str`, `fn lemminx_asset(platform: Os, architecture: Architecture) -> Result<&'static str>`, `fn lemminx_sha256(asset: &str) -> Result<&'static str>`.

- [ ] **Step 1: Write the failing tests**

Append these tests inside the existing `mod tests` in `src/lib.rs`:

```rust
    #[test]
    fn maps_lemminx_assets_for_supported_platforms() {
        assert_eq!(
            lemminx_asset(Os::Linux, Architecture::X8664).unwrap(),
            "lemminx-linux-x86_64"
        );
        assert_eq!(
            lemminx_asset(Os::Linux, Architecture::Aarch64).unwrap(),
            "lemminx-linux-aarch_64"
        );
        assert_eq!(
            lemminx_asset(Os::Mac, Architecture::X8664).unwrap(),
            "lemminx-osx-x86_64"
        );
        assert_eq!(
            lemminx_asset(Os::Mac, Architecture::Aarch64).unwrap(),
            "lemminx-osx-aarch_64"
        );
        assert_eq!(
            lemminx_asset(Os::Windows, Architecture::X8664).unwrap(),
            "lemminx-win32"
        );
        assert_eq!(
            lemminx_asset(Os::Windows, Architecture::Aarch64).unwrap(),
            "lemminx-win32"
        );
        assert!(lemminx_asset(Os::Linux, Architecture::X86).is_err());
    }

    #[test]
    fn records_a_checksum_for_every_lemminx_asset() {
        for asset in [
            "lemminx-linux-x86_64",
            "lemminx-linux-aarch_64",
            "lemminx-osx-x86_64",
            "lemminx-osx-aarch_64",
            "lemminx-win32",
        ] {
            let hash = lemminx_sha256(asset).unwrap();
            assert_eq!(hash.len(), 64, "{asset}");
            assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{asset}");
        }
        assert!(lemminx_sha256("lemminx-unknown").is_err());
    }
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test -p unity lemminx`

Expected: compile failure because `lemminx_asset` and `lemminx_sha256` do not exist.

- [ ] **Step 3: Add the constants and helpers**

Add after the existing `CHECKSUMS_ASSET` constant:

```rust
const UXML_LSP_NAME: &str = "unity-uxml";
const LEM_MINX_REPO: &str = "redhat-developer/vscode-xml";
const LEM_MINX_TAG: &str = "0.29.3";
const LEM_MINX_SHA256: &[(&str, &str)] = &[
    (
        "lemminx-win32",
        "25f571c0f07d0ad76be60a86c1b864d46cf7bf94773dc21e5d5da646478aac79",
    ),
    (
        "lemminx-osx-x86_64",
        "e13b4c135f56f906a7450bc9f64f49718bf743c5c4df8003a5610e1fb8772a55",
    ),
    (
        "lemminx-osx-aarch_64",
        "c5e9e2aee768eb69d041e9541cd0568a432ebd51c7fe9c1d7b7a1130b759475a",
    ),
    (
        "lemminx-linux-x86_64",
        "84cbbe3c31eb158799327d5f68a0dc876e81c431da596747b92eecffe956b3f2",
    ),
    (
        "lemminx-linux-aarch_64",
        "e706d10fbda4fa354952edb604800d067518401521d4d1e491b8bee7cf4cbcdb",
    ),
];
```

Add after `asset_name`:

```rust
fn lemminx_asset(platform: Os, architecture: Architecture) -> Result<&'static str> {
    match (platform, architecture) {
        (Os::Linux, Architecture::X8664) => Ok("lemminx-linux-x86_64"),
        (Os::Linux, Architecture::Aarch64) => Ok("lemminx-linux-aarch_64"),
        (Os::Mac, Architecture::X8664) => Ok("lemminx-osx-x86_64"),
        (Os::Mac, Architecture::Aarch64) => Ok("lemminx-osx-aarch_64"),
        (Os::Windows, Architecture::X8664 | Architecture::Aarch64) => Ok("lemminx-win32"),
        (_, Architecture::X86) => Err("unsupported architecture: 32-bit x86".to_string()),
    }
}

fn lemminx_sha256(asset: &str) -> Result<&'static str> {
    LEM_MINX_SHA256
        .iter()
        .find(|(name, _)| *name == asset)
        .map(|(_, hash)| *hash)
        .ok_or_else(|| format!("no pinned checksum for {asset}"))
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cargo test -p unity maps_lemminx_assets_for_supported_platforms records_a_checksum_for_every_lemminx_asset`

Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs
git commit -m "feat: add pinned LemMinX release metadata"
```

---

### Task 2: LemMinX installation and server dispatch

**Files:**
- Modify: `src/lib.rs`
- Test: `src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: `UXML_LSP_NAME`, `LEM_MINX_REPO`, `LEM_MINX_TAG`, `lemminx_asset`, `lemminx_sha256` from Task 1.
- Produces: `enum ServerKind`, `fn server_kind(&str) -> Option<ServerKind>`, `fn lemminx_binary_file(Os, &str) -> String`, `fn verify_sha256(&str, &str) -> Result<()>`, `fn install_lemminx(&LanguageServerId) -> Result<String>`, `fn uxml_language_server_binary_path(&mut self, &LanguageServerId, &Worktree) -> Result<String>`, and `language_server_command` routing for both servers.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests`:

```rust
    #[test]
    fn dispatches_known_language_server_ids() {
        assert_eq!(server_kind("unity-asmdef"), Some(ServerKind::Asmdef));
        assert_eq!(server_kind("unity-uxml"), Some(ServerKind::Uxml));
        assert_eq!(server_kind("unknown"), None);
    }

    #[test]
    fn names_the_windows_lemminx_executable() {
        assert_eq!(
            lemminx_binary_file(Os::Windows, "lemminx-win32"),
            "lemminx-win32.exe"
        );
        assert_eq!(
            lemminx_binary_file(Os::Linux, "lemminx-linux-x86_64"),
            "lemminx-linux-x86_64"
        );
    }

    #[test]
    fn verifies_checksums_case_insensitively() {
        assert!(verify_sha256("ABCDEF", "abcdef").is_ok());
        let error = verify_sha256("abcdef", "abcdee").unwrap_err();
        assert!(error.contains("expected abcdef"), "{error}");
        assert!(error.contains("got abcdee"), "{error}");
    }
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test -p unity dispatches_known_language_server_ids`

Expected: compile failure because the helpers do not exist.

- [ ] **Step 3: Add the struct field and dispatch helpers**

Change the struct and constructor:

```rust
struct UnityExtension {
    cached_binary_path: Option<String>,
    cached_uxml_binary_path: Option<String>,
}

impl zed::Extension for UnityExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
            cached_uxml_binary_path: None,
        }
    }
```

Add above the `impl UnityExtension` block:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerKind {
    Asmdef,
    Uxml,
}

fn server_kind(language_server_id: &str) -> Option<ServerKind> {
    match language_server_id {
        LSP_NAME => Some(ServerKind::Asmdef),
        UXML_LSP_NAME => Some(ServerKind::Uxml),
        _ => None,
    }
}

fn lemminx_binary_file(platform: Os, asset: &str) -> String {
    match platform {
        Os::Windows => format!("{asset}.exe"),
        _ => asset.to_string(),
    }
}

fn verify_sha256(expected: &str, actual: &str) -> Result<()> {
    if expected.eq_ignore_ascii_case(actual) {
        Ok(())
    } else {
        Err(format!("checksum mismatch: expected {expected}, got {actual}"))
    }
}
```

- [ ] **Step 4: Split the command handlers and add the LemMinX path**

Inside `impl UnityExtension`, rename the existing `language_server_binary_path` body into a new `asmdef_command` and keep `language_server_binary_path` as the downloader. Replace the current `language_server_command` body with:

```rust
    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        match server_kind(language_server_id.as_ref()) {
            Some(ServerKind::Asmdef) => self.asmdef_command(language_server_id, worktree),
            Some(ServerKind::Uxml) => self.uxml_command(language_server_id, worktree),
            None => Err(format!(
                "unknown language server id: {}",
                language_server_id.as_ref()
            )),
        }
    }
```

Add these methods to `impl UnityExtension`:

```rust
    fn asmdef_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(LSP_NAME, worktree)
            .ok()
            .and_then(|settings| settings.binary);
        let (path, args, env) = match binary {
            Some(binary) => (binary.path, binary.arguments, binary.env),
            None => (None, None, None),
        };
        let command = match path {
            Some(path) => path,
            None => self.language_server_binary_path(language_server_id, worktree)?,
        };

        Ok(zed::Command {
            command,
            args: args.unwrap_or_default(),
            env: env.unwrap_or_default().into_iter().collect(),
        })
    }

    fn uxml_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(UXML_LSP_NAME, worktree)
            .ok()
            .and_then(|settings| settings.binary);
        let (path, args, env) = match binary {
            Some(binary) => (binary.path, binary.arguments, binary.env),
            None => (None, None, None),
        };
        let command = match path {
            Some(path) => path,
            None => self.uxml_language_server_binary_path(language_server_id, worktree)?,
        };

        Ok(zed::Command {
            command,
            args: args.unwrap_or_default(),
            env: env.unwrap_or_default().into_iter().collect(),
        })
    }

    fn uxml_language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        if let Some(path) = worktree.which("lemminx") {
            return Ok(path);
        }

        if let Some(path) = &self.cached_uxml_binary_path {
            if is_file(path) {
                return Ok(path.clone());
            }
        }

        let path = install_lemminx(language_server_id).map_err(|error| {
            format!(
                "failed to install LemMinX {LEM_MINX_TAG} from {LEM_MINX_REPO}: {error}\n\
                 Install LemMinX yourself and put `lemminx` on PATH, or point \
                 `lsp.{UXML_LSP_NAME}.binary.path` at the binary in your Zed settings."
            )
        })?;

        self.cached_uxml_binary_path = Some(path.clone());
        Ok(path)
    }
```

- [ ] **Step 5: Add the verified install flow**

Add after `install_binary`:

```rust
fn install_lemminx(language_server_id: &LanguageServerId) -> Result<String> {
    let (platform, architecture) = zed::current_platform();
    let asset = lemminx_asset(platform, architecture)?;
    let binary_file = lemminx_binary_file(platform, asset);
    let version_dir = format!("lemminx-{LEM_MINX_TAG}");
    let binary_path = format!("{version_dir}/{binary_file}");

    if is_file(&binary_path) {
        return Ok(binary_path);
    }

    let expected = lemminx_sha256(asset)?;
    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::CheckingForUpdate,
    );

    let release = zed::github_release_by_tag_name(LEM_MINX_REPO, LEM_MINX_TAG)?;
    let zip_name = format!("{asset}.zip");
    let zip_asset = release
        .assets
        .iter()
        .find(|candidate| candidate.name == zip_name)
        .ok_or_else(|| format!("release {LEM_MINX_TAG} has no asset named {zip_name}"))?;

    zed::set_language_server_installation_status(
        language_server_id,
        &LanguageServerInstallationStatus::Downloading,
    );

    let download_dir = format!("{version_dir}.download");
    fs::remove_dir_all(&download_dir).ok();

    let result = download_and_verify_lemminx(
        &zip_asset.download_url,
        &download_dir,
        &binary_file,
        expected,
    );
    if let Err(error) = result {
        fs::remove_dir_all(&download_dir).ok();
        return Err(error);
    }

    fs::remove_dir_all(&version_dir).ok();
    fs::rename(&download_dir, &version_dir)
        .map_err(|error| format!("failed to move {download_dir} to {version_dir}: {error}"))?;
    Ok(binary_path)
}

fn download_and_verify_lemminx(
    zip_url: &str,
    download_dir: &str,
    binary_file: &str,
    expected: &str,
) -> Result<()> {
    zed::download_file(zip_url, download_dir, DownloadedFileType::Zip)
        .map_err(|error| format!("failed to download {zip_url}: {error}"))?;
    let downloaded = format!("{download_dir}/{binary_file}");
    let actual = sha256_file(&downloaded)?;
    verify_sha256(expected, &actual)?;
    zed::make_file_executable(&downloaded)?;
    Ok(())
}
```

- [ ] **Step 6: Run the tests and lint**

Run: `cargo test -p unity`

Expected: 3 passed.

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src/lib.rs
git commit -m "feat: download and launch LemMinX for UXML"
```

---

### Task 3: UXML settings, schema binding, and initialization options

**Files:**
- Modify: `src/lib.rs`
- Test: `src/lib.rs` (`mod tests`)

**Interfaces:**
- Consumes: `UXML_LSP_NAME`, `ServerKind`, `server_kind` from Tasks 1-2.
- Produces: `fn uxml_settings(&str, bool, Option<Value>) -> Value`, `fn uxml_initialization_options(Option<Value>, &str, bool, Option<Value>) -> Value`, `fn merge_value(&mut Value, Value)`, `fn resolve_relative_paths(&mut Value, &str)`, `fn absolute_path(&str, &str) -> String`, and the `language_server_initialization_options` / `language_server_workspace_configuration` overrides.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests`:

```rust
    #[test]
    fn uxml_settings_disable_validation_by_default() {
        let settings = uxml_settings("/project", true, None);
        assert_eq!(
            settings,
            json!({
                "xml": {
                    "fileAssociations": [{
                        "pattern": "**/*.uxml",
                        "systemId": "/project/UIElementsSchema/UIElements.xsd"
                    }],
                    "validation": {
                        "enabled": false,
                        "schema": { "enabled": "always" },
                        "noGrammar": "ignore"
                    }
                }
            })
        );
    }

    #[test]
    fn uxml_settings_surface_a_missing_schema() {
        let settings = uxml_settings("/project", false, None);
        assert_eq!(settings["xml"]["validation"]["enabled"], json!(true));
        assert_eq!(settings["xml"]["validation"]["noGrammar"], json!("warning"));
        assert_eq!(
            settings["xml"]["validation"]["namespaces"]["enabled"],
            json!("never")
        );
        assert_eq!(
            settings["xml"]["fileAssociations"][0]["systemId"],
            json!("/project/UIElementsSchema/UIElements.xsd")
        );
    }

    #[test]
    fn uxml_settings_merge_user_validation_without_losing_defaults() {
        let settings = uxml_settings(
            "/project",
            true,
            Some(json!({ "xml": { "validation": { "enabled": true } } })),
        );
        assert_eq!(settings["xml"]["validation"]["enabled"], json!(true));
        assert_eq!(
            settings["xml"]["validation"]["schema"]["enabled"],
            json!("always")
        );
        assert_eq!(settings["xml"]["validation"]["noGrammar"], json!("ignore"));
    }

    #[test]
    fn uxml_settings_replace_file_associations_and_resolve_paths() {
        let settings = uxml_settings(
            "/project/",
            true,
            Some(json!({
                "xml": {
                    "fileAssociations": [
                        { "pattern": "**/UI/**/*.uxml", "systemId": "schemas/ui.xsd" }
                    ],
                    "catalogs": [
                        "schemas/catalog.xml",
                        "/absolute/catalog.xml",
                        "https://example.com/catalog.xml"
                    ]
                }
            })),
        );
        assert_eq!(
            settings["xml"]["fileAssociations"],
            json!([{
                "pattern": "**/UI/**/*.uxml",
                "systemId": "/project/schemas/ui.xsd"
            }])
        );
        assert_eq!(
            settings["xml"]["catalogs"],
            json!([
                "/project/schemas/catalog.xml",
                "/absolute/catalog.xml",
                "https://example.com/catalog.xml"
            ])
        );
    }

    #[test]
    fn uxml_settings_resolve_a_windows_root() {
        let settings = uxml_settings(
            "C:\\project",
            true,
            Some(json!({
                "xml": {
                    "fileAssociations": [{
                        "pattern": "**/*.uxml",
                        "systemId": "UIElementsSchema/UIElements.xsd"
                    }]
                }
            })),
        );
        assert_eq!(
            settings["xml"]["fileAssociations"][0]["systemId"],
            json!("C:\\project/UIElementsSchema/UIElements.xsd")
        );
    }

    #[test]
    fn uxml_initialization_options_wrap_settings() {
        let settings = uxml_settings("/project", true, None);
        assert_eq!(
            uxml_initialization_options(None, "/project", true, None),
            json!({ "settings": settings })
        );
    }

    #[test]
    fn uxml_initialization_options_preserve_user_options() {
        let options = uxml_initialization_options(
            Some(json!({ "extendedClientCapabilities": { "validate": true } })),
            "/project",
            true,
            None,
        );
        assert_eq!(
            options["extendedClientCapabilities"]["validate"],
            json!(true)
        );
        assert!(options.get("settings").is_some());
    }

    #[test]
    fn uxml_initialization_options_keep_user_settings_override() {
        let user = json!({ "settings": { "xml": { "validation": { "enabled": false } } } });
        assert_eq!(
            uxml_initialization_options(Some(user.clone()), "/project", true, None),
            user
        );
    }

    #[test]
    fn leaves_uris_and_absolute_paths_alone() {
        for path in [
            "/x",
            "\\x",
            "file:///x",
            "https://example.com/x",
            "C:\\x",
        ] {
            assert_eq!(absolute_path(path, "/project"), path);
        }
        assert_eq!(
            absolute_path("schemas/a.xsd", "/project/"),
            "/project/schemas/a.xsd"
        );
    }
```

Add the `json!` macro import at the top of `mod tests` (leave the existing `use super::*;`):

```rust
    use zed_extension_api::serde_json::json;
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cargo test -p unity uxml_settings`

Expected: compile failure because the UXML settings functions do not exist.

- [ ] **Step 3: Add the settings implementation**

Add at the top of the file, with the other `use` statements:

```rust
use zed_extension_api::serde_json::{json, Value};
```

Add before `zed::register_extension!`:

```rust
fn uxml_settings(root: &str, schema_present: bool, user: Option<Value>) -> Value {
    let root = root.trim_end_matches(|character| character == '/' || character == '\\');
    let schema_path = format!("{root}/UIElementsSchema/UIElements.xsd");
    let no_grammar = if schema_present { "ignore" } else { "warning" };
    let mut settings = json!({
        "xml": {
            "fileAssociations": [
                { "pattern": "**/*.uxml", "systemId": schema_path }
            ],
            "validation": {
                "enabled": !schema_present,
                "schema": { "enabled": "always" },
                "noGrammar": no_grammar
            }
        }
    });

    if !schema_present {
        settings["xml"]["validation"]["namespaces"] = json!({ "enabled": "never" });
    }

    if let Some(Value::Object(user)) = user {
        merge_value(&mut settings, Value::Object(user));
    }

    resolve_relative_paths(&mut settings, root);
    settings
}

fn merge_value(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge_value(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn resolve_relative_paths(settings: &mut Value, root: &str) {
    let Some(xml) = settings.get_mut("xml") else {
        return;
    };

    if let Some(catalogs) = xml.get_mut("catalogs").and_then(Value::as_array_mut) {
        for catalog in catalogs.iter_mut() {
            let Some(path) = catalog.as_str() else {
                continue;
            };
            let resolved = absolute_path(path, root);
            *catalog = Value::String(resolved);
        }
    }

    if let Some(associations) = xml
        .get_mut("fileAssociations")
        .and_then(Value::as_array_mut)
    {
        for association in associations.iter_mut() {
            let Some(system_id) = association.get_mut("systemId") else {
                continue;
            };
            let Some(path) = system_id.as_str() else {
                continue;
            };
            let resolved = absolute_path(path, root);
            *system_id = Value::String(resolved);
        }
    }
}

fn is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || has_uri_scheme(path)
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn has_uri_scheme(path: &str) -> bool {
    let Some((scheme, _)) = path.split_once(':') else {
        return false;
    };
    scheme.len() > 1
        && scheme.starts_with(|character: char| character.is_ascii_alphabetic())
        && scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn absolute_path(path: &str, root: &str) -> String {
    if is_absolute(path) {
        return path.to_string();
    }
    let root = root.trim_end_matches(|character| character == '/' || character == '\\');
    let relative = path.strip_prefix("./").unwrap_or(path);
    format!("{root}/{relative}")
}

fn uxml_initialization_options(
    user_initialization: Option<Value>,
    root: &str,
    schema_present: bool,
    user_settings: Option<Value>,
) -> Value {
    let settings = uxml_settings(root, schema_present, user_settings);
    let mut options = match user_initialization {
        Some(Value::Object(options)) => Value::Object(options),
        _ => Value::Object(Default::default()),
    };
    if let Value::Object(options) = &mut options {
        options.entry("settings").or_insert(settings);
    }
    options
}
```

- [ ] **Step 4: Wire initialization options and workspace configuration**

Add these methods to `impl zed::Extension for UnityExtension`, after `language_server_command`:

```rust
    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        if server_kind(language_server_id.as_ref()) != Some(ServerKind::Uxml) {
            return Ok(None);
        }

        let settings = LspSettings::for_worktree(UXML_LSP_NAME, worktree).ok();
        let user_initialization = settings
            .as_ref()
            .and_then(|settings| settings.initialization_options.clone());
        let user_settings = settings.and_then(|settings| settings.settings);
        let root = worktree.root_path();
        let schema_present = worktree
            .read_text_file("UIElementsSchema/UIElements.xsd")
            .is_ok();

        Ok(Some(uxml_initialization_options(
            user_initialization,
            &root,
            schema_present,
            user_settings,
        )))
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        if server_kind(language_server_id.as_ref()) != Some(ServerKind::Uxml) {
            return Ok(None);
        }

        let user_settings = LspSettings::for_worktree(UXML_LSP_NAME, worktree)
            .ok()
            .and_then(|settings| settings.settings);
        let root = worktree.root_path();
        let schema_present = worktree
            .read_text_file("UIElementsSchema/UIElements.xsd")
            .is_ok();

        Ok(Some(uxml_settings(&root, schema_present, user_settings)))
    }
```

- [ ] **Step 5: Run the tests and lint**

Run: `cargo test -p unity`

Expected: all tests pass.

Run: `cargo clippy --workspace --all-targets -- -D warnings`

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs
git commit -m "feat: bind UXML to the generated schema with safe defaults"
```

---

### Task 4: Manifest, version, and documentation

**Files:**
- Modify: `extension.toml`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock` (via `cargo check`)
- Modify: `lsp/Cargo.toml` (version stays `0.2.1`; do not change it)
- Modify: `README.md`
- Modify: `ROADMAP.md`
- Modify: `PUBLISHING.md`

**Interfaces:**
- Consumes: the `unity-uxml` server ID and the behavior implemented in Tasks 1-3.
- Produces: a publishable `0.3.0` manifest and user-facing documentation.

- [ ] **Step 1: Register the server and bump the extension version**

Change `extension.toml` line 3 to:

```toml
version = "0.3.0"
```

Append after the `[language_servers.unity-asmdef]` block:

```toml
[language_servers.unity-uxml]
name = "Unity UXML Language Server"
languages = ["Unity UXML"]
language_ids = { "Unity UXML" = "xml" }
```

Change `Cargo.toml` line 3 to:

```toml
version = "0.3.0"
```

Leave `lsp/Cargo.toml` at `0.2.1` and leave `LSP_TAG` in `src/lib.rs` at
`unity-asmdef-lsp-v0.2.1`.

- [ ] **Step 2: Update the README**

Replace the closing sentence of the asmdef section:

```markdown
The extension detects Unity projects at the workspace root and in nested folders. It
only attaches the asmdef server to **Unity Assembly Definition**; C# remains fully owned
by the official C# extension. UXML completion is implemented in `0.3.0` through LemMinX;
USS completion is still deferred.
```

Add this section immediately after the asmdef section's binary-override block and before
`## Installation`:

````markdown
## UXML language server

`.uxml` files are served by LemMinX `0.29.3`, downloaded from the upstream
`redhat-developer/vscode-xml` release on first use. The extension points LemMinX at the
Unity-generated schema:

```text
<project>/UIElementsSchema/UIElements.xsd
```

Unity regenerates that schema when it imports scripts, so keep the Editor open when you
add custom `UxmlFactory`/`UxmlElement` types; LemMinX reloads the schema through its
watched-file registration.

Completion for Unity tags, attributes, and enum values is on by default. Schema
validation is **off by default** because some Unity versions generate XSD files that do
not compile cleanly. Opt in with:

```json
{
  "lsp": {
    "unity-uxml": {
      "settings": {
        "xml": {
          "validation": { "enabled": true }
        }
      }
    }
  }
}
```

The full LemMinX settings object is accepted under `lsp.unity-uxml.settings`, using the
`xml` wrapper shown above. Relative `xml.catalogs` and
`xml.fileAssociations[].systemId` paths are resolved against the project root.

If `UIElementsSchema/UIElements.xsd` is missing, the server still starts but completion
returns no Unity elements and LemMinX reports `schema_reference.4` naming the missing
file. Generate the schema by opening a UXML file in Unity, or restart the language server
after Unity creates it. Multi-Unity-project workspaces can set explicit
`xml.fileAssociations` to route each project to its own schema.

To use a local LemMinX instead:

```json
{
  "lsp": {
    "unity-uxml": {
      "binary": {
        "path": "/absolute/path/to/lemminx"
      }
    }
  }
}
```

Known upstream limit: namespaced package custom elements may be inserted without their
namespace prefix. Global-namespace project custom elements complete normally.
````

Update the manual Zed check by inserting after item 9:

```markdown
10. Open a `.uxml` in a Unity project with a generated `UIElementsSchema`; confirm tags,
    attributes, and enum values complete.
11. Temporarily rename `UIElementsSchema`; confirm the missing-schema diagnostic appears
    and the server stays running. Restore it and confirm completion returns.
12. Enable UXML validation in `lsp.unity-uxml.settings`; confirm diagnostics appear only
    after opting in.
```

Renumber the remaining items (the old 10-13 become 13-16) and keep the existing content.

- [ ] **Step 3: Update the ROADMAP**

At the top, change the current-state heading to:

```markdown
`0.3.0` is a Rust Zed extension with native language servers for assembly definitions and
UI Toolkit UXML:
```

Add after the asmdef bullet:

```markdown
- `unity-uxml`: LemMinX-backed tag, attribute, and enum completion for `.uxml`, using
  Unity's generated `UIElementsSchema/UIElements.xsd`; validation is opt-in.
```

Replace the paragraph that begins `` `0.2.1` adds `` with:

```markdown
`0.3.0` adds UXML completion through LemMinX. `0.2.1` added the navigation and
refactoring layer on top of the `0.2.0` asmdef server: find-references across assembly
files, rename across declarations and name-based references, assembly hover, reference
quick fixes, and project symbols.
```

Replace the sentence `` `0.1.0` was the declarative release... `` with:

```markdown
`0.1.0` was the declarative release. USS completion and C#-aware UXML remain deferred;
`0.3.0` attaches a language server to UXML only.
```

Change Phase 0's status line to:

```markdown
**Status: complete (2026-11-04).** Findings are in
`docs/superpowers/specs/2026-11-04-uxml-lemminx-design.md`.
```

Change Phase 1's heading block to:

```markdown
### Phase 1 — Rust/LSP foundation

**Status: implemented in `0.2.0` and extended in `0.3.0`.**
```

Change Phase 2's heading block to:

```markdown
### Phase 2 — UXML completion and validation

**Status: implemented in `0.3.0`; validation is opt-in.**
```

Replace the Phase 2 bullet that starts with `- Fall back to a bundled static schema`
with:

```markdown
- Do not bundle a static schema. If Unity has not generated
  `UIElementsSchema/UIElements.xsd`, start without Unity completion, report the missing
  file, and recover when Unity generates it.
```

Replace the last Phase 2 bullet (`- Decide whether to expose LemMinX diagnostics...`) with:

```markdown
- Keep LemMinX schema diagnostics off by default and expose them through
  `lsp.unity-uxml.settings`.
```

- [ ] **Step 4: Update PUBLISHING**

Change the identity table rows to:

```markdown
| Version | `0.3.0` in `extension.toml` |
| Bundled binaries | None; `unity-asmdef-lsp` and LemMinX are downloaded at runtime |
```

Add after the `unity-asmdef-lsp` paragraph in `### Language server distribution`:

```markdown
The UXML server is upstream LemMinX `0.29.3`, downloaded from
`redhat-developer/vscode-xml` and verified against pinned SHA-256 values in
`src/lib.rs`. It is not built by this repository and is not bundled with the extension.
The extension release does not require a new `unity-asmdef-lsp` tag when only the
extension changes.
```

Update the manual checklist by inserting after item 9:

```markdown
10. Open a `.uxml` with a generated `UIElementsSchema`; confirm tag, attribute, and enum
    completion, including a project custom element if the project has one.
11. Temporarily rename the schema directory; confirm `schema_reference.4` names the
    missing file and that Zed stays responsive. Restore it and confirm completion returns.
12. Enable `lsp.unity-uxml.settings.xml.validation.enabled` and confirm schema diagnostics
    appear only after opting in.
```

Renumber the remaining checklist items.

Replace the update instructions in `## Updating an already published extension` with:

````markdown
1. For an extension-only release, bump `version` in `extension.toml` and `Cargo.toml`;
   commit and push `main`.
2. For a language-server release, also bump `lsp/Cargo.toml` and the pinned `LSP_TAG` in
   `src/lib.rs`, then push the `unity-asmdef-lsp-v<version>` tag.
3. Push `main` to the public repository.
4. In your `zed-industries/extensions` fork:

   ```bash
   git submodule update --remote extensions/unity
   ```

5. Update the `version` in the registry's `extensions.toml` to match the new
   `extension.toml`.
6. Run `pnpm sort-extensions`, commit, push, and open a PR.
````

- [ ] **Step 5: Update the lockfile and verify the manifest**

Run: `cargo check --workspace`

Expected: succeeds and updates `Cargo.lock` so the `unity` package is `0.3.0`.

Run: `./scripts/check-publish.sh`

Expected: PASS with `2 language server(s) declared` and no version mismatch warning.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock extension.toml README.md ROADMAP.md PUBLISHING.md
git commit -m "docs: release UXML support in 0.3.0"
```

---

## Final Verification

Run from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p unity-asmdef-lsp
cargo build --target wasm32-wasip2 --release
./scripts/validate.sh all
./scripts/check-publish.sh
```

Then verify the generated settings against a real Unity project:

1. Run `cargo test -p unity uxml_settings_disable_validation_by_default -- --nocapture`.
2. Confirm the expected JSON contains `"systemId": "<root>/UIElementsSchema/UIElements.xsd"`.
3. Run the existing LemMinX probe against Game Collector with the same settings shape and
   confirm `ui:VisualElement` and `ui:Button` appear.
4. With `UIElementsSchema` renamed, confirm the probe reports `schema_reference.4` and the
   server remains alive.
5. Restore the schema and send a `workspace/didChangeWatchedFiles` notification; confirm
   completion returns without restarting.

The final review must confirm that `.github/workflows/release-lsp.yml` is unchanged.
