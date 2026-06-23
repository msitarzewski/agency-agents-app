#!/usr/bin/env bash
#
# release.sh — signed + notarized macOS build of Agency Agents.
#
# Secrets live in the macOS Keychain, NOT in env files or shell history. This
# script pulls them out at build time and hands them to Tauri (which signs with
# your Developer ID, submits to Apple's notary service, and staples the ticket).
#
# ── ONE-TIME SETUP ──────────────────────────────────────────────────────────
# 1. App-specific password (NOT your iCloud password): create one at
#    https://appleid.apple.com → Sign-In and Security → App-Specific Passwords.
#    Then store it in the Keychain:
#
#       security add-generic-password \
#         -a "msitarzewski@mac.com" -s "agency-agents-notary" -U -w
#       # (paste the app-specific password when prompted)
#
#    Optional sanity check that the password actually works with Apple:
#       xcrun notarytool store-credentials "agency-agents" \
#         --apple-id "msitarzewski@mac.com" --team-id "7JQGQ7CRH8" \
#         --password "<app-specific-password>"
#
# 2. Tauri updater signing key (minisign — signs the auto-update artifacts so
#    the app trusts its own updates; separate from Apple signing). Store the
#    PRIVATE key + its password in the Keychain too:
#
#       security add-generic-password -a "agency-agents" -s "agency-agents-updater-key"     -U -w   # paste private key contents
#       security add-generic-password -a "agency-agents" -s "agency-agents-updater-key-pw"  -U -w   # paste its password
#
# After setup, just run:  ./scripts/release.sh
# ────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# Non-secret identifiers (safe to keep in the script).
export APPLE_ID="msitarzewski@mac.com"
export APPLE_TEAM_ID="7JQGQ7CRH8"   # from "Developer ID Application: … (7JQGQ7CRH8)"

NOTARY_SERVICE="agency-agents-notary"
UPDATER_KEY_SERVICE="agency-agents-updater-key"
UPDATER_KEY_PW_SERVICE="agency-agents-updater-key-pw"

kc() { security find-generic-password -a "$1" -s "$2" -w 2>/dev/null || true; }

# ── Apple notarization password (from Keychain) ──
APPLE_PASSWORD="$(kc "$APPLE_ID" "$NOTARY_SERVICE")"
if [[ -z "$APPLE_PASSWORD" ]]; then
  echo "✗ No notarization password in the Keychain (service '$NOTARY_SERVICE')." >&2
  echo "  Run the ONE-TIME SETUP at the top of scripts/release.sh, then retry." >&2
  exit 1
fi
export APPLE_PASSWORD

# ── Tauri updater signing key (from Keychain) ──
# tauri.conf has createUpdaterArtifacts: true, so a normal build signs the
# auto-update bundle and REQUIRES the minisign private key. SKIP_UPDATER=1
# turns the updater artifact OFF for this build (config override) so the run
# exits clean with just the signed+notarized .app/.dmg.
# A --config flag forces the tauri CLI to fully re-resolve the config, which
# auto-merges tauri.macos.conf.json (where macOSPrivateApi:true lives) into the
# config the build-script allowlist check reads. Without ANY --config, that
# check sees only base tauri.conf.json (no macOSPrivateApi) and rejects the
# macos-private-api feature from the [target.macos] block — see tauri#11142.
# (This is why every SKIP_UPDATER build worked but the updater-on path did not.)
BUILD_ARGS=(--config '{"app":{"macOSPrivateApi":true}}')
if [[ "${SKIP_UPDATER:-0}" == "1" ]]; then
  echo "▸ SKIP_UPDATER=1 — building WITHOUT updater artifacts."
  BUILD_ARGS+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
else
  TAURI_SIGNING_PRIVATE_KEY="$(kc "agency-agents" "$UPDATER_KEY_SERVICE")"
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(kc "agency-agents" "$UPDATER_KEY_PW_SERVICE")"
  if [[ -z "$TAURI_SIGNING_PRIVATE_KEY" ]]; then
    echo "✗ No updater signing key in the Keychain (service '$UPDATER_KEY_SERVICE')." >&2
    echo "  Store it (see SETUP) or run with SKIP_UPDATER=1 to skip update artifacts." >&2
    exit 1
  fi
  export TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
fi

# Build BOTH macOS architectures so we ship Apple Silicon + Intel DMGs. Each
# build signs with your Developer ID, notarizes, and staples. The HOST arch
# builds natively into target/release/bundle/; other arches cross-compile into
# target/<triple>/release/bundle/. Override the set with e.g.
# RELEASE_TARGETS="aarch64-apple-darwin x86_64-apple-darwin".
#
# ⚠️ macOS BETA TOOLCHAIN GOTCHA: on a beta macOS/Xcode (e.g. Tahoe/26+), this
# build can fail with "can't find crate for <proc-macro>" (ctor_proc_macro,
# tauri_macros, serde_with_macros, …). Root cause: the MACOSX_DEPLOYMENT_TARGET
# the Tauri CLI sets (from minimumSystemVersion) breaks FRESH proc-macro
# compilation under the beta toolchain. Stable macOS / CI are unaffected.
# Recovery (pre-compile proc-macros without that env var, then let Tauri reuse
# the cache to bundle/sign/notarize) is documented in docs/BUILD.md →
# "Troubleshooting: proc-macro 'can't find crate' on a beta macOS".
read -r -a TARGETS <<< "${RELEASE_TARGETS:-aarch64-apple-darwin x86_64-apple-darwin}"
HOST_TRIPLE="$(rustc -vV | awk '/^host:/{print $2}')"
DMG_DIRS=()

for target in "${TARGETS[@]}"; do
  if [[ "$target" == "$HOST_TRIPLE" ]]; then
    echo "▸ Building signed + notarized Agency Agents for ${target} (native, Team $APPLE_TEAM_ID)…"
    npm run tauri build -- ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"}
    DMG_DIRS+=("src-tauri/target/release/bundle/dmg/")
  else
    echo "▸ Building signed + notarized Agency Agents for ${target} (cross, Team $APPLE_TEAM_ID)…"
    npm run tauri build -- --target "$target" ${BUILD_ARGS[@]+"${BUILD_ARGS[@]}"}
    DMG_DIRS+=("src-tauri/target/${target}/release/bundle/dmg/")
  fi
done

echo
echo "✓ Done. Signed + notarized DMGs:"
for d in "${DMG_DIRS[@]}"; do echo "    $d"; done
