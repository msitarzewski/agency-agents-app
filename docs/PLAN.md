# Agency Agents App Plan

**Product:** Agency Agents  
**Repo:** `github:msitarzewski/agency-agents-app`  
**Catalog:** `github:msitarzewski/agency-agents`  
**Stack:** Tauri 2, Rust, SvelteKit, Svelte 5, TypeScript  
**License:** MIT

## Vision

Ship a native app for browsing, installing, and tracking the `agency-agents` catalog across AI coding tools.

The app should answer three questions clearly:

1. Which agents exist?
2. Where are they installed?
3. Are those installed files current, modified, missing, or foreign?

## Current Architecture

```text
Svelte UI                      four pillars: Agents / Tools / Teams / Projects
  Agents workspace
  Tools panel
  Teams (preset + saved)
  Projects (project-scoped installs)
  Dashboard
  Playbook
  Settings
      |
      | typed Tauri IPC
      v
Rust backend
  corpus/     catalog source, refresh, indexing
  registry    single-source tools.json (shared with the frontend)
  render/     deterministic tool renderers (format-dispatched)
  install/    write, uninstall, backups, ledger, reconcile
  github/     optional OAuth + GitHub API features
  settings/   local settings and network gates
  updater/    manifest fetch + minisign verify (present, endpoint not yet live)
      |
      v
Local filesystem
  app state
  agency-agents clone/baseline
  tool-specific agent directories
```

## MVP Scope

In scope:

1. Browse the `agency-agents` catalog by division, search, and detail.
2. Select a bundled, managed, or user-cloned catalog source.
3. Render supported tools natively in Rust.
4. Install and uninstall supported one-file-per-agent targets.
5. Track local install state with a ledger.
6. Reconcile disk state into current, outdated, modified, removed, and foreign.
7. Back up divergent files before removal or overwrite.
8. Show tool coverage and project targets.
9. Build signed macOS artifacts and cross-platform development builds.

Out of scope for the current release:

- executing agents
- arbitrary third-party plugin execution
- telemetry
- cloud sync
- paid tiers
- unverified install paths
- multi-file/aggregate renderers unless explicitly implemented and tested

## Supported Renderer Set

Current app-supported (installable) targets — 8:

- Claude Code
- Codex
- Gemini CLI
- GitHub Copilot
- Qwen Code
- Cursor
- opencode
- Osaurus (`skill-md` → `~/.osaurus/skills/agency-<slug>/SKILL.md`)

Known AA repo targets that still need app support (recognized-only in the Tools panel) — 5:

- Antigravity — blocked on upstream: `convert_antigravity()` still stamps a non-deterministic `date_added: '${TODAY}'`, so byte-parity is impossible until that field is removed or made deterministic.
- Aider
- Windsurf
- OpenClaw
- Kimi

## Near-Term Plan

### Phase A: Core Workspace

Done. Unified Agents/Library workspace with deployment matrix, search, filters, and persistent detail panel.

### Phase B: Dashboard And Tools Console

Done. Coverage charts, health summaries, category distribution, tool list/detail console, and deep links.

### Phase C: Cross-Platform Correctness

Mostly done. macOS retains overlay titlebar and vibrancy. Windows/Linux use opaque native decorated windows. Remaining work is repeatable build automation and native runtime verification on available VMs.

### Phase D: Tool Target Manifest

Done (v0.2.0). Shipped as the upstream-owned single `tools.json` — the twin of `divisions.json` —
declaring id, label, scopes, detect paths, version probe, output format, and destinations per tool.
Both the Rust backend (`registry`) and the frontend read it; the Rust `Tool` enum is gone and the
renderer dispatches on `format`. Installability is derived (`format ∈ IMPLEMENTED_FORMATS`), not stored.
Upstream guards drift with `scripts/check-tools.sh` (the no-jq twin of `check-divisions.sh`).

Remaining: the app bundles a baseline copy of `tools.json` — it should refresh from the catalog clone
at runtime (like the corpus), and aa's `check-tools.yml` CI workflow is still staged to land.

### Phase E: Multi-File Renderers

Implement special output shapes only after their path semantics are verified:

- Aider `CONVENTIONS.md`
- Windsurf `.windsurfrules`
- OpenClaw workspace directory
- Antigravity skill directories
- Kimi if current docs validate an installable custom-agent format

## Post-0.2.0 Punch List

Tracked inventory for the release after v0.2.0. Grouped by what unblocks each item.

### Auto-update

The updater UI, store, plugin, dedicated signing key, and publish tooling all ship.

1. **Endpoint — activated at the v0.2.0 release cut** (no longer post-0.2.0): host is `agencyagents.app`
   (Caddy on umbp from `~/Sites/agency-agents/`), the v0.2.0 build runs without `SKIP_UPDATER`, and
   `publish-manifest.sh` rsyncs the signed manifest there. Resolved in `decisions.md` (2026-06-22).
2. **Opt-in automatic install** *(remaining)* — today the live path is check → notify → one-click Install;
   the user still clicks. Wire the inert "Install updates automatically" toggle to a real off-by-default
   setting that does background download → verify → install. Backend install/relaunch plumbing exists.
3. **Beta channel** — "Update channel: Stable" is a read-only placeholder; wire real channel selection.
4. **Bulk-install auto-deploy** (separate idea) — a subscription that auto-deploys newly-added catalog
   agents into a division/team/project. Distinct from app self-update.

### Catalog / registry

5. Refresh `tools.json` from the catalog clone at runtime instead of bundling a baseline copy.
6. Land `check-tools.yml` CI upstream (aa repo).
7. Foreign-sweep for nested `…/<dir>/SKILL.md` skills — CLI-installed Osaurus/Antigravity aren't
   auto-detected; app-installed ones are.

### New install targets (recognized → installable)

8. Multi-file renderers per Phase E (Aider, Windsurf, OpenClaw, Kimi).
9. Antigravity — *blocked on upstream* removing the non-deterministic `date_added`.

### Platform / packaging

10. Windows code signing — *blocked on a paid cert*.
11. Native runtime verification on Windows/Linux VMs (Phase C remainder).

### Accessibility (pre-existing)

12. Bulk-delete dialog focus management.
13. `role=menu` keyboard navigation.

### Longer horizon

14. Local-runtime system-prompt target (Ollama / LM Studio).

## Quality Gates

Before release:

```sh
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run check
npm run build
npm run build:phase-c
```

Renderer parity should be checked against the active AA clone:

```sh
AGENCY_AGENTS_PARITY_ROOT=/Users/michael/Software/AgentLand/agency-agents \
cargo test --manifest-path src-tauri/Cargo.toml upstream_convert_sh_is_byte_identical_for_transform_tools -- --ignored
```

## Definition Of Done For 1.0

- public docs describe Agency Agents, not the inherited source app
- app name, bundle ID, updater host, and release artifacts are consistent
- supported install paths have primary-source verification
- renderer parity passes for supported transform tools
- uninstall is recoverable for modified files
- macOS signed build is verified
- Windows/Linux builds are produced or explicitly marked unavailable
- Memory Bank task docs are updated after human approval
