# Phase Roadmap — Agency Agents

Mirrors how brew-browser was built (incremental, always-green). Each phase ends with a
compiling app + a demoable slice.

## ⭐ Current plan (v2, 2026-06-06) — the active sequence

Phases 0–3 are DONE (details below). The Library "frozen" saga is resolved (root cause:
`each_key_duplicate` from Copilot dual-write, NOT reactivity — see agentLog). Dev port moved to
1430 (brew-browser keeps 1420). The active forward sequence is now:

0. **🧹 Brew-domain sweep** — remove the now-unreachable inherited brew code (catalog/cask/formula,
   vulns, trending, enrichment, services, brewfiles, brew exec) end-to-end: backend modules +
   commands + `state` fields + `data/` placeholders + `types`; frontend brew section components +
   their stores + brew Settings sections + `+page` branches + brew `api.ts`/`types.ts`.
   **KEEP (not-yet-used infra):** Settings shell + generic sections, design system, updater,
   `github/` (→ P4), util, the agency domain. Optional final step: rename `BrewError` → `AppError`.
   Run in green passes. Goal: a clean, stable foundation ASAP.
1. **Clone-as-source-of-truth** — detect/prompt for an existing agency-agents clone OR provision
   `~/.agency-agents`; **pull on first launch**; **dynamic category discovery** (a division = any
   top-level dir with agent-frontmatter files); cache in `.agency-cache/` (gitignored upstream);
   honor `aliases.json` (renames); surface **orphans** (removed upstream); enforce unique slugs.
   First-run **selection modal** + **Settings → Catalog source**.
2. **Track-all / Update-all** — trustworthy once #1 compares against real HEAD.
3. **Tool-grouped Library IA** — L1 tools+counts+state-breakdown+bulk → L2 per-tool agents+filters.
   With the **safe verbs**: **Track** (non-destructive, no write) / **Update** (diff+backup+confirm) /
   catalog-drift guard (never downgrade); **symlink-aware reconcile** (new `Linked` state; NEVER
   write through a symlink; recurse subdirs; recognize whole-dir aliases). Library & Tools stay
   separate.
4. **Renderer-parity test** vs `scripts/convert.sh` (CLI-installed transform-tool files must not
   read as "modified"). ✅ DONE 2026-06-14 — `--ignored` test diffs the real converter byte-for-byte:
   232 agents × 5 transform tools = 1160/1160 byte-identical. Plus uninstall safety (backup-first) +
   cross-platform chrome (tauri.macos.conf.json split) shipped in the same Phase C branch.
5. **Phase 4** Trending + GitHub · **Phase 5** Quality (lint+originality) · **Phase 6** Release
   (signed/notarized DMG, updater key). Deferred: 4 multi-file renderers (task #8).

### Posture (locked)
The app is a **respectful frontend, never an authority**: never auto-mutate; default read-only;
all writes explicit/previewed/reversible; source of truth is the user's clone, shared with the CLI.

---

## Original phase log

- **Phase 0 — Fork & boot** ✅ DONE (2026-06-05)
  Forked brew-browser scaffold, rebranded to Agency Agents / `com.zerologic.agency-agents-app`,
  green `cargo check` + `vite build`. Brew's real bundled data restored so brew-domain tests stay
  green until that domain retires.

- **Phase 1 — Corpus + Discover** ✅ DONE (2026-06-05)
  `corpus/` module (bundled baseline, GitHub-tarball refresh, frontmatter parse, sha256 split-hash
  `corpus-index.json`), `corpus_*` commands, Agents catalog view (16-cat grid + search). Catalog =
  **210 agent personas / 16 categories** (strategy/examples are docs, excluded). Verified live:
  app seeds + parses 210 agents at launch. Fixed a concurrent-seed race in `ensure_corpus`
  (released cache lock during init → parallel seeders stomped temp files). 615 tests green.

- **Phase 1.5 — Agency-first polish** ◀ IN PROGRESS
  Make the whole shell read as Agency Agents, not brew-with-a-tab:
  1. Real Lucide category icons (kill the "?" fallback in the tile component).
  2. Retire/replace brew sidebar sections (Dashboard/Library/Discover/Snapshots/Services) + the
     `brew 5.1.x` footer with agency equivalents; lead nav with Agents.
  3. Re-enable window vibrancy (`tauri.macOSPrivateApi = true`).
  4. Agency-flavored empty/loading states.

- **Phase 2 — Install + Reconcile** (NEXT — the differentiator)
  `render/` deterministic per-tool converters (port `scripts/convert.sh`, 11 tools), `ledger/`
  (installs.json), `reconcile/` (5 states), `tools/` detection, `projects/` registry. Commands:
  install_agent, uninstall_agent, installs_reconcile, adopt_agent, update_agent, tools_list,
  projects_*. UI: category → agent list → persona detail → **Install**; Library (status chips);
  Tools; Activity stream wired to install jobs.

- **Phase 3 — Loadouts + Dashboard** ✅ DONE (2026-06-05)
  "Agentfile" JSON export/import (loadout_export/import; restore set on a new Mac). Agency
  Dashboard rollup (available / installed-by-you / need-attention / found-to-adopt → deep-links;
  catalog-by-category bars; detected-tools strip) — replaced brew Dashboard. Full agency nav.

- **Phase 4 — Trending + GitHub**
  New & Updated / Popular from corpus git history. github star/watch/issues (port near-verbatim),
  show the 107K★ repo stats.

- **Phase 5 — Quality**
  Opt-in lint + originality scan (port agency-agents `lint-agents.sh` +
  `check-agent-originality.sh`), per-agent quality card, footprint.

- **Phase 6 — Polish + Release**
  Settings sections, command palette verbs, regenerate minisign updater key, provision endpoint,
  first signed+notarized DMG via `tools/build/sign-and-notarize.sh`.

## Parallelization map (for any future team)
- Sequential prereqs: `types.rs`/`types.ts` → everything; `lib.rs` wiring = integrator.
- Safe-parallel: the 11 `render/<tool>.rs` converters; independent frontend section components.

## Future / parked ideas
- **Playbooks section** — surface the NEXUS playbooks (`strategy/`) + workflow examples
  (`examples/`) from the upstream repo as a separate browsable section (they're docs, not agents).
