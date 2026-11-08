import type { ClientInit } from "@sveltejs/kit";

/**
 * Client bootstrap — installs the browser IPC mock when there is no Tauri shell.
 *
 * The frontend is a desktop app: every screen reads through `invoke()`. Running
 * `npm run dev` and opening `http://localhost:1430` in a browser therefore used
 * to dead-end on "Corpus unavailable" (0 agents) because
 * `window.__TAURI_INTERNALS__` does not exist outside the WebView.
 *
 * The mock is dev-only and is skipped when a real runtime is present, so
 * `npm run tauri dev` is unaffected. See `$lib/mock` for what it can and cannot
 * do.
 */
export const init: ClientInit = async () => {
  // Production builds must never answer IPC from fixtures — AGENTS.md: no mock
  // data in shipped code. `DEV` is statically false there, so this branch (and
  // the module it imports) is dropped at build time.
  if (!import.meta.env.DEV) return;

  // A real Tauri WebView — its IPC is the source of truth, never shadow it.
  const internals = (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  if (internals) return;

  const { installBrowserIpcMock } = await import("$lib/mock");
  installBrowserIpcMock();
};
