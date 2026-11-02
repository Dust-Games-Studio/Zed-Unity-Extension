# Unity Zed Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a declarative Zed extension that gives Unity developers first-class editing support for assembly definitions, Unity YAML assets, UI Toolkit UXML/USS, and ShaderLab, plus a small set of Unity CLI tasks.

**Architecture:** A single `extension.toml`-driven Zed extension with five language directories. Each language maps a suffix family to a pinned Tree-sitter grammar and ships highlight/bracket/indent/outline queries. No Rust, no LSP, no bundled binaries. A local validation script clones the pinned grammars and runs parse/query checks against fixtures.

**Tech Stack:** Zed extension manifest and language configs, Tree-sitter queries, TOML, JSON, Bash, Python 3.11+ (validation only).

**Spec:** `docs/superpowers/specs/2026-11-02-unity-zed-extension-design.md`

## Global Constraints

- Extension ID: `unity`; name: `Unity`; version: `0.1.0`; license: MIT.
- Repository: `https://github.com/Dust-Games-Studio/Zed-Unity-Extension`.
- No `Cargo.toml`, no `src/`, no Rust, no language server, no bundled binaries.
- **Do not create, edit, stage, or commit `snippets/`.** It is owned by a separate snippet workstream. `extension.toml` carries a comment reserving that integration point.
- **Do not stage or commit `csharp-main/`.** It is a read-only reference checkout and is gitignored.
- No `.sln`/`.csproj` generation, no `.zed/settings.json` writing, no Unity Editor package, no external-editor registration.
- No C# language ownership and no C# snippets in this plan.
- Grammar names are unique: `unity_json`, `unity_yaml`, `unity_xml`, `unity_css`, `unity_shaderlab`.
- Pinned grammar commits:
  - `tree-sitter/tree-sitter-json` at `254c42a6476413b776221e03982ac8ae159eeb72`
  - `zed-industries/tree-sitter-yaml` at `baff0b51c64ef6a1fb1f8390f3ad6015b83ec13a`
  - `tree-sitter-grammars/tree-sitter-xml` at `5000ae8f22d11fbe93939b05c1e37cf21117162d`, `path = "xml"`
  - `tree-sitter/tree-sitter-css` at `dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f`
  - `msagca/tree-sitter-shaderlab` at `67e8a4e67b1026889eb41709cea66ec1bcfa6ad5`
- No formatter is configured for any language (`prettier_parser_name` absent everywhere).
- Unity CLI tasks are verified against `unity 1.0.0-beta.15`; the README presents them as optional.
- All user-facing text is English.
- Every commit stages explicit paths. Never use `git add -A` or `git add .`.

## Review Focus

These are the failure modes most likely to bite a user that the happy-path fixtures do not fully exercise. Each line names the expected behavior and the task that pins it.

1. **Binary-serialized Unity assets** (`.unity`, `.prefab`, `.asset`) are not YAML. They must fail visibly rather than silently produce a misleading tree; the README must tell users to switch Unity to Force Text. Pinned by `tests/fixtures/invalid/sample-invalid-binary.asset` and `scripts/validate.sh --self-test`.
2. **Malformed JSON/UXML/USS/ShaderLab** must produce parser error nodes; UXML and USS queries capture `(ERROR) @error`. Pinned by the invalid fixtures and `--self-test`.
3. **ShaderLab without the HLSL extension installed** must still parse and highlight the ShaderLab structure; embedded HLSL is optional. Pinned by the ShaderLab fixture parse and the injections query validating in `validate.sh`, plus the manual checklist.
4. **Missing Unity CLI or disconnected Editor** must not affect language support; tasks fail with the CLI's own error in the task terminal. Pinned by the README requirements and the manual checklist.
5. **Large Unity YAML files** must parse without crashing. Pinned by `scripts/validate.sh --stress`, which generates a ~400 KB YAML file.

---

### Task 1: Repository scaffolding and manifest

**Files:**
- Create: `.gitignore`
- Create: `LICENSE`
- Create: `extension.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: the extension ID, grammar declarations, and ignore rules used by every later task.

- [ ] **Step 1: Write `.gitignore`**

```gitignore
# Reference checkout; never commit
/csharp-main/

# Grammar cache created by scripts/validate.sh
/tests/.grammars/

# Rust/tooling leftovers (the extension itself has no Rust code)
/target/
/grammars/
*.wasm

# Editor and OS noise
.DS_Store
*.swp
*.swo
.idea/
.vscode/
```

- [ ] **Step 2: Write `LICENSE`**

Use the standard MIT text with this copyright line:

```text
MIT License

Copyright (c) 2026 Dust Games Studio

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 3: Write `extension.toml`**

```toml
id = "unity"
name = "Unity"
version = "0.1.0"
schema_version = 1
authors = ["Dust Games Studio"]
license = "MIT"
description = "Unity file format support: assembly definitions, Unity YAML assets, UI Toolkit UXML/USS, and ShaderLab."
repository = "https://github.com/Dust-Games-Studio/Zed-Unity-Extension"

# Snippet registration is owned by the separate snippet workstream.
# Keep the grammar and language declarations below stable for that integration.

[grammars.unity_json]
repository = "https://github.com/tree-sitter/tree-sitter-json"
commit = "254c42a6476413b776221e03982ac8ae159eeb72"

[grammars.unity_yaml]
repository = "https://github.com/zed-industries/tree-sitter-yaml"
commit = "baff0b51c64ef6a1fb1f8390f3ad6015b83ec13a"

[grammars.unity_xml]
repository = "https://github.com/tree-sitter-grammars/tree-sitter-xml"
commit = "5000ae8f22d11fbe93939b05c1e37cf21117162d"
path = "xml"

[grammars.unity_css]
repository = "https://github.com/tree-sitter/tree-sitter-css"
commit = "dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f"

[grammars.unity_shaderlab]
repository = "https://github.com/msagca/tree-sitter-shaderlab"
commit = "67e8a4e67b1026889eb41709cea66ec1bcfa6ad5"
```

- [ ] **Step 4: Validate the manifest and ignore rules**

Run:

```bash
python3 -c 'import tomllib; tomllib.load(open("extension.toml", "rb"))'
git status --short
git check-ignore -v csharp-main/ || true
grep -n 'tests/.grammars' .gitignore
```

Expected: the Python command exits 0; `git status` shows the three new files plus any untracked work from other streams; `git check-ignore` reports the `csharp-main/` rule; `grep` prints the `tests/.grammars/` rule.

- [ ] **Step 5: Commit**

```bash
git add .gitignore LICENSE extension.toml
git commit -m "chore: scaffold Unity Zed extension manifest"
```

---

### Task 2: Validation harness, fixtures, and negative tests

**Files:**
- Create: `scripts/validate.sh`
- Create: `tests/fixtures/sample.asmdef`
- Create: `tests/fixtures/sample.asmref`
- Create: `tests/fixtures/sample-guid.asmref`
- Create: `tests/fixtures/sample.unity`
- Create: `tests/fixtures/sample.prefab`
- Create: `tests/fixtures/sample.asset`
- Create: `tests/fixtures/sample.meta`
- Create: `tests/fixtures/sample.uxml`
- Create: `tests/fixtures/sample.uss`
- Create: `tests/fixtures/sample.tss`
- Create: `tests/fixtures/sample.shader`
- Create: `tests/fixtures/invalid/sample-invalid.asmdef`
- Create: `tests/fixtures/invalid/sample-invalid.uxml`
- Create: `tests/fixtures/invalid/sample-invalid.uss`
- Create: `tests/fixtures/invalid/sample-invalid.shader`
- Create: `tests/fixtures/invalid/sample-invalid.unity`
- Create: `tests/fixtures/invalid/sample-invalid-binary.asset`

**Interfaces:**
- Consumes: `extension.toml` from Task 1.
- Produces: `scripts/validate.sh`, invoked by every later language task. It accepts `--fixtures-only`, `--self-test`, `--stress`, and `--all`.

- [ ] **Step 1: Write `scripts/validate.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GRAMMARS_DIR="${GRAMMARS_DIR:-$ROOT/tests/.grammars}"
TS_BIN="${TS_BIN:-tree-sitter}"
MODE="${1:-all}"

say() { printf '%s\n' "$*"; }
progress() { printf '%s\n' "$*" >&2; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

command -v "$TS_BIN" >/dev/null 2>&1 || fail "tree-sitter CLI not found. Install it with 'cargo install tree-sitter-cli' or download it from https://github.com/tree-sitter/tree-sitter/releases."
command -v python3 >/dev/null 2>&1 || fail "python3 is required for TOML/JSON validation."
python3 -c 'import tomllib' >/dev/null 2>&1 || fail "Python 3.11+ is required for TOML validation (tomllib)."

grammar_repo() {
  case "$1" in
    unity_json) echo "https://github.com/tree-sitter/tree-sitter-json" ;;
    unity_yaml) echo "https://github.com/zed-industries/tree-sitter-yaml" ;;
    unity_xml) echo "https://github.com/tree-sitter-grammars/tree-sitter-xml" ;;
    unity_css) echo "https://github.com/tree-sitter/tree-sitter-css" ;;
    unity_shaderlab) echo "https://github.com/msagca/tree-sitter-shaderlab" ;;
    *) return 1 ;;
  esac
}

grammar_commit() {
  case "$1" in
    unity_json) echo "254c42a6476413b776221e03982ac8ae159eeb72" ;;
    unity_yaml) echo "baff0b51c64ef6a1fb1f8390f3ad6015b83ec13a" ;;
    unity_xml) echo "5000ae8f22d11fbe93939b05c1e37cf21117162d" ;;
    unity_css) echo "dda5cfc5722c429eaba1c910ca32c2c0c5bb1a3f" ;;
    unity_shaderlab) echo "67e8a4e67b1026889eb41709cea66ec1bcfa6ad5" ;;
    *) return 1 ;;
  esac
}

grammar_subdir() {
  case "$1" in
    unity_xml) echo "xml" ;;
    *) echo "" ;;
  esac
}

ensure_generated() {
  local name="$1" dir="$2" stamp="$GRAMMARS_DIR/.generated/$name"
  if [ ! -f "$stamp" ]; then
    progress "Generating $name parser..."
    ( cd "$dir" && "$TS_BIN" generate >/dev/null 2>&1 ) || fail "tree-sitter generate failed for $name"
    mkdir -p "$(dirname "$stamp")"
    touch "$stamp"
  fi
}

ensure_grammar() {
  local name="$1" repo commit dir sub
  repo="$(grammar_repo "$name")"
  commit="$(grammar_commit "$name")"
  dir="$GRAMMARS_DIR/$name"
  if [ ! -d "$dir/.git" ]; then
    progress "Cloning $name..."
    mkdir -p "$GRAMMARS_DIR"
    git init -q "$dir"
    git -C "$dir" remote add origin "$repo"
    git -C "$dir" fetch -q --depth 1 origin "$commit"
    git -C "$dir" checkout -q FETCH_HEAD
  fi
  sub="$(grammar_subdir "$name")"
  if [ -n "$sub" ]; then
    dir="$dir/$sub"
  fi
  ensure_generated "$name" "$dir"
  echo "$dir"
}

fixture_grammar() {
  case "$1" in
    *.asmdef|*.asmref) echo unity_json ;;
    *.unity|*.prefab|*.asset|*.meta) echo unity_yaml ;;
    *.uxml) echo unity_xml ;;
    *.uss|*.tss) echo unity_css ;;
    *.shader) echo unity_shaderlab ;;
    *) return 1 ;;
  esac
}

language_grammar() {
  case "$1" in
    unity-assembly-definition) echo unity_json ;;
    unity-yaml) echo unity_yaml ;;
    unity-uxml) echo unity_xml ;;
    unity-uss) echo unity_css ;;
    unity-shaderlab) echo unity_shaderlab ;;
    *) return 1 ;;
  esac
}

language_fixtures() {
  case "$1" in
    unity-assembly-definition)
      printf '%s\n' \
        "$ROOT/tests/fixtures/sample.asmdef" \
        "$ROOT/tests/fixtures/sample.asmref" \
        "$ROOT/tests/fixtures/sample-guid.asmref"
      ;;
    unity-yaml)
      printf '%s\n' \
        "$ROOT/tests/fixtures/sample.unity" \
        "$ROOT/tests/fixtures/sample.prefab" \
        "$ROOT/tests/fixtures/sample.asset" \
        "$ROOT/tests/fixtures/sample.meta"
      ;;
    unity-uxml)
      printf '%s\n' "$ROOT/tests/fixtures/sample.uxml"
      ;;
    unity-uss)
      printf '%s\n' "$ROOT/tests/fixtures/sample.uss" "$ROOT/tests/fixtures/sample.tss"
      ;;
    unity-shaderlab)
      printf '%s\n' "$ROOT/tests/fixtures/sample.shader"
      ;;
    *) return 1 ;;
  esac
}

validate_toml() {
  python3 - "$1" <<'PY'
import sys, tomllib
with open(sys.argv[1], "rb") as f:
    tomllib.load(f)
PY
}

validate_json() {
  python3 -m json.tool "$1" >/dev/null
}

parse_fixtures() {
  say "Parsing fixtures..."
  local fixture grammar dir
  while IFS= read -r fixture; do
    grammar="$(fixture_grammar "$fixture")" || fail "No grammar mapping for $fixture"
    dir="$(ensure_grammar "$grammar")"
    ( cd "$dir" && "$TS_BIN" parse --quiet "$fixture" >/dev/null 2>&1 ) || fail "Parse failed: $fixture ($grammar)"
    say "  ok  $fixture"
  done < <(find "$ROOT/tests/fixtures" -maxdepth 1 -type f | sort)
}

self_test_invalid() {
  say "Checking that malformed fixtures fail to parse..."
  local fixture grammar dir
  for fixture in "$ROOT"/tests/fixtures/invalid/*; do
    [ -e "$fixture" ] || continue
    grammar="$(fixture_grammar "$fixture")" || fail "No grammar mapping for $fixture"
    dir="$(ensure_grammar "$grammar")"
    if ( cd "$dir" && "$TS_BIN" parse --quiet "$fixture" >/dev/null 2>&1 ); then
      fail "Expected parse failure but succeeded: $fixture"
    fi
    say "  ok (fails as expected) $fixture"
  done
}

stress_yaml() {
  say "Stress-testing large Unity YAML..."
  local dir out i
  dir="$(ensure_grammar unity_yaml)"
  out="$GRAMMARS_DIR/large.meta"
  {
    printf 'fileFormatVersion: 2\n'
    printf 'guid: 17b36165d09634a48bf5a0e4bb27f4bd\n'
    i=0
    while [ "$i" -lt 20000 ]; do
      printf 'key%s: value%s\n' "$i" "$i"
      i=$((i + 1))
    done
  } > "$out"
  ( cd "$dir" && "$TS_BIN" parse --quiet "$out" >/dev/null 2>&1 ) || fail "Large YAML parse failed"
  say "  ok  large YAML ($(wc -c < "$out") bytes)"
  rm -f "$out"
}

validate_queries() {
  say "Validating language configs and queries..."
  local lang_dir lang grammar fixture query dir
  for lang_dir in "$ROOT"/languages/*; do
    [ -d "$lang_dir" ] || continue
    lang="$(basename "$lang_dir")"
    grammar="$(language_grammar "$lang")" || fail "No grammar mapping for language $lang"
    dir="$(ensure_grammar "$grammar")"
    [ -f "$lang_dir/config.toml" ] || fail "Missing config.toml in $lang_dir"
    validate_toml "$lang_dir/config.toml" || fail "Invalid TOML: $lang_dir/config.toml"
    if [ -f "$lang_dir/tasks.json" ]; then
      validate_json "$lang_dir/tasks.json" || fail "Invalid JSON: $lang_dir/tasks.json"
    fi
    while IFS= read -r fixture; do
      [ -n "$fixture" ] || continue
      for query in "$lang_dir"/*.scm; do
        [ -e "$query" ] || continue
        ( cd "$dir" && "$TS_BIN" query "$query" "$fixture" >/dev/null 2>&1 ) || fail "Query failed: $query against $fixture"
      done
    done < <(language_fixtures "$lang")
    say "  ok  $lang"
  done
}

case "$MODE" in
  --fixtures-only)
    parse_fixtures
    self_test_invalid
    ;;
  --self-test)
    self_test_invalid
    ;;
  --stress)
    stress_yaml
    ;;
  all|--all)
    validate_toml "$ROOT/extension.toml" || fail "Invalid TOML: extension.toml"
    parse_fixtures
    self_test_invalid
    validate_queries
    say "All checks passed."
    ;;
  *)
    echo "Usage: $0 [--fixtures-only|--self-test|--stress|--all]" >&2
    exit 2
    ;;
esac
```

- [ ] **Step 2: Make the script executable**

```bash
chmod +x scripts/validate.sh
```

- [ ] **Step 3: Write `tests/fixtures/sample.asmdef`**

```json
{
    "name": "MyGame.Runtime",
    "rootNamespace": "MyGame",
    "references": [
        "UnityEngine.UI",
        "GUID:17b36165d09634a48bf5a0e4bb27f4bd"
    ],
    "includePlatforms": ["Android", "WebGL"],
    "excludePlatforms": [],
    "allowUnsafeCode": false,
    "overrideReferences": true,
    "precompiledReferences": ["Newtonsoft.Json.dll"],
    "autoReferenced": false,
    "defineConstraints": ["UNITY_2022_3_OR_NEWER"],
    "versionDefines": [
        {
            "name": "com.unity.test-framework",
            "expression": "[1.3,2.0]",
            "define": "MY_TESTS"
        }
    ],
    "noEngineReferences": false
}
```

- [ ] **Step 4: Write `tests/fixtures/sample.asmref` and `tests/fixtures/sample-guid.asmref`**

`sample.asmref`:

```json
{
    "reference": "MyGame.Runtime"
}
```

`sample-guid.asmref`:

```json
{
    "reference": "GUID:17b36165d09634a48bf5a0e4bb27f4bd"
}
```

- [ ] **Step 5: Write `tests/fixtures/sample.unity`**

```yaml
%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!29 &1
OcclusionCullingSettings:
  m_ObjectHideFlags: 0
  serializedVersion: 2
  m_OcclusionBakeSettings:
    smallestOccluder: 5
    smallestHole: 0.25
    backfaceThreshold: 100
  m_SceneGUID: 00000000000000000000000000000000
  m_OcclusionCullingData: {fileID: 0}
--- !u!1 &123456
GameObject:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  serializedVersion: 6
  m_Component:
  - component: {fileID: 123457}
  - component: {fileID: 123458}
  m_Layer: 0
  m_Name: Player
  m_TagString: Player
  m_Icon: {fileID: 0}
  m_NavMeshLayer: 0
  m_StaticEditorFlags: 0
  m_IsActive: 1
--- !u!4 &123457
Transform:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  m_GameObject: {fileID: 123456}
  serializedVersion: 2
  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}
  m_LocalPosition: {x: 0, y: 0, z: 0}
  m_LocalScale: {x: 1, y: 1, z: 1}
  m_ConstrainProportionsScale: 0
  m_Children: []
  m_Father: {fileID: 0}
  m_LocalEulerAnglesHint: {x: 0, y: 0, z: 0}
--- !u!114 &123458
MonoBehaviour:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  m_GameObject: {fileID: 123456}
  m_Enabled: 1
  m_EditorHideFlags: 0
  m_Script: {fileID: 11500000, guid: 17b36165d09634a48bf5a0e4bb27f4bd, type: 3}
  m_Name:
  m_EditorClassIdentifier:
--- !u!1001 &123459
PrefabInstance:
  m_ObjectHideFlags: 0
  serializedVersion: 2
  m_Modification:
    serializedVersion: 3
    m_TransformParent: {fileID: 0}
    m_Modifications: []
    m_RemovedComponents: []
    m_RemovedGameObjects: []
    m_AddedGameObjects: []
    m_AddedComponents: []
  m_SourcePrefab: {fileID: 100100000, guid: 17b36165d09634a48bf5a0e4bb27f4bd, type: 3}
```

- [ ] **Step 6: Write `tests/fixtures/sample.prefab`**

```yaml
%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!1 &100000
GameObject:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  serializedVersion: 6
  m_Component:
  - component: {fileID: 100001}
  - component: {fileID: 100002}
  m_Layer: 0
  m_Name: Weapon
  m_TagString: Untagged
  m_Icon: {fileID: 0}
  m_NavMeshLayer: 0
  m_StaticEditorFlags: 0
  m_IsActive: 1
--- !u!4 &100001
Transform:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  m_GameObject: {fileID: 100000}
  serializedVersion: 2
  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}
  m_LocalPosition: {x: 0, y: 0, z: 0}
  m_LocalScale: {x: 1, y: 1, z: 1}
  m_Children: []
  m_Father: {fileID: 0}
--- !u!114 &100002
MonoBehaviour:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  m_GameObject: {fileID: 100000}
  m_Enabled: 1
  m_EditorHideFlags: 0
  m_Script: {fileID: 11500000, guid: 17b36165d09634a48bf5a0e4bb27f4bd, type: 3}
  m_Name:
  m_EditorClassIdentifier:
  damage: 25
  fireRate: 4.5
```

- [ ] **Step 7: Write `tests/fixtures/sample.asset`**

```yaml
%YAML 1.1
%TAG !u! tag:unity3d.com,2011:
--- !u!114 &11400000
MonoBehaviour:
  m_ObjectHideFlags: 0
  m_CorrespondingSourceObject: {fileID: 0}
  m_PrefabInstance: {fileID: 0}
  m_PrefabAsset: {fileID: 0}
  m_GameObject: {fileID: 0}
  m_Enabled: 1
  m_EditorHideFlags: 0
  m_Script: {fileID: 11500000, guid: 17b36165d09634a48bf5a0e4bb27f4bd, type: 3}
  m_Name: GameConfig
  m_EditorClassIdentifier:
  maxHealth: 100
  moveSpeed: 5.5
  startingWeapon: {fileID: 100000, guid: 17b36165d09634a48bf5a0e4bb27f4bd, type: 3}
```

- [ ] **Step 8: Write `tests/fixtures/sample.meta`**

```yaml
fileFormatVersion: 2
guid: 17b36165d09634a48bf5a0e4bb27f4bd
MonoImporter:
  externalObjects: {}
  serializedVersion: 2
  defaultReferences: []
  executionOrder: 0
  icon: {instanceID: 0}
  userData:
  assetBundleName:
  assetBundleVariant:
```

- [ ] **Step 9: Write `tests/fixtures/sample.uxml`**

```xml
<ui:UXML xmlns:ui="UnityEngine.UIElements" xmlns:uie="UnityEditor.UIElements">
    <ui:Style src="sample.uss" />
    <ui:VisualElement name="root" class="container">
        <ui:Label text="Hello" usage-hints="DynamicTransform" />
        <ui:Button name="play-button" text="Play" class="primary-button" />
        <ui:ListView name="items" binding-path="m_Items" />
    </ui:VisualElement>
</ui:UXML>
```

- [ ] **Step 10: Write `tests/fixtures/sample.uss` and `tests/fixtures/sample.tss`**

`sample.uss` (valid USS subset; no inline styles, no unsupported CSS properties):

```css
:root {
    --unity-primary-color: rgb(255, 100, 0);
    --unity-font-size: 14px;
}

.container {
    flex-grow: 1;
    padding: 8px;
    background-color: var(--unity-primary-color);
    -unity-font-style: bold;
    -unity-text-align: middle-center;
}

#play-button:hover {
    opacity: 0.85;
    background-color: rgb(255, 120, 20);
}
```

`sample.tss`:

```css
@import url("sample.uss");

:root {
    --unity-accent-color: rgb(0, 120, 255);
}

.unity-theme-root {
    background-color: var(--unity-accent-color);
}
```

- [ ] **Step 11: Write `tests/fixtures/sample.shader`**

```hlsl
Shader "Custom/UnlitColor"
{
    Properties
    {
        _Color ("Color", Color) = (1,1,1,1)
        _MainTex ("Texture", 2D) = "white" {}
        [Range(0,1)] _Cutoff ("Cutoff", Range(0,1)) = 0.5
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" }
        LOD 100
        Cull Off
        ZWrite On

        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"

            struct appdata { float4 vertex : POSITION; float2 uv : TEXCOORD0; };
            struct v2f { float2 uv : TEXCOORD0; float4 vertex : SV_POSITION; };
            sampler2D _MainTex;
            float4 _Color;

            v2f vert (appdata v) {
                v2f o;
                o.vertex = UnityObjectToClipPos(v.vertex);
                o.uv = v.uv;
                return o;
            }
            fixed4 frag (v2f i) : SV_Target {
                return tex2D(_MainTex, i.uv) * _Color;
            }
            ENDCG
        }

        Pass
        {
            HLSLPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "Packages/com.unity.render-pipelines.universal/ShaderLibrary/Core.hlsl"

            float4 vert (float4 positionOS : POSITION) : SV_POSITION
            {
                return TransformObjectToHClip(positionOS.xyz);
            }

            half4 frag () : SV_Target
            {
                return half4(1, 1, 1, 1);
            }
            ENDHLSL
        }
    }
    FallBack "Diffuse"
}
```

- [ ] **Step 12: Write the invalid fixtures**

`tests/fixtures/invalid/sample-invalid.asmdef`:

```json
{"name": }
```

`tests/fixtures/invalid/sample-invalid.uxml`:

```xml
<ui:UXML><ui:VisualElement></ui:UXML>
```

`tests/fixtures/invalid/sample-invalid.uss`:

```css
.broken { color red; }
```

`tests/fixtures/invalid/sample-invalid.shader`:

```hlsl
Shader "Broken" {
```

`tests/fixtures/invalid/sample-invalid.unity`:

```yaml
%YAML 1.1
--- !u!1 &1
GameObject: [unclosed
```

`tests/fixtures/invalid/sample-invalid-binary.asset` is written with non-text bytes so the YAML parser must fail:

```bash
printf '\x80\x81\x82\x83' > tests/fixtures/invalid/sample-invalid-binary.asset
```

- [ ] **Step 13: Run the fixture and negative tests**

Run:

```bash
./scripts/validate.sh --fixtures-only
./scripts/validate.sh --self-test
./scripts/validate.sh --stress
```

Expected:
- Every valid fixture prints `ok`.
- Every invalid fixture prints `ok (fails as expected)`.
- The stress run prints `ok large YAML (<size> bytes)`.
- No `FAIL` lines.

- [ ] **Step 14: Commit**

```bash
git add scripts/validate.sh tests/fixtures
git commit -m "test: add grammar fixtures and validation harness"
```

---

### Task 3: Unity Assembly Definition language

**Files:**
- Create: `languages/unity-assembly-definition/config.toml`
- Create: `languages/unity-assembly-definition/highlights.scm`
- Create: `languages/unity-assembly-definition/brackets.scm`
- Create: `languages/unity-assembly-definition/indents.scm`
- Create: `languages/unity-assembly-definition/outline.scm`
- Create: `languages/unity-assembly-definition/tasks.json`

**Interfaces:**
- Consumes: `unity_json` grammar from `extension.toml`; fixtures from Task 2.
- Produces: the `Unity Assembly Definition` language, its query files, and the canonical five-task `tasks.json` copied by Tasks 4–7.

- [ ] **Step 1: Write `config.toml`**

```toml
name = "Unity Assembly Definition"
grammar = "unity_json"
path_suffixes = ["asmdef", "asmref"]
tab_size = 4
hard_tabs = false
autoclose_before = ",]}"
brackets = [
    { start = "{", end = "}", close = true, surround = true, newline = true },
    { start = "[", end = "]", close = true, surround = true, newline = true },
    { start = "\"", end = "\"", close = true, surround = true, newline = false, not_in = ["string"] },
]
```

- [ ] **Step 2: Write `highlights.scm`**

```scheme
(comment) @comment

(string) @string

(escape_sequence) @string.escape

(number) @number

[
  (true)
  (false)
] @boolean

(null) @constant.builtin

[
  ","
  ":"
] @punctuation.delimiter

[
  "{"
  "}"
  "["
  "]"
] @punctuation.bracket

(pair
  key: (string) @property)

(pair
  key: (string (string_content) @_key)
  (#any-of? @_key "name" "rootNamespace" "reference")
  value: (string (string_content) @type))

((string (string_content) @string.special)
  (#match? @string.special "^GUID:"))

(pair
  key: (string (string_content) @_key)
  (#any-of? @_key "includePlatforms" "excludePlatforms")
  value: (array (string (string_content) @constant.builtin)))

(pair
  key: (string (string_content) @_key)
  (#eq? @_key "defineConstraints")
  value: (array (string (string_content) @constant)))

(pair
  key: (string (string_content) @_key)
  (#eq? @_key "precompiledReferences")
  value: (array (string (string_content) @string)))

(pair
  key: (string (string_content) @_key)
  (#eq? @_key "versionDefines")
  value: (array
    (object
      (pair
        key: (string (string_content) @_vk)
        (#eq? @_vk "name")
        value: (string (string_content) @string))
      (pair
        key: (string (string_content) @_vk2)
        (#eq? @_vk2 "expression")
        value: (string (string_content) @string.special))
      (pair
        key: (string (string_content) @_vk3)
        (#eq? @_vk3 "define")
        value: (string (string_content) @constant)))))
```

- [ ] **Step 3: Write `brackets.scm`**

```scheme
("[" @open
  "]" @close)

("{" @open
  "}" @close)

(("\"" @open
  "\"" @close)
  (#set! rainbow.exclude))
```

- [ ] **Step 4: Write `indents.scm`**

```scheme
(array
  "]" @end) @indent

(object
  "}" @end) @indent
```

- [ ] **Step 5: Write `outline.scm`**

```scheme
(document
  (object
    (pair
      key: (string (string_content) @name)) @item))

(array
  (object
    (pair
      key: (string (string_content) @_key)
      (#eq? @_key "name")
      value: (string (string_content) @name)) @item))
```

- [ ] **Step 6: Write `tasks.json`**

```json
[
  {
    "label": "Unity: Recompile scripts",
    "command": "unity",
    "args": ["recompile", "--project-path", "$ZED_WORKTREE_ROOT", "--timeout", "120"],
    "use_new_terminal": false,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "reveal_target": "center",
    "hide": "on_success",
    "shell": "system",
    "save": "all"
  },
  {
    "label": "Unity: Run EditMode tests",
    "command": "unity",
    "args": ["test", "$ZED_WORKTREE_ROOT", "--mode", "EditMode", "--output", "$ZED_WORKTREE_ROOT/Logs/zed-editmode-results.xml"],
    "use_new_terminal": false,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "reveal_target": "center",
    "hide": "never",
    "shell": "system",
    "save": "all"
  },
  {
    "label": "Unity: Run PlayMode tests",
    "command": "unity",
    "args": ["test", "$ZED_WORKTREE_ROOT", "--mode", "PlayMode", "--output", "$ZED_WORKTREE_ROOT/Logs/zed-playmode-results.xml"],
    "use_new_terminal": false,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "reveal_target": "center",
    "hide": "never",
    "shell": "system",
    "save": "all"
  },
  {
    "label": "Unity: Check .meta pairing",
    "command": "unity",
    "args": ["vcs", "status", "--project-path", "$ZED_WORKTREE_ROOT"],
    "use_new_terminal": false,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "reveal_target": "center",
    "hide": "on_success",
    "shell": "system",
    "save": "none"
  },
  {
    "label": "Unity: Editor status",
    "command": "unity",
    "args": ["status", "--project-path", "$ZED_WORKTREE_ROOT"],
    "use_new_terminal": false,
    "allow_concurrent_runs": false,
    "reveal": "always",
    "reveal_target": "center",
    "hide": "on_success",
    "shell": "system",
    "save": "none"
  }
]
```

- [ ] **Step 7: Run validation**

Run:

```bash
./scripts/validate.sh all
```

Expected: every fixture prints `ok`, every invalid fixture prints `ok (fails as expected)`, `ok unity-assembly-definition`, and `All checks passed.`

- [ ] **Step 8: Commit**

```bash
git add languages/unity-assembly-definition
git commit -m "feat: add Unity assembly definition language support"
```

---

### Task 4: Unity YAML language

**Files:**
- Create: `languages/unity-yaml/config.toml`
- Create: `languages/unity-yaml/highlights.scm`
- Create: `languages/unity-yaml/brackets.scm`
- Create: `languages/unity-yaml/indents.scm`
- Create: `languages/unity-yaml/outline.scm`
- Create: `languages/unity-yaml/tasks.json`

**Interfaces:**
- Consumes: `unity_yaml` grammar; fixtures `sample.unity`, `sample.prefab`, `sample.asset`, `sample.meta`.
- Produces: the `Unity YAML` language covering Unity's serialized text assets.

- [ ] **Step 1: Write `config.toml`**

```toml
name = "Unity YAML"
grammar = "unity_yaml"
path_suffixes = [
    "unity", "prefab", "asset", "meta", "mat", "anim", "controller",
    "overrideController", "physicMaterial", "physicsMaterial2D", "renderTexture",
    "cubemap", "flare", "guiskin", "fontsettings", "spriteatlas", "spriteatlasv2",
    "shadervariants", "lighting", "brush", "terrainlayer", "mask", "preset",
    "signal", "mixer", "playable",
]
line_comments = ["# "]
tab_size = 2
autoclose_before = ",]}"
brackets = [
    { start = "{", end = "}", close = true, newline = true },
    { start = "[", end = "]", close = true, newline = true },
    { start = "\"", end = "\"", close = true, newline = false, not_in = ["string"] },
    { start = "'", end = "'", close = true, newline = false, not_in = ["string"] },
]
auto_indent_on_paste = false
auto_indent_using_last_non_empty_line = false
increase_indent_pattern = "(:?^[^#]*:\\s*[|>]?\\s*$)|(:?^\\s*-[^#]*:\\s*(:?#+.*)$)"
```

- [ ] **Step 2: Write `highlights.scm`**

```scheme
(boolean_scalar) @boolean

(null_scalar) @constant.builtin

[
  (double_quote_scalar)
  (single_quote_scalar)
  (block_scalar)
  (string_scalar)
] @string

(escape_sequence) @string.escape

[
  (integer_scalar)
  (float_scalar)
] @number

(comment) @comment

[
  (anchor_name)
  (alias_name)
] @label

(tag) @type

(yaml_directive) @preproc
(tag_directive) @preproc

key: (flow_node
  [
    (plain_scalar
      (string_scalar))
    (double_quote_scalar)
    (single_quote_scalar)
  ] @property)

[
  ","
  "-"
  ":"
  ">"
  "?"
  "|"
] @punctuation.delimiter

[
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  "*"
  "&"
  "---"
  "..."
] @punctuation.special

((plain_scalar) @string.special
  (#match? @string.special "^[0-9a-fA-F]{32}$"))

((tag) @type @type.builtin
  (#match? @type.builtin "^!u![0-9]+$"))
```

- [ ] **Step 3: Write `brackets.scm`**

```scheme
("[" @open
  "]" @close)

("{" @open
  "}" @close)

(("\"" @open
  "\"" @close)
  (#set! rainbow.exclude))

(("'" @open
  "'" @close)
  (#set! rainbow.exclude))
```

- [ ] **Step 4: Write `indents.scm`**

Indentation is driven by `increase_indent_pattern` in `config.toml`, matching Zed's built-in YAML language. Write a no-op file with an explanatory comment:

```scheme
; Unity YAML indentation is driven by `increase_indent_pattern` in config.toml.
; Zed's built-in YAML language does not ship an indents.scm for the same reason.
```

- [ ] **Step 5: Write `outline.scm`**

```scheme
(document
  (block_node
    (tag)? @context
    (anchor)? @context
    (block_mapping
      (block_mapping_pair
        key: (flow_node
          (plain_scalar
            (string_scalar) @name)) @item))))

(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @_key))
  (#eq? @_key "m_Name")
  value: (flow_node
    (plain_scalar
      (string_scalar) @name))) @item
```

- [ ] **Step 6: Copy the canonical tasks file**

```bash
cp languages/unity-assembly-definition/tasks.json languages/unity-yaml/tasks.json
```

- [ ] **Step 7: Run validation**

Run:

```bash
./scripts/validate.sh all
```

Expected: `ok unity-assembly-definition`, `ok unity-yaml`, and `All checks passed.` The YAML queries are validated against all four YAML fixtures.

- [ ] **Step 8: Commit**

```bash
git add languages/unity-yaml
git commit -m "feat: add Unity YAML language support"
```

---

### Task 5: Unity UXML language

**Files:**
- Create: `languages/unity-uxml/config.toml`
- Create: `languages/unity-uxml/highlights.scm`
- Create: `languages/unity-uxml/indents.scm`
- Create: `languages/unity-uxml/outline.scm`
- Create: `languages/unity-uxml/tasks.json`

**Interfaces:**
- Consumes: `unity_xml` grammar (`path = "xml"`); `sample.uxml`.
- Produces: the `Unity UXML` language.

- [ ] **Step 1: Write `config.toml`**

```toml
name = "Unity UXML"
grammar = "unity_xml"
path_suffixes = ["uxml"]
block_comment = ["<!-- ", " -->"]
autoclose_before = "<"
brackets = [{ start = "<", end = ">", close = true, newline = true }]
tab_size = 4
hard_tabs = false
```

- [ ] **Step 2: Write `highlights.scm`**

```scheme
"xml" @keyword

[ "version" "encoding" "standalone" ] @property

(EncName) @string.special
(VersionNum) @number
[ "yes" "no" ] @boolean

(PI) @embedded
(PI (PITarget) @keyword)

(STag (Name) @tag)
(ETag (Name) @tag)
(EmptyElemTag (Name) @tag)

(Attribute (Name) @property)
(Attribute (AttValue) @string)

[
  "<?" "?>"
  "<!" "]]>"
  "<" ">"
  "</" "/>"
] @punctuation.delimiter

[ "(" ")" "[" "]" ] @punctuation.bracket
[ "\"" "'" ] @punctuation.delimiter
[ "," "|" "=" ] @operator

(CharData) @markup
(CDSect
  (CDStart) @markup.heading
  (CData) @markup.raw
  "]]>" @markup.heading)
(Comment) @comment
(ERROR) @error

((STag (Name) @type.builtin)
  (#match? @type.builtin "^(ui:|uie:)?(UXML|VisualElement|Button|Label|TextField|Toggle|Slider|SliderInt|MinMaxSlider|ProgressBar|ScrollView|ListView|TreeView|MultiColumnListView|MultiColumnTreeView|Foldout|Box|Image|IMGUIContainer|Template|Instance|Style|AttributeOverrides|Bindings|Binding|DataBinding|Editor|Toolbar|ToolbarButton|ToolbarToggle|ToolbarMenu|ToolbarSearchField|ToolbarBreadcrumbs|DropdownField|EnumField|ObjectField|ColorField|CurveField|GradientField|LayerMaskField|MaskField|TagField|PropertyField|HelpBox|RadioButton|RadioButtonGroup|GroupBox|PopupWindow|Window|TextElement)$"))

((Attribute (Name) @attribute)
  (#match? @attribute "^(name|class|style|text|binding-path|data-source-path|data-source-type|picking-mode|usage-hints|tooltip|focusable|tabindex|enabled|visible|content-container|xmlns|xmlns:ui|xmlns:uie|xmlns:xsi)$"))
```

- [ ] **Step 3: Write `indents.scm`**

```scheme
(STag ">" @end) @indent

(EmptyElemTag "/>" @end) @indent

(element
  (STag) @start
  (ETag)? @end) @indent
```

- [ ] **Step 4: Write `outline.scm`**

```scheme
(element
  (STag
    (Name) @name)) @item

(EmptyElemTag
  (Name) @name) @item
```

- [ ] **Step 5: Copy the canonical tasks file**

```bash
cp languages/unity-assembly-definition/tasks.json languages/unity-uxml/tasks.json
```

- [ ] **Step 6: Run validation**

Run:

```bash
./scripts/validate.sh all
```

Expected: `ok unity-assembly-definition`, `ok unity-yaml`, `ok unity-uxml`, and `All checks passed.`

- [ ] **Step 7: Commit**

```bash
git add languages/unity-uxml
git commit -m "feat: add Unity UXML language support"
```

---

### Task 6: Unity USS language

**Files:**
- Create: `languages/unity-uss/config.toml`
- Create: `languages/unity-uss/highlights.scm`
- Create: `languages/unity-uss/brackets.scm`
- Create: `languages/unity-uss/indents.scm`
- Create: `languages/unity-uss/outline.scm`
- Create: `languages/unity-uss/tasks.json`

**Interfaces:**
- Consumes: `unity_css` grammar; `sample.uss` and `sample.tss`.
- Produces: the `Unity USS` language for `.uss` and `.tss`.

- [ ] **Step 1: Write `config.toml`**

```toml
name = "Unity USS"
grammar = "unity_css"
path_suffixes = ["uss", "tss"]
block_comment = ["/* ", " */"]
autoclose_before = ";:.,=}])>"
brackets = [
    { start = "{", end = "}", close = true, newline = true },
    { start = "[", end = "]", close = true, newline = true },
    { start = "(", end = ")", close = true, newline = true },
    { start = "\"", end = "\"", close = true, newline = false, not_in = ["string", "comment"] },
    { start = "'", end = "'", close = true, newline = false, not_in = ["string", "comment"] },
]
tab_size = 4
hard_tabs = false
word_characters = ["#"]
completion_query_characters = ["-", "@"]
```

- [ ] **Step 2: Write `highlights.scm`**

```scheme
(comment) @comment

[
  (tag_name)
  (nesting_selector)
  (universal_selector)
] @tag

[
  "~"
  ">"
  "+"
  "-"
  "|"
  "*"
  "/"
  "="
  "^="
  "|="
  "~="
  "$="
  "*="
] @operator

[
  "and"
  "or"
  "not"
  "only"
] @keyword.operator

(id_name) @selector.id
(class_name) @selector.class
(namespace_name) @namespace

(namespace_selector
  (tag_name) @namespace
  "|")

(attribute_name) @attribute

(pseudo_element_selector
  "::"
  (tag_name) @selector.pseudo)

(pseudo_class_selector
  ":"
  (class_name) @selector.pseudo)

[
  (feature_name)
  (property_name)
] @property

(function_name) @function

[
  (plain_value)
  (keyframes_name)
  (keyword_query)
] @constant.builtin

(attribute_selector
  (plain_value) @string)

(parenthesized_query
  (keyword_query) @property)

([
  (property_name)
  (plain_value)
] @variable
  (#match? @variable "^--"))

[
  "@media"
  "@import"
  "@charset"
  "@namespace"
  "@supports"
  "@keyframes"
  (at_keyword)
  (to)
  (from)
  (important)
] @keyword

(string_value) @string
(color_value) @string.special

[
  (integer_value)
  (float_value)
] @number

(unit) @type.unit

[
  ","
  ":"
  "."
  "::"
  ";"
] @punctuation.delimiter

(id_selector
  "#" @punctuation.delimiter)

[
  "{"
  ")"
  "("
  "}"
  "["
  "]"
] @punctuation.bracket

; `--unity-*` variables are covered by the generic `^--` rule above.
; `-unity-*` properties are covered by the generic `(property_name) @property` rule.
((function_name) @function @function.builtin
  (#any-of? @function.builtin "var" "resource" "url"))
```

- [ ] **Step 3: Write `brackets.scm`**

```scheme
("[" @open "]" @close)
("{" @open "}" @close)
("(" @open ")" @close)
```

- [ ] **Step 4: Write `indents.scm`**

```scheme
(block
  "}" @end) @indent
```

- [ ] **Step 5: Write `outline.scm`**

```scheme
(rule_set
  (selectors) @name) @item
```

- [ ] **Step 6: Copy the canonical tasks file**

```bash
cp languages/unity-assembly-definition/tasks.json languages/unity-uss/tasks.json
```

- [ ] **Step 7: Run validation**

Run:

```bash
./scripts/validate.sh all
```

Expected: `ok unity-assembly-definition`, `ok unity-yaml`, `ok unity-uxml`, `ok unity-uss`, and `All checks passed.`

- [ ] **Step 8: Commit**

```bash
git add languages/unity-uss
git commit -m "feat: add Unity USS language support"
```

---

### Task 7: Unity ShaderLab language

**Files:**
- Create: `languages/unity-shaderlab/config.toml`
- Create: `languages/unity-shaderlab/highlights.scm`
- Create: `languages/unity-shaderlab/brackets.scm`
- Create: `languages/unity-shaderlab/indents.scm`
- Create: `languages/unity-shaderlab/outline.scm`
- Create: `languages/unity-shaderlab/injections.scm`
- Create: `languages/unity-shaderlab/tasks.json`

**Interfaces:**
- Consumes: `unity_shaderlab` grammar; `sample.shader` with CG and HLSL blocks.
- Produces: the `Unity ShaderLab` language and optional HLSL/GLSL injections.

- [ ] **Step 1: Write `config.toml`**

```toml
name = "Unity ShaderLab"
grammar = "unity_shaderlab"
path_suffixes = ["shader"]
line_comments = ["// "]
block_comment = ["/* ", " */"]
autoclose_before = "{}()[];:.,="
brackets = [
    { start = "{", end = "}", close = true, newline = true },
    { start = "[", end = "]", close = true, newline = true },
    { start = "(", end = ")", close = true, newline = true },
    { start = "\"", end = "\"", close = true, newline = false, not_in = ["string", "comment"] },
]
tab_size = 4
hard_tabs = false
```

- [ ] **Step 2: Write `highlights.scm`**

```scheme
(comment) @comment

(string) @string

(number) @number

(identifier) @variable

[
  "Shader"
  "Properties"
  "SubShader"
  "Pass"
  "Tags"
  "Name"
  "LOD"
  "UsePass"
  "GrabPass"
  "PackageRequirements"
  "CustomEditor"
  "CustomEditorForRenderPipeline"
  "Fallback"
  "Dependency"
  "Stencil"
] @keyword

[
  "HLSLPROGRAM"
  "HLSLINCLUDE"
  "ENDHLSL"
  "CGPROGRAM"
  "CGINCLUDE"
  "ENDCG"
  "GLSLPROGRAM"
  "ENDGLSL"
] @keyword

(fallback
  "Off" @constant.builtin)

(shader
  name: (string) @label)

(name
  (string) @label)

(use_pass
  (string) @label)

(command_name) @function

(stencil_field) @property

(render_state
  argument: (identifier) @constant.builtin)

(legacy_command
  argument: (identifier) @constant.builtin)

(stencil_operation
  value: (identifier) @constant.builtin)

(legacy_block
  (identifier) @constant.builtin)

(property
  name: (identifier) @property)

(property_reference
  name: (identifier) @property)

(attribute
  name: (identifier) @attribute)

(tag
  key: (string) @property)

(property_type) @type.builtin

(property_type
  (identifier) @type.builtin)

[
  "Range"
  "Vector"
] @type.builtin
```

- [ ] **Step 3: Write `brackets.scm`**

```scheme
("{" @open "}" @close)
("[" @open "]" @close)
("(" @open ")" @close)
```

- [ ] **Step 4: Write `indents.scm`**

```scheme
(shader
  "}" @end) @indent

(properties
  "}" @end) @indent

(subshader
  "}" @end) @indent

(pass
  "}" @end) @indent

(stencil
  "}" @end) @indent

(tags
  "}" @end) @indent

(hlsl_block
  "ENDHLSL" @end) @indent

(cg_block
  "ENDCG" @end) @indent

(glsl_block
  "ENDGLSL" @end) @indent
```

- [ ] **Step 5: Write `outline.scm`**

```scheme
(shader
  name: (string) @name) @item

(property
  name: (identifier) @name) @item
```

- [ ] **Step 6: Write `injections.scm`**

```scheme
((hlsl_block
  (code) @injection.content)
  (#set! injection.language "hlsl"))

((cg_block
  (code) @injection.content)
  (#set! injection.language "hlsl"))

((glsl_block
  (code) @injection.content)
  (#set! injection.language "glsl"))
```

- [ ] **Step 7: Copy the canonical tasks file**

```bash
cp languages/unity-assembly-definition/tasks.json languages/unity-shaderlab/tasks.json
```

- [ ] **Step 8: Run validation**

Run:

```bash
./scripts/validate.sh all
```

Expected: all five languages print `ok`, all fixtures and invalid fixtures pass, and `All checks passed.`

- [ ] **Step 9: Commit**

```bash
git add languages/unity-shaderlab
git commit -m "feat: add Unity ShaderLab language support"
```

---

### Task 8: README, setup guidance, and publishing polish

**Files:**
- Create: `README.md`

**Interfaces:**
- Consumes: the finished extension from Tasks 1–7.
- Produces: installation, C#/Roslyn setup, task requirements, `.meta` safety guidance, limitations, development instructions, and publishing notes.

- [ ] **Step 1: Write `README.md`**

Use this content:

````markdown
# Unity for Zed

Zed language support for Unity projects: assembly definitions, Unity's YAML-serialized
assets, UI Toolkit UXML/USS, and ShaderLab.

This extension is declarative. It ships no language server, no Rust code, and no
binaries. C# remains handled by Zed's official C# extension.

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
- Unity YAML highlighting for `!u!` class tags, anchors, `%YAML`/`%TAG` directives,
  `m_*` keys, 32-hex GUIDs, and `fileID` values.
- Outlines for assembly fields, serialized objects and `m_Name`, UXML elements, USS rule
  sets, shaders, and shader properties.
- Bracket matching and indentation tuned for each format.
- Unity CLI tasks available from supported Unity files.
- Optional embedded HLSL/CG highlighting in ShaderLab when an HLSL language is installed.

## Installation

Install **Unity** from the Zed extension registry, or install this repository as a dev
extension with `zed: install dev extension`.

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
3. Detect pairing problems with `unity vcs status` or `unity vcs doctor`.

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

Only text-serialized Unity assets are supported. If a `.unity`, `.prefab`, or `.asset`
file uses Unity's binary serialization, it will show parse errors. Set
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
- **Grammar build errors:** open `zed: open log` and check the grammar build output. Zed
  downloads `wasi-sdk` automatically; no Rust toolchain is required for this extension.

## Development

Prerequisites:

- A `tree-sitter` CLI on `PATH` (`cargo install tree-sitter-cli` or a release binary).
- Python 3.11+ for TOML/JSON validation.
- Git (the validation script fetches pinned grammar commits into `tests/.grammars/`).

Run all checks:

```bash
./scripts/validate.sh
```

Useful modes:

```bash
./scripts/validate.sh --fixtures-only   # parse fixtures only
./scripts/validate.sh --self-test       # confirm malformed fixtures fail
./scripts/validate.sh --stress          # parse a generated ~400 KB YAML file
```

The script is local-only. It clones each pinned grammar into `tests/.grammars/`
(gitignored) and validates every language config and `.scm` query against its fixtures.

Manual Zed check:

1. Install the repository as a dev extension.
2. Open each fixture in `tests/fixtures/` and verify language mode, highlighting, outline,
   comments, brackets, and indentation.
3. Confirm `.cs` still uses the official C# extension.
4. Open `task: spawn` from a Unity YAML file and confirm all five tasks appear.
5. Run the checks that your environment supports and report any query or grammar errors.

## Future work

- C# snippets are handled by a separate snippet workstream.
- `.meta` automation requires a Unity Editor package or a language server using
  `workspace/willRenameFiles`; it is not part of this extension.
- `.shadergraph`, `.shadersubgraph`, and `.inputactions` support are candidate future
  formats.

## Publishing

The extension is published by making this repository public and adding it as a submodule
at `extensions/unity` in `zed-industries/extensions`, with a `[unity]` entry in
`extensions.toml`. The manifest must build in Zed's extension CI.

## License

MIT. See `LICENSE`.
````

- [ ] **Step 2: Validate the complete repository**

Run:

```bash
./scripts/validate.sh all
python3 -c 'import tomllib; tomllib.load(open("extension.toml", "rb"))'
grep -nE 'TBD|TODO|PLACEHOLDER' README.md || true
git status --short
```

Expected:
- `All checks passed.`
- The TOML command exits 0.
- The `grep` finds no placeholder markers.
- `git status` shows only the new `README.md` (plus untracked work from other streams).

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document installation, setup, and limitations"
```

---

### Task 9: Manual validation in Zed (human step)

**Files:**
- Modify: none.

**Interfaces:**
- Consumes: the complete extension.
- Produces: a confirmed working extension or a list of query/language issues to fix.

- [ ] **Step 1: Install as a dev extension**

In Zed, run `zed: install dev extension` and select the repository root. Confirm the
extension appears without build errors.

- [ ] **Step 2: Check each language mode**

Open these fixtures and confirm the language mode shown in the status bar:

| Fixture | Expected mode |
|---|---|
| `tests/fixtures/sample.asmdef` | Unity Assembly Definition |
| `tests/fixtures/sample.unity` | Unity YAML |
| `tests/fixtures/sample.meta` | Unity YAML |
| `tests/fixtures/sample.uxml` | Unity UXML |
| `tests/fixtures/sample.uss` | Unity USS |
| `tests/fixtures/sample.shader` | Unity ShaderLab |

Also open a `.cs` file and confirm it still uses C#.

- [ ] **Step 3: Check highlighting and navigation**

For each fixture, verify:
- Highlighting is visibly applied (keys, types, strings, GUIDs, tags).
- The outline contains the expected entries.
- Comment toggling works where applicable.
- Brackets match and indentation behaves.
- Invalid fixtures show parser errors rather than silently succeeding.

- [ ] **Step 4: Check tasks**

From `sample.unity`, run `task: spawn` and confirm all five Unity tasks are listed.
Run `Unity: Editor status` if a Unity Editor is running, and `Unity: Check .meta pairing`
in a Git/UVCS project. Confirm the terminal shows the CLI's output.

- [ ] **Step 5: Check logs**

Run `zed: open log` and confirm there are no grammar build, query, or language
registration errors.

- [ ] **Step 6: Report results**

Record any failures with the fixture, expected behavior, actual behavior, and the relevant
log line. Fix queries or configs and repeat Tasks 3–8 as needed.

---

## Self-Review

**Spec coverage:** Tasks 1–8 cover every section of the spec: manifest and grammars (§5,
§10), five languages (§6.1–§6.5), tasks (§7), repository layout (§8), validation (§9),
README/publishing (§10), risks (§11), and follow-ups (§12). Task 9 covers the manual
acceptance criteria (§9.3–§9.4) and success criteria (§13).

**Placeholder scan:** No `TBD`, `TODO`, or “implement later” steps. Every code-bearing
step contains the actual file content or an exact command.

**Type consistency:** Grammar names (`unity_json`, `unity_yaml`, `unity_xml`, `unity_css`,
`unity_shaderlab`), language directory names, and task labels are used consistently across
the manifest, configs, validation script, and README.

**Review Focus tests:** Binary assets are pinned by the invalid binary fixture and
`--self-test`; malformed files are pinned by the remaining invalid fixtures; ShaderLab
without HLSL is pinned by the fixture parse and injection query validation; missing CLI or
Editor is covered by the README and Task 9; large YAML is pinned by `--stress`.

## Execution Handoff

After saving and self-reviewing this plan, ask the human partner to review it and choose an
execution method: subagent-driven (recommended for independent per-language tasks) or
native. Do not begin implementation until they confirm.
