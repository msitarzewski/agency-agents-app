/**
 * "Report to Agency Agents" — surfaces a pre-filled GitHub new-issue URL
 * from any error context the user encounters in the app.
 *
 * Entry point:
 *
 *   - `reportableToastError(title, error)` — for catch blocks. Shows a
 *     `toast.error` with the friendly message in the body AND a
 *     "Report to Agency Agents" action button below it. One-call upgrade
 *     of the old `toast.error(title, isAppError(e) ? e.code : String(e))`
 *     anti-pattern (which threw away the friendly message and gave the
 *     user no recourse beyond the raw discriminator string).
 *
 * The opened URL routes through `safeOpenUrl`, so the http(s)-only +
 * Tauri-opener sandbox apply identically to GitHub as to any other
 * outbound link.
 */

import { appVersion } from "$lib/api";
import { toast } from "$lib/stores/toast.svelte";
import {
  appErrorMessage,
  isAppError,
  type AppErrorPayload,
} from "$lib/types";
import { safeOpenUrl } from "$lib/util/url";

const REPO_NEW_ISSUE_URL =
  "https://github.com/msitarzewski/agency-agents-app/issues/new";

/** Cap on the stderr excerpt we include in the pre-filled body. Keeps
 *  the resulting URL well under GitHub's ~8 KiB limit even with the
 *  rest of the templated context. */
const STDERR_MAX_CHARS = 2000;

interface ReportContext {
  /** User-facing summary (becomes the issue title's suffix). */
  summary: string;
  /** Command that failed (when available). */
  command?: string;
  /** Process exit code (when available). */
  exitCode?: number;
  /** Tail of stderr output, capped to {@link STDERR_MAX_CHARS}. */
  stderrExcerpt?: string;
  /** Backend-generated friendly message, when the error matched a known
   *  upstream pattern. Surfaces in the body so the maintainer knows the
   *  user already saw the friendly version. */
  friendlyMessage?: string;
  /** Raw AppError discriminator (e.g. "internal"). Helps
   *  triage when the user types unrelated free-form text. */
  errorCode?: string;
}

/** Cached app version. `appVersion()` is cheap but does cross the IPC
 *  boundary; we only need to resolve it once per session. */
let cachedAppVersion: string | null = null;
async function getAppVersion(): Promise<string> {
  if (cachedAppVersion !== null) return cachedAppVersion;
  try {
    cachedAppVersion = await appVersion();
  } catch {
    cachedAppVersion = "unknown";
  }
  return cachedAppVersion;
}

/** Build the issue body (markdown). */
function buildBody(ctx: ReportContext, appVer: string): string {
  const lines: string[] = [
    `**Agency Agents:** ${appVer}`,
  ];
  if (ctx.errorCode) lines.push(`**Error code:** \`${ctx.errorCode}\``);
  if (ctx.command) lines.push(`**Command:** \`${ctx.command}\``);
  if (ctx.exitCode !== undefined) lines.push(`**Exit code:** ${ctx.exitCode}`);
  if (ctx.friendlyMessage) {
    lines.push("", `**Friendly message shown to user:**`, "", `> ${ctx.friendlyMessage}`);
  }
  if (ctx.stderrExcerpt && ctx.stderrExcerpt.trim().length > 0) {
    const trimmed =
      ctx.stderrExcerpt.length > STDERR_MAX_CHARS
        ? "…(truncated)…\n" + ctx.stderrExcerpt.slice(-STDERR_MAX_CHARS)
        : ctx.stderrExcerpt;
    lines.push("", "**stderr excerpt:**", "", "```", trimmed, "```");
  }
  lines.push(
    "",
    "---",
    "",
    "_Replace this line with what you were doing when the error appeared, and what you expected to happen._",
  );
  return lines.join("\n");
}

/** Open the GitHub new-issue page pre-filled with the supplied context. */
export async function openReportIssue(ctx: ReportContext): Promise<void> {
  const appVer = await getAppVersion();

  const params = new URLSearchParams();
  params.set("title", `[Agency Agents] ${ctx.summary}`);
  params.set("body", buildBody(ctx, appVer));
  params.set("labels", "from-app");

  await safeOpenUrl(`${REPO_NEW_ISSUE_URL}?${params.toString()}`);
}

/** Extract a ReportContext from a typed AppError. */
export function reportContextFromError(
  e: AppErrorPayload,
  summary: string,
): ReportContext {
  // Capture the friendly message so the report carries human-readable
  // context, and pin the discriminator for triage.
  return {
    summary,
    friendlyMessage: appErrorMessage(e),
    errorCode: e.code,
  };
}

/**
 * Drop-in replacement for the old anti-pattern:
 *   toast.error(title, isAppError(e) ? e.code : String(e))
 *
 * Renders the friendly message in the toast body AND attaches a
 * "Report to Agency Agents" action button that opens the pre-filled
 * GitHub new-issue URL via `safeOpenUrl`.
 *
 * The friendly message comes from `appErrorMessage(e)`, which maps each
 * `AppError` discriminator to human-readable text.
 */
export function reportableToastError(title: string, e: unknown): void {
  if (isAppError(e)) {
    const ctx = reportContextFromError(e, title);
    toast.error(title, appErrorMessage(e), {
      label: "Report to Agency Agents",
      onClick: () => {
        void openReportIssue(ctx);
      },
    });
    return;
  }
  const stringified = String(e);
  toast.error(title, stringified, {
    label: "Report to Agency Agents",
    onClick: () => {
      void openReportIssue({
        summary: title,
        stderrExcerpt: stringified,
      });
    },
  });
}
