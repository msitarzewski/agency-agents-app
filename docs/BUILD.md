# Building Agency Agents

## Development Build

```sh
npm install
npm run tauri dev
```

The dev server runs on port `1430`. The app opens with HMR for frontend changes; Rust changes trigger a backend rebuild.

## Local QA Batch

The repeatable Phase C batch is:

```sh
npm run build:phase-c
```

It runs the local build/test checks and static platform-config validation.

When the configured Ubuntu and Windows VM targets are available:

```sh
npm run build:phase-c:full
```

This includes VM-assisted packaging checks. Do not claim native runtime verification for an OS unless the app was actually launched there.

## Standard Checks

```sh
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run check
npm run build
```

Renderer parity against the active AA repo:

```sh
AGENCY_AGENTS_PARITY_ROOT=/Users/michael/Software/AgentLand/agency-agents \
cargo test --manifest-path src-tauri/Cargo.toml upstream_convert_sh_is_byte_identical_for_transform_tools -- --ignored
```

## Release Build On macOS

The release build produces a signed `.app` and `.dmg`.

### Prerequisites

1. Apple Developer ID Application certificate in the login keychain:

   ```sh
   security find-identity -v -p codesigning
   ```

2. App-specific Apple password generated at <https://appleid.apple.com>.

3. Signing environment outside the repo:

   ```sh
   mkdir -p ~/.config/agency-agents-app
   chmod 700 ~/.config/agency-agents-app
   ```

   Create `~/.config/agency-agents-app/signing.env`:

   ```sh
   export APPLE_ID="your@email.com"
   export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"
   export APPLE_TEAM_ID="XXXXXXXXXX"
   # Optional:
   # export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
   ```

   Then:

   ```sh
   chmod 600 ~/.config/agency-agents-app/signing.env
   ```

### Build

```sh
source ~/.config/agency-agents-app/signing.env
./scripts/release.sh
```

If using the lower-level Tauri build directly:

```sh
npm run tauri build
```

Expected artifacts live under:

```text
src-tauri/target/release/bundle/
```

## Verify macOS Artifacts

```sh
DMG=src-tauri/target/release/bundle/dmg/Agency\ Agents_0.1.0_aarch64.dmg

codesign -dv --verbose=4 "$DMG"
spctl --assess --type install --verbose=4 "$DMG"
xcrun stapler validate "$DMG"
```

The exact filename may vary by version and architecture.

## Updater Manifest

Agency Agents uses `tauri-plugin-updater`. The configured endpoint is:

```text
https://agency-agents-app.zerologic.com/updater.json
```

The updater artifact is a gzipped `.app` tarball, not the `.dmg`.

Manifest generation is handled by:

```sh
tools/release/publish-manifest.sh <version>
```

The updater public key is embedded in the app config/source. The matching private key must live outside the repo, for example:

```text
~/.config/agency-agents-app/updater.key
```

## Release Checklist

The ordered runbook for cutting a release. The mechanics referenced here are detailed in the sections above.

### Decisions for v0.1.0 (first release)

- **Version:** `0.1.0` — already set in `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`. No bump needed.
- **Distribution:** signed + notarized `.dmg`, manual download.
- **Auto-update: deferred.** The updater public key ships, but the endpoint (`agencyagents.app/updater.json`) is not yet provisioned. Build with `SKIP_UPDATER=1` so no updater artifact/manifest is expected. A later release turns auto-update on once the endpoint serves a manifest.
- **Out of scope (known limitations, noted in the release notes):** auto-update, multi-file renderers (antigravity / openclaw / aider / windsurf), Windows/Linux runtime verification, and the local-runtime (Ollama / LM Studio) target.

### Steps

1. **Pre-flight — confirm green on a clean `main`:**

   ```sh
   (cd src-tauri && cargo test --lib)                 # expect 258/0
   AGENCY_AGENTS_PARITY_ROOT=<clone> cargo test --manifest-path src-tauri/Cargo.toml --lib \
     upstream_convert_sh_is_byte_identical_for_transform_tools -- --ignored   # expect 1160/1160
   npm run check                                      # 0 errors
   npm run build                                      # clean
   ```

   Or the bundled batch: `npm run build:phase-c`.

2. **Finalize release notes** — `git mv docs/release-notes/unreleased.md docs/release-notes/0.1.0.md`, retitle to `Agency Agents v0.1.0 - <date>`, drop the staging-file line, then recreate an empty `unreleased.md` stub for the next cycle.

3. **Build** — `SKIP_UPDATER=1 ./scripts/release.sh` (see *Release Build On macOS*). Produces the signed, notarized, stapled `.app` + signed `.dmg`.

4. **Verify artifacts** — run the `codesign` / `spctl` / `stapler` checks (see *Verify macOS Artifacts*).

5. **Smoke test** — mount the `.dmg`, launch, confirm Gatekeeper acceptance, the first-run catalog picker, and the Tahoe glass icon.

6. **Publish** — `git tag v0.1.0 && git push origin v0.1.0`, then `gh release create v0.1.0 <dmg> --notes-file docs/release-notes/0.1.0.md`.

7. **Post-release** — log the cut in `memory-bank/agentLog.md`; open the next milestone for the deferred items (updater endpoint, Phase 5 quality gate).

### Enabling auto-update (a later release)

1. Provision `agency-agents-app.zerologic.com/updater.json` to serve the manifest.
2. Confirm the updater private key is available (Keychain, or `~/.config/agency-agents-app/updater.key`).
3. Build **without** `SKIP_UPDATER`, then run `tools/release/publish-manifest.sh <version>` and host the gzipped `.app` tarball + manifest at the endpoint.

## macOS Icon Notes

macOS 26 Tahoe uses the compiled `Assets.car` path for Liquid Glass icons. Do not blindly run `npm run tauri icon`; it can clobber the curated icon outputs.

See [docs/icon/README-liquid-glass.md](./icon/README-liquid-glass.md).

## Cross-Platform Notes

- macOS uses overlay titlebar, vibrancy, and the Tahoe icon setup.
- Windows and Linux use native decorated opaque windows.
- Windows Intel and ARM builds should be verified as separate artifacts.
- Linux packages should be smoke-tested in the Ubuntu VM before claiming support.

## Troubleshooting: proc-macro "can't find crate" on a beta macOS

On a **beta macOS / Xcode** (first hit on macOS 26/27 Tahoe beta during the v0.1.0 cut), `scripts/release.sh` can fail with:

```
error[E0463]: can't find crate for `ctor_proc_macro`   (or tauri_macros / serde_with_macros / …)
```

**Root cause:** the Tauri CLI sets `MACOSX_DEPLOYMENT_TARGET` (from `bundle.macOS.minimumSystemVersion`, `13.0`) before invoking cargo, and on the beta toolchain that breaks *fresh* proc-macro dylib compilation. The exact same `cargo build --bins --features tauri/custom-protocol --release` succeeds **bare** (no `MACOSX_DEPLOYMENT_TARGET`) and on the **CI runners** (stable macOS) — so prefer CI for releases when possible. It is non-deterministic (a different proc-macro fails each run) and is NOT a code problem.

**Local recovery** — pre-compile the crate graph (incl. proc-macros) *without* `MACOSX_DEPLOYMENT_TARGET`, using Tauri's exact config so it won't recompile, then let `release.sh` bundle/sign/notarize off the warm cache (proc-macro fingerprints don't include `MACOSX_DEPLOYMENT_TARGET`, so they're reused):

```sh
# 1. Capture the exact TAURI_CONFIG the CLI passes to cargo (one-shot; the run
#    is expected to fail at a proc-macro — we only need the dumped value).
cat > /tmp/dump.sh <<'W'
#!/bin/bash
[ -f /tmp/tc.txt ] || printf '%s' "$TAURI_CONFIG" > /tmp/tc.txt
exec "$@"
W
chmod +x /tmp/dump.sh; rm -f /tmp/tc.txt
RUSTC_WRAPPER=/tmp/dump.sh npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}' >/dev/null 2>&1 || true
TC=$(cat /tmp/tc.txt)

# 2. Per arch: pre-build WITHOUT MACOSX_DEPLOYMENT_TARGET, then bundle.
#    (For the non-host arch, use a rustup toolchain that has the target's std.)
rm -rf src-tauri/target
env -u MACOSX_DEPLOYMENT_TARGET TAURI_CONFIG="$TC" \
  cargo build --bins --features tauri/custom-protocol --release   # host arch
SKIP_UPDATER=1 RELEASE_TARGETS="aarch64-apple-darwin" ./scripts/release.sh

env -u MACOSX_DEPLOYMENT_TARGET TAURI_CONFIG="$TC" \
  cargo build --bins --features tauri/custom-protocol --release --target x86_64-apple-darwin
SKIP_UPDATER=1 RELEASE_TARGETS="x86_64-apple-darwin" ./scripts/release.sh
```

The cleaner long-term fix is to cut macOS releases on a stable macOS box or a macOS CI runner. (Homebrew/CLT vs rustup also matters for the x86_64 cross — that target's std lives in the rustup toolchain; put `~/.rustup/toolchains/stable-<host>/bin` first on `PATH`.)

## Secrets

Never commit signing credentials, updater private keys, GitHub tokens, or notary credentials. Use local keychain items or files outside the repo with `0600` permissions.
