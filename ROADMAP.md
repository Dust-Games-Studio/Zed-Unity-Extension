# Unity for Zed — Roadmap

Last updated: 2026-11-02

## Current state

`0.1.0` is a declarative Zed extension:

- Five languages: Unity Assembly Definition, Unity YAML, Unity UXML, Unity USS,
  Unity ShaderLab.
- Pinned Tree-sitter grammars, highlighting, outlines, brackets, indentation, and
  ShaderLab injections.
- Five Unity CLI tasks and 101 Unity C# snippets.
- Preflight and publish documentation in [PUBLISHING.md](PUBLISHING.md).

This release has **no semantic completion**. Zed only provides dynamic completion
through a language server; a declarative extension cannot compute project-specific
values such as "other assemblies" in an `.asmdef` or the available children of a
UXML element.

## The decision to make

Adding completion changes the extension's architecture. It is a separate release,
not a patch to `0.1.0`.

| Option | What it means | Cost | Accuracy |
|---|---|---|---|
| A. Stay declarative | Keep static snippets and queries only | None | No dynamic completion |
| B. Add a Rust/LSP layer to this extension | The extension becomes an LSP client; it downloads/starts language servers | Medium to high | Good for UXML/USS, good for asmdef |
| C. Add a Unity Editor companion package | Unity package exposes project data to a local server | High | Best, including C# reflection |
| D. Separate companion extension | Another extension provides the servers | Low per repo, but users install two extensions | Same as B |

**Recommendation: B.** Keep one extension, add a Rust client layer, and reuse
existing servers where possible. Add C only for features that genuinely need the
Unity Editor.

Publish `0.1.0` first. Start completion work as `0.2.0` so the language release is
not blocked by server distribution and platform testing.

## What can be reused

| Need | Reuse | Notes |
|---|---|---|
| UXML tags, attributes, enums, validation | [LemMinX](https://github.com/eclipse-lemminx/lemminx) + Unity's generated `UIElementsSchema/UIElements.xsd` | Unity already generates the XSD, including custom `UxmlFactory` elements. The MIT [zed-xml](https://github.com/sweetppro/zed-xml) extension already downloads and runs a native LemMinX binary. |
| USS properties, values, hover, diagnostics, formatting | [unity_code_native](https://github.com/hackerzhuli/unity_code_native) USS language server | MIT, Rust, `tower-lsp`. The current GitHub release has no prebuilt assets yet, so distribution is the open question. |
| USS property/value tables | [helizac/uxml-uss-language-tools](https://github.com/helizac/uxml-uss-language-tools) | MIT. VS Code provider code and data tables; not an LSP, but useful reference or extraction source. |
| `.asmdef` completion | Nothing existing | Needs a small custom LSP. |
| C#-aware UXML (`binding-path`, serialized fields, custom attributes beyond XSD) | Roslyn, Unity `CompilationPipeline`, or a Unity package | Defer; decide after UXML/asmdef phases. |

Unity's UXML XSD is the key leverage point: it gives element, attribute, and enum
completion for free, including custom elements, without a Unity-side package. The
file is normally at `UIElementsSchema/UIElements.xsd`; UXML files created by Unity
reference it with `xsi:noNamespaceSchemaLocation`.

## Phased plan

### Phase 0 — UXML/LemMinX spike

**Goal:** Prove that LemMinX gives useful completion on a real Unity project.

- Run the existing `zed-xml` extension's LemMinX binary against a Unity project.
- Associate `.uxml` with `UIElementsSchema/UIElements.xsd`.
- Verify tag completion, attribute completion, enum values, custom elements, and
  diagnostics.
- Check Unity 2022.3 and Unity 6 projects, including a project without a generated
  schema.
- Record the result and decide whether a static fallback schema is needed.

**Exit:** a short findings note with screenshots or capture output. No extension
changes yet.

### Phase 1 — Rust/LSP foundation

**Goal:** Make this repository a Rust extension that can launch language servers
without breaking the declarative language support.

- Add `Cargo.toml`, `src/lib.rs`, and the Zed `[lib]` manifest entry.
- Add a `[language_servers.unity-uxml]` entry for `Unity UXML`.
- Implement `language_server_command` for LemMinX: use a configured path, a
  `lemminx` on `PATH`, or download a checksummed native release, following the
  MIT `zed-xml` approach.
- Keep the existing language configs, grammars, snippets, and tasks unchanged.
- Add settings passthrough for LemMinX catalogs/file associations, or rely on the
  `xsi:noNamespaceSchemaLocation` in UXML files.
- Verify the extension still builds as WASM and installs as a dev extension.

**Exit:** `Unity UXML` starts LemMinX and completion works; `0.1.0` language
behavior is unchanged.

### Phase 2 — UXML completion and validation

**Goal:** Ship Rider-like UXML editing.

- Default `.uxml` files to `UIElementsSchema/UIElements.xsd`.
- Support custom elements and attributes from the project's generated schema.
- Fall back to a bundled static schema when Unity has not generated one.
- Document the schema-generation step and the fallback.
- Test against Unity 2022.3, Unity 6, and a project with custom `UxmlFactory`
  elements.
- Decide whether to expose LemMinX diagnostics as Zed diagnostics or keep them
  opt-in.

**Exit:** tag/attribute/enum completion and validation on the fixtures and a real
Unity project.

### Phase 3 — `.asmdef` language server MVP

**Goal:** Complete "other assemblies" and the rest of the asmdef schema.

This has no reusable server; build a small LSP. Keep it a separate binary release
so the Zed extension can download it rather than bundle it.

MVP features:

- Index `.asmdef` and `.asmref` files under the project, excluding `Library`,
  `Temp`, `Logs`, `obj`, and build directories.
- Complete `references` with assembly names and `GUID:` forms.
- Complete `includePlatforms` / `excludePlatforms` with Unity platform names.
- Complete `versionDefines[].name` from `Packages/manifest.json` and
  `packages-lock.json`, and `define` values as free text.
- Complete `precompiledReferences` from DLLs in the project/package cache.
- Diagnostics: unresolved references, duplicate assembly names, reference cycles,
  and an `.asmref` pointing at a missing assembly.
- Go-to-definition from a reference to the `.asmdef`/`.asmref` that declares it.
- Watch `.asmdef`, `.asmref`, `manifest.json`, and `packages-lock.json` for changes.

Distribution: platform-specific binaries on GitHub Releases with SHA-256 checksums;
the extension downloads the matching binary. Do not bundle it in the extension.

**Exit:** completion and diagnostics for the asmdef fixture set and a real Unity
project, with cross-platform release artifacts.

### Phase 4 — USS completion

**Goal:** Complete USS properties, values, selectors, and asset URLs.

Choose one:

- Reuse `unity_code_native` if a stable cross-platform release exists or can be
  produced; or
- Build a small USS LSP using the MIT property/value tables from the `helizac`
  extension.

Scope: property names, Unity-specific properties, finite value enums, selector
classes/IDs, pseudo-classes, `url()` asset paths, hover docs, and diagnostics.
Decide whether formatting/rename are in scope or deferred.

**Exit:** USS completion and diagnostics in a real project, without interfering with
the built-in CSS language server on `.css` files.

### Phase 5 — C#-aware UXML (research spike)

**Goal:** Decide how to complete `binding-path`, `data-source-type`, serialized
fields, USS class names, and custom attributes that are not in the XSD.

Options:

- **Unity Editor companion package:** reads `CompilationPipeline` assemblies,
  `SerializedObject` fields, `VisualTreeAsset`, and USS classes. Most accurate; adds
  package installation and requires the Editor.
- **Roslyn/assembly inspection:** parse `.sln`/`.csproj` or assemblies without the
  Editor. Less accurate for Unity serialization and custom `UxmlTraits`.
- **Hybrid:** Editor package for live data, file-based fallback when the Editor is
  closed.

Do not implement until Phase 2 and Phase 3 are shipped. Write a separate spec for
the chosen path.

### Phase 6 — Hardening and publishing

- Cross-platform CI for the Rust extension and LSP binaries.
- Download/checksum tests; offline and proxy failure messages.
- Settings documentation and troubleshooting.
- Versioned server compatibility matrix (Unity 2022.3 through 6.x).
- Update `PUBLISHING.md` for the Rust extension and downloaded servers.
- Registry review: the extension now provides language servers, so Rust is allowed,
  but it must not bundle the servers.

## Non-goals

- Reimplementing Rider or a full C# IDE.
- Owning C# language support; the official C# extension and Roslyn remain in charge.
- Bundling language servers in the extension.
- Requiring a Unity Editor package for UXML tag/attribute completion.
- Completing Unity YAML internals or ShaderLab semantics in this roadmap.

## Open questions

1. Ship `0.1.0` before adding Rust, or hold the release for Phase 1?
   Recommendation: ship `0.1.0` now.
2. One extension or a separate language-server extension? Recommendation: one.
3. For USS, wait for `unity_code_native` releases or build a small server?
4. Is a static UXML fallback schema acceptable when Unity has not generated
   `UIElementsSchema`?
5. Is C#-aware UXML worth a Unity Editor companion package, or is XSD completion
   enough for v1?

## Success criteria

A Unity developer in Zed can:

- Complete UXML element and attribute names from the project's schema.
- Complete `.asmdef` references with real assembly names or GUIDs.
- Complete USS property names and common values.
- Get actionable diagnostics for broken assembly references and invalid UXML/USS.
- Keep the existing highlighting, snippets, tasks, and C# extension behavior.
