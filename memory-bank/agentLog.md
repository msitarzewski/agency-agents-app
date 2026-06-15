# Agent Log — Agency Agents (append-only)

2026-06-05 — Phase 0 (lead): recon of agency-agents (107.5K★, 251 agents/18 cats) +
brew-browser (Tauri2/Svelte5/Rust, 73 cmds). Locked 1:1 architecture. Forked scaffold,
rebranded to Agency Agents. Green cargo check + vite build. Wrote memory-bank.
Next: Phase 1 team build (Corpus + Discover).

2026-06-05 — Phase 1 (team wh3t9mkee + lead-as-integrator): Types agent landed types.rs+types.ts
(+5 wire-format tests). Corpus agent landed corpus/mod.rs+parse.rs (frontmatter parse, sha256
split-hash index, corpus_* commands, tarball refresh) + bundled baseline + Cargo deps
(serde_yaml/sha2/hex/flate2/tar/tracing) + tauri.conf resources. Discover agent landed
corpus.svelte.ts store + PersonaDiscover.svelte + nav wiring. Workflow threw on a StructuredOutput
protocol error AFTER the code compiled, so the Integrate phase didn't run — lead took over as
integrator: fixed rebrand keychain id (brew-browser→agency-agents-app) + its 2 tests, decoupled
corpus category metadata onto its own data/agency-categories.json (was colliding with brew's
categories.json), restored brew's real bundled data (placeholders had broken brew's tests), added a
real-baseline parse test. THAT test caught the big one: seeding undercounted (flat layout missed
nested game-dev/strategy agents) AND the true agent count is 210, not 251 — strategy/ (NEXUS
playbooks) + examples/ (workflow docs) have no agent frontmatter, so they're documentation, not
agents. Corrected CATEGORY_DIRS 18→16, baseline re-seeded recursively to 210 agents / 16 categories.
Verdict: cargo check GREEN, vite build GREEN, cargo test 615 passed / 0 failed. Phase 1 DONE.
Next: Phase 2 (Install + Reconcile).

2026-06-05 — Phase 1.5 (lead): agency-first polish. Added 12 Lucide category icons (killed "?"
fallbacks); rewrote Sidebar lean (🤖 brand + Agents/Activity nav, dropped brew package-search +
brew env footer, added live agent-count footer); default landing dashboard→personas; enabled
macOSPrivateApi vibrancy. Verified live (window-only screenshot). 615 tests stay green.

2026-06-05 — Phase 2 (lead, drove solo): Install + Reconcile.
- 2a render/: deterministic per-tool converters ported from convert.sh — claude-code/copilot
  (identity), cursor(.mdc), codex(TOML+escape), gemini-cli, qwen, opencode(color→hex). dests() per
  tool/scope; Copilot dual-write; 4 unsupported tools (antigravity/openclaw/aider/windsurf) error
  cleanly. 10 tests.
- 2b install/: ledger (installs.json), reconcile classify() → 5 states, tools detection, projects
  registry. Commands: install/update/adopt/uninstall_agent, installs_reconcile, installs_for_agent,
  tools_list, projects_list. Exposed corpus helpers (entry/version/read_source, pub(crate) paths).
  End-to-end tests (write→disk→reconcile through all 5 states; identity verbatim; project-root write).
- 2c UI: install store (install.svelte.ts), persona-detail Install menu (user tools 1-click;
  project tools folder-pick via dialog), installed-state chips with status + uninstall, toasts.
Verdict: cargo test 630 passed / 0 failed; vite GREEN; app launches healthy (210 agents, no panic).
Remaining for Phase 2: dedicated Library + Tools nav sections (install flow itself is done in the
persona panel). Next: Library/Tools views, then Phase 3 (Loadouts + Dashboard).

2026-06-05 — Phase 2 follow-ups (lead): (1) update_kind — added body_hash to InstallRecord
(#[serde(default)] for back-compat), reconcile now labels Outdated installs Cosmetic vs Substantive.
(2) Foreign sweep — installs_reconcile scans each tool's dir(s) (user + ledger project dirs) for
recognized-but-unledgered corpus agents → Foreign state (Adopt-able). (3) Library view
(AgentLibrary.svelte) — reconciled cross-tool installs, attention-first, status chips, per-state
actions (Update/Reinstall/Adopt/Remove). (4) Tools view (ToolsView.svelte) — detected tools + scope
+ counts + dest. Wired both into sidebar nav (Agents/Library/Tools/Activity) + fixed ⌘1/2/3/6 keymap
to match. Library badge shows attention count. cargo test 630 / 0; vite GREEN. Deferred (noted in
install/mod.rs): the 4 multi-file renderers (antigravity/openclaw/aider/windsurf). Next: Phase 3.
NB: real-world validation — Michael ran the repo's install.sh (184 agents → ~/.claude/agents); the
Foreign sweep correctly surfaced ~180 as Adopt-able in the Library. Zero fake data.

2026-06-05 — Phase 3 (lead): Loadouts + Dashboard.
- Loadouts: loadout_export/import commands (Agentfile JSON manifest; export reads ledger, import
  installs each entry, skips failures). Loadouts.svelte (Export/Restore via dialog save/open +
  current-loadout list). agentfile_roundtrips test.
- Dashboard: AgencyDashboard.svelte rollup (available / installed-by-you / need-attention /
  found-to-adopt stat tiles → deep-link to Library; catalog-by-category bars; detected-tools strip).
  Replaced brew Dashboard in routing.
- Nav now full agency: Dashboard/Agents/Library/Tools/Loadouts/Activity (⌘0-4,6 keymap aligned).
- cargo test 631 / 0; vite GREEN. Phase 3 DONE. Next: Phase 4 (Trending + GitHub).

2026-06-05 — Bugfix (lead): "Library panel broken" + Tools rescan.
- ROOT CAUSE (proved via temp DBG readout): components called `install.reconcile()` / `loadTools()`
  at SCRIPT TOP LEVEL, mutating install-store $state (reconciling/installed) DURING the component's
  own setup/derivation → froze that component's reactivity. Backend was fine (reconcile logged
  "DONE: 190 rows"); the Sidebar (read-only) updated to 190 while AgentLibrary stayed frozen at
  installed=0/reconciling=true → header/body/badge disagreed.
- FIX: moved all reconcile()/loadTools()/ensureLoaded() side-effects into onMount() in AgentLibrary,
  ToolsView, Loadouts, AgencyDashboard, PersonaDiscover. Replaced the store's class #private
  in-flight guard with a module-level promise (class #private fields can trip Svelte 5's $state
  transform). Added reconciling/reconciled flags + a real LoadingState in the Library; emoji lookup
  via a Map (was O(n) per row × 180+). Null-safe row sort. Dashboard <li role=button> → <button>.
- ADDED: Tools view "Rescan" button (re-detect tools + re-reconcile) — the requested Tool Update.
- cargo test 631/0; vite GREEN. NOTE: couldn't visually confirm in-session — screen capture is
  blocked (terminal holds focus; transparent webview blanks on occluded window-id capture). Needs
  Michael to confirm the Library populates (it should now show the ~190 Foreign agents).

2026-06-06 — Library "frozen" bug RESOLVED (lead, via Michael's devtools console). ROOT CAUSE was
NOT reactivity (hours lost chasing that): a Svelte `each_key_duplicate` crash. The `{#each rows}` key
was `slug+tool+projectPath`; **Copilot dual-writes to ~/.github AND ~/.copilot**, so the Foreign
sweep produced TWO identical-key rows → Svelte threw during render → AgentLibrary's whole reactive
subtree halted and froze on the last good frame (loading skeleton), reverting all reads to 0 → which
LOOKED exactly like a reactivity freeze. LESSON: for a frontend frozen/blank bug, open the webview
devtools console FIRST (right-click → Inspect in Tauri dev) — it named the cause in one line. FIX:
(1) `{#each rows as r (r.dest)}` (unique per file); (2) backend `installs_reconcile` dedupes by
(slug, tool, project) so Copilot is one logical row. The single-`$effect` snapshot refactor was kept
(good hygiene) but was NOT the fix. Library now renders 185 installs / 184 foreign with Adopt
buttons. Debug tags removed; default landing restored to Agents. Next: tool-grouped Library IA +
Track/Update safety + clone-as-source-of-truth.

2026-06-06 — Brew-domain sweep DONE (delegated to a focused agent w/ exact keep/remove spec, lead
verified). Removed the whole inherited brew domain: backend dirs brew/catalog/enrichment/trending/
vulns + 16 brew command files + brew state fields/init + brew data + brew types; frontend brew
components (Dashboard/Library/Discover/Snapshots/Services/PackageDetail/Row/Trending/Upgrade/Issue)
+ 4 brew Settings sections + ~14 brew stores + brew api.ts/types.ts + +page brew branches +
CommandPalette/ui trims. KEPT (forward infra, not-yet-wired): Settings shell + generic sections,
full design system, updater, github/ (→P4), util, the agency domain. Smart relocations: SSRF guard
`is_public_host` → util/net.rs (used by github/url.rs); `app_version` → commands/settings.rs.
Verified: backend = corpus/render/install/github/util/commands{github,settings,updater}; cargo test
265/0; vite + svelte-check GREEN; app boots clean (210 corpus seeded, no panic). Also fixed
EmptyState to accept `children` (agency empty states now render their copy). Second-pass polish
deferred (not blocking): rename BrewError→AppError; agency-ify Settings Network/CaskIcon fields;
Pill formula/cask tones; brew strings in tokens.css/comments. Next: #1 clone-as-source-of-truth.

2026-06-06 — Adopt→Track SAFETY FIX + #1 slice 1 (dynamic categories) (lead).
- TRACK (was Adopt): `install/mod.rs` — `track_agent`/`do_track` records provenance in the
  ledger but WRITES NOTHING (was do_install = re-render + overwrite). Reconcile then shows the
  tracked file as Current (matches catalog) or Modified (differs) — never clobbered. All writes now
  back up first: `write_agent_files` gained `backup_dir`; `backup_if_differs` copies prior bytes to
  `<app_data>/backups/<file>.<stamp>.bak` (outside tool dirs → invisible to the Foreign sweep) before
  any overwrite, so install/update/restore/import are reversible. New `agent_diff` cmd (+ store
  `diff()`) returns {onDisk,proposed,differs} for review-before-Update. UI: Foreign "Adopt"→"Track";
  modified row now "Restore" via backup-aware update; Dashboard "found to adopt"→"found to track".
  Tests: track_writes_no_file, write_backs_up_existing_differing_file. lib.rs swaps adopt_agent→
  track_agent + agent_diff.
- #1 SLICE 1 — categories now parsed from the REPO TOOLING, not hardcoded (Michael: "categories can
  be parsed from the repo itself, look in the tooling"). `discover_categories(root)` parses the
  `AGENT_DIRS=( … )` bash array from `<root>/scripts/convert.sh` (→ install.sh → lint-agents.sh),
  `parse_agent_dirs` is pure/tested; `DEFAULT_CATEGORIES` is the offline fallback. Threaded through
  resolve_active/build_from_dir/seed_from_baseline/empty_corpus + Corpus.category_order; refresh()
  reads categories from the tarball's own convert.sh (`categories_from_tarball`) and extract writes
  scripts/convert.sh into the working copy so launches stay self-describing. Bundled
  `scripts/convert.sh` into corpus-baseline (ships via existing resource glob).
  DATA CORRECTION: `integrations/` is convert.sh OUTPUT, not a category → dropped (our list wrongly
  had it); `strategy/` IS canonical → added. Net: baseline 210→**209** (the lone integrations file
  backend-architect-with-memory.md is an enrichment artifact, no longer indexed; file still on disk,
  flagged for possible removal). agency-categories.json: −integrations +strategy(Network icon);
  categoryIcon.ts −Plug +Network. Existing installs auto-correct via the DEFAULT fallback (their old
  seeded corpus has no scripts/). Tests updated incl. real-baseline→209, +4 new. Verdict: cargo test
  271/0; svelte-check 0 err; build GREEN. Next: #1 slices 2–4 (CatalogSource model → detect/provision/
  pull → first-run modal + Settings). Managed path = ~/.agency-agents (confirmed).

2026-06-06 — #1 slices 2–4: clone-as-source-of-truth (lead). Catalog now has a SOURCE.
- SLICE 2 — CatalogSource model: `types.rs` enum {Bundled | Managed{path} | UserClone{path,manage}}
  (serde tag "kind"), persisted to `state/catalog.json`. corpus/mod.rs: load/save_catalog_source +
  `catalog_root(app_data,source)` resolver. Refactored resolve_active/read_source/refresh to read the
  RESOLVED root (was hardcoded `<app_data>/corpus`); only Bundled seeds from baseline; refresh refuses
  a read-only UserClone. Cmds: catalog_source_get/set (set validates path + looks_like_catalog, then
  rebuilds+swaps the memoized corpus), catalog_configured (catalog.json exists? → first-run gate).
- SLICE 3 — detect/provision/pull: `detect_catalogs(scan)` always checks ~/.agency-agents, scan=true
  walks ~/{Software,Projects,git,Developer,code,dev,src} for an agency-agents checkout; returns
  CatalogCandidate{path,kind,has_git,agent_count}. `provision_managed` = git clone --depth1 (if git on
  PATH) else snapshot tarball into ~/.agency-agents. `pull_active` = git pull --ff-only (git checkout)
  else tarball refresh. git via spawn_blocking(std::process). Cmds: catalog_detect,
  catalog_provision_managed, catalog_pull (all require_network where they hit the net). Factored
  download_corpus_tarball + rebuild_corpus helpers.
- SLICE 4 — frontend: `stores/catalog.svelte.ts` (source/configured/detection/busy + load/detect/
  setSource/useBundled/useClone/provisionManaged/pull, calls corpus.reload() after switches; added
  corpus.reload()). `CatalogFirstRun.svelte` (first-launch picker: use my clone[Find+folder picker,
  manage checkbox] / set up ~/.agency-agents / bundled) rendered from +layout when !configured.
  `SettingsSectionCatalog.svelte` + Settings nav "Catalog" (Library icon) + SettingsSection type:
  shows source/path/agent-count, Pull latest, switch source, Find/pick clone. +layout loads
  catalog on mount.
  Decisions honored: picker-primary + "Find Agency Agents" button; manage-with-permission;
  managed path ~/.agency-agents; verbs Track/Update. Verdict: cargo test 275/0; svelte-check 0 err;
  build GREEN. NOT yet visually run by Michael. Deferred refinements: aliases.json (renames),
  explicit orphan surfacing, symlink-aware reconcile (those were #2/#3 in the old plan; core source
  switching works). Next: Michael runs it; then #2 Track-all/Update-all, #3 tool-grouped Library IA.

2026-06-06 — Warning purge + AppError rename + Catalog GitHub/sync (lead). Michael flagged
dead-code warnings on `tauri dev` + asked for repo pull/sync management with GitHub login + diff stats.
- CLEANUP (0 warnings now): renamed `BrewError`→`AppError` everywhere (412 refs, sed; Rust-only, wire
  `code` strings unchanged). Pruned dead brew variants (BrewNotFound/BrewExitNonZero/JobNotFound/
  Canceled/BrewfileNotFound/FeatureDisabled/VulnsNotInstalled) + truncate_head/tail + ~30 dead tests;
  removed AppState require_enhanced_trending/live_enrichment/vulnerability_scanning + their tests;
  removed render Tool::supported; removed github resolve_github_homepage (brew-metadata helper) + tests.
  Kept extract_github_repo (now wired). cargo build 0 warnings; cargo test 245/0.
- REAL BUG FIXED: `state.rs::resolve_app_data_dir` pushed "brew-browser" → settings.json + github-cache
  were written to `~/Library/Application Support/brew-browser/` (colliding with the other app), split
  from corpus/ledger/catalog which use the proper bundle dir. Changed to
  "com.zerologic.agency-agents-app" so ALL app data unifies. (Existing users: settings/github-cache
  effectively reset to defaults — acceptable.)
- FEATURE — Catalog GitHub + sync status: backend `catalog_status` (source, git commit/branch/last-
  commit/dirty, remote→repo_slug via extract_github_repo, version/fetchedAt, agentCount) +
  `catalog_check_updates` (git fetch → behind/ahead via rev-list --left-right, `git diff --stat`
  preview + changedFiles). provision_managed now FULL clone (dropped --depth1) for accurate history.
  Frontend: catalog store gains status/updateCheck/loadStatus/checkUpdates (refreshed after pull/
  switch/provision); SettingsSectionCatalog rebuilt to show commit/branch/dirty, "Check for updates"
  → diffstat + Pull CTA, and a GitHub card (repo stars/forks/issues/last-release via the EXISTING
  github.getRepoStats; sign-in via github.signIn → global DeviceFlowModal). Reused all existing github
  infra (auth/device-flow/stats) — no new auth code. svelte-check 0 err; build green.
  Next: Michael runs it (first-run picker appears once); deferred #1 bits (aliases/orphans/.agency-cache/
  symlink reconcile); then #2/#3.

2026-06-06 — Settings panel refocus to Agency Agents' lens (lead). Michael: "we're nowhere near
close :9 look at the setting panel." The whole Settings panel was still a Homebrew control panel.
Refocused every section to show ONLY what's functional + real for Agency Agents:
- NETWORK → "Network & Privacy": removed dead brew controls (Cask icon fetching, Trending TTL,
  Catalog auto-refresh, stale-banner — all non-functional after the brew sweep). Kept the one
  functional toggle (Offline Mode / paranoid_mode, 19 live refs). Rewrote the outbound-paths
  disclosure to the REAL hosts: github.com/codeload (catalog clone/pull/snapshot), api.github.com
  (stats/auth/star), raw/objects.githubusercontent.com (avatars/assets), agency-agents-app.zerologic.com
  (updates). Kept Updates subsection.
- APPEARANCE: removed the brew "AI features" enrichment toggle (aiFeaturesEnabled — 0 live agency
  consumers; gated brew enriched metadata) + its CSS; fixed "launch brew-browser"→"Agency Agents".
- GITHUB: reframed "stats on package pages"→"repository stats" for the catalog repo; de-brewed copy.
- ABOUT: rebranded repo links (app + catalog) + zero-telemetry copy to Agency Agents.
- UPDATES: brew-browser→Agency Agents throughout; updater URL brew-browser.zerologic.com→
  agency-agents-app.zerologic.com/updater.json.
- Visible strings elsewhere: reportIssue (issue URL → agency-agents-app, "Report to Agency Agents"),
  ui.svelte storage key prefix, UpdateIndicator badge, ActivityDrawer empty-state + report button,
  TitlebarControls donate label, types.ts appError toast messages (no more "Homebrew"/"brew exited").
  ui localStorage key brew-browser→agency-agents.
Verified: svelte-check 0 errors (was 3 brew-CSS warnings → now only the pre-existing tsconfig `node`
note), build green. STILL BREW-NAMED PLUMBING (invisible to user, deferred): types.ts BrewErrorPayload/
isBrewError/BrewStreamEvent type names + dead codes; reportIssue shell-exit semantics (command/exitCode/
stderr) that agency never produces; dead frontend Settings fields (caskIconMode etc.) + backend Settings
struct fields. These are a bounded "frontend brew-plumbing purge" — next.

2026-06-06 — App icon + About rebrand (lead). Built a new app icon from Michael's brain-circuit.svg
(purple→blue→cyan glyph) composited onto a macOS-style rounded square (rsvg-convert glyph → magick
rounded-gradient bg + composite): dark + light 1024 masters in docs/icon/agency-icon-{dark,light}-
1024.png. Generated the full Tauri icon set via `npm run tauri icon docs/icon/agency-icon-dark-1024
.png` (dark = shipped). GOTCHA: incremental `tauri dev` did NOT re-embed the new icns — had to
`touch src-tauri/build.rs` to force tauri-build to re-run, then rebuild + `killall Dock`; then the
Dock showed the new icon. Rebranded AboutModal.svelte: new icon (src/lib/assets/app-icon.png 256px,
drop-shadow), title "Agency Agents", tagline "A native macOS app store for AI agents", repo/license
→ agency-agents-app, credits → built-by-agents + Opus 4.8, dropped Homebrew thanks. Also: Dock-hover
shows "agency-agents-app" (dev binary name) while menu bar shows "Agency Agents" (productName) — a
DEV-ONLY artifact; `tauri build` produces "Agency Agents.app" where both match. Plan persisted to
phase-roadmap.md (v2 sequence). First git commit made. Next: #1 clone-as-source-of-truth.

2026-06-08 — Install-management loop + grouped Library + signed build + report (lead). Long session;
many commits (see git log eacbf35..2370ce0). Highlights:
- CATALOG SOURCE (#1): CatalogSource{bundled|managed|userClone} in state/catalog.json; detect/provision
  (git clone or tarball)/pull; catalog_status + catalog_check_updates (git behind/ahead + diffstat);
  first-run picker; Settings→Catalog (git status + GitHub repo stats + sign-in via existing github
  store). Categories parsed from convert.sh AGENT_DIRS (integrations dropped, strategy added).
- RECURSIVE INDEXING: build_from_dir + read_source now recurse; real clones nest agents
  (game-development/godot/<slug>.md etc.). Michael's clone had 19 nested agents the flat scan dropped.
  Bundled baseline is flat → count unchanged (209). Test added.
- BYTE-IDENTICAL → CURRENT: installs_reconcile foreign sweep now byte-compares each recognized file vs
  the canonical render; identical → Current (in sync, no "adopt"), divergent → Foreign. Michael's
  "these are mine now" — a clean CLI setup stops nagging. (Load-bearing on render parity for non-CC tools.)
- DIFF VIEWER: agent_diff (already existed) wired to DiffModal + util/diff.ts (dependency-free LCS).
  Click a divergent pill → unified diff (− on disk / + catalog).
- UPDATE-FROM-CATALOG on divergent installs (backs up first). The common "pull what the repo has."
- LIBRARY = GROUPED BY AGENT: one row, a pill per tool. Pills color-coded by state; per-pill ✕ remove,
  ↻ update, click→diff. "Select" button gates bulk checkboxes (per-agent); "With selected" →
  Update/Track/Delete (Delete confirms + warns no-undo; inapplicable disabled). install.bulk()=1 reconcile.
- AppError rename + dead-brew purge (0 warnings); Settings refocused; resizable/persisted panels;
  window geometry persists on resize; install-into multi-select + remembered selection; app-data-dir
  bug fixed (was ~/Library/Application Support/brew-browser → com.zerologic.agency-agents-app).
- SIGNED + NOTARIZED BUILD: scripts/release.sh pulls notary pw + updater key from Keychain.
  SKIP_UPDATER=1 ./scripts/release.sh → Gatekeeper-accepted notarized+stapled .app + signed .dmg.
  Notary creds: keychain service "agency-agents-notary" / msitarzewski@mac.com, Team 7JQGQ7CRH8.
  Updater minisign key NOT set up yet. .gitignore hardened.
- ICON: new white-glyph-on-purple flat icon shipped; Liquid Glass layered source staged in docs/icon/
  (AppIcon.icon validates with actool) for Tahoe — not wired to build (Icon Composer is the export path).
- THE REPORT (Michael asked "did you run a report on the 8?"): replicated the app's compare against the
  userClone. 184 installed = 157 identical, 8 divergent (repo NEWER — e.g. software-architect +2766b
  real new sections; a "1-byte" one was a corrupted→fixed emoji, NOT whitespace → all 8 are genuine
  repo-newer versions → Update is right + frequent), 19 nested-unknown (→ fixed by recursion).
- ⚠️ RE-ORG COMING (Michael): re-verify catalog layout/categories/counts after it lands.
- IMMEDIATE NEXT: (1) renderer parity vs convert.sh for transform tools (now load-bearing); (2) decide
  uninstall-backs-up-first. Then: Dashboard rollup; multi-file renderers (#8); brew tech-debt sweep.
  Verdict: cargo test 247/0; svelte-check 0 err; build green. Memory bank updated; committing + pushing
  (created private GitHub repo — no remote existed before).

## 2026-06-08 (later) — IA RE-ORG: Agents + Library unified into a three-pane workspace
Michael: "the 'Library' and 'Agents' views are overlapping … bring the latest methods, charting, UX
patterns … cross-platform (macOS/Linux/Windows 11) … organize 200+ agents, the tools they're installed
in, and their purpose/roles." Decided via AskUserQuestion: (1) UNIFY Agents+Library into one view;
(2) THREE-PANE (mail/IDE) layout; (3) all four Dashboard charts (coverage matrix, health donut, category
distribution, per-tool coverage). Diagnosis: brew's "installed vs not" binary doesn't fit a 5-state
cross-tool world, so the same (agent×tool) install rows were rendered twice (PersonaDiscover slide-over
+ AgentLibrary) in two visual languages — and an agent's PURPOSE and its DEPLOYMENT never appeared
together.
- **PHASE A DONE (this session).** New `AgentsWorkspace.svelte` = three-pane: list pane (segmented filter
  lens All/Installed/Needs-attention/Untracked with live counts + search + Category ▾ dropdown +
  Select-mode bulk Update/Track/Delete, lifted from the old Library) · `ResizeHandle` · persistent
  resizable detail pane (reuses `detailPaneWidth`). Rows show emoji·name·vibe + compact tool-state dots.
- Extracted `PersonaBody.svelte` (persona header + markdown, verbatim from PersonaDiscover) — now takes a
  `deploy` Snippet rendered BETWEEN header and persona text (so deployment sits under the name/division).
- New `DeploymentMatrix.svelte` (fuses PersonaDiscover's install menu + AgentLibrary's per-pill actions):
  SUMMARY line of pills for installed tools (state-dotted) + a DISCLOSURE ("USE WITH ⌄", was "Deploy
  into") of every tool. User tools = a `Switch` (on=installed; off=removed). Project tools (Cursor,
  opencode) = Install/Add-project + per-project sub-rows (a switch can't represent N projects). Drift
  actions (Diff/Track/Update) inline only when applicable. Disclosure auto-opens per-agent when nothing's
  deployed, else closed. Pill label nudged up 2px (`.dm-plabel`).
- New `Switch.svelte` — extracted from Settings→Network "Offline Mode" toggle; shared primitive.
- New `util/platform.ts` — `isMac` / `modKey` / `shortcut()`; shortcut glyphs now adapt (⌘ vs Ctrl+).
- **DELETED** `PersonaDiscover.svelte` + `AgentLibrary.svelte` (net −2 big components, +4 smaller/shared).
- Nav: retired the `library` section everywhere (Sidebar, +page keymap renumbered 0–4, CommandPalette,
  types `SidebarSection`, ui `SECTION_TITLES`/`DEFAULT_SECTION_VALUES`, SettingsSectionAppearance). New
  `ui.agentsFilter` + `ui.openAgents(filter)` deep-link; Dashboard stat cards + palette use it. Agents
  carries the attention badge now. Section id stayed `personas` (renders the workspace) to limit churn.
- Verdict: svelte-check 0 err (only the benign `node` tsconfig note); `npm run build` ✓; cargo 247/0
  (no Rust touched — pure frontend re-org).
- **REMAINING:** Phase B (4 Dashboard charts, dependency-free SVG/CSS, deep-link cells into the
  workspace) · Phase C (Windows/Linux titlebar / traffic-light degradation; "this device" copy;
  home-path display). Plus prior IMMEDIATE: renderer parity vs convert.sh; uninstall-backup decision.
- **BACKLOG (new, Michael):** a "local-runtime system-prompt" target class — Ollama Modelfile `SYSTEM`
  / LM Studio preset — as its OWN renderer+runtime track, NOT an entry in the agent-host tool set
  (those 7+4 ingest a persona file into an agents dir; runtimes serve weights and have no agents dir).

## 2026-06-09 — Phase B charts, back/forward nav, Tools console, dev icon + squircle
Big session on top of the IA re-org. All frontend except the Tools backend commands + dev-icon (Rust).
- **PHASE B — Dashboard charts (dependency-free SVG/CSS).** `HealthDonut.svelte` (install-health donut,
  clickable legend deep-links by state), `CoverageMatrix.svelte` (category × tool heatmap; only
  tools/columns with installs — empty = noise; cells deep-link). Rebuilt `AgencyDashboard`: stats +
  donut + coverage-by-tool bars + matrix + full category distribution. Dropped redundant "Tools on this
  device" card. Zero-value stat cards (need attention / found to track) hide when 0.
- **BACK/FORWARD NAV.** `ui` gained a `NavLocation` history (section + agentsFilter + agentsCategory +
  agentsSelected) with `back()/forward()/canBack/canForward`, commit-on-navigation + `applyingNav` guard.
  Lifted `agentsCategory` + `agentsSelected` into `ui` (workspace drives its detail off them via an
  effect). Titlebar ◀▶ buttons, ⌘[ / ⌘], mouse buttons 3/4. `initNav()` seeded in +layout.
- **DIVISION DEEP-LINKS.** `ui.openDivision(slug)` → Agents filtered to that category (keeps open agent).
  Wired to the persona-header pill (PersonaBody `onCategory`), Dashboard category bars, CoverageMatrix
  rows+cells. Lens counts now NARROW to the selected division. Added a **"Not installed"** lens
  (complement of Installed). Lens row hides any zero-count lens except All + the active one.
- **TOOLS CONSOLE (the "by tool" axis).** Rust: `reveal_path` (open dir in Finder/Explorer/xdg) +
  `tool_versions` (best-effort `<bin> --version`, concurrent, 3s timeout) + `ToolVersion` type, both
  registered. `ToolsView.svelte` rebuilt as **list/detail two-pane** (resizable list, persisted width):
  brand badges (`util/toolBadge.ts` accent+initial), per-tool health bars, version, detected dot; detail
  console = Reveal folder, Default-target `Switch`, **Sync to catalog / Track all / Remove all** (via
  `install.bulk`), per-agent diff/update/track/remove, projects list for project tools. `install` store
  gained `versions`/`loadVersions`/`versionOf`/`revealPath`. New shared `Switch.svelte` (from Settings).
- **DEV DOCK ICON (macOS, debug-only).** Set `NSApplication.applicationIconImage` from `icons/icon.png`
  via objc2 msg_send. KEY FIX: do it in the **`RunEvent::Ready`** handler (changed `.run(ctx)` →
  `.build(ctx)?.run(|_,e| …)`), NOT `setup` — setup is too early, macOS reassigns the default at
  activation. Deps: `objc2` + `objc2-foundation` (already locked via window-vibrancy). Confirmed working.
- **ICON → macOS squircle.** Source was full-bleed purple square (looked like a sticker). Regenerated:
  ImageMagick rounds the brain art to an 824px squircle (r≈185, ~22.4%) on a 1024 transparent canvas
  (100px margin) → `docs/icon/agency-icon-macos-1024.png`, then `npm run tauri icon` regenerated all
  sizes + `.icns`/`.ico`. Now sits like a native app icon.
- Verdict: svelte-check 0 err; `npm run build` ✓; `cargo test --lib` 247/0; `cargo check` ✓.
- **REMAINING:** Phase C (Windows/Linux titlebar/traffic-light degradation) · renderer parity vs
  convert.sh (load-bearing) · uninstall-backup decision · local-runtime target · #8 multi-file renderers.

## 2026-06-09 (later) — macOS 26 Tahoe app icon (LIQUID GLASS) — FIXED + long detour
The app showed a blank/gray squircle in Finder/Dock. We burned a LOT of time treating it as an `.icns`
problem (made a squircle, regenerated 8-bit, canonical `iconutil` icns, swapped into the bundle, cleared
icon caches, lsregister) — the `.icns` was PROVEN byte-correct the whole time (composite-over-red = red
corners). **ROOT CAUSE: macOS 26 Tahoe renders app icons from a compiled `Assets.car` (Icon Composer
"Liquid Glass"), not `.icns`. Apps shipping only `.icns` get "icon jail" → blank squircle.** `tauri icon`
can't make `Assets.car` (tauri#14207/#14979). brew-browser "works" because its full-bleed square art
survives the legacy path. Web research (9to5Mac gray-box, Apple forums 801181, hendrik-erz.de) nailed it.
- **THE FIX (reproducible, wired + verified):** `actool` compiles `docs/icon/AppIcon.icon` (Michael's
  finished Icon Composer build) → `Assets.car` + a Tahoe-aware `AppIcon.icns`. **actool is ONLY in full
  Xcode, not CLT** — call it by path: `/Applications/Xcode-beta.app/Contents/Developer/usr/bin/actool …
  --compile … --app-icon AppIcon --include-all-app-icons`. Then: `Assets.car`→`src-tauri/Assets.car`
  (added to `bundle.resources`); actool's icns→`src-tauri/icons/icon.icns` (legacy fallback);
  new `src-tauri/Info.plist` with `CFBundleIconName=AppIcon` (Tauri 2 MERGES `src-tauri/Info.plist`).
  Full recipe in `docs/icon/README-liquid-glass.md`.
- Verified bundle (unsigned `npm run tauri build --config …signingIdentity:null`): `Contents/Resources/`
  has BOTH `Assets.car`(2.2MB) + `icon.icns`(72KB); Info.plist has BOTH `CFBundleIconName=AppIcon` +
  `CFBundleIconFile=icon.icns`. **Dock confirmed showing the live glass icon ("BAM").**
- **Dev Dock hack REMOVED** (was the source of black-corner/blank confusion): lib.rs back to plain
  `.run()`, `objc2`/`objc2-foundation` deps dropped. 247/0.
- GOTCHAS: don't run `npm run tauri icon` after — it overwrites `icon.icns` with a flat non-glass version.
  Re-run the `actool` step whenever `AppIcon.icon` changes. Tahoe caches icons HARD; if a build looks
  stale, `lsregister -kill -r -domain local -domain system -domain user` + clear iconservices + restart
  Dock/Finder.

## 2026-06-14 — Phase C: renderer parity VERIFIED + uninstall safety + cross-platform chrome
Picked up the in-flight `codex/renderer-parity-safety-phase-c` branch (nothing committed; entire diff
was working-tree only). Resolved the two IMMEDIATE backlog items and the deferred Phase C chrome.
- **#1 Renderer parity — VERIFIED (was load-bearing + unproven).** `render/mod.rs` reimplements the
  upstream shell converter's exact byte semantics: `source_field` mirrors `scripts/lib.sh#get_field`
  (first literal `field: value` between `---` fences, quotes/spelling preserved — NOT YAML parsing);
  `source_body` mirrors `body="$(get_body)"` (awk one-newline-per-line + command-substitution strips
  trailing newlines, heredoc re-adds exactly one); `slugify` mirrors `lib.sh#slugify`; `output_slug`
  encodes the converter's filename rules (identity tools preserve the source filename, transform tools
  derive from frontmatter `name`); Qwen's optional `tools` line preserved literally. New `--ignored`
  test `upstream_convert_sh_is_byte_identical_for_transform_tools` shells out to the REAL catalog
  `scripts/convert.sh` and diffs every transform tool byte-for-byte. **Result: 232 agents × 5 transform
  tools = 1160/1160 byte-identical** (Cursor `.mdc`, Codex TOML, Gemini, opencode, qwen). The
  `current`/Diff/Update state model now rests on proven ground.
- **#2 Uninstall safety — RESOLVED.** `install/mod.rs` `remove_agent_files` does a **backup-first pass**
  (`backup_if_differs` per destination) BEFORE any deletion, so a preservation failure can never strand
  a half-deleted agent. Decision locked: **modified files back up to `backups/` first (recoverable ✕);
  byte-identical/canonical files need NO backup (re-installable); if backup fails, the delete aborts and
  the original is preserved.** Tests: `uninstall_modified_file_backs_up_before_delete`,
  `uninstall_canonical_file_needs_no_backup`, `uninstall_backup_failure_preserves_original`,
  `uninstall_copilot_removes_both_destinations`, `uninstall_missing_file_is_successful`,
  `uninstall_removal_failure_is_reported`.
- **Phase C cross-platform chrome — DONE.** Split Tauri config: base `tauri.conf.json` is now
  cross-platform-safe (`decorations: true`, opaque, no macOS-only keys) so Windows/Linux get native
  titlebars; new `tauri.macos.conf.json` override re-adds `macOSPrivateApi` + `transparent` +
  `titleBarStyle: Overlay` + `hiddenTitle` + `trafficLightPosition` so macOS keeps the custom overlay
  titlebar. Frontend `TitlebarControls`/`ToolsView`/`AgentsWorkspace`/`ResizeHandle` touched for the
  degradation path.
- **brew-browser → Agency Agents rename completed** in `lib.rs` (doc header, env filter
  `agency_agents_app=info`, menu event ids `agency-agents/menu/*`, updater key paths). Dead brew
  `Settings` fields purged from `commands/settings.rs` + `types.ts`. Docs overhauled (README,
  CONTRIBUTING, SECURITY, BUILD, PHILOSOPHY, PLAN); stale `release-notes/0.3.0–0.5.0` removed; tool
  READMEs trimmed; Android icons regenerated.
- **New `tools/phase-c/` runner** (`phase-c.sh` + `validate-config.mjs`, npm `build:phase-c[:full]`):
  runs cargo tests + the `--ignored` parity test against `AGENCY_AGENTS_PARITY_ROOT`, frontend build,
  config validation, optional mac build + Parallels Win/Linux VM matrix.
- **Catalog is now 232 agents** (the warned re-org landed; counts shifted from ~222 as predicted —
  indexing + parity absorbed it cleanly).
- **Green:** cargo test 258/0 (+ the 1 parity test 1/0), svelte-check 0 errors, vite build clean.
  Then: memory bank updated, committed, pushed, PR opened.

## 2026-06-15 — Pre-release polish: brew vestiges, Activity Journal, Tools lens, cargo-test gate
On `release-planning` (off merged main). Documented the v0.1.0 release plan in `docs/BUILD.md#Release
Checklist` + a `decisions.md` ADR (manual signed DMG, SKIP_UPDATER, auto-update deferred) — NOT cutting yet.
Then knocked out pre-release issues, all green (svelte 0, cargo 258/0), committed + pushed:
- **brew vestige cleanup**: renamed `BrewErrorPayload`/`isBrewError`/`brewErrorMessage` → `AppError*`;
  removed the dead `catalogAutoRefresh` setting (struct/enum/tests + types.ts); removed dead error codes
  (`brew_not_found`/`brew_exit_non_zero`/`brewfile_not_found`/`job_not_found`/`canceled`/`feature_disabled`/
  `vulns_not_installed` — backend `AppError` emits none). **Deleted the brew-era Python pipeline**
  `tools/{catalog,categorize,enrich,pipeline,trending-collector}` — TRIAGE LESSON: I first wrongly flagged
  these as "live catalog pipeline"; they actually fetch Homebrew formulae (`fetch.py` → formulae.brew.sh)
  and are NOT used by AA (catalog = `corpus/mod.rs` reading the agency repo). `categoryIcon.ts` only
  referenced `categorize.py` in a comment (repointed to `agency-categories.json`).
- **Activity Journal**: the inherited "Activity" was a fully-built but PERMANENTLY-EMPTY brew streaming log
  (Sidebar ⌘4 + `ActivityHistory` + 223-line store + `ActivityDrawer`) — no AA backend ever emits
  `AppStreamEvent` (`startJob`/`handleEvent` called from nowhere). Michael: "Use it!" as a journal. Pivoted
  `activity.svelte.ts` → `JournalEntry` store (reused the localStorage persist/hydrate/cap machinery, v1→v2);
  `install.svelte.ts` logs install/uninstall/update/track/bulk + default-target switch; `ActivityHistory`
  rewritten (day-grouped rows, action icons, relative time, ok/error dots, Clear). Deleted `ActivityDrawer`,
  `AppStreamEvent`/`ActivityJob`/`ActivityLine`, `reportContextFromActivityJob`, the sidebar running badge.
  **Built via Workflow** (Frontend-Developer planner→builder, then a Code-Reviewer + UX-Architect verify
  team + fix loop): clean in 1 iteration, but the UX-Architect caught 3 real `major` issues my loop's
  blocker-only gate let through — Clear button hidden <1000px, "Switched added as default target →…"
  broken English, "Bulk 3 agents" non-verb. Hand-fixed all three. LESSON: gate the loop on `major`+, not
  just `blocker`.
- **Tools pane lens**: default to **Installed** (detected OR has agents); `Installed · Not installed · All`
  segmented toggle on the top row next to rescan (no count chips, per Michael's sketch). `ToolsView.svelte`.
- **Cold `cargo test` tauri-gate fix**: bare cargo (tests/CI, no Tauri CLI) reads only base `tauri.conf.json`
  which omits `macOSPrivateApi` (it's in `tauri.macos.conf.json`, merged only by the CLI) → `tauri-build`
  rejects the `macos-private-api` Cargo feature. Hidden locally by a warm build-script cache; fails on fresh
  checkout/CI. FIX: `.cargo/config.toml` sets `TAURI_CONFIG='{"app":{"macOSPrivateApi":false}}'` (the value
  that passes; `:true` does NOT). The Tauri CLI sets its own process-env `TAURI_CONFIG` (wins), so real
  `tauri dev`/`build` are unaffected — VERIFIED `tauri dev` launches clean. `macos-private-api` in `Cargo.toml`.
- **Showed it live**: `npm run tauri dev` (warm ~4.6s). Can't self-screenshot — Michael drove. GOTCHA stays.

### 2026-06-15 (later) — live UI polish in the running app (HMR feedback loop)
With `tauri dev` running, iterated on the Tools + Agents panes via screenshots; all svelte 0-error:
- **Tools pane**: bar reworked from a sync-state breakdown to a **catalog-coverage** bar — green =
  distinct catalog agents installed in the tool, gray track = the rest; shown on every row
  (`installedCount(tool)` / `catalogTotal`). (Dot = tool detected; bar = coverage — two different axes.)
- **Agents workspace**: REMOVED the All/Installed/… filter lens (the per-row install dots already show
  install count = number of tools deployed, so the lens was redundant; All==Installed looked pointless).
  List now filters by search + category only; `ui.agentsFilter` plumbing is now dead in the ui store
  (Dashboard/palette deep-links that set a filter are now no-ops — flagged, not yet cleaned).
- **Search row**: moved the Division (category) dropdown onto the search line as the FIRST element
  (pick target → then search), restyled to the neutral `.ghost` form look (dropped the brand/orange
  `.active`), menu now left-aligned.
- **Detail pane hidden when empty**: the resize handle + `<aside class="detail-pane">` only render when an
  agent is selected (`{#if panelAgent}`); the list goes full-width otherwise. Dropped the "Pick an agent"
  empty state + its `counts` derived + `.dpe-*` CSS.

### 2026-06-15 (later 2) — cross-platform creds, dead-code/brew pass, coverage-% matrix, Category→Division
- **Cross-platform GitHub credential storage FIXED + validated on VMs.** The token was stored macOS-only:
  `keyring` was built with `apple-native` only ("we only use the macOS path") so Windows/Linux had no
  native vault. Restructured `Cargo.toml` to per-target keyring backends — macOS `apple-native` (Keychain),
  Windows `windows-native` (Credential Manager), Linux `sync-secret-service`+`crypto-rust` (Secret Service
  via pure-Rust zbus, no system libdbus to build). ALSO found+fixed a latent error in my earlier `build:`
  commit — it wrongly added `macos-private-api` to the BASE `[dependencies]` tauri, which failed
  `validate-config.mjs` ("base excludes macos-private-api") and broke the **Linux** `cargo test` gate (the
  feature was on but config said false). It belongs in `[target.macos]` only (already there). **Validated via
  `phase-c.sh` VM matrix** (Scratch=Ubuntu + Windows 11, both running): Ubuntu cargo **258/0** + deb/rpm/
  appimage bundles; Windows x64 `.exe` + PE headers + artifacts; keyring backends compile on all three.
  (Windows arm64 build was a transient VM file-lock `Access denied` on a stale `.exe`, not code.) macOS
  stays 258/0. **GOTCHA for next session**: macos-private-api lives ONLY in `[target.macos]`; the cold
  cargo gate is handled by `.cargo/config.toml`. Linux Secret Service needs a running daemon at RUNTIME
  (headless VM has none) — build/compile is validated, true runtime auth needs a desktop session.
- **Dead code + brew pass**: removed the dead filter-lens plumbing (`ui.agentsFilter`, `AgentsFilter` type,
  `setAgentsFilter`, nav-history field) + repointed Dashboard/palette callers (dropped the dead
  "Open Agents — Installed/Needs attention" palette entries). A fork scrubbed ALL residual brew comment
  mentions (`grep -rin brew src/ src-tauri/src/` → none) + the `net.rs` test fixture; `cargo build --tests`
  → zero dead_code/unused warnings. **Consequence flagged**: the Dashboard drift cards (Outdated/Modified/
  Untracked/Missing) now just open Agents — no per-state drill-down anymore (lens is gone).
- **Adaptive uninstall/delete wording** (`AgentsWorkspace`): bulk destructive action relabels by ownership —
  "Uninstall — remove from disk (re-installable)" + neutral styling when all selected are ours; the loud red
  "Delete — remove files from disk" only when the selection includes `foreign` (not-ours) files.
- **OS-style menu dismiss** (`AgentsWorkspace`): category + bulk menus close on click-outside / Escape,
  excluding the trigger button so it still toggles (matches `TitlebarControls`' pattern).
- **Tools detail closes on lens change** (`ToolsView`): `sel` now resolves against `visibleTools`, so
  switching the lens away from the selected tool closes its detail panel.
- **CoverageMatrix → coverage-%**: cells shade by `installed ÷ division's catalog size` (0–100% per cell),
  not absolute count vs the global max — a fully-deployed small division reads as strong as a big one. Row
  number is now the division's catalog size (the denominator); tooltip `X of Y (Z%)`. (Alt on the table:
  Idea 2 = row-normalized distribution, one-line denominator swap.)
- **Terminology: user-facing "Category" → "Division"** (the catalog repo's term). Changed the visible labels
  only (CoverageMatrix header, "All divisions", empty-state copy, "Catalog by division"); the internal
  `category` field/identifiers (`agentsCategory`, `openDivision`, `agent.category`, `agency-categories.json`)
  stay — that's the catalog's frontmatter key.
- Green throughout: svelte-check 0, cargo 258/0 (macOS + Linux), config validation all-pass.

## 2026-06-15 (later 3) — Dashboard viz overhaul: donuts, division color scheme, cross-platform creds
Iterating live on the Dashboard "Cross-tool coverage" + division color. All green (svelte 0, cargo 258/0).
- **CoverageMatrix → coverage-% first** (committed in PR #3): cells shade by `installed ÷ division catalog
  size` (0–100% per cell), not raw count vs global max.
- **Then replaced the matrix with `CoverageDonuts.svelte`** (NEW, uncommitted): one donut per tool, sliced by
  division (segment = agents of that division in that tool), the tool's badge (`toolBadge` mark+accent) in the
  hole, a shared division legend, LINKED HOVER (hover a slice or legend row → highlights that division across
  all donuts, dims the rest). Dependency-free SVG arcs (HealthDonut technique). Swapped into AgencyDashboard
  (CoverageMatrix.svelte kept in tree, unused, for A/B). DONUT SIZING GOTCHA: must set `.cd-chart { flex: none }`
  (and `.cd-cell`) like HealthDonut's `.hd-chart`, else the flex row resizes donuts unevenly by label length;
  STROKE=16 R=50 viewBox 120 size 132 is the canonical donut spec.
- **DIVISION COLOR SCHEME (the big one).** Divisions had NO color (just label+icon). Established a curated
  18-color palette (approved by Michael) and made it CATALOG METADATA:
  - **Catalog PR**: `divisions.json` in the agency-agents repo (branch `add-division-metadata`) — slug →
    {label, icon, color} for all 18 dirs. PR: github.com/msitarzewski/agency-agents/pull/592. Fixes "GIS"
    (was title-cased "Gis") + covers gis/integrations which had no metadata.
  - **App reads it**: `src-tauri/data/agency-categories.json` mirrors it (color + the 2 missing divisions);
    threaded through Rust (`CategoryMetaRow.color`, `category_meta` → 3-tuple, `Category.color`, default
    `#94A3B8`) and TS (`Category.color`); exposed as **`corpus.colorOf(slug)`**. Charts read THAT (the
    earlier hardcoded `util/divisionColor.ts` was a stepping stone — now DELETED). `category_meta` test asserts
    `#3B82F6` for engineering.
  - Palette (slug→hex): specialized #6366F1, marketing #F97316, engineering #3B82F6, game-development #A855F7,
    gis #14B8A6, security #EF4444, product #D946EF, sales #10B981, design #EC4899, testing #F59E0B,
    paid-media #EAB308, project-management #0EA5E9, support #84CC16, spatial-computing #06B6D4, academic
    #8B5CF6, finance #22C55E, integrations #64748B, strategy #F43F5E.
- **Dashboard "Coverage by tool" click now selects the tool**: added `ui.toolsSelected: Tool | null` +
  `ui.openTools(tool)`; ToolsView honors it via an `$effect` (overrides its auto-pick) and keeps it synced on
  row-click; the dashboard bars call `ui.openTools(t.id)`.

### ⏳ RESUME HERE (queued, NOT done — Michael approved the plan, paused at 90% context)
1. **Build the catalog-by-division segmented bar** (Michael's mockup: a single full-width proportion bar,
   segments = divisions colored via `corpus.colorOf`, with leader-line callouts above/below — top row = larger
   divisions near their segment centers, bottom row = smaller divisions EVENLY spaced with angled dotted
   leaders to their clustered-right segments; header "Total agents {N} / 100%", footer caption). Plan: new
   `CatalogByDivision.svelte`, replace the current "Catalog by division" bar-list in `AgencyDashboard.svelte`
   (the `<ul class="bars scroll">` block ~line 119-133). Data = `corpus.tiles` sorted by count desc; use an
   SVG overlay (viewBox "0 0 100 H" preserveAspectRatio=none + vector-effect non-scaling-stroke) for leaders,
   absolutely-positioned HTML label blocks at even x%, segments as `<button>` (openDivision, keyboard-ok).
2. **Tint the division ICONS with the division color** — the `Division ▾` dropdown menu items in
   `AgentsWorkspace.svelte` (the `resolveCategoryIcon` usage) and the persona division pill in
   `PersonaBody.svelte`. Use `corpus.colorOf(slug)` for the icon color.
3. categoryIcon.ts: gis→"Map", integrations→"Workflow" Lucide names are in the metadata now but NOT imported
   in `categoryIcon.ts` (it statically imports ~19 icons) → add `Map` + `Workflow` imports + map entries, else
   they fall back to HelpCircle.
4. Then: commit the color-scheme batch + push (onto PR #3), and the catalog PR #592 awaits merge.
- **Uncommitted at checkpoint**: agency-categories.json, corpus/mod.rs, types.rs, AgencyDashboard.svelte,
  ToolsView.svelte, corpus.svelte.ts, ui.svelte.ts, types.ts, + NEW CoverageDonuts.svelte.

## 2026-06-15 (later 4) — Catalog-by-division chart DONE + icon tinting (resume items 1–3 all closed)
All three queued items landed; green (svelte-check 0). Built live with Michael's screenshot feedback each round.
- **`CatalogByDivision.svelte`** (NEW) replaces the orange `<ul class="bars">` bar-list in AgencyDashboard.
  ONE proportional bar: a `<button>` segment per division, width ∝ count, painted with `corpus.colorOf`,
  thin inset separators, hover brighten. Header reads "{total} agents … 100%". Labels span FOUR lanes:
  - top-inner = **majors** (pct ≥ MAJOR_PCT=6: Specialized/Marketing/Engineering/Game-Dev) centered over
    their segment with a short colored stem;
  - top-outer = next tier (TOP_TIER=4: GIS/Security/Design/Sales) fanned across the right side (fills the
    space majors leave empty);
  - bottom-inner + bottom-outer = the tail, split even/odd into two interleaved fan-rows.
  - **Leaders are non-crossing Z-elbows** (Michael: "use elbows … so lines and labels never cross"): each is
    a `<polyline>` vertical-off-segment → short horizontal at a **rank-staggered rail** (`5+rank*3` px off the
    bar, so no two horizontals share a y) → vertical down the **label's own column**. Bottom rows use
    **phase-shifted x-spans** ([2,88] vs [14,99]) so one row's verticals fall between the other's labels.
    Axis-aligned segments stay axis-aligned under preserveAspectRatio=none → true 90° corners.
  - **Linked hover** like CoverageDonuts: `hovered` slug set by segment/label mouseenter dims everything else
    (segments .22, labels .32, leaders .12). Tunable knobs at top: MAJOR_PCT, TOP_TIER, the two span ranges.
- **Division icons tinted** with `corpus.colorOf`: `Division ▾` dropdown items in AgentsWorkspace (`.cat-ic`
  span; neutralizes to brand on the active row) + persona pill leading glyph in PersonaBody. Added
  **`corpus.iconOf(slug)`** (mirrors labelOf/colorOf, falls back "HelpCircle").
- **`categoryIcon.ts`**: imported `Map` + `Workflow` → GIS (was the "?" in the screenshot) now a map pin,
  Integrations a workflow glyph; everything else unchanged.
- GOTCHA repeat: `{@const DivIcon = …}` must be an immediate child of `{#if}` (moved it above `<header>`),
  not nested inside the `<div class="pb-titles">`.
- Files: NEW CatalogByDivision.svelte; AgencyDashboard.svelte (swap + drop dead cats/maxCat/resolveCategoryIcon
  import + `.bars.scroll`/`.bar-ic` CSS); AgentsWorkspace.svelte; PersonaBody.svelte; corpus.svelte.ts (iconOf);
  categoryIcon.ts. Next: merge PR #3 → main, then VM matrix Win/Linux build verify.

## 2026-06-15 (later 5) — PR #3 MERGED to main; giant-donut fix; full VM matrix GREEN (21/0/1)
- **PR #3 merged** (`release-planning → main`, merge commit 633ff3ff). Catalog-by-division + icon work shipped.
- **Giant CoverageDonut bug** (Michael: "that ain't right" — one donut ballooned to full card width). ROOT
  CAUSE: the donut `<svg>` had only `viewBox` + relied on `.cd-chart { width:132px }`; when that scoped style
  doesn't apply (stale/raced webview CSS, e.g. a dead `tauri dev`'s last frame), a viewBox-only svg grows to
  full width and its 1:1 ratio balloons into one giant circle. Verified source + dev-served CSS were BOTH
  correct → it was a stale webview. FIX (hardened, not just a reload): pinned the svg with explicit
  `width="132" height="132"` attributes + `.cd-chart svg { width:132px;height:132px }` (was 100%) so it has a
  hard px floor and can never balloon again. `CoverageDonuts.svelte`, svelte-check 0. (HealthDonut shares the
  technique but renders fine; left as-is per minimal-change.)
- **⚠️ FOOTGUN (bit us again): `tauri dev`/`tauri build` on macOS auto-rewrites `src-tauri/Cargo.toml`** — it
  ADDS `macos-private-api` to the BASE `[dependencies] tauri` to match the config (it doesn't understand our
  base-excludes / `[target.macos]`-includes split). That breaks `validate-config.mjs` ("base excludes
  macos-private-api") AND the macOS-release + Linux cargo gates ("tauri features don't match the allowlist").
  The 2nd matrix run failed 3 steps purely because I'd launched `tauri dev` to show the donut. RULE: before any
  cross-platform gate, **stop `tauri dev` and `git checkout -- src-tauri/Cargo.toml`** (revert the base-feature
  injection; line ~17 must NOT list macos-private-api, only line ~89 `[target.macos]` does). See [[tauri-dev-rewrites-cargo-toml-base-features]].
- **Final matrix run 20260615T190208Z: 21 passed / 0 failed / 1 skip** (skip = opt-in PHASE_C_STRICT_TAURI).
  arm64 PASS after Michael closed the running .exe that held the file lock on `C:\AgencyAgentsWinTarget`.
- **Uncommitted**: CoverageDonuts.svelte (the donut hardening) — awaiting Michael's go to branch + PR (on main).

## 2026-06-15 (later 6) — app reads catalog divisions.json directly (corpus, backend-only)
Catalog PR #592 merged (divisions.json now canonical, with a check-divisions CI linter + becoming the CLI
installer source). Verified the catalog clone's divisions.json (18 divisions) is byte-in-sync with the app's
bundled `agency-categories.json` (no drift). Then wired the app to read it directly:
- `Corpus` gained `division_meta: BTreeMap<String, CategoryMetaRow>`, resolved at build time in
  `resolve_active` via new **`load_division_meta(catalog_root)`**: start from the bundled floor
  (`bundled_division_meta()` ← `agency-categories.json`), then OVERLAY the catalog root's `divisions.json`
  (PR #592, `{ "divisions": {...} }` via new `DivisionsFile`). `categories()` now reads the field
  (`category_meta_from`) instead of the old per-call `category_meta(slug)` (removed).
- **Resolution chain**: catalog divisions.json (overlay) → bundled agency-categories.json (floor) →
  title_case/Folder/grey (per-slug last resort). **First-run + pre-#592 clones have no divisions.json → bundled
  floor, identical to before (zero risk).** Overlay (not replace) means a divisions.json that omits a division
  keeps the bundled row; a NEW catalog division presents correctly WITHOUT an app update — the payoff.
- Re-resolved on every `resolve_active` (startup + the two refresh paths) → a freshly-pulled divisions.json is
  picked up on corpus refresh, no restart. Frontend unchanged (already reads Category.color/icon).
- Tests: +4 (missing→bundled, malformed→bundled, overlay override+net-new+retain-omitted, unknown-slug
  fallback); updated the bundled-json test to use `category_meta_from`. cargo lib 262/0, no warnings. GOTCHA:
  raw string with hex colors needs `r##"…"##` (a `"#` closes `r#"…"#`).
