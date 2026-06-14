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

### OPEN: updater signing key + endpoint host
The minisign pubkey in `tauri.conf.json`/`lib.rs` is still brew-browser's placeholder, and
the endpoint `agency-agents-app.zerologic.com` is not yet provisioned. Regenerate a keypair
per `docs/BUILD.md` and provision the host before cutting a release. Until then, updater is
inert (fine for dev).
