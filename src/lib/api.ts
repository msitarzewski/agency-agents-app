/**
 * Typed `invoke()` wrappers for the agency backend command surface.
 *
 * Convention: each function resolves with the typed result, or *throws* a
 * `AppErrorPayload`-shaped object on backend error. Callers should use
 * `try/catch` and `isAppError(e)` to narrow.
 *
 * Covers the cross-cutting infrastructure the agency shell relies on:
 * app version, settings persistence, GitHub integration, and the in-app
 * updater. (Agent catalog/install commands live in their own modules —
 * `corpus` / `install`.)
 */

import { invoke } from "@tauri-apps/api/core";

import type {
  CreatedIssue,
  DeviceFlowPoll,
  DeviceFlowStart,
  GithubStatus,
  RepoStats,
  Settings,
  UpdateCheckOutcome,
} from "./types";

// ============================================================
// App version (from tauri::App::package_info)
// ============================================================

/**
 * App version string from `tauri::App::package_info()` — the source of
 * truth is `Cargo.toml` (mirrored by `tauri.conf.json`). Cheaper and
 * more honest than reading `package.json` from the renderer.
 */
export function appVersion(): Promise<string> {
  return invoke<string>("app_version");
}

// ============================================================
// Settings persistence
// ============================================================

/**
 * Read the currently-loaded settings.
 *
 * Throws a `AppErrorPayload` with `code === "internal"` when the
 * settings file on disk is unparseable — in that case the backend is
 * already failing closed (`require_network` denies all outbound calls
 * until the user resets). The Settings UI should catch the throw and
 * show a "Settings file unreadable — Reset to defaults?" affordance
 * that calls `settingsReset()`.
 */
export function settingsGet(): Promise<Settings> {
  return invoke<Settings>("settings_get");
}

/**
 * Persist a complete settings object. Returns the canonicalized
 * settings (numerics clamped, etc.) so the caller can re-broadcast the
 * authoritative values to the store.
 */
export function settingsSet(settings: Settings): Promise<Settings> {
  return invoke<Settings>("settings_set", { settings });
}

/**
 * Overwrite `settings.json` with defaults. Used by the "Reset to
 * defaults" button in Settings → Network when the file is corrupt or
 * the user wants to start fresh.
 */
export function settingsReset(): Promise<Settings> {
  return invoke<Settings>("settings_reset");
}

// ============================================================
// GitHub integration
// ============================================================

/**
 * Fetch repo stats for `homepage`. Returns `null` when the user hasn't
 * enabled GitHub stats, the URL doesn't parse as a github repo, or the
 * repo 404s.
 *
 * Throws `AppErrorPayload` with `code === "paranoid_mode_blocked"`
 * when paranoid mode is on, or `"github_rate_limited"` on the anonymous
 * 60/hr per-IP cap.
 */
export function githubRepoStats(homepage: string): Promise<RepoStats | null> {
  return invoke<RepoStats | null>("github_repo_stats", { homepage });
}

/**
 * Read the current sign-in status. Reads from the macOS Keychain only —
 * no network call. The DTO contains `{ signedIn, username, scopes }`,
 * never the token.
 */
export function githubStatus(): Promise<GithubStatus> {
  return invoke<GithubStatus>("github_status");
}

/**
 * Begin a GitHub Device Flow sign-in. POSTs to
 * `github.com/login/device/code` and returns the user code +
 * verification URI to show in the DeviceFlowModal. Subject to the
 * paranoid-mode gate.
 */
export function githubSigninStart(): Promise<DeviceFlowStart> {
  return invoke<DeviceFlowStart>("github_signin_start");
}

/**
 * Poll the token endpoint once with the opaque `deviceCode` returned
 * by `githubSigninStart`. Caller drives the polling loop using the
 * `interval` from the start response.
 */
export function githubSigninPoll(deviceCode: string): Promise<DeviceFlowPoll> {
  return invoke<DeviceFlowPoll>("github_signin_poll", { deviceCode });
}

/**
 * Delete the stored OAuth token (and cached username/scopes) from the
 * macOS Keychain. Idempotent.
 */
export function githubSignout(): Promise<void> {
  return invoke<void>("github_signout");
}

/**
 * Star the repo whose URL matches `homepage`. The backend validates
 * the URL is `github.com/<owner>/<repo>` before any network call.
 */
export function githubStar(homepage: string): Promise<void> {
  return invoke<void>("github_star", { homepage });
}

/** Unstar — idempotent on the GitHub side. */
export function githubUnstar(homepage: string): Promise<void> {
  return invoke<void>("github_unstar", { homepage });
}

/**
 * Check whether the signed-in user has starred `homepage`. Backend maps
 * 204 → true, 404 → false.
 */
export function githubIsStarred(homepage: string): Promise<boolean> {
  return invoke<boolean>("github_is_starred", { homepage });
}

/** Watch the repo (`subscribed: true, ignored: false`). */
export function githubWatch(homepage: string): Promise<void> {
  return invoke<void>("github_watch", { homepage });
}

/** Stop watching — idempotent. */
export function githubUnwatch(homepage: string): Promise<void> {
  return invoke<void>("github_unwatch", { homepage });
}

/**
 * File an issue against the repo. Backend sanitises and caps title,
 * body, and labels. Returns the new issue's `{ number, htmlUrl }`.
 */
export function githubCreateIssue(
  homepage: string,
  title: string,
  body: string,
  labels: string[],
): Promise<CreatedIssue> {
  return invoke<CreatedIssue>("github_create_issue", {
    homepage,
    title,
    body,
    labels,
  });
}

// ============================================================
// In-app updater
// ============================================================

/**
 * Check the manifest for a newer release. Backend handles the version
 * comparison, the skip-list consultation, and the URL allowlist.
 *
 * Throws `AppErrorPayload` with `code === "paranoid_mode_blocked"`
 * (feature: "update_check") when Offline Mode is on.
 */
export function updateCheckNow(): Promise<UpdateCheckOutcome> {
  return invoke<UpdateCheckOutcome>("update_check_now");
}

/**
 * Download, verify, and install the named version. The backend
 * cross-checks `version` against the cached "available" entry from the
 * most recent `update_check_now` call.
 */
export function updateInstall(version: string): Promise<void> {
  return invoke<void>("update_install", { version });
}

/**
 * Add `version` to the skip-list so the title-bar indicator stops
 * surfacing for this release.
 */
export function updateSkip(version: string): Promise<void> {
  return invoke<void>("update_skip", { version });
}

/**
 * Restart the running process so the freshly-installed .app picks up.
 * Called from the "Relaunch now" affordance after `updateInstall`.
 */
export function updateRelaunch(): Promise<void> {
  return invoke<void>("update_relaunch");
}
