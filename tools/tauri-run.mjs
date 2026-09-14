#!/usr/bin/env node
// Wrapper for `npm run tauri` (dev/build/bundle).
//
// Why this exists: tauri-cli 2.11.2 does not export TAURI_CONFIG to the
// cargo child process it spawns for `tauri dev`/`tauri build`, contrary to
// the assumption baked into `.cargo/config.toml`'s `[env] TAURI_CONFIG`
// fallback (confirmed by tracing the actual env cargo receives). That
// fallback pins `macOSPrivateApi: false` so plain `cargo build`/`cargo test`
// pass tauri-build's feature-allowlist check against the intentionally
// featureless base `tauri` dependency (see tools/phase-c/validate-config.mjs).
//
// Left alone, that same false value leaks into real `tauri dev`/`tauri
// build` runs too — and TAURI_CONFIG is also read by `generate_context!()`
// to build the config embedded in the compiled binary, so this isn't just a
// build-script lint: a real release build would silently ship with the
// macOS private-API window styling disabled, no error.
//
// Fix: explicitly set TAURI_CONFIG for CLI-driven runs only, computed from
// the actual platform config file rather than hardcoded, so it stays
// correct if that value ever changes. This env var wins over
// `.cargo/config.toml`'s fallback (cargo's `[env]` never overrides a value
// already present in the process environment).
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

const repo = path.resolve(import.meta.dirname, "..");

let macOSPrivateApi = false;
if (process.platform === "darwin") {
  const macConf = JSON.parse(
    readFileSync(path.join(repo, "src-tauri/tauri.macos.conf.json"), "utf8"),
  );
  macOSPrivateApi = macConf.app?.macOSPrivateApi === true;
}

// Run the CLI's JS entry point with this same node binary rather than the
// `tauri` shim on PATH. On Windows that shim is `tauri.cmd`, and Node refuses
// to spawn `.cmd`/`.bat` without `shell: true` — which would then mangle the
// JSON `--config` argument through cmd.exe quoting. Resolving the entry point
// avoids both problems and is independent of how node_modules is laid out.
const cli = createRequire(path.join(repo, "package.json")).resolve(
  "@tauri-apps/cli/tauri.js",
);

const result = spawnSync(process.execPath, [cli, ...process.argv.slice(2)], {
  stdio: "inherit",
  cwd: repo,
  env: {
    ...process.env,
    TAURI_CONFIG: JSON.stringify({ app: { macOSPrivateApi } }),
  },
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
