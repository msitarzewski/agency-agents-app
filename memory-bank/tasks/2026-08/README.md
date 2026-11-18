# Tasks — 2026-08 (catch-up: covers the 07→08 arc)

The memory-bank went quiet after the v0.2.0 ship (2026-06-23); the working log through July lived in the
per-session resume notes. This README catches the bank up on the July→August arc. `main` @ `04c10be`.
See `NEXT-SESSION.md` (2026-08-10 block) for the authoritative live picture.

## Shipped / merged (branch → PR → merge)

- **v0.3.0 release (~07-05)** — Runbooks headline (strategy/ runbooks as their own nav pillar, ⌘5; Activity → ⌘6).
- **#56** i18n copy fixes · **#58** Linux AppImage EGL crash — root cause was a stale bundled `libwayland-*`
  vs host Mesa; fix un-bundles them in `linux-build.yml` (reporter-confirmed on Arch/Hyprland; **closed #57**).
- **#59** Russian Runbooks i18n.
- **#68** agent-updates modal — "N updates available" agents×tools **dot-grid** (matches InstallModal), per-agent
  selection, drives `install.bulk("update", …)`.
- **#64** ZCode tool support (Z.ai GLM harness; `zcode-md` renderer byte-identical to `qwen-md`) + **#70** ZCode
  brand icon (lobehub `zhipu` mark, monochrome so it tints with the tool accent). Upstream-first: catalog #700
  merged before app #64.
- **#63** project-only install guidance (dead "—" → explanation; **closed #40**).
- **#74** Antigravity uninstall fix — skill-md tools install each agent as a `<slug>/SKILL.md` **directory**;
  uninstall only removed the leaf file, orphaning the dir, which the reconcile scan re-surfaced as an untracked
  phantom. Fix: `remove_dir` the now-empty agent dir (empty-only, gated on `tool_is_dir_unit`). **Closed #60.**
  CI green on Linux + Windows.
- **#73** Persian (fa-IR) localization (contributor @montajebii; reviewed → change-requested → re-reviewed →
  merged; **closed #72**).

## In flight (open PRs)

- **#81 RTL layout support (Phase 1)** — the session's focus. `document.documentElement.dir` off `isRTL(locale)`;
  flexbox mirrors most of the UI for free. Hand-fixed the two non-flex spots: the **titlebar** (physical `left:`
  → `right:` in RTL + macOS traffic-light clearance) and **Settings.svelte** close-X (`right:` →
  `inset-inline-end`). Verified live in Persian on macOS. Phase 2 (logical-property sweep, ~½ day) not started.
- **#82** add missing `category.healthcare` label (stale-catalog i18n gap; en + fa `سلامت`).
- **#85** Windows console-flash · **#80** winget README · **#77** `npm run tauri` TAURI_CONFIG fix · **#69**
  WebView2 embedBootstrapper · **#67** clone-on-first-run (removes bundled baseline) · **#62** Runbooks polish.
  → #67/#69 need Linux+Windows CI dispatched before merge.

## Key decisions / facts recorded this arc

- **Localization is layered across two repos.** App chrome + division `category.*` labels are app i18n
  (translatable). **Agent names/descriptions/personas are catalog content, authored upstream in English, and are
  NOT translated by design** — documented in `en.ts:49`. English agent names inside a localized UI = correct.
- **Catalog integrations never auto-appear in the app.** The Tools list is compiled in
  (`registry.rs include_str!(tools.json)` + `IMPLEMENTED_FORMATS`), so every new tool needs a **paired app PR**
  (definition + renderer + format gate + brand icon), even after the catalog ships it.
- **clone-on-first-run pivot (#67)** — remove the bundled corpus snapshot (perpetually drifts) and clone the live
  catalog from GitHub on first run instead. Tradeoff: first run needs network.

## Dev gotchas (see NEXT-SESSION.md for the full list)

- `tauri dev` on macOS re-injects `macos-private-api` into base `Cargo.toml`; run with
  `--config '{"app":{"macOSPrivateApi":true}}'` and revert Cargo.toml after (PR #77 fixes it).
- Vite does not watch `src-tauri/data/tools.json` (outside `src/`) — restart the dev server after it changes; a
  webview reload won't pick it up. Dev-only.

## Completed

### 2026-08-28: Pi Subagent support

Added Pi as a dual-scope install target using native Subagent definitions:
`~/.pi/agent/agents/<slug>.md` globally and `.pi/agents/<slug>.md` per project. The renderer
preserves five Claude metadata fields and maps the current `tools:` vocabulary.
See [260828_pi-subagent-support.md](./260828_pi-subagent-support.md).
