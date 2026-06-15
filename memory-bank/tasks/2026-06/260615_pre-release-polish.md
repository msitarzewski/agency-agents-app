# 260615_pre-release-polish

## Objective
On `release-planning`: document the v0.1.0 release plan (not cutting yet), then knock out final
pre-release issues — brew vestiges, a permanently-empty inherited UI surface, a cold-build test gate,
and a Tools-pane filter.

## Outcome
- ✅ Release runbook in `docs/BUILD.md#Release Checklist` + ADR (manual signed DMG, SKIP_UPDATER, auto-update deferred).
- ✅ brew vestige cleanup: error-type rename (`BrewError*`→`AppError*`), dead `catalogAutoRefresh` removed,
  dead error codes removed, brew-era Python pipeline (`tools/{catalog,categorize,enrich,pipeline,trending-collector}`) deleted.
- ✅ **Activity Journal** — repurposed the inherited empty streaming "Activity" into a clearable journal of
  install/uninstall/update/track/bulk/switch actions. Built via a Workflow (planner→builder→Code-Reviewer+UX-Architect→fix loop); 3 UX findings hand-polished.
- ✅ **Tools pane lens** — default to Installed (detected/in-use); `Installed · Not installed · All` toggle.
- ✅ Cold `cargo test` tauri feature-gate fix (`.cargo/config.toml`); `tauri dev` verified clean.
- ✅ Green: svelte-check 0 errors, cargo 258/0. Committed + pushed (NOT merged; release not cut).

## Files
- Removed: `tools/{catalog,categorize,enrich,pipeline,trending-collector}/*`, `src/lib/components/ActivityDrawer.svelte`.
- New: `.cargo/config.toml`.
- Changed: `activity.svelte.ts`, `install.svelte.ts`, `ActivityHistory.svelte`, `ToolsView.svelte`, `types.ts`,
  `reportIssue.ts`, `api.ts`, `{github,settings,updater}.svelte.ts`, `Sidebar.svelte`, `+page.svelte`,
  `categoryIcon.ts`, `commands/settings.rs`, `error.rs`, `Cargo.toml`, `.gitignore`, `docs/BUILD.md`.

## Decisions
- `decisions.md` (2026-06-15): repurpose Activity → journal; `.cargo/config.toml` tauri-gate approach.

## Lessons
- Triage before delete: I first wrongly flagged the brew `tools/` pipeline as "live"; it fetches Homebrew formulae, unused by AA.
- Gate verify→fix loops on `major`+, not just `blocker` — the UX-Architect's `major` findings slipped the loop.

## Artifacts
- Branch: `release-planning` (pushed; not merged — release not cut).
