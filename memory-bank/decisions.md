# Decisions (ADRs) — Agency Agents

### 2026-06-05: Fork brew-browser structurally
**Status**: Approved. **Context**: brew-browser is a proven, signed, shipping Tauri 2 +
Svelte 5 native macOS app that is "a thin respectful frontend over a CLI." Agency Agents
is the same pattern over the agency-agents repo. **Decision**: rsync the scaffold, keep
the entire shell/UI/build/updater infra, replace only the brew *domain*. **Consequences**:
~90% of non-domain code reused; fastest path to a real signed app.

### 2026-06-05: Plan B — native Rust install engine (not shell-out)
**Status**: Approved. **Context**: brew-browser shells out to `brew` because brew does
heavy lifting (downloads, deps, compile). Our "install" is just transform-frontmatter +
copy-file. **Alternatives**: (A) shell out to repo's `install.sh`/`convert.sh` — adds
runtime bash/python dep, non-deterministic output. **Decision**: reimplement conversion +
install natively in Rust; `convert.sh` is the reference spec. **Consequences**: self-
contained, cross-platform, and — critically — **deterministic output we can hash**, which
is the prerequisite for state tracking. Load-bearing.

### 2026-06-05: Corpus-copy model (own the repo locally)
**Status**: Approved. **Context**: the catalog (agency-agents) is small (3.4MB), git-
versioned, and changes constantly. **Decision**: maintain our own working copy in app
support, seeded from a bundled baseline, refreshed from the GitHub **tarball** (no runtime
git). Derive `corpus-index.json` (hash index) from it. **Consequences**: one decision
unifies catalog + updates + provenance + trending (git history). Commit SHA = version.

### 2026-06-05: State tracking — ledger reconciled against disk (we ARE the database)
**Status**: Approved. **Context**: AI tools have no install registry; `install.sh` copies
and forgets. **Decision**: maintain a ledger (`installs.json`) and reconcile it against
disk + corpus-index into 5 states (Current/Outdated/Modified/Removed/Foreign). Two hashes:
`source_hash` (version identity) + `rendered_hash` (local-edit detection). **Consequences**:
this is the app's core differentiator — cross-tool agent state nobody else has.

### 2026-06-05: Provenance by hash-match only — never mutate agent files
**Status**: Approved. **Alternatives**: stamp `x-agency-source` into frontmatter (rejected:
mutates content, breaks TOML/.mdc/rules formats). **Decision**: identify "ours" by slug +
re-render hash-match against corpus-index; offer an explicit **Adopt** for recognized
Foreign files. **Consequences**: zero content mutation; respects every tool's format.

### 2026-06-05: Both scopes (user-global AND project-scoped), fully tracked
**Status**: Approved. **Decision**: user-global tools use fixed `~/…` dests; project-scoped
tools install into any dir and are tracked per `project_path` via a Projects registry.

### 2026-06-05: vulns→Quality, services→Tools, Snapshots→Loadouts
**Status**: Approved. **Decision**: repurpose brew-browser's opt-in vuln scanner as an
opt-in lint+originality scanner (agency-agents ships `lint-agents.sh` +
`check-agent-originality.sh`); `brew services` view becomes per-tool deployment management;
Brewfile snapshots become "Agentfile" loadouts.

### 2026-06-05: Agent catalog = 210 personas / 16 categories (not 251)
**Status**: Approved. **Context**: the agency-agents repo has ~251 `.md` total, but many are
docs. The corpus parser's real-baseline test revealed only files with `name:` frontmatter are
agents. **Decision**: the catalog is the **210** agent personas across **16** categories. The
repo's `strategy/` (NEXUS playbooks/runbooks) and `examples/` (multi-agent workflow walkthroughs)
are documentation, excluded from `CATEGORY_DIRS`. **Consequences**: honest headline count (210);
nested agents (game-development/unity, strategy subdirs) are flattened to their top category during
seeding so none are undercounted. **Future**: the NEXUS playbooks + workflow examples are good
content — candidate for a separate "Playbooks/Workflows" section later, not the agent catalog.

### 2026-06-05: Restore brew's real bundled data until the brew domain retires
**Status**: Approved. **Context**: Phase 0 swapped brew's bundled catalog for empty placeholders to
compile; that broke 14 brew-domain tests. **Decision**: restore brew's real `data/` files so the
not-yet-replaced brew domain stays green (always-green principle); delete them when brew
catalog/enrichment/categories modules are removed. Corpus uses its own `agency-categories.json`.

### 2026-06-14: Renderer parity is a tested contract, not an assumption
**Status**: Approved. **Context**: the `current`/Diff/Update state model assumes Rust `render/` output
is byte-identical to the upstream `scripts/convert.sh` for transform tools; a single newline drift would
make every CLI-installed Cursor/Codex/Gemini/opencode/qwen agent falsely read `foreign`/`modified`.
**Decision**: encode the converter's exact shell semantics in `render/mod.rs` (`source_field` =
`lib.sh#get_field` literal-field extraction with quotes preserved, `source_body` = awk +
command-substitution newline handling, `slugify`, `output_slug` filename rules) and enforce parity with
an `--ignored` test that shells out to the REAL converter and diffs every transform tool byte-for-byte.
**Consequences**: parity is now proven (232 agents × 5 tools = 1160/1160 identical) and regressions are
caught; the test must be re-run after any converter or catalog change (`npm run build:phase-c`).

### 2026-06-14: Uninstall is recoverable (backup-first), byte-identical needs no backup
**Status**: Approved. **Context**: quick ✕ / bulk Delete deleted files with no backup, unlike
Update/Restore. **Decision**: `remove_agent_files` runs a backup-first pass — modified/divergent files
back up to `backups/` BEFORE any deletion; byte-identical/canonical files need no backup (re-installable);
if a backup fails, the delete ABORTS and the original is preserved (a preservation failure can never
strand a half-removed agent). **Alternatives**: keep deletion final (rejected — data loss for divergent
agents). **Consequences**: the ✕ is now reversible for the cases that matter, with full test coverage.

### 2026-06-14: First release = v0.1.0, manual DMG, auto-update deferred
**Status**: Approved (plan only — NOT cutting yet). **Context**: all three manifests already read
`0.1.0`; signing + notarization are proven; the updater pubkey is real but the endpoint
(`agency-agents-app.zerologic.com/updater.json`) is not provisioned. **Decision**: ship v0.1.0 as a
signed + notarized `.dmg` for manual download, built with `SKIP_UPDATER=1`; defer auto-update to a later
release once the endpoint serves a manifest. **Out of scope** (documented as known limitations):
auto-update, multi-file renderers, Windows/Linux runtime verification, local-runtime target.
**Runbook**: `docs/BUILD.md#Release Checklist`. **Consequences**: fastest path to a real first release;
0.1.0 users update manually, but the shipped pubkey lets a later 0.1.x flip auto-update on. **Note**:
we are NOT cutting now — knocking out final pre-release issues first.

### 2026-06-15: Repurpose the inherited "Activity" surface into a usage journal
**Status**: Approved. **Context**: AA inherited brew-browser's "Activity" view (Sidebar ⌘4 + components +
223-line store) for streaming long-running `brew` jobs — but AA installs are instant native file writes, no
backend emits `AppStreamEvent`, so the section was fully built yet permanently EMPTY. **Decision**: keep the
surface but repurpose it as a frontend **journal** of discrete agent actions (install/uninstall/update/track/
bulk + default-target switch), logged from `install.svelte.ts`, persisted in localStorage, clearable. Delete
the dead streaming machinery (`AppStreamEvent`, `ActivityJob`, `ActivityDrawer`, the dead error codes).
**Alternatives**: remove Activity entirely (rejected — AA has plausible future long-running ops: catalog
clone/pull, updater download, bulk reconcile, which could stream into it later); wire real streaming now
(rejected — bigger scope). **Consequences**: turns dead weight into a useful history; localStorage is fine
(it's a UX journal, not a system of record — the ledger remains the source of truth). Future option: a
backend `activity.json` if it must survive webview-data clears.

### 2026-06-15: `.cargo/config.toml` to pass the tauri feature-gate on bare cargo
**Status**: Approved. **Context**: the cross-platform config split (PR #1) keeps `macOSPrivateApi` only in
`tauri.macos.conf.json`, merged by the Tauri CLI — but bare `cargo test`/`build`/CI read only base
`tauri.conf.json`, so `tauri-build` rejects the `macos-private-api` Cargo feature (fails on fresh checkout;
hidden locally by a warm build-script cache). **Decision**: a repo-root `.cargo/config.toml` sets
`TAURI_CONFIG='{"app":{"macOSPrivateApi":false}}'` for bare cargo invocations. **Alternatives**: put
`macOSPrivateApi` back in base config (rejected — adding it did NOT satisfy the gate empirically; the gate is
bypassed only when `TAURI_CONFIG` is set, and only with the `false` value). **Consequences**: bare cargo is
green from cold; the Tauri CLI sets its own process-env `TAURI_CONFIG` (precedence over `[env]`), so real
`tauri dev`/`build` use the merged config (`macOSPrivateApi: true`) — verified `tauri dev` launches clean.

### 2026-06-22: Updater host = `agencyagents.app`; dedicated agency signing key (resolves the OPEN host ADR)
**Status**: Approved (v0.2.0). **Context**: the prior OPEN entry assumed the endpoint would be
`agency-agents-app.zerologic.com` and that the embedded pubkey was a one-off real key. Tracing the live
build host settled both. **Decision (host — the important detail)**: the updater endpoint is
**`https://agencyagents.app/updater.json`**, NOT the `…zerologic.com` host the UI/docs/source comments
had drifted to. It is already in `tauri.conf.json` `endpoints` + the CSP `connect-src`, and Caddy on
`umacbookpro` serves `agencyagents.app` from `~/Sites/agency-agents/` (a sibling vhost to the live
`brew-browser.zerologic.com/updater.json`, so the file_server pattern is proven). Publishing is an
`rsync` of `dist/updater.json` to `umacbookpro:Sites/agency-agents/updater.json`. **Decision (key)**:
the embedded pubkey was byte-identical to brew-browser's shared key (id `7335DD0F`); swapped to a
**dedicated agency minisign key** (id `ABF5AFD8`) generated on the build machine — clean signing
isolation, done now while NO updater client is live (endpoint never served a manifest under
`SKIP_UPDATER`), so there is zero continuity cost. Private key + `signing.env` live at
`~/.config/agency-agents-app/` (chmod 600, outside the repo); Apple notarization uses the Developer ID
identity in the login keychain. The Rust `UPDATER_PUBKEY` const (`lib.rs`) is kept in sync with
`tauri.conf.json` for documentation only — verification is driven entirely by the conf value the
`tauri-plugin-updater` reads at startup. **Consequences**: v0.1.0 users (old embedded pubkey, and no
manifest ever existed) upgrade to v0.2.0 manually; auto-update is real from v0.2.0 forward. **Remaining
before live**: build without `SKIP_UPDATER`, run `tools/release/publish-manifest.sh`, host the signed
`.app.tar.gz` + manifest. Until then the updater ships present-but-disabled.

### 2026-06-16: Defer Windows code signing for v0.1.0
**Status**: Approved. **Context**: v0.1.0 is a Mac-first manual release (signed/notarized DMG); Windows is
build-validated by the Phase C matrix but not a committed shipping artifact. An unsigned Windows `.exe`
trips Defender SmartScreen ("Windows protected your PC" / unknown publisher). Removing that warning needs
both a trusted-CA signature AND SmartScreen reputation. **Decision**: defer Windows signing. Ship v0.1.0
without it; early Windows users click **More info → Run anyway** (documented in the release notes).
**Alternatives**: (a) **EV code-signing cert** (DigiCert/Sectigo/SSL.com, ~$250–600/yr, hardware/cloud-key)
— instant SmartScreen reputation, the "no warnings day 1" answer; (b) **Azure Trusted Signing** (~$10/mo,
cloud, Microsoft-managed) — cheap and modern but org must be 3+ yrs old or pass identity validation;
(c) **OV cert** — cheaper but reputation warms up over downloads, so early users still get warned. All
rejected *for now* on cost + weeks-long CA vetting lead time vs. an early, technical Windows audience.
**Consequences**: Windows users see a one-time SmartScreen click-through until we sign. **Revisit trigger**:
real Windows download demand → pick EV cert (instant reputation) or Azure Trusted Signing (if ZeroLogic
qualifies); the cert publisher name should match the legal entity behind bundle id `com.zerologic.*`. Build
wiring when we do it: a `tauri.windows.conf.json` (mirroring the macOS split) with
`bundle.windows.certificateThumbprint`/`signCommand` + `timestampUrl`, build the **NSIS** installer (drop
`--no-bundle`), sign exe + installer for both arm64 and x64, and verify on the Parallels Windows 11 VM with
a Mark-of-the-Web download.

### 2026-06-21: Tool registry as the single source of truth (drop the `Tool` enum)
**Status**: Approved (PRs #18 + #19). **Context**: tool knowledge was scattered — a Rust `Tool` enum + 8
hardcoded match-arm functions (label/detect/version/dests/scope/render), and on the frontend `ACCENTS`/
`ICONS_SVG`/`SHORT`/a hardcoded `SUPPORTED_TOOLS` + a `Tool` union. Adding a tool meant editing ~13 places;
adding one upstream in the CLI meant another ~13. **Decision**: a JSON registry is the single source. The Rust
`Tool` enum is removed — a tool is a `String` id; `label`/`detect`/`version`/`dests`/`scope` are registry lookups
and `render()` dispatches on a `format` key. The string id IS the serialized wire value (camelCase), so ledger
JSON stays compatible. **Alternatives**: codegen a typed union from the JSON (rejected — a generate step is "an
index to forget," the exact thing we're killing). **Consequences**: adding a tool that reuses an existing
`format` is data-only on both sides; a brand-new output shape needs one formatter function. Byte-parity tests
guard the render output. Load-bearing.

### 2026-06-21: `tools.json` is upstream-owned; installability is derived app-side
**Status**: Approved (coordinated with the `agency-agents` catalog repo). **Context**: who owns the canonical
tool list? The catalog repo already owns `divisions.json` (validated by a no-jq `check-divisions.sh`), and the
app is "a respectful frontend over the clone." **Decision**: the catalog repo owns the canonical **`tools.json`**
(twin of `divisions.json`), CI-guarded by `check-tools.sh` (the no-jq twin of `check-divisions.sh`, enforced by
`check-tools.yml`). It carries *upstream truth only* — what the CLI converts + installs. The app consumes a
bundled baseline (follow-up: refresh from the clone). Whether THIS app can install a tool is **derived, not
stored**: `installable(tool) = tool.format ∈ IMPLEMENTED_FORMATS` (the 7 formats the Rust renderer implements).
**Alternatives**: an `appRenderer`/`wired` bool in the catalog (rejected — couples upstream to app release
state; the format-membership rule is self-maintaining: ship a renderer → add its format → those tools light up).
**Consequences**: the catalog stays app-agnostic; the app's renderer coverage is one constant in two places
(`registry.rs` + `toolRegistry.ts`); the two repos share one CI-protected contract.

### 2026-06-21: Contribute transforms UPSTREAM first (Osaurus / the `skill-md` format)
**Status**: Approved. **Context**: the Rust `render/` is a byte-identical *port* of the catalog's
`scripts/convert.sh` (guarded by the parity test). A transform the app invents but `convert.sh` lacks would
**diverge** — a CLI `install.sh` user wouldn't get it. **Decision**: new transforms land in the catalog's
`convert.sh`/`install.sh` FIRST (canonical), then the app mirrors them. Osaurus shipped this way: a
`convert_osaurus` → Agent-Skills `SKILL.md` upstream, mirrored by a Rust `skill-md` format (with `slugPrefix:
"agency-"`). **Consequences**: one `skill-md` renderer covers Osaurus and any future Agent-Skills tool; the
parity test keeps the app honest. Antigravity stays app-recognized-only until upstream makes its skill
deterministic (its `date_added` is non-deterministic, so it can't share `skill-md`).
