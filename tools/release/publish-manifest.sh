#!/usr/bin/env bash
# Phase 15 — emit the in-app updater manifest for a released .app.tar.gz.
#
# Usage:
#   tools/release/publish-manifest.sh 0.3.0
#
# What it does:
#   1. Validates the version argument shape.
#   2. Locates the .app.tar.gz artifact at the canonical macos bundle
#      path. **The Tauri updater plugin's macOS install path expects a
#      gzipped tar of the .app bundle — NOT the .dmg.** Feeding it a
#      .dmg results in an "invalid gzip" error on every install attempt.
#      The .dmg is still uploaded to GitHub Releases for fresh installs;
#      only the auto-updater path needs the .app.tar.gz.
#   3. Computes the artifact's SHA-256 digest.
#   4. Reads the .sig file the Tauri build already produced beside the
#      artifact (the bundler runs minisign-via-TAURI_SIGNING_* during
#      `npm run tauri build` — see `tools/build/sign-and-notarize.sh`).
#      We do NOT re-sign here: doing so would produce a minisign-native
#      .minisig that the Tauri plugin's verification path doesn't accept.
#      The Tauri-format .sig is what the embedded pubkey was generated
#      to verify against.
#   5. Emits dist/updater.json with the shape the Tauri updater
#      plugin expects:
#        {
#          "version": "0.3.0",
#          "notes": "<release notes — empty placeholder for now>",
#          "pub_date": "2026-05-24T00:00:00Z",
#          "platforms": {
#            "darwin-aarch64": {
#              "signature": "<contents of .app.tar.gz.sig, single-line>",
#              "url": "<github release asset URL of the .app.tar.gz>",
#              "sha256": "<artifact digest>"
#            }
#          }
#        }
#   6. Echoes (but does NOT execute) the rsync command the user runs
#      to publish the manifest to agencyagents.app via
#      umacbookpro:Sites/agency-agents/updater.json (the Caddy docroot
#      for agencyagents.app). Publishing is a
#      deliberate manual step.
#
# What it does NOT do:
#   - Generate the minisign keypair (one-time setup, see BUILD.md).
#   - Re-sign the artifact (Tauri's bundler already did via TAURI_SIGNING_*).
#   - Publish to the CDN (the rsync is the user's call).
#   - Build the artifact (npm run tauri build is upstream of this).
#   - Update CHANGELOG.md or push the git tag.
#   - Upload the .app.tar.gz to GitHub Releases (the user attaches it
#     to the `gh release create` invocation, alongside the .dmg).
#
# Exit codes:
#   0  — manifest written successfully
#   1  — usage error
#   2  — artifact or .sig missing
#   3  — sha256 tooling missing

set -euo pipefail

# ---------- Argument validation ----------

if [[ $# -ne 1 ]]; then
    echo "usage: $0 <version>" >&2
    echo "  e.g. $0 0.3.0" >&2
    exit 1
fi

VERSION="$1"
# Reject anything that isn't strict semver-three-part. Defense against
# accidental "v0.3.0" or "0.3" arguments that would mis-name the artifact.
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
    echo "error: VERSION must be semver (got: $VERSION)" >&2
    echo "  expected: <major>.<minor>.<patch>[-prerelease]" >&2
    exit 1
fi

# ---------- Paths ----------

# Resolve repo root from this script's location so it works regardless
# of the caller's cwd. `realpath` is portable on macOS via coreutils;
# we fall back to BASH_SOURCE-based resolution if it isn't installed.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# The Tauri bundler emits the updater artifact + signature at these
# paths. Filenames are fixed (no version stamp) inside `bundle/macos/`;
# we upload to GitHub Releases under versioned names so the manifest
# URL is unambiguous.
ARTIFACT_PATH="$REPO_ROOT/src-tauri/target/release/bundle/macos/Agency Agents.app.tar.gz"
SIGNATURE_FILE="${ARTIFACT_PATH}.sig"
# Versioned name used in the published GitHub Release asset URL.
ARTIFACT_RELEASE_NAME="Agency_Agents_${VERSION}_aarch64.app.tar.gz"
DIST_DIR="$REPO_ROOT/dist"
MANIFEST_PATH="$DIST_DIR/updater.json"

# ---------- Preflight ----------

if [[ ! -f "$ARTIFACT_PATH" ]]; then
    echo "error: updater artifact not found at $ARTIFACT_PATH" >&2
    echo "  did you run 'npm run tauri build' first?" >&2
    echo "  the macOS updater target produces .app.tar.gz alongside the .dmg." >&2
    exit 2
fi

if [[ ! -f "$SIGNATURE_FILE" ]]; then
    echo "error: updater signature not found at $SIGNATURE_FILE" >&2
    echo "  The Tauri bundler produces this when TAURI_SIGNING_PRIVATE_KEY[_PATH]" >&2
    echo "  + TAURI_SIGNING_PRIVATE_KEY_PASSWORD are set during 'npm run tauri build'." >&2
    echo "  Source ~/.config/agency-agents-app/signing.env (exports TAURI_SIGNING_PRIVATE_KEY" >&2
    echo "  [_PATH] + _PASSWORD for the agency updater key), then re-run the build." >&2
    exit 2
fi

if ! command -v shasum >/dev/null 2>&1; then
    echo "error: shasum not on PATH" >&2
    echo "  on macOS this is built-in; on Linux: apt install perl" >&2
    exit 3
fi

# ---------- Compute hash ----------

echo "info: computing SHA-256 of $(basename "$ARTIFACT_PATH")..." >&2
SHA256=$(shasum -a 256 "$ARTIFACT_PATH" | awk '{print $1}')
echo "info: sha256 = $SHA256" >&2

# ---------- Read signature ----------

# Tauri's bundler emits a single-line base64 blob in the .sig file
# (Tauri's format, NOT raw minisign's multi-line .minisig format).
# The plugin's verification path reads this string directly and parses
# it against the embedded pubkey; do not re-encode.
SIGNATURE_RAW=$(cat "$SIGNATURE_FILE")
# Trim trailing newline if present (shasum + cat both append one).
SIGNATURE_JSON="${SIGNATURE_RAW%$'\n'}"
# Defensive: JSON-escape any literal newlines (Tauri's .sig is normally
# a single line but be belt-and-braces in case bundler version drift
# changes the shape).
SIGNATURE_JSON=$(perl -pe 's/\n/\\n/g' <<< "$SIGNATURE_JSON")
SIGNATURE_JSON="${SIGNATURE_JSON%\\n}"

# ---------- Emit manifest ----------

mkdir -p "$DIST_DIR"

# Release notes are intentionally a placeholder. The user can hand-edit
# the manifest before the rsync to inject the CHANGELOG entry — keeping
# the manifest generator and the release notes editorial step separate
# is the simpler shape than wiring CHANGELOG parsing here.
PUB_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)
URL="https://github.com/msitarzewski/agency-agents-app/releases/download/v${VERSION}/${ARTIFACT_RELEASE_NAME}"

cat > "$MANIFEST_PATH" <<EOF
{
  "version": "${VERSION}",
  "notes": "See https://github.com/msitarzewski/agency-agents-app/releases/tag/v${VERSION} for release notes.",
  "pub_date": "${PUB_DATE}",
  "platforms": {
    "darwin-aarch64": {
      "signature": "${SIGNATURE_JSON}",
      "url": "${URL}",
      "sha256": "${SHA256}"
    }
  }
}
EOF

echo ""
echo "✓ manifest written to: $MANIFEST_PATH"
echo "  version:    $VERSION"
echo "  sha256:     $SHA256"
echo "  url:        $URL"
echo "  pub_date:   $PUB_DATE"
echo "  signature:  $(wc -c < "$SIGNATURE_FILE" | tr -d ' ') bytes from $SIGNATURE_FILE"
echo ""
echo "next steps (run manually):"
echo "  1. rsync -av $MANIFEST_PATH umacbookpro:Sites/agency-agents/updater.json"
echo "  2. gh release create v${VERSION} \\"
echo "       src-tauri/target/release/bundle/dmg/Agency\\ Agents_${VERSION}_aarch64.dmg \\"
echo "       $ARTIFACT_PATH#${ARTIFACT_RELEASE_NAME} \\"
echo "       --notes-file <release-notes.md>"
echo ""
echo "verify before publishing:"
echo "  shasum -a 256 $ARTIFACT_PATH"
echo "  curl -s https://agencyagents.app/updater.json | jq"
