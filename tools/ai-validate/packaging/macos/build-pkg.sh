#!/bin/bash
# packaging/macos/build-pkg.sh — builds the macOS .pkg installer.
#
# Approach:
#   1. Stage the bundled .jsx into a payload root that pkgbuild will
#      copy to /tmp/ai-validate-staging/ on the user's machine.
#   2. The Scripts/postinstall hook runs after copy, finds every
#      Illustrator install, and drops the .jsx into each Scripts folder.
#   3. pkgbuild produces a component .pkg, productbuild wraps it into
#      a distribution .pkg with the correct UI metadata.
#
# Output: dist/ai-validate-<version>.pkg
#
# Build prerequisites: pkgbuild + productbuild (shipped with macOS Xcode
# Command Line Tools — already present on macos-latest CI runners).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

VERSION="${VERSION:-$(cat packaging/version.txt | tr -d '[:space:]')}"
BUNDLE="dist/ai-validate-${VERSION}.jsx"
if [[ ! -f "$BUNDLE" ]]; then
    echo "[build-pkg] bundle not found at $BUNDLE — run packaging/bundle.sh first" >&2
    exit 1
fi

WORK="$(mktemp -d -t ai-validate-pkg)"
trap 'rm -rf "$WORK"' EXIT

# Payload: bundled .jsx, staged at /tmp/ai-validate-staging/ on the
# target machine. The postinstall script picks it up from there.
mkdir -p "$WORK/payload/tmp/ai-validate-staging"
cp "$BUNDLE" "$WORK/payload/tmp/ai-validate-staging/ai-validate.jsx"

# Scripts folder for postinstall.
mkdir -p "$WORK/scripts"
cp packaging/macos/postinstall "$WORK/scripts/postinstall"
chmod +x "$WORK/scripts/postinstall"

# Component pkg.
COMPONENT="$WORK/ai-validate-component.pkg"
pkgbuild \
    --root "$WORK/payload" \
    --scripts "$WORK/scripts" \
    --identifier com.alexeytuboltsev.ai-validate \
    --version "$VERSION" \
    --install-location / \
    "$COMPONENT"

# Distribution wrapper — gives the installer its title + welcome text.
DIST_XML="$WORK/distribution.xml"
cat > "$DIST_XML" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>ai-validate ${VERSION}</title>
    <organization>com.alexeytuboltsev</organization>
    <options customize="never" require-scripts="true" rootVolumeOnly="true"/>
    <welcome language="en"><![CDATA[
ai-validate ${VERSION} for Adobe Illustrator.

Installs ai-validate.jsx into every detected Illustrator's Scripts
folder. After install, run it via File > Scripts > ai-validate.
    ]]></welcome>
    <choices-outline>
        <line choice="default">
            <line choice="com.alexeytuboltsev.ai-validate"/>
        </line>
    </choices-outline>
    <choice id="default"/>
    <choice id="com.alexeytuboltsev.ai-validate" visible="false">
        <pkg-ref id="com.alexeytuboltsev.ai-validate"/>
    </choice>
    <pkg-ref id="com.alexeytuboltsev.ai-validate" version="${VERSION}" onConclusion="none">ai-validate-component.pkg</pkg-ref>
</installer-gui-script>
EOF

OUT="dist/ai-validate-${VERSION}.pkg"
productbuild \
    --distribution "$DIST_XML" \
    --package-path "$WORK" \
    "$OUT"

echo "[build-pkg] $OUT"
