#!/usr/bin/env bash
#
# release-linux.sh — build Linux desktop bundles (.deb / .rpm / .AppImage) for
# Agency Agents inside a Docker container, so they can be produced from a Mac
# (or any Docker host) without a native Linux machine.
#
#   ./scripts/release-linux.sh            # x86_64 (default; QEMU-emulated on Apple Silicon)
#   PLATFORM=linux/arm64 ./scripts/release-linux.sh   # native arm64 on an Apple-Silicon Mac
#
# Why a container: Tauri's Linux bundling links native GTK/WebKitGTK and uses
# Linux-only packagers (dpkg-deb, rpmbuild, appimagetool), so it must run on
# Linux. On Apple Silicon, `--platform linux/amd64` runs the x86_64 toolchain
# under QEMU — correct output, just slow.
#
# Artifacts land in dist/linux-<arch>/. Unsigned (standard for Linux desktop).
# Updater artifacts are disabled (matches SKIP_UPDATER=1 for the macOS DMG).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PLATFORM="${PLATFORM:-linux/amd64}"
ARCH_TAG="$(printf '%s' "$PLATFORM" | sed 's#linux/##')"
OUT="$ROOT/dist/linux-${ARCH_TAG}"
IMAGE="ubuntu:22.04"

mkdir -p "$OUT"

echo "▸ Building Linux bundles for $PLATFORM (image $IMAGE)"
echo "  repo: $ROOT"
echo "  out:  $OUT"
echo "  (QEMU emulation is slow for non-native arches — be patient.)"

docker run --rm --platform "$PLATFORM" \
  -v "$ROOT":/src:ro \
  -v "$OUT":/out \
  -e DEBIAN_FRONTEND=noninteractive \
  -e APPIMAGE_EXTRACT_AND_RUN=1 \
  "$IMAGE" bash -euo pipefail -c '
    echo "::deps"
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends \
      curl wget file ca-certificates rsync xz-utils \
      build-essential pkg-config \
      libwebkit2gtk-4.1-dev librsvg2-dev libssl-dev \
      libayatana-appindicator3-dev libxdo-dev \
      patchelf rpm fakeroot \
      xdg-utils librsvg2-bin libgtk-3-bin >/dev/null
    # ^ xdg-utils (xdg-open), librsvg2-bin (rsvg-convert), libgtk-3-bin
    #   (gtk-update-icon-cache) are needed by the AppImage GTK bundler.

    echo "::node"
    curl -fsSL https://deb.nodesource.com/setup_22.x | bash - >/dev/null 2>&1
    apt-get install -y -qq nodejs >/dev/null
    echo "  node $(node -v) / npm $(npm -v)"

    echo "::rust"
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal >/dev/null 2>&1
    . "$HOME/.cargo/env"
    echo "  $(rustc --version)"

    echo "::copy source (excluding host build dirs)"
    rsync -a \
      --exclude .git --exclude node_modules --exclude dist \
      --exclude target --exclude src-tauri/target \
      --exclude .svelte-kit --exclude build --exclude .phase-c \
      /src/ /build/
    cd /build

    echo "::npm ci"
    npm ci --no-audit --no-fund

    echo "::tauri build (deb, rpm, appimage; no updater artifacts)"
    set +e
    npm run tauri build -- \
      --bundles deb,rpm,appimage \
      --config "{\"bundle\":{\"createUpdaterArtifacts\":false}}"
    BUILD_RC=$?
    set -e

    # Collect whatever bundled, even on partial failure, so one broken bundler
    # never discards the others with the container.
    echo "::collect (build rc=$BUILD_RC)"
    find src-tauri/target -path "*/release/bundle/*" \
      \( -name "*.deb" -o -name "*.rpm" -o -name "*.AppImage" \) \
      -exec cp -v {} /out/ \; || true

    exit $BUILD_RC
  '

echo ""
echo "✓ Linux ($ARCH_TAG) bundles:"
ls -lh "$OUT"
