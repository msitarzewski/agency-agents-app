#!/bin/bash
# Derive app UI and landing page assets from a synced Icon Lab release.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RELEASE="$(cd "${1:-$HOME/Software/icon-lab/current}" && pwd -P)"
ASSETS="$RELEASE/agency"
LANDING="$ROOT/landing"

SYNCED_RELEASE="$(plutil -extract release raw -o - "$ROOT/.icon-release.json" 2>/dev/null || true)"
if [ "$SYNCED_RELEASE" != "$(basename "$RELEASE")" ] ||
  ! cmp -s "$ASSETS/png/plated-256.png" "$ROOT/src/lib/assets/app-icon.png"; then
  echo "Sync the matching Icon Lab Agency Agents release before running this script." >&2
  exit 1
fi

sips -z 256 256 "$ASSETS/preview/mac-Dark.png" --out "$ROOT/src/lib/assets/app-icon-dark.png" >/dev/null
sips -z 320 320 "$ASSETS/preview/mac-Default.png" --out "$LANDING/agency-agents.png" >/dev/null
sips -z 320 320 "$ASSETS/preview/mac-Dark.png" --out "$LANDING/agency-agents-dark.png" >/dev/null

cp "$ASSETS/tauri/icon.ico" "$LANDING/favicon.ico"
cp "$ASSETS/png/plated-16.png" "$LANDING/favicon-16x16.png"
cp "$ASSETS/png/plated-32.png" "$LANDING/favicon-32x32.png"
cp "$ASSETS/png/plated-512.png" "$LANDING/icon-512.png"
sips -z 180 180 "$ASSETS/png/plated-512.png" --out "$LANDING/apple-touch-icon.png" >/dev/null
sips -z 192 192 "$ASSETS/png/plated-512.png" --out "$LANDING/icon-192.png" >/dev/null

echo "App and landing page icons refreshed from $(basename "$RELEASE")."
