# Unity UXML via LemMinX — Design

**Date:** 2026-11-04
**Status:** Approved for implementation
**Release:** `0.3.0`

## 1. Summary

Attach the upstream LemMinX XML language server to Unity UXML files. The extension
computes a file association to the Unity-generated
`UIElementsSchema/UIElements.xsd`, starts a pinned LemMinX release, and lets LemMinX
provide tag, attribute, and enum completion. No UXML parser, schema, or language server
is written in this repository.

Validation is **completion-only by default**. Schema diagnostics are opt-in through
`lsp.unity-uxml.settings` because Unity-generated schemas can be invalid in shipping
Unity versions.

## 2. Goals

- Register `[language_servers.unity-uxml]` for the existing `Unity UXML` language with
  language ID `xml`.
- Resolve LemMinX in this order: `lsp.unity-uxml.binary.path`, `lemminx` on `PATH`,
  pinned download from `redhat-developer/vscode-xml` release `0.29.3`.
- Verify the downloaded executable against a pinned SHA-256 before it is used.
- Point LemMinX at the worktree's `UIElementsSchema/UIElements.xsd`.
- Provide working completion for Unity elements, attributes, enums, and global-namespace
  custom elements.
- Keep validation off by default; allow opt-in through Zed settings.
- Handle a missing schema without crashing the server, and recover automatically when
  Unity generates the schema later.
- Keep the existing asmdef server, languages, grammars, snippets, and tasks unchanged.

## 3. Non-goals

- Writing a UXML parser or a second UXML language server.
- Bundling a static UXML schema. The project's generated schema is the only schema used.
- C#-aware UXML (`binding-path` fields, serialized properties, custom attributes beyond
  the generated XSD).
- USS support. USS stays deferred.
- Validating UXML against Unity's schema by default.
- Fixing Unity's generated XSD files. The extension never writes into a project.
- Multi-project workspace routing beyond the worktree root (see section 9).

## 4. Phase 0 spike findings

Tested with LemMinX `0.29.3` against Game Collector (Unity `6000.6.6f1`) and Oh My Boss
(Unity `6000.5.3f1`). The evidence scripts live in `/tmp/opencode/lemminx_spike*.py`,
`lemminx_config_probe.py`, `lemminx_init_probe.py`,
`lemminx_validation_probe.py`, `lemminx_missing_diag.py`, and
`lemminx_regen_probe.py`.

1. **Completion works.** The generated schema supplies 387 tag items in the
   `UIElements.xsd` association case, including `ui:VisualElement` and `ui:Button`,
   attribute completion (`name`, `class`, `style`, `picking-mode`), enum completion
   (`Position`, `Ignore`), and global-namespace custom elements with
   `substitutionGroup`.
2. **Settings arrive through initialization options, not `workspace/configuration`.**
   LemMinX reads `initializationOptions.settings.xml`. The working shape is
   `{"settings":{"xml":{...}}}`. The same object can be returned for
   `workspace/configuration` for compatibility, but LemMinX did not request it.
3. **Unity does not write a usable schema hint.** Unity writes unprefixed
   `noNamespaceSchemaLocation` (or a plain `xsi=` attribute), which LemMinX ignores.
   `xsi:noNamespaceSchemaLocation` still cannot validate namespaced UXML and produces
   `cvc-elt.1.a`. The file association is mandatory.
4. **Validation is noisy.** Game Collector's generated
   `UnityEngine.Localization.SmartFormat.PersistentVariables.xsd` is missing an
   `xs:import`; the same file is correct in Oh My Boss. The no-namespace association also
   produces false root errors. `validation.enabled=false` suppresses all diagnostics while
   completion keeps working.
5. **Custom elements.** Global-namespace custom elements complete correctly when the
   generated schema gives them `substitutionGroup="...VisualElement"`. Namespaced package
   elements (for example Addressables) may be inserted without their prefix; this is a
   LemMinX limitation.
6. **Schema regeneration recovers.** LemMinX dynamically registers
   `workspace/didChangeWatchedFiles` for `**/*.xsd`, `**/*.dtd`, `**/*.rng`, and
   `**/*.rnc`. After copying a schema into place and sending the watched-file event,
   completion went from 0 to 387 items without restarting the server. Completion
   recovers without a restart, but the extension computes its validation defaults only
   when the language server starts; a restart is needed to return to the completion-only
   defaults after the schema appears.
7. **Missing schema degrades safely.** Completion returns no Unity items and the server
   stays alive. With an association to the missing path and schema validation on,
   LemMinX publishes `schema_reference.4` naming the missing file; this is the clearest
   native message available.
8. **Checksums cover the extracted executable, not the zip.** The published `.sha256`
   assets hash the binary inside each zip.

## 5. Architecture

```text
Zed
 └─ unity extension (WASM)
     ├─ [unity-asmdef] → unity-asmdef-lsp (native, released separately)
     └─ [unity-uxml]   → LemMinX 0.29.3 (native, downloaded from upstream)
                          └─ UIElementsSchema/UIElements.xsd (project-generated)
```

The extension owns only launch, download/verification, and LemMinX settings. Parsing,
completion, validation, and schema watching stay in LemMinX.

Repository changes:

```text
src/lib.rs                 # dispatch by server id, LemMinX install + settings
extension.toml             # second language server, version 0.3.0
Cargo.toml                 # extension version 0.3.0
README.md                  # UXML behavior and settings
ROADMAP.md                 # Phase 0/1/2 status, no static fallback
PUBLISHING.md              # manual checks and release notes
```

The asmdef binary and `lsp/` crate stay at `0.2.1`; `0.3.0` is an extension-only release.

## 6. Language registration

```toml
[language_servers.unity-uxml]
name = "Unity UXML Language Server"
languages = ["Unity UXML"]
language_ids = { "Unity UXML" = "xml" }
```

`Unity UXML` is the existing language name in
`languages/unity-uxml/config.toml`; the language ID must be `xml` because LemMinX expects
XML documents. No other language is attached to this server.

`language_server_command` dispatches on the server ID. Unknown IDs return an error.
`unity-asmdef` keeps its current resolution order. `unity-uxml` uses its own cached path
so the two servers cannot overwrite each other.

## 7. LemMinX distribution

Pinned release: `redhat-developer/vscode-xml` tag `0.29.3`.

| Platform | Zip asset | Executable | SHA-256 |
|---|---|---|---|
| Linux x86_64 | `lemminx-linux-x86_64.zip` | `lemminx-linux-x86_64` | `84cbbe3c31eb158799327d5f68a0dc876e81c431da596747b92eecffe956b3f2` |
| Linux aarch64 | `lemminx-linux-aarch_64.zip` | `lemminx-linux-aarch_64` | `e706d10fbda4fa354952edb604800d067518401521d4d1e491b8bee7cf4cbcdb` |
| macOS x86_64 | `lemminx-osx-x86_64.zip` | `lemminx-osx-x86_64` | `e13b4c135f56f906a7450bc9f64f49718bf743c5c4df8003a5610e1fb8772a55` |
| macOS aarch64 | `lemminx-osx-aarch_64.zip` | `lemminx-osx-aarch_64` | `c5e9e2aee768eb69d041e9541cd0568a432ebd51c7fe9c1d7b7a1130b759475a` |
| Windows x86_64/aarch64 | `lemminx-win32.zip` | `lemminx-win32.exe` | `25f571c0f07d0ad76be60a86c1b864d46cf7bf94773dc21e5d5da646478aac79` |

Resolution order:

1. `lsp.unity-uxml.binary.path` (with `arguments` and `env`).
2. `lemminx` on `PATH` via `worktree.which("lemminx")`.
3. The pinned release above.

Install flow:

1. Check `lemminx-0.29.3/<executable>`; reuse it when present.
2. Download the platform zip with `DownloadedFileType::Zip` into
   `lemminx-0.29.3.download/`.
3. Hash the extracted executable and compare with the pinned SHA-256.
4. Mark it executable, remove the previous version directory, and rename the download
   directory into place.
5. Delete the download directory on any failure.

Errors explain how to install LemMinX manually or set
`lsp.unity-uxml.binary.path`. Installation status is reported against the `unity-uxml`
server ID.

LemMinX is downloaded at runtime and is not bundled in the extension.

## 8. Settings and schema binding

`lsp.unity-uxml.settings` is the LemMinX settings object, conventionally wrapped in
`xml`:

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

The extension computes defaults, deep-merges user settings over them, and passes the
result as `initializationOptions.settings`. Objects merge recursively; arrays and scalars
replace. `lsp.unity-uxml.initialization_options` is preserved as-is, except that
`settings` is inserted when the user did not provide it.

Default association, emitted even when the file is missing:

```json
{
  "xml": {
    "fileAssociations": [
      {
        "pattern": "**/*.uxml",
        "systemId": "<worktree-root>/UIElementsSchema/UIElements.xsd"
      }
    ]
  }
}
```

A user-provided `xml.fileAssociations` replaces this array. Relative `systemId` and
`xml.catalogs` paths are resolved against the worktree root; absolute paths and URIs are
left untouched.

### 8.1 Schema present

```json
{
  "xml": {
    "validation": {
      "enabled": false,
      "schema": { "enabled": "always" },
      "noGrammar": "ignore"
    }
  }
}
```

Completion works; diagnostics are silent unless the user enables validation. When the
user opts in, `schema.enabled` is already `always`, so the adapter behaves like the VS
Code XML client. Users may override `schema.enabled` to `onValidSchema` or `never`.

### 8.2 Schema missing

```json
{
  "xml": {
    "validation": {
      "enabled": true,
      "schema": { "enabled": "always" },
      "noGrammar": "warning",
      "namespaces": { "enabled": "never" }
    }
  }
}
```

The association still points at the expected path, so LemMinX binds the schema if Unity
creates it while the server is running. Until then, LemMinX publishes
`schema_reference.4` naming the missing file (plus `cvc-elt.1.a`) and completion returns
no Unity items. The server stays alive. A user who explicitly sets
`validation.enabled=false` also suppresses this message.

These missing-schema defaults apply only when the extension observes the schema as
absent at language server start. If Unity creates the schema while the server is
running, the association picks it up and completion returns without a restart, but
validation stays enabled for that session. Restart the language server after the schema
appears to restore the completion-only defaults in 8.1.

The extension does not force validation on in any other state.

### 8.3 Why not `onValidSchema`

Unity writes an unprefixed `noNamespaceSchemaLocation`. LemMinX does not treat that as a
valid schema hint, so `onValidSchema` would silently disable validation for normal Unity
files. `always` with validation disabled by default is the only configuration that keeps
completion working while keeping diagnostics predictable.

## 9. Project discovery

The schema path is derived from `worktree.root_path()`:

```text
<worktree-root>/UIElementsSchema/UIElements.xsd
```

Schema existence is checked through `worktree.read_text_file`; the extension does not
scan the worktree. This assumes the worktree is the Unity project root, which is the
normal Zed setup and matches the asmdef server's primary case.

Multi-Unity-project worktrees are an acknowledged edge case. The default association
covers every `.uxml` in the worktree. Users in that setup can provide explicit
`xml.fileAssociations` with per-project patterns and `systemId` values.

## 10. Failure behavior

| Condition | Behavior |
|---|---|
| LemMinX download fails | Actionable error with PATH and `lsp.unity-uxml.binary.path` instructions; asmdef server unaffected. |
| Checksum mismatch | Delete the partial download and fail with expected/actual hashes. |
| Schema missing | Server starts; no Unity completion; `schema_reference.4` names the missing file; completion recovers on watched-file event, but validation stays forced on until the server restarts. |
| Unity schema is invalid | Validation stays off by default; user can opt in and see LemMinX diagnostics. |
| Namespaced custom element loses its prefix | Known LemMinX limitation; documented, not patched. |
| Binary path configured but missing | Zed reports the launch failure; no fallback to a different binary. |

## 11. Testing

Unit tests in `src/lib.rs`:

- LemMinX asset name for every supported platform; 32-bit x86 rejected.
- A pinned 64-hex checksum for every asset.
- Settings builder: schema present, schema missing, user validation override, user
  `fileAssociations` replacement, deep merge of nested objects.
- Initialization options: `settings` inserted, user `initialization_options` preserved,
  user-provided `settings` wins.
- Relative-path resolution for `catalogs` and `fileAssociations[].systemId`.
- Existing asmdef asset/checksum tests remain green.

Verification:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p unity-asmdef-lsp
cargo build --target wasm32-wasip2 --release
./scripts/validate.sh all
./scripts/check-publish.sh
```

Manual checks on a real Unity project:

1. Open a `.uxml`; confirm tag, attribute, and enum completion appears.
2. Confirm an unknown tag is not completed.
3. Test the missing-schema path on a worktree where the schema is absent at server start,
   or rename `UIElementsSchema` and restart the language server; confirm the
   missing-schema diagnostic and that the server stays alive.
4. Restore the schema; confirm completion returns without restarting Zed, then restart
   the language server and confirm validation is off again (or document a restart if Zed
   does not forward the watched-file event).
5. Enable `lsp.unity-uxml.settings.xml.validation.enabled`; confirm diagnostics appear.
6. Confirm `.asmdef` features are unchanged.

## 12. Documentation and release

- README: UXML completion, schema-generation step, missing-schema behavior, validation
  opt-in, settings example, LemMinX download, custom-element and multi-project limits.
- ROADMAP: mark Phase 0 spike complete, Phase 1/2 implemented in `0.3.0`, remove the
  bundled-static-schema fallback, keep USS and C#-aware UXML deferred.
- PUBLISHING: version `0.3.0`, two language servers, LemMinX upstream license and
  runtime download, manual UXML checks.
- Bump `Cargo.toml`, `extension.toml`, and `Cargo.lock` to `0.3.0`. Keep `lsp/` at
  `0.2.1` and set `LSP_TAG = "v0.3.0"`. The release tag is `v<extension-version>` and
  carries both the extension source and the asmdef LSP assets.
- Do not modify `.github/workflows/release-lsp.yml`; the existing
  `actions/upload-artifact@v7` edit belongs to the user.

## 13. Risks

| Risk | Mitigation |
|---|---|
| Unity's generated schema is temporarily or permanently invalid | Validation off by default; opt-in is documented as best-effort. |
| Missing schema forces diagnostics on | Only in the broken state; the message names the missing file and preserves recovery. |
| Zed does not forward watched-file events in some versions | Document restart as fallback; spike simulated the notification successfully. |
| Multi-project worktrees get the wrong schema | Acknowledged edge case; explicit `fileAssociations` override. |
| Namespaced custom elements are inserted without prefixes | Upstream LemMinX limitation; documented, not patched. |
| LemMinX is EPL-2.0, repository is MIT | Binary is downloaded at runtime from upstream, not bundled or modified. |
| Windows path and executable naming differ | Asset table tests cover Windows; reuse the `.exe` name in the binary path. |
| `read_text_file` on a large schema is wasteful | Once per server start; only used for existence/schema settings. |

## 14. Decisions

- Completion-only default, validation opt-in, per user selection.
- LemMinX `0.29.3` pinned by tag and per-asset SHA-256 constants, verified after zip
  extraction.
- Settings are passed through `initializationOptions.settings.xml`; this is the only
  path LemMinX actually reads.
- The file association is always emitted, including when the schema is missing, to
  enable recovery through LemMinX's watched-file registration.
- No bundled or generated fallback schema.
- Worktree-root schema discovery only; explicit settings cover multi-project worktrees.
