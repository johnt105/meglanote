#!/usr/bin/env bash
# Builds, signs, and prepares a MeglaNote release for GitHub Releases.
# Run this from the meglanote/ folder (where package.json lives).
#
# One-time setup before the first run:
#   1. Generate a signing keypair (only once, ever):
#        npx tauri signer generate -w ~/.tauri/meglanote.key
#      This prints a public key — paste it into src-tauri/tauri.conf.json
#      under plugins.updater.pubkey (replacing the placeholder).
#   2. Put your GitHub repo details into src-tauri/tauri.conf.json's
#      plugins.updater.endpoints (replace REPLACE_WITH_GITHUB_USERNAME).
#   3. Create the GitHub repo (can be private) and push this project to it.
#   4. Set these two env vars in your shell profile so every future
#      release is signed automatically:
#        export TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/meglanote.key)"
#        export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # or your password if you set one
#
# Each release, just run:  ./release.sh 1.1.0

set -euo pipefail

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
  echo "Usage: ./release.sh <version>   e.g. ./release.sh 1.1.0"
  exit 1
fi

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  echo "TAURI_SIGNING_PRIVATE_KEY is not set - see the one-time setup notes at the top of this script."
  exit 1
fi

echo "==> Bumping version to $VERSION"
node -e "
  const fs = require('fs');
  for (const path of ['package.json', 'src-tauri/tauri.conf.json']) {
    const j = JSON.parse(fs.readFileSync(path, 'utf8'));
    j.version = '$VERSION';
    fs.writeFileSync(path, JSON.stringify(j, null, 2) + '\n');
  }
"

echo "==> Building (this can take a few minutes)"
npm run tauri build

BUNDLE_DIR="src-tauri/target/release/bundle"
DMG=$(find "$BUNDLE_DIR/dmg" -name "*.dmg" | head -1)
APP_TAR=$(find "$BUNDLE_DIR/macos" -name "*.app.tar.gz" | head -1)
APP_SIG=$(find "$BUNDLE_DIR/macos" -name "*.app.tar.gz.sig" | head -1)

if [ -z "$DMG" ] || [ -z "$APP_TAR" ] || [ -z "$APP_SIG" ]; then
  echo "Could not find expected build artifacts under $BUNDLE_DIR - check the build output above."
  exit 1
fi

SIG_CONTENT=$(cat "$APP_SIG")
PUBDATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Figure out the repo's "owner/name" from the git remote, so latest.json's
# download URLs point at the release we're about to create.
REMOTE_URL=$(git config --get remote.origin.url || true)
REPO_SLUG=$(echo "$REMOTE_URL" | sed -E 's#(git@github.com:|https://github.com/)##; s#\.git$##')
if [ -z "$REPO_SLUG" ]; then
  echo "Couldn't figure out the GitHub repo from 'git remote origin' - set REPO_SLUG manually and re-run, or edit latest.json by hand after this script finishes."
  REPO_SLUG="REPLACE_WITH_GITHUB_USERNAME/meglanote"
fi

cat > "$BUNDLE_DIR/latest.json" <<JSON
{
  "version": "$VERSION",
  "notes": "See the GitHub release notes.",
  "pub_date": "$PUBDATE",
  "platforms": {
    "darwin-aarch64": {
      "signature": "$SIG_CONTENT",
      "url": "https://github.com/$REPO_SLUG/releases/download/v$VERSION/$(basename "$APP_TAR")"
    }
  }
}
JSON

echo ""
echo "==> Build artifacts ready:"
echo "    DMG:        $DMG"
echo "    Update tar: $APP_TAR"
echo "    Signature:  $APP_SIG"
echo "    Manifest:   $BUNDLE_DIR/latest.json"
echo ""

if command -v gh >/dev/null 2>&1; then
  echo "==> Creating GitHub release v$VERSION and uploading files"
  gh release create "v$VERSION" \
    "$DMG" "$APP_TAR" "$BUNDLE_DIR/latest.json" \
    --title "v$VERSION" \
    --notes "MeglaNote v$VERSION"
  echo "Done - your wife's app will pick this up next time it checks for updates."
else
  echo "The 'gh' command isn't installed, so finish this manually:"
  echo "  1. Go to https://github.com/$REPO_SLUG/releases/new"
  echo "  2. Tag: v$VERSION"
  echo "  3. Upload these three files: the .dmg, the .app.tar.gz, and latest.json"
  echo "  4. Publish the release"
fi
