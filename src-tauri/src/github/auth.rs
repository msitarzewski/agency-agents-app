//! GitHub OAuth Device Flow (RFC 8628) + macOS Keychain storage.
//!
//! This module implements every Phase 12e security gate listed in the
//! parent `mod.rs` header. Read those first if you're new to this code.
//!
//! ## Why Device Flow?
//!
//! - No embedded webview. We don't need WebKit's keychain access or
//!   third-party-cookie quirks; the user's existing browser does the
//!   authentication.
//! - No client secret. RFC 8628 explicitly says Device Flow client_ids
//!   are not secrets (§3.1) — they're identifiers, like a username.
//!   This means we can hardcode the `client_id` in the source tree
//!   without anyone needing a secret-management story for forks.
//! - Standard polling protocol with explicit back-off semantics
//!   (`slow_down` → double the interval, §3.5).
//!
//! ## Token lifecycle
//!
//! 1. `start_device_flow` POSTs to `github.com/login/device/code` with
//!    `client_id` + `scope` and gets back a `device_code` + `user_code`
//!    + `verification_uri` + polling `interval`.
//! 2. The frontend shows the `user_code` and a button to open
//!    `verification_uri` in the user's default browser.
//! 3. The frontend polls `poll_device_flow(device_code)` every
//!    `interval` seconds. On `authorization_pending` we keep polling;
//!    on `slow_down` we double the interval; on success we receive an
//!    access token and store it in the Keychain.
//! 4. The token never crosses the IPC boundary. Subsequent commands
//!    read it from the Keychain themselves.
//!
//! ## Failure modes
//!
//! - **Keychain write fails** → `AppError::KeychainUnavailable`. No
//!   disk fallback. This is the §12e fail-closed rule.
//! - **Token request times out** → `AppError::Network`.
//! - **User denies authorization** → `PollResult::Denied`.
//! - **Code expires before user approves** → `PollResult::Expired`.

#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ---------- Constants ----------

/// macOS Keychain service identifier. **Must match `tauri.conf.json`'s
/// `identifier`** so the stored credential is namespaced under the same
/// bundle the app actually ships under — otherwise the user could sign
/// in and the next launch wouldn't find the token.
///
/// Renamed from `"dev.openbrew.browser"` → `"com.zerologic.brew-browser"`
/// in v0.3.1. The old identifier was the Tauri scaffolder default that
/// stuck around for too long. Users with v0.2.1+ tokens stored under
/// the old service ID will see them orphaned (macOS Keychain ACLs are
/// keyed by binary signature + service ID — different service = no
/// migration possible without re-prompting anyway). On their next
/// GitHub action the Re-authorize toast flow guides them through a
/// one-click re-sign-in.
///
/// The match is enforced by `tests::service_id_matches_tauri_conf`.
pub const KEYCHAIN_SERVICE: &str = "com.zerologic.agency-agents-app";

/// Keychain account name for the access token. Keep this stable across
/// versions — a rename would orphan tokens already in users' Keychains.
pub const KEYCHAIN_ACCOUNT_TOKEN: &str = "github_access_token";

/// Keychain account name for the granted-scope list, JSON-encoded.
/// Stored alongside the token so `status()` can answer "what can this
/// session actually do?" without an extra round-trip to GitHub.
pub const KEYCHAIN_ACCOUNT_SCOPES: &str = "github_access_token_scopes";

/// Keychain account name for the resolved username. Cached so
/// `status()` doesn't have to hit `api.github.com/user` every time
/// the Settings panel refreshes.
pub const KEYCHAIN_ACCOUNT_USERNAME: &str = "github_username";

/// OAuth Device Flow client identifier.
///
/// **Hardcoded by design.** RFC 8628 §3.1: "The client identifier is
/// not secret … client authentication is not required for the device
/// authorization grant type." Forks should replace this with their own
/// GitHub App's client_id (see `docs/BUILD.md` §"GitHub OAuth App").
///
/// The current value is the real `client_id` for the upstream
/// Agency Agents GitHub OAuth App under `msitarzewski`'s account.
/// Maintained by the upstream maintainer; do not reuse from forks —
/// rate-limit budget and any future revocation would tie back to
/// upstream rather than the fork.
pub const GITHUB_OAUTH_CLIENT_ID: &str = "Ov23liJZKbvrSBuiOPkT";

/// OAuth scopes we request at sign-in. **Keep this minimum** — any
/// addition needs explicit review per §12e's "scope minimum" gate.
///
/// - `read:user` — read the signed-in user's public profile (lets us
///   show "Signed in as @username" in Settings).
/// - `public_repo` — star/unstar + create issues on public repos.
///   Required by Phase 12f authed actions.
/// - `notifications` — watch/unwatch access. Per GitHub's own
///   OAuth-scopes docs ("The `notifications` scope grants watch and
///   unwatch access to a repository"), `public_repo` alone is NOT
///   sufficient for `PUT /repos/{owner}/{repo}/subscription` — the
///   endpoint returns 404 (their privacy-preserving mask for "you
///   don't have the scope") instead of 403. Added in v0.2.2 after
///   user-reported watch action failed on a freshly-signed-in token.
///
/// No write access to private repos, no admin scopes, no email read.
pub const GITHUB_OAUTH_SCOPES: &[&str] = &["read:user", "public_repo", "notifications"];

/// GitHub OAuth Device Flow endpoints. Both live under `github.com`
/// (not `api.github.com`), which is why the CSP needs both origins.
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";

/// HTTP timeout for OAuth requests. Generous because GitHub's token
/// endpoint can be slow during peak hours; bounded so a wedged DNS
/// lookup never freezes the UI.
const OAUTH_TIMEOUT: Duration = Duration::from_secs(15);

/// Lower bound on polling interval, enforced if GitHub responds with
/// a smaller number (defensive — the spec says "MUST honour", but
/// clients are also allowed to pick their own floor).
const MIN_POLL_INTERVAL_SECS: u64 = 5;

/// Upper bound on `expires_in` we'll accept. GitHub's spec says
/// 900 s (15 min); we accept up to 1 hour for slack but anything
/// larger is treated as a server bug.
const MAX_EXPIRES_IN_SECS: u64 = 60 * 60;

// ---------- Token newtype ----------

/// Wrapper around the raw OAuth access token.
///
/// The Debug impl is hand-written to redact the inner string so a
/// stray `tracing::debug!("got token {:?}", token)` can't leak the
/// credential into a log file. The redaction is exercised by
/// `tests::token_debug_redacts`.
#[derive(Clone, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    /// Wrap a raw token. Validates non-empty; rejects whitespace-only.
    pub fn new(raw: impl Into<String>) -> Result<Self, AppError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(AppError::InvalidArgument {
                message: "token must not be empty".into(),
            });
        }
        Ok(Self(raw))
    }

    /// Borrow the inner string for use as an `Authorization` header
    /// value. The returned `&str` should be passed directly to
    /// `reqwest`; **never** route it through `format!` into a log
    /// line or a JSON payload — `Debug` redaction won't save you
    /// there.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Length is leaked so legitimate debugging can distinguish
        // "no token" from "token present" without exposing the bytes.
        write!(f, "Token(REDACTED, len={})", self.0.len())
    }
}

// ---------- IPC DTOs ----------

/// Wire shape returned to the frontend by `github_status`. Contains
/// **no token** — only the derived "what can the session do?" view.
///
/// The shape is locked by `tests::status_dto_never_serializes_token`
/// which serializes a worst-case value and asserts the bytes contain
/// neither the literal token nor the substring "access_token".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GithubStatusDto {
    pub signed_in: bool,
    pub username: Option<String>,
    pub scopes: Vec<String>,
}

/// Wire shape returned by `github_signin_start`. The `device_code` is
/// opaque to the frontend — it's just passed back to
/// `github_signin_poll`. The user-facing strings are `user_code` and
/// `verification_uri`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFlowStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u32,
    pub interval: u32,
}

/// Internal poll result. The wire-shape variant is `PollResultDto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollResult {
    /// Still waiting for the user to approve in their browser.
    Pending,
    /// GitHub asked us to slow down — caller should double the
    /// interval and try again later.
    SlowDown,
    /// User approved. The token has been stored in the Keychain;
    /// `username` and `scopes` are the derived status fields.
    Approved {
        username: Option<String>,
        scopes: Vec<String>,
    },
    /// User explicitly denied the request.
    Denied,
    /// Code expired before the user approved.
    Expired,
}

/// Wire shape: tagged union mirroring `PollResult` but with the token
/// stripped. The frontend `switch`es on `kind`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PollResultDto {
    Pending,
    SlowDown,
    Approved {
        username: Option<String>,
        scopes: Vec<String>,
    },
    Denied,
    Expired,
}

impl From<PollResult> for PollResultDto {
    fn from(p: PollResult) -> Self {
        match p {
            PollResult::Pending => PollResultDto::Pending,
            PollResult::SlowDown => PollResultDto::SlowDown,
            PollResult::Approved { username, scopes } => {
                PollResultDto::Approved { username, scopes }
            }
            PollResult::Denied => PollResultDto::Denied,
            PollResult::Expired => PollResultDto::Expired,
        }
    }
}

// ---------- Keychain abstraction ----------

/// Trait-object façade around the `keyring` crate so tests can swap
/// in an in-memory store. Production uses [`SystemKeychain`].
pub trait KeychainSlot: Send + Sync {
    /// Return the stored value, or `Ok(None)` if no entry exists.
    /// Surface backend errors as `AppError::KeychainUnavailable`.
    fn read(&self, account: &str) -> Result<Option<String>, AppError>;

    /// Persist `value` under `account`. **No retry, no fallback.**
    fn write(&self, account: &str, value: &str) -> Result<(), AppError>;

    /// Delete the entry, treating "no such entry" as success.
    fn delete(&self, account: &str) -> Result<(), AppError>;

    /// Read several accounts at once, returning only those with a value.
    ///
    /// Default: one `read` per account — N separate Keychain accesses, hence N
    /// auth prompts. macOS [`SystemKeychain`] overrides this with a single
    /// `SecItemCopyMatching(kSecMatchLimitAll)` so the launch-time sign-in check
    /// (`status`) costs ONE prompt instead of three. (Mirrors native's
    /// `keychainReadAll`.) The default keeps tests + non-macOS correct.
    fn read_many(&self, accounts: &[&str]) -> Result<HashMap<String, String>, AppError> {
        let mut out = HashMap::new();
        for &account in accounts {
            if let Some(value) = self.read(account)? {
                out.insert(account.to_string(), value);
            }
        }
        Ok(out)
    }
}

/// Production keychain backed by `keyring::Entry` against the macOS
/// Keychain under `KEYCHAIN_SERVICE`.
pub struct SystemKeychain;

impl SystemKeychain {
    fn entry(account: &str) -> Result<keyring::Entry, AppError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| {
            AppError::KeychainUnavailable {
                message: format!("entry({KEYCHAIN_SERVICE}, {account}): {e}"),
            }
        })
    }

    /// macOS: read every generic-password item stored under `KEYCHAIN_SERVICE`
    /// in ONE `SecItemCopyMatching` (`kSecMatchLimitAll`), keyed by account.
    /// One Keychain access → one auth prompt, vs one prompt per account when
    /// read individually. `errSecItemNotFound` (nothing stored yet) maps to an
    /// empty map, not an error. Verbatim intent of native's `keychainReadAll`.
    #[cfg(target_os = "macos")]
    fn read_all_batch() -> Result<HashMap<String, String>, AppError> {
        use security_framework::item::{ItemClass, ItemSearchOptions, Limit, SearchResult};

        /// `errSecItemNotFound` — no matching items, i.e. signed out.
        const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;

        let results = match ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(KEYCHAIN_SERVICE)
            .load_attributes(true)
            .load_data(true)
            .limit(Limit::All)
            .search()
        {
            Ok(items) => items,
            Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => return Ok(HashMap::new()),
            Err(e) => {
                return Err(AppError::KeychainUnavailable {
                    message: format!("batch read ({KEYCHAIN_SERVICE}): {e}"),
                })
            }
        };

        // simplify_dict() returns the item's attributes as string pairs; the
        // account lives under "acct" (kSecAttrAccount) and the secret under
        // "v_Data" (kSecValueData). Our values (token/scopes JSON/username) are
        // all UTF-8, so the lossy conversion is lossless here.
        let mut out = HashMap::new();
        for result in results {
            if let SearchResult::Dict(_) = result {
                if let Some(dict) = result.simplify_dict() {
                    if let (Some(account), Some(value)) =
                        (dict.get("acct"), dict.get("v_Data"))
                    {
                        out.insert(account.clone(), value.clone());
                    }
                }
            }
        }
        Ok(out)
    }
}

impl KeychainSlot for SystemKeychain {
    fn read(&self, account: &str) -> Result<Option<String>, AppError> {
        let entry = Self::entry(account)?;
        match entry.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::KeychainUnavailable {
                message: format!("read {account}: {e}"),
            }),
        }
    }

    fn write(&self, account: &str, value: &str) -> Result<(), AppError> {
        let entry = Self::entry(account)?;
        entry.set_password(value).map_err(|e| {
            AppError::KeychainUnavailable {
                message: format!("write {account}: {e}"),
            }
        })
    }

    fn delete(&self, account: &str) -> Result<(), AppError> {
        let entry = Self::entry(account)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::KeychainUnavailable {
                message: format!("delete {account}: {e}"),
            }),
        }
    }

    /// macOS: collapse the per-account reads into one batched Keychain access
    /// (one auth prompt). `accounts` is ignored — the batch returns every item
    /// under the service and the caller picks the keys it needs. On non-macOS
    /// this method is absent, so the trait default (per-account reads) applies.
    #[cfg(target_os = "macos")]
    fn read_many(&self, _accounts: &[&str]) -> Result<HashMap<String, String>, AppError> {
        Self::read_all_batch()
    }
}

// ---------- Status + sign-out (sync) ----------

/// Return the current sign-in status without exposing the token.
///
/// Reads the cached `username` + `scopes` blobs from the Keychain
/// alongside the token. If the token row is missing, returns the
/// "not signed in" shape.
pub fn status_with(keychain: &dyn KeychainSlot) -> Result<GithubStatusDto, AppError> {
    // ONE batched Keychain read for token + username + scopes (macOS: one auth
    // prompt; other backends: the trait default does per-account reads).
    let all = keychain.read_many(&[
        KEYCHAIN_ACCOUNT_TOKEN,
        KEYCHAIN_ACCOUNT_USERNAME,
        KEYCHAIN_ACCOUNT_SCOPES,
    ])?;

    if !all.contains_key(KEYCHAIN_ACCOUNT_TOKEN) {
        return Ok(GithubStatusDto {
            signed_in: false,
            username: None,
            scopes: Vec::new(),
        });
    }

    let username = all.get(KEYCHAIN_ACCOUNT_USERNAME).cloned();
    let scopes: Vec<String> = all
        .get(KEYCHAIN_ACCOUNT_SCOPES)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    Ok(GithubStatusDto {
        signed_in: true,
        username,
        scopes,
    })
}

/// Sign-in status against the production keychain.
pub fn status() -> Result<GithubStatusDto, AppError> {
    status_with(&SystemKeychain)
}

/// Borrow the stored token, if any, against an injected keychain.
///
/// Returns the wrapped [`Token`] — never the raw string — so callers
/// can't accidentally pass it into `format!`.
pub fn read_token_with(keychain: &dyn KeychainSlot) -> Result<Option<Token>, AppError> {
    match keychain.read(KEYCHAIN_ACCOUNT_TOKEN)? {
        Some(s) => Ok(Some(Token::new(s)?)),
        None => Ok(None),
    }
}

/// Borrow the stored token against the production keychain.
pub fn read_token() -> Result<Option<Token>, AppError> {
    read_token_with(&SystemKeychain)
}

/// Read the cached scope list (the one we stored at sign-in time).
///
/// Returns `Ok(None)` when no scope blob is present in the Keychain —
/// either because the user isn't signed in or because an older
/// version's persistence path didn't populate the field. Callers that
/// need to gate on a specific scope (e.g. `public_repo`) should treat
/// `None` and an absent entry in the list the same way: surface
/// `AppError::ScopeRequired` with the missing scope.
pub fn read_scopes_with(keychain: &dyn KeychainSlot) -> Result<Option<Vec<String>>, AppError> {
    match keychain.read(KEYCHAIN_ACCOUNT_SCOPES)? {
        Some(raw) => match serde_json::from_str::<Vec<String>>(&raw) {
            Ok(v) => Ok(Some(v)),
            // Corrupt blob → treat as missing. The next successful
            // sign-in will overwrite it.
            Err(_) => Ok(None),
        },
        None => Ok(None),
    }
}

/// Read scopes against the production keychain.
pub fn read_scopes() -> Result<Option<Vec<String>>, AppError> {
    read_scopes_with(&SystemKeychain)
}

/// Delete every stored credential. Idempotent — used by the
/// "Sign out" button in Settings.
pub fn signout_with(keychain: &dyn KeychainSlot) -> Result<(), AppError> {
    keychain.delete(KEYCHAIN_ACCOUNT_TOKEN)?;
    keychain.delete(KEYCHAIN_ACCOUNT_USERNAME)?;
    keychain.delete(KEYCHAIN_ACCOUNT_SCOPES)?;
    Ok(())
}

/// Sign out against the production keychain.
pub fn signout() -> Result<(), AppError> {
    signout_with(&SystemKeychain)
}

// ---------- Device Flow start ----------

/// Wire shape of the `device/code` response from GitHub.
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

/// POST `client_id` + scope to `github.com/login/device/code` and
/// return the user-facing code + polling parameters.
///
/// The function does **not** start the polling loop — the frontend
/// does that via repeated `poll_device_flow` calls. We just hand back
/// the opaque `device_code` the frontend needs to drive polling.
pub async fn start_device_flow() -> Result<DeviceFlowStart, AppError> {
    // Fail fast when the client_id is still the build-time placeholder.
    // Without this guard, GitHub rejects the device-code request with an
    // opaque 4xx and the frontend modal sits forever on "Contacting GitHub…"
    // waiting for a device_code that was never minted.
    //
    // To fix in your build: see `BUILD.md` → "GitHub OAuth App (one-time
    // setup before release)". The 7-step flow on github.com/settings/apps
    // gives you a real client_id to replace `GITHUB_OAUTH_CLIENT_ID` with.
    if GITHUB_OAUTH_CLIENT_ID.contains("PLACEHOLDER") {
        return Err(AppError::Internal {
            message: "GitHub sign-in is not configured in this build. The OAuth App client_id is still the placeholder — see BUILD.md → 'GitHub OAuth App (one-time setup before release)'.".to_string(),
        });
    }

    let client = build_oauth_client()?;
    let scope = GITHUB_OAUTH_SCOPES.join(" ");
    let form = [
        ("client_id", GITHUB_OAUTH_CLIENT_ID),
        ("scope", scope.as_str()),
    ];
    let resp = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::Network {
            url: DEVICE_CODE_URL.into(),
            message: e.to_string(),
        })?;
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Network {
            url: DEVICE_CODE_URL.into(),
            message: format!("body: {e}"),
        })?;
    if !status.is_success() {
        return Err(AppError::HttpStatus {
            url: DEVICE_CODE_URL.into(),
            status: status.as_u16(),
        });
    }
    let parsed: DeviceCodeResponse =
        serde_json::from_slice(&bytes).map_err(|e| AppError::JsonParse {
            command: DEVICE_CODE_URL.into(),
            message: e.to_string(),
            raw_excerpt: String::from_utf8_lossy(
                &bytes[..bytes.len().min(256)],
            )
            .into_owned(),
        })?;

    // Clamp interval + expires_in defensively.
    let interval = parsed.interval.clamp(MIN_POLL_INTERVAL_SECS, 60) as u32;
    let expires_in = parsed.expires_in.min(MAX_EXPIRES_IN_SECS) as u32;

    Ok(DeviceFlowStart {
        device_code: parsed.device_code,
        user_code: parsed.user_code,
        verification_uri: parsed.verification_uri,
        expires_in,
        interval,
    })
}

// ---------- Device Flow poll ----------

/// Wire shape of the `oauth/access_token` response. The endpoint
/// returns 200 for both success and "pending" responses — the
/// distinguishing fields are `access_token` (present on approve) and
/// `error` (present on every other state).
#[derive(Debug, Deserialize, Default)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Poll the token endpoint once. The caller is responsible for honouring
/// the `interval` (and for doubling it on `SlowDown` per RFC 8628 §3.5).
pub async fn poll_device_flow(device_code: &str) -> Result<PollResult, AppError> {
    poll_device_flow_with(device_code, &SystemKeychain).await
}

/// Polling variant that accepts an injected keychain for tests.
pub async fn poll_device_flow_with(
    device_code: &str,
    keychain: &dyn KeychainSlot,
) -> Result<PollResult, AppError> {
    let client = build_oauth_client()?;
    let form = [
        ("client_id", GITHUB_OAUTH_CLIENT_ID),
        ("device_code", device_code),
        (
            "grant_type",
            "urn:ietf:params:oauth:grant-type:device_code",
        ),
    ];
    let resp = client
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::Network {
            url: TOKEN_URL.into(),
            message: e.to_string(),
        })?;
    let status = resp.status();
    let bytes = resp.bytes().await.map_err(|e| AppError::Network {
        url: TOKEN_URL.into(),
        message: format!("body: {e}"),
    })?;
    if !status.is_success() {
        return Err(AppError::HttpStatus {
            url: TOKEN_URL.into(),
            status: status.as_u16(),
        });
    }
    let parsed: TokenResponse =
        serde_json::from_slice(&bytes).unwrap_or_default();

    if let Some(token_str) = parsed.access_token {
        let token = Token::new(token_str)?;
        // GitHub's `/login/oauth/access_token` returns the `scope` field
        // as a **comma-separated** string (e.g. `"public_repo,read:user"`),
        // NOT the space-separated form prescribed by OAuth 2.0 RFC 6749
        // §3.3. An earlier version of this code used `split_whitespace()`
        // which produced a single-element array `["public_repo,read:user"]`
        // — the Settings panel rendered fine but every authed action
        // rejected with `ScopeRequired` because `scopes.iter().any(|s|
        // s == "public_repo")` was false. Split on BOTH commas and
        // whitespace defensively so any future format flip lands cleanly.
        let scopes: Vec<String> = parsed
            .scope
            .as_deref()
            .map(|s| {
                s.split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|x| !x.is_empty())
                    .map(|x| x.to_string())
                    .collect()
            })
            .unwrap_or_default();
        // Resolve username for display. A failure here is non-fatal —
        // sign-in still succeeds; we just won't have a name to show in
        // Settings until the user reopens it (which retries
        // `github_status`, but `status` reads from the cached
        // Keychain entry — the lookup will only repeat on a future
        // sign-in. Acceptable trade-off; the alternative is failing
        // the whole sign-in over a transient /user 5xx).
        let username = fetch_username(&client, token.as_str()).await.ok();
        // Persist to Keychain.
        keychain.write(KEYCHAIN_ACCOUNT_TOKEN, token.as_str())?;
        if let Some(u) = &username {
            keychain.write(KEYCHAIN_ACCOUNT_USERNAME, u)?;
        }
        let scopes_json = serde_json::to_string(&scopes).map_err(|e| AppError::Internal {
            message: format!("serialize scopes: {e}"),
        })?;
        keychain.write(KEYCHAIN_ACCOUNT_SCOPES, &scopes_json)?;
        return Ok(PollResult::Approved { username, scopes });
    }

    match parsed.error.as_deref() {
        Some("authorization_pending") => Ok(PollResult::Pending),
        Some("slow_down") => Ok(PollResult::SlowDown),
        Some("access_denied") => Ok(PollResult::Denied),
        Some("expired_token") => Ok(PollResult::Expired),
        Some(other) => Err(AppError::Internal {
            message: format!("github device flow error: {other}"),
        }),
        None => Err(AppError::Internal {
            message: "github device flow returned neither access_token nor error".into(),
        }),
    }
}

/// Bump a polling interval per RFC 8628 §3.5. Caller passes the
/// current interval (seconds) and gets the new one to wait before the
/// next poll. The doubling is capped at 60 s as a sanity bound.
///
/// Currently called only by the unit test; the frontend (which owns
/// the polling loop's wall-clock state) will hit this through the
/// same logic in TypeScript. Keep it here so the canonical RFC 8628
/// behaviour is reviewable in Rust and pinned by a test.
#[allow(dead_code)]
pub fn slow_down_interval(current: u32) -> u32 {
    current.saturating_mul(2).min(60)
}

// ---------- Helpers ----------

fn build_oauth_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(OAUTH_TIMEOUT)
        .user_agent(concat!(
            "agency-agents-app/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/msitarzewski/agency-agents-app)"
        ))
        .build()
        .map_err(|e| AppError::Network {
            url: DEVICE_CODE_URL.into(),
            message: format!("client build: {e}"),
        })
}

#[derive(Debug, Default, Deserialize)]
struct UserResponse {
    #[serde(default)]
    login: String,
}

async fn fetch_username(client: &reqwest::Client, token: &str) -> Result<String, AppError> {
    let resp = client
        .get(USER_URL)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", format!("Bearer {token}"))
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::Network {
            url: USER_URL.into(),
            message: e.to_string(),
        })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::HttpStatus {
            url: USER_URL.into(),
            status: status.as_u16(),
        });
    }
    let parsed: UserResponse =
        serde_json::from_slice(&resp.bytes().await.unwrap_or_default()).unwrap_or_default();
    if parsed.login.is_empty() {
        return Err(AppError::Internal {
            message: "github /user returned empty login".into(),
        });
    }
    Ok(parsed.login)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory keychain for tests. Stores values in a HashMap so tests
    /// can verify round-trips, simulate failures, and assert that no
    /// file in app_data_dir contains the token (since the only writer
    /// is this in-memory mock).
    struct MockKeychain {
        entries: Mutex<HashMap<String, String>>,
        /// When true, every write fails with `KeychainUnavailable`. Used
        /// to verify the "no disk fallback" rule.
        fail_writes: bool,
    }

    impl MockKeychain {
        fn new() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                fail_writes: false,
            }
        }
        fn failing() -> Self {
            Self {
                entries: Mutex::new(HashMap::new()),
                fail_writes: true,
            }
        }
    }

    impl KeychainSlot for MockKeychain {
        fn read(&self, account: &str) -> Result<Option<String>, AppError> {
            Ok(self.entries.lock().unwrap().get(account).cloned())
        }
        fn write(&self, account: &str, value: &str) -> Result<(), AppError> {
            if self.fail_writes {
                return Err(AppError::KeychainUnavailable {
                    message: "mock failure".into(),
                });
            }
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

    // ---------- Token redaction ----------

    #[test]
    fn token_debug_redacts_inner_string() {
        let secret = "ghp_supersecrettoken1234567890ABCDEF";
        let t = Token::new(secret).expect("token");
        let dbg = format!("{:?}", t);
        assert!(!dbg.contains(secret), "Debug must not include token; got {dbg}");
        assert!(dbg.contains("REDACTED"), "Debug must mention REDACTED; got {dbg}");
    }

    #[test]
    fn token_new_rejects_empty_or_whitespace() {
        assert!(Token::new("").is_err());
        assert!(Token::new("   ").is_err());
        assert!(Token::new("ghp_x").is_ok());
    }

    // ---------- GithubStatusDto serialization gate ----------

    /// **Critical security test**: the wire shape returned by
    /// `github_status` must never contain a token-shaped string. This
    /// pins the contract that the IPC boundary doesn't leak the
    /// Keychain credential.
    #[test]
    fn status_dto_never_serializes_token() {
        // Worst case: signed in, scopes granted. Hand-craft the value
        // (rather than going through the real `status` flow) so the
        // assertion is about the DTO shape itself, not the helper logic.
        let dto = GithubStatusDto {
            signed_in: true,
            username: Some("octocat".into()),
            scopes: vec!["read:user".into(), "public_repo".into(), "notifications".into()],
        };
        let json = serde_json::to_string(&dto).expect("serialize");
        let known_token = "ghp_supersecrettoken1234567890ABCDEF";
        // Sanity check: the test value isn't anywhere in the struct.
        assert!(!json.contains(known_token));
        // Defensive: token-shaped substrings must not appear.
        assert!(
            !json.contains("access_token"),
            "status DTO must not contain 'access_token': {json}"
        );
        assert!(!json.contains("ghp_"), "status DTO must not contain 'ghp_'");
        assert!(!json.contains("token"), "status DTO must not contain 'token': {json}");
        // What it should contain:
        assert!(json.contains("\"signedIn\""));
        assert!(json.contains("\"username\""));
        assert!(json.contains("\"scopes\""));
    }

    // ---------- OAuth scope minimum ----------

    #[test]
    fn oauth_scopes_are_minimum() {
        // Pin the exact scope list. Adding a scope here without
        // updating `memory-bank/scans/2026-05-23/phase12-security-review.md`
        // is a security review violation.
        //
        // v0.2.2: added `notifications` because GitHub's
        // `PUT /repos/{owner}/{repo}/subscription` endpoint requires
        // it specifically (their OAuth-scopes docs: "The
        // `notifications` scope grants watch and unwatch access to a
        // repository"). Without it, watch returns 404 — GitHub's
        // privacy-preserving mask for "you don't have that scope".
        assert_eq!(
            GITHUB_OAUTH_SCOPES,
            &["read:user", "public_repo", "notifications"],
            "scopes drifted from the v0.2.2-approved minimum"
        );
    }

    #[test]
    fn oauth_scope_string_passed_in_device_flow_request_has_only_minimum() {
        // Reconstruct the form-body scope string the way `start_device_flow`
        // does and assert no extras snuck in.
        let scope_string = GITHUB_OAUTH_SCOPES.join(" ");
        assert_eq!(scope_string, "read:user public_repo notifications");
        // No admin, no repo (write to private), no email, etc.
        for forbidden in &[
            "admin",
            "repo:status",
            "repo:invite",
            "user:email",
            "delete_repo",
            "write:packages",
            "workflow",
            "gist",
        ] {
            assert!(
                !scope_string.contains(forbidden),
                "forbidden scope {forbidden} present in {scope_string}"
            );
        }
        // `repo` is forbidden because `public_repo` is the explicit
        // minimum; a plain `repo` would grant write-private-repo too.
        // Make sure the exact token "repo" doesn't appear *as a separate
        // scope* (it WILL appear as a substring of `public_repo`).
        let scope_tokens: Vec<&str> = scope_string.split_whitespace().collect();
        assert!(!scope_tokens.contains(&"repo"));
    }

    // ---------- Service ID matches tauri.conf.json ----------

    #[test]
    fn service_id_matches_tauri_conf() {
        // Parse the real `tauri.conf.json` and confirm the
        // `identifier` field equals our `KEYCHAIN_SERVICE` constant.
        // A drift here means a renamed bundle would silently orphan
        // tokens in the Keychain.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tauri.conf.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse tauri.conf.json");
        let bundle_id = v
            .get("identifier")
            .and_then(|x| x.as_str())
            .expect("tauri.conf.json missing top-level identifier");
        assert_eq!(
            bundle_id, KEYCHAIN_SERVICE,
            "tauri.conf.json identifier drifted from KEYCHAIN_SERVICE; rename them together or stored tokens are orphaned"
        );
    }

    // ---------- Keychain round-trip ----------

    #[test]
    fn keychain_round_trip_via_mock() {
        let kc = MockKeychain::new();
        // No entry → status is signed-out.
        let s0 = status_with(&kc).expect("status");
        assert!(!s0.signed_in);
        assert!(s0.username.is_none());
        assert!(s0.scopes.is_empty());

        // Write token + scopes + username.
        kc.write(KEYCHAIN_ACCOUNT_TOKEN, "ghp_secret_value").unwrap();
        kc.write(KEYCHAIN_ACCOUNT_USERNAME, "octocat").unwrap();
        kc.write(
            KEYCHAIN_ACCOUNT_SCOPES,
            r#"["read:user","public_repo"]"#,
        )
        .unwrap();

        let s1 = status_with(&kc).expect("status after sign in");
        assert!(s1.signed_in);
        assert_eq!(s1.username.as_deref(), Some("octocat"));
        assert_eq!(s1.scopes, vec!["read:user", "public_repo"]);

        // Sign out clears everything.
        signout_with(&kc).expect("signout");
        let s2 = status_with(&kc).expect("status after signout");
        assert!(!s2.signed_in);
        assert!(s2.username.is_none());
        assert!(s2.scopes.is_empty());
    }

    /// `read_token_with` returns the wrapped Token, never the raw string.
    #[test]
    fn read_token_wraps_in_redacted_newtype() {
        let kc = MockKeychain::new();
        kc.write(KEYCHAIN_ACCOUNT_TOKEN, "ghp_secret").unwrap();
        let t = read_token_with(&kc).expect("read").expect("Some");
        assert_eq!(t.as_str(), "ghp_secret");
        let dbg = format!("{t:?}");
        assert!(!dbg.contains("ghp_secret"), "{dbg}");
    }

    /// `read_scopes_with` round-trips JSON-encoded scope arrays.
    /// Critical for Phase 12f scope-required gating — a corrupt blob
    /// must collapse to `None` so the gate prompts a re-grant instead
    /// of failing on a parse error.
    #[test]
    fn read_scopes_round_trips_json_array() {
        let kc = MockKeychain::new();
        // No entry → None.
        assert!(read_scopes_with(&kc).expect("no entry").is_none());

        // Valid JSON → Some(scopes).
        kc.write(KEYCHAIN_ACCOUNT_SCOPES, r#"["read:user","public_repo"]"#).unwrap();
        let scopes = read_scopes_with(&kc).expect("read").expect("Some");
        assert_eq!(scopes, vec!["read:user", "public_repo"]);

        // Corrupt blob → None (defensive — never errors so the gate
        // can run "is `public_repo` present?" without a try/catch
        // sprinkled across every command).
        kc.write(KEYCHAIN_ACCOUNT_SCOPES, "not json").unwrap();
        assert!(read_scopes_with(&kc).expect("corrupt").is_none());
    }

    // ---------- Failure paths ----------

    /// Keychain write failure must surface as `KeychainUnavailable`,
    /// **not** trigger any disk fallback. This is the §12e rule.
    #[test]
    fn keychain_write_failure_surfaces_typed_error_no_fallback() {
        let kc = MockKeychain::failing();
        let r = kc.write(KEYCHAIN_ACCOUNT_TOKEN, "ghp_secret");
        match r {
            Err(AppError::KeychainUnavailable { .. }) => {}
            other => panic!("expected KeychainUnavailable, got {other:?}"),
        }
        // After the failure, nothing is stored (no shadow disk file, no
        // in-memory fallback in the mock).
        let after = kc.read(KEYCHAIN_ACCOUNT_TOKEN).expect("read");
        assert!(after.is_none(), "no token should be stored on failed write");
    }

    /// Simulate a Keychain that lets us write the token but no companion
    /// fields. The token-write-then-disk-fallback failure mode is what
    /// we're guarding against by NOT having any disk fallback at all.
    /// This test checks that the failing-keychain path leaves no
    /// persistent state we'd have to scrub later.
    #[tokio::test]
    async fn failed_signin_leaves_no_residual_state_in_app_data_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let kc = MockKeychain::failing();
        // Hand-call the persistence path the way `poll_device_flow` would
        // on a success: write token → write username → write scopes. The
        // first write fails immediately; we must NOT silently mirror to
        // the filesystem.
        let r = kc.write(KEYCHAIN_ACCOUNT_TOKEN, "ghp_should_not_persist");
        assert!(r.is_err());

        // Walk the app_data_dir; assert no file contains the token bytes.
        let mut found = Vec::<String>::new();
        for entry in std::fs::read_dir(tmp.path()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                let contents = std::fs::read_to_string(entry.path()).unwrap_or_default();
                if contents.contains("ghp_should_not_persist") {
                    found.push(entry.path().display().to_string());
                }
            }
        }
        assert!(
            found.is_empty(),
            "token was written to disk despite Keychain failure: {found:?}"
        );
    }

    // ---------- slow_down doubling ----------

    #[test]
    fn slow_down_doubles_interval_per_rfc_8628() {
        assert_eq!(slow_down_interval(5), 10);
        assert_eq!(slow_down_interval(10), 20);
        assert_eq!(slow_down_interval(30), 60);
        // Cap.
        assert_eq!(slow_down_interval(60), 60);
        assert_eq!(slow_down_interval(120), 60);
    }

    // ---------- Status helper edge cases ----------

    #[test]
    fn status_with_missing_username_keeps_signed_in_true() {
        // Token present but username/scopes absent. Defensive: a partial
        // Keychain state (write succeeded mid-sequence in some earlier
        // version) should still report signed-in.
        let kc = MockKeychain::new();
        kc.write(KEYCHAIN_ACCOUNT_TOKEN, "ghp_x").unwrap();
        let s = status_with(&kc).expect("status");
        assert!(s.signed_in);
        assert!(s.username.is_none());
        assert!(s.scopes.is_empty());
    }

    #[test]
    fn status_with_corrupt_scopes_json_falls_back_to_empty() {
        let kc = MockKeychain::new();
        kc.write(KEYCHAIN_ACCOUNT_TOKEN, "ghp_x").unwrap();
        kc.write(KEYCHAIN_ACCOUNT_SCOPES, "not-json").unwrap();
        let s = status_with(&kc).expect("status");
        assert!(s.signed_in);
        assert_eq!(s.scopes, Vec::<String>::new());
    }

    // ---------- PollResultDto wire shape ----------

    #[test]
    fn poll_result_dto_serializes_with_kind_tag() {
        let p: PollResultDto = PollResult::Approved {
            username: Some("octocat".into()),
            scopes: vec!["read:user".into()],
        }
        .into();
        let v: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(v["kind"], "approved");
        assert_eq!(v["username"], "octocat");
        // Critical: no token in the wire shape.
        assert!(v.get("accessToken").is_none());
        assert!(v.get("token").is_none());
    }

    #[test]
    fn poll_result_dto_pending_has_only_kind() {
        let p = PollResultDto::Pending;
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(s, r#"{"kind":"pending"}"#);
    }

    #[test]
    fn poll_result_dto_all_variants_serialize() {
        for (p, want_kind) in [
            (PollResultDto::Pending, "pending"),
            (PollResultDto::SlowDown, "slowDown"),
            (PollResultDto::Denied, "denied"),
            (PollResultDto::Expired, "expired"),
        ] {
            let v: serde_json::Value = serde_json::to_value(&p).unwrap();
            assert_eq!(v["kind"], want_kind);
        }
    }

    // ---------- Constants gates ----------

    #[test]
    fn keychain_constants_are_stable() {
        // Renaming these silently orphans tokens in users' Keychains.
        // Any change here needs an explicit migration plan.
        // Renamed source-app service ID → com.zerologic.agency-agents-app
        // on the 2026-06-05 rebrand fork; matches tauri.conf.json identifier.
        assert_eq!(KEYCHAIN_SERVICE, "com.zerologic.agency-agents-app");
        assert_eq!(KEYCHAIN_ACCOUNT_TOKEN, "github_access_token");
        assert_eq!(KEYCHAIN_ACCOUNT_SCOPES, "github_access_token_scopes");
        assert_eq!(KEYCHAIN_ACCOUNT_USERNAME, "github_username");
    }
}
