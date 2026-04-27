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

# Pass mode + target through tmp files — ExtendScript's `do javascript
# file` argument-passing has version-specific quirks; files are
# bulletproof. validate.jsx switches to the right document itself
# (matching by fsName), or opens the file if it isn't open. We do NOT
# pre-`open POSIX file` here because the artist may have ~20 docs open
# already and we don't want AppleScript fighting JSX over which one is
# active.
echo "$MODE"   > /tmp/ai-validate-mode.txt
echo "$TARGET" > /tmp/ai-validate-target.txt

# Run validate.jsx. Illustrator re-reads it (and its #includes) every
# call, so iterating doesn't need a restart.
osascript <<EOF >/dev/null
tell application "Adobe Illustrator"
  activate
  do javascript file "$SCRIPT_DIR/validate.jsx"
end tell
EOF

REPORT_JSON=/tmp/ai-validate-report.json
NEXT_TO_AI="${TARGET%.*}.report.md"

if [[ ! -f "$REPORT_JSON" ]]; then
  echo "[!] no report written — check Illustrator's JavaScript Console" >&2
  exit 1
fi

# Echo the artist-facing report path. Print its head so the caller
# sees the headline numbers without opening the file.
echo "[*] report: $NEXT_TO_AI"
echo
if [[ -f "$NEXT_TO_AI" ]]; then
  head -n 8 "$NEXT_TO_AI"
fi
