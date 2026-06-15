# Tasks — 2026-06

Agency Agents was forked from brew-browser and stood up this month.

## Completed

### 2026-06-05: Phase 0 — Fork & rebrand
Forked the brew-browser scaffold into `agency-agents-app`, rebranded to "Agency Agents"
(`com.zerologic.agency-agents-app`). Green `cargo check` + `vite build`.
See [260605_phase0-fork-rebrand.md](./260605_phase0-fork-rebrand.md).

### 2026-06-05: Phase 1 — Corpus + Discover
Built the corpus subsystem (parse, sha256 split-hash index, GitHub-tarball refresh) and the Agents
catalog view. Catalog = 210 agent personas / 16 categories. Verified live; fixed a concurrent-seed
race. See [260605_phase1-corpus-discover.md](./260605_phase1-corpus-discover.md).

### 2026-06-05: Phase 1.5 — Agency-first polish
Real Lucide category icons, lean agency sidebar (🤖 brand, Agents/Activity nav, agent-count footer),
default landing = Agents, window vibrancy. Verified live.

### 2026-06-05: Phase 2 — Install + Reconcile
Native deterministic per-tool renderers + ledger + 5-state reconcile + tools/projects + install UI.
630 tests green; install verified end-to-end (write→disk→reconcile).
See [260605_phase2-install-reconcile.md](./260605_phase2-install-reconcile.md).

### 2026-06-05: Phase 2 follow-ups + Phase 3
Library + Tools views, Foreign-sweep + Adopt (validated against a real install.sh run: 180 agents),
update_kind. Then Phase 3: Loadouts (Agentfile export/import) + agency Dashboard rollup.
See [260605_phase3-loadouts-dashboard.md](./260605_phase3-loadouts-dashboard.md).

### 2026-06-14: Phase C — Renderer parity + uninstall safety + cross-platform chrome
Closed both IMMEDIATE backlog items. Renderer parity VERIFIED — Rust `render/` is byte-identical to the
upstream `scripts/convert.sh` (232 agents × 5 transform tools = 1160/1160). Uninstall safety RESOLVED
(backup-first; modified recoverable, byte-identical none, backup-fail aborts). Cross-platform titlebar
degradation via a `tauri.macos.conf.json` config split. New `tools/phase-c/` validation runner.
cargo 258/0 (+parity 1/0), svelte-check 0, build clean.
See [260614_phase-c-parity-safety.md](./260614_phase-c-parity-safety.md).

### 2026-06-15: Pre-release polish (release plan + brew vestiges + Activity Journal + Tools lens)
Documented the v0.1.0 release runbook (`docs/BUILD.md`), then cleaned brew vestiges (error-type rename,
dead `catalogAutoRefresh`, dead error codes, deleted the brew-era Python pipeline), repurposed the empty
inherited "Activity" surface into a **usage journal** (workflow-built + team-reviewed), added a Tools-pane
**Installed/Not-installed/All** lens, and fixed the cold-build `cargo test` tauri feature-gate
(`.cargo/config.toml`). Green: svelte-check 0, cargo 258/0. Pushed on `release-planning` (not cut).
See [260615_pre-release-polish.md](./260615_pre-release-polish.md).

## Next
- Phase 4 — Trending + GitHub.
- Deferred: multi-file tool renderers (antigravity/openclaw/aider/windsurf); local-runtime system-prompt
  target (Ollama/LM Studio).
