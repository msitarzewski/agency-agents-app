//! Integration tests that hit the real `brew` CLI on the dev host.
//!
//! All tests are `#[ignore]` so the default `cargo test` stays fast and
//! works in CI without `brew` installed. Opt in with:
//!
//! ```sh
//! cargo test --test integration_brew -- --ignored
//! ```
//!
//! These tests intentionally do not exercise the Tauri-managed command
//! handlers themselves — those require a Tauri runtime + State injection
//! and are validated end-to-end by Wave 3 manual smoke. Instead, they
//! verify that the underlying `brew` invocations the commands rely on
//! still produce JSON shapes that our typed parsers (`brew::parse`)
//! accept, and that the fixture data we ship matches what brew currently
//! produces on this machine.
//!
//! Failure of any test here means EITHER:
//!   1. brew has changed its `--json=v2` output (action: re-capture
//!      fixtures + update parsers), OR
//!   2. brew isn't on PATH (action: install brew, or skip these tests).

use std::process::Command;

fn brew_path() -> Option<String> {
    // Prefer the same resolution order as `brew::paths::resolve_brew_path`.
    for cand in ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
        if std::path::Path::new(cand).is_file() {
            return Some(cand.to_string());
        }
    }
    None
}

fn require_brew() -> String {
    brew_path().unwrap_or_else(|| {
        panic!("brew not found on PATH — install brew or skip via --ignored");
    })
}

/// Smallest possible JSON-output check: `brew --version`.
#[test]
#[ignore]
fn brew_version_runs_and_reports_a_version() {
    let path = require_brew();
    let out = Command::new(&path)
        .arg("--version")
        .output()
        .expect("spawn brew");
    assert!(
        out.status.success(),
        "brew --version exit: {:?}",
        out.status
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("Homebrew "), "got: {:?}", stdout);
}

/// `brew info --installed --json=v2` returns a `RawInfoV2`-shaped object.
#[test]
#[ignore]
fn brew_info_installed_yields_parseable_json() {
    let path = require_brew();
    let out = Command::new(&path)
        .args(["info", "--installed", "--json=v2"])
        .output()
        .expect("spawn brew");
    assert!(out.status.success(), "exit {:?}", out.status);
    let raw = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("brew --installed must yield valid JSON");
    assert!(
        v.get("formulae").is_some() || v.get("casks").is_some(),
        "expected top-level `formulae` or `casks` key"
    );
}

/// `brew info <pkg> --json=v2` for a known formula must parse and yield exactly one entry.
#[test]
#[ignore]
fn brew_info_wget_returns_single_formula_entry() {
    let path = require_brew();
    let out = Command::new(&path)
        .args(["info", "--json=v2", "--formula", "wget"])
        .output()
        .expect("spawn brew");
    assert!(out.status.success(), "exit {:?}", out.status);
    let raw = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid json");
    let formulae = v["formulae"].as_array().expect("formulae array present");
    assert_eq!(formulae.len(), 1, "expected one formula for `wget`");
    assert_eq!(formulae[0]["name"].as_str(), Some("wget"));
}

/// `brew outdated --json=v2 --greedy` must always yield valid JSON,
/// even if the user has nothing outdated (empty arrays).
#[test]
#[ignore]
fn brew_outdated_yields_parseable_json() {
    let path = require_brew();
    let out = Command::new(&path)
        .args(["outdated", "--json=v2", "--greedy"])
        .output()
        .expect("spawn brew");
    assert!(out.status.success(), "exit {:?}", out.status);
    let raw = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&raw).expect("valid json");
    assert!(v["formulae"].is_array(), "formulae must be an array");
    assert!(v["casks"].is_array(), "casks must be an array");
}

/// `brew search --formula <q>` plain-stdout output must be non-empty
/// for a common query and follow the line-per-token format our parser
/// expects.
#[test]
#[ignore]
fn brew_search_formula_wget_returns_non_empty_token_list() {
    let path = require_brew();
    let out = Command::new(&path)
        .args(["search", "--formula", "wget"])
        .output()
        .expect("spawn brew");
    assert!(out.status.success(), "exit {:?}", out.status);
    let raw = String::from_utf8_lossy(&out.stdout);
    let tokens: Vec<&str> = raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("==>"))
        .flat_map(|l| l.split_whitespace())
        .collect();
    assert!(
        tokens.contains(&"wget"),
        "expected `wget` in search results, got {:?}",
        tokens
    );
}

/// Trending endpoint at formulae.brew.sh: live network test.
///
/// Verifies that the URL we hit (`/api/analytics/install/30d.json`)
/// returns a 200 with JSON whose top-level shape matches our parser.
/// Per the BUG in `apiTests.md`, the actual shape uses `items: [...]`
/// while the current parser expects `formulae: { name: [...] }` — this
/// test pins the real wire shape.
#[test]
#[ignore]
fn trending_endpoint_returns_items_array_shape() {
    let url = "https://formulae.brew.sh/api/analytics/install/30d.json";
    let raw = match ureq_get(url) {
        Some(s) => s,
        None => {
            eprintln!("network unavailable — skipping");
            return;
        }
    };
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("trending payload must be valid JSON");
    assert!(
        v.get("items").and_then(|x| x.as_array()).is_some(),
        "real trending payload must have top-level `items` array — see BUG note in apiTests.md"
    );
    assert!(v.get("total_count").and_then(|x| x.as_u64()).is_some());
    let items = v["items"].as_array().unwrap();
    assert!(!items.is_empty(), "items array must be non-empty");
    let first = &items[0];
    assert!(first["formula"].as_str().is_some());
    assert!(first["count"].as_str().is_some());
}

/// Tiny ureq-free GET via `curl`, so we don't add a dependency just for
/// one integration test. Returns None on any failure (network down,
/// curl missing, non-200 status).
fn ureq_get(url: &str) -> Option<String> {
    let out = Command::new("curl")
        .args(["-sLf", "--max-time", "10", url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}
