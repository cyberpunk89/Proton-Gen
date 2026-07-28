#!/usr/bin/env bash
# Publish docs/wiki/ to the GitHub wiki.
#
# The repo is the source of truth: this mirrors docs/wiki/*.md into the wiki git
# repo, deleting any wiki page that no longer exists here. Browser edits to the
# wiki are therefore overwritten — _Footer.md says so on every page.
#
# Usage:
#   scripts/sync-wiki.sh          push
#   scripts/sync-wiki.sh --dry    show the diff, push nothing
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
src="$here/docs/wiki"
wiki_url="${WIKI_URL:-https://github.com/cyberpunk89/Proton-Gen.wiki.git}"
dry=0
[[ "${1:-}" == "--dry" ]] && dry=1

[[ -d "$src" ]] || { echo "error: $src not found" >&2; exit 1; }

# GitHub only creates <repo>.wiki.git once the first page has been saved through
# the web UI. Until then the clone 404s, which is confusing on its own.
if ! git ls-remote "$wiki_url" >/dev/null 2>&1; then
  cat >&2 <<'EOF'
error: the wiki repository does not exist yet.

GitHub creates it only after the first page is saved in the web UI. Open

    https://github.com/cyberpunk89/Proton-Gen/wiki/_new

save any page (title "Home", any body), then re-run this script — it will
overwrite that placeholder with docs/wiki/Home.md.
EOF
  exit 1
fi

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
git clone --quiet --depth 1 "$wiki_url" "$tmp/wiki"

# Mirror: drop the wiki's current pages, copy ours in. Anything removed from
# docs/wiki/ therefore disappears from the wiki too.
find "$tmp/wiki" -maxdepth 1 -name '*.md' -delete
cp "$src"/*.md "$tmp/wiki/"
[[ -d "$src/img" ]] && cp -r "$src/img" "$tmp/wiki/"

cd "$tmp/wiki"
if git diff --quiet && [[ -z "$(git status --porcelain)" ]]; then
  echo "Wiki is already up to date."
  exit 0
fi

git add -A
echo "== Changes =="
git --no-pager diff --cached --stat

if [[ $dry == 1 ]]; then
  echo
  echo "Dry run - nothing pushed."
  exit 0
fi

sha="$(git -C "$here" rev-parse --short HEAD 2>/dev/null || echo unknown)"
git -c user.name="${GIT_AUTHOR_NAME:-wiki-sync}" \
    -c user.email="${GIT_AUTHOR_EMAIL:-wiki-sync@users.noreply.github.com}" \
    commit --quiet -m "Sync wiki from docs/wiki@$sha"
git push --quiet origin HEAD
echo
echo "Pushed. https://github.com/cyberpunk89/Proton-Gen/wiki"
