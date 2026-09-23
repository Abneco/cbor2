#!/bin/sh
# Check that a release tag matches every workspace crate version before
# anything is built or published. The Homebrew formula test also compares
# `cbor --version` with the tag.
#
# Usage: scripts/check-release-version.sh v1.2.3 (defaults to GITHUB_REF_NAME)

set -eu

TAG="${1:-${GITHUB_REF_NAME:-}}"
TAG="${TAG#refs/tags/}"
VERSION="${TAG#v}"

error() { printf 'Error: %s\n' "$1" >&2; exit 1; }

[ -n "$VERSION" ] || error "Usage: $0 <tag>"

for crate in cbor2 cbor2-derive cbor2-cli; do
    # `cargo pkgid` ends in `#<version>` or `#<name>@<version>`.
    actual=$(cargo pkgid --offline -p "$crate" 2>/dev/null || cargo pkgid -p "$crate")
    actual="${actual##*[#@]}"
    [ "$actual" = "$VERSION" ] || error "${crate} is ${actual}, but the tag is ${TAG}"
done

printf 'Release %s matches the workspace crates.\n' "$TAG"
