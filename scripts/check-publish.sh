#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STRICT=0

for arg in "$@"; do
  case "$arg" in
    --strict)
      STRICT=1
      ;;
    -h|--help)
      cat <<'EOF'
Usage: ./scripts/check-publish.sh [--strict]

Checks this repository against the Zed extension publishing prerequisites.
With --strict, warnings are treated as failures.
EOF
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$arg" >&2
      exit 2
      ;;
  esac
done

command -v python3 >/dev/null 2>&1 || {
  printf 'FAIL: python3 is required for the publishing preflight.\n' >&2
  exit 1
}
python3 -c 'import tomllib' >/dev/null 2>&1 || {
  printf 'FAIL: Python 3.11+ is required for TOML validation (tomllib).\n' >&2
  exit 1
}

exec python3 - "$ROOT" "$STRICT" <<'PY'
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

root = Path(sys.argv[1])
strict = sys.argv[2] == "1"

passes = []
warnings = []
notes = []
errors = []


def ok(message):
    passes.append(message)


def warn(message):
    warnings.append(message)


def note(message):
    notes.append(message)


def err(message):
    errors.append(message)


def load_toml(path):
    with path.open("rb") as handle:
        return tomllib.load(handle)


def git(*args):
    return subprocess.run(
        ["git", "-C", str(root), *args],
        text=True,
        capture_output=True,
    )


def tracked_files():
    result = git("ls-files")
    if result.returncode != 0:
        err(f"git ls-files failed: {result.stderr.strip()}")
        return []
    return [line for line in result.stdout.splitlines() if line]


manifest_path = root / "extension.toml"
manifest = {}
if not manifest_path.is_file():
    err("extension.toml is missing")
else:
    try:
        manifest = load_toml(manifest_path)
        ok("extension.toml parses")
    except Exception as exc:  # noqa: BLE001 - report the parser error verbatim
        err(f"extension.toml does not parse: {exc}")

if manifest:
    extension_id = manifest.get("id")
    version = manifest.get("version")
    license_name = manifest.get("license")
    repository = manifest.get("repository")
    authors = manifest.get("authors")
    description = manifest.get("description")
    schema_version = manifest.get("schema_version")

    if not isinstance(extension_id, str) or not re.fullmatch(
        r"[a-z0-9]+(?:-[a-z0-9]+)*", extension_id
    ):
        err("id must be kebab-cased (for example: unity)")
    elif "zed" in extension_id or "extension" in extension_id:
        err("id must not contain the words 'zed' or 'extension'")
    else:
        ok(f"id '{extension_id}' is kebab-cased and does not use reserved words")

    if not isinstance(manifest.get("name"), str) or not manifest.get("name"):
        err("name must be a non-empty string")
    else:
        ok("name is set")

    if not isinstance(version, str) or not re.fullmatch(
        r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version
    ):
        err("version must be semver (for example: 0.1.0)")
    else:
        ok(f"version '{version}' is semver")

    if schema_version != 1:
        err("schema_version must be 1")
    else:
        ok("schema_version is 1")

    accepted_licenses = {
        "apache-2.0",
        "bsd-2-clause",
        "bsd-3-clause",
        "cc-by-4.0",
        "gpl-3.0",
        "lgpl-3.0",
        "mit",
        "unlicense",
        "zlib",
    }
    if not isinstance(license_name, str) or license_name.lower() not in accepted_licenses:
        err("license must be one of the Zed-accepted licenses (MIT is used here)")
    else:
        ok(f"license '{license_name}' is accepted by Zed")

    if not isinstance(repository, str) or not repository.startswith("https://"):
        err("repository must be an HTTPS URL")
    else:
        ok("repository is HTTPS")

    if not isinstance(authors, list) or not authors or not all(
        isinstance(author, str) and author.strip() for author in authors
    ):
        err("authors must be a non-empty list of strings")
    else:
        ok("authors are set")

    if not isinstance(description, str) or not description.strip():
        err("description must be non-empty")
    else:
        ok("description is set")

license_files = sorted(
    path
    for path in root.iterdir()
    if path.is_file() and path.name.lower().startswith(("license", "licence"))
)
if not license_files:
    err("no LICENSE file found at the repository root")
else:
    license_text = license_files[0].read_text(errors="replace").lower()
    if "mit license" in license_text or "permission is hereby granted" in license_text:
        ok(f"{license_files[0].name} contains an accepted license text")
    else:
        err(f"{license_files[0].name} does not look like an accepted license")

grammars = manifest.get("grammars", {})
if not isinstance(grammars, dict) or not grammars:
    err("extension.toml must declare at least one grammar")
else:
    ok(f"{len(grammars)} grammar(s) declared")
    for grammar_name, grammar in grammars.items():
        if not isinstance(grammar, dict):
            err(f"grammar '{grammar_name}' must be a table")
            continue
        repo = grammar.get("repository")
        if not isinstance(repo, str) or not repo.startswith("https://"):
            err(f"grammar '{grammar_name}' repository must be HTTPS")
        revision = grammar.get("rev") or grammar.get("commit")
        if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
            err(f"grammar '{grammar_name}' must pin a 40-character commit")
        if "path" in grammar and not isinstance(grammar["path"], str):
            err(f"grammar '{grammar_name}' path must be a string")

grammar_names = set(grammars) if isinstance(grammars, dict) else set()
languages_root = root / "languages"
language_dirs = (
    sorted(path for path in languages_root.iterdir() if path.is_dir())
    if languages_root.is_dir()
    else []
)
if not language_dirs:
    err("no language directories found under languages/")
else:
    ok(f"{len(language_dirs)} language directory(ies) found")

seen_language_names = set()
for language_dir in language_dirs:
    config_path = language_dir / "config.toml"
    if not config_path.is_file():
        err(f"{config_path.relative_to(root)} is missing")
        continue
    try:
        config = load_toml(config_path)
    except Exception as exc:  # noqa: BLE001 - report the parser error verbatim
        err(f"{config_path.relative_to(root)} does not parse: {exc}")
        continue

    language_name = config.get("name")
    grammar_name = config.get("grammar")
    suffixes = config.get("path_suffixes")

    if not isinstance(language_name, str) or not language_name.strip():
        err(f"{config_path.relative_to(root)} is missing a name")
    elif language_name in seen_language_names:
        err(f"duplicate language name '{language_name}'")
    else:
        seen_language_names.add(language_name)

    if grammar_name not in grammar_names:
        err(f"{config_path.relative_to(root)} references undeclared grammar '{grammar_name}'")

    if not isinstance(suffixes, list) or not suffixes:
        err(f"{config_path.relative_to(root)} needs a non-empty path_suffixes list")

    if "prettier_parser_name" in config:
        err(f"{config_path.relative_to(root)} must not configure a formatter")

    tasks_path = language_dir / "tasks.json"
    if not tasks_path.is_file():
        err(f"{tasks_path.relative_to(root)} is missing")
        continue
    try:
        tasks = json.loads(tasks_path.read_text())
    except Exception as exc:  # noqa: BLE001 - report the parser error verbatim
        err(f"{tasks_path.relative_to(root)} does not parse: {exc}")
        continue
    if not isinstance(tasks, list) or len(tasks) != 5:
        err(f"{tasks_path.relative_to(root)} must contain exactly five tasks")
        continue
    labels = []
    for task in tasks:
        if not isinstance(task, dict) or not task.get("label") or not task.get("command"):
            err(f"{tasks_path.relative_to(root)} has a task without a label or command")
            break
        labels.append(task["label"])
    if len(labels) != len(set(labels)):
        err(f"{tasks_path.relative_to(root)} has duplicate task labels")

ok("language configs and task files are present")

readme_path = root / "README.md"
if not readme_path.is_file():
    err("README.md is missing")
else:
    readme = readme_path.read_text(errors="replace")
    readme_failures = len(errors)
    required_readme = [
        "## Installation",
        "zed: install dev extension",
        "auto_install_extensions",
    ]
    for needle in required_readme:
        if needle not in readme:
            err(f"README.md is missing '{needle}'")
    if re.search(r"\b(TBD|TODO|PLACEHOLDER)\b", readme):
        err("README.md contains a placeholder marker")
    if len(errors) == readme_failures:
        ok("README.md covers registry and dev-extension installation")

snippets_manifest = manifest.get("snippets")
declared_snippets = set()
if isinstance(snippets_manifest, str):
    declared_snippets.add(snippets_manifest)
elif isinstance(snippets_manifest, list):
    for snippet_path in snippets_manifest:
        if not isinstance(snippet_path, str):
            err("snippets entries must be strings")
        else:
            declared_snippets.add(snippet_path)
elif snippets_manifest is not None:
    err("snippets must be a string or an array of strings")

if declared_snippets:
    ok(f"{len(declared_snippets)} snippet file(s) registered")
    for relative_path in sorted(declared_snippets):
        snippet_path = root / relative_path
        if not snippet_path.is_file():
            err(f"registered snippet file does not exist: {relative_path}")
            continue
        try:
            snippet_data = json.loads(snippet_path.read_text())
        except Exception as exc:  # noqa: BLE001 - report the parser error verbatim
            err(f"registered snippet file does not parse: {relative_path}: {exc}")
            continue
        if not isinstance(snippet_data, dict) or not snippet_data:
            err(f"registered snippet file must contain a non-empty object: {relative_path}")
            continue
        for snippet_name, snippet in snippet_data.items():
            if not isinstance(snippet, dict):
                err(f"{relative_path}: snippet '{snippet_name}' must be an object")
                break
            body = snippet.get("body")
            if not isinstance(body, (str, list)) or not body:
                err(f"{relative_path}: snippet '{snippet_name}' needs a non-empty body")
                break
            prefix = snippet.get("prefix")
            if prefix is not None and not isinstance(prefix, (str, list)):
                err(f"{relative_path}: snippet '{snippet_name}' prefix must be a string or array")
                break

tracked = tracked_files()
tracked_set = set(tracked)

rust_sources = [path for path in tracked if path.endswith(".rs")]
if rust_sources:
    lib_table = manifest.get("lib")
    if not isinstance(lib_table, dict) or lib_table.get("kind") != "Rust":
        err("extension.toml must declare [lib] kind = \"Rust\" when Rust source is tracked")
    else:
        ok("extension.toml declares a Rust extension library")

    servers = manifest.get("language_servers", {})
    if not isinstance(servers, dict) or not servers:
        err("Rust extension must declare at least one [language_servers] entry")
    else:
        ok(f"{len(servers)} language server(s) declared")
        forbidden_languages = {"CSharp", "Unity USS"}
        for server_name, server in servers.items():
            if not isinstance(server, dict):
                err(f"language server '{server_name}' must be a table")
                continue
            languages = server.get("languages")
            if languages is None and isinstance(server.get("language"), str):
                languages = [server["language"]]
            if not isinstance(languages, list) or not languages:
                err(f"language server '{server_name}' must declare languages")
                continue
            unknown = [language for language in languages if language not in seen_language_names]
            if unknown:
                err(
                    f"language server '{server_name}' references unknown language(s): "
                    + ", ".join(unknown)
                )
            forbidden = sorted(forbidden_languages.intersection(languages))
            if forbidden:
                err(
                    f"language server '{server_name}' must not attach to: "
                    + ", ".join(forbidden)
                )

    cargo_path = root / "Cargo.toml"
    if not cargo_path.is_file():
        err("Cargo.toml is required when Rust source is tracked")
    else:
        try:
            cargo = load_toml(cargo_path)
            ok("Cargo.toml parses")
        except Exception as exc:  # noqa: BLE001 - report the parser error verbatim
            cargo = {}
            err(f"Cargo.toml does not parse: {exc}")
        lib = cargo.get("lib", {}) if isinstance(cargo, dict) else {}
        crate_type = lib.get("crate-type", []) if isinstance(lib, dict) else []
        if "cdylib" not in crate_type:
            err("Cargo.toml [lib] must include crate-type = [\"cdylib\"]")
        else:
            ok("Cargo.toml builds a cdylib extension")
        workspace = cargo.get("workspace", {}) if isinstance(cargo, dict) else {}
        members = workspace.get("members", []) if isinstance(workspace, dict) else []
        if "lsp" not in members:
            err("Cargo.toml [workspace] must include lsp")
        else:
            ok("Cargo workspace includes the LSP crate")

    for required in ("src/lib.rs", "lsp/Cargo.toml", "lsp/src/main.rs"):
        if not (root / required).is_file():
            err(f"{required} is required for the Rust extension")
        elif required not in tracked_set:
            err(f"{required} exists but is not tracked in git")
    if "Cargo.lock" not in tracked_set:
        err("Cargo.lock must be tracked for reproducible builds")
    else:
        ok("Cargo.lock is tracked")

for relative_path in sorted(declared_snippets):
    if relative_path not in tracked_set:
        err(f"registered snippet file is not tracked in git: {relative_path}")

tracked_snippets = {
    path for path in tracked if path.startswith("snippets/") and path.endswith(".json")
}
if tracked_snippets:
    missing_snippets = sorted(tracked_snippets - declared_snippets)
    if missing_snippets:
        err(
            "snippet files are tracked but not registered in extension.toml: "
            + ", ".join(missing_snippets)
        )
    else:
        ok("all tracked snippet files are registered")

binary_suffixes = (
    ".exe",
    ".dll",
    ".so",
    ".dylib",
    ".wasm",
    ".zip",
    ".tar.gz",
    ".gz",
)
binary_magics = (
    b"\x7fELF",
    b"\0asm",
    b"MZ",
    b"\xfe\xed\xfa\xce",
    b"\xce\xfa\xed\xfe",
    b"\xfe\xed\xfa\xcf",
    b"\xcf\xfa\xed\xfe",
    b"\xca\xfe\xba\xbe",
)
fixture_placeholder_dlls = {
    path
    for path in tracked
    if path.startswith("tests/fixtures/lsp-project/") and path.endswith(".dll")
}

for path in tracked:
    if path.startswith("csharp-main/"):
        err("csharp-main/ is tracked; remove the Rust reference checkout from the publishable tree")
    if path.startswith("target/") or path.startswith("lsp/target/"):
        err(f"build output is tracked: {path}")
    if path.startswith("tests/.grammars/"):
        err("tests/.grammars/ is a local cache and must not be tracked")
    if path.startswith(".superpowers/"):
        err(".superpowers/ is local review scaffolding and must not be tracked")
    if path.rsplit("/", 1)[-1] in ("unity-asmdef-lsp", "unity-asmdef-lsp.exe"):
        err(f"the language server binary must not be bundled: {path}")
    if path.endswith(binary_suffixes) and path not in fixture_placeholder_dlls:
        err(f"tracked binary artifact: {path}")
    candidate = root / path
    if candidate.is_file() and path not in fixture_placeholder_dlls:
        try:
            head = candidate.read_bytes()[:4]
        except OSError:
            continue
        if candidate.stat().st_size < 16 * 1024 * 1024 and any(
            head.startswith(magic) for magic in binary_magics
        ):
            err(f"tracked file looks like a native binary: {path}")

status = git("status", "--porcelain")
if status.returncode != 0:
    err(f"git status failed: {status.stderr.strip()}")
elif status.stdout.strip():
    warn("working tree is dirty; commit the publishable state before submitting")
else:
    ok("working tree is clean")

branch = git("symbolic-ref", "-q", "--short", "HEAD")
if branch.returncode != 0 or not branch.stdout.strip():
    warn("HEAD is detached; the registry requires the submitted commit to be on a branch")
else:
    ok(f"HEAD is on branch '{branch.stdout.strip()}'")

print("Publishing preflight")
print("====================")
for message in passes:
    print(f"PASS  {message}")
for message in warnings:
    print(f"WARN  {message}")
for message in notes:
    print(f"NOTE  {message}")
for message in errors:
    print(f"FAIL  {message}")

print()
if errors:
    print("Publishing preflight failed. Fix the FAIL items above.")
    sys.exit(1)
if strict and warnings:
    print("Publishing preflight failed in --strict mode because of warnings.")
    sys.exit(1)

print("Publishing preflight passed.")
print()
print("Registry entry for zed-industries/extensions/extensions.toml:")
print()
print(f"[{manifest.get('id', '<id>')}]")
print(f"submodule = \"extensions/{manifest.get('id', '<id>')}\"")
print(f"version = \"{manifest.get('version', '<version>')}\"")
print()
print("Next steps:")
print("  1. Push main to the public repository; confirm the submitted commit is on a branch.")
print("  2. Complete the manual Zed check in PUBLISHING.md.")
print("  3. Fork zed-industries/extensions, add the HTTPS submodule, paste the entry above,")
print("     run pnpm sort-extensions, and open the pull request.")
PY
