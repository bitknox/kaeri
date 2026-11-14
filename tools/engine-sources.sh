#!/usr/bin/env bash
# The files an app's engine is built from, one per line, relative to the repository: each
# workspace crate kaeri-ffi depends on, directly or not (normal and build dependencies, the
# optional ones too), without their tests, benches and examples, which its library never
# compiles, and the store's admin page sources, which only the committed build in assets/ is
# compiled from; wit/, the workspace's manifests and lock file, the toolchain pin, .cargo, and the
# files named as arguments. Tracked or not, but not ignored.
#
#   tools/engine-sources.sh <file…>
#   tools/engine-sources.sh --crates      the crates' directories alone, one per line
#
# apple/scripts/engine.sh and android/scripts/engine.sh hash this list for their stamps and
# caches, and CI keys its caches of the engines by the same hash (engine.sh --sources-hash): a
# change to the terminal, the store's server or a test leaves the apps' engines as they were.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# The crates from the manifests alone: no registry is read, so this needs no network. Each path is
# made relative to cargo's own workspace_root, which is written the way manifest_path is: on
# Windows that is C:\... while pwd in Git Bash says /c/..., and on macOS cargo resolves a
# symlinked folder that pwd keeps. Separators become slashes, and jq.exe's CRLF line ends go.
crates="$(cargo metadata --no-deps --format-version 1 | jq -r '
  def slashes: gsub("\\\\"; "/");
  (.workspace_root | slashes | rtrimstr("/") + "/") as $root
  | (.packages | map({key: .name, value: .}) | from_entries) as $packages
  | def deps: [.[] as $name | $packages[$name].dependencies[]
      | select(.path != null and (.kind == null or .kind == "build")) | .name];
    def closure: (. + deps | unique) as $next
      | if ($next | length) == length then . else $next | closure end;
    ["kaeri-ffi"] | closure | .[] | $packages[.].manifest_path
  | slashes | ltrimstr($root) | rtrimstr("/Cargo.toml")' | tr -d '\r')"

# A list that is empty or not relative to the repository would hash the manifests alone, and an
# engine built from other sources would then pass for this one. So it stops here instead.
fail() {
  echo "error: tools/engine-sources.sh: $1" >&2
  echo "error: the engines' stamps and caches cannot be trusted without it; nothing was listed." >&2
  exit 1
}
[[ -n "$crates" ]] || fail "cargo metadata gave no crates for kaeri-ffi"
while IFS= read -r crate; do
  case "$crate" in
    /* | [A-Za-z]:* | *\\* | "")
      fail "the crate path '$crate' is not relative to the repository at $root"
      ;;
  esac
  [[ -f "$crate/Cargo.toml" ]] || fail "the crate path '$crate' has no Cargo.toml under $root"
done <<<"$crates"

if [[ "${1:-}" == --crates ]]; then
  printf '%s\n' "$crates"
  exit 0
fi

# shellcheck disable=SC2086 # one crate directory per word
files="$(
  git ls-files -co --exclude-standard -- $crates wit Cargo.toml Cargo.lock rust-toolchain.toml .cargo "$@" |
    { grep -v -E '^crates/[^/]+/(tests|benches|examples|admin)/' || true; } |
    LC_ALL=C sort -u |
    while IFS= read -r file; do
      if [[ -f "$file" ]]; then printf '%s\n' "$file"; fi
    done
)"
# Every crate lists its own sources, at least its manifest and one more file.
while IFS= read -r crate; do
  [[ "$(grep -c -E "^$crate/" <<<"$files" || true)" -ge 2 ]] ||
    fail "git listed no sources under $crate"
done <<<"$crates"
printf '%s\n' "$files"
