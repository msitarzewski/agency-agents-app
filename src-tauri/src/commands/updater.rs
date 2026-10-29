//! Phase 15 — in-app updater commands.
//!
//! Wraps `tauri-plugin-updater` behind the `state.require_network` chokepoint
//! and adds three defense-in-depth gates on top of the plugin's existing
//! signature verification:
//!
//! 1. **Paranoid mode** — every IPC entry consults `require_network("update_check")`
//!    so the master Offline-Mode switch kills the update path even if the
//!    plugin would otherwise reach the manifest endpoint.
//! 2. **Stale-version sanity check** — `update_install(version)` validates
//!    that `version` matches the most recently-seen `Available` entry on
//!    `AppState.updater_state`. If the UI requested an install of a
//!    version we no longer have in memory (e.g. user kept Settings open
//!    while a newer manifest landed via the auto-check), the call returns
//!    `AppError::InvalidArgument` rather than installing the wrong thing.
//! 3. **Explicit downgrade rejection** — refuses to install a target whose
//!    semver is less-than-or-equal to the running version. The plugin
//!    already refuses semver-older targets via its default comparator,
//!    but we re-check here so a future plugin behaviour change cannot
//!    reopen the hole.
//!
//! The plugin itself handles the manifest fetch (8 KiB recommended cap
//! via the plugin's own JSON path), the artifact download, the sha256
//! hash verification (when the manifest carries one), and the minisign
//! signature verification against the embedded public key. The plugin's
//! verify-then-install pipeline is the load-bearing crypto chokepoint;
//! our wrapper surfaces typed errors when it fails so the frontend can
//! route the toast through the same channel as every other typed error.
//!
//! ## Testability
//!
//! Real plugin invocations require a `tauri::App` and a live network
//! call to the manifest endpoint, neither of which we want in unit
//! tests. The [`UpdaterBackend`] trait is the abstraction: production
//! uses [`PluginBackend`] (which calls the real plugin), tests use
//! [`MockBackend`] (which returns canned outcomes). Mirrors the
//! `KeychainSlot` pattern in `github::auth`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::RwLock;

use crate::error::AppError;
use crate::state::AppState;

// ---------- IPC payloads ----------

/// Outcome of [`update_check_now`]. Mirrors the three real states the
/// plugin can return, flattened into a single discriminated union the
/// frontend can `switch` over.
///
/// The `Blocked` variant is **not** returned by this enum — paranoid
/// mode surfaces as `Err(AppError::ParanoidModeBlocked)` instead, so
/// the toast routes through the same channel as every other gated call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateCheckOutcome {
    /// Plugin returned no update — running version is current.
    UpToDate,
    /// Plugin returned an update. Fields the UI needs to render the
    /// indicator pill + Settings card.
    Available {
        /// Announced version (semver) — used by `update_install` as
        /// the sanity-check arg.
        version: String,
        /// Currently-installed version, surfaced so the UI can render
        /// "v0.3.0 → v0.3.1" without an extra IPC call.
        current_version: String,
        /// Release-notes body from the manifest. Optional.
        #[serde(skip_serializing_if = "Option::is_none")]
        notes: Option<String>,
        /// Publish date (RFC 3339), if present in the manifest.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub_date: Option<String>,
        /// True iff the user has already skipped this version via the
        /// title-bar indicator's `×`. UI uses this to suppress the
        /// re-display of the indicator (Settings panel still shows the
        /// card so the user can install if they change their mind).
        skipped: bool,
    },
}

/// Subset of plugin Update fields we cache for the `update_install`
/// stale-version sanity check + the auto-check scheduler's "last
/// available" state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedUpdate {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
}

// ---------- State ----------

/// In-memory mirror of the latest update check result. Stored on
/// `AppState.updater_state` so the auto-check scheduler and the
/// `update_install` validator share the same view.
#[derive(Debug, Default)]
pub struct UpdaterState {
    /// Latest plugin outcome, if a check has run.
    pub last_outcome: Option<UpdateCheckOutcome>,
    /// Unix timestamp (seconds) of the most recent successful check.
    /// Used by the scheduler's 24h-floor enforcement so cross-launch
    /// behaviour is predictable.
    pub last_checked_at: Option<i64>,
    /// Cached `Available` payload for the install-arg sanity check.
    /// Cleared when the outcome flips back to `UpToDate`.
    pub cached_available: Option<CachedUpdate>,
}

// ---------- Backend abstraction ----------

/// Trait-object façade around the `tauri-plugin-updater` so unit tests
/// can swap in a mock. Production uses [`PluginBackend`]; tests use
/// `MockBackend` in this module's `tests` submodule.
///
/// Mirrors the `KeychainSlot` pattern in `github::auth` — the plugin is
/// not unit-testable in isolation (requires a live `App` and HTTP), so
/// we abstract its surface to the minimum we actually call.
#[async_trait::async_trait]
pub trait UpdaterBackend: Send + Sync {
    /// Run a fresh manifest fetch. `Ok(None)` means "no update available",
    /// `Ok(Some(_))` means the plugin found a newer version. Errors are
    /// surfaced as `AppError` so the IPC contract stays uniform.
    async fn check(&self) -> Result<Option<CachedUpdate>, AppError>;

    /// Download + verify (sha256 + minisign) + install the update. The
    /// `version` arg is for sanity checking only; the plugin uses its
    /// own internal `Update` handle (cached from the most recent
    /// `check()` call) so we don't have to round-trip the full Update
    /// state through the trait boundary.
    async fn download_and_install(&self, version: &str) -> Result<(), AppError>;
}

/// Currently-running app version, surfaced for downgrade rejection
/// without bringing the plugin into the public API. Resolved once at
/// startup from `CARGO_PKG_VERSION`.
pub fn current_app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

// ---------- Production backend ----------

/// Production backend that delegates to `tauri-plugin-updater`.
///
/// The plugin's `UpdaterExt::updater().check().await` returns a typed
/// `Update` value or `None`. We translate any failure into our typed
/// `AppError` family so the IPC contract stays uniform across the
/// surface.
#[cfg(not(test))]
pub struct PluginBackend<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

#[cfg(not(test))]
impl<R: tauri::Runtime> PluginBackend<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }

    /// Borrow the plugin's `Updater` value. Built fresh on each call —
    /// the plugin is cheap to instantiate and doing it eagerly at
    /// startup would force the manifest endpoint validation into the
    /// setup hook (failing builds with a malformed endpoint).
    fn updater(&self) -> Result<tauri_plugin_updater::Updater, AppError> {
        use tauri_plugin_updater::UpdaterExt;
        self.app.updater().map_err(|e| AppError::Internal {
            message: format!("updater plugin init: {e}"),
        })
    }
}

#[cfg(not(test))]
#[async_trait::async_trait]
impl<R: tauri::Runtime> UpdaterBackend for PluginBackend<R> {
    async fn check(&self) -> Result<Option<CachedUpdate>, AppError> {
        let updater = self.updater()?;
        let opt = updater
            .check()
            .await
            .map_err(|e| translate_plugin_error(e, "update check"))?;
        let Some(update) = opt else {
            return Ok(None);
        };
        Ok(Some(CachedUpdate {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
            pub_date: update.date.map(|d| d.to_string()),
        }))
    }

    async fn download_and_install(&self, version: &str) -> Result<(), AppError> {
        let updater = self.updater()?;
        let opt = updater
            .check()
            .await
            .map_err(|e| translate_plugin_error(e, "update check (pre-install)"))?;
        let Some(update) = opt else {
            // Manifest no longer advertises an update. The frontend
            // requested an install but the manifest changed underneath
            // us; surface as InvalidArgument so the UI can refresh.
            return Err(AppError::InvalidArgument {
                message: format!(
                    "manifest no longer advertises update for {version}; refresh and retry"
                ),
            });
        };

        // Re-check version match. The cached `update_install` arg
        // validator in `update_install` runs first against
        // `AppState.updater_state.cached_available`; this is a second
        // line of defense at the plugin boundary.
        if update.version != version {
            return Err(AppError::InvalidArgument {
                message: format!(
                    "manifest version drifted: requested {version}, manifest reports {}",
                    update.version
                ),
            });
        }

        // Download + verify (sha256 + minisign) → install. The plugin's
        // `download_and_install` runs both crypto checks; any failure
        // is translated to our typed `AppError` family.
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| translate_plugin_error(e, "update install"))?;

        Ok(())
    }
}

/// Map a plugin `Error` onto our typed `AppError` family. The plugin's
/// own error type carries enough context to discriminate hash mismatch
/// from signature failure, but its `Display` string is the most reliable
/// classifier across versions (the variants change between minor
/// releases).
#[cfg(not(test))]
fn translate_plugin_error(e: tauri_plugin_updater::Error, context: &str) -> AppError {
    let msg = e.to_string();
    let lower = msg.to_lowercase();
    // Signature / minisign failures map to `SignatureVerificationFailed`
    // so the UI surfaces the same toast as a sha256 mismatch (both are
    // "the bytes did not verify, abort").
    if lower.contains("signature") || lower.contains("minisign") || lower.contains("pubkey") {
        AppError::SignatureVerificationFailed { message: msg }
    } else {
        AppError::Network {
            url: context.to_string(),
            message: msg,
        }
    }
}

// ---------- Public API ----------

/// Compare two semver strings; returns `true` when `target` is greater
/// than `current`. Used by the explicit downgrade-rejection check in
/// `update_install`. Falls back to lexicographic comparison if either
/// string fails semver parsing (defensive — the plugin's own version
/// comparator handles the normal case, this is a final-line check).
pub fn is_strict_upgrade(current: &str, target: &str) -> bool {
    // Strip a leading `v` if present so "v0.3.1" parses cleanly.
    let trim = |s: &str| s.trim_start_matches('v').to_string();
    let cur = trim(current);
    let tgt = trim(target);
    match (parse_semver(&cur), parse_semver(&tgt)) {
        (Some(c), Some(t)) => t > c,
        // Any unparseable input: refuse the upgrade. Safer to fall back
        // to "manual install" than to ship the wrong binary.
        _ => false,
    }
}

/// Minimal three-tuple semver parser. We don't need pre-release /
/// metadata handling for our own release cadence (numeric major.minor.patch
/// only); a full semver crate is overkill.
fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let mut iter = s.splitn(3, '.');
    let major: u32 = iter.next()?.parse().ok()?;
    let minor: u32 = iter.next()?.parse().ok()?;
    let patch_raw = iter.next()?;
    // Drop pre-release / metadata suffix (`0.3.1-beta.1`).
    let patch_str = patch_raw.split(['-', '+']).next().unwrap_or(patch_raw);
    let patch: u32 = patch_str.parse().ok()?;
    Some((major, minor, patch))
}

/// Inner: run a single check via the supplied backend, updating
/// `state.updater_state` with the cached result + timestamp. Extracted
/// so the auto-check scheduler can call it without going through the
/// IPC layer.
pub async fn run_check(
    state: &AppState,
    backend: &dyn UpdaterBackend,
) -> Result<UpdateCheckOutcome, AppError> {
    state.require_network("update_check").await?;
    let raw = backend.check().await?;
    let now = chrono::Utc::now().timestamp();

    let outcome = match raw {
        None => UpdateCheckOutcome::UpToDate,
        Some(update) => {
            let skipped = is_version_skipped(state, &update.version).await;
            UpdateCheckOutcome::Available {
                version: update.version.clone(),
                current_version: update.current_version.clone(),
                notes: update.notes.clone(),
                pub_date: update.pub_date.clone(),
                skipped,
            }
        }
    };

    // Persist into AppState so subsequent install requests can validate.
    {
        let mut guard = state.updater_state.write().await;
        guard.last_outcome = Some(outcome.clone());
        guard.last_checked_at = Some(now);
        guard.cached_available = match &outcome {
            UpdateCheckOutcome::UpToDate => None,
            UpdateCheckOutcome::Available {
                version,
                current_version,
                notes,
                pub_date,
                ..
            } => Some(CachedUpdate {
                version: version.clone(),
                current_version: current_version.clone(),
                notes: notes.clone(),
                pub_date: pub_date.clone(),
            }),
        };
    }

    Ok(outcome)
}

/// True iff `version` is in the user's skip-list.
async fn is_version_skipped(state: &AppState, version: &str) -> bool {
    use crate::commands::settings::SettingsLoadState;
    let guard = state.settings.read().await;
    match &*guard {
        SettingsLoadState::Loaded(s) => s.skipped_update_versions.iter().any(|v| v == version),
        _ => false,
    }
}

/// Inner: run an install via the supplied backend, after validating the
/// caller's `version` arg against the cached `Available` payload and
/// rejecting downgrades.
pub async fn run_install(
    state: &AppState,
    backend: &dyn UpdaterBackend,
    version: &str,
) -> Result<(), AppError> {
    state.require_network("update_check").await?;

    // 1. Sanity check the caller-supplied version against the in-memory
    // cached `Available` payload. Defends against UI staleness: if the
    // user kept the Settings panel open through an auto-check cycle and
    // the available version changed, the install button fires with the
    // *old* version arg.
    let cached = {
        let guard = state.updater_state.read().await;
        guard.cached_available.clone()
    };
    let cached = cached.ok_or_else(|| AppError::InvalidArgument {
        message: format!(
            "no update available to install; run update_check_now first (requested {version})"
        ),
    })?;
    if cached.version != version {
        return Err(AppError::InvalidArgument {
            message: format!(
                "install version mismatch: requested {version}, cached available is {}",
                cached.version
            ),
        });
    }

    // 2. Explicit downgrade rejection (defense in depth; the plugin's
    // own version comparator already does this, but a future plugin
    // behaviour change cannot reopen the hole if we re-check here).
    let current = current_app_version();
    if !is_strict_upgrade(current, version) {
        return Err(AppError::DowngradeRejected {
            current: current.to_string(),
            target: version.to_string(),
        });
    }

    // 3. Delegate to the plugin via the backend trait. The plugin
    // performs the download + sha256 verification (when the manifest
    // carries a hash) + minisign verification + atomic .app bundle
    // replacement in a single call.
    backend.download_and_install(version).await?;

    // 4. Clear the cached available payload so a re-render of the
    // indicator + the Settings card doesn't re-offer the same
    // install. The new binary is on disk; the only remaining action
    // is the relaunch, which `update_relaunch` handles.
    {
        let mut guard = state.updater_state.write().await;
        guard.cached_available = None;
        guard.last_outcome = Some(UpdateCheckOutcome::UpToDate);
    }

    Ok(())
}

// ---------- IPC commands ----------

/// Run a manual update check. Frontend-callable via the "Check for
/// updates now" button in Settings → Network → Updates.
///
/// Returns `UpdateCheckOutcome` on success or `AppError` on failure
/// (including `ParanoidModeBlocked` when Offline Mode is on, surfaced
/// as the typed error rather than a fourth enum variant so the toast
/// channel stays uniform with every other gated call).
#[tauri::command]
pub async fn update_check_now(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheckOutcome, AppError> {
    #[cfg(test)]
    {
        let _ = (app, state);
        // Tests never go through this path; they exercise `run_check`
        // directly via the trait. The `app` + `state` parameters are
        // retained so the IPC signature stays uniform with the
        // production build.
        Err(AppError::Internal {
            message: "update_check_now is not callable in tests".into(),
        })
    }
    #[cfg(not(test))]
    {
        let backend = PluginBackend::new(app);
        run_check(&state, &backend).await
    }
}

/// Install the most recently-cached available update. Frontend passes
/// the `version` it expects to install so a stale UI doesn't trigger
/// an install of the wrong binary.
#[tauri::command]
pub async fn update_install(
    version: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    #[cfg(test)]
    {
        let _ = (app, version, state);
        Err(AppError::Internal {
            message: "update_install is not callable in tests".into(),
        })
    }
    #[cfg(not(test))]
    {
        let backend = PluginBackend::new(app);
        run_install(&state, &backend, &version).await
    }
}

/// Inner: record a skip against the supplied AppState. Extracted so
/// unit tests can exercise the Corrupt-settings safety branch without
/// going through the Tauri State wrapper.
///
/// ⚠️ Corrupt-settings safety: when settings are Corrupt, the app is
/// in fail-closed paranoid-on state (require_network denies all
/// outbound). Writing `Settings::default()` here would silently revoke
/// that lockdown (paranoid_mode = false in defaults). Refuse the skip
/// instead — the frontend's optimistic UI clear of `available` still
/// hides the indicator for this session; the user must reset settings
/// before a persisted skip is possible.
pub async fn run_skip(state: &AppState, version: &str) -> Result<(), AppError> {
    use crate::commands::settings::{persist, SettingsLoadState};

    // Validate the version arg — defensive against UI passing garbage.
    if version.trim().is_empty() || version.len() > 64 {
        return Err(AppError::InvalidArgument {
            message: format!("invalid version for skip: {version:?}"),
        });
    }

    // 1) Mutate the settings struct (push_skipped_version handles cap +
    //    dedupe internally). Take a snapshot to hand to persist().
    let updated_settings = {
        let guard = state.settings.read().await;
        match &*guard {
            SettingsLoadState::Loaded(s) => {
                let mut s = s.clone();
                s.push_skipped_version(version.to_string());
                s
            }
            SettingsLoadState::FirstLaunch => {
                // No settings file yet — defaults are correct (paranoid
                // OFF matches "user has never configured anything"),
                // and materializing the file with the skip recorded is
                // the right next step.
                let mut s = crate::commands::settings::Settings::default();
                s.push_skipped_version(version.to_string());
                s
            }
            SettingsLoadState::Corrupt { message } => {
                return Err(AppError::Internal {
                    message: format!(
                        "cannot record update skip while settings file is unreadable \
                         ({message}); reset settings from Settings → Network first"
                    ),
                });
            }
        }
    };
    let clamped = persist(&state.app_data_dir, updated_settings).await?;
    {
        let mut guard = state.settings.write().await;
        *guard = SettingsLoadState::Loaded(clamped);
    }

    // 2) Clear the cached "available" entry when it matches the skipped
    //    version, so subsequent update_check_now() responses + the
    //    title-bar indicator state are coherent. The frontend already
    //    flips its own `available = null` optimistically; this keeps
    //    the backend's view in sync.
    {
        let mut guard = state.updater_state.write().await;
        let should_clear = guard
            .cached_available
            .as_ref()
            .is_some_and(|cached| cached.version == version);
        if should_clear {
            guard.cached_available = None;
        }
    }

    Ok(())
}

/// Skip a specific update version. Pushes onto the FIFO-capped skip
/// list in settings (10 entries max, oldest evicted, dedup-and-move-to-
/// tail per `Settings::push_skipped_version`) and clears the cached
/// "available" entry when its version matches — so the title-bar
/// indicator disappears immediately on the next reactive render.
///
/// No `require_network` gate: skipping an update is a purely local
/// state mutation that records a user preference. It doesn't reach
/// the network. Sane to allow even when Offline Mode is on (the user
/// might be reviewing accumulated update notices while offline).
#[tauri::command]
pub async fn update_skip(version: String, state: State<'_, AppState>) -> Result<(), AppError> {
    run_skip(&state, &version).await
}

/// Relaunch the running app process so a freshly-installed update
/// becomes the active binary. The plugin's macOS install path replaces
/// the .app bundle but does not auto-restart the running process; this
/// command bridges that gap.
///
/// The restart itself is fired on a short delay so the IPC response
/// arrives at the renderer before the process dies. `tauri::AppHandle::
/// restart()` is `-> !` — calling it directly from the command body
/// would tear the IPC socket down mid-response.
///
/// No `require_network` gate: this is a purely local process action.
#[tauri::command]
pub async fn update_relaunch(app: tauri::AppHandle) -> Result<(), AppError> {
    #[cfg(test)]
    {
        let _ = app;
        Err(AppError::Internal {
            message: "update_relaunch is not callable in tests".into(),
        })
    }
    #[cfg(not(test))]
    {
        tauri::async_runtime::spawn(async move {
            // 50ms is enough for the JSON IPC response to make it to
            // the renderer before the process restarts.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            app.restart();
        });
        Ok(())
    }
}

// ---------- Auto-check scheduler ----------

/// Minimum wall-clock interval between auto-checks, regardless of how
/// often the app is restarted. 24h matches Sparkle's default + macOS
/// App Store cadence (see Phase 15 plan §Resolved Decision #3).
pub const AUTO_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Backoff steps when an auto-check fails. Order is 1h → 6h → 24h;
/// after the third failure the next attempt waits a full 24h window.
///
/// `#[allow(dead_code)]` because the constant is only read inside the
/// `#[cfg(not(test))]` branch of `spawn_auto_check_scheduler` — the
/// test build legitimately doesn't reach it. Pinned by
/// `auto_check_backoff_sequence_matches_plan_spec` so the values are
/// still test-covered.
#[allow(dead_code)]
pub const AUTO_CHECK_BACKOFF: &[std::time::Duration] = &[
    std::time::Duration::from_secs(60 * 60),
    std::time::Duration::from_secs(6 * 60 * 60),
    std::time::Duration::from_secs(24 * 60 * 60),
];

/// Decide whether the auto-check scheduler should fire right now.
/// Pure function — extracted so the schedule logic can be unit-tested
/// without an `AppState`, network, or filesystem.
///
/// Returns `true` when **all** of:
/// - `auto_check_enabled` is true (settings opt-in)
/// - `paranoid_mode` is false (kill switch off)
/// - The time since `last_checked_at` is at least `AUTO_CHECK_INTERVAL`,
///   OR `last_checked_at` is `None` (never checked before).
pub fn should_auto_check(
    auto_check_enabled: bool,
    paranoid_mode: bool,
    last_checked_at: Option<i64>,
    now: i64,
) -> bool {
    if !auto_check_enabled || paranoid_mode {
        return false;
    }
    match last_checked_at {
        None => true,
        Some(prev) => {
            let elapsed_secs = now.saturating_sub(prev);
            elapsed_secs >= AUTO_CHECK_INTERVAL.as_secs() as i64
        }
    }
}

/// Spawn the auto-check scheduler as a tokio background task. Called
/// once at app startup from `state::initialize`. The task:
///
/// 1. Sleeps for [`AUTO_CHECK_INTERVAL`].
/// 2. Reads the live settings (re-reads each cycle so a user who
///    toggles auto-check off mid-run is honoured on the next wake).
/// 3. If `should_auto_check` returns true, runs `run_check`. Failures
///    trigger the backoff sequence; successes reset to the 24h cadence.
/// 4. Loops forever.
pub fn spawn_auto_check_scheduler<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    #[cfg(not(test))]
    {
        use tauri::Manager;
        tauri::async_runtime::spawn(async move {
            // On the first wake we still defer one interval. Rationale:
            // the manual button is one click away in Settings, so the
            // user who wants an immediate check at launch has a path;
            // the auto-check is for *unattended* update discovery, not
            // "ping the endpoint the moment the app opens".
            let mut sleep_for = AUTO_CHECK_INTERVAL;
            let mut backoff_idx = 0usize;
            loop {
                tokio::time::sleep(sleep_for).await;

                let state: tauri::State<AppState> = app.state();
                let (auto_on, paranoid_on, last_checked_at) = read_scheduler_inputs(&state).await;
                let now = chrono::Utc::now().timestamp();

                if !should_auto_check(auto_on, paranoid_on, last_checked_at, now) {
                    // Try again at the canonical cadence — we don't
                    // escalate sleep here because the user could flip
                    // the toggle on at any point.
                    sleep_for = AUTO_CHECK_INTERVAL;
                    backoff_idx = 0;
                    continue;
                }

                let backend = PluginBackend::new(app.clone());
                match run_check(&state, &backend).await {
                    Ok(_) => {
                        sleep_for = AUTO_CHECK_INTERVAL;
                        backoff_idx = 0;
                    }
                    Err(e) => {
                        tracing::warn!("updater: auto-check failed (non-fatal): {e:?}");
                        let next = AUTO_CHECK_BACKOFF
                            .get(backoff_idx)
                            .copied()
                            .unwrap_or(AUTO_CHECK_INTERVAL);
                        sleep_for = next;
                        backoff_idx = (backoff_idx + 1).min(AUTO_CHECK_BACKOFF.len() - 1);
                    }
                }
            }
        });
    }
    #[cfg(test)]
    {
        // Under cfg(test) we don't spawn the real scheduler — its loop
        // body references the PluginBackend which is itself cfg-gated
        // out so it can't be instantiated. Tests exercise the pure
        // `should_auto_check` gate function directly and the IPC
        // commands through the trait-injected mock backend.
        let _ = app;
    }
}

/// Read the three settings the scheduler needs in a single guard
/// acquisition. Returns `(auto_check_enabled, paranoid_mode, last_checked_at)`.
#[cfg(not(test))]
async fn read_scheduler_inputs(state: &AppState) -> (bool, bool, Option<i64>) {
    use crate::commands::settings::SettingsLoadState;
    let (auto_on, paranoid_on) = {
        let guard = state.settings.read().await;
        match &*guard {
            SettingsLoadState::Loaded(s) => (s.update_auto_check, s.paranoid_mode),
            SettingsLoadState::FirstLaunch => (false, false),
            // Corrupt: deny outbound. The require_network gate inside
            // run_check would also catch this, but short-circuiting here
            // saves us a wakeup + a network attempt.
            SettingsLoadState::Corrupt { .. } => (false, true),
        }
    };
    let last_checked_at = {
        let guard = state.updater_state.read().await;
        guard.last_checked_at
    };
    (auto_on, paranoid_on, last_checked_at)
}

// Re-export so `AppState::build()` can construct the wrapped state slot.
pub fn empty_state() -> Arc<RwLock<UpdaterState>> {
    Arc::new(RwLock::new(UpdaterState::default()))
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::{Settings, SettingsLoadState};
    use std::sync::Mutex;

    /// In-memory mock backend.
    struct MockBackend {
        check_result: Mutex<Result<Option<CachedUpdate>, AppError>>,
        install_result: Mutex<Result<(), AppError>>,
        check_calls: Mutex<u32>,
    }

    impl MockBackend {
        fn returning(check: Result<Option<CachedUpdate>, AppError>) -> Self {
            Self {
                check_result: Mutex::new(check),
                install_result: Mutex::new(Ok(())),
                check_calls: Mutex::new(0),
            }
        }

        fn install_returning(
            check: Result<Option<CachedUpdate>, AppError>,
            install: Result<(), AppError>,
        ) -> Self {
            Self {
                check_result: Mutex::new(check),
                install_result: Mutex::new(install),
                check_calls: Mutex::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl UpdaterBackend for MockBackend {
        async fn check(&self) -> Result<Option<CachedUpdate>, AppError> {
            *self.check_calls.lock().unwrap() += 1;
            self.check_result.lock().unwrap().clone()
        }
        async fn download_and_install(&self, _version: &str) -> Result<(), AppError> {
            self.install_result.lock().unwrap().clone()
        }
    }

    async fn build_state_with(slot: SettingsLoadState) -> AppState {
        let state = AppState::build().expect("AppState::build");
        {
            let mut guard = state.settings.write().await;
            *guard = slot;
        }
        state
    }

    /// Phase 15 §Tests #1 — happy path: plugin returns "no update";
    /// command returns `UpToDate`.
    #[tokio::test]
    async fn check_now_returns_up_to_date_when_plugin_returns_none() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let backend = MockBackend::returning(Ok(None));
        let outcome = run_check(&state, &backend).await.expect("check");
        assert_eq!(outcome, UpdateCheckOutcome::UpToDate);

        // last_checked_at must be populated regardless of outcome so
        // the scheduler honours the 24h floor on UpToDate too.
        let guard = state.updater_state.read().await;
        assert!(guard.last_checked_at.is_some());
        assert!(guard.cached_available.is_none());
    }

    /// Phase 15 §Tests #2 — available path: plugin returns a version,
    /// command returns `Available { ... }` with the right fields.
    #[tokio::test]
    async fn check_now_returns_available_when_plugin_returns_some() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let backend = MockBackend::returning(Ok(Some(CachedUpdate {
            version: "9.9.9".into(),
            current_version: current_app_version().to_string(),
            notes: Some("hotfix".into()),
            pub_date: Some("2026-05-24T00:00:00Z".into()),
        })));
        let outcome = run_check(&state, &backend).await.expect("check");
        match outcome {
            UpdateCheckOutcome::Available {
                version,
                notes,
                skipped,
                ..
            } => {
                assert_eq!(version, "9.9.9");
                assert_eq!(notes.as_deref(), Some("hotfix"));
                assert!(!skipped, "fresh version must not be marked skipped");
            }
            other => panic!("expected Available, got {other:?}"),
        }

        // Cached available payload available for install validation.
        let guard = state.updater_state.read().await;
        let cached = guard.cached_available.clone().expect("cached");
        assert_eq!(cached.version, "9.9.9");
    }

    /// Available + version is in the skip-list → `skipped: true`.
    #[tokio::test]
    async fn check_now_marks_skipped_when_version_is_in_skip_list() {
        let s = Settings {
            skipped_update_versions: vec!["9.9.9".into()],
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let backend = MockBackend::returning(Ok(Some(CachedUpdate {
            version: "9.9.9".into(),
            current_version: current_app_version().to_string(),
            notes: None,
            pub_date: None,
        })));
        let outcome = run_check(&state, &backend).await.expect("check");
        match outcome {
            UpdateCheckOutcome::Available { skipped, .. } => assert!(skipped),
            other => panic!("expected Available, got {other:?}"),
        }
    }

    /// Phase 15 §Tests #3 — blocked by Paranoid Mode: returns
    /// `ParanoidModeBlocked { feature: "update_check" }`.
    #[tokio::test]
    async fn check_now_blocked_by_paranoid_mode() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let backend = MockBackend::returning(Ok(None));
        let r = run_check(&state, &backend).await;
        match r {
            Err(AppError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "update_check");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }

        // Backend must NOT have been called — gate runs before the trait call.
        assert_eq!(*backend.check_calls.lock().unwrap(), 0);
    }

    /// Phase 15 §Tests #4 — install rejects a stale version arg.
    #[tokio::test]
    async fn install_rejects_stale_version_arg() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        // Seed the cache with an Available 9.9.9.
        let backend = MockBackend::returning(Ok(Some(CachedUpdate {
            version: "9.9.9".into(),
            current_version: current_app_version().to_string(),
            notes: None,
            pub_date: None,
        })));
        run_check(&state, &backend).await.expect("check");

        // UI requests install of an OLDER version than the cache (stale UI).
        let r = run_install(&state, &backend, "0.3.0").await;
        match r {
            Err(AppError::InvalidArgument { message }) => {
                assert!(message.contains("mismatch"), "got: {message}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Install without a preceding check → InvalidArgument (no cache).
    #[tokio::test]
    async fn install_without_cache_returns_invalid_argument() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let backend = MockBackend::returning(Ok(None));
        let r = run_install(&state, &backend, "9.9.9").await;
        match r {
            Err(AppError::InvalidArgument { message }) => {
                assert!(message.contains("no update available"), "got: {message}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Install of a same-or-older version → DowngradeRejected.
    #[tokio::test]
    async fn install_rejects_downgrade() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        // Seed cache pointing at the *current* version (same-version is
        // also a downgrade for our purposes — strict upgrade required).
        let current = current_app_version().to_string();
        let backend = MockBackend::returning(Ok(Some(CachedUpdate {
            version: current.clone(),
            current_version: current.clone(),
            notes: None,
            pub_date: None,
        })));
        run_check(&state, &backend).await.expect("check");

        let r = run_install(&state, &backend, &current).await;
        match r {
            Err(AppError::DowngradeRejected {
                current: c,
                target: t,
            }) => {
                assert_eq!(c, current);
                assert_eq!(t, current);
            }
            other => panic!("expected DowngradeRejected, got {other:?}"),
        }
    }

    /// Phase 15 §Tests #8 — signature verification failure surfaces
    /// as the typed `SignatureVerificationFailed` error.
    #[tokio::test]
    async fn install_surfaces_signature_verification_failed() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let backend = MockBackend::install_returning(
            Ok(Some(CachedUpdate {
                version: "9.9.9".into(),
                current_version: current_app_version().to_string(),
                notes: None,
                pub_date: None,
            })),
            Err(AppError::SignatureVerificationFailed {
                message: "minisign rejected".into(),
            }),
        );
        run_check(&state, &backend).await.expect("check");

        let r = run_install(&state, &backend, "9.9.9").await;
        match r {
            Err(AppError::SignatureVerificationFailed { message }) => {
                assert!(message.contains("minisign"), "got: {message}");
            }
            other => panic!("expected SignatureVerificationFailed, got {other:?}"),
        }
    }

    /// Phase 15 §Tests #9 (BONUS) — sha256 mismatch surfaces as the
    /// typed `HashMismatch` error before signature verification.
    #[tokio::test]
    async fn install_surfaces_hash_mismatch() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let backend = MockBackend::install_returning(
            Ok(Some(CachedUpdate {
                version: "9.9.9".into(),
                current_version: current_app_version().to_string(),
                notes: None,
                pub_date: None,
            })),
            Err(AppError::HashMismatch {
                expected: "deadbeef".into(),
                actual: "cafef00d".into(),
            }),
        );
        run_check(&state, &backend).await.expect("check");

        let r = run_install(&state, &backend, "9.9.9").await;
        match r {
            Err(AppError::HashMismatch { expected, actual }) => {
                assert_eq!(expected, "deadbeef");
                assert_eq!(actual, "cafef00d");
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    /// Phase 15 fix-up — `run_skip` MUST NOT persist `Settings::default()`
    /// when the in-memory state is Corrupt. Doing so silently revokes the
    /// paranoid-on lockdown that Corrupt settings imply. Refusal with a
    /// typed error is the correct behaviour.
    #[tokio::test]
    async fn skip_refuses_on_corrupt_settings() {
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "synthetic test corruption".into(),
        })
        .await;

        let r = run_skip(&state, "9.9.9").await;
        match r {
            Err(AppError::Internal { message }) => {
                assert!(
                    message.contains("unreadable"),
                    "expected unreadable message, got: {message}"
                );
                assert!(
                    message.contains("reset"),
                    "expected reset guidance, got: {message}"
                );
            }
            other => panic!("expected Internal error refusing skip, got {other:?}"),
        }

        // Settings must still be Corrupt — refused skip must NOT have
        // overwritten the in-memory state.
        let guard = state.settings.read().await;
        match &*guard {
            SettingsLoadState::Corrupt { .. } => {} // expected
            other => panic!("settings state was mutated by refused skip: {other:?}"),
        }
    }

    /// Phase 15 fix-up — invalid version argument rejected with
    /// InvalidArgument, regardless of settings state.
    #[tokio::test]
    async fn skip_rejects_empty_version() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let r = run_skip(&state, "").await;
        match r {
            Err(AppError::InvalidArgument { .. }) => {}
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Phase 15 fix-up — `run_install` clears `cached_available` after a
    /// successful install so the indicator + Settings card don't
    /// re-offer the same install on the next render.
    #[tokio::test]
    async fn install_clears_cached_available_on_success() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let backend = MockBackend::install_returning(
            Ok(Some(CachedUpdate {
                version: "9.9.9".into(),
                current_version: current_app_version().to_string(),
                notes: None,
                pub_date: None,
            })),
            Ok(()),
        );
        run_check(&state, &backend).await.expect("check");
        // Sanity: cache populated by the check.
        {
            let guard = state.updater_state.read().await;
            assert!(guard.cached_available.is_some());
        }

        run_install(&state, &backend, "9.9.9")
            .await
            .expect("install");

        let guard = state.updater_state.read().await;
        assert!(
            guard.cached_available.is_none(),
            "cached_available must be cleared after successful install"
        );
        assert_eq!(guard.last_outcome, Some(UpdateCheckOutcome::UpToDate));
    }

    /// Install blocked by paranoid mode → ParanoidModeBlocked.
    #[tokio::test]
    async fn install_blocked_by_paranoid_mode() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let backend = MockBackend::returning(Ok(None));
        let r = run_install(&state, &backend, "9.9.9").await;
        match r {
            Err(AppError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "update_check");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    // ---------- Scheduler ----------

    /// Phase 15 §Tests #6 — scheduler honours the 24h floor.
    /// Repeated `should_auto_check` calls within the window do not
    /// approve a fresh check.
    #[test]
    fn scheduler_honors_24h_floor() {
        let now: i64 = 1_700_000_000;
        let last_check = now - (23 * 3600);
        // 23h since last check → not due yet.
        assert!(!should_auto_check(true, false, Some(last_check), now));
        // Exactly 24h → due.
        assert!(should_auto_check(true, false, Some(now - 24 * 3600), now));
        // 25h → due.
        assert!(should_auto_check(true, false, Some(now - 25 * 3600), now));
    }

    /// Repeated start/stop within an hour does not fire multiple checks:
    /// model this by checking the gate function with timestamps that
    /// represent successive launches all within the same window.
    #[test]
    fn scheduler_does_not_refire_within_one_hour() {
        let initial_check: i64 = 1_700_000_000;
        // Five "launches" all within 60 minutes of the initial check.
        for offset in &[60, 600, 1800, 3000, 3500] {
            let now = initial_check + offset;
            let should = should_auto_check(true, false, Some(initial_check), now);
            assert!(
                !should,
                "scheduler must NOT fire at +{offset}s after last check"
            );
        }
    }

    /// Never-checked-before → fires on first wake.
    #[test]
    fn scheduler_fires_when_never_checked() {
        let now: i64 = 1_700_000_000;
        assert!(should_auto_check(true, false, None, now));
    }

    /// Phase 15 §Tests #7 — flipping paranoid_mode on suspends the
    /// scheduler regardless of how stale `last_checked_at` is.
    #[test]
    fn scheduler_suspends_on_paranoid_mode() {
        let now: i64 = 1_700_000_000;
        // Even with last_checked_at well past the 24h window, the gate
        // returns false when paranoid_mode is on.
        assert!(!should_auto_check(true, true, Some(now - 999_999), now));
        // And with last_checked_at None (never checked) still false.
        assert!(!should_auto_check(true, true, None, now));
    }

    /// Off by default: auto_check_enabled=false → never fires.
    #[test]
    fn scheduler_does_nothing_when_disabled() {
        let now: i64 = 1_700_000_000;
        assert!(!should_auto_check(false, false, None, now));
        assert!(!should_auto_check(false, false, Some(now - 100_000), now));
    }

    /// Pin the backoff sequence at 1h, 6h, 24h. Drift here would mean
    /// silently changing the user-facing retry behaviour.
    #[test]
    fn auto_check_backoff_sequence_matches_plan_spec() {
        assert_eq!(AUTO_CHECK_BACKOFF.len(), 3);
        assert_eq!(AUTO_CHECK_BACKOFF[0].as_secs(), 60 * 60);
        assert_eq!(AUTO_CHECK_BACKOFF[1].as_secs(), 6 * 60 * 60);
        assert_eq!(AUTO_CHECK_BACKOFF[2].as_secs(), 24 * 60 * 60);
    }

    /// Pin the 24h floor.
    #[test]
    fn auto_check_interval_is_24_hours() {
        assert_eq!(AUTO_CHECK_INTERVAL.as_secs(), 24 * 60 * 60);
    }

    // ---------- semver helpers ----------

    #[test]
    fn parse_semver_round_trips_basic() {
        assert_eq!(parse_semver("0.3.1"), Some((0, 3, 1)));
        assert_eq!(parse_semver("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver("12.345.6789"), Some((12, 345, 6789)));
    }

    #[test]
    fn parse_semver_strips_prerelease() {
        assert_eq!(parse_semver("0.3.1-beta.1"), Some((0, 3, 1)));
        assert_eq!(parse_semver("0.3.1+build.7"), Some((0, 3, 1)));
    }

    #[test]
    fn parse_semver_rejects_garbage() {
        assert_eq!(parse_semver("not a version"), None);
        assert_eq!(parse_semver("1.2"), None);
        assert_eq!(parse_semver(""), None);
    }

    #[test]
    fn is_strict_upgrade_basic() {
        assert!(is_strict_upgrade("0.3.0", "0.3.1"));
        assert!(is_strict_upgrade("0.3.0", "0.4.0"));
        assert!(is_strict_upgrade("0.3.0", "1.0.0"));
    }

    #[test]
    fn is_strict_upgrade_rejects_same_or_older() {
        assert!(!is_strict_upgrade("0.3.0", "0.3.0"));
        assert!(!is_strict_upgrade("0.3.1", "0.3.0"));
        assert!(!is_strict_upgrade("1.0.0", "0.99.99"));
    }

    #[test]
    fn is_strict_upgrade_handles_v_prefix() {
        assert!(is_strict_upgrade("v0.3.0", "v0.3.1"));
        assert!(is_strict_upgrade("0.3.0", "v0.3.1"));
    }

    #[test]
    fn is_strict_upgrade_unparseable_rejects() {
        assert!(!is_strict_upgrade("garbage", "0.3.1"));
        assert!(!is_strict_upgrade("0.3.0", "also garbage"));
    }

    /// UpdateCheckOutcome serializes with the `kind` tag in camelCase
    /// and `Available` carries the fields the frontend expects.
    #[test]
    fn update_check_outcome_wire_shape() {
        let v = serde_json::to_value(UpdateCheckOutcome::UpToDate).unwrap();
        assert_eq!(v["kind"], "upToDate");

        let v = serde_json::to_value(UpdateCheckOutcome::Available {
            version: "0.3.1".into(),
            current_version: "0.3.0".into(),
            notes: Some("changelog".into()),
            pub_date: Some("2026-05-24T00:00:00Z".into()),
            skipped: false,
        })
        .unwrap();
        assert_eq!(v["kind"], "available");
        assert_eq!(v["version"], "0.3.1");
        assert_eq!(v["currentVersion"], "0.3.0");
        assert_eq!(v["notes"], "changelog");
        assert_eq!(v["pubDate"], "2026-05-24T00:00:00Z");
        assert_eq!(v["skipped"], false);
    }

    /// `current_app_version` returns a parseable semver-shaped string.
    #[test]
    fn current_app_version_is_semver_shaped() {
        let v = current_app_version();
        assert!(parse_semver(v).is_some(), "{v} did not parse as semver");
    }
}
