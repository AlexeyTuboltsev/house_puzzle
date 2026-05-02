#!/bin/bash
# packaging/bundle.sh — concatenates lib/*.jsx + validate.jsx into a
# single self-contained .jsx that an end-user can drop into Illustrator's
# Scripts folder. The bundled file replaces the dev-time #include chain
# (which would point at paths the artist's machine doesn't have).
#
# Output: dist/ai-validate-<version>.jsx
#
# Usage:
#   ./packaging/bundle.sh                # uses packaging/version.txt
#   VERSION=1.2.3 ./packaging/bundle.sh  # override
#
# CI calls this from the release workflow before invoking the platform-
# specific installer builder. Local Makefile target `make bundle` also
# wraps it.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${VERSION:-$(cat packaging/version.txt | tr -d '[:space:]')}"
MANIFEST_URL="${MANIFEST_URL:-https://raw.githubusercontent.com/AlexeyTuboltsev/house_puzzle/main/tools/ai-validate/script-version.json}"

OUT_DIR="dist"
OUT_FILE="$OUT_DIR/ai-validate-${VERSION}.jsx"

mkdir -p "$OUT_DIR"

# Bundle order matches validate.jsx's #include chain. update_check.jsx
# goes BEFORE validate.jsx because validate.jsx calls into it from the
# top-level entry block.
LIB_ORDER=(
    "lib/json2.jsx"
    "lib/log.jsx"
    "lib/walk_paths.jsx"
    "lib/checks.jsx"
    "lib/fixes.jsx"
    "lib/render_md.jsx"
    "lib/update_check.jsx"
)

{
    echo "// ai-validate $VERSION — bundled"
    echo "// generated $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
    echo "// DO NOT EDIT — regenerate via packaging/bundle.sh"
    echo ""
    echo "var AI_VALIDATE_VERSION = \"$VERSION\";"
    echo "var AI_VALIDATE_MANIFEST_URL = \"$MANIFEST_URL\";"
    echo ""
    for f in "${LIB_ORDER[@]}"; do
        if [[ ! -f "$f" ]]; then
            # update_check.jsx is added later; tolerate its absence
            # while bootstrapping so this script stays runnable.
            echo "// (skipped $f — file absent at bundle time)"
            echo ""
            continue
        fi
        echo "// =============================================================="
        echo "// $f"
        echo "// =============================================================="
        # Strip #include lines from sources (already inlined here).
        grep -v '^#include' "$f"
        echo ""
    done
    # validate.jsx last, with its #include directives also stripped.
    echo "// =============================================================="
    echo "// validate.jsx (entry)"
    echo "// =============================================================="
    grep -v '^#include' validate.jsx
} > "$OUT_FILE"

echo "[bundle] $OUT_FILE ($(wc -l < "$OUT_FILE") lines, $(wc -c < "$OUT_FILE") bytes)"
