#!/bin/bash
set -euo pipefail

# Claudit release script
# Builds the app with signing, generates latest.json, and creates a GitHub release.
#
# Prerequisites:
#   - Tauri signing key at ~/.tauri/claudit.key
#   - gh CLI authenticated
#   - Rust toolchain sourced

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Read version from Cargo.toml
VERSION=$(grep '^version' "$PROJECT_DIR/src-tauri/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="v$VERSION"

echo "Building Claudit $TAG..."

# Ensure signing key exists
KEY_PATH="$HOME/.tauri/claudit.key"
if [ ! -f "$KEY_PATH" ]; then
  echo "ERROR: Signing key not found at $KEY_PATH"
  echo "Generate one with: npx tauri signer generate -w $KEY_PATH"
  exit 1
fi

# Source Rust if needed
if ! command -v cargo &>/dev/null; then
  source "$HOME/.cargo/env"
fi

# Build with signing
cd "$PROJECT_DIR"
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_PATH")"
if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  echo -n "Signing key password (or set TAURI_SIGNING_PRIVATE_KEY_PASSWORD): "
  read -rs TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  echo
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
fi
npx tauri build

# Find build artifacts
BUNDLE_DIR="$PROJECT_DIR/src-tauri/target/release/bundle"
DMG=$(find "$BUNDLE_DIR/dmg" -name "*.dmg" | head -1)
TARGZ=$(find "$BUNDLE_DIR/macos" -name "*.tar.gz" ! -name "*.sig" | head -1)
SIG="${TARGZ}.sig"

if [ ! -f "$DMG" ] || [ ! -f "$TARGZ" ] || [ ! -f "$SIG" ]; then
  echo "ERROR: Missing build artifacts"
  echo "  DMG: $DMG"
  echo "  tar.gz: $TARGZ"
  echo "  sig: $SIG"
  exit 1
fi

echo "Build artifacts:"
echo "  DMG: $DMG"
echo "  tar.gz: $TARGZ"
echo "  sig: $SIG"

# Generate latest.json
SIGNATURE=$(cat "$SIG")
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat > "$BUNDLE_DIR/latest.json" << ENDJSON
{
  "version": "$VERSION",
  "notes": "Claudit $TAG",
  "pub_date": "$PUB_DATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIGNATURE",
      "url": "https://github.com/psurma/claudit/releases/download/$TAG/Claudit.app.tar.gz"
    }
  }
}
ENDJSON

echo "Generated latest.json"

# Check if tag already exists
if gh release view "$TAG" --repo psurma/claudit &>/dev/null; then
  echo "ERROR: Release $TAG already exists"
  exit 1
fi

# Create GitHub release
echo "Creating GitHub release $TAG..."
gh release create "$TAG" \
  --repo psurma/claudit \
  --title "Claudit $TAG" \
  --notes "See [CHANGELOG](https://github.com/psurma/claudit/blob/main/CHANGELOG.md) for details." \
  "$DMG" \
  "${TARGZ}#Claudit.app.tar.gz" \
  "${SIG}#Claudit.app.tar.gz.sig" \
  "${BUNDLE_DIR}/latest.json"

echo ""
echo "Release $TAG created successfully!"
echo "https://github.com/psurma/claudit/releases/tag/$TAG"
