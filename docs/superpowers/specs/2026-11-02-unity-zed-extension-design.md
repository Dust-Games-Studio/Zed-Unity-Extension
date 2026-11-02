# Unity Zed Extension — Design

**Date:** 2026-11-02
**Status:** Design approved; spec pending review
**Repository:** https://github.com/Dust-Games-Studio/Zed-Unity-Extension
**Extension ID:** `unity`
**License:** MIT

## 1. Summary

A declarative Zed extension that adds first-class editing support for Unity-specific
text formats: assembly definitions, Unity's YAML-serialized assets, UI Toolkit UXML/USS,
and ShaderLab. It complements Zed's official C# extension rather than replacing it. It
ships no Rust code, no language server, and no bundled binaries.

The extension targets Unity 2022.3 through Unity 6.x on all desktop platforms. Its
primary deliverables are correct file-to-language association, syntax highlighting,
outlines, bracket/indent behavior, and a small set of Unity CLI tasks. Snippets are
explicitly deferred to a separate agent.

## 2. Goals

- Make Unity's non-C# text files open in Zed with the correct language mode.
- Provide meaningful highlighting beyond generic JSON/YAML/XML/CSS for Unity semantics:
  assembly-definition fields, Unity YAML class tags and GUIDs, UI Toolkit tags and
  properties, and ShaderLab structure.
- Provide outlines and indentation that match how Unity files are actually structured.
- Provide Unity CLI tasks for the common editor loop.
- Remain publishable to the Zed extension registry: one coherent Unity-focused
  extension, allowed license, pinned grammars, no procedural code.
- Coexist cleanly with the official `csharp`, `unity-snippets`, `unity-debugger`,
  `hlsl`, and `shader-ls` extensions.

## 3. Non-goals

- **No `.cs` language ownership.** C# remains owned by Zed's official C# extension.
- **No snippets in this implementation.** Another agent will add them. This design
  keeps language names and the manifest stable so snippets can be added later without
  restructuring.
- **No automatic `.meta` file moving or repair.** Zed extensions cannot rename, move, or
  watch files. See §4 and §12.
- **No Unity Editor package, `.sln`/`.csproj` generation, or `.zed/settings.json`
  writing.** That work belongs to the Unity-side extension, not this repository.
- **No debugger.** The published `unity-debugger` extension already covers that.
- **No semantic validation.** There is no LSP; the extension highlights syntax and
  structure, not whether a USS property is valid for a given Unity version.
- **No binary-serialized Unity assets.** Only text-serialized YAML is supported.
- **No `.scene` suffix.** Unity scenes use `.unity`; `.scene` is not a Unity format.
- **No bundled HLSL/GLSL languages.** ShaderLab injections use the user's installed
  HLSL extension when present.

## 4. Platform constraints

- Zed extensions can provide languages, themes, snippets, language servers, debuggers,
  and MCP servers. They cannot move files, watch the filesystem, or write project
  settings. A `meta`-pairing feature therefore cannot live in this extension.
- Every extension language must name a grammar declared in `extension.toml`. A grammar
  may point at a subdirectory of its repository with `path`.
- Extension grammars are compiled to WASM by Zed using `wasi-sdk`. Grammars with C or
  C++ external scanners are supported.
- Zed's extension grammar builder links each parser with
  `--export=tree_sitter_<grammar key>`, so the manifest key must match the parser
  symbol exported by the upstream grammar. The keys are `json`, `yaml`, `xml`, `css`,
  and `shaderlab`. The first four are also common native grammar names in Zed; where a
  native parser is registered, Zed resolves the name to it, while `shaderlab` is loaded
  from this extension's pin. The queries target stable node names shared by both.
- Extensions without language servers must not include Rust code, per the publishing
  prerequisites.
- Zed supports `workspace/willRenameFiles` / `workspace/didRenameFiles` for LSP servers.
  That is the technically viable future route for `.meta` automation, but it is out of
  scope for this extension.
- Tasks are supported per language. The official `csharp` extension already ships
  `languages/csproj/tasks.json` and `languages/slnx/tasks.json`, so this is a supported
  pattern even though it is not prominent in the extension docs.

## 5. Architecture

A single declarative extension with five language definitions and five pinned
Tree-sitter grammars:

| Language | Suffixes | Grammar repository | Pinned commit | Grammar path |
|---|---|---|---|---|
| Unity Assembly Definition | `asmdef`, `asmref` | `tree-sitter/tree-sitter-json` | `254c42a6476413b776221e03982ac8ae159eeb72` | root |
| Unity YAML | `unity`, `prefab`, `asset`, `meta`, `mat`, `anim`, `controller`, `overrideController`, `physicMaterial`, `physicsMaterial2D`, `renderTexture`, `cubemap`, `flare`, `guiskin`, `fontsettings`, `spriteatlas`, `spriteatlasv2`, `shadervariants`, `lighting`, `brush`, `terrainlayer`, `mask`, `preset`, `signal`, `mixer`, `playable` | `zed-industries/tree-sitter-yaml` | `baff0b51c64ef6a1fb1f8390f3ad6015b83ec13a` | root |
| Unity UXML | `uxml` | `tree-sitter-grammars/tree-sitter-xml` | `5000ae8f22d11fbe93939b05c1e37cf21117162d` | `xml` |
| Unity USS | `uss`, `tss` | `tree-sitter/tree-sitter-css` | `dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f` | root |
| Unity ShaderLab | `shader` | `msagca/tree-sitter-shaderlab` | `67e8a4e67b1026889eb41709cea66ec1bcfa6ad5` | root |

Zed's grammar builder requires the manifest key to match the upstream parser symbol
(`tree_sitter_<key>`). The keys are therefore the upstream names. Where Zed already
registers a native grammar under the same name, the native parser is used; the extension
still declares its pinned grammar so the registry requirement is satisfied. The
`shaderlab` grammar is unique to this extension and is loaded from the pin.

### 5.1 Coexistence rules

- `.cs` files continue to use the official C# extension and whichever C# language server
  the user configures.
- C# snippets are not added here. The existing `unity-snippets` extension is outdated
  and the user does not want to install it; snippets will be contributed separately to
  this extension by another agent.
- ShaderLab embedded code uses injections into `hlsl`/`glsl`. If the HLSL extension is
  installed, embedded blocks receive HLSL highlighting. If not, the ShaderLab structure
  still highlights correctly.
- No theme, icon theme, debugger, MCP server, or agent server is added.

## 6. Language specifications

### 6.1 Unity Assembly Definition

**Files:** `.asmdef`, `.asmref`
**Grammar:** JSON
**Purpose:** make the small but semantically dense assembly-definition schema readable.

**Configuration**

- `name = "Unity Assembly Definition"`
- `grammar = "json"`
- 4-space indentation, no hard tabs.
- No comment toggle: Unity expects strict JSON.
- No formatter: Prettier is not guaranteed to preserve Unity's expected shape, and
  formatting is not needed.
- JSON-style brackets and `autoclose_before`.

**Highlighting**

- Generic JSON: strings, numbers, booleans, null, punctuation, escapes.
- Keys as properties.
- `name`, `rootNamespace`, and asmref `reference` values as types.
- `references` entries starting with `GUID:` as special strings; other reference names
  remain strings.
- `includePlatforms` / `excludePlatforms` entries as built-in constants.
- `precompiledReferences` entries as strings.
- `defineConstraints` and version-define `define` values as constants.
- `versionDefines[].expression` as a special string.
- `versionDefines[].name` as a string.

**Outline**

- Top-level fields as outline items.
- `versionDefines` objects expanded by their `name` field.

**Brackets / indentation**

- `{}`, `[]`, and string quotes.
- Object and array indent ranges using the grammar's explicit start/end tokens.

**Notes**

- `.asmref` is the same JSON grammar with a single `reference` field; no separate
  language is needed.

### 6.2 Unity YAML

**Files:** all Unity YAML suffixes listed in §5.
**Grammar:** YAML (`zed-industries/tree-sitter-yaml`)
**Purpose:** make scenes, prefabs, assets, and `.meta` files navigable without pretending
they are ordinary YAML documents.

**Configuration**

- `name = "Unity YAML"`
- `grammar = "yaml"`
- 2-space indentation.
- `line_comments = ["# "]`.
- YAML bracket pairs (`{}`, `[]`, quotes) and YAML indent patterns.
- No formatter. Prettier would rewrite Unity YAML and risks breaking serialized data.

**Highlighting**

- Generic YAML: strings, numbers, booleans, null, comments, escapes.
- Document tags (`!u!1`, `!u!114`, …) as types.
- Anchors (`&123`) and aliases (`*123`) as labels.
- `%YAML` and `%TAG` directives as preprocessor-like tokens.
- Mapping keys as properties, including `m_*` keys and flow-mapping keys such as
  `fileID` and `guid`.
- 32-hex values as special strings, covering `.meta` GUIDs and serialized references.
- `fileID` and other integer values as numbers.
- `---` / `...` document markers as special punctuation.
- Best-effort: Unity class names (the first mapping key after a document tag) as types,
  with the generic property capture as the fallback if the theme or grammar resolves
  only one capture.

**Outline**

- One outline item per serialized object, labelled by its class name
  (`GameObject`, `Transform`, `MonoBehaviour`, …).
- `m_Name` entries as additional named items so GameObjects are navigable by name.

**Brackets / indentation**

- YAML flow brackets and quotes.
- The same indent patterns Zed uses for YAML, which already handle `key:` and list items.

**Notes**

- Only text-serialized assets are supported. Binary-serialized `.asset`/`.unity`/
  `.prefab` files will show parse errors. The README explains how to switch Unity's
  asset serialization to Force Text.
- `.meta` files are always text YAML.

### 6.3 Unity UXML

**Files:** `.uxml`
**Grammar:** XML
**Purpose:** readable UI Toolkit markup with Unity's element and attribute vocabulary.

**Configuration**

- `name = "Unity UXML"`
- `grammar = "xml"`
- 4-space indentation.
- XML block comments.
- `<`/`>` autoclose behavior.
- No formatter. Prettier's XML formatting is not UXML-aware.

**Highlighting**

- Generic XML: element names, attribute names, attribute values, comments, CDATA,
  declarations, punctuation.
- Unity tags (`UXML`, `VisualElement`, `Button`, `Label`, `TextField`, `Toggle`,
  `Slider`, `SliderInt`, `MinMaxSlider`, `ProgressBar`, `ScrollView`, `ListView`,
  `TreeView`, `MultiColumnListView`, `MultiColumnTreeView`, `Foldout`, `Box`, `Image`,
  `IMGUIContainer`, `Template`, `Instance`, `Style`, `AttributeOverrides`, `Bindings`,
  `Binding`, `DataBinding`, `Editor`, `Toolbar`, `ToolbarButton`, `ToolbarToggle`,
  `ToolbarMenu`, `ToolbarSearchField`, `ToolbarBreadcrumbs`, `DropdownField`,
  `EnumField`, `ObjectField`, `ColorField`, `CurveField`, `GradientField`,
  `LayerMaskField`, `MaskField`, `TagField`, `PropertyField`, `HelpBox`, `RadioButton`,
  `RadioButtonGroup`, `GroupBox`, `PopupWindow`, `Window`, `TextElement`) as built-in
  types.
- Namespace prefixes `ui:` and `uie:` are preserved in the element names.
- UXML attributes (`name`, `class`, `style`, `text`, `binding-path`,
  `data-source-path`, `data-source-type`, `picking-mode`, `usage-hints`, `tooltip`,
  `focusable`, `tabindex`, `enabled`, `visible`, `content-container`) as attributes.
- `xmlns`, `xmlns:ui`, `xmlns:uie`, and `xmlns:xsi` as attributes.
- `ERROR` nodes as errors.

**Outline**

- One outline item per element, labelled by its tag name.
- The `name` attribute is highlighted in the editor but is not used as outline context
  in v1.

**Brackets / indentation**

- XML start/end tag indent ranges.
- `<`/`>` bracket handling.

**Notes**

- The extension highlights syntax only. It does not enforce UXML conventions such as
  avoiding inline `style` attributes; those remain project policy.
- Fixtures follow the `ui-uitk` skill's UXML rules (no inline styles, one top-level
  container, `<ui:Style>` for stylesheets).

### 6.4 Unity USS

**Files:** `.uss`, `.tss`
**Grammar:** CSS
**Purpose:** Unity Style Sheets and Theme Style Sheets, including Unity-specific
properties and variables.

**Configuration**

- `name = "Unity USS"`
- `grammar = "css"`
- 4-space indentation.
- CSS block comments.
- CSS-style brackets and autoclose behavior.
- No formatter. Prettier is CSS-aware, not USS-aware, and can normalize away Unity
  conventions.

**Highlighting**

- Generic CSS: selectors, classes, IDs, pseudo-classes, properties, values, colors,
  numbers, units, functions, comments, punctuation.
- `--unity-*` custom properties as variables.
- `-unity-*` property names as properties.
- `var()`, `resource()`, `url()` as functions.
- USS-specific value identifiers (for example `middle-center`, `DynamicTransform`) as
  built-in constants where the grammar exposes them as plain values.
- `@import` and other at-rules as keywords; USS's at-rule support is limited, so this is
  syntax highlighting rather than a claim of semantic support.
- `ERROR` nodes as errors.

**Outline**

- One outline item per rule set, labelled by its selector.

**Brackets / indentation**

- CSS braces, parentheses, and brackets.

**Notes**

- The extension is not a USS validator. Common invalid-CSS-for-USS constructs (`gap`,
  `z-index`, `outline`, `box-shadow`, `filter`, `pointer-events`, attribute selectors,
  `:first-child`, `:last-child`, `:nth-child`, `linear-gradient()`, external `url()`)
  are documented in the README as Unity restrictions; no diagnostics are emitted.
- `.tss` files use the same language because they are USS with imports and theme
  variables.
- Fixtures follow the `ui-uitk` skill's USS restrictions.

### 6.5 Unity ShaderLab

**Files:** `.shader`
**Grammar:** ShaderLab (`msagca/tree-sitter-shaderlab`)
**Purpose:** Unity shader files, whose structure is ShaderLab rather than raw HLSL.

**Configuration**

- `name = "Unity ShaderLab"`
- `grammar = "shaderlab"`
- 4-space indentation.
- `// ` line comments and `/* */` block comments.
- ShaderLab brackets (`{}`, `[]`, `()`).
- No formatter.

**Highlighting**

- `Shader`, `Properties`, `SubShader`, `Pass`, `Tags`, `Name`, `LOD`, `UsePass`,
  `GrabPass`, `PackageRequirements`, `CustomEditor`, `Fallback`, `Dependency`,
  `Stencil` as keywords.
- `HLSLPROGRAM`, `HLSLINCLUDE`, `ENDHLSL`, `CGPROGRAM`, `CGINCLUDE`, `ENDCG`,
  `GLSLPROGRAM`, `ENDGLSL` as directive-like keywords.
- Shader name and `Name`/`UsePass` strings as labels.
- Property names, property types, property display names, and defaults.
- Render-state commands and their arguments as built-in constants.
- Stencil fields, legacy commands, and legacy blocks.
- `Fallback "Off"` as a built-in constant.
- HLSL/CG/GLSL code blocks injected into the corresponding language when available.

**Outline**

- The shader declaration.
- Property declarations.

**Brackets / indentation**

- Braces, brackets, and parentheses.
- Indent ranges for shader, properties, subshader, pass, stencil, tags, and embedded
  code blocks.

**Notes**

- The ShaderLab grammar handles case-insensitive keywords.
- Embedded-code highlighting depends on the user's installed HLSL/GLSL extensions. The
  README documents this as optional.
- No `.hlsl`, `.cginc`, or `.compute` file association is claimed; the published HLSL
  extension owns those formats.

## 7. Unity CLI tasks

Tasks are duplicated into each Unity language directory (the same `tasks.json` content)
so they are available from `task: spawn` while editing any supported Unity file. They
require the Unity CLI and are verified against `1.0.0-beta.15`; because the CLI is still
in beta, the README presents them as optional and language support does not depend on
them.

| Label | Command | Requirements |
|---|---|---|
| `Unity: Recompile scripts` | `unity recompile --project-path $ZED_WORKTREE_ROOT --timeout 120` | Connected Editor; Pipeline package for Editor-side command support |
| `Unity: Run EditMode tests` | `unity test $ZED_WORKTREE_ROOT --mode EditMode --output $ZED_WORKTREE_ROOT/Logs/zed-editmode-results.xml` | Unity CLI; no running Editor required |
| `Unity: Run PlayMode tests` | `unity test $ZED_WORKTREE_ROOT --mode PlayMode --output $ZED_WORKTREE_ROOT/Logs/zed-playmode-results.xml` | Unity CLI; no running Editor required |
| `Unity: Check .meta pairing` | `unity vcs status --project-path $ZED_WORKTREE_ROOT` | Unity CLI; Git/UVCS workspace |
| `Unity: Editor status` | `unity status --project-path $ZED_WORKTREE_ROOT` | Connected Editor |

Task behavior:

- `save: "all"` before running so compilation and tests see current buffers.
- `reveal: "always"`, `reveal_target: "center"` for tests; `hide: "on_success"` for the
  checks.
- `shell: "system"`, `use_new_terminal: false`, `allow_concurrent_runs: false`.
- `cwd` defaults to the project root; the explicit `--project-path` keeps the task
  correct when the Unity project is the Zed worktree root. For a Unity project nested in
  a monorepo, copy the tasks into `.zed/tasks.json` and change `--project-path`, or open
  the Unity project folder as the worktree.

## 8. Repository layout

```text
extension.toml
languages/
  unity-assembly-definition/
    config.toml
    highlights.scm
    brackets.scm
    indents.scm
    outline.scm
    tasks.json
  unity-yaml/
    config.toml
    highlights.scm
    brackets.scm
    indents.scm
    outline.scm
    tasks.json
  unity-uxml/
    config.toml
    highlights.scm
    indents.scm
    outline.scm
    tasks.json
  unity-uss/
    config.toml
    highlights.scm
    brackets.scm
    indents.scm
    outline.scm
    tasks.json
  unity-shaderlab/
    config.toml
    highlights.scm
    brackets.scm
    indents.scm
    outline.scm
    injections.scm
    tasks.json
scripts/
  validate.sh
tests/
  fixtures/
    sample.asmdef
    sample.asmref
    sample-guid.asmref
    sample.unity
    sample.prefab
    sample.asset
    sample.meta
    sample.uxml
    sample.uss
    sample.tss
    sample.shader
README.md
LICENSE
.gitignore
```

There is no `Cargo.toml` and no `src/`. The `csharp-main/` directory currently in the
working tree is a reference checkout and must not be committed; `.gitignore` will exclude
it, along with `tests/.grammars/`, grammar build caches, and OS/editor noise. This
implementation does not create or reference snippet files; the follow-up snippet work may
populate a `snippets/` directory independently.

## 9. Validation

### 9.1 Automated grammar/query validation

`scripts/validate.sh` will:

1. Require `tree-sitter` on `PATH`; print an actionable message if missing.
2. Clone each pinned grammar commit into `tests/.grammars/` (gitignored) if not already
   present.
3. Run `tree-sitter generate` for each grammar.
4. Parse each fixture with its grammar and fail on parse errors.
5. Run every `.scm` query for the matching language against at least one fixture. Query
   syntax errors and invalid node names fail the run.
6. Optionally run `tree-sitter highlight` on each fixture as a visual smoke test.

The script is intentionally local-only; no network is required after the grammars are
cached. CI is optional and not part of v1.

### 9.2 Fixtures

Fixtures use realistic Unity content, not toy snippets:

- `.asmdef` with assembly-name and GUID references, include/exclude platforms,
  precompiled references, define constraints, and version defines.
- `.asmref` by name and by GUID (two small fixtures).
- `.unity` with multiple `!u!` documents, a GameObject, Transform, MonoBehaviour,
  prefab instance, and `m_Name`.
- `.prefab`, `.asset`, and `.meta`.
- `.uxml` with a `<ui:Style>` reference, named elements, classes, and UXML attributes,
  with no inline `style` attributes.
- `.uss` using valid USS subset features: `--unity-*` variables, `-unity-*` properties,
  `flex-grow`, margins, padding, background colors, and no unsupported CSS properties
  (including `gap`, `z-index`, `outline`, `box-shadow`, attribute selectors, and child
  pseudo-classes).
- `.tss` with an `@import`.
- `.shader` with Properties, SubShader, Tags, render states, a Stencil block, a
  CGPROGRAM block, an HLSLPROGRAM block, and Fallback.

### 9.3 Manual validation in Zed

Because this environment has no Zed binary, the README documents the manual pass:

1. `zed: install dev extension` and select the repository root.
2. Open each fixture and confirm the language mode, highlighting, outline, comments,
   brackets, and indentation.
3. Confirm `.asmdef`, `.unity`, `.meta`, `.uxml`, `.uss`, and `.shader` are claimed by
   this extension and `.cs` is still handled by the C# extension.
4. Open `task: spawn` from a Unity YAML file and confirm the five Unity tasks appear.
5. Run `Unity: Editor status` and `Unity: Check .meta pairing`; run tests where a Unity
   project is available.
6. Check `zed: open log` for grammar build or query errors.

### 9.4 Acceptance criteria

- All fixtures parse with zero `ERROR` nodes under their pinned grammars.
- All query files compile and match their intended fixtures.
- The extension installs as a dev extension without errors.
- Supported files open with the correct language mode.
- The official C# extension continues to handle `.cs`; no snippets or LSPs are
  overridden.
- Tasks appear from Unity languages and the commands work where the CLI/Editor are
  available.
- The README explains setup, limitations, `.meta` safety, and optional HLSL support.

## 10. Publishing

`extension.toml`:

```toml
id = "unity"
name = "Unity"
version = "0.1.0"
schema_version = 1
authors = ["Dust Games Studio"]
license = "MIT"
description = "Unity file format support: assembly definitions, Unity YAML assets, UI Toolkit UXML/USS, and ShaderLab."
repository = "https://github.com/Dust-Games-Studio/Zed-Unity-Extension"

snippets = [
    "snippets/animation/csharp.json",
    "snippets/audio/csharp.json",
    "snippets/debug/csharp.json",
    "snippets/editor/csharp.json",
    "snippets/general/csharp.json",
    "snippets/input/csharp.json",
    "snippets/lifecycle/csharp.json",
    "snippets/networking/csharp.json",
    "snippets/particles/csharp.json",
    "snippets/physics/csharp.json",
    "snippets/rendering/csharp.json",
]

[grammars.json]
repository = "https://github.com/tree-sitter/tree-sitter-json"
rev = "254c42a6476413b776221e03982ac8ae159eeb72"

[grammars.yaml]
repository = "https://github.com/zed-industries/tree-sitter-yaml"
rev = "baff0b51c64ef6a1fb1f8390f3ad6015b83ec13a"

[grammars.xml]
repository = "https://github.com/tree-sitter-grammars/tree-sitter-xml"
rev = "5000ae8f22d11fbe93939b05c1e37cf21117162d"
path = "xml"

[grammars.css]
repository = "https://github.com/tree-sitter/tree-sitter-css"
rev = "dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f"

[grammars.shaderlab]
repository = "https://github.com/msagca/tree-sitter-shaderlab"
rev = "67e8a4e67b1026889eb41709cea66ec1bcfa6ad5"
```

The manifest also registers the 11 Unity C# snippet files under `snippets/`. They are
available in C# buffers and require the official C# extension.

The extension ID `unity` is currently free in the registry; `unity-snippets` and
`unity-debugger` are separate IDs. The name and ID do not contain `zed` or `extension`.

Publishing path:

1. Push the repository to `Dust-Games-Studio/Zed-Unity-Extension` and make it public.
2. Fork `zed-industries/extensions`.
3. Add the repository as a submodule at `extensions/unity`.
4. Add a `[unity]` entry to `extensions.toml`.
5. Open a pull request; maintainers build the extension and review the manifest.
6. Address review feedback, then publish.

The `README.md` will cover installation, features, supported files, C#/Roslyn setup,
recommended Unity `.zed/settings.json` exclusions, optional HLSL support, Unity CLI task
requirements, `.meta` safety, limitations, and development/validation instructions.

## 11. Risks and mitigations

| Risk | Mitigation |
|---|---|
| A grammar fails to build under Zed's WASI toolchain. | Validate each grammar with the Tree-sitter CLI first; if one fails, use a known-good fork or commit. If it still fails, drop that language from v1 rather than shipping a broken parser. The YAML grammar is the same parser Zed uses natively and has a C scanner, which is supported. |
| Query captures are unsupported by themes. | Prefer documented captures; use fallback captures where a theme may not define a specialized one. Validate by inspecting the query output and manual Zed highlighting. |
| Duplicate language or grammar names. | Use `unity_*` grammar names; language names are unique. |
| `.shader` is claimed by another extension. | No current registry extension claims `.shader`; verify after install. `shader-ls` provides an LSP, not a ShaderLab language. |
| Tasks require a beta CLI and sometimes a running Editor. | Document requirements and failure modes; keep commands optional and isolated from language support. |
| Multiple formats in one extension may look scope-broad to registry reviewers. | Frame it as Unity-ecosystem language support; all formats are Unity-specific and share one purpose. |
| Large Unity YAML files are slow. | Use the same YAML parser Zed already ships; if performance is unacceptable, consider a custom lean grammar later. |
| No Zed binary in the development environment. | Automated grammar/query validation plus a documented manual dev-extension pass; ask the user to run the final Zed check. |
| Prettier or another formatter corrupts Unity files. | Do not set `prettier_parser_name` for any language. |

## 12. Follow-ups

- **Semantic completion:** UXML element/attribute completion, `.asmdef` assembly
  references, and USS completion require a language server. The phased plan, reuse
  inventory, and open decisions are in [`ROADMAP.md`](../../../ROADMAP.md); this work is
  deferred beyond `0.1.0`.
- **Additional snippets:** add more Unity snippet files under `snippets/` and register
  them in `extension.toml`. Snippet files are scoped by filename (`csharp.json` for C#).
- **`.meta` automation:** a companion Unity Editor package (safe while Unity runs) or an
  LSP that handles `workspace/willRenameFiles` (works while Unity is closed). Both are
  separate projects.
- **Unity-side integration:** `.sln`/`.csproj` generation, `.zed/settings.json` writing,
  and external-editor registration belong to the Unity-side extension.
- **Optional format expansions:** `.shadergraph` / `.shadersubgraph` (JSON), `.inputactions`
  (JSON), and `.cs`-adjacent snippets. Each needs its own scope decision.

## 13. Success criteria

The design succeeds when a Unity developer can:

1. Install the extension and open any supported Unity file with the correct language mode.
2. Read Unity-specific structures without mentally parsing generic JSON/YAML/XML/CSS.
3. Navigate scenes, prefabs, and UXML files through outlines.
4. Run the common Unity CLI loop from Zed without leaving the editor.
5. Trust that `.cs` behavior and existing extensions are unaffected.
6. Understand exactly what the extension cannot do (`.meta` automation, semantic
   validation, binary assets) and where to go for those needs.

## 14. Decisions and confirmations

- `.tss` and `.spriteatlasv2` are included in v1.
- All five Unity CLI tasks are attached to all five Unity languages.
- The author string is `Dust Games Studio`; replace it with a name/email before
  publishing if desired.
- The repository must be public before the registry submission.
