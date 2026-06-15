//! Typed error model for Agency Agents.
//!
//! `AppError` is the single error type returned by every Tauri command.
//! It serializes to a tagged JSON shape (`code` discriminator) so the
//! frontend can `switch (err.code)` over a closed union.

use serde::Serialize;
use thiserror::Error;

/// Errors returned by every Tauri command.
///
/// Serializes with `#[serde(tag = "code")]` so the JSON shape on the
/// frontend matches `AppErrorPayload` in `src/lib/types.ts`.
#[derive(Debug, Error, Serialize, Clone)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AppError {
    #[error("failed to parse JSON output: {message}")]
    #[serde(rename_all = "camelCase")]
    JsonParse {
        command: String,
        message: String,
        raw_excerpt: String,
    },

    #[error("I/O error: {message}")]
    Io { message: String },

    #[error("network error fetching {url}: {message}")]
    Network { url: String, message: String },

    #[error("HTTP {status} fetching {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },

    #[error("internal error: {message}")]
    Internal { message: String },

    /// Paranoid/Offline mode is on (or settings are corrupt → fail closed).
    /// The `feature` field identifies which outbound command was
    /// rejected, so the UI can route the toast to the right setting.
    #[error("paranoid mode is enabled; outbound feature {feature} is blocked")]
    ParanoidModeBlocked { feature: String },

    /// GitHub's REST API returned a rate-limit response (typically
    /// `403 Forbidden` with `X-RateLimit-Remaining: 0`). `reset_at` is
    /// the unix timestamp from the `X-RateLimit-Reset` header when the
    /// budget refills. **Callers must not retry** — the only correct
    /// response is to surface the limit with a "Sign in to lift the limit" CTA.
    #[error("github rate limit exceeded; resets at {reset_at}")]
    #[serde(rename_all = "camelCase")]
    GithubRateLimited { reset_at: u64 },

    /// The macOS Keychain refused to store or retrieve a credential.
    /// **No disk fallback** — the OAuth token never lands on disk. The
    /// frontend should surface this with a "Keychain unavailable" error
    /// rather than offering a workaround that weakens the security posture.
    #[error("keychain unavailable: {message}")]
    KeychainUnavailable { message: String },

    /// An authenticated GitHub action was attempted without a stored token.
    /// The frontend routes this to the "Sign in" CTA in Settings → GitHub.
    #[error("github authentication required")]
    AuthRequired,

    /// An authenticated GitHub action requires an OAuth scope the stored
    /// token doesn't carry. Surfaces the missing scope so the frontend can
    /// prompt a re-sign-in with the expanded scope set.
    #[error("github scope required: {scope}")]
    ScopeRequired { scope: String },

    /// The updater downloaded an artifact whose sha256 did not match the
    /// value declared in the manifest. **Fail closed**: the .dmg is deleted
    /// before this error returns. Currently only constructed by the mock
    /// backend in `commands::updater::tests` — the production plugin path
    /// checks minisign only; this is the defense-in-depth hook for a separate
    /// manifest-sha256 check.
    #[allow(dead_code)]
    #[error("update artifact hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    /// Minisign verification of the downloaded update artifact failed against
    /// the embedded public key. **Fail closed**: the .dmg is deleted before
    /// this error returns.
    #[error("update signature verification failed: {message}")]
    SignatureVerificationFailed { message: String },

    /// The updater was asked to install a version the same as, or older than,
    /// the currently-running build — the explicit downgrade-attack defense.
    #[error("update would downgrade {current} to {target}; refusing")]
    DowngradeRejected { current: String, target: String },
}

// ---------- From impls ----------

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::JsonParse {
            command: String::new(),
            message: e.to_string(),
            raw_excerpt: String::new(),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        // `e.url()` returns None for some reqwest error variants that fire
        // before the URL is attached (DNS, connect-time, redirect-policy).
        // Falling back to a placeholder keeps the toast message parseable.
        let url = e
            .url()
            .map(|u| u.as_str().to_string())
            .unwrap_or_else(|| "<unknown url>".to_string());
        if let Some(status) = e.status() {
            AppError::HttpStatus {
                url,
                status: status.as_u16(),
            }
        } else {
            AppError::Network {
                url,
                message: e.to_string(),
            }
        }
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Helper: serialize an AppError and pull out the `code` discriminator.
    fn code_of(err: &AppError) -> String {
        let v: Value = serde_json::to_value(err).expect("serialize");
        v.get("code")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| panic!("no `code` field in serialized error: {:?}", v))
    }

    #[test]
    fn json_parse_serializes_with_camel_case_fields() {
        let err = AppError::JsonParse {
            command: "corpus_refresh".into(),
            message: "expected `,`".into(),
            raw_excerpt: "{...}".into(),
        };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "json_parse");
        assert_eq!(v["rawExcerpt"], "{...}");
        assert!(v.get("raw_excerpt").is_none());
    }

    #[test]
    fn io_serializes_with_message() {
        let err = AppError::Io { message: "ENOENT".into() };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "io");
        assert_eq!(v["message"], "ENOENT");
    }

    #[test]
    fn network_serializes_with_url_and_message() {
        let err = AppError::Network {
            url: "https://codeload.github.com/...".into(),
            message: "timeout".into(),
        };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "network");
        assert_eq!(v["url"], "https://codeload.github.com/...");
    }

    #[test]
    fn http_status_serializes_with_url_and_status() {
        let err = AppError::HttpStatus {
            url: "https://codeload.github.com/foo".into(),
            status: 503,
        };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "http_status");
        assert_eq!(v["status"], 503);
    }

    #[test]
    fn invalid_argument_serializes_with_message() {
        let err = AppError::InvalidArgument {
            message: "agent slug is empty".into(),
        };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "invalid_argument");
        assert_eq!(v["message"], "agent slug is empty");
    }

    #[test]
    fn internal_serializes_with_message() {
        let err = AppError::Internal { message: "boom".into() };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "internal");
        assert_eq!(v["message"], "boom");
    }

    #[test]
    fn paranoid_mode_blocked_serializes_with_feature() {
        let err = AppError::ParanoidModeBlocked { feature: "corpus_refresh".into() };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "paranoid_mode_blocked");
        assert_eq!(v["feature"], "corpus_refresh");
    }

    #[test]
    fn github_rate_limited_serializes_with_camel_case_reset_at() {
        let err = AppError::GithubRateLimited { reset_at: 1_700_000_000 };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "github_rate_limited");
        assert_eq!(v["resetAt"], 1_700_000_000u64);
        assert!(v.get("reset_at").is_none(), "must not emit snake_case `reset_at`");
    }

    #[test]
    fn keychain_unavailable_serializes_with_message() {
        let err = AppError::KeychainUnavailable { message: "no entry".into() };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "keychain_unavailable");
    }

    #[test]
    fn auth_required_serializes_to_auth_required_code() {
        assert_eq!(code_of(&AppError::AuthRequired), "auth_required");
    }

    #[test]
    fn scope_required_serializes_with_scope() {
        let err = AppError::ScopeRequired { scope: "public_repo".into() };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "scope_required");
        assert_eq!(v["scope"], "public_repo");
    }

    #[test]
    fn hash_mismatch_serializes_with_expected_and_actual() {
        let err = AppError::HashMismatch {
            expected: "deadbeef".into(),
            actual: "feedface".into(),
        };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "hash_mismatch");
    }

    #[test]
    fn signature_verification_failed_serializes_with_message() {
        let err = AppError::SignatureVerificationFailed { message: "bad signature".into() };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "signature_verification_failed");
    }

    #[test]
    fn downgrade_rejected_serializes_with_current_and_target() {
        let err = AppError::DowngradeRejected {
            current: "0.3.0".into(),
            target: "0.2.1".into(),
        };
        let v: Value = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "downgrade_rejected");
    }

    #[test]
    fn io_error_maps_to_app_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
        let err: AppError = io_err.into();
        match err {
            AppError::Io { message } => assert!(message.contains("no access")),
            other => panic!("expected Io, got {:?}", other),
        }
    }

    #[test]
    fn serde_json_error_maps_to_app_error_json_parse() {
        let bad: Result<serde_json::Value, _> = serde_json::from_str("{not json");
        let err: AppError = bad.unwrap_err().into();
        match err {
            AppError::JsonParse { .. } => {}
            other => panic!("expected JsonParse, got {:?}", other),
        }
    }
}
