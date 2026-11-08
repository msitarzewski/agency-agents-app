# Browser IPC mock

Dev-only shim that makes the frontend runnable in a plain browser. `npm run dev`
→ <http://localhost:1430> → the real catalog, with no Rust process behind it.

## How it works

- `src/hooks.client.ts` installs the mock only when `import.meta.env.DEV` **and**
  no `window.__TAURI_INTERNALS__` exists. `npm run tauri dev` is untouched;
  production builds drop the branch, so no fixture ships.
- `@tauri-apps/api/mocks` provides the internals contract (`invoke`,
  `transformCallback`, the event plugin). `handlers.ts` answers the command
  surface with the same camelCase JSON the Rust commands return.
- `corpus.ts` parses `src-tauri/resources/corpus-baseline/**/*.md` at build time
  with the same rules as `src-tauri/src/corpus/parse.rs`. Division labels, icons
  and colours come from `src-tauri/data/agency-categories.json`, and destinations
  from `src-tauri/data/tools.json` — no catalog truth is duplicated here.
- `state.ts` keeps `settings.json` and the install ledger in `localStorage`, so
  install/uninstall/settings survive a reload.

## What it cannot do

Reading or writing real files, probing installed tool binaries, git, the macOS
Keychain, GitHub OAuth and the signed update channel. Those commands reject with
an `AppError`-shaped object naming the limitation, instead of pretending to
succeed. Consequences worth knowing:

- Install ledger starts empty and lives in `localStorage`. Tool detection cannot
  probe binaries from a browser, so every tool the app can render reads as
  present (otherwise the install matrix would have no targets); tools it can only
  recognise stay "not installed".
- Install states are always `current` — there is no disk to drift from.
- Agentfile export/import, catalog switching and update installs error out.
- File dialogs return the synthetic path `browser-mock/projects/demo-site`.
- Runbooks are empty: `strategy/` only ships in a synced clone.

Append `?tauriMock=0` to the URL to go back to the bare "no backend" state.
