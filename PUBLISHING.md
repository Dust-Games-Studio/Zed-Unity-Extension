# Publishing the Unity Extension

This document is the release checklist for submitting the extension to the
[Zed extension registry](https://zed.dev/docs/extensions/publishing/overview).

## Current identity

| Field | Value |
|---|---|
| Extension ID | `unity` |
| Registry submodule | `extensions/unity` |
| Repository | `https://github.com/Dust-Games-Studio/Zed-Unity-Extension` |
| License | MIT, at the repository root |
| Version | `0.2.1` in `extension.toml` |
| Rust extension | `Cargo.toml`, `src/`, and the `lsp/` workspace member |
| Bundled binaries | None; `unity-asmdef-lsp` is downloaded from a GitHub release at runtime |

The ID `unity` is not used by another extension in `zed-industries/extensions`.
`unity-debugger` and `unity-snippets` are separate IDs.

## Before opening the registry PR

Run these from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p unity-asmdef-lsp
cargo build --target wasm32-wasip2 --release
./scripts/validate.sh all
./scripts/check-publish.sh --strict
```

`validate.sh` checks the grammars, fixtures, language configs, and queries.
`check-publish.sh` checks the registry prerequisites: manifest fields, license,
grammar pins, language directories, tasks files, Rust extension structure, and the
tracked file tree. It also rejects bundled native binaries and prints the exact
`[unity]` entry for the registry PR.

Then complete the manual Zed check at the exact commit you will submit:

1. Run `zed: install dev extension` and select this repository.
2. Open each file in `tests/fixtures/` and confirm the language mode, highlighting,
   outline, comments, brackets, and indentation.
3. Open an `.asmdef`, trigger completion in `references`, and confirm project and package
   assembly names and `GUID:` forms appear.
4. Use go-to-definition on a reference and confirm it opens the target assembly.
5. Use find-references on an assembly name and confirm references from other
   `.asmdef`/`.asmref` files appear.
6. Hover an assembly name and confirm path, GUID, origin, and referencing count.
7. Use rename on a project assembly and confirm the declaration and name-based references
   appear in the preview; confirm a package assembly refuses the rename.
8. Place the cursor on an unresolved reference and confirm the remove, nearest-name, and
   `GUID:`/name conversion quick fixes appear where applicable.
9. Open project symbols and confirm project and package assemblies are listed.
10. Open `tests/fixtures/lsp-project/Assets/Broken/Broken.asmdef` and confirm the parse
    diagnostic; confirm malformed asmdef syntax also highlights as an error.
11. Confirm `.cs` still uses the official C# extension and no second C# server starts.
12. Run `task: spawn` from a Unity file and confirm all five Unity tasks appear.
13. Run `zed: open log` and confirm there are no grammar, query, language registration,
    or server startup errors.
14. Confirm `@error` is visible in your theme for the invalid UXML/USS fixtures.

Do not submit until `main` is clean, pushed, and the checked-out commit is on a
branch. The registry uses the checked-out submodule commit, and detached commits
are rejected.

### Language server distribution

The registry permits a Rust extension that provides language servers. It must not
bundle the server binary. This repository keeps `unity-asmdef-lsp` in the `lsp/`
workspace member and downloads a pinned release at runtime.

Publish the server first, using a tag matching the version in `src/lib.rs`:

```text
unity-asmdef-lsp-v0.2.1
```

The release workflow builds these assets and generates `SHA256SUMS`:

- `unity-asmdef-lsp-linux-x86_64`
- `unity-asmdef-lsp-linux-aarch64`
- `unity-asmdef-lsp-macos-x86_64`
- `unity-asmdef-lsp-macos-aarch64`
- `unity-asmdef-lsp-windows-x86_64.exe`

The extension verifies the downloaded binary against `SHA256SUMS` before marking it
executable. Users can always bypass the download with `lsp.unity-asmdef.binary.path`
or a `unity-asmdef-lsp` binary on `PATH`.

### Snippets

The Unity C# snippets under `snippets/` are registered in `extension.toml` and
ship with the extension. Each file is named `csharp.json`, so Zed scopes it to the
C# language. `check-publish.sh` verifies that every tracked snippet file is
registered and that every registered file is tracked.

The registry already lists `unity-snippets` as a separate extension. This extension
intentionally includes the Unity snippets so users do not need two extensions. If the
maintainers ask about the overlap, explain the consolidation or contribute the snippets
upstream.

## Submit to `zed-industries/extensions`

1. Fork `https://github.com/zed-industries/extensions` to a personal GitHub
   account (recommended by the Zed maintainers so they can push fixes to the PR).
2. Clone your fork and initialize its submodules:

   ```bash
   git clone https://github.com/YOUR-USERNAME/extensions
   cd extensions
   git submodule init
   git submodule update
   ```

3. Add this repository as a submodule using HTTPS, not SSH:

   ```bash
   git submodule add https://github.com/Dust-Games-Studio/Zed-Unity-Extension extensions/unity
   git add extensions/unity
   ```

4. Add this entry to the top-level `extensions.toml`:

   ```toml
   [unity]
   submodule = "extensions/unity"
   version = "0.2.1"
   ```

   The `version` must exactly match `version` in this repository's
   `extension.toml` at the submitted submodule commit.

5. Sort the registry files:

   ```bash
   pnpm sort-extensions
   ```

6. Commit, push your fork, and open a pull request against
   `zed-industries/extensions`. Follow the registry PR rules:

   - exactly one extension per PR;
   - at most three open PRs at a time;
   - respond to maintainer feedback within three weeks, or the PR is closed.

After the PR is merged, the extension is packaged and published to the registry.
Update the "pending" note in the README installation section at that point.

## Updating an already published extension

1. Bump `version` in `extension.toml`, `Cargo.toml`, and `lsp/Cargo.toml`; update the
   pinned `LSP_TAG` in `src/lib.rs`; commit the release to `main`.
2. Push the `unity-asmdef-lsp-v<version>` tag so the server release is published.
3. Push `main` to the public repository.
4. In your `zed-industries/extensions` fork:

   ```bash
   git submodule update --remote extensions/unity
   ```

5. Update the `version` in the registry's `extensions.toml` to match the new
   `extension.toml`.
6. Run `pnpm sort-extensions`, commit, push, and open a PR.

Update PRs follow the same rules as the initial submission.

## Reference

- [Installing extensions](https://zed.dev/docs/extensions/installing-extensions)
- [Publishing overview](https://zed.dev/docs/extensions/publishing/overview)
- [Publishing prerequisites](https://zed.dev/docs/extensions/publishing/prerequisites)
- [Publishing guide](https://zed.dev/docs/extensions/publishing/publishing-guide)
- [Updating an extension](https://zed.dev/docs/extensions/publishing/updating-and-maintenance)
- [License requirements](https://zed.dev/docs/extensions/publishing/license-requirements)
- [Publishing FAQ](https://zed.dev/docs/extensions/publishing/faq)
