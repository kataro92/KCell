#!/usr/bin/env bash
# Gate `git push` in Cursor agent shells until scripts/check.sh passes.
set -euo pipefail

input="$(cat)"
command=""
if command -v python3 >/dev/null 2>&1; then
  command="$(printf '%s' "$input" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("command",""))')"
fi

if printf '%s' "$command" | grep -Eq '(^|[[:space:];|&])git[[:space:]]+push([[:space:]]|$)'; then
  ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
  if ! "$ROOT/scripts/check.sh" >&2; then
    printf '%s\n' '{"permission":"deny","user_message":"Push blocked: fix fmt/clippy/tests (scripts/check.sh).","agent_message":"Denied git push because scripts/check.sh failed. Fix lint/tests, then push again."}'
    exit 0
  fi
fi

printf '%s\n' '{"permission":"allow"}'
exit 0
