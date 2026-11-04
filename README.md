# Unity for Zed

Zed language support for Unity projects: assembly definitions, Unity's YAML-serialized
assets, UI Toolkit UXML/USS, and ShaderLab.

The extension includes a native language server for assembly definitions. It is a
Rust Zed extension and downloads its server binary on first use. C# remains handled
by Zed's official C# extension.

## Supported files

| Format | Extensions | Language mode |
|---|---|---|
| Assembly definitions | `.asmdef`, `.asmref` | Unity Assembly Definition |
| Unity serialized YAML | `.unity`, `.prefab`, `.asset`, `.meta`, `.mat`, `.anim`, `.controller`, `.overrideController`, `.physicMaterial`, `.physicsMaterial2D`, `.renderTexture`, `.cubemap`, `.flare`, `.guiskin`, `.fontsettings`, `.spriteatlas`, `.spriteatlasv2`, `.shadervariants`, `.lighting`, `.brush`, `.terrainlayer`, `.mask`, `.preset`, `.signal`, `.mixer`, `.playable` | Unity YAML |
| UI Toolkit markup | `.uxml` | Unity UXML |
| UI Toolkit styles | `.uss`, `.tss` | Unity USS |
| ShaderLab | `.shader` | Unity ShaderLab |

## Features

- Unity-aware highlighting for assembly-definition fields, GUID references, platform
  lists, define constraints, and version defines.
- Project-aware completion, diagnostics, and go-to-definition for `.asmdef` and
  `.asmref` files through `unity-asmdef-lsp`.
- Unity YAML highlighting for `!u!` class tags, anchors, `%YAML`/`%TAG` directives,
  `m_*` keys, 32-hex GUIDs, and `fileID` values.
- Outlines for assembly fields, serialized objects and `m_Name`, UXML elements, USS rule
  sets, shaders, and shader properties.
- Bracket matching and indentation tuned for each format.
- Unity CLI tasks available from supported Unity files.
- Unity C# snippets for common MonoBehaviour, lifecycle, physics, rendering, and editor
  patterns (requires the official C# extension).
- Optional embedded HLSL/CG highlighting in ShaderLab when an HLSL language is installed.

## Assembly definition language server

`.asmdef` and `.asmref` files are served by `unity-asmdef-lsp`, a native language
server launched by this extension. It indexes the Unity project and its resolved
packages, so completion is project-aware:

- `references` completes assembly names and `GUID:` forms.
- `includePlatforms`, `excludePlatforms`, `versionDefines[].name`, and
  `precompiledReferences` complete their values.
- Schema keys complete in asmdef and asmref files.
- Malformed JSON and invalid fields are reported as diagnostics.
- Unresolved references, duplicate assembly names, cycles, and missing `.asmref`
  targets are diagnosed.
- Go-to-definition jumps from a reference to the declaring assembly.
- Find references (`shift-f12`) across `.asmdef` and `.asmref` files.
- Rename (`f2`) updates the declaration and every name-based reference; `GUID:`
  references stay valid because the `.meta` GUID does not change.
- Hover shows the assembly path, GUID, project/package origin, root namespace, and
  referencing count.
- Quick fixes remove an unresolved reference, convert a reference between `GUID:`
  and name form, or replace a typo with the nearest assembly name.
- Project symbols (`cmd-t` on macOS, `ctrl-t` elsewhere) list project and package
  assemblies.
- Changes are picked up from the editor and a file watcher.

Package assemblies are read-only completion, definition, reference, and hover
sources. They are indexed from `Packages/` and from the package directories named by
`Packages/manifest.json` or `packages-lock.json` under `Library/PackageCache/`. Rename
refuses package assemblies and never edits package files. The rest of `Library/` is
never scanned.

The server is downloaded from a pinned GitHub release on first use. No Rust toolchain
is needed for normal installation. To use a local build instead:

```bash
cargo build --release -p unity-asmdef-lsp
```

Then either put `target/release/unity-asmdef-lsp` on `PATH` or configure it:

```json
{
  "lsp": {
    "unity-asmdef": {
      "binary": {
        "path": "/absolute/path/to/unity-asmdef-lsp",
        "arguments": [],
        "env": {}
      }
    }
  }
}
```

The extension detects Unity projects at the workspace root and in nested folders. It
only attaches the server to **Unity Assembly Definition**; C# remains fully owned by
the official C# extension. UXML and USS completion is not part of `0.2.1`.

## Installation

### From the Zed extension registry

> The registry submission for this extension is pending. Until it is accepted, use the
> dev-extension flow below.

Open the Extensions view with `cmd-shift-x` (macOS) or `ctrl-shift-x` (Linux/Windows),
search for **Unity**, and install it. To install it automatically on new machines, add it
to your Zed settings:

```json
{
  "auto_install_extensions": {
    "unity": true
  }
}
```

### As a dev extension

1. Clone this repository.
2. In Zed, run `zed: install dev extension` and select the repository root.
3. Open a supported Unity file and confirm the language mode in the status bar.
4. Open an `.asmdef` file. The first launch downloads `unity-asmdef-lsp`; the Zed log
   shows the version and index summary.

C# support is separate. Install Zed's official **C#** extension and configure one of its
language servers (Roslyn is the current default). See the
[C# extension documentation](https://zed.dev/docs/languages/csharp).

## Recommended Unity project settings

Unity projects contain very large generated folders. Keep Zed's file scan fast by adding
exclusions to `.zed/settings.json`:

```json
{
  "file_scan_exclusions": [
    "**/.git",
    "**/Library",
    "**/Temp",
    "**/Logs",
    "**/obj",
    "**/Build",
    "**/Builds",
    "**/UserSettings",
    "**/.vs",
    "**/.idea"
  ]
}
```

If you use Roslyn through the official C# extension, enable it explicitly:

```json
{
  "languages": {
    "CSharp": {
      "language_servers": ["roslyn", "..."]
    }
  }
}
```

Project and solution generation is not part of this extension. Use Unity's Visual Studio
Editor package or the Unity-side Zed integration package for `.sln`/`.csproj` generation.

## Unity CLI tasks

The extension adds these tasks to every supported Unity language:

| Task | Command |
|---|---|
| Unity: Recompile scripts | `unity recompile --project-path $ZED_WORKTREE_ROOT --timeout 120` |
| Unity: Run EditMode tests | `unity test $ZED_WORKTREE_ROOT --mode EditMode --output $ZED_WORKTREE_ROOT/Logs/zed-editmode-results.xml` |
| Unity: Run PlayMode tests | `unity test $ZED_WORKTREE_ROOT --mode PlayMode --output $ZED_WORKTREE_ROOT/Logs/zed-playmode-results.xml` |
| Unity: Check .meta pairing | `unity vcs status --project-path $ZED_WORKTREE_ROOT` |
| Unity: Editor status | `unity status --project-path $ZED_WORKTREE_ROOT` |

Requirements:

- Install the [Unity CLI](https://docs.unity.com/en-us/unity-cli/unity-cli) and put it on
  the `PATH` that Zed's login shell sees.
- `Unity: Recompile scripts` and `Unity: Editor status` require a running Editor with the
  Pipeline package (`unity pipeline install`).
- `Unity: Check .meta pairing` requires a Git or Unity Version Control workspace.
- Test tasks use Unity's batch test runner and do not require a running Editor.

The tasks assume the Unity project is the Zed worktree root. If the project is nested in a
monorepo, copy the tasks into `.zed/tasks.json` and change `--project-path`, or open the
Unity project folder as the worktree.

## `.meta` files: read this before renaming assets

Unity identifies assets by the GUID stored in their `.meta` file. Renaming or moving an
asset without moving its `.meta` file can break references and cause Unity to generate a
new GUID.

Safe workflows:

1. Rename or move assets in Unity's Project window, not in Zed.
2. If you must rename in Zed, move the matching `.meta` file at the same time, keeping the
   same basename.
3. Detect pairing problems with `unity vcs status`; use `unity vcs doctor` to check
   repository, ignore-rule, and LFS health.

This extension cannot automate `.meta` moves. Zed extensions cannot rename, move, or watch
files. A companion Unity Editor package or a language server using
`workspace/willRenameFiles` could do it in a future project, but that is out of scope here.

## UI Toolkit notes

UXML and USS support is syntax highlighting, not validation. Unity parses these files on
import and reports problems in the Console.

USS is a subset of CSS. It does not support `gap`, `z-index`, `outline`, `box-shadow`,
`filter`, `pointer-events`, attribute selectors, `:first-child`, `:last-child`,
`:nth-child`, or CSS gradients. Use `project://database/...` URLs only. Do not use inline
`style` attributes in UXML; use USS classes and `<ui:Style>` instead.

For a deeper authoring guide, see Unity's
[USS documentation](https://docs.unity3d.com/Manual/UIE-USS.html) and
[UXML documentation](https://docs.unity3d.com/Manual/UIE-UXML.html).

## ShaderLab notes

ShaderLab support covers the shader structure and property declarations. Embedded
`HLSLPROGRAM`, `CGPROGRAM`, and `GLSLPROGRAM` blocks are injected into the corresponding
language when one is installed. Install an HLSL language extension for embedded HLSL
highlighting; ShaderLab structure still highlights without it.

## Binary assets

Only text-serialized Unity assets are supported. Binary-serialized files are not
supported and may show parse errors or render as an empty tree, depending on the header
bytes. If a `.unity`, `.prefab`, or `.asset` file uses Unity's binary serialization, set
**Edit > Project Settings > Editor > Asset Serialization > Mode** to **Force Text** (the
recommended setting for version control) and let Unity reserialize the project.

`.meta` files are always text YAML.

## Troubleshooting

- **A file opens with the wrong language:** check `file_types` overrides in your Zed
  settings and make sure another extension is not claiming the suffix.
- **ShaderLab embedded code is plain:** install an HLSL/GLSL language extension.
- **Tasks fail with "unity: command not found":** install the Unity CLI and reopen Zed from
  a shell that can find it.
- **`Unity: Recompile scripts` cannot connect:** start the Editor, install the Pipeline
  package, and check `unity status`.
- **Assembly completion is empty:** open `zed: open log`. The log reports the server
  start and index summary. Confirm the workspace contains `ProjectSettings/ProjectVersion.txt`
  or `Packages/manifest.json`, and wait for the first index to finish.
- **The language server cannot be downloaded:** build it locally with
  `cargo build --release -p unity-asmdef-lsp`, put it on `PATH`, or set
  `lsp.unity-asmdef.binary.path`.
- **A malformed asmdef has no diagnostics:** open the file once and check the diagnostics
  panel; the server reports strict-JSON parse errors. The asmdef highlight query also
  captures `ERROR` nodes.
- **Grammar build errors:** open `zed: open log` and check the grammar build output. Zed
  downloads `wasi-sdk` automatically.

## Development

Prerequisites:

- Rust stable with the `wasm32-wasip2` target (`rustup target add wasm32-wasip2`).
- A `tree-sitter` CLI on `PATH` (`cargo install tree-sitter-cli` or a release binary).
- Python 3.11+ for TOML/JSON validation.
- Git (the validation script fetches pinned grammar commits into `tests/.grammars/`).

Run all checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release -p unity-asmdef-lsp
cargo build --target wasm32-wasip2 --release
./scripts/validate.sh
./scripts/check-publish.sh
```

The committed LSP fixture intentionally contains broken references, duplicates, a cycle,
and malformed JSON, so `--check` exits non-zero on it:

```bash
cargo run -p unity-asmdef-lsp -- --check tests/fixtures/lsp-project
```

Use it to verify diagnostics. For a clean smoke test, run the same command against a real
Unity project.

Useful modes:

```bash
./scripts/validate.sh --fixtures-only   # parse fixtures only
./scripts/validate.sh --self-test       # confirm malformed fixtures fail
./scripts/validate.sh --stress          # parse a generated ~400 KB YAML file
```

The script is local-only. It clones each pinned grammar into `tests/.grammars/`
(gitignored) and validates every language config and `.scm` query against its fixtures.

Publishing preflight:

```bash
./scripts/check-publish.sh
./scripts/check-publish.sh --strict   # warnings become failures
```

Before opening the registry PR, also complete the checklist in
[PUBLISHING.md](PUBLISHING.md).

Manual Zed check:

1. Install the repository as a dev extension.
2. Open each fixture in `tests/fixtures/` and verify language mode, highlighting, outline,
   comments, brackets, and indentation.
3. Open an `.asmdef`, type a `references` entry, and confirm project and package assembly
   completion plus `GUID:` completion.
4. Use go-to-definition on a reference and confirm it opens the target `.asmdef`/`.asmref`.
5. Use find-references (`shift-f12`) on an assembly name and confirm references in other
   `.asmdef`/`.asmref` files appear.
6. Hover an assembly name and confirm the path, GUID, origin, and referencing count.
7. Run rename (`f2`) on a project assembly and confirm the declaration and name-based
   references appear in the preview; confirm a package assembly refuses the rename.
8. Put the cursor on an unresolved reference and confirm the remove, nearest-name, and
   `GUID:`/name conversion quick fixes appear where applicable.
9. Open project symbols (`cmd-t` / `ctrl-t`) and confirm project and package assemblies are
   listed.
10. Open the committed broken fixture and confirm the parse diagnostic appears; confirm a
    malformed asmdef also highlights as an error.
11. Confirm `.cs` still uses the official C# extension and receives no extra server.
12. Open `task: spawn` from a Unity YAML file and confirm all five tasks appear.
13. Run the checks that your environment supports and report any query or grammar errors.

## Future work

See [ROADMAP.md](ROADMAP.md) for the remaining semantic-completion plan (UXML, USS, and
C#-aware features). Assembly definition completion, diagnostics, definitions, references,
rename, hover, quick fixes, and workspace symbols are implemented in `0.2.1`.

- `.meta` automation requires a Unity Editor package or a language server using
  `workspace/willRenameFiles`; it is not part of this extension.
- `.shadergraph`, `.shadersubgraph`, and `.inputactions` support are candidate future
  formats.

## Publishing

See [PUBLISHING.md](PUBLISHING.md) for the preflight checklist, the exact
`zed-industries/extensions` submodule and `extensions.toml` steps, and the update flow.

## License

MIT. See `LICENSE`.
