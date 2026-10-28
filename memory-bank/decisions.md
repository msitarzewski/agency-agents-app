# Decisions (ADRs) — Agency Agents

## 2026-09-12 — Strip AppImage bundle paths from every child process, rather than patching git

**Context.** The v0.3.0 AppImage could not clone the catalog on any host that was not the build host. linuxdeploy's
`AppRun.wrapped` exports `LD_LIBRARY_PATH` with the bundle's directories first and the caller's original appended,
and saves no copy of the original anywhere. Children inherit it, so a host binary loads bundle libraries it was not
built against. Confirmed by unsquashing the shipped AppImage and reading `AppRun`, `apprun-hooks/…-gtk.sh` and the
strings in `AppRun.wrapped`.

**Decision.** One helper, `util::proc`, applied at **every** spawn site rather than a targeted fix in `run_git`.
It removes only path entries under `$APPDIR`, leaving the host's own search path intact, and is a no-op when
`$APPDIR` is unset.

**Why not the alternatives.**
- *Fix `run_git` only* — rejected: `probe_version` and `reveal_path` spawn host binaries through the same
  environment. Tool detection on Linux was plausibly degraded by this too. A bug report names a symptom; grep for
  the siblings (`grep -rn "Command::new"`).
- *Restore the original `LD_LIBRARY_PATH`* — impossible: AppRun keeps no copy. Removal is the only correct option.
- *Clear the whole environment* — rejected: too blunt, would break legitimate inherited config.
- *Strip `XDG_DATA_DIRS`/`GTK_*` wholesale* — rejected: AppRun *prepends* to those, so the host halves are real and
  must survive. We filter per entry.

**Consequence.** Bundle-vs-host library conflicts are closed off for all current and future child processes. The
rule generalizes: **anything an AppImage spawns must have the bundle stripped from its environment first.**


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

**Superseded 2026-10-27**: Windows releases are signed with Azure Artifact Signing (see that entry).

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

### 2026-06-23: Updater-enabled macOS release build mechanics
**Status**: Approved (v0.2.0 — the first release built WITHOUT `SKIP_UPDATER`). **Context**: turning auto-update
on exposed three latent traps that the manual-DMG (`SKIP_UPDATER`) path had always sidestepped. **Decisions**
(codified in `scripts/release.sh` + `docs/BUILD.md`, PR #22):
1. **Always pass a `--config` to the macOS build.** The tauri build-script's `macos-private-api` allowlist check
   reads **only base `tauri.conf.json`** — it does NOT honor the platform-split `tauri.macos.conf.json` where
   `macOSPrivateApi:true` lives (tauri#11142, closed "not planned"). With no `--config`, the feature (from the
   `[target.macos]` Cargo block) looks unauthorized and the build aborts. A `--config` forces a full config
   re-resolution that merges the platform file. `release.sh` always passes `--config '{"app":{"macOSPrivateApi":true}}'`.
   *Alternatives rejected*: putting `macOSPrivateApi` in base config (breaks the Linux/Windows allowlist) or the
   feature in base deps (the revert-before-cross-platform dance — fragile, must not be committed).
2. **Intel cross-compile via the rustup toolchain, not Homebrew rust.** The active `rustc` is Homebrew's
   (`/opt/homebrew/…`), host-only → `can't find crate for core` for `x86_64-apple-darwin`. Build with
   `PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"` after `rustup target add x86_64-apple-darwin`.
3. **The updater signing key lives in the Keychain; store it via `$(cat keyfile)`, never a manual paste.** A
   trailing newline corrupts the stored key and signing fails with `incorrect updater private key password:
   Invalid input`. The canonical key is the file `~/.config/agency-agents-app/updater.key` (its pubkey is embedded);
   the Keychain copy must match it byte-for-byte. `release.sh` is fully Keychain-based — no `signing.env`.
**Consequences**: `release.sh` now signs updater artifacts cleanly with no manual `signer sign -f` step; the next
release "just works." **References**: `tasks/2026-06/260623_v0.2.0-ship.md`, `~/Downloads/fix-updater-keychain.sh`.

---

## 2026-09-14 — Verify a PR set together before merging any of it, and gate Windows separately

**Status**: Approved. **Context**: seven PRs were merged for v0.3.1. All seven were first merged into a throwaway
worktree off `main` and verified as one tree (279 Rust tests, `svelte-check` 322 files / 0 errors) before any of them
touched `main`. That caught nothing — but #77, verified only on macOS, broke the Windows build immediately after
merge, because `spawnSync("tauri")` resolves to a real executable on macOS/Linux and to `tauri.cmd` on Windows.

**Decision**:
1. **A combined-tree check is the minimum bar** when landing more than two PRs at once; PR-by-PR checks do not prove
   the set.
2. **A combined tree is not enough on its own** — it only exercises the host platform. Any PR touching build
   invocation, process spawning or path handling must be checked on Windows, either by `pr-check.yml` or by
   dispatching a build against the branch before merge.
3. **Never route a command through a shim or a shell when the real entry point can be called directly.** Every
   Windows failure this session was an intermediary altering or hiding the command: `tauri.cmd` (Node refuses to exec
   `.cmd`), the npm shim through PowerShell (`Unknown command: "pm"`), `-ArgumentList` stripping quotes out of inline
   JSON, and `stdio: "inherit"` swallowing the output that would have explained any of it. The fix each time was
   `node <entry>.js` — and for config, a file path rather than inline JSON.

**Consequences**: #105 resolves `@tauri-apps/cli/tauri.js` and runs it with `process.execPath`. `pr-check.yml` moves
from "nice to have" to required. **References**: PRs #77, #105; issue #87.

---

## 2026-09-14 — Windows packaging stays on CI; the ARM64 VM is a build-and-test box, not a packaging box

**Status**: Approved. **Context**: the Parallels Windows 11 ARM64 VM was provisioned to build releases locally
(VS Build Tools, SDK 10.0.26100, Rust 1.98.1 both MSVC targets, Node 22.20). It compiles both architectures natively
in about three minutes each, but neither bundler runs: WiX's `candle.exe` and NSIS's `makensis.exe` are both 32-bit
x86. `makensis` exits `0xC0000135` with every plausible cause ruled out — x86 emulation verified working,
`zlib1.dll` x86 alongside, `MSVCP60.dll` present in System32 and SysWOW64, x86 VC++ redistributable installed.

**Decision**: CI packages Windows. The VM compiles, installs CI artifacts and tests them. `bundle.targets: "all"` is
accepted as **x64-only on Windows**; any ARM64 Windows build must pass `--bundles nsis`.

**Consequences**: two gaps to document — the build requires **clang**, which the VC workload omits and CI only
satisfies because the runner image ships LLVM; and a from-scratch Windows build host cannot produce installers.
**References**: `activeContext.md` § Windows build box.

**Superseded 2026-10-27** for releases: `makensis` runs in the VM now, and release installers are built and signed
there (next entry). CI keeps building unsigned installers as a check.

---

## 2026-10-27 — Sign Windows releases with Azure Artifact Signing, built and signed in the VM

**Status**: Approved (user: "yes, wire it in, sign in the VM"). Supersedes the 2026-06-16 deferral and, for
releases, the 2026-09-14 packaging decision. **Context**: issue msitarzewski/agency-agents#1181 — Smart App Control
blocked the unsigned v0.3.2 installer outright. SAC has no "Run anyway" and no per-app exception; it lets an app it
can't classify run only when a certificate from a trusted root signs it, and it checks every binary that loads.
Both v0.3.2 installers carry no Authenticode signature at all. The user's Artifact Signing account
(`msitarzewski-signing`, East US, Basic) already signs AutoPaper; its certificate names the person
(`CN=Michael Sitarzewski, O=Michael Sitarzewski, L=Dallas, S=tx, C=US`, RSA 3072, chained through
Microsoft ID Verified CS EOC CA 04 to Microsoft Identity Verification Root CA 2020), not the app, so the account's
one Public Trust profile (`autopaper`) signs this app as-is. Basic allows one profile of each type; a second would
need Premium.

**Decision**: `scripts/release-windows.ps1`, run inside the Windows 11 VM, builds the x64 and arm64 NSIS installers
and sets `bundle.windows.signCommand` through a `--config` file (SignTool + the Artifact Signing dlib, pinned by
SHA-256, timestamped). With a sign command, Tauri 2.11 signs the app executable, the five NSIS plugin DLLs, the
uninstaller and the installer. The script checks the installer's and the plugins' signer and timestamp; Tauri puts
the unsigned executable back in `target\` after packing, so the app's and uninstaller's signatures show only in an
install. The Azure credential stays in the VM (`az login`); none goes into GitHub. `-TestCert` exercises the whole
path with a self-signed certificate and no Azure.

**Consequences**: Windows release assets come from the VM, not CI; the tag-triggered CI build stays as an unsigned
check. The VM sees only `~/vm-exchange` (`\\Mac\Exchange`), as for AutoPaper. Windows auto-update is still off.

**Verified 2026-10-28** in the VM with SAC enforcing (`VerifiedAndReputableDesktop`; it blocked another app at
logon): both signed installers installed, the installed app and uninstaller verified Valid and timestamped, and the
app ran and uninstalled. The VM did not reproduce #1181's block: the unsigned v0.3.2 installers and a never-seen
unsigned build also ran there, even launched from the desktop with Mark of the Web. So the fix is confirmed only once
a user who hit the block installs a signed build.
