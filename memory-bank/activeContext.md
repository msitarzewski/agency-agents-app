# Active Context — Agency Agents

**State**: 🚀 **v0.2.0 SHIPPED (2026-06-23)** — `main` @ `16182e5`. First feature release since the v0.1.0
launch (the internally-tracked "0.1.1"/"0.1.2" milestones were never cut separately — they ship here), and
**auto-update is now LIVE** at [`agencyagents.app/updater.json`](https://agencyagents.app/updater.json) for
**both Mac arches** (`darwin-aarch64` + `darwin-x86_64`). Release at
[releases/tag/v0.2.0](https://github.com/msitarzewski/agency-agents-app/releases/tag/v0.2.0): **9 assets** (macOS
aarch64+x64 signed/notarized DMGs **+ updater tarballs**, Linux deb/rpm/AppImage, Windows x64/arm64). Homebrew:
`brew tap msitarzewski/agency-agents && brew install --cask agency-agents` (cask @ 0.2.0). Cross-platform CI in
`.github/workflows/` (linux-build, windows-build) fires on `v*` tags; macOS DMGs build locally via
`scripts/release.sh`. Full ship log: `agentLog.md` 2026-06-23; task doc `tasks/2026-06/260623_v0.2.0-ship.md`.

**Workflow (from 2026-06-16):** ALL changes go through a **branch → PR → merge to `main`**. No direct commits to main.

## ✅ v0.2.0 — first feature release + LIVE auto-update — SHIPPED (2026-06-23, PRs #21 + #22; `main` @ 16182e5)
- **Auto-update is on.** Endpoint `agencyagents.app/updater.json` (Caddy on `umacbookpro` from `~/Sites/agency-agents/`,
  sibling vhost to the live `brew-browser.zerologic.com` manifest). **Dedicated agency signing key `ABF5AFD8`**
  (embedded pubkey in `tauri.conf.json`; private key + password in the **macOS Keychain**, services
  `agency-agents-updater-key` / `…-key-pw`; canonical key file backup at `~/.config/agency-agents-app/updater.key`).
  Live path = check → notify → one-click install; full hands-off auto-install is still deferred (the
  "Install updates automatically" toggle ships **present-but-disabled**).
- **Release build gotchas (now fixed + documented in `BUILD.md` / `release.sh` / PR #22):**
  - Updater-on macOS builds **must pass a `--config`** flag — the macos-private-api allowlist check reads only
    base `tauri.conf.json`, so the split `tauri.macos.conf.json` (`macOSPrivateApi:true`) is invisible to it
    (tauri#11142). `release.sh` now always passes `--config '{"app":{"macOSPrivateApi":true}}'`. (Every old
    `SKIP_UPDATER` build worked only because it happened to pass a `--config`.)
  - **Intel cross-compile needs the rustup toolchain** — Homebrew's `rust` is host-only (`can't find crate for
    core`). Build with `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`.
  - **Store the updater Keychain key via `$(cat …)`**, never a manual paste — a trailing newline corrupts it and
    signing fails with `incorrect updater private key password: Invalid input`.
- **Asset naming uses underscores** (`Agency_Agents_0.2.0_*`), NOT v0.1.0's auto-sanitized dots — the live updater
  manifest and the brew cask url both depend on this.
- **Post-ship UX (PR #23):** the Agents pane now has a **"Needs attention" filter** (= Outdated ∪ Modified ∪
  Missing). The install-state lens moved into nav (`ui.agentsLens`) so the Dashboard "N need attention" stat +
  health-donut segments **deep-link** to a **flat all-divisions** filtered list (`showDivisions` gates on
  `lens === "all"`); cross-launch lens persistence dropped (would hijack the landing).
- Decisions: `decisions.md` 2026-06-22 (host + dedicated key) and 2026-06-23 (build mechanics). Gates: cargo 264/0,
  svelte-check 0, signed+notarized, CI green.

## ✅ v0.1.2 — Tool registry + Osaurus + Playbook + Projects dashboard — SHIPPED (2026-06-21, PRs #18 + #19; `main` @ 1df932c)
**Tool knowledge is now a single source of truth.** It consolidated to the single canonical `tools.json` the
upstream `agency-agents` repo OWNS (twin of `divisions.json`, CI-guarded by its no-jq `check-tools.sh`); both the
Rust backend (`registry.rs`, `include_str!`) and the frontend (`toolRegistry.ts`) read it. **The Rust `Tool` enum
is GONE** — a tool is a string id; `label`/`detect`/`version`/`dests`/`scope` are registry lookups; `render()`
dispatches on the JSON `format` key. Frontend deleted ACCENTS/ICONS_SVG/SHORT/hardcoded SUPPORTED_TOOLS.
**Adding a tool = editing one JSON file** (+ a Rust formatter only for a brand-new output format).
- **All 13 tools** modeled (incl. Kimi, Osaurus); Tools panel shows installable + recognized-only (dimmed). Real
  brand logos (Lobe Icons, MIT) under `assets/tools/`, letter fallback otherwise.
- **Installability derived, not stored:** `installable(tool) = format ∈ IMPLEMENTED_FORMATS` ({identity,
  codex-toml, gemini-md, qwen-md, cursor-mdc, opencode-md, skill-md}). aa owns upstream truth; renderer coverage
  is app-side + self-maintaining (ship a renderer → add its format → those tools light up).
- **Osaurus wired** via a `skill-md` format (Agent-Skills `SKILL.md`, `slugPrefix:"agency-"`), byte-identical to
  upstream `convert_osaurus` — contributed UPSTREAM first (catalog owns transforms), mirrored here (parity test).
  Verified live: catalog agents run as native Osaurus skills.
- **Playbook** (in-app practices + copyable starter prompts + per-team/division examples; title-bar 📖 + ⌘K) +
  `docs/USING-AGENTS.md`. **Teams & Projects master/detail** via the system back arrow
  (`ui.projectsSelected`/`teamsSelected`); **division overview** + deploy.
- **Dashboard**: two-ring Global-vs-Projects install **sunburst** (`InstallSunburst.svelte`) so totals reconcile;
  cross-tool coverage **merged** with catalog-by-division (linked hover); uniform donuts, equal-height cards.

Task doc: [tasks/2026-06/260621_tool-registry-12-tools-osaurus.md](tasks/2026-06/260621_tool-registry-12-tools-osaurus.md).
Green: cargo 264/0, svelte-check 0, build clean. Upstream `agency-agents` (same machine): Osaurus transformer +
`tools.json` + `check-tools.sh` + `check-tools.yml` landed (aa PRs #605/#606).

## ✅ v0.1.1 IA arc — SHIPPED (2026-06-17 → 06-20, PRs #15 + #16, + the deploy-browser PR)
The whole "how people think about agents" reorganization landed:
- **Divisions landing** — the Agents tab opens on divisions; select-mode → bulk deploy.
- **Install-state lens** — filter the agent list by deployment state (In sync / Outdated / Untracked / Missing /
  Not installed), counts scoped to the division.
- **Teams** (renamed from Loadouts) — "Your team" (current installs, division-grouped) + "Team presets"
  (app-bundled `presetTeams.ts` + your saved teams, `teams.svelte.ts`).
- **how × where engine** (backend, PR #15) — tools are dual-scope; `render::dests()` scope-aware;
  `supports_user()`/`supports_project()`; install scope derived from the chosen project. Verified tool-path
  matrix (June 2026) in the PR. Cursor is project-only; Windsurf/Aider/Antigravity/openclaw deferred.
- **Projects pillar** (4th nav section, ⌘4; Activity → ⌘5) — `projects.svelte.ts` store (registered roots in
  localStorage ∪ the live ledger), `Projects.svelte` panel with rosters.
- **One `InstallModal`** — the destinations × tools GRID (rows = Global + each project + "Add project…",
  columns = detected tools, cells = tri-state toggles). Reused by agent detail, divisions, Teams, Projects.
  Replaced `DeployModal` + the inline switch-matrix. Agent detail: "Install…" in the title, pills on their own row.
- **Two-pane `DeployBrowser.svelte`** (Projects "Deploy…") — System-Settings master/detail: left =
  searchable list of EVERY granularity (agents · divisions · teams incl. saved · current roster); right =
  per-project per-tool install.

**Four-pillar model (drives IA copy):** Agents = *who* · Tools = *how* · Teams = *which* · Projects = *where*.

## 🔵 Backlog (next — full list in `docs/PLAN.md` post-0.2.0 punch list)
- **Opt-in automatic install** — the v0.2.0 "Install updates automatically" toggle is inert; wire it to a real
  off-by-default background download → verify → install (live updater is check → notify → one-click install today).
- **Refresh `tools.json` from the catalog clone** (vs. the bundled baseline) — like the corpus + `divisions.json`;
  cleaner now that aa #605 prunes stale convert output.
- **Foreign-sweep for nested skill dirs** (`…/<dir>/SKILL.md`) so CLI-installed Osaurus/Antigravity skills are
  detected (app-installed ones already are).
- **Antigravity wiring** once upstream makes its skill deterministic (drops the non-deterministic `date_added`).
- **"Auto Updates" subscription** for bulk installs — installing all of a division/team into a project/tool
  offers to auto-deploy newly-added catalog agents.
- **Copilot `.md` → `.agent.md`** (needs reconcile `file_stem` double-extension handling).
- Optionally tighten backend `detect()` to require the tool **binary**, not just a lingering config dir.
- Bonus: a "scaffold AGENT-ZERO into a project" action (every assistant honors repo-root `AGENTS.md`).

**Dev-harness note:** to screenshot-verify the Svelte frontend in a browser (the native Tauri window can't be
auto-driven), a `?shim=1` Tauri-IPC shim is temporarily injected into `src/app.html` then reverted — it never
ships. The shim can't open a real native folder dialog (returns a fixture path), so "Add project…" looks broken
in the shim though it works natively.

**Last updated**: 2026-06-23

## ✅ Pre-release polish (2026-06-15) — committed + pushed on `release-planning`
- **brew vestige cleanup**: error-type rename (`BrewError*`→`AppError*`), removed dead `catalogAutoRefresh`
  setting, removed the dead error codes (`brew_*`, `job_not_found`, `canceled`, `feature_disabled`,
  `vulns_not_installed`), and **deleted the brew-era Python pipeline** (`tools/{catalog,categorize,enrich,
  pipeline,trending-collector}` — they fetched Homebrew formulae, NOT used by AA; the catalog comes from
  `corpus/mod.rs`).
- **Activity Journal** (replaces the inherited, permanently-empty brew streaming "Activity"): pivoted
  `activity.svelte.ts` to a `JournalEntry` store (localStorage), `install.svelte.ts` logs every
  install/uninstall/update/track/bulk + default-target switch, `ActivityHistory.svelte` rewritten as a
  day-grouped clearable journal. Deleted `ActivityDrawer.svelte` + `AppStreamEvent`/`ActivityJob` types.
  Built via a Workflow (planner→builder→Code-Reviewer+UX-Architect team→fix loop); UX nits hand-polished.
- **Tools pane lens**: defaults to **Installed** (detected/in-use) tools; toggle `Installed · Not installed
  · All` (top row beside rescan, no count chips). `ToolsView.svelte`. Bar = **catalog coverage**
  (green installed / gray rest), not sync-state.
- **Agents workspace streamlined**: removed the filter lens (per-row install dots already show count);
  Division dropdown moved onto the search row as the first element (neutral form styling); detail pane
  hidden when no agent is selected (list goes full-width).
- **Cold `cargo test` tauri-gate fix**: `.cargo/config.toml` feeds `TAURI_CONFIG` so bare cargo (tests/CI)
  passes the `macos-private-api` allowlist gate (Tauri CLI overrides it for real builds). `macos-private-api`
  enabled in `Cargo.toml`. Verified `tauri dev` still launches clean.
- **Cross-platform creds FIXED + VM-validated**: GitHub token now persists to the OS-native vault per
  platform (Keychain / Credential Manager / Secret Service) via per-target `keyring` features; also moved
  `macos-private-api` to `[target.macos]` only (was wrongly in base deps → broke the Linux gate). Built +
  tested on Ubuntu (258/0 + deb/rpm/appimage) and Windows x64 via `phase-c.sh` VM matrix.
- **Dead-code/brew pass**: removed dead `agentsFilter` lens plumbing; scrubbed ALL brew comment mentions
  (grep → none); zero cargo dead_code warnings.
- **UX**: adaptive Uninstall/Delete wording by ownership; OS-style click-outside menu dismiss; Tools detail
  closes when the lens hides the tool; CoverageMatrix shades by **coverage-%** (not raw size).
- **Terminology**: user-facing **Category → Division** (catalog repo's term); internal `category` field kept.
- **Dashboard viz DONE**: replaced the cross-tool matrix with **CoverageDonuts** (one donut per tool,
  sliced by division, shared legend, linked hover); established a curated **division color scheme** as catalog
  metadata (PR github.com/msitarzewski/agency-agents/pull/592 = `divisions.json`) read via `corpus.colorOf`;
  Dashboard "Coverage by tool" click now selects the tool (`ui.openTools`). **`CatalogByDivision.svelte`** (NEW)
  replaces the orange bar-list: ONE proportional bar (segment per division, brand-colored), labels across FOUR
  lanes (2 top, 2 bottom) tied to segments by **non-crossing Z-elbow leaders** (rank-staggered rails +
  phase-shifted bottom columns), plus CoverageDonuts-style **linked hover** (dim others). Division **icons
  tinted** with their color in the `Division ▾` dropdown + persona pill (added `corpus.iconOf`); `categoryIcon.ts`
  gained `Map`+`Workflow` so gis/integrations stop falling back to "?". See `agentLog.md` 2026-06-15 (later 4).
- **Green throughout**: svelte-check 0 errors, cargo 258/0 (macOS + Linux), config validation all-pass.

## ✅ Phase C (2026-06-14) — both red items closed
- **Renderer parity VERIFIED.** `render/mod.rs` mirrors the upstream shell converter byte-for-byte
  (`source_field`/`source_body`/`slugify`/`output_slug`); new `--ignored` test diffs the real
  `scripts/convert.sh` → **232 agents × 5 transform tools = 1160/1160 byte-identical**. The
  `current`/Diff/Update model is now proven, not assumed.
- **Uninstall safety RESOLVED.** `remove_agent_files` backs up modified files FIRST (separate pass),
  byte-identical files need no backup, backup failure aborts the delete (original preserved). Tests cover
  every path.
- **Cross-platform chrome DONE.** Config split: base `tauri.conf.json` (decorations, opaque, no
  macOS-only keys) + `tauri.macos.conf.json` override (overlay titlebar/traffic-light/transparency).
- **Cleanup:** brew→Agency rename finished in `lib.rs`; dead `Settings` fields purged; docs overhauled;
  stale release notes removed; new `tools/phase-c/` validation runner. **Catalog now = 232 agents**
  (the re-org landed). Green: cargo 258/0 + parity 1/0, svelte-check 0, build clean.

## 🟣 Tahoe app icon (read first if touching icons)
macOS 26 renders icons from a compiled **`Assets.car`** (Icon Composer Liquid Glass), NOT `.icns` — `.icns`
-only = blank/gray squircle ("icon jail"). FIXED: `actool` (full Xcode only, by path) compiles
`docs/icon/AppIcon.icon` → `src-tauri/Assets.car` (in `bundle.resources`) + Tahoe-aware
`src-tauri/icons/icon.icns`; `src-tauri/Info.plist` adds `CFBundleIconName=AppIcon` (Tauri merges it).
**Don't run `npm run tauri icon`** (clobbers the glass icns). Full recipe: `docs/icon/README-liquid-glass.md`.
Dev Dock hack REMOVED (lib.rs plain `.run()`, objc2 deps dropped).


## Current state (read NEXT-SESSION.md for the full picture + IMMEDIATE backlog)
- **Phase B + nav + Tools (2026-06-09):** Dashboard has 4 dependency-free charts (`HealthDonut`,
  `CoverageMatrix` category×tool, coverage-by-tool bars, category distribution). **Back/forward nav**
  (titlebar ◀▶, ⌘[/], mouse 3/4) over a `ui` NavLocation history; `agentsCategory`+`agentsSelected`
  lifted into `ui`. **Division pills deep-link** everywhere (`ui.openDivision`); lens counts narrow to
  the division; added "Not installed" lens; zero-count lenses/stats hide. **Tools = list/detail console**
  (`ToolsView` rebuilt): badges (`util/toolBadge`), health bars, versions (`tool_versions`), Reveal
  (`reveal_path`), Default-target Switch, Sync-to-catalog/Track-all/Remove-all, projects list. Dev Dock
  icon set on `RunEvent::Ready` (macOS debug). Icon redrawn as a **macOS squircle** (regenerated).
- **UNIFIED Agents workspace (Phase A done).** Agents + Library are ONE three-pane surface
  (`AgentsWorkspace.svelte`): list pane (filter lens All/Installed/Needs-attention/Untracked + search +
  Category ▾ + Select-mode bulk) · `ResizeHandle` · persistent detail pane (`PersonaBody` + the
  `DeploymentMatrix`). `PersonaDiscover.svelte` + `AgentLibrary.svelte` DELETED.
- **Deployment band under the name/division**: summary pills for installed tools + a "USE WITH ⌄"
  disclosure. User tools = `Switch` (on=installed); project tools = Install/Add-project + per-project
  sub-rows. Drift actions (Diff/Track/Update) inline when applicable. New `Switch.svelte` (shared,
  extracted from Settings→Network), `util/platform.ts` (⌘/Ctrl shortcut glyphs).
- Nav: `library` section retired everywhere; `ui.agentsFilter` + `ui.openAgents(filter)` deep-link
  (Dashboard cards + palette use it). Section id stayed `personas`.
- **Byte-identical foreign → `current`**; **recursive indexing**; `agent_diff` + `DiffModal`; Track (safe).
- Active catalog = **userClone** `/Users/michael/Software/AgentLand/agency-agents` (manage:true).
- **Signed + notarized `.app`/`.dmg`** via `scripts/release.sh` (SKIP_UPDATER=1). 247 Rust tests / 0.
- 🔵 NEXT: **Phase B** = 4 Dashboard charts (coverage matrix · health donut · category distribution ·
  per-tool coverage), dependency-free SVG/CSS, cells deep-link into the workspace. Then **Phase C** =
  Windows/Linux titlebar degradation + "this device" copy + home-path display.
- ✅ CLOSED 2026-06-14: (1) **renderer parity** vs convert.sh — VERIFIED 1160/1160 byte-identical;
  (2) **uninstall safety** — RESOLVED (backup-first for modified, none for byte-identical, abort-on-fail).

## (historical) Earlier this arc
- **Adopt → Track**: destructive Adopt gone. `track_agent` records provenance, writes nothing; every
  write backs up first (`<app_data>/backups/`); `agent_diff` for review-before-Update.
- **categories from tooling**: `discover_categories` parses `AGENT_DIRS` from
  `scripts/convert.sh`. **Data fix: `integrations` (convert.sh output) dropped (210→209); `strategy`
  added.** Removed the orphan `integrations/backend-architect-with-memory.md` from the baseline (it's
  a valid-but-misfiled enrichment example; to ship it for real, promote it UPSTREAM into a real
  category — then it flows in via refresh).
- **#1 slices 2–4 — catalog source**: `CatalogSource` (Bundled | Managed{~/.agency-agents} |
  UserClone{path,manage}) in `state/catalog.json`; corpus reads/writes the RESOLVED root. Detect
  (~/.agency-agents + "Find" scan), provision (git clone or snapshot), pull (git pull or tarball).
  First-run picker (`CatalogFirstRun`) + `Settings → Catalog`. Verbs Track/Update, manage-with-
  permission, picker+Find — all as decided. cargo test 275/0; svelte-check 0 err; build green.
- ⚠️ NOTE: existing installs (incl. Michael's) have no catalog.json → the **first-run picker WILL
  appear** on next launch (by design — one-time source choice; pick "Bundled" to keep current).

> Full plan + sequence: `phases/phase-roadmap.md` (the "v2" block). Detailed resume notes +
> gotchas: `NEXT-SESSION.md`. Build spec: `contracts.md`. Architecture: `systemPatterns.md`.

## How to run (dev)
- `npm run tauri dev` from repo root. **Dev server is on port 1430** (NOT 1420 — that's
  brew-browser; sharing it makes one app load the other's frontend). HMR for frontend; Rust changes
  recompile. The app opens on **Agents** (personas).
- Reference clones (read-only): `/tmp/brew-browser-inspect`, `/tmp/agency-agents-inspect`.

## What works (verified)
- **Agents** catalog (210 agents / 16 categories), search, persona detail with an **Install** menu.
- **Library** — flat list of installs; your ~184 `install.sh` agents show as `foreign` with Adopt.
- **Tools**, **Loadouts** (Agentfile), **Dashboard** (agency rollup), Activity, Settings (⌘,).
- Backend: `corpus · render · install · github · util · commands{github,settings,updater}`.
  `cargo test` ~267/0; `vite build` + `svelte-check` green; app boots clean (210 corpus seeded).
- New brain-circuit **app icon** (dark shipped; light master in `docs/icon/`). About window rebranded.

## Immediate next: Michael runs it, then #2 / #3
**#1 slices 1–4 ✅ done.** Remaining for #1 (deferred refinements, non-blocking):
- `aliases.json` (slug renames across catalog versions) — not yet honored.
- Explicit **orphan** surfacing (ledger rows whose slug left the catalog) + unique-slug enforcement.
- `.agency-cache/` convention + add to the agency-agents repo `.gitignore` (cache not yet written).
- Symlink-aware reconcile (the `~/.claude` alias case) — still the old behavior.

Then: **#2 Track-all / Update-all**; **#3 tool-grouped Library IA** (L1 tools+counts → L2 per-tool)
+ wire `agent_diff` into a review-before-Update UI.

## Decisions locked (this session)
- Build order: **Both, Track first** → Track DONE, now #1.
- Clone detection: **picker-primary + a "Find Agency Agents" button** (opt-in scan, not auto).
- Existing clone: **manage-with-permission**. Managed path: **`~/.agency-agents`**.
- Cache dir: `.agency-cache/`. Verbs: **Track / Update**.
- Categories: **parse from repo tooling** (`AGENT_DIRS` in convert.sh), not a frontmatter heuristic.

## ✅ RESOLVED: "Adopt" is no longer unsafe
Adopt → **Track** (non-destructive) + backup-on-write shipped this session. The old clobber path is
gone.
