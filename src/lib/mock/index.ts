/**
 * Browser IPC mock — lets the Svelte frontend run at `localhost:1430` in a plain
 * browser (VS Code's Simple Browser, Chrome, Safari) with a working catalog.
 *
 * Why it exists: every screen loads through `invoke()` against the Rust backend.
 * Without a Tauri runtime `window.__TAURI_INTERNALS__` is undefined, `listen()`
 * throws, and the Agents pane shows "Corpus unavailable" with zero agents — the
 * UI is unreachable for design review or screenshot work.
 *
 * How: `@tauri-apps/api/mocks` installs the full internals contract
 * (`invoke`, `transformCallback`, callback registry, event plugin) and
 * `./handlers` answers the command surface with the same JSON the Rust commands
 * return, read from the bundled baseline corpus.
 *
 * Scope: dev-only. Installed from `src/hooks.client.ts` behind
 * `import.meta.env.DEV`, so production builds never reference this module.
 */

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

import { MOCK_AGENTS } from "./corpus";
import { createMockInvokeHandler } from "./handlers";

let active = false;

/** True once the mock owns `window.__TAURI_INTERNALS__.invoke`. */
export function isBrowserIpcMockActive(): boolean {
  return active;
}

/**
 * Install the mock. Returns whether it is active.
 *
 * Callers must have already established that no Tauri runtime is present —
 * shadowing a real backend's IPC would make the native window lie about state.
 */
export function installBrowserIpcMock(): boolean {
  if (active) return true;
  if (typeof window === "undefined") return false;

  // Escape hatch: `?tauriMock=0` restores the bare "no backend" behaviour, which
  // is how the app's own empty/error states were reviewed before this existed.
  if (new URLSearchParams(window.location.search).get("tauriMock") === "0") return false;

  // Must run before anything touches `@tauri-apps/api/window`.
  mockWindows("main");

  const handle = createMockInvokeHandler();
  mockIPC((command, args) => handle(command, (args ?? {}) as Record<string, unknown>), {
    // Routes plugin:event|listen / emit / unlisten through the mock, so the
    // native-menu listeners in +layout.svelte resolve instead of rejecting.
    shouldMockEvents: true,
  });

  active = true;
  document.documentElement.dataset.tauriMock = "browser";
  console.info(
    `[agency-agents] browser IPC mock active — ${MOCK_AGENTS.length} agents from the bundled baseline. Append ?tauriMock=0 to disable.`,
  );
  return true;
}
