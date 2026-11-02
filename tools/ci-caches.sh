#!/usr/bin/env bash
# Keeps the repository's Actions caches inside their 10 GB. Over it, GitHub evicts the caches read
# longest ago, which were often the ones every pull request restores, so a pull request found
# nothing and compiled everything; and where a budget caps the storage, every save failed instead.
# Most of the room went to caches nothing reads any more: each new Cargo.lock, toolchain or set of
# engine sources saves an entry beside the last one, and the old one stays until it has gone
# unread for seven days.
#
#   tools/ci-caches.sh [--dry-run]
#
# Deletes, of main's caches, every entry a newer entry of the same family has replaced, where the
# family is the key without its trailing hashes, and the caches of pull requests that are closed.
# Only the families CI's own steps save (Rust's, the engines', libmpv's, the packages', Robolectric's)
# are touched: Gradle and BuildKit keep their entries by their own rules. Then it lists what is
# left by family, largest first, into the job summary when there is one. With --dry-run it deletes
# nothing and says what it would.
#
# Needs GH_TOKEN with actions: write, and GITHUB_REPOSITORY (owner/name).
set -euo pipefail

dry=0
case "${1:-}" in
  --dry-run) dry=1 ;;
  "") ;;
  *)
    echo "usage: tools/ci-caches.sh [--dry-run]" >&2
    exit 2
    ;;
esac
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY names the repository, as owner/name}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# id, ref, created, size and key, one cache a line, newest first.
list() {
  gh api --paginate "repos/$repo/actions/caches?per_page=100&sort=created_at&direction=desc" \
    --jq '.actions_caches[] | [.id, .ref, .created_at, .size_in_bytes, .key] | @tsv'
}

# A key without its trailing hashes: v0-rust-linux-Linux-x64-c893db98-f71e2346 is of the family
# v0-rust-linux-Linux-x64, ci-apple-engines-sim-xcode27.1-<sha256> of ci-apple-engines-sim-xcode27.1.
family() {
  local key="$1"
  while [[ "$key" =~ ^(.*)-[0-9a-f]{8,}$ ]]; do key="${BASH_REMATCH[1]}"; done
  echo "$key"
}

list >"$work/caches"
: >"$work/doomed"

# main's superseded entries. The list is newest first, so the first of a family is kept.
pruned='^(v0-rust-|ci-|apps-|mpv-debs-|linux-app-debs-|robolectric-)'
while IFS=$'\t' read -r id ref _ size key; do
  [[ "$ref" == refs/heads/main && "$key" =~ $pruned ]] || continue
  name="$(family "$key")"
  if grep -qxF "$name" "$work/kept" 2>/dev/null; then
    printf '%s\t%s\t%s\n' "$id" "$size" "$key (superseded on main)" >>"$work/doomed"
  else
    echo "$name" >>"$work/kept"
  fi
done <"$work/caches"

# Closed pull requests' caches: nothing reads a merge ref once its pull request is closed.
while IFS= read -r number; do
  state="$(gh api "repos/$repo/pulls/$number" --jq .state 2>/dev/null || echo unknown)"
  [[ "$state" == closed ]] || continue
  awk -F'\t' -v ref="refs/pull/$number/merge" '$2 == ref { print $1 "\t" $4 "\t" $5 " (closed pull request '"$number"')" }' \
    "$work/caches" >>"$work/doomed"
done < <(cut -f2 "$work/caches" | sed -nE 's|^refs/pull/([0-9]+)/merge$|\1|p' | sort -u)

freed=0
while IFS=$'\t' read -r id size what; do
  freed=$((freed + size))
  if ((dry)); then
    echo "would delete $((size / 1000000)) MB: $what"
  elif gh api -X DELETE "repos/$repo/actions/caches/$id" >/dev/null; then
    echo "deleted $((size / 1000000)) MB: $what"
  else
    # Another run may have deleted it first.
    echo "::warning::could not delete $what"
  fi
done <"$work/doomed"
echo "ci-caches.sh: $( ((dry)) && echo would free || echo freed) $((freed / 1000000)) MB"

# What is left, by ref and family. A deleted entry is still listed for a moment, so the list is
# what was there less what was deleted.
cut -f1 "$work/doomed" | sort >"$work/doomed-ids"
{
  echo "### Actions caches"
  echo
  echo "| MB | Ref | Family |"
  echo "| ---: | --- | --- |"
  while IFS=$'\t' read -r id ref _ size key; do
    grep -qxF "$id" "$work/doomed-ids" && continue
    printf '%s\t%s\t%s\n' "$size" "$ref" "$(family "$key")"
  done <"$work/caches" |
    awk -F'\t' '{ size[$2 "\t" $3] += $1; total += $1 }
      END {
        for (k in size) { split(k, p, "\t"); printf "| %d | %s | %s |\n", size[k] / 1000000, p[1], p[2] }
        printf "| %d | | all |\n", total / 1000000
      }' |
    sort -t'|' -k2,2nr
} >"$work/summary"
cat "$work/summary"
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  cat "$work/summary" >>"$GITHUB_STEP_SUMMARY"
fi
