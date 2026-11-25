#!/usr/bin/env bash
# Makes room for one cache that a CI job is about to save, so the repository's caches stay inside
# their 10 GB. Above it GitHub refuses every save ("Unable to reserve cache"), and from then on
# pull requests restore whatever main saved last, however old, and compile everything: from 14 to
# 25 November 2026 no save landed, and every job compiled all ~800 crates.
#
#   tools/ci-cache-room.sh <key or family> [MB]
#   tools/ci-cache-room.sh --rust <shared-key> [MB]
#
# First it deletes this ref's entries of the same family, which the new entry replaces: the family
# is the key without its trailing hashes, as tools/ci-caches.sh has it, so
# engine-android-testing-…-ndk30.0.16248370-<sha256> and v0-rust-linux-Linux-x64-c893db98-1b13bd87 are
# replaced by the next of their kind and never sit beside it. A family's new entry is only ever
# saved after its old one went, so a push that renews every cache at once needs no room beyond
# what they held. --rust names Swatinem/rust-cache's family by its shared-key, with this runner's
# OS and architecture as rust-cache writes them.
#
# Then it says whether the new entry (MB, an estimate) fits under the limit. main always saves:
# when it would not fit, the other refs' entries go, least recently read first. A pull request
# saves only when it fits under a lower limit, so its entries never take main's room.
# Writes save=true or save=false to $GITHUB_OUTPUT when there is one.
#
# Needs GH_TOKEN with actions: write, GITHUB_REPOSITORY and GITHUB_REF. Without the write
# permission (a fork's pull request) nothing is deleted and a pull request does not save.
set -euo pipefail

usage() {
  echo "usage: tools/ci-cache-room.sh <key or family> [MB] | --rust <shared-key> [MB]" >&2
  exit 2
}

[[ $# -ge 1 ]] || usage
if [[ "$1" == --rust ]]; then
  [[ $# -ge 2 ]] || usage
  case "$(uname -s)" in
    Darwin) os=Darwin ;;
    Linux) os=Linux ;;
    MINGW* | MSYS* | CYGWIN*) os=Windows_NT ;;
    *) os="$(uname -s)" ;;
  esac
  case "$(uname -m)" in
    arm64 | aarch64) arch=arm64 ;;
    *) arch=x64 ;;
  esac
  name="v0-rust-$2-$os-$arch"
  shift 2
else
  name="$1"
  shift
fi
need_mb="${1:-500}"
repo="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY names the repository, as owner/name}"
ref="${GITHUB_REF:?GITHUB_REF names the ref the cache is saved for}"
# GitHub's limit is 10,000 MB. main fills up to the first, pull requests only up to the second.
main_limit="${CACHE_ROOM_MAIN_MB:-9600}"
pr_limit="${CACHE_ROOM_PR_MB:-8800}"

out() {
  echo "save=$1" >>"${GITHUB_OUTPUT:-/dev/null}"
  echo "ci-cache-room.sh: save=$1"
  exit 0
}

family() {
  local key="$1"
  while [[ "$key" =~ ^(.*)-[0-9a-f]{8,}$ ]]; do key="${BASH_REMATCH[1]}"; done
  echo "$key"
}
want="$(family "$name")"

# id, ref, last read, size and key, one cache a line.
if ! caches="$(gh api --paginate "repos/$repo/actions/caches?per_page=100" \
  --jq '.actions_caches[] | [.id, .ref, .last_accessed_at, .size_in_bytes, .key] | @tsv')"; then
  echo "::warning::the caches could not be listed, so no room was made for $want"
  [[ "$ref" == refs/heads/main ]] && out true
  out false
fi

delete() {
  gh api -X DELETE "repos/$repo/actions/caches/$1" >/dev/null 2>&1
}

used=0
can_delete=1
while IFS=$'\t' read -r id cref _ size key; do
  [[ -n "$id" ]] || continue
  if [[ "$cref" == "$ref" && "$(family "$key")" == "$want" ]]; then
    if ((can_delete)) && delete "$id"; then
      echo "deleted $((size / 1000000)) MB: $key, which this job's save replaces"
      continue
    fi
    can_delete=0
  fi
  used=$((used + size))
done <<<"$caches"
used_mb=$((used / 1000000))
echo "The caches hold $used_mb MB; $want needs about $need_mb more."

if [[ "$ref" == refs/heads/main ]]; then
  if ((used_mb + need_mb > main_limit)); then
    # Other refs' entries, least recently read first, until main's fits.
    while IFS=$'\t' read -r id cref _ size key; do
      ((used_mb + need_mb > main_limit)) || break
      if delete "$id"; then
        used_mb=$((used_mb - size / 1000000))
        echo "deleted $((size / 1000000)) MB of $cref: $key, to make room for main's"
      fi
    done < <(awk -F'\t' '$2 != "refs/heads/main"' <<<"$caches" | sort -t$'\t' -k3,3)
  fi
  ((used_mb + need_mb <= main_limit)) ||
    echo "::warning::main's caches alone hold $used_mb MB: $want may not save. Delete the families no workflow reads any more."
  out true
fi

((can_delete)) || out false
if ((used_mb + need_mb > pr_limit)); then
  echo "::notice::No room for this pull request's $want ($used_mb MB held, $pr_limit for pull requests): not saved."
  out false
fi
out true
