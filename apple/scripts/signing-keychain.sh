#!/usr/bin/env bash
# The Mac app's signing identity, in a keychain made for one build and deleted after it.
#
# The identity is a self-signed code-signing certificate, CN "Kaeri", with no Apple account behind
# it. It isn't Developer ID, so Gatekeeper treats the app as it treats an ad hoc signed one. What
# it changes is the designated requirement: `identifier "app.kaeri.app" and certificate leaf =
# H"<its SHA-1>"` in place of the build's cdhash. That is the same for every release, so the login
# keychain's "Always Allow" lasts across updates. Without the secrets the builds are signed ad hoc,
# as before. The private half lives in ~/.kaeri-signing/mac on the maintainer's Mac; see
# apple/CLAUDE.md.
#
#   apple/scripts/signing-keychain.sh ci-setup
#       From MAC_CODESIGN_P12 (the .p12, base64) and MAC_CODESIGN_P12_PASSWORD: a new keychain
#       under $RUNNER_TEMP, unlocked, never locking, first in the search list, holding the
#       identity. Writes KAERI_SIGN_IDENTITY (its SHA-1) and KAERI_SIGN_KEYCHAIN to $GITHUB_ENV,
#       which adhoc-sign.sh and the terminal's build sign with. Nothing, and a notice, when either
#       secret is unset.
#   apple/scripts/signing-keychain.sh ci-delete
#       Deletes that keychain, which takes it out of the search list.
#   apple/scripts/signing-keychain.sh import <keychain> <p12> <password file>
#       The identity into a keychain made and unlocked beforehand, its password in
#       KAERI_KEYCHAIN_PASSWORD; prints the identity's SHA-1. What ci-setup and sign do after
#       making the keychain.
#   apple/scripts/signing-keychain.sh sign <dir> <Kaeri.app>
#       A local build of the Mac app, such as a merge train's review build, signed as a release
#       is: the identity from <dir>, which holds kaeri.p12 and p12-password (the maintainer's
#       ~/.kaeri-signing/mac), into a keychain made with a random password in a temporary
#       directory and never put in the search list, then adhoc-sign.sh with it. The keychain is
#       deleted when this ends, however it ends. The login keychain and the search list are left
#       as they were, and nothing asks for a password.
#
# Nothing here prints the key, the .p12 or a password.
set -euo pipefail

usage() {
  sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//' >&2
  exit 2
}

# The identity in `p12` into `keychain`, and its SHA-1 printed. -A lets any program use the key
# without asking: the keychain lives for one job and holds nothing else, and nothing may stop to
# ask. The partition list does the same for codesign on current macOS.
import_identity() {
  local keychain="$1" p12="$2" password_file="$3" sha=""
  : "${KAERI_KEYCHAIN_PASSWORD:?the password of the keychain}"
  security import "$p12" -k "$keychain" -f pkcs12 -P "$(cat "$password_file")" -A \
    -T /usr/bin/codesign >/dev/null
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
    -k "$KAERI_KEYCHAIN_PASSWORD" "$keychain" >/dev/null
  # Every certificate in it: the keychain holds this identity's alone.
  sha="$(security find-certificate -a -Z "$keychain" | awk '/^SHA-1 hash:/ { print $3 }')"
  if [[ "$(wc -w <<<"$sha")" -ne 1 ]]; then
    echo "error: $keychain holds $(wc -w <<<"$sha") certificates, not one" >&2
    exit 1
  fi
  echo "$sha"
}

case "${1:-}" in
  ci-setup)
    if [[ -z "${MAC_CODESIGN_P12:-}" || -z "${MAC_CODESIGN_P12_PASSWORD:-}" ]]; then
      echo "::notice::No MAC_CODESIGN_P12 secret, so the Mac builds are signed ad hoc."
      exit 0
    fi
    dir="${RUNNER_TEMP:?}/kaeri-signing"
    mkdir -p "$dir" && chmod 700 "$dir"
    keychain="$dir/signing.keychain-db"
    KAERI_KEYCHAIN_PASSWORD="$(openssl rand -hex 24)"
    export KAERI_KEYCHAIN_PASSWORD
    security create-keychain -p "$KAERI_KEYCHAIN_PASSWORD" "$keychain"
    # The path, always: without one these change the runner's default keychain.
    security set-keychain-settings "$keychain"
    security unlock-keychain -p "$KAERI_KEYCHAIN_PASSWORD" "$keychain"
    # First in the search list, before the runner's own, so codesign finds the identity.
    existing=()
    while IFS= read -r line; do
      line="${line#"${line%%[![:space:]]*}"}"
      line="${line#\"}"
      line="${line%\"}"
      [[ -n "$line" && "$line" != "$keychain" ]] && existing+=("$line")
    done < <(security list-keychains -d user)
    security list-keychains -d user -s "$keychain" "${existing[@]}"

    (
      umask 077
      printf '%s' "$MAC_CODESIGN_P12" | base64 --decode >"$dir/identity.p12"
      printf '%s' "$MAC_CODESIGN_P12_PASSWORD" >"$dir/password"
    )
    identity="$(import_identity "$keychain" "$dir/identity.p12" "$dir/password")"
    rm -f "$dir/identity.p12" "$dir/password"
    {
      echo "KAERI_SIGN_IDENTITY=$identity"
      echo "KAERI_SIGN_KEYCHAIN=$keychain"
    } >>"${GITHUB_ENV:?}"
    echo "signing-keychain.sh: the Mac builds are signed as Kaeri, $identity"
    ;;
  ci-delete)
    if [[ -n "${KAERI_SIGN_KEYCHAIN:-}" && -f "$KAERI_SIGN_KEYCHAIN" ]]; then
      security delete-keychain "$KAERI_SIGN_KEYCHAIN"
    fi
    rm -rf "${RUNNER_TEMP:?}/kaeri-signing"
    ;;
  import)
    [[ $# -eq 4 && -f "$2" && -f "$3" && -f "$4" ]] || usage
    import_identity "$2" "$3" "$4"
    ;;
  sign)
    [[ $# -eq 3 && -d "$2" && -d "$3" ]] || usage
    dir="$2"
    app="$3"
    for file in kaeri.p12 p12-password; do
      if [[ ! -f "$dir/$file" ]]; then
        echo "error: $dir has no $file" >&2
        exit 1
      fi
    done
    work="$(mktemp -d)"
    chmod 700 "$work"
    keychain="$work/signing.keychain-db"
    trap 'security delete-keychain "$keychain" 2>/dev/null || true; rm -rf "$work"' EXIT
    KAERI_KEYCHAIN_PASSWORD="$(openssl rand -hex 24)"
    export KAERI_KEYCHAIN_PASSWORD
    # With its path, create-keychain leaves the search list and the default keychain alone.
    security create-keychain -p "$KAERI_KEYCHAIN_PASSWORD" "$keychain"
    security set-keychain-settings "$keychain"
    security unlock-keychain -p "$KAERI_KEYCHAIN_PASSWORD" "$keychain"
    identity="$(import_identity "$keychain" "$dir/kaeri.p12" "$dir/p12-password")"
    unset KAERI_KEYCHAIN_PASSWORD
    # adhoc-sign.sh finds the identity through --keychain, so the search list never needs it.
    KAERI_SIGN_IDENTITY="$identity" KAERI_SIGN_KEYCHAIN="$keychain" \
      "$(dirname "$0")/adhoc-sign.sh" "$app"
    ;;
  *) usage ;;
esac
