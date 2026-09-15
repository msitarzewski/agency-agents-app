# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Workflow rules (summary of AGENTS.md)

AGENTS.md is the full cross-tool workflow guide. The binding parts:

- **Approval gates.** Propose a plan (cite `file:line`) and wait for approval before building. After building, present the diff and QA results, then wait for explicit approval ("approved", "looks good", "ship it") before applying, committing, or writing docs.
- **Branch, never main.** Do the work on a branch or in a temp clone.
- **Reuse before creation.** Before adding a file, search for something to extend and say why it can't be extended. Refactor incrementally rather than rewriting.
- **No fake data or stubs** in production code, and never ignore failing tests. Fix root causes; don't add defensive workarounds. Test fixtures are fine.
- **Memory Bank writes need approval**: task docs under `memory-bank/tasks/`, monthly READMEs, and `decisions.md`. Keep `activeContext.md` updated at state transitions so work can resume after a context compaction (entry points are listed at the end of this file).

## Commands

```sh
npm install
npm run tauri dev                 # run the desktop app (Vite dev server + Rust backend)
npm run check                     # svelte-kit sync + svelte-check — the only frontend type/lint gate (no JS test suite)
npm run build                     # static SPA build (adapter-static, index.html fallback)

cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --lib                       # all Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml --lib <name_substring>      # single test, e.g. require_network_blocks_when_paranoid_on
```

Plain `cargo` works without the Tauri CLI because `.cargo/config.toml` sets a fallback `TAURI_CONFIG`, which disables the macOS private API. Always pass `--lib`. `src-tauri/tests/integration_brew.rs` is a leftover from brew-browser (this app's ancestor), shells out to `brew`, and has nothing to do with agents.

Renderer parity vs. the upstream converter (ignored by default; needs any local `agency-agents` clone):

```sh
AGENCY_AGENTS_PARITY_ROOT=/path/to/agency-agents \
cargo test --manifest-path src-tauri/Cargo.toml upstream_convert_sh_is_byte_identical_for_transform_tools -- --ignored
```

`npm run build:phase-c` runs the local QA batch (`tools/phase-c/phase-c.sh`, report under `.phase-c/runs/<ts>/`). Its defaults assume the maintainer's macOS paths; override with `AGENCY_AGENTS_PARITY_ROOT`, `PHASE_C_SKIP_MAC_BUILD=1`. Release builds: `docs/BUILD.md`, `scripts/release.sh` (macOS, signed), `scripts/release-linux.sh` (Docker). CI (`.github/workflows/`) builds Linux and Windows with `createUpdaterArtifacts: false`. CI only runs `tauri build`; it does **not** run `cargo test`, `cargo fmt --check`, or `npm run check`. Run all three locally before a PR (CONTRIBUTING.md § Tests).

## Architecture

Tauri 2 shell, SvelteKit + Svelte 5 (runes) SPA frontend, Rust backend that owns all state and file I/O. The app installs agent personas from the upstream [`agency-agents`](https://github.com/msitarzewski/agency-agents) catalog into other AI tools and keeps the install ledger those tools lack. It never executes agents and never shells out to converter scripts at runtime.

### IPC boundary

- Every command is registered in `tauri::generate_handler!` in `src-tauri/src/lib.rs`. Settings/GitHub/updater commands live in `src-tauri/src/commands/`; catalog and install commands live in their domain modules and are registered fully qualified (`corpus::*`, `install::*`).
- DTOs are defined in `src-tauri/src/types.rs` with `#[serde(rename_all = "camelCase")]` and **mirrored by hand** in `src/lib/types.ts`. Change both together. `memory-bank/contracts.md` is the canonical wire-format spec.
- Frontend calls go through typed `invoke()` wrappers (`src/lib/api.ts` and store modules). Wrappers throw an `AppErrorPayload`; narrow with `isAppError(e)` from `types.ts`.
- State lives in rune-based stores `src/lib/stores/*.svelte.ts`; components in `src/lib/components/`. Native menu items emit Tauri events that stores listen for.
- Adding backend behavior: types.rs → module logic → register in lib.rs → types.ts → store/UI → tests (see CONTRIBUTING.md).

### Catalog → render → install pipeline

1. **Corpus** (`src-tauri/src/corpus/`): catalog source is a bundled baseline (`src-tauri/resources/corpus-baseline/`, used for offline first run), a managed clone at `~/.agency-agents`, or a user-selected clone. The division list (which is also the set of directories that get indexed) is the key set of the catalog's `divisions.json`. If the catalog has no `divisions.json`, the code falls back to the bundled mirror `src-tauri/data/agency-categories.json`. `AGENT_DIRS` in `convert.sh` is parsed only to filter tarball extraction. Indexing is recursive (nested `<category>/<sub>/<slug>.md`). Don't hard-code agent counts or categories.
2. **Render** (`src-tauri/src/render/mod.rs`): per-tool deterministic Rust ports of upstream `scripts/convert.sh`. Output must be **byte-identical** to the upstream converter, because `rendered_hash` is what reconciliation compares against.
3. **Install** (`src-tauri/src/install/mod.rs`): writes, ledger, detection, reconciliation, projects, and loadout import/export. Each install records `source_hash` + `rendered_hash`. Reconciliation classifies files on disk as Current / Outdated / Modified / Removed / Foreign by re-rendering and hash-matching. **Provenance is hash-match only: never stamp or mutate installed files.** Write atomically, back up divergent files before destructive ops, and only write to approved destinations.

### Tool registry: upstream is the source of truth

`src-tauri/data/tools.json` is a verbatim mirror of the catalog repo's `tools.json`. It is embedded by `src-tauri/src/registry.rs` (`include_str!`) **and** imported directly by `src/lib/data/toolRegistry.ts`. Whether a tool is installable is derived from whether a renderer exists for its format; everything else is shown as "recognized-only". Changing how an existing tool installs (format, paths, slug prefix, frontmatter, scope) is an **upstream change first**: land it in the catalog's `tools.json` + `convert.sh`, then sync the mirrors here. Otherwise the next catalog sync silently reverts it.

### Network gating

Every outbound path must call `AppState::require_network(feature)` (`src-tauri/src/state.rs`), which blocks when Offline Mode (`paranoid_mode`) is on or settings are corrupt. This applies even to paths that a Tauri plugin could otherwise reach on its own (the updater). New network hosts need discussion first; the app ships no telemetry.

### Discuss before doing

Per CONTRIBUTING.md § PR Guidance: new production dependencies, new top-level UI surfaces, installer architecture changes, multi-file renderer support, signing/updater/release pipeline changes, and telemetry/accounts/sync features.

### Other cross-cutting details

- **Updater**: the minisign public key lives in both `UPDATER_PUBKEY` (`src-tauri/src/lib.rs`) and `tauri.conf.json`; keep them in sync. The app version comes from `Cargo.toml` via `app_version`, not `package.json`.
- **i18n**: `src/lib/i18n/locales/en.ts` is the source of truth (defines `MessageKey`); other locales are `satisfies Partial<Messages>`, registered in `src/lib/i18n/messages.ts`. Only app chrome is translated, never catalog content. Locale changes need key and placeholder parity, plus a human content scan (warnings and destructive confirmations must keep their severity).
- **Linux**: `lib.rs` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` unless the user already set it (EGL crash workaround).

## Memory Bank: actual entry points

The Memory Bank layout described in AGENTS.md is generic. This repo's real files are `memory-bank/NEXT-SESSION.md` (read first), `activeContext.md`, `agentLog.md` (append-only history), `contracts.md` (IPC contracts), `decisions.md`, `projectbrief.md`, `phases/`, and `tasks/`. `systemPatterns.md` §1–5 (corpus model, indexes, renderer, reconciliation states, scopes) is accurate. Its §6–8 module map is the original brew-browser-derived plan and does **not** match the code: there are no `ledger/`, `reconcile/`, or `catalog/` modules; that logic lives in `install/` and `corpus/`.
