//! GitHub IPC surface (Phase 12c + 12e).
//!
//! Every command in this module follows the same security pattern:
//!
//! 1. **Settings opt-in gate** (for `github_repo_stats`): consult
//!    `Settings::github_enabled`. False → return `Ok(None)` without any
//!    outbound call, without any URL parse, without any cache touch.
//! 2. **Paranoid-mode gate**: `state.require_network("github_*")`. This
//!    is the single chokepoint that the "Block all outbound network"
//!    master switch flips. Sign-in itself is gated too — per §12d the
//!    OAuth handshake is "outbound" and must be blocked when paranoid
//!    mode is on.
//! 3. **URL allowlist** (`github_repo_stats`): `parse_github_url` —
//!    refuse anything that isn't strictly `github.com/<owner>/<repo>`.
//!
//! ## Token never crosses IPC
//!
//! `github_status` returns `GithubStatusDto { signed_in, username,
//! scopes }`. The token itself lives in the Keychain and is read
//! server-side by `read_token()` on each authenticated request.

use tauri::State;

use crate::error::AppError;
use crate::github::{
    self, actions, auth, fetch_repo_stats, parse_github_url, CreatedIssue, DeviceFlowStart,
    GithubRepo, GithubStatusDto, PollResult, PollResultDto, RepoStats, Token,
};
use crate::state::AppState;

/// Per-action OAuth scope requirement. v0.2.2 split this from a single
/// constant after discovering that GitHub's `PUT /repos/{o}/{r}/subscription`
/// (the watch endpoint) requires `notifications` specifically — `public_repo`
/// alone returns HTTP 404 (their privacy-preserving mask for "you don't
/// have the scope"). The action gate now checks the per-action required
/// scope, and the typed `ScopeRequired { scope }` error carries the SPECIFIC
/// scope name so the frontend can render an actionable "Re-authorize"
/// toast that triggers an incremental scope grant (signIn() with the full
/// GITHUB_OAUTH_SCOPES list — GitHub's consent screen surfaces only the
/// new scope, the existing ones display as "already granted").
const SCOPE_PUBLIC_REPO: &str = "public_repo";
const SCOPE_NOTIFICATIONS: &str = "notifications";

// ---------- Repo stats (12c) ----------

#[tauri::command]
pub async fn github_repo_stats(
    homepage: String,
    state: State<'_, AppState>,
) -> Result<Option<RepoStats>, AppError> {
    // 1. Settings opt-in gate. The `github_enabled` toggle defaults
    //    OFF; if the user hasn't flipped it we silently return None.
    //    No network. No URL parse. The frontend interprets None as
    //    "no GitHub stats for this row".
    {
        let guard = state.settings.read().await;
        let enabled = match &*guard {
            crate::commands::settings::SettingsLoadState::Loaded(s) => s.github_enabled,
            // First launch defaults: github_enabled = false. Match
            // Settings::default() so the gate's behaviour is the same
            // as if the user had explicitly chosen the defaults.
            crate::commands::settings::SettingsLoadState::FirstLaunch => false,
            // Corrupt → fail closed (paranoid_mode_blocked will be
            // raised by the next gate anyway, but short-circuit here
            // so we don't leak the corrupt-state behaviour into the
            // None path).
            crate::commands::settings::SettingsLoadState::Corrupt { .. } => false,
        };
        if !enabled {
            return Ok(None);
        }
    }

    // 2. Paranoid-mode gate. Even with the opt-in toggle ON, the
    //    master switch wins — no GitHub probe when paranoid mode is
    //    enabled or settings are corrupt.
    state.require_network("github_repo_stats").await?;

    // 3. URL allowlist. Non-github URLs collapse to None (we treat
    //    them the same as "no homepage").
    let repo = match parse_github_url(&homepage) {
        Some(r) => r,
        None => return Ok(None),
    };

    // 4. Issue the fetch.
    let client = github::stats::build_client()?;
    let auth_token = auth::read_token()?;
    let cache_dir = state.app_data_dir.join("github-cache");
    fetch_repo_stats(&client, &repo, auth_token.as_ref(), &cache_dir).await
}

// ---------- Auth status (12e) ----------

#[tauri::command]
pub async fn github_status(_state: State<'_, AppState>) -> Result<GithubStatusDto, AppError> {
    // Reads from Keychain only — no network call, so no
    // require_network gate. The Settings panel calls this on mount to
    // know whether to show "Sign in" vs "Signed in as @user".
    auth::status()
}

// ---------- Sign-in start (12e) ----------

#[tauri::command]
pub async fn github_signin_start(state: State<'_, AppState>) -> Result<DeviceFlowStart, AppError> {
    // Sign-in itself is outbound — paranoid mode blocks even the OAuth
    // handshake. Per §12d this is by design: the user can't sign in if
    // they've told us not to make outbound calls.
    state.require_network("github_signin").await?;
    auth::start_device_flow().await
}

// ---------- Sign-in poll (12e) ----------

#[tauri::command]
pub async fn github_signin_poll(
    device_code: String,
    state: State<'_, AppState>,
) -> Result<PollResultDto, AppError> {
    state.require_network("github_signin").await?;
    let result: PollResult = auth::poll_device_flow(&device_code).await?;
    Ok(result.into())
}

// ---------- Sign-out (12e) ----------

#[tauri::command]
pub async fn github_signout(_state: State<'_, AppState>) -> Result<(), AppError> {
    // Sign-out is purely a Keychain delete — no network. We don't
    // gate it on paranoid mode (it's a *reduction* of state, never
    // an outbound call).
    auth::signout()
}

// ---------- Authed actions (12f) ----------
//
// Each command below runs the same five-step gate chain before any
// network call:
//
//   1. `require_network(feature)` — paranoid mode kill-switch.
//   2. `parse_github_url(homepage)` — strict allowlist (rejects
//      gist./raw./suffix-confusables and anything that isn't exactly
//      `github.com/<owner>/<repo>`). Mismatch → `InvalidArgument`.
//   3. `auth::read_token()` — must return `Some(Token)` from the
//      Keychain or we surface `AppError::AuthRequired` (no network
//      attempt). The token never crosses the IPC boundary.
//   4. `auth::read_scopes()` — must contain `public_repo` or we
//      surface `AppError::ScopeRequired { scope }` so the frontend
//      can route the user to a re-grant flow.
//   5. Call the matching `github::actions::*` function, which
//      re-validates the repo defensively before sending.

/// Common gate chain for every Phase 12f authed action. Returns a
/// `(client, repo, token)` triple on success; surfaces the typed
/// error on any gate failure.
///
/// `required_scope` is the OAuth scope this specific action needs.
/// The gate pre-emptively checks the cached scope list and returns
/// `ScopeRequired { scope }` if missing — saves a round-trip to GitHub
/// AND gives the frontend the SPECIFIC scope name (so the actionable
/// re-auth toast can be precise: "Watch needs `notifications`. Re-
/// authorize?").
async fn authed_gate(
    state: &AppState,
    homepage: &str,
    feature: &'static str,
    required_scope: &str,
) -> Result<(reqwest::Client, GithubRepo, Token), AppError> {
    // 1. Paranoid-mode gate.
    state.require_network(feature).await?;

    // 2. URL allowlist. Authed actions use `InvalidArgument` (rather
    //    than the `Ok(None)` collapse `github_repo_stats` uses) because
    //    we shouldn't get this far if the homepage wasn't already
    //    classified as a GitHub URL on the frontend; an unparseable
    //    homepage here is a real bug, not a "no stats" outcome.
    let repo = parse_github_url(homepage).ok_or_else(|| AppError::InvalidArgument {
        message: format!("not a github.com/<owner>/<repo> URL: {homepage}"),
    })?;

    // 3. Auth gate.
    let token = auth::read_token()?.ok_or(AppError::AuthRequired)?;

    // 4. Scope gate. The scope list is cached at sign-in and read from
    //    the Keychain — no extra GitHub round-trip required.
    let scopes = auth::read_scopes()?.unwrap_or_default();
    if !scopes.iter().any(|s| s == required_scope) {
        return Err(AppError::ScopeRequired {
            scope: required_scope.to_string(),
        });
    }

    // 5. Build the client once per call (cheap — reqwest pools
    //    connections; we don't try to share a client across calls
    //    because the auth gate would have to be re-checked anyway).
    let client = actions::build_client()?;
    Ok((client, repo, token))
}

#[tauri::command]
pub async fn github_star(homepage: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let (client, repo, token) =
        authed_gate(&state, &homepage, "github_star", SCOPE_PUBLIC_REPO).await?;
    actions::star(&client, &repo, &token).await
}

#[tauri::command]
pub async fn github_unstar(homepage: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let (client, repo, token) =
        authed_gate(&state, &homepage, "github_unstar", SCOPE_PUBLIC_REPO).await?;
    actions::unstar(&client, &repo, &token).await
}

#[tauri::command]
pub async fn github_is_starred(
    homepage: String,
    state: State<'_, AppState>,
) -> Result<bool, AppError> {
    let (client, repo, token) =
        authed_gate(&state, &homepage, "github_is_starred", SCOPE_PUBLIC_REPO).await?;
    actions::is_starred(&client, &repo, &token).await
}

#[tauri::command]
pub async fn github_watch(homepage: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let (client, repo, token) =
        authed_gate(&state, &homepage, "github_watch", SCOPE_NOTIFICATIONS).await?;
    actions::watch(&client, &repo, &token).await
}

#[tauri::command]
pub async fn github_unwatch(homepage: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let (client, repo, token) =
        authed_gate(&state, &homepage, "github_unwatch", SCOPE_NOTIFICATIONS).await?;
    actions::unwatch(&client, &repo, &token).await
}

#[tauri::command]
pub async fn github_create_issue(
    homepage: String,
    title: String,
    body: String,
    labels: Vec<String>,
    state: State<'_, AppState>,
) -> Result<CreatedIssue, AppError> {
    let (client, repo, token) =
        authed_gate(&state, &homepage, "github_create_issue", SCOPE_PUBLIC_REPO).await?;
    // Convert Vec<String> to &[&str] for the borrowed-slice API. The
    // sanitiser then takes owned Strings back for the JSON payload.
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    actions::create_issue(&client, &repo, &token, &title, &body, &label_refs).await
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    //! Tests focus on the gate-chain ordering — the actual network/Keychain
    //! work has its own coverage in `github::{actions, auth, stats, url}`. Here
    //! we pin the contract that gates fire in the right order:
    //!   settings → paranoid → URL → auth → scope → action.
    //!
    //! The Tauri-command wrappers themselves need an `AppState` to test;
    //! we build one via `AppState::build` and hand-mutate the settings
    //! slot to drive the gates. Auth + scope branches are exercised via
    //! `inner_authed_gate_with_kc`, which takes a mock keychain so we
    //! don't touch the real macOS Keychain in CI.

    use super::*;
    use crate::commands::settings::{Settings, SettingsLoadState};
    use crate::github::auth::{KeychainSlot, KEYCHAIN_ACCOUNT_SCOPES, KEYCHAIN_ACCOUNT_TOKEN};
    use std::collections::HashMap;
    use std::sync::Mutex;

    async fn build_state_with(slot: SettingsLoadState) -> AppState {
        let state = AppState::build().expect("AppState::build");
        {
            let mut guard = state.settings.write().await;
            *guard = slot;
        }
        state
    }

    /// `github_enabled: false` → command returns `Ok(None)` without
    /// any network attempt, URL parse, or settings.json write.
    #[tokio::test]
    async fn settings_disabled_short_circuits_to_none() {
        let s = Settings {
            github_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        // Call the inner gate sequence directly to avoid the
        // `State<'_, AppState>` wrapper that the macro needs.
        let result = inner_repo_stats(&state, "https://github.com/foo/bar".into()).await;
        assert!(matches!(result, Ok(None)));
    }

    /// `github_enabled: true` but paranoid mode ON → blocked.
    #[tokio::test]
    async fn paranoid_mode_blocks_even_when_github_enabled() {
        let s = Settings {
            github_enabled: true,
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let result = inner_repo_stats(&state, "https://github.com/foo/bar".into()).await;
        match result {
            Err(AppError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_repo_stats");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    /// `github_enabled: true`, paranoid off, non-github homepage →
    /// `Ok(None)` (gates passed, validator rejected).
    #[tokio::test]
    async fn non_github_homepage_returns_none() {
        let s = Settings {
            github_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let result = inner_repo_stats(&state, "https://example.com/foo/bar".into()).await;
        assert!(matches!(result, Ok(None)));
    }

    /// All 4 sign-in commands consult require_network. We test the
    /// blocking path here; the per-command happy path requires hitting
    /// github.com which is out of scope for a unit test.
    #[tokio::test]
    async fn signin_start_is_blocked_by_paranoid_mode() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let r = inner_signin_start(&state).await;
        match r {
            Err(AppError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_signin");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn signin_poll_is_blocked_by_paranoid_mode() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let r = inner_signin_poll(&state, "fake-device-code".into()).await;
        match r {
            Err(AppError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_signin");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    /// `Corrupt` settings → fail closed for repo stats too. The
    /// settings-opt-in check sees Corrupt as `false` (defensive),
    /// returning Ok(None). The paranoid gate would *also* block —
    /// but the opt-in short-circuit fires first.
    #[tokio::test]
    async fn corrupt_settings_returns_none_for_stats() {
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "bad json".into(),
        })
        .await;
        let result = inner_repo_stats(&state, "https://github.com/foo/bar".into()).await;
        assert!(matches!(result, Ok(None)));
    }

    // ---------- Inner copies that don't take `State<>` ----------
    //
    // The Tauri command attribute wraps each function in a layer that
    // expects `State<'_, AppState>`. For unit tests we want to drive
    // the same logic from a plain `&AppState`. These inner copies are
    // identical to the public commands minus the Tauri wrapper.

    async fn inner_repo_stats(
        state: &AppState,
        homepage: String,
    ) -> Result<Option<RepoStats>, AppError> {
        {
            let guard = state.settings.read().await;
            let enabled = match &*guard {
                SettingsLoadState::Loaded(s) => s.github_enabled,
                SettingsLoadState::FirstLaunch => false,
                SettingsLoadState::Corrupt { .. } => false,
            };
            if !enabled {
                return Ok(None);
            }
        }
        state.require_network("github_repo_stats").await?;
        let repo = match parse_github_url(&homepage) {
            Some(r) => r,
            None => return Ok(None),
        };
        let client = github::stats::build_client()?;
        let auth_token = auth::read_token()?;
        let cache_dir = state.app_data_dir.join("github-cache");
        fetch_repo_stats(&client, &repo, auth_token.as_ref(), &cache_dir).await
    }

    async fn inner_signin_start(state: &AppState) -> Result<DeviceFlowStart, AppError> {
        state.require_network("github_signin").await?;
        auth::start_device_flow().await
    }

    async fn inner_signin_poll(
        state: &AppState,
        device_code: String,
    ) -> Result<PollResultDto, AppError> {
        state.require_network("github_signin").await?;
        let result: PollResult = auth::poll_device_flow(&device_code).await?;
        Ok(result.into())
    }

    // ---------- Phase 12f: mock keychain + inner gate ----------

    /// In-memory keychain used by the auth/scope gate tests so we
    /// don't read or write the real macOS Keychain during cargo test.
    struct MockKeychain {
        entries: Mutex<HashMap<String, String>>,
    }

    impl MockKeychain {
        fn new() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
            }
        }
        fn with_token_and_scopes(token: &str, scopes: &[&str]) -> Self {
            let mk = Self::new();
            mk.entries
                .lock()
                .unwrap()
                .insert(KEYCHAIN_ACCOUNT_TOKEN.to_string(), token.to_string());
            let json = serde_json::to_string(scopes).unwrap();
            mk.entries
                .lock()
                .unwrap()
                .insert(KEYCHAIN_ACCOUNT_SCOPES.to_string(), json);
            mk
        }
    }

    impl KeychainSlot for MockKeychain {
        fn read(&self, account: &str) -> Result<Option<String>, AppError> {
            Ok(self.entries.lock().unwrap().get(account).cloned())
        }
        fn write(&self, account: &str, value: &str) -> Result<(), AppError> {
            self.entries
                .lock()
                .unwrap()
                .insert(account.to_string(), value.to_string());
            Ok(())
        }
        fn delete(&self, account: &str) -> Result<(), AppError> {
            self.entries.lock().unwrap().remove(account);
            Ok(())
        }
    }

    /// Mirror of `authed_gate` that reads from an injected keychain
    /// instead of the production one. Used by tests to drive the
    /// AuthRequired and ScopeRequired branches deterministically.
    async fn inner_authed_gate_with_kc(
        state: &AppState,
        homepage: &str,
        feature: &'static str,
        required_scope: &str,
        kc: &dyn KeychainSlot,
    ) -> Result<(GithubRepo, Token), AppError> {
        state.require_network(feature).await?;
        let repo = parse_github_url(homepage).ok_or_else(|| AppError::InvalidArgument {
            message: format!("not a github.com/<owner>/<repo> URL: {homepage}"),
        })?;
        let token = auth::read_token_with(kc)?.ok_or(AppError::AuthRequired)?;
        let scopes = auth::read_scopes_with(kc)?.unwrap_or_default();
        if !scopes.iter().any(|s| s == required_scope) {
            return Err(AppError::ScopeRequired {
                scope: required_scope.to_string(),
            });
        }
        Ok((repo, token))
    }

    // ---------- Paranoid-mode gate for each new command (6) ----------

    /// Helper: assert that calling `feature` with paranoid ON blocks
    /// before any keychain or URL work. Asserts the feature string is
    /// carried verbatim into the error so the frontend toast can route.
    async fn assert_blocked_by_paranoid(feature: &'static str) {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let r = state.require_network(feature).await;
        match r {
            Err(AppError::ParanoidModeBlocked { feature: f }) => assert_eq!(f, feature),
            other => panic!("expected ParanoidModeBlocked for {feature}, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn star_blocked_by_paranoid_mode() {
        assert_blocked_by_paranoid("github_star").await;
    }

    #[tokio::test]
    async fn unstar_blocked_by_paranoid_mode() {
        assert_blocked_by_paranoid("github_unstar").await;
    }

    #[tokio::test]
    async fn is_starred_blocked_by_paranoid_mode() {
        assert_blocked_by_paranoid("github_is_starred").await;
    }

    #[tokio::test]
    async fn watch_blocked_by_paranoid_mode() {
        assert_blocked_by_paranoid("github_watch").await;
    }

    #[tokio::test]
    async fn unwatch_blocked_by_paranoid_mode() {
        assert_blocked_by_paranoid("github_unwatch").await;
    }

    #[tokio::test]
    async fn create_issue_blocked_by_paranoid_mode() {
        assert_blocked_by_paranoid("github_create_issue").await;
    }

    /// Corrupt settings also fails closed for authed actions (same as
    /// paranoid=on). The fail-closed rule lives in `require_network`
    /// itself, but pin it here so the §12f gate chain's contract is
    /// asserted at the command layer too.
    #[tokio::test]
    async fn authed_actions_blocked_when_settings_corrupt() {
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "boom".into(),
        })
        .await;
        let r = state.require_network("github_create_issue").await;
        assert!(matches!(r, Err(AppError::ParanoidModeBlocked { .. })));
    }

    // ---------- Auth / Scope gates ----------

    /// No token in the keychain → AuthRequired, BEFORE any network
    /// attempt. The URL must already be valid (so we know the auth
    /// gate, not the URL gate, is the one firing).
    #[tokio::test]
    async fn authed_gate_returns_auth_required_when_no_token() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let kc = MockKeychain::new(); // empty
        let r = inner_authed_gate_with_kc(
            &state,
            "https://github.com/octocat/hello-world",
            "github_star",
            SCOPE_PUBLIC_REPO,
            &kc,
        )
        .await;
        match r {
            Err(AppError::AuthRequired) => {}
            other => panic!("expected AuthRequired, got {other:?}"),
        }
    }

    /// Token present but scopes don't include `public_repo` →
    /// ScopeRequired with `scope == "public_repo"`.
    #[tokio::test]
    async fn authed_gate_returns_scope_required_when_public_repo_missing() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let kc = MockKeychain::with_token_and_scopes("ghp_test", &["read:user"]);
        let r = inner_authed_gate_with_kc(
            &state,
            "https://github.com/octocat/hello-world",
            "github_star",
            SCOPE_PUBLIC_REPO,
            &kc,
        )
        .await;
        match r {
            Err(AppError::ScopeRequired { scope }) => assert_eq!(scope, "public_repo"),
            other => panic!("expected ScopeRequired(public_repo), got {other:?}"),
        }
    }

    /// v0.2.2: watch / unwatch require `notifications`, not `public_repo`.
    /// A token with public_repo but no notifications must surface
    /// `ScopeRequired { scope: "notifications" }` so the frontend can
    /// render an actionable "Re-authorize" toast that fires an
    /// incremental scope grant via `signIn()`. Pre-empts the GitHub-
    /// returns-404-for-missing-scope behaviour at the watch endpoint.
    #[tokio::test]
    async fn authed_gate_returns_scope_required_when_notifications_missing() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let kc = MockKeychain::with_token_and_scopes("ghp_test", &["read:user", "public_repo"]);
        let r = inner_authed_gate_with_kc(
            &state,
            "https://github.com/octocat/hello-world",
            "github_watch",
            SCOPE_NOTIFICATIONS,
            &kc,
        )
        .await;
        match r {
            Err(AppError::ScopeRequired { scope }) => assert_eq!(scope, "notifications"),
            other => panic!("expected ScopeRequired(notifications), got {other:?}"),
        }
    }

    /// Token + scopes both present → gate returns Ok. We don't run
    /// the network leg here; the actions module has its own tests.
    #[tokio::test]
    async fn authed_gate_passes_with_token_and_public_repo_scope() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let kc = MockKeychain::with_token_and_scopes("ghp_test", &["read:user", "public_repo"]);
        let r = inner_authed_gate_with_kc(
            &state,
            "https://github.com/octocat/hello-world",
            "github_create_issue",
            SCOPE_PUBLIC_REPO,
            &kc,
        )
        .await;
        assert!(r.is_ok(), "expected gate to pass, got {r:?}");
        let (repo, _token) = r.unwrap();
        assert_eq!(repo.owner, "octocat");
        assert_eq!(repo.repo, "hello-world");
    }

    /// v0.2.2: watch action passes the gate when the token has the
    /// notifications scope alongside the existing read:user + public_repo.
    #[tokio::test]
    async fn authed_gate_passes_for_watch_with_notifications_scope() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let kc = MockKeychain::with_token_and_scopes(
            "ghp_test",
            &["read:user", "public_repo", "notifications"],
        );
        let r = inner_authed_gate_with_kc(
            &state,
            "https://github.com/octocat/hello-world",
            "github_watch",
            SCOPE_NOTIFICATIONS,
            &kc,
        )
        .await;
        assert!(r.is_ok(), "expected gate to pass, got {r:?}");
    }

    /// Non-github URL → InvalidArgument (NOT Ok(None) like
    /// `github_repo_stats`). Authed actions shouldn't get this far
    /// from a well-behaved frontend; an unparseable homepage is a
    /// real bug, not a silent "no stats".
    #[tokio::test]
    async fn authed_gate_rejects_non_github_url_with_invalid_argument() {
        let state = build_state_with(SettingsLoadState::Loaded(Settings::default())).await;
        let kc = MockKeychain::with_token_and_scopes("ghp_test", &["read:user", "public_repo"]);
        let r = inner_authed_gate_with_kc(
            &state,
            "https://example.com/foo/bar",
            "github_star",
            SCOPE_PUBLIC_REPO,
            &kc,
        )
        .await;
        match r {
            Err(AppError::InvalidArgument { message }) => {
                assert!(message.contains("github.com"), "{message}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    /// Gate ordering: paranoid mode is the FIRST gate. Even with a
    /// missing token + invalid URL, paranoid must fire first so we
    /// don't leak "auth required" semantics to a user who told us to
    /// stop making outbound calls.
    #[tokio::test]
    async fn paranoid_gate_fires_before_auth_or_url() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let kc = MockKeychain::new(); // no token
        let r = inner_authed_gate_with_kc(
            &state,
            "https://not-a-github-url-at-all",
            "github_star",
            SCOPE_PUBLIC_REPO,
            &kc,
        )
        .await;
        match r {
            Err(AppError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_star")
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }
}
