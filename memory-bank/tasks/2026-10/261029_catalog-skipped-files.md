# 261029_catalog-skipped-files

## Objective
Tell the user when the catalog has files the app can't read, instead of a count that is quietly short (#79: a
user saw 57 of 58 engineering agents because one agent's frontmatter had an unquoted colon).

## Outcome
- ✅ The indexer records every file under the divisions that looks like an agent but isn't indexed, with a reason:
  unreadable or over 1 MB, not UTF-8, frontmatter that never closes, invalid YAML, no `name`, or a file name (slug)
  an earlier file already took. Files with no frontmatter (READMEs, plain docs) are still not agents and not listed.
- ✅ Invalid YAML keeps the parser's own message, with line numbers that match the file (the frontmatter is
  re-parsed after a leading newline on failure only, so the normal path is one parse).
- ✅ A duplicate slug keeps the first file; before, the agent list showed both while the index kept the last.
- ✅ `CatalogStatus.skipped` carries the list. Settings → Catalog shows "Couldn't read: N files" and each file with
  its reason; the sidebar shows a second, amber line ("1 file couldn't be read") that opens Settings → Catalog.
- ✅ Catalog status now loads at startup (`+layout.svelte`): the app opens on Agents, not the Dashboard, so
  otherwise most people would never see the warning. Local only: a few `git` commands, no network.
- ✅ User approved the plan, the two changes made during the build (startup load, two-line sidebar), and the result.

## Files Modified
- `src-tauri/src/corpus/parse.rs` — `split_frontmatter` tells "no frontmatter" from "unclosed"; `parse_agent`
  returns a `ParseError` with a `SkipReason`. Two tests now assert "reported" instead of "silently not an agent".
- `src-tauri/src/corpus/mod.rs` — `build_from_dir` collects `skipped`; `Corpus::skipped()`; `catalog_status`.
- `src-tauri/src/types.rs`, `src/lib/types.ts` — `SkipReason`, `SkippedFile`, `CatalogStatus.skipped`.
- `src/lib/components/SettingsSectionCatalog.svelte`, `src/lib/components/Sidebar.svelte`,
  `src/lib/i18n/locales/en.ts` (English only; other locales fall back), `src/routes/+layout.svelte`.

## Validation
- `cargo test --lib`: 292 passed (4 new). `build_reports_files_it_cannot_index` builds a temp catalog with one
  file per reason plus a README and a plain doc; the bundled baseline must report nothing.
- `cargo fmt --check` clean; `npm run check` 0 errors, 0 warnings.
- In the running app (`npm run tauri dev` with `HOME` pointed at a temp folder, so the real app data was never
  touched), on a catalog checkout at `c89557f` (07-30): exactly one file skipped,
  `engineering/engineering-developer-tooling-engineer.md`, "mapping values are not allowed in this context at
  line 3 column 132" — line 3, column 132 is the colon in "great DX: intuitive". Sidebar and Settings confirmed
  by the user's screenshot.

## Notes
- The dev build shares the installed app's data folder (`com.zerologic.agency-agents-app`); there is no override.
  Run it with `HOME=<temp>` (plus real `CARGO_HOME`/`RUSTUP_HOME`/`npm_config_cache`) and seed
  `<temp>/Library/Application Support/com.zerologic.agency-agents-app/state/catalog.json`. The web view's
  storage is not under `HOME`, so registered projects still show (read only).
- `npm run tauri dev` on macOS adds `macos-private-api` to `src-tauri/Cargo.toml`; revert it before committing.
