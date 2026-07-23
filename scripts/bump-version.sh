#!/usr/bin/env bash
# Bump the app version in the three manifests that must stay in sync. The in-app
# updater compares the running CARGO_PKG_VERSION against the latest GitHub release
# tag, so these must match the tag you push (tag `vX.Y.Z` → version `X.Y.Z`).
#
# Usage: scripts/bump-version.sh 0.2.0
set -euo pipefail

ver="${1:?usage: scripts/bump-version.sh X.Y.Z}"
if ! [[ "$ver" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: version must be X.Y.Z (got '$ver')" >&2
  exit 1
fi

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# package.json + tauri.conf.json each have exactly one `"version": "X.Y.Z"` key.
sed -i -E "s/\"version\": \"[0-9]+\.[0-9]+\.[0-9]+\"/\"version\": \"$ver\"/" \
  "$here/package.json" "$here/src-tauri/tauri.conf.json"

# Cargo.toml: replace only the first `version = "X.Y.Z"` (the [package] version);
# dependency versions like "2" don't match the X.Y.Z pattern anyway.
sed -i -E "0,/^version = \"[0-9]+\.[0-9]+\.[0-9]+\"/s//version = \"$ver\"/" \
  "$here/src-tauri/Cargo.toml"

echo "Bumped to $ver in package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml"
echo "Next:"
echo "  git commit -am \"Release v$ver\""
echo "  git tag v$ver && git push && git push origin v$ver"
