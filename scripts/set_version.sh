#!/usr/bin/env bash
# Set the Unity extension release version across the files that matter.
#
# Usage:
#   ./scripts/set_version.sh 0.3.1
#   ./scripts/set_version.sh v0.3.1
#   ./scripts/set_version.sh 0.3.1 --bump-lsp
#   ./scripts/set_version.sh 0.3.1 --dry-run
#
# Updates:
#   - Cargo.toml       extension crate version
#   - extension.toml   Zed registry version
#   - src/lib.rs       LSP_TAG (must be v<extension-version>)
#   - Cargo.lock       refreshed by `cargo check --workspace`
#
# The asmdef LSP crate version in lsp/Cargo.toml is independent. Pass --bump-lsp
# when the asmdef server itself changed, so its binary version matches the release.

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/set_version.sh <version> [--bump-lsp] [--dry-run] [--no-lock]

  <version>     New version, for example 0.3.1 or v0.3.1.
  --bump-lsp    Also set the unity-asmdef-lsp crate version in lsp/Cargo.toml.
  --dry-run     Print what would change without writing files or running cargo.
  --no-lock     Skip `cargo check --workspace` and leave Cargo.lock untouched.
  -h, --help    Show this help.

The default updates Cargo.toml, extension.toml, src/lib.rs, and Cargo.lock.
EOF
}

error() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd -- "$script_dir/.." && pwd)"

cargo_toml="$root/Cargo.toml"
extension_toml="$root/extension.toml"
lsp_cargo_toml="$root/lsp/Cargo.toml"
lib_rs="$root/src/lib.rs"
cargo_lock="$root/Cargo.lock"

version_arg=""
bump_lsp=0
dry_run=0
no_lock=0

while (($# > 0)); do
  case "$1" in
    --bump-lsp) bump_lsp=1 ;;
    --dry-run) dry_run=1 ;;
    --no-lock) no_lock=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    --*)
      error "unknown option: $1"
      ;;
    *)
      if [[ -n "$version_arg" ]]; then
        error "expected one version, got '$version_arg' and '$1'"
      fi
      version_arg="$1"
      ;;
  esac
  shift
done

if [[ -z "$version_arg" ]]; then
  usage >&2
  exit 2
fi

version="${version_arg#v}"
if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
  error "invalid version '$version_arg'; expected MAJOR.MINOR.PATCH, optionally prefixed with 'v'"
fi

find_line() {
  local file="$1" mode="$2" line=""
  case "$mode" in
    package)
      line="$(awk '
        /^[[:space:]]*\[[^]]+\][[:space:]]*$/ {
          section = $0
          gsub(/^[[:space:]]+|[[:space:]]+$/, "", section)
          in_package = (section == "[package]")
          next
        }
        in_package && /^[[:space:]]*version[[:space:]]*=/ { print NR; exit }
      ' "$file")"
      ;;
    top-level)
      line="$(awk '
        /^[[:space:]]*\[/ { exit }
        /^[[:space:]]*version[[:space:]]*=/ { print NR; exit }
      ' "$file")"
      ;;
    *)
      error "internal error: unknown find_line mode '$mode'"
      ;;
  esac
  printf '%s' "$line"
}

replace_line() {
  local file="$1" line="$2" expression="$3" tmp
  tmp="$(mktemp)"
  sed -E "${line}s/${expression}/" "$file" >"$tmp"
  mv "$tmp" "$file"
}

get_extension_version() {
  local line
  line="$(find_line "$extension_toml" top-level)"
  [[ -n "$line" ]] || error "no top-level version found in $extension_toml"
  sed -n "${line}s/^[[:space:]]*version[[:space:]]*=[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$extension_toml"
}

set_extension_version() {
  local line
  line="$(find_line "$extension_toml" top-level)"
  [[ -n "$line" ]] || error "no top-level version found in $extension_toml"
  replace_line "$extension_toml" "$line" \
    '^([[:space:]]*version[[:space:]]*=[[:space:]]*")[^"]*(".*)$/\1'"$version"'\2'
}

set_package_version() {
  local file="$1" line
  line="$(find_line "$file" package)"
  [[ -n "$line" ]] || error "no [package] version found in $file"
  replace_line "$file" "$line" \
    '^([[:space:]]*version[[:space:]]*=[[:space:]]*")[^"]*(".*)$/\1'"$version"'\2'
}

set_lsp_tag() {
  local count tmp
  count="$(grep -c 'const LSP_TAG: &str = "' "$lib_rs" || true)"
  [[ "$count" -eq 1 ]] || error "expected exactly one LSP_TAG in $lib_rs, found $count"
  tmp="$(mktemp)"
  sed -E 's/(const LSP_TAG: &str = ")[^"]+(";)/\1'"v$version"'\2/' "$lib_rs" >"$tmp"
  mv "$tmp" "$lib_rs"
}

is_git_repo() {
  git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1
}

git_is_dirty() {
  is_git_repo || return 1
  [[ -n "$(git -C "$root" status --porcelain)" ]]
}

old_version="$(get_extension_version)"

if [[ "$old_version" == "$version" ]]; then
  printf 'Already at %s; nothing to do.\n' "$version"
  exit 0
fi

if ((dry_run)); then
  printf 'Would set %s -> %s:\n' "$old_version" "$version"
  printf '  Cargo.toml\n'
  printf '  extension.toml\n'
  printf '  src/lib.rs\n'
  if ((!no_lock)); then
    printf '  Cargo.lock (via cargo check --workspace)\n'
  fi
  printf '  LSP_TAG -> v%s\n' "$version"
  if ((!bump_lsp)); then
    printf '  lsp/Cargo.toml stays unchanged (pass --bump-lsp to update it)\n'
  fi
  exit 0
fi

if git_is_dirty; then
  printf 'warning: working tree is not clean; the version bump will be mixed with existing changes.\n' >&2
fi

backup_dir="$(mktemp -d)"
success=0

restore() {
  cp "$backup_dir/Cargo.toml" "$cargo_toml"
  cp "$backup_dir/extension.toml" "$extension_toml"
  cp "$backup_dir/lib.rs" "$lib_rs"
  if ((bump_lsp)); then
    cp "$backup_dir/lsp-Cargo.toml" "$lsp_cargo_toml"
  fi
  if [[ -f "$backup_dir/Cargo.lock" ]]; then
    cp "$backup_dir/Cargo.lock" "$cargo_lock"
  fi
}

cleanup() {
  if ((!success)); then
    restore
    printf 'error: restored the version files after a failure.\n' >&2
  fi
  rm -rf "$backup_dir"
}
trap cleanup EXIT

cp "$cargo_toml" "$backup_dir/Cargo.toml"
cp "$extension_toml" "$backup_dir/extension.toml"
cp "$lib_rs" "$backup_dir/lib.rs"
if ((bump_lsp)); then
  cp "$lsp_cargo_toml" "$backup_dir/lsp-Cargo.toml"
fi
if [[ -f "$cargo_lock" ]]; then
  cp "$cargo_lock" "$backup_dir/Cargo.lock"
fi

set_package_version "$cargo_toml"
set_extension_version
set_lsp_tag
if ((bump_lsp)); then
  set_package_version "$lsp_cargo_toml"
fi

if ((!no_lock)); then
  if ! (cd "$root" && cargo check --workspace); then
    exit 1
  fi
fi

success=1
printf 'Set version %s -> %s.\n' "$old_version" "$version"
printf '  updated Cargo.toml\n'
printf '  updated extension.toml\n'
printf '  updated src/lib.rs\n'
if ((!no_lock)); then
  printf '  refreshed Cargo.lock\n'
fi
if ((bump_lsp)); then
  printf '  updated lsp/Cargo.toml\n'
else
  printf '  lsp/Cargo.toml unchanged (pass --bump-lsp to update it)\n'
fi

if is_git_repo; then
  git -C "$root" diff --stat
fi

add_files="Cargo.toml extension.toml src/lib.rs"
if ((!no_lock)); then
  add_files="$add_files Cargo.lock"
fi
if ((bump_lsp)); then
  add_files="$add_files lsp/Cargo.toml"
fi

printf '\nNext steps:\n'
printf '  cargo test -p unity asmdef_release_tag_matches_extension_version\n'
printf '  git add %s\n' "$add_files"
printf '  git commit -m "chore: release %s"\n' "$version"
printf '  git tag v%s\n' "$version"
printf '  git push origin main v%s\n' "$version"
printf '  Review PUBLISHING.md, README.md, and ROADMAP.md for current-release references.\n'
