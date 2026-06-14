# 260614_phase-c-parity-safety

## Objective
Close the two IMMEDIATE backlog items and the deferred Phase C chrome on the
`codex/renderer-parity-safety-phase-c` branch: (1) verify renderer parity vs the upstream
`scripts/convert.sh` for transform tools (load-bearing for the `current`/Diff/Update state model),
(2) resolve the open uninstall-backup decision, (3) ship Windows/Linux titlebar degradation.

## Outcome
- ✅ Renderer parity: **232 agents × 5 transform tools = 1160/1160 byte-identical** vs the live converter.
- ✅ Uninstall safety: backup-first pass; modified files recoverable, byte-identical need none, backup
  failure aborts the delete.
- ✅ Cross-platform chrome: Tauri config split (base + macOS override).
- ✅ Tests: cargo 258 passing / 0 failed (+1 `--ignored` parity test, 1/0). svelte-check 0 errors.
  vite build clean.
- ✅ Review: approved by Michael (commit/push/PR authorized).

## Files Modified
- `src-tauri/src/render/mod.rs` — `source_field` (mirrors `lib.sh#get_field`, literal field:value
  between `---` fences, preserves quotes — not YAML), `source_body` (mirrors `body="$(get_body)"`: awk
  one-newline-per-line + command-substitution trailing-newline strip + heredoc single newline),
  `slugify` (mirrors `lib.sh#slugify`), `output_slug` (converter filename rules: identity tools keep
  source filename, transform tools derive from frontmatter `name`), Qwen optional `tools` line literal,
  + the `--ignored` `upstream_convert_sh_is_byte_identical_for_transform_tools` test.
- `src-tauri/src/install/mod.rs` — `remove_agent_files` backup-first pass, `backup_if_differs`,
  `remove_file_strict`, `candidate_dests`, `write_agent_files_to(backup_dir)`; 6 uninstall-safety tests.
- `src-tauri/tauri.conf.json` — cross-platform-safe base (`decorations: true`, opaque, no macOS-only keys).
- `src-tauri/tauri.macos.conf.json` (NEW) — macOS override: `macOSPrivateApi`, `transparent`,
  `titleBarStyle: Overlay`, `hiddenTitle`, `trafficLightPosition`.
- `src-tauri/src/lib.rs` — finished brew-browser → Agency Agents rename (env filter, menu event ids,
  updater key paths, doc header).
- `src-tauri/src/commands/settings.rs`, `src/lib/types.ts` — dead brew `Settings` fields purged.
- Frontend: `TitlebarControls.svelte`, `ToolsView.svelte`, `AgentsWorkspace.svelte`, `ResizeHandle.svelte`,
  `stores/{activity,ui}.svelte.ts`, `util/token.ts`, `routes/+page.svelte` — degradation path + rename.
- `tools/phase-c/{phase-c.sh,validate-config.mjs}` (NEW) — Phase C validation runner
  (`npm run build:phase-c[:full]`).
- Docs: README, CONTRIBUTING, SECURITY, BUILD, PHILOSOPHY, PLAN rewritten; stale `release-notes/0.3.0–
  0.5.0` removed; tool READMEs trimmed; Android icons regenerated.

## Decisions Locked
- **Uninstall = recoverable ✕.** Backup-first; modified→backup, byte-identical→none, backup-fail→abort.
- **Parity is the contract.** Rust `render/` must stay byte-identical to `scripts/convert.sh`; the
  `--ignored` test enforces it against the live catalog clone (re-run after any converter or catalog change).
- **Platform chrome via config split** (not `#[cfg]`): `tauri.macos.conf.json` merges over the base.

## Verification
`AGENCY_AGENTS_PARITY_ROOT=/Users/michael/Software/AgentLand/agency-agents cargo test --manifest-path
src-tauri/Cargo.toml --lib upstream_convert_sh_is_byte_identical_for_transform_tools -- --ignored
--nocapture` → `renderer parity: 232 agents, 1160 byte comparisons … ok`. Or `npm run build:phase-c`.

## Artifacts
- Branch: `codex/renderer-parity-safety-phase-c`
- PR: (linked on creation)
