//! GitHub authed actions (Phase 12f).
//!
//! Star / unstar / is_starred / watch / unwatch / create_issue against
//! `api.github.com`. Every function in this module:
//!
//! 1. Re-validates the `(owner, repo)` pair against the same allowlist
//!    `parse_github_url` uses (`is_valid_owner_or_repo`). Defense in
//!    depth — the caller already validated the URL, but a typo
//!    elsewhere shouldn't be allowed to slip a `..` or shell metachar
//!    into a request path.
//! 2. Sends `Authorization: Bearer <token>` via `Token::as_str()`.
//!    The token never crosses an IPC boundary and never enters a log
//!    line — `Token::Debug` redacts and reqwest swallows the header
//!    immediately.
//! 3. Honours GitHub's rate-limit response (`403` with
//!    `X-RateLimit-Remaining: 0`) by surfacing
//!    `AppError::GithubRateLimited { reset_at }`. **No retry. No
//!    exponential backoff.** Per the §12f review.
//! 4. Caps every response body at [`MAX_RESPONSE_BYTES`] (256 KiB)
//!    via `Response::content_length()` + a defensive post-read check.
//!
//! Per-endpoint shapes:
//!
//! | endpoint                                | success | notes |
//! |-----------------------------------------|---------|-------|
//! | `PUT /user/starred/{owner}/{repo}`      | 204     | repo 404 → `AppError::HttpStatus { status: 404, .. }` |
//! | `DELETE /user/starred/{owner}/{repo}`   | 204     | idempotent |
//! | `GET /user/starred/{owner}/{repo}`      | 204=yes 404=no | any other status → error |
//! | `PUT /repos/{o}/{r}/subscription`       | 200     | body `{subscribed:true,ignored:false}` |
//! | `DELETE /repos/{o}/{r}/subscription`    | 204     | idempotent |
//! | `POST /repos/{o}/{r}/issues`            | 201     | body with `title`/`body`/`labels`; returns `{number, html_url}` |
//!
//! ## Issue creation input rules
//!
//! - **Title:** ≤ 256 chars after stripping control characters
//!   (`\x00`-`\x1f` except `\t`).
//! - **Body:** ≤ 64 KiB after stripping null bytes only. Other
//!   characters pass through unchanged because GitHub renders the body
//!   as Markdown and we don't want to maul user-intended markup.
//! - **Labels:** ≤ 10 labels, each ≤ 50 chars matching
//!   `^[A-Za-z0-9_./-]+$`. Rejecting anything else also rejects empty
//!   strings, spaces, and the GitHub label-emoji shortcodes (the
//!   user-facing label *display* may include emoji, but the canonical
//!   label slug is plain).

#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::github::auth::Token;
use crate::github::url::GithubRepo;

/// Cap for response bodies. The create-issue response is the only one
/// with a meaningful payload (full issue JSON) — typical size is
/// ~2-4 KiB, hard cap is 256 KiB for slack. Star / watch / unwatch
/// responses are empty so the cap is purely defensive.
const MAX_RESPONSE_BYTES: u64 = 256 * 1024;

/// Title length cap per the §12f review.
pub const ISSUE_TITLE_MAX_CHARS: usize = 256;

/// Body length cap per the §12f review.
pub const ISSUE_BODY_MAX_BYTES: usize = 64 * 1024;

/// Maximum number of labels per issue.
pub const ISSUE_LABELS_MAX_COUNT: usize = 10;

/// Maximum length of a single label slug.
pub const ISSUE_LABEL_MAX_CHARS: usize = 50;

/// HTTP per-request timeout. Aligns with `stats::HTTP_TIMEOUT` for
/// uniform UX — every github.com round-trip times out at 10s.
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

const API_BASE: &str = "https://api.github.com";

const USER_AGENT: &str = concat!(
    "agency-agents-app/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/msitarzewski/agency-agents-app)"
);

// ---------- DTOs ----------

/// Wire shape returned by `github_create_issue`. Only the fields the
/// frontend actually needs — the full issue JSON is intentionally
/// discarded (GitHub returns ~40 fields, most of which would mean
/// nothing in the Agency Agents UI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreatedIssue {
    /// GitHub-assigned issue number (e.g. 42 → `#42`).
    pub number: u32,
    /// Canonical `html_url` to open in the user's browser. Always
    /// `https://github.com/<owner>/<repo>/issues/<number>`. We pass
    /// it through verbatim so a future GitHub URL change doesn't break
    /// us, but the host check in `safeOpenUrl` still applies on the
    /// frontend side.
    pub html_url: String,
}

/// Raw shape we decode from the create-issue response. Subset of the
/// real payload — we ignore everything else.
#[derive(Debug, Deserialize)]
struct RawCreatedIssue {
    #[serde(default)]
    number: u32,
    #[serde(default)]
    html_url: String,
}

// ---------- Public actions ----------

/// Star the given repo. PUT `/user/starred/{owner}/{repo}` → 204 on
/// success. A 404 means the repo doesn't exist (or is private and the
/// token can't see it); surfaced as a regular HTTP error.
pub async fn star(
    client: &reqwest::Client,
    repo: &GithubRepo,
    token: &Token,
) -> Result<(), AppError> {
    revalidate_repo(repo)?;
    let url = format!("{}/user/starred/{}/{}", API_BASE, repo.owner, repo.repo);
    let resp = send(client.put(&url), token).await?;
    match resp.status().as_u16() {
        // GitHub also returns 204 on "already starred" — same idempotent
        // outcome the caller wanted.
        204 => Ok(()),
        403 => Err(maybe_rate_limited(&resp, &url)),
        s => Err(AppError::HttpStatus { url, status: s }),
    }
}

/// Unstar the given repo. DELETE same path. Idempotent — unstarring a
/// repo that wasn't starred is a 204 too.
pub async fn unstar(
    client: &reqwest::Client,
    repo: &GithubRepo,
    token: &Token,
) -> Result<(), AppError> {
    revalidate_repo(repo)?;
    let url = format!("{}/user/starred/{}/{}", API_BASE, repo.owner, repo.repo);
    let resp = send(client.delete(&url), token).await?;
    match resp.status().as_u16() {
        204 => Ok(()),
        403 => Err(maybe_rate_limited(&resp, &url)),
        s => Err(AppError::HttpStatus { url, status: s }),
    }
}

/// Check whether the signed-in user has starred this repo. GitHub
/// returns 204 for "yes" and 404 for "no" on this endpoint
/// (`/user/starred/{owner}/{repo}` with a GET).
pub async fn is_starred(
    client: &reqwest::Client,
    repo: &GithubRepo,
    token: &Token,
) -> Result<bool, AppError> {
    revalidate_repo(repo)?;
    let url = format!("{}/user/starred/{}/{}", API_BASE, repo.owner, repo.repo);
    let resp = send(client.get(&url), token).await?;
    match resp.status().as_u16() {
        204 => Ok(true),
        404 => Ok(false),
        403 => Err(maybe_rate_limited(&resp, &url)),
        s => Err(AppError::HttpStatus { url, status: s }),
    }
}

/// Watch the repo (subscribed = true, ignored = false). PUT
/// `/repos/{owner}/{repo}/subscription`. GitHub returns 200 with a
/// subscription JSON body (which we discard — the success status is
/// the contract).
pub async fn watch(
    client: &reqwest::Client,
    repo: &GithubRepo,
    token: &Token,
) -> Result<(), AppError> {
    revalidate_repo(repo)?;
    let url = format!("{}/repos/{}/{}/subscription", API_BASE, repo.owner, repo.repo);
    let body = serde_json::json!({ "subscribed": true, "ignored": false });
    let resp = send(client.put(&url).json(&body), token).await?;
    let status = resp.status().as_u16();
    if status == 403 {
        return Err(maybe_rate_limited(&resp, &url));
    }
    // Best-effort drain so the connection can be reused; cap any body
    // we read at MAX_RESPONSE_BYTES via content_length pre-check.
    if let Some(len) = resp.content_length() {
        if len > MAX_RESPONSE_BYTES {
            return Err(AppError::Network {
                url,
                message: format!("watch body length {len} exceeds {MAX_RESPONSE_BYTES}"),
            });
        }
    }
    let _ = resp.bytes().await;
    match status {
        200 => Ok(()),
        s => Err(AppError::HttpStatus { url, status: s }),
    }
}

/// Stop watching. DELETE same path. Idempotent — unwatching a repo
/// you weren't watching returns 204.
pub async fn unwatch(
    client: &reqwest::Client,
    repo: &GithubRepo,
    token: &Token,
) -> Result<(), AppError> {
    revalidate_repo(repo)?;
    let url = format!("{}/repos/{}/{}/subscription", API_BASE, repo.owner, repo.repo);
    let resp = send(client.delete(&url), token).await?;
    match resp.status().as_u16() {
        204 => Ok(()),
        403 => Err(maybe_rate_limited(&resp, &url)),
        s => Err(AppError::HttpStatus { url, status: s }),
    }
}

/// File an issue against the repo. Validates and sanitises the
/// payload before sending; returns the freshly-minted issue's number
/// and html_url.
///
/// `title`, `body`, and `labels` are all caller-influenced strings
/// (the frontend builds them from a textarea). We enforce the §12f
/// caps here as the last line of defense regardless of any frontend
/// validation.
pub async fn create_issue(
    client: &reqwest::Client,
    repo: &GithubRepo,
    token: &Token,
    title: &str,
    body: &str,
    labels: &[&str],
) -> Result<CreatedIssue, AppError> {
    revalidate_repo(repo)?;
    let sanitised_title = sanitise_title(title)?;
    let sanitised_body = sanitise_body(body)?;
    let sanitised_labels = sanitise_labels(labels)?;

    let url = format!("{}/repos/{}/{}/issues", API_BASE, repo.owner, repo.repo);
    let payload = serde_json::json!({
        "title": sanitised_title,
        "body": sanitised_body,
        "labels": sanitised_labels,
    });
    let resp = send(client.post(&url).json(&payload), token).await?;
    let status = resp.status().as_u16();
    if status == 403 {
        return Err(maybe_rate_limited(&resp, &url));
    }
    if status != 201 {
        return Err(AppError::HttpStatus { url, status });
    }

    // Body cap — refuse oversize before deserialising. The successful
    // shape is bounded (~2-4 KiB typical) so 256 KiB is generous.
    if let Some(len) = resp.content_length() {
        if len > MAX_RESPONSE_BYTES {
            return Err(AppError::Network {
                url,
                message: format!("create_issue body length {len} exceeds {MAX_RESPONSE_BYTES}"),
            });
        }
    }
    let bytes = resp.bytes().await.map_err(|e| AppError::Network {
        url: url.clone(),
        message: format!("body: {e}"),
    })?;
    if (bytes.len() as u64) > MAX_RESPONSE_BYTES {
        return Err(AppError::Network {
            url,
            message: format!(
                "create_issue body length {} exceeds {MAX_RESPONSE_BYTES}",
                bytes.len()
            ),
        });
    }

    let raw: RawCreatedIssue = serde_json::from_slice(&bytes).map_err(|e| AppError::JsonParse {
        command: url.clone(),
        message: e.to_string(),
        raw_excerpt: String::from_utf8_lossy(&bytes[..bytes.len().min(256)]).into_owned(),
    })?;
    if raw.number == 0 || raw.html_url.is_empty() {
        return Err(AppError::Internal {
            message: "github create_issue returned no number/html_url".into(),
        });
    }
    Ok(CreatedIssue {
        number: raw.number,
        html_url: raw.html_url,
    })
}

// ---------- Builders / helpers ----------

/// Construct a reqwest client tuned for github.com actions. Mirrors
/// `stats::build_client` so all GitHub traffic uses the same UA and
/// timeout.
pub fn build_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| AppError::Network {
            url: API_BASE.into(),
            message: format!("client build: {e}"),
        })
}

/// Apply the Authorization header + standard accept headers, then
/// send.
async fn send(
    builder: reqwest::RequestBuilder,
    token: &Token,
) -> Result<reqwest::Response, AppError> {
    builder
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        // `format!` here is a redaction risk only if the header value
        // ends up in a log line. reqwest consumes the value immediately
        // and never re-emits it; `Token::as_str()` is the chokepoint.
        .header("Authorization", format!("Bearer {}", token.as_str()))
        .send()
        .await
        .map_err(|e| AppError::Network {
            url: e
                .url()
                .map(|u| u.as_str().to_string())
                .unwrap_or_else(|| "<unknown github url>".into()),
            message: e.to_string(),
        })
}

/// Defense-in-depth re-validation of an already-validated `GithubRepo`.
/// Catches accidental hand-construction with bad characters.
fn revalidate_repo(repo: &GithubRepo) -> Result<(), AppError> {
    if !is_valid_owner_or_repo(&repo.owner) || !is_valid_owner_or_repo(&repo.repo) {
        return Err(AppError::InvalidArgument {
            message: format!(
                "github repo failed re-validation: {}/{}",
                repo.owner, repo.repo
            ),
        });
    }
    Ok(())
}

/// Same lexical rule as `url::is_valid_owner_or_repo` — duplicated here
/// to keep this module self-contained for the security gate. If the
/// rule ever changes both copies need to update together; the pinning
/// test asserts identical behaviour.
fn is_valid_owner_or_repo(name: &str) -> bool {
    if name.is_empty() || name.len() > 39 {
        return false;
    }
    if name == "." || name == ".." {
        return false;
    }
    let bytes = name.as_bytes();
    let first = bytes[0];
    if first == b'.' || first == b'-' {
        return false;
    }
    for &b in bytes {
        let ok = b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.';
        if !ok {
            return false;
        }
    }
    true
}

/// Inspect rate-limit headers on a 403 response and build the
/// appropriate typed error. Mirrors `stats::maybe_rate_limited`.
fn maybe_rate_limited(resp: &reqwest::Response, url: &str) -> AppError {
    let remaining = resp
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    if remaining == 0 {
        let reset_at = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        AppError::GithubRateLimited { reset_at }
    } else {
        AppError::HttpStatus {
            url: url.to_string(),
            status: 403,
        }
    }
}

// ---------- Issue input sanitisers ----------

/// Strip control chars (keep `\t`) and enforce the title cap.
///
/// Trims leading/trailing whitespace after stripping because a title
/// of `"   "` should be rejected the same as `""`.
fn sanitise_title(raw: &str) -> Result<String, AppError> {
    let cleaned: String = raw
        .chars()
        .filter(|c| {
            let n = *c as u32;
            // Keep tab; drop other C0 control chars (0x00..0x1F).
            *c == '\t' || n >= 0x20
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidArgument {
            message: "issue title must not be empty".into(),
        });
    }
    if trimmed.chars().count() > ISSUE_TITLE_MAX_CHARS {
        return Err(AppError::InvalidArgument {
            message: format!("issue title exceeds {ISSUE_TITLE_MAX_CHARS}-char cap"),
        });
    }
    Ok(trimmed.to_string())
}

/// Strip null bytes only and enforce the body cap. Markdown
/// passthrough; GitHub renders the body.
fn sanitise_body(raw: &str) -> Result<String, AppError> {
    let cleaned: String = raw.chars().filter(|c| *c != '\0').collect();
    if cleaned.len() > ISSUE_BODY_MAX_BYTES {
        return Err(AppError::InvalidArgument {
            message: format!(
                "issue body exceeds {ISSUE_BODY_MAX_BYTES}-byte cap"
            ),
        });
    }
    Ok(cleaned)
}

/// Validate the labels array and return owned strings the JSON
/// encoder can consume directly.
fn sanitise_labels(raw: &[&str]) -> Result<Vec<String>, AppError> {
    if raw.len() > ISSUE_LABELS_MAX_COUNT {
        return Err(AppError::InvalidArgument {
            message: format!(
                "too many labels ({} > {ISSUE_LABELS_MAX_COUNT})",
                raw.len()
            ),
        });
    }
    let mut out = Vec::with_capacity(raw.len());
    for label in raw {
        if label.is_empty() || label.len() > ISSUE_LABEL_MAX_CHARS {
            return Err(AppError::InvalidArgument {
                message: format!(
                    "label length must be 1..={ISSUE_LABEL_MAX_CHARS}; got {}",
                    label.len()
                ),
            });
        }
        for b in label.bytes() {
            let ok =
                b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'/';
            if !ok {
                return Err(AppError::InvalidArgument {
                    message: format!("label contains invalid character: {label:?}"),
                });
            }
        }
        out.push(label.to_string());
    }
    Ok(out)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_repo() -> GithubRepo {
        GithubRepo {
            owner: "octocat".into(),
            repo: "hello-world".into(),
        }
    }

    // ---------- revalidate_repo ----------

    #[test]
    fn revalidate_repo_accepts_valid_pair() {
        let r = fake_repo();
        assert!(revalidate_repo(&r).is_ok());
    }

    #[test]
    fn revalidate_repo_rejects_path_traversal_owner() {
        let r = GithubRepo {
            owner: "..".into(),
            repo: "bar".into(),
        };
        match revalidate_repo(&r) {
            Err(AppError::InvalidArgument { .. }) => {}
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn revalidate_repo_rejects_space_in_repo() {
        let r = GithubRepo {
            owner: "foo".into(),
            repo: "bar baz".into(),
        };
        assert!(revalidate_repo(&r).is_err());
    }

    #[test]
    fn revalidate_repo_rejects_oversize_owner() {
        let r = GithubRepo {
            owner: "a".repeat(40),
            repo: "bar".into(),
        };
        assert!(revalidate_repo(&r).is_err());
    }

    #[test]
    fn revalidate_repo_rejects_leading_dot() {
        let r = GithubRepo {
            owner: ".foo".into(),
            repo: "bar".into(),
        };
        assert!(revalidate_repo(&r).is_err());
    }

    /// The local copy of `is_valid_owner_or_repo` must agree with
    /// `url::parse_github_url`'s rules on representative samples.
    /// Pinning this lock-steps the two definitions.
    #[test]
    fn revalidator_matches_url_module_rules() {
        let url_mod_accepts: &[&str] = &[
            "foo",
            "foo-bar",
            "foo.bar",
            "foo_bar",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLM", // 39 chars
        ];
        for name in url_mod_accepts {
            assert!(is_valid_owner_or_repo(name), "should accept {name}");
        }
        let oversize = "a".repeat(40);
        let url_mod_rejects: &[&str] = &[
            "", ".", "..", ".foo", "-foo", "foo bar", "foo!bar", "föö",
            oversize.as_str(),
        ];
        for name in url_mod_rejects {
            assert!(!is_valid_owner_or_repo(name), "should reject {name:?}");
        }
    }

    // ---------- Issue title sanitiser ----------

    #[test]
    fn sanitise_title_strips_control_chars_keeps_tab() {
        let raw = "hello\x07\x01world\twith tab";
        let cleaned = sanitise_title(raw).expect("title");
        assert_eq!(cleaned, "helloworld\twith tab");
    }

    #[test]
    fn sanitise_title_rejects_empty() {
        assert!(sanitise_title("").is_err());
        assert!(sanitise_title("   ").is_err());
        // Title made entirely of control chars also collapses to empty.
        assert!(sanitise_title("\x01\x02\x03").is_err());
    }

    #[test]
    fn sanitise_title_rejects_over_256_chars() {
        let raw = "a".repeat(ISSUE_TITLE_MAX_CHARS + 1);
        match sanitise_title(&raw) {
            Err(AppError::InvalidArgument { message }) => {
                assert!(message.contains("256"), "{message}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn sanitise_title_accepts_exactly_256_chars() {
        let raw = "a".repeat(ISSUE_TITLE_MAX_CHARS);
        let cleaned = sanitise_title(&raw).expect("title");
        assert_eq!(cleaned.chars().count(), ISSUE_TITLE_MAX_CHARS);
    }

    // ---------- Issue body sanitiser ----------

    #[test]
    fn sanitise_body_strips_null_bytes_only() {
        let raw = "hello\x00world\nwith *markdown* and `code`\n";
        let cleaned = sanitise_body(raw).expect("body");
        assert_eq!(cleaned, "helloworld\nwith *markdown* and `code`\n");
    }

    #[test]
    fn sanitise_body_rejects_oversize() {
        // 64 KiB + 1.
        let raw = "a".repeat(ISSUE_BODY_MAX_BYTES + 1);
        match sanitise_body(&raw) {
            Err(AppError::InvalidArgument { message }) => {
                // The byte count (65536) appears in the cap message.
                assert!(
                    message.contains(&ISSUE_BODY_MAX_BYTES.to_string()),
                    "expected cap byte count in message, got {message}"
                );
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn sanitise_body_accepts_exactly_64kib() {
        let raw = "a".repeat(ISSUE_BODY_MAX_BYTES);
        let cleaned = sanitise_body(&raw).expect("body");
        assert_eq!(cleaned.len(), ISSUE_BODY_MAX_BYTES);
    }

    // ---------- Labels sanitiser ----------

    #[test]
    fn sanitise_labels_accepts_typical_set() {
        let labels = vec!["bug", "category-suggestion", "good_first_issue", "v0.1"];
        let cleaned = sanitise_labels(&labels).expect("labels");
        assert_eq!(cleaned, labels);
    }

    #[test]
    fn sanitise_labels_rejects_more_than_10() {
        let labels: Vec<&str> = (0..(ISSUE_LABELS_MAX_COUNT + 1))
            .map(|_| "bug")
            .collect();
        match sanitise_labels(&labels) {
            Err(AppError::InvalidArgument { message }) => {
                assert!(message.contains("too many"), "{message}");
            }
            other => panic!("expected InvalidArgument, got {other:?}"),
        }
    }

    #[test]
    fn sanitise_labels_rejects_empty_label() {
        let labels = vec!["bug", "", "category"];
        assert!(sanitise_labels(&labels).is_err());
    }

    #[test]
    fn sanitise_labels_rejects_oversize_label() {
        let huge = "a".repeat(ISSUE_LABEL_MAX_CHARS + 1);
        let labels = vec!["bug", huge.as_str()];
        assert!(sanitise_labels(&labels).is_err());
    }

    #[test]
    fn sanitise_labels_rejects_invalid_chars() {
        for bad in &[
            "bug!",
            "needs review",
            "ünicode",
            "lab\tel",
            "<script>",
            "foo;rm",
        ] {
            assert!(
                sanitise_labels(&[bad]).is_err(),
                "should reject label {bad:?}"
            );
        }
    }

    // ---------- DTO shape ----------

    #[test]
    fn created_issue_serializes_camel_case() {
        let c = CreatedIssue {
            number: 42,
            html_url: "https://github.com/foo/bar/issues/42".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["number"], 42);
        assert_eq!(v["htmlUrl"], "https://github.com/foo/bar/issues/42");
        // No snake_case slip-through.
        assert!(v.get("html_url").is_none());
    }

    // ---------- Rate-limit error shape ----------

    /// Reuse the same typed-error shape `stats::maybe_rate_limited`
    /// produces. We can't easily mint a real `reqwest::Response` in a
    /// unit test, so the test focuses on the variant the helper would
    /// emit on a 403 with `X-RateLimit-Remaining: 0`.
    #[test]
    fn rate_limit_error_serializes_with_camel_case_reset_at() {
        let err = AppError::GithubRateLimited {
            reset_at: 1_700_000_000,
        };
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["code"], "github_rate_limited");
        assert_eq!(v["resetAt"], 1_700_000_000u64);
    }

    // ---------- Constants stability ----------

    #[test]
    fn issue_caps_are_pinned() {
        // Loosening any of these requires updating
        // memory-bank/scans/phase12-security-review.md §12f.
        assert_eq!(ISSUE_TITLE_MAX_CHARS, 256);
        assert_eq!(ISSUE_BODY_MAX_BYTES, 64 * 1024);
        assert_eq!(ISSUE_LABELS_MAX_COUNT, 10);
        assert_eq!(ISSUE_LABEL_MAX_CHARS, 50);
        assert_eq!(MAX_RESPONSE_BYTES, 256 * 1024);
    }
}
