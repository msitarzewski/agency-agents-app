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

### 2026-06-16: v0.1.0 SHIPPED — public, signed, Homebrew
First public release: 7 cross-platform artifacts (signed/notarized macOS DMGs, Linux deb/rpm/AppImage, Windows
NSIS), repo made public, `brew tap msitarzewski/agency-agents`. See agentLog 2026-06-16.

### 2026-06-17→20: v0.1.1 — the IA re-org (PRs #15 + #16 + deploy-browser)
Divisions landing, install-state lens, Teams (← Loadouts), the how×where engine, Projects pillar, the single
destinations×tools InstallModal, the two-pane DeployBrowser. Four-pillar model (Agents/Tools/Teams/Projects =
who/how/which/where). See activeContext.md.

### 2026-06-21: v0.1.2 — tool registry + all 13 tools + Osaurus + Playbook + Projects dashboard (PRs #18 + #19)
Made tool knowledge a single source of truth (upstream-owned `tools.json`, twin of `divisions.json`; Rust `Tool`
enum dropped; installability derived from `format ∈ IMPLEMENTED_FORMATS`). All 13 tools + real brand logos;
**Osaurus wired** via a `skill-md` format byte-identical to the upstream transformer (contributed upstream first).
In-app Playbook + `docs/USING-AGENTS.md`; Teams/Projects master-detail; Global-vs-Projects dashboard sunburst.
cargo 264/0, svelte-check 0. See [260621_tool-registry-12-tools-osaurus.md](./260621_tool-registry-12-tools-osaurus.md).

### 2026-06-23: v0.2.0 SHIPPED — first feature release + LIVE auto-update (cross-platform) (PRs #21 + #22)
First release since v0.1.0 (the "0.1.1"/"0.1.2" milestones were never cut separately — they ship here), and
**auto-update is now live** at `agencyagents.app/updater.json` for both Mac arches. 9 assets (macOS aarch64+x64
DMGs + updater tarballs, Linux deb/rpm/AppImage, Windows x64/arm64); Homebrew cask bumped to 0.2.0. Dedicated
agency signing key (`ABF5AFD8`). Shook out two real `release.sh` bugs (`set -u` empty-array + missing `--config`
allowlist merge / tauri#11142), the rustup-vs-Homebrew Intel cross-compile split, and a corrupted updater
Keychain key — all fixed + documented (`BUILD.md` reconciled to the Keychain flow). `main` @ `16182e5`.
See [260623_v0.2.0-ship.md](./260623_v0.2.0-ship.md).

### 2026-06-29: Fix Windows-specific test path mismatches
Fixed path separator mismatches in tests on Windows by comparing Path objects instead of strings.
See [260629_fix-windows-tests.md](./260629_fix-windows-tests.md).

## Next
- **Opt-in automatic install** — wire the inert "Install updates automatically" toggle to a real off-by-default
  background download → verify → install (the live updater currently does check → notify → one-click install).
- Refresh `tools.json` from the catalog clone; Foreign-sweep for nested skill dirs; Antigravity once its skill is
  deterministic (`date_added` dropped); multi-file renderers (Aider/Windsurf/OpenClaw/Kimi).
- Windows code signing; local-runtime system-prompt target (Ollama/LM Studio).
- See the full post-0.2.0 punch list in `docs/PLAN.md`.
