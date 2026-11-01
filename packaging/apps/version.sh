#!/usr/bin/env bash
# The apps' version numbers from a release tag, as KEY=value lines for $GITHUB_ENV:
#
#   packaging/apps/version.sh v0.1.1
#   VERSION=0.1.1                 the file names, and Android's versionName
#   APPLE_VERSION=0.1.1           CFBundleShortVersionString, which takes only numbers and dots
#   BUILD_NUMBER=1000199          Android's versionCode and Apple's CFBundleVersion
#
#   packaging/apps/version.sh --test    checks the numbering against the cases below
#
# The build number grows with the version, which Android needs to install one over another and
# Sparkle needs to offer a newer Mac app. It is the major, then the minor in two digits, the patch
# in four, and 99 for a release or the pre-release's place below it, so
# 0.2.0-alpha.1 < 0.2.0-beta.1 < 0.2.0-rc.1 < 0.2.0:
#
#   v0.1.0         1000099
#   v0.1.1         1000199
#   v0.1.137       1013799
#   v0.2.0-rc.2    2000062        (rc.N is 60 + N, beta.N 30 + N, alpha.N N, for N up to 29)
#   v0.2.0         2000099
#   v1.0.0         100000099
#
# The patch has four digits because every release from main raises it by one, features included
# while the major is 0 (packaging/next-version.sh), so it runs to 9999 within a minor. The minor
# runs to 99. Android's versionCode stops at 2100000000, which puts the major at 20 or less. The
# Windows installer's version is APPLE_VERSION with a fourth field of 0, and each of its fields
# stops at 65535, which the limits above keep.
#
# With no tag, as on a pull request or a dry run of a branch, the version is 0.0.0. Without the
# environment this sets, a local build keeps the version in android/app/build.gradle.kts and
# apple/project.yml.
set -euo pipefail

# numbers <tag>: the KEY=value lines, or a failure.
numbers() {
  local tag="$1" version
  version="${tag#v}"
  [[ -n "$version" ]] || version=0.0.0

  if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-(alpha|beta|rc)\.([0-9]+))?$ ]]; then
    echo "version.sh: $tag is not vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-<alpha|beta|rc>.N" >&2
    return 1
  fi
  local major minor patch stage n
  major=$((10#${BASH_REMATCH[1]}))
  minor=$((10#${BASH_REMATCH[2]}))
  patch=$((10#${BASH_REMATCH[3]}))
  stage=99
  if [[ -n "${BASH_REMATCH[4]}" ]]; then
    n=$((10#${BASH_REMATCH[6]}))
    (( n >= 1 && n <= 29 )) || { echo "version.sh: $tag: a pre-release number runs from 1 to 29" >&2; return 1; }
    case "${BASH_REMATCH[5]}" in
      alpha) stage=$n ;;
      beta) stage=$((30 + n)) ;;
      rc) stage=$((60 + n)) ;;
    esac
  fi
  (( minor <= 99 )) || { echo "version.sh: $tag: the minor version goes up to 99" >&2; return 1; }
  (( patch <= 9999 )) || { echo "version.sh: $tag: the patch version goes up to 9999" >&2; return 1; }
  # Android's versionCode stops at 2100000000.
  (( major <= 20 )) || { echo "version.sh: $tag: the major version goes up to 20" >&2; return 1; }

  echo "VERSION=$version"
  echo "APPLE_VERSION=$major.$minor.$patch"
  echo "BUILD_NUMBER=$((major * 100000000 + minor * 1000000 + patch * 100 + stage))"
}

run_tests() {
  local failed=0 count=0 tag want got previous=-1 build
  # Each tag's build number, in release order: every one above the one before.
  while read -r tag want; do
    count=$((count + 1))
    # "-" is no tag at all.
    [[ "$tag" != - ]] || tag=""
    got="$(numbers "$tag" 2>/dev/null | sed -n 's/^BUILD_NUMBER=//p')" || got=""
    if [[ "$got" != "$want" ]]; then
      echo "not ok: $tag numbered ${got:-(failed)}, not $want" >&2
      failed=$((failed + 1))
    fi
    build="${got:-0}"
    if (( build <= previous )); then
      echo "not ok: $tag numbered $build, not above the version before it ($previous)" >&2
      failed=$((failed + 1))
    fi
    previous=$build
  done <<'CASES'
- 99
v0.1.0 1000099
v0.1.1 1000199
v0.1.2 1000299
v0.1.137 1013799
v0.1.9999 1999999
v0.2.0-alpha.1 2000001
v0.2.0-beta.1 2000031
v0.2.0-rc.2 2000062
v0.2.0 2000099
v0.2.1 2000199
v0.99.9999 99999999
v1.0.0-rc.1 100000061
v1.0.0 100000099
v1.0.1 100000199
v1.1.0 101000099
v20.99.9999 2099999999
CASES
  # Numbers past what Android, Windows or the scheme can carry are refused.
  for tag in v0.100.0 v0.1.10000 v21.0.0 v0.2.0-rc.30 v0.2.0-rc.0 v0.1 v0.1.0-pre.1; do
    count=$((count + 1))
    if numbers "$tag" >/dev/null 2>&1; then
      echo "not ok: $tag should be refused" >&2
      failed=$((failed + 1))
    fi
  done
  count=$((count + 1))
  if [[ "$(numbers v0.1.1 | paste -sd ' ' -)" != "VERSION=0.1.1 APPLE_VERSION=0.1.1 BUILD_NUMBER=1000199" ]]; then
    echo "not ok: v0.1.1's lines" >&2
    failed=$((failed + 1))
  fi
  if (( failed )); then
    echo "version.sh: $failed of $count tests failed" >&2
    exit 1
  fi
  echo "version.sh: all $count tests passed"
}

if [[ "${1:-}" == "--test" ]]; then
  run_tests
else
  numbers "${1:-}"
fi
