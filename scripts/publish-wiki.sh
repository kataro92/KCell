#!/usr/bin/env bash
# Publish wiki/ to GitHub Wiki. Requires the wiki repo to exist
# (create the first page once in the GitHub UI if clone fails).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/wiki"
TMP="$(mktemp -d)"
TOKEN="$(gh auth token)"
REMOTE="https://x-access-token:${TOKEN}@github.com/kataro92/KCell.wiki.git"

cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

if ! git ls-remote "$REMOTE" &>/dev/null; then
  echo "Wiki remote not found yet."
  echo "Open https://github.com/kataro92/KCell/wiki and click Create the first page, Save, then re-run:"
  echo "  scripts/publish-wiki.sh"
  exit 1
fi

git clone "$REMOTE" "$TMP/wiki"
rsync -a --delete --exclude .git "$SRC/" "$TMP/wiki/"
cd "$TMP/wiki"
git add -A
if git diff --cached --quiet; then
  echo "Wiki already up to date."
  exit 0
fi
git -c user.name=kataro92 -c user.email=kataro92@users.noreply.github.com \
  commit -m "Sync wiki from repository wiki/ folder"
git push origin HEAD
echo "Wiki published: https://github.com/kataro92/KCell/wiki"
