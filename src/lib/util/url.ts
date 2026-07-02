/**
 * URL safety helpers — the single source of truth for what the renderer is
 * allowed to hand off to `tauri-plugin-opener` / `window.open`.
 *
 * Why this exists: cask/formula `homepage` strings are attacker-influenced
 * metadata (poisoned tap, compromised upstream JSON). `tauri-plugin-opener`
 * resolves to macOS `open(1)`, which honors any registered URL scheme —
 * `file://`, `vscode://`, `slack://`, `mailto:`, etc. The scheme allowlist
 * here is the only thing standing between malicious cask metadata and a
 * scheme-handler escape.
 *
 * Security audit §H1 (memory-bank/security.md) — keep this helper centralized
 * so every future caller picks up the same defense automatically.
 */
import { i18n } from "$lib/i18n.svelte";
import { toast } from "$lib/stores/toast.svelte";

/** Schemes we will actually pass to the opener. Intentionally narrow. */
const ALLOWED_PROTOCOLS = new Set(["http:", "https:"]);

/**
 * Parse + scheme-check a URL string. Returns the normalized URL string when
 * safe, or `null` (and toasts an error) when it isn't. Never throws.
 *
 * `null` callers should bail without further fallback — the toast already
 * communicated the rejection.
 */
export function classifyUrl(url: string): { ok: true; url: string } | { ok: false; reason: string } {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return { ok: false, reason: i18n.t("error.invalidUrl") };
  }
  if (!ALLOWED_PROTOCOLS.has(parsed.protocol)) {
    return { ok: false, reason: i18n.t("error.refuseProtocol", { protocol: parsed.protocol }) };
  }
  return { ok: true, url: parsed.toString() };
}

/**
 * Open a URL via `tauri-plugin-opener`, falling back to `window.open` only
 * when the opener throws. Rejects (and toasts) anything that isn't `http(s):`.
 *
 * Use this everywhere instead of importing `openUrl` directly.
 */
export async function safeOpenUrl(url: string): Promise<void> {
  const classified = classifyUrl(url);
  if (!classified.ok) {
    toast.error(classified.reason);
    return;
  }
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(classified.url);
  } catch {
    // Browser/web fallback for non-Tauri contexts (dev tooling, tests).
    // The URL has already passed the scheme allowlist, so this is safe.
    window.open(classified.url, "_blank");
  }
}
