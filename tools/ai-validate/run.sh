#!/bin/bash
# Run the validator against an AI file. Illustrator must already be
# running. Edits to validate.jsx (or any lib) are picked up immediately
# — `do javascript file` re-reads from disk on every call.
#
# Usage:
#   ./run.sh path/to/file.ai           # report-only (read-only)
#   ./run.sh path/to/file.ai --fix     # apply deterministic fixes

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET="${1:-}"
MODE_FLAG="${2:-}"

if [[ -z "$TARGET" ]]; then
  echo "Usage: $0 <path-to-ai-file> [--fix]" >&2
  exit 1
fi

# Resolve to an absolute path; do javascript file requires it.
if [[ ! "$TARGET" = /* ]]; then
  TARGET="$(cd "$(dirname "$TARGET")" && pwd)/$(basename "$TARGET")"
fi

if [[ ! -f "$TARGET" ]]; then
  echo "[!] $TARGET does not exist" >&2
  exit 1
fi

MODE="report"
if [[ "$MODE_FLAG" == "--fix" ]]; then
  MODE="fix"
fi

echo "[*] target: $TARGET"
echo "[*] mode:   $MODE"

# Pass the mode through a tmp file — ExtendScript's `do javascript file`
# argument-passing has version-specific quirks; a file is bulletproof.
echo "$MODE" > /tmp/ai-validate-mode.txt

# Make sure Illustrator is running and has the file open.
osascript <<EOF >/dev/null
tell application "Adobe Illustrator"
  if not running then
    activate
    delay 1
  end if
  open POSIX file "$TARGET"
  activate
end tell
EOF

# Run validate.jsx. Illustrator re-reads it (and its #includes) every
# call, so iterating doesn't need a restart.
osascript <<EOF >/dev/null
tell application "Adobe Illustrator"
  do javascript file "$SCRIPT_DIR/validate.jsx"
end tell
EOF

REPORT=/tmp/ai-validate-report.json
echo "[*] report: $REPORT"

if [[ ! -f "$REPORT" ]]; then
  echo "[!] no report written — check Illustrator's JavaScript Console" >&2
  exit 1
fi

# Pretty-print if jq is available; otherwise raw cat.
if command -v jq >/dev/null 2>&1; then
  jq '.' "$REPORT"
else
  cat "$REPORT"
fi
