#!/usr/bin/env bash
# Install repo git hooks into .git/hooks (no git config changes).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_DIR="$ROOT/.git/hooks"
mkdir -p "$HOOKS_DIR"
cp "$ROOT/.githooks/pre-push" "$HOOKS_DIR/pre-push"
chmod +x "$HOOKS_DIR/pre-push" "$ROOT/.githooks/pre-push" "$ROOT/scripts/check.sh"
echo "installed pre-push -> $HOOKS_DIR/pre-push"
