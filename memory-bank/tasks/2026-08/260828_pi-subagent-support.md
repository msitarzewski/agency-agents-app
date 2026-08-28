# 260828_pi-subagent-support

## Objective

Add Pi as an installable Agency Agents target using Pi Subagent definitions rather than Pi Skills.
Support both the default global agent directory and project-local agent directory without adding a
new dependency.

## Outcome

- ✅ Pi is an installable, dual-scope tool in the shared app registry.
- ✅ Global installs target `~/.pi/agent/agents/<slug>.md`.
- ✅ Project installs target `<project>/.pi/agents/<slug>.md`.
- ✅ Pi uses the supplied official `currentColor` SVG mark across tool badges.
- ✅ Pi uses a deterministic `pi-agent-md` Subagent shape (`name`, quoted `description`, body).
  This deliberately drops source `tools:` metadata, whose Claude-style values conflict with
  `pi-subagents` tool allowlist semantics.
- ✅ User reviewed and approved the app implementation on 2026-08-28.

## Files Modified

- `src-tauri/data/tools.json` — registered Pi detection, version probe, scopes, format, and destinations.
- `src-tauri/src/registry.rs` — included Pi in the installable registry contract.
- `src-tauri/src/render/mod.rs` — documented the shared-safe renderer behavior and tested Pi output/paths.
- `src/lib/assets/tools/pi.svg` — added the official Pi tool mark supplied by the user.
- `README.md` — documented Pi Subagent destinations and the `pi-subagents` prerequisite.

## Patterns Applied

- `memory-bank/systemPatterns.md#3-deterministic-renderer-plan-b--load-bearing` — added the minimal
  deterministic Pi format needed to avoid changing existing Gemini output.
- `memory-bank/decisions.md#2026-06-21-tool-registry-as-the-single-source-of-truth-drop-the-tool-enum`
  — added Pi through the registry so backend and frontend pick it up automatically.
- No new components or dependencies were needed; the supplied Pi SVG is the only new app asset.

## Integration Points

- `registry::wired()` exposes Pi to backend tool listing and install operations.
- Frontend `toolRegistry.ts` derives Pi as installable because `pi-agent-md` is implemented.
- Existing install/reconcile/uninstall logic resolves the registered user/project destination templates.
- `pi-subagents` discovers the emitted filename as the agent type and the Markdown body as its system prompt.

## Validation

- Rust: `273 passed; 0 failed; 2 ignored` (`cargo test --manifest-path src-tauri/Cargo.toml --lib`).
- Pi renderer parity: all 273 current upstream agents matched the canonical converter byte-for-byte.
- Production frontend: `npm run build` passed.
- Rust LSP: zero diagnostics in changed Rust files.
- Actual `pi-subagents` parser smoke test passed for name, description, body, and default tool access.
- ego-browser: Pi appears as an installable tool under Tools → All, renders the official two-path mark,
  and opens its normal management detail.
- Local arm64 `.app` and DMG were built, ad-hoc signature verified, and the DMG mounted successfully.
- `git diff --check` and JSON parsing passed.

## Known Baseline / Scope Limits

- `npm run check` still fails on the unchanged `vite.config.js:4` unused `@ts-expect-error` directive.
- Repository-wide `cargo fmt --check` still reports pre-existing formatting drift.
- Plain-browser QA reports expected Tauri event-bridge errors; no failed HTTP requests were observed.
- Canonical catalog support is handled in a separate `agency-agents` pull request.
- Pi requires the `pi-subagents` extension; default `PI_CODING_AGENT_DIR=~/.pi/agent` is covered.

## Artifacts

- Branch: `feat/pi-support`
- Local DMG: `src-tauri/target/release/bundle/dmg/Agency_Agents_0.3.0_pi-icon_local_aarch64.dmg`
