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
# Each release, run:  ./release.sh 1.1.0 "What's new in this version"
# The description is optional. It's shown in the app's "Update available"
# popup and on the GitHub release page.

set -euo pipefail

VERSION="${1:-}"
NOTES="${2:-}"
if [ -z "$VERSION" ]; then
  echo "Usage: ./release.sh <version> [\"what's new\"]   e.g. ./release.sh 1.1.0 \"Fixed X, added Y\""
  exit 1
fi
if [ -z "$NOTES" ]; then
  NOTES="MeglaNote v$VERSION"
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
# Match this version's .dmg specifically - older versions' .dmg files stay
# in the folder between builds.
DMG=$(find "$BUNDLE_DIR/dmg" -name "*_${VERSION}_*.dmg" | head -1)
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

# Written with node so quotes etc. in the release notes can't break the JSON.
VERSION="$VERSION" NOTES="$NOTES" PUBDATE="$PUBDATE" SIG_CONTENT="$SIG_CONTENT" \
URL="https://github.com/$REPO_SLUG/releases/download/v$VERSION/$(basename "$APP_TAR")" \
OUT="$BUNDLE_DIR/latest.json" node -e "
  const e = process.env;
  const manifest = {
    version: e.VERSION,
    notes: e.NOTES,
    pub_date: e.PUBDATE,
    platforms: { 'darwin-aarch64': { signature: e.SIG_CONTENT, url: e.URL } }
  };
  require('fs').writeFileSync(e.OUT, JSON.stringify(manifest, null, 2) + '\\n');
"

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
    --notes "$NOTES"
  echo "Done - your wife's app will pick this up next time it checks for updates."
else
  echo "The 'gh' command isn't installed, so finish this manually:"
  echo "  1. Go to https://github.com/$REPO_SLUG/releases/new"
  echo "  2. Tag: v$VERSION"
  echo "  3. Upload these three files: the .dmg, the .app.tar.gz, and latest.json"
  echo "  4. Paste in the release notes: $NOTES"
  echo "  5. Publish the release"
fi
