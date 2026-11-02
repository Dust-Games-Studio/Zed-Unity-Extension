# Unity Assembly Definition LSP — Implementation Plan

**Spec:** `docs/superpowers/specs/2026-11-02-asmdef-lsp-design.md`
**Branch:** `feat/asmdef-lsp`
**Release:** `0.2.0`

## Global constraints

- TDD: every production behavior is preceded by a failing test that was watched fail.
- No bundled server binary in the extension repository.
- No C#, UXML, or USS server registration.
- Keep existing grammar/language/snippet/task behavior unchanged.
- Run `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
  before the final commit.

## Task 1: Cargo workspace and extension client

**Files:** `Cargo.toml`, `src/lib.rs`, `extension.toml`, `.gitignore`

**Steps**

1. Add a failing test for platform-to-asset mapping and checksum-file parsing in
   `src/lib.rs`.
2. Run `cargo test --lib`; expect failure because the functions do not exist.
3. Implement the extension package, `language_server_command`, PATH/settings
   resolution, pinned release download, SHA-256 verification, executable bit, and
   versioned install directory.
4. Add `[lib]` and `[language_servers.unity-asmdef]` to `extension.toml`; bump to
   `0.2.0`.
5. Run `cargo test --lib`; expect pass.
6. Build for `wasm32-wasip2` and expect success.

**Produces:** `UnityExtension`, `asset_name`, `parse_sha256sums`, `sha256_file`.

## Task 2: LSP crate scaffold, platforms, paths, text positions

**Files:** `lsp/Cargo.toml`, `lsp/src/main.rs`, `lsp/src/platforms.rs`,
`lsp/src/paths.rs`, `lsp/src/context.rs`

**Steps**

1. Add failing tests for `LineIndex` UTF-16 conversion, excluded-directory
   filtering, project-root detection, and the platform list.
2. Run `cargo test -p unity-asmdef-lsp`; expect failure.
3. Implement the crate, `--version`, `--check` dispatch, `LineIndex`, path helpers,
   excluded-directory rules, Unity project detection, and the platform list.
4. Run `cargo test -p unity-asmdef-lsp`; expect pass.

**Produces:** `LineIndex::{offset_to_position, position_to_offset}`,
`paths::{is_excluded, is_unity_project, discover_unity_projects}`,
`platforms::PLATFORMS`.

## Task 3: asmdef/asmref parser and schema diagnostics

**Files:** `lsp/src/parse.rs`

**Steps**

1. Add failing tests for:
   - valid asmdef extraction with spans;
   - valid asmref extraction;
   - malformed JSON diagnostic with a range;
   - missing name;
   - wrong field types;
   - malformed version define;
   - platform conflict warning;
   - `.meta` GUID parsing.
2. Run `cargo test -p unity-asmdef-lsp parse`; expect failure.
3. Implement strict `jsonc-parser` parsing, field extraction, span capture, schema
   validation, and meta GUID reading.
4. Run `cargo test -p unity-asmdef-lsp parse`; expect pass.

**Produces:** `parse::{ParsedFile, AsmdefData, AsmrefData, RawDiagnostic, parse_file,
read_guid}`.

## Task 4: Project/package index

**Files:** `lsp/src/index.rs`, `lsp/src/packages.rs`

**Steps**

1. Add failing tests using a temporary fixture tree for:
   - project asmdefs and asmrefs;
   - external package assemblies from `Library/PackageCache`;
   - DLL discovery;
   - manifest/lock package names;
   - name/GUID lookup;
   - longest-project-root ownership.
2. Run `cargo test -p unity-asmdef-lsp index`; expect failure.
3. Implement discovery, scanning, package selection, DLL indexing, and lookup maps.
4. Run `cargo test -p unity-asmdef-lsp index`; expect pass.

**Produces:** `index::{WorkspaceIndex, ProjectIndex, AssemblyDef, AsmRef, DllFile,
build, replace_file}`.

## Task 5: Completion context and items

**Files:** `lsp/src/context.rs`, `lsp/src/completion.rs`

**Steps**

1. Add failing tests for completion context in:
   - a `references` string;
   - an unclosed reference string;
   - an include-platform string;
   - a version-define name;
   - an object key position.
2. Add failing tests for item generation:
   - project and package names;
   - GUID-only mode for a `GUID:` prefix;
   - self exclusion;
   - text-edit ranges.
3. Run `cargo test -p unity-asmdef-lsp completion`; expect failure.
4. Implement the tolerant JSON cursor scanner and completion item generation.
5. Run `cargo test -p unity-asmdef-lsp completion`; expect pass.

**Produces:** `context::{completion_context, CompletionContext, CompletionKind}`,
`completion::completions`.

## Task 6: Diagnostics

**Files:** `lsp/src/diagnostics.rs`

**Steps**

1. Add failing tests for unresolved name/GUID references, duplicate names, cycles,
   missing asmref target, package diagnostics suppression, and diagnostic ranges.
2. Run `cargo test -p unity-asmdef-lsp diagnostics`; expect failure.
3. Implement diagnostic assembly, cycle detection, and LSP conversion.
4. Run `cargo test -p unity-asmdef-lsp diagnostics`; expect pass.

**Produces:** `diagnostics::{compute, to_lsp}`.

## Task 7: LSP server, definition, watcher

**Files:** `lsp/src/server.rs`, `lsp/src/watcher.rs`, `lsp/src/check.rs`,
`lsp/src/main.rs`, `lsp/tests/lsp_integration.rs`

**Steps**

1. Add failing unit tests for definition resolution and changed-diagnostic
   comparison.
2. Add a failing stdio integration test that initializes the server against a
   temporary fixture, requests `references` completion, requests definition, and
   asserts a malformed-file diagnostic.
3. Run `cargo test -p unity-asmdef-lsp`; expect failure.
4. Implement the `tower-lsp` backend, initialize/initialized, open/change/save/close,
   completion, definition, diagnostic publishing, watcher, and `--check`.
5. Run `cargo test -p unity-asmdef-lsp`; expect pass.

**Produces:** working `unity-asmdef-lsp` over stdio and `--check`.

## Task 8: Fixtures, scripts, CI, docs, verification

**Files:** `tests/fixtures/lsp-project/**`, `scripts/check-publish.sh`,
`.github/workflows/ci.yml`, `.github/workflows/release-lsp.yml`,
`languages/unity-assembly-definition/highlights.scm`, `README.md`,
`PUBLISHING.md`, `ROADMAP.md`

**Steps**

1. Add the fixture project and an assertion test that it contains every case.
2. Add `(ERROR) @error` to the asmdef highlights.
3. Update `check-publish.sh` to allow Rust/LSP source and reject tracked binaries,
   `target/`, and `.wasm` server artifacts.
4. Add CI and release workflows.
5. Update README/PUBLISHING/ROADMAP.
6. Run:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `cargo build --release -p unity-asmdef-lsp`
   - `cargo build --target wasm32-wasip2 --release`
   - `./scripts/validate.sh`
   - `./scripts/check-publish.sh`
   - `target/release/unity-asmdef-lsp --check "<real Unity project>"`
7. Expect all commands to pass; record any intentional warnings.

**Produces:** complete `0.2.0` implementation and release/documentation updates.
