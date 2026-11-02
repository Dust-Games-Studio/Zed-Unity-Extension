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
