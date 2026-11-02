# Unity Assembly Definition LSP — Design

**Date:** 2026-11-02
**Status:** Approved for implementation
**Release:** `0.2.0`

## 1. Summary

Add a Rust language server, `unity-asmdef-lsp`, for Unity `.asmdef` and `.asmref`
files. The existing Zed extension becomes a thin Rust client that starts the server
and downloads a pinned release when the binary is not available locally.

The server provides project-aware completion, diagnostics, and go-to-definition for
assembly definitions. It does not attach to C#, UXML, or USS. Roslyn remains fully in
charge of C#.

## 2. Goals

- Complete `references` with project and package assembly names and `GUID:` forms.
- Complete `includePlatforms`/`excludePlatforms`, `versionDefines[].name`,
  `precompiledReferences`, and the asmdef schema keys.
- Detect malformed JSON and invalid asmdef/asmref structure.
- Diagnose unresolved references, duplicate assembly names, reference cycles, and
  missing asmref targets.
- Go to the declaring asmdef/asmref from a reference.
- Refresh from server-side file watching and editor notifications.
- Keep all existing language, grammar, snippet, and task behavior unchanged.
- Work without a Unity Editor and without writing project files.

## 3. Non-goals

- UXML or USS language servers.
- Generating C# constants from UXML.
- Attaching a second language server to C#.
- Replacing Roslyn or generating `.sln`/`.csproj` files.
- Editing Unity YAML or ShaderLab.
- Requiring a Unity Editor companion package.

## 4. Architecture

```text
Zed
 └─ unity extension (WASM)
     └─ launches unity-asmdef-lsp (native, stdio LSP)
         ├─ project discovery + index
         ├─ completion
         ├─ diagnostics
         ├─ definition
         └─ watcher
```

Repository layout:

```text
Cargo.toml                 # extension package + workspace
src/lib.rs                 # binary resolution/download
lsp/Cargo.toml             # unity-asmdef-lsp
lsp/src/main.rs
lsp/src/server.rs
lsp/src/index.rs
lsp/src/parse.rs
lsp/src/context.rs
lsp/src/completion.rs
lsp/src/diagnostics.rs
lsp/src/watcher.rs
lsp/src/paths.rs
lsp/src/platforms.rs
lsp/src/check.rs
```

The extension registers one server:

```toml
[language_servers.unity-asmdef]
name = "Unity Assembly Definition LSP"
languages = ["Unity Assembly Definition"]
language_ids = { "Unity Assembly Definition" = "json" }
```

No other language is registered for the server.

## 5. Project discovery and indexing

- Workspace roots come from `workspaceFolders`, then `rootUri`, then the current
  directory.
- A Unity project root contains `ProjectSettings/ProjectVersion.txt` or
  `Packages/manifest.json`. Search the workspace root and nested directories up to
  depth 3, skipping generated directories.
- Index `.asmdef` and `.asmref` under each project's `Assets/` and `Packages/`.
- Package assemblies are read-only:
  - `Packages/<package>/` is indexed with `external = true`.
  - `Library/PackageCache/<package>@*/` is indexed only for packages named by
    `Packages/manifest.json` or `packages-lock.json`.
  - `Library` is never scanned wholesale.
- Read GUIDs from sibling `.asmdef.meta` files.
- Index DLL file names from project and selected package directories.
- Excluded directories: `Library` (except selected package caches), `Temp`, `Logs`,
  `obj`, `Build`, `Builds`, `UserSettings`, `.git`, `.vs`, `.idea`, `node_modules`,
  `TestResults`.
- An open document outside a discovered project triggers ancestor discovery and a
  rebuild.

## 6. Parsing and spans

- Use `jsonc-parser` in strict JSON mode so Unity's expected format is enforced.
- `jsonc-parser` supplies byte ranges for every value.
- Store byte spans for names, references, platform entries, precompiled references,
  and version-define fields.
- Convert byte offsets to LSP UTF-16 positions with a `LineIndex`.
- A separate tolerant scanner handles completion while JSON is incomplete.
- Parse errors produce an `Error` diagnostic; schema errors produce precise
  diagnostics with stable codes.

## 7. Completion

Contexts:

- `references`: assembly names and GUID forms.
- `includePlatforms` / `excludePlatforms`: Unity platform names.
- `versionDefines[].name`: package names from manifest and lock.
- `precompiledReferences`: DLL names.
- Top-level asmdef keys and version-define keys.

Behavior:

- Project assemblies sort before package assemblies.
- The current assembly is excluded from its own references.
- If the typed prefix starts with `GUID:`, only GUID items are returned.
- Otherwise name items and GUID items are returned; GUID items filter by assembly
  name.
- Completion replaces string contents, not the surrounding quotes, using `text_edit`.
- Trigger character is `"`.
- Completion works in unclosed strings and arrays.

## 8. Diagnostics

Codes and severities:

| Code | Severity | Condition |
|---|---|---|
| `asmdef.parse-error` | Error | Strict JSON parse failure |
| `asmdef.root-not-object` | Error | Root value is not an object |
| `asmdef.missing-name` | Error | Missing or empty `name` |
| `asmdef.invalid-field` | Error | Known field has the wrong type |
| `asmdef.invalid-version-define` | Error | Malformed `versionDefines` entry |
| `asmdef.unknown-platform` | Warning | Platform not in the known list |
| `asmdef.platform-conflict` | Warning | Both include and exclude arrays are non-empty |
| `asmdef.unresolved-reference` | Error | Name/GUID reference does not resolve |
| `asmdef.duplicate-name` | Error | Two project assemblies share a name |
| `asmdef.reference-cycle` | Error | Assembly participates in a cycle |
| `asmref.missing-reference` | Error | Missing or empty `reference` |
| `asmref.unresolved-reference` | Error | asmref target does not resolve |

- Package assemblies are excluded from unresolved, duplicate, cycle, and schema
  diagnostics; they remain completion/definition sources.
- Publish diagnostics for all indexed project files with
  `textDocument/publishDiagnostics`.
- Keep the last valid index entry for an open document while its JSON is temporarily
  invalid; the parse error is still reported.
- Add `(ERROR) @error` to the asmdef highlight query so malformed syntax is also
  visible in Zed.

## 9. Go-to-definition

- From a `references` string or asmref `reference` string.
- Resolve by name or `GUID:`.
- Return all matches for duplicate names.
- Point at the target's `name` value (or the file start if unavailable).

## 10. Refresh

- Watch each project's `Assets/` and `Packages/` with `notify`.
- Debounce events; full rebuild on create/remove/meta/manifest/lock changes.
- Use `didChange` for fast per-file reindex from the buffer.
- Use `didSave`, `didClose`, and `didChangeWatchedFiles` as fallbacks.
- If watching fails, log and continue with editor-driven refresh.

## 11. Extension client and distribution

Binary resolution order:

1. `lsp.unity-asmdef.binary.path` (plus arguments/env).
2. `unity-asmdef-lsp` on `PATH`.
3. Pinned GitHub release download.

Release assets:

- `unity-asmdef-lsp-linux-x86_64`
- `unity-asmdef-lsp-linux-aarch64`
- `unity-asmdef-lsp-macos-x86_64`
- `unity-asmdef-lsp-macos-aarch64`
- `unity-asmdef-lsp-windows-x86_64.exe`
- `SHA256SUMS`

Download to a versioned extension directory, verify SHA-256, mark executable, and
clean old versions. Errors explain how to build locally or set the binary path.
Windows arm64 uses the x86_64 asset under emulation.

Local development:

```bash
cargo build -p unity-asmdef-lsp --release
cargo install --path lsp
# or set lsp.unity-asmdef.binary.path
```

## 12. Testing

- Unit tests for parsing, schema validation, GUID/meta parsing, project discovery,
  package selection, index maps, completion contexts/items, diagnostics, cycles,
  definitions, and position mapping.
- Integration tests spawn the real binary over stdio against a temporary fixture
  project and assert completion, definition, and diagnostics.
- Fixture project under `tests/fixtures/lsp-project/` with project/package
  assemblies, GUIDs, cycles, duplicates, malformed JSON, asmrefs, and DLLs.
- `unity-asmdef-lsp --check <project>` prints an index/diagnostic summary for manual
  and CI smoke tests.
- Update `scripts/check-publish.sh` to allow Rust/LSP source while rejecting tracked
  binaries and `target/`.
- CI workflow for tests; release workflow for server artifacts.

## 13. Documentation

- README: features, setup, binary override, diagnostics, package behavior, limits.
- PUBLISHING.md: Rust extension and separately downloaded server.
- ROADMAP.md: Phase 3 implemented; UXML/USS/C# intentionally skipped.
- Keep the existing language support, tasks, and snippets unchanged.

## 14. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Package cache scan is slow | Only scan package directories named by manifest/lock; never walk `Library` wholesale. |
| Platform list drifts across Unity versions | Include both legacy and current names; report unknown names as warnings. |
| Release checksums do not exist before the first tag | Local PATH/settings override remains fully supported; download errors are actionable. |
| Watcher behavior differs by platform | Editor save/change/close handlers provide a fallback; watcher failures are non-fatal. |
| Open document is temporarily invalid JSON | Keep the last valid index entry and report the parse error. |
| Strict JSON rejects something Unity accepts | Unity documents strict JSON; if a real project proves otherwise, relax the parser deliberately. |
