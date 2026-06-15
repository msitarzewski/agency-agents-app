//! Corpus subsystem (Phase 1) — the maintained copy of the agency-agents
//! repo that the whole app reads from.
//!
//! ## Source of truth (systemPatterns.md §1)
//!
//! ```text
//! <app_data_dir>/
//! ├── corpus/                 our maintained copy of the agency-agents repo
//! │   └── <category>/<slug>.md
//! └── state/
//!     └── corpus-index.json   slug → CorpusEntry (hashes, category, version)
//! ```
//!
//! - **Seed**: a baseline corpus ships inside the app bundle
//!   (`resources/corpus-baseline/<category>/<slug>.md`). On first run it is
//!   copied to `<app_data_dir>/corpus/` so the app works offline.
//! - **Refresh** ([`corpus_refresh`]): fetch the GitHub tarball
//!   `https://codeload.github.com/msitarzewski/agency-agents/tar.gz/refs/heads/main`,
//!   extract the category dirs over the working copy, and rebuild
//!   `corpus-index.json`. No runtime git dependency.
//!
//! ## Determinism (contracts.md §E)
//!
//! `corpus-index.json` is keyed by a `BTreeMap` so its serialization has a
//! stable key order. The three per-agent hashes are SHA-256 of canonical
//! byte regions of the source `.md` (see [`parse`]). Nothing in the index
//! carries a timestamp; the only timestamp is [`CorpusMeta::fetched_at`],
//! which lives in a separate meta file, not the index.

mod parse;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::github::extract_github_repo;
use crate::types::{
    Agent, Category, CatalogCandidate, CatalogDetection, CatalogSource, CatalogStatus,
    CatalogUpdateCheck, CorpusEntry, CorpusMeta,
};
use crate::util::fs::atomic_write;

// ---------- Constants ----------

/// Canonical fallback category set, used only when the repo tooling can't be
/// read (e.g. a corpus dir with no `scripts/`). This MIRRORS the `AGENT_DIRS`
/// array the agency-agents repo declares in `scripts/convert.sh` (kept in sync
/// upstream with `install.sh` / `lint-agents.sh`) — the repo is the source of
/// truth, this is just the offline default. See [`discover_categories`].
///
/// NB: `integrations/` is NOT a category — it is `convert.sh`'s *output*
/// directory (per-tool converted copies), so it's intentionally absent.
/// `strategy/` IS a declared category (NEXUS playbooks/runbooks); it currently
/// holds no `name:`-frontmatter personas, so it indexes as an empty category.
const DEFAULT_CATEGORIES: [&str; 16] = [
    "academic",
    "design",
    "engineering",
    "finance",
    "game-development",
    "marketing",
    "paid-media",
    "product",
    "project-management",
    "sales",
    "security",
    "spatial-computing",
    "specialized",
    "strategy",
    "support",
    "testing",
];

/// Discover the agent category directories from the repo's own tooling rather
/// than hardcoding them — so when upstream adds/renames a division, a corpus
/// refresh (which re-fetches `scripts/`) picks it up with no app change.
///
/// We parse the `AGENT_DIRS=( … )` bash array from, in order of preference,
/// `scripts/convert.sh`, `scripts/install.sh`, or `scripts/lint-agents.sh`
/// under `root`. Tokens may be spread across lines and/or one-per-line; inline
/// `#` comments are stripped. Falls back to [`DEFAULT_CATEGORIES`] when no
/// script is present or the array can't be parsed (e.g. the bundled baseline
/// before `scripts/` is seeded).
fn discover_categories(root: &Path) -> Vec<String> {
    for script in ["convert.sh", "install.sh", "lint-agents.sh"] {
        let path = root.join("scripts").join(script);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(cats) = parse_agent_dirs(&text) {
            if !cats.is_empty() {
                return cats;
            }
        }
    }
    DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect()
}

/// Extract the `AGENT_DIRS=( … )` bash array body from a shell script's text.
/// Returns the ordered, de-duplicated directory names, or `None` if the array
/// isn't found. Pure string work so it's unit-testable without the filesystem.
fn parse_agent_dirs(script: &str) -> Option<Vec<String>> {
    let start = script.find("AGENT_DIRS=(")?;
    let after = &script[start + "AGENT_DIRS=(".len()..];
    let end = after.find(')')?;
    let body = &after[..end];

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw_line in body.lines() {
        // Strip an inline comment, then split on whitespace.
        let line = raw_line.split('#').next().unwrap_or("");
        for tok in line.split_whitespace() {
            // Defensive: ignore anything that isn't a plausible dir slug.
            if tok.is_empty() || !tok.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                continue;
            }
            if seen.insert(tok.to_string()) {
                out.push(tok.to_string());
            }
        }
    }
    Some(out)
}

/// GitHub `codeload` tarball for the live corpus. Streamed, gunzipped,
/// and unpacked on [`corpus_refresh`]. No git binary required.
const CORPUS_TARBALL_URL: &str =
    "https://codeload.github.com/msitarzewski/agency-agents/tar.gz/refs/heads/main";

/// Git remote used to clone/pull a managed catalog when `git` is available.
const CATALOG_GIT_URL: &str = "https://github.com/msitarzewski/agency-agents.git";

/// Dev-root directory names scanned (under `$HOME`) by the "Find Agency Agents"
/// button when looking for an existing clone.
const SCAN_ROOTS: [&str; 7] = ["Software", "Projects", "git", "Developer", "code", "dev", "src"];

/// User-Agent for the refresh fetch. Mirrors the catalog refresh style.
const USER_AGENT: &str = "agency-agents/0.1 (+https://github.com/msitarzewski/agency-agents)";

/// Whole-request timeout for the tarball fetch. The repo is small (a few
/// hundred small markdown files) so 60s is generous.
const REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Cap on the raw `tar.gz` response (defends against a hostile mirror).
/// The real tarball is well under 5 MiB; 32 MiB is large headroom.
const MAX_TARBALL_BYTES: u64 = 32 * 1024 * 1024;

/// Cap on a single decompressed agent `.md`. Personas run a few KiB;
/// 1 MiB is absurdly generous and still bounds memory.
const MAX_AGENT_BYTES: u64 = 1024 * 1024;

/// Version string recorded for the bundled baseline before any refresh
/// has resolved a commit SHA.
const BASELINE_VERSION: &str = "baseline";

// ---------- On-disk meta ----------

/// `corpus-meta.json` — top-level metadata for the working copy. Distinct
/// from the index (which is per-agent) so [`corpus_status`] can answer
/// "what version / how many / fetched when" with one small read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMeta {
    version: String,
    commit: Option<String>,
    fetched_at: String,
    count: u32,
}

impl From<StoredMeta> for CorpusMeta {
    fn from(m: StoredMeta) -> Self {
        CorpusMeta {
            version: m.version,
            commit: m.commit,
            fetched_at: m.fetched_at,
            count: m.count,
        }
    }
}

// ---------- In-memory corpus ----------

/// The parsed, in-memory corpus: every agent plus its index row, ordered
/// deterministically by `(category, slug)`. Memoized on `AppState` so the
/// hot read commands (`corpus_list` / `corpus_get` / `corpus_categories`)
/// never touch disk after the first build.
#[derive(Debug, Clone)]
pub struct Corpus {
    /// Agents in stable `(category, slug)` order. `Agent.body` is fully
    /// populated here; list views clone-and-clear it (see
    /// [`Corpus::list`]).
    agents: Vec<Agent>,
    /// Index rows keyed by slug — `BTreeMap` so the serialized
    /// `corpus-index.json` has stable key order.
    index: BTreeMap<String, CorpusEntry>,
    /// The category directories this corpus was built from, in tooling order
    /// (from [`discover_categories`]). Drives the Discover grid so the tiles
    /// match the active catalog's actual divisions.
    category_order: Vec<String>,
    /// Division presentation metadata (label / icon / color) keyed by slug,
    /// resolved at build time: the catalog root's `divisions.json` overlaid on
    /// the bundled `agency-categories.json` floor (see [`load_division_meta`]).
    /// Carrying it on the corpus means `categories()` never touches disk and a
    /// catalog that ships a new division presents correctly without an app
    /// update.
    division_meta: BTreeMap<String, CategoryMetaRow>,
    meta: CorpusMeta,
}

impl Corpus {
    /// Number of indexed agents.
    pub fn count(&self) -> u32 {
        self.index.len() as u32
    }

    /// [`CorpusMeta`] for `corpus_status`.
    pub fn meta(&self) -> CorpusMeta {
        self.meta.clone()
    }

    /// List view — agents (optionally filtered to one `category`) with the
    /// `body` omitted to keep the IPC payload small (contracts.md §C).
    pub fn list(&self, category: Option<&str>) -> Vec<Agent> {
        self.agents
            .iter()
            .filter(|a| category.is_none_or(|c| a.category == c))
            .map(|a| Agent {
                body: String::new(),
                ..a.clone()
            })
            .collect()
    }

    /// Full agent (incl. body) by slug, or `None` if unknown.
    pub fn get(&self, slug: &str) -> Option<Agent> {
        self.agents.iter().find(|a| a.slug == slug).cloned()
    }

    /// Resolve a filename emitted by `convert.sh` back to the catalog's
    /// filename-based identity. Most upstream filenames include a division
    /// prefix while transformed installs use `slugify(frontmatter.name)`.
    pub fn get_by_conversion_slug(&self, slug: &str) -> Option<Agent> {
        self.agents
            .iter()
            .find(|a| crate::render::slugify(&a.name) == slug)
            .cloned()
    }

    /// Index row (hashes + category) by slug, for the install/reconcile layer.
    pub fn entry(&self, slug: &str) -> Option<CorpusEntry> {
        self.index.get(slug).cloned()
    }

    /// The active corpus version (from meta), used to stamp ledger records.
    pub fn version(&self) -> String {
        self.meta.version.clone()
    }

    /// Per-category counts in tooling order (from [`discover_categories`]).
    /// Label + icon + color come from [`Corpus::division_meta`] — the catalog's
    /// `divisions.json` overlaid on the bundled floor. Categories with zero
    /// agents are still returned so the Discover grid renders the full division
    /// set.
    pub fn categories(&self) -> Vec<Category> {
        let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
        for entry in self.index.values() {
            *counts.entry(entry.category.as_str()).or_default() += 1;
        }
        self.category_order
            .iter()
            .map(|slug| {
                let (label, icon, color) = category_meta_from(&self.division_meta, slug);
                Category {
                    slug: slug.clone(),
                    label,
                    icon,
                    color,
                    count: counts.get(slug.as_str()).copied().unwrap_or(0),
                }
            })
            .collect()
    }

    /// Serialize the index to canonical pretty JSON. Stable key order
    /// (BTreeMap) → byte-identical output for an unchanged corpus.
    fn index_json(&self) -> Result<Vec<u8>, AppError> {
        serde_json::to_vec_pretty(&self.index).map_err(|e| AppError::Internal {
            message: format!("serialize corpus-index.json: {e}"),
        })
    }
}

// ---------- Category metadata ----------

/// The bundled `categories.json` shape we read label + icon from. Only
/// the `categories` map is needed here.
#[derive(Debug, Deserialize)]
struct CategoriesFile {
    categories: BTreeMap<String, CategoryMetaRow>,
}

#[derive(Debug, Clone, Deserialize)]
struct CategoryMetaRow {
    label: String,
    icon: String,
    #[serde(default = "default_division_color")]
    color: String,
}

/// The catalog's `divisions.json` shape (PR #592): the canonical, first-class
/// source for division presentation metadata, shared with the CLI installer +
/// linters. Same row shape as the bundled file, under a `divisions` key.
#[derive(Debug, Deserialize)]
struct DivisionsFile {
    divisions: BTreeMap<String, CategoryMetaRow>,
}

/// Neutral fallback color for a division without one in the metadata.
fn default_division_color() -> String {
    "#94A3B8".to_string()
}

const CATEGORIES_JSON: &str = include_str!("../../data/agency-categories.json");
const DIVISIONS_FILENAME: &str = "divisions.json";

/// The bundled `agency-categories.json` parsed into a slug → row map. This is
/// the floor the app always ships — used directly on first run / for an old
/// clone, and as the base that `divisions.json` overlays onto.
fn bundled_division_meta() -> BTreeMap<String, CategoryMetaRow> {
    serde_json::from_str::<CategoriesFile>(CATEGORIES_JSON)
        .map(|f| f.categories)
        .unwrap_or_default()
}

/// Resolve division metadata for the active catalog: start from the bundled
/// floor, then overlay the catalog root's `divisions.json` (PR #592 — the
/// canonical source shared with the CLI installer + linters) when present and
/// parseable. First-run (Bundled) users and pre-#592 clones simply have no
/// `divisions.json`, so they keep the bundled metadata — no drift, no failure.
/// Overlaying (rather than replacing) means a `divisions.json` that omits a
/// division still falls back to the bundled row for it.
fn load_division_meta(catalog_root: &Path) -> BTreeMap<String, CategoryMetaRow> {
    let mut meta = bundled_division_meta();
    let path = catalog_root.join(DIVISIONS_FILENAME);
    match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<DivisionsFile>(&raw) {
            Ok(file) => {
                for (slug, row) in file.divisions {
                    meta.insert(slug, row);
                }
                tracing::debug!("corpus: division metadata sourced from {}", path.display());
            }
            Err(e) => tracing::warn!(
                "corpus: {} present but unparseable ({e}); using bundled division metadata",
                path.display()
            ),
        },
        // Absent is the common, expected case (first run / old clone) — not a warning.
        Err(_) => tracing::debug!(
            "corpus: no {DIVISIONS_FILENAME} at catalog root; using bundled division metadata"
        ),
    }
    meta
}

/// Resolve `(label, icon, color)` for a category slug from a resolved division
/// metadata map. Falls back to a title-cased slug + a neutral `Folder` icon +
/// a neutral color if the slug is somehow absent (keeps Discover rendering
/// rather than dropping a tile).
fn category_meta_from(
    meta: &BTreeMap<String, CategoryMetaRow>,
    slug: &str,
) -> (String, String, String) {
    match meta.get(slug) {
        Some(row) => (row.label.clone(), row.icon.clone(), row.color.clone()),
        None => (title_case(slug), "Folder".to_string(), default_division_color()),
    }
}

/// `"game-development"` → `"Game Development"`. Deterministic fallback for
/// the unlikely missing-slug case.
fn title_case(slug: &str) -> String {
    slug.split('-')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- Path helpers ----------

/// The working corpus directory: `<app_data_dir>/corpus`. ALWAYS derived
/// from `app_data_dir` — never composed from IPC input.
pub(crate) fn corpus_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("corpus")
}

/// The state directory holding `corpus-index.json` + `corpus-meta.json` and
/// (Phase 2) the install ledger `installs.json`.
pub(crate) fn state_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("state")
}

fn index_path(app_data_dir: &Path) -> PathBuf {
    state_dir(app_data_dir).join("corpus-index.json")
}

fn meta_path(app_data_dir: &Path) -> PathBuf {
    state_dir(app_data_dir).join("corpus-meta.json")
}

fn catalog_source_path(app_data_dir: &Path) -> PathBuf {
    state_dir(app_data_dir).join("catalog.json")
}

// ---------- Catalog source (where the corpus content lives) ----------

/// Load the persisted [`CatalogSource`], or [`CatalogSource::Bundled`] when no
/// choice has been made yet / the file is unreadable. The catalog SOURCE
/// (content location) is distinct from the STATE dir (index/meta/ledger/backups
/// always live under app data, regardless of source).
pub(crate) async fn load_catalog_source(app_data_dir: &Path) -> CatalogSource {
    let path = catalog_source_path(app_data_dir);
    match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => CatalogSource::default(),
    }
}

/// Persist the chosen [`CatalogSource`] to `state/catalog.json`.
pub(crate) async fn save_catalog_source(
    app_data_dir: &Path,
    source: &CatalogSource,
) -> Result<(), AppError> {
    let sdir = state_dir(app_data_dir);
    tokio::fs::create_dir_all(&sdir).await.map_err(|e| AppError::Io {
        message: format!("create state dir {}: {e}", sdir.display()),
    })?;
    let bytes = serde_json::to_vec_pretty(source).map_err(|e| AppError::Internal {
        message: format!("serialize catalog.json: {e}"),
    })?;
    atomic_write(&catalog_source_path(app_data_dir), &bytes).await
}

/// Resolve the active catalog ROOT directory (where `<category>/<slug>.md` and
/// `scripts/convert.sh` live) for a source. `Bundled` lives inside app data;
/// `Managed`/`UserClone` point at a clone elsewhere on disk.
pub(crate) fn catalog_root(app_data_dir: &Path, source: &CatalogSource) -> PathBuf {
    match source {
        CatalogSource::Bundled => corpus_dir(app_data_dir),
        CatalogSource::Managed { path } => PathBuf::from(path),
        CatalogSource::UserClone { path, .. } => PathBuf::from(path),
    }
}

// ---------- Build / load ----------

/// Resolve the active corpus for the current process:
///
/// 1. Seed the working copy from the bundled baseline if `corpus/` is
///    empty (first run).
/// 2. Parse + index everything under `corpus/`.
/// 3. Write `corpus-index.json` + `corpus-meta.json` if they are missing
///    or stale (so reconciliation has the index on disk too).
///
/// `baseline_dir` is the bundled baseline resolved from the Tauri
/// resource dir (`resource_dir()/resources/corpus-baseline`). `Never`
/// panics: a fully empty or unreadable corpus yields an empty [`Corpus`]
/// with `count == 0` so the UI degrades to "no agents" rather than
/// failing to launch.
pub async fn resolve_active(app_data_dir: &Path, baseline_dir: &Path) -> Corpus {
    let source = load_catalog_source(app_data_dir).await;
    let dir = catalog_root(app_data_dir, &source);

    // Only the Bundled source seeds from the baseline (into app data). Managed /
    // UserClone roots are populated by provisioning (detect/clone/pull) — if one
    // is empty here it just hasn't been provisioned yet, so we serve what's
    // there (possibly empty) rather than stamping the baseline over a clone.
    if matches!(source, CatalogSource::Bundled) && is_empty_dir(&dir) {
        let seed_cats = discover_categories(baseline_dir);
        if let Err(e) = seed_from_baseline(baseline_dir, &dir, &seed_cats).await {
            tracing::warn!("corpus: seed from baseline failed: {e}");
        }
    }

    // Categories for indexing come from the ACTIVE root's tooling — after the
    // seed (or in a clone) `scripts/convert.sh` lives alongside the agents, so
    // the division set always reflects the catalog actually present.
    let categories = discover_categories(&dir);

    // Determine the version to stamp the index with: keep whatever a prior
    // refresh recorded, else the baseline marker.
    let version = match load_stored_meta(app_data_dir).await {
        Some(m) => m.version,
        None => BASELINE_VERSION.to_string(),
    };

    let mut corpus = match build_from_dir(&dir, &version, &categories).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("corpus: index build failed ({e}); serving empty corpus");
            empty_corpus(&version, &categories)
        }
    };

    // Prefer the catalog's own divisions.json (PR #592) for division label /
    // icon / color, falling back to the bundled metadata for first-run users
    // and pre-#592 clones that don't carry it yet.
    corpus.division_meta = load_division_meta(&dir);

    // Persist index + meta (best effort — read commands work from the
    // in-memory copy regardless; the on-disk index exists for the
    // reconciliation subsystem built in a later phase).
    if let Err(e) = persist(app_data_dir, &corpus).await {
        tracing::warn!("corpus: persist index/meta failed: {e}");
    }

    corpus
}

/// Recursively collect every `*.md` under `root`, sorted by full path for
/// determinism. Real catalog clones nest agents in subdirectories (e.g.
/// `game-development/godot/<slug>.md`, `game-development/unity/<slug>.md`), so a
/// flat top-level scan would silently miss them.
fn collect_md_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for ent in rd.flatten() {
            let path = ent.path();
            match ent.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(path),
                Ok(_) if path.extension().and_then(|e| e.to_str()) == Some("md") => out.push(path),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Find `<file_name>` anywhere under `dir` (depth-first). Used by `read_source`
/// to resolve a nested agent's canonical file when the flat path doesn't exist.
fn find_md_under(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
                return Some(path);
            }
        }
    }
    None
}

/// Build an in-memory [`Corpus`] by walking `<dir>/<category>/**/<slug>.md`
/// for every known category (recursively — real clones nest agents in
/// subdirs). Files without valid frontmatter (READMEs, workflow docs) are
/// skipped. The category is the top-level dir; the resulting `agents` vec and
/// `index` map are ordered deterministically by `(category, path)`.
async fn build_from_dir(dir: &Path, version: &str, categories: &[String]) -> Result<Corpus, AppError> {
    let mut rows: Vec<(Agent, CorpusEntry)> = Vec::new();

    for category in categories.iter() {
        let category = category.as_str();
        let cat_dir = dir.join(category);
        if !cat_dir.is_dir() {
            continue; // category dir absent — fine, skip.
        }
        // Recursive, sorted-by-path collection (catches nested agents).
        let files = collect_md_files(&cat_dir);

        for path in files {
            let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let raw = match read_capped(&path, MAX_AGENT_BYTES).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::warn!("corpus: skip {} ({e})", path.display());
                    continue;
                }
            };
            let source = match String::from_utf8(raw) {
                Ok(s) => s,
                Err(_) => {
                    tracing::warn!("corpus: skip {} (non-utf8)", path.display());
                    continue;
                }
            };
            match parse::parse_agent(slug, category, &source) {
                Ok(Some(pair)) => rows.push(pair),
                Ok(None) => {} // not an agent (no frontmatter) — skip silently.
                Err(e) => tracing::warn!("corpus: {e}"),
            }
        }
    }

    // `rows` is already in `(category, path)` order because we iterate
    // `categories` in tooling order and `collect_md_files` sorts by path.
    let mut agents = Vec::with_capacity(rows.len());
    let mut index = BTreeMap::new();
    for (agent, entry) in rows {
        index.insert(entry.slug.clone(), entry);
        agents.push(agent);
    }

    let count = index.len() as u32;
    Ok(Corpus {
        agents,
        index,
        category_order: categories.to_vec(),
        // Bundled floor; resolve_active overlays the catalog's divisions.json.
        division_meta: bundled_division_meta(),
        meta: CorpusMeta {
            version: version.to_string(),
            commit: None,
            // The build itself carries no timestamp; fetched_at reflects
            // when the *content* was last fetched. For a baseline build
            // that is the seed time, captured at persist below if no meta
            // exists yet.
            fetched_at: String::new(),
            count,
        },
    })
}

fn empty_corpus(version: &str, categories: &[String]) -> Corpus {
    Corpus {
        agents: Vec::new(),
        index: BTreeMap::new(),
        category_order: categories.to_vec(),
        division_meta: bundled_division_meta(),
        meta: CorpusMeta {
            version: version.to_string(),
            commit: None,
            fetched_at: String::new(),
            count: 0,
        },
    }
}

// ---------- Seeding ----------

/// True if `dir` does not exist or contains no entries.
fn is_empty_dir(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => true,
    }
}

/// Copy `<baseline>/<category>/*.md` into `<dest>/<category>/` for each
/// `category`, plus the repo tooling (`scripts/convert.sh`) so the seeded
/// working copy can discover its own divisions. Anything else in the baseline
/// is ignored. Idempotent: re-seeding overwrites file-for-file.
async fn seed_from_baseline(baseline: &Path, dest: &Path, categories: &[String]) -> Result<(), AppError> {
    if !baseline.exists() {
        return Err(AppError::Io {
            message: format!("baseline corpus not found at {}", baseline.display()),
        });
    }
    let mut seeded = 0u32;
    for category in categories.iter() {
        let src_cat = baseline.join(category);
        let mut read = match tokio::fs::read_dir(&src_cat).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let dst_cat = dest.join(category);
        tokio::fs::create_dir_all(&dst_cat)
            .await
            .map_err(|e| AppError::Io {
                message: format!("create {}: {e}", dst_cat.display()),
            })?;
        while let Ok(Some(ent)) = read.next_entry().await {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(fname) = path.file_name() else { continue };
            let bytes = read_capped(&path, MAX_AGENT_BYTES).await?;
            atomic_write(&dst_cat.join(fname), &bytes).await?;
            seeded += 1;
        }
    }

    // Carry the tooling forward so the seeded copy is self-describing: the
    // category list is then read from the working tree, not just the baseline.
    let src_script = baseline.join("scripts").join("convert.sh");
    if let Ok(bytes) = read_capped(&src_script, MAX_AGENT_BYTES).await {
        let dst_script = dest.join("scripts").join("convert.sh");
        if let Some(parent) = dst_script.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = atomic_write(&dst_script, &bytes).await;
    }

    tracing::info!("corpus: seeded {seeded} agents from baseline");
    Ok(())
}

// ---------- Persistence ----------

/// Write `corpus-index.json` + `corpus-meta.json` atomically into the
/// state dir. The meta `fetched_at` is preserved from any prior meta;
/// when none exists (fresh baseline seed) it is stamped once with the
/// current UTC time so subsequent launches don't re-stamp it (keeps the
/// index byte-stable across launches).
async fn persist(app_data_dir: &Path, corpus: &Corpus) -> Result<(), AppError> {
    let sdir = state_dir(app_data_dir);
    tokio::fs::create_dir_all(&sdir)
        .await
        .map_err(|e| AppError::Io {
            message: format!("create state dir {}: {e}", sdir.display()),
        })?;

    // Index — deterministic, no timestamp.
    let index_bytes = corpus.index_json()?;
    atomic_write(&index_path(app_data_dir), &index_bytes).await?;

    // Meta — preserve prior fetched_at/commit if present; else stamp now.
    let prior = load_stored_meta(app_data_dir).await;
    let fetched_at = prior
        .as_ref()
        .map(|m| m.fetched_at.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let commit = prior.as_ref().and_then(|m| m.commit.clone());

    let stored = StoredMeta {
        version: corpus.meta.version.clone(),
        commit,
        fetched_at,
        count: corpus.count(),
    };
    let meta_bytes = serde_json::to_vec_pretty(&stored).map_err(|e| AppError::Internal {
        message: format!("serialize corpus-meta.json: {e}"),
    })?;
    atomic_write(&meta_path(app_data_dir), &meta_bytes).await?;
    Ok(())
}

/// Load `corpus-meta.json` if present + parseable, else `None`.
async fn load_stored_meta(app_data_dir: &Path) -> Option<StoredMeta> {
    let path = meta_path(app_data_dir);
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

// ---------- Refresh (live tarball) ----------

/// Fetch the GitHub tarball, extract its category dirs over the working
/// copy, re-index, and persist. Returns the fresh [`CorpusMeta`].
///
/// The extraction is done into a temp dir first, then the known category
/// dirs are swapped in, so a partial/failed download never corrupts the
/// live `corpus/`.
async fn refresh(app_data_dir: &Path) -> Result<CorpusMeta, AppError> {
    // A read-only catalog source (Bundled-app-data is fine to refresh; a
    // user clone we lack permission to manage is NOT) must never be written by
    // a tarball refresh. Bundled writes into app data, so it's always allowed.
    let source = load_catalog_source(app_data_dir).await;
    if matches!(&source, CatalogSource::UserClone { manage: false, .. }) {
        return Err(AppError::InvalidArgument {
            message: "catalog source is a read-only user clone; enable manage-with-permission or switch source to refresh".into(),
        });
    }

    let bytes = download_corpus_tarball().await?;

    // Discover the live category set from the tarball's OWN tooling
    // (`scripts/convert.sh`) so a freshly-added upstream division is picked up
    // automatically. Falls back to the canonical default if absent.
    let categories = categories_from_tarball(&bytes)
        .unwrap_or_else(|| DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect());

    // Extract the category dirs (+ the tooling) into the active catalog root.
    // The tarball has a single top-level `agency-agents-main/` prefix we strip.
    let dir = catalog_root(app_data_dir, &source);
    let extracted = extract_categories(&bytes, &dir, &categories)?;
    if extracted == 0 {
        return Err(AppError::Internal {
            message: "corpus tarball contained no agent files under known categories".into(),
        });
    }

    // Re-index from the freshly-written working copy. Use a `main`-tagged
    // version marker; codeload does not expose the resolved commit SHA in
    // the tarball, so we record the ref name. A later phase can resolve
    // the exact SHA via the GitHub API if needed.
    let version = format!("github:main@{}", chrono::Utc::now().format("%Y-%m-%d"));
    let mut corpus = build_from_dir(&dir, &version, &categories).await?;
    let fetched_at = chrono::Utc::now().to_rfc3339();
    corpus.meta.fetched_at = fetched_at.clone();

    // Persist a fresh meta (overwrite fetched_at/version this time —
    // unlike the baseline persist which preserves prior fetched_at).
    let sdir = state_dir(app_data_dir);
    tokio::fs::create_dir_all(&sdir)
        .await
        .map_err(|e| AppError::Io {
            message: format!("create state dir {}: {e}", sdir.display()),
        })?;
    let index_bytes = corpus.index_json()?;
    atomic_write(&index_path(app_data_dir), &index_bytes).await?;
    let stored = StoredMeta {
        version: version.clone(),
        commit: None,
        fetched_at: fetched_at.clone(),
        count: corpus.count(),
    };
    let meta_bytes = serde_json::to_vec_pretty(&stored).map_err(|e| AppError::Internal {
        message: format!("serialize corpus-meta.json: {e}"),
    })?;
    atomic_write(&meta_path(app_data_dir), &meta_bytes).await?;

    Ok(corpus.meta)
}

/// Fetch the GitHub `codeload` tarball for the corpus (capped, timed out).
/// Shared by [`refresh`] and managed-catalog provisioning (the git-absent path).
async fn download_corpus_tarball() -> Result<Vec<u8>, AppError> {
    let client = reqwest::Client::builder()
        .timeout(REFRESH_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| AppError::Network {
            url: CORPUS_TARBALL_URL.to_string(),
            message: format!("client build: {e}"),
        })?;
    let resp = client
        .get(CORPUS_TARBALL_URL)
        .send()
        .await
        .map_err(|e| AppError::Network {
            url: CORPUS_TARBALL_URL.to_string(),
            message: e.to_string(),
        })?;
    if !resp.status().is_success() {
        return Err(AppError::HttpStatus {
            url: CORPUS_TARBALL_URL.to_string(),
            status: resp.status().as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|e| AppError::Network {
        url: CORPUS_TARBALL_URL.to_string(),
        message: format!("read body: {e}"),
    })?;
    if bytes.len() as u64 > MAX_TARBALL_BYTES {
        return Err(AppError::Io {
            message: format!("corpus tarball {} bytes exceeds {} cap", bytes.len(), MAX_TARBALL_BYTES),
        });
    }
    Ok(bytes.to_vec())
}

/// Gunzip the tarball and decode it to raw `tar` bytes, capped against a gzip
/// bomb. Shared by [`extract_categories`] and [`categories_from_tarball`].
fn gunzip_capped(tar_gz: &[u8]) -> Result<Vec<u8>, AppError> {
    use std::io::Read;
    let gz = flate2::read::GzDecoder::new(tar_gz);
    let mut capped = gz.take(MAX_TARBALL_BYTES * 8);
    let mut tar_bytes = Vec::new();
    capped.read_to_end(&mut tar_bytes).map_err(|e| AppError::Io {
        message: format!("gunzip corpus tarball: {e}"),
    })?;
    Ok(tar_bytes)
}

/// Read `scripts/convert.sh` out of the tarball and parse its `AGENT_DIRS`
/// array, so a refresh adopts upstream's current division set. `None` if the
/// script isn't present or doesn't parse (caller falls back to the default).
fn categories_from_tarball(tar_gz: &[u8]) -> Option<Vec<String>> {
    use std::io::Read;
    let tar_bytes = gunzip_capped(tar_gz).ok()?;
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().ok()?;
        let comps: Vec<String> = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();
        // top/scripts/convert.sh
        if comps.len() == 3 && comps[1] == "scripts" && comps[2] == "convert.sh" {
            let mut text = String::new();
            entry.read_to_string(&mut text).ok()?;
            return parse_agent_dirs(&text).filter(|v| !v.is_empty());
        }
    }
    None
}

/// Gunzip + untar `tar_gz`, writing every `<category>/<slug>.md` whose category
/// is in `categories` into `<dest>/<category>/`, plus `scripts/convert.sh` (so
/// the working copy stays self-describing). The codeload tarball nests
/// everything under a single `agency-agents-main/` top-level dir, which we
/// strip. Returns the count of agent files written.
///
/// Path-traversal safe: we only ever join the *sanitized* `category` +
/// `file_name` onto `dest`; the raw archive path is never used to build a
/// write target.
fn extract_categories(tar_gz: &[u8], dest: &Path, categories: &[String]) -> Result<u32, AppError> {
    use std::io::Read;

    let tar_bytes = gunzip_capped(tar_gz)?;
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let entries = archive.entries().map_err(|e| AppError::Io {
        message: format!("read tar entries: {e}"),
    })?;

    let is_category = |c: &str| categories.iter().any(|cat| cat == c);
    let mut written = 0u32;
    for entry in entries {
        let mut entry = entry.map_err(|e| AppError::Io {
            message: format!("tar entry: {e}"),
        })?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| AppError::Io {
            message: format!("tar entry path: {e}"),
        })?;
        // Strip the single top-level `agency-agents-main/` component.
        let comps: Vec<String> = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        // Persist the tooling so subsequent launches re-derive categories.
        if comps.len() == 3 && comps[1] == "scripts" && comps[2] == "convert.sh" {
            let scripts_dir = dest.join("scripts");
            let _ = std::fs::create_dir_all(&scripts_dir);
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                let _ = std::fs::write(scripts_dir.join("convert.sh"), &buf);
            }
            continue;
        }

        if comps.len() < 3 {
            continue; // need top/<category>/<file>
        }
        let category = comps[1].as_str();
        let fname = comps.last().unwrap().as_str();
        if !is_category(category) {
            continue;
        }
        if !fname.ends_with(".md") || fname == "README.md" {
            continue;
        }
        // Sanitized target — built only from validated components.
        let cat_dir = dest.join(category);
        std::fs::create_dir_all(&cat_dir).map_err(|e| AppError::Io {
            message: format!("create {}: {e}", cat_dir.display()),
        })?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| AppError::Io {
            message: format!("read tar file {}: {e}", fname),
        })?;
        std::fs::write(cat_dir.join(fname), &buf).map_err(|e| AppError::Io {
            message: format!("write {}: {e}", cat_dir.join(fname).display()),
        })?;
        written += 1;
    }
    Ok(written)
}

// ---------- Small fs helper ----------

/// Read up to `max` bytes; error (not truncate) on oversize. Mirrors
/// `util::fs::read_capped` but accepts a sync `Path` + tokio read so we
/// don't need to thread the catalog's exact helper here.
async fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>, AppError> {
    let bytes = tokio::fs::read(path).await.map_err(|e| AppError::Io {
        message: format!("read {}: {e}", path.display()),
    })?;
    if bytes.len() as u64 > max {
        return Err(AppError::Io {
            message: format!("{} exceeds {} byte cap", path.display(), max),
        });
    }
    Ok(bytes)
}

// =====================================================================
// Catalog detection / provisioning / pull (#1 clone-as-source-of-truth)
// =====================================================================

/// `~/.agency-agents` — the default managed-catalog location (shared with the
/// agency-agents CLI).
fn home_agency_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".agency-agents"))
}

/// Is a `git` binary on PATH? Determines clone/pull vs tarball-snapshot.
async fn git_available() -> bool {
    run_git(&["--version"], None).await.is_ok()
}

/// Is `root` a git checkout (so a pull is `git pull`, not a tarball swap)?
fn has_git_dir(root: &Path) -> bool {
    root.join(".git").exists()
}

/// Run `git` with `args` (optionally in `cwd`) off the async runtime. Errors
/// carry git's stderr so failures are diagnosable.
async fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<String, AppError> {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let cwd = cwd.map(|p| p.to_path_buf());
    let out = tokio::task::spawn_blocking(move || {
        let mut c = std::process::Command::new("git");
        if let Some(d) = &cwd {
            c.current_dir(d);
        }
        c.args(&owned).output()
    })
    .await
    .map_err(|e| AppError::Internal { message: format!("join git task: {e}") })?
    .map_err(|e| AppError::Io { message: format!("spawn git: {e}") })?;

    if !out.status.success() {
        return Err(AppError::Io {
            message: format!("git {:?} failed: {}", args, String::from_utf8_lossy(&out.stderr).trim()),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Quick agent count for a candidate badge: top-level `.md` files across the
/// root's discovered categories. Cheap + synchronous (cold path, small repo).
fn quick_agent_count(root: &Path) -> u32 {
    let mut n = 0u32;
    for cat in discover_categories(root) {
        if let Ok(rd) = std::fs::read_dir(root.join(&cat)) {
            n += rd
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                .filter(|e| e.file_name().to_string_lossy() != "README.md")
                .count() as u32;
        }
    }
    n
}

/// Build a [`CatalogCandidate`] for `path` if it looks like a catalog.
fn candidate_for(path: &Path, kind: &str) -> Option<CatalogCandidate> {
    if !looks_like_catalog(path) {
        return None;
    }
    Some(CatalogCandidate {
        path: path.to_string_lossy().to_string(),
        kind: kind.to_string(),
        has_git: has_git_dir(path),
        agent_count: quick_agent_count(path),
    })
}

/// Detect candidate catalogs. Always checks `~/.agency-agents`; when `scan` is
/// true also walks common dev roots for an `agency-agents` checkout (the "Find
/// Agency Agents" button). Pure of app state — safe to call anytime.
async fn detect_catalogs(scan: bool) -> CatalogDetection {
    let git_available = git_available().await;
    let mut candidates: Vec<CatalogCandidate> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let push = |c: Option<CatalogCandidate>, list: &mut Vec<CatalogCandidate>, seen: &mut std::collections::HashSet<String>| {
        if let Some(c) = c {
            if seen.insert(c.path.clone()) {
                list.push(c);
            }
        }
    };

    if let Some(managed) = home_agency_dir() {
        push(candidate_for(&managed, "managed"), &mut candidates, &mut seen);
    }

    if scan {
        if let Some(home) = dirs::home_dir() {
            for root in SCAN_ROOTS {
                // Look for `<home>/<root>/agency-agents` and a direct
                // `<home>/<root>` that is itself a catalog.
                let base = home.join(root);
                push(candidate_for(&base.join("agency-agents"), "userClone"), &mut candidates, &mut seen);
                // One level of children named with "agency" (cheap heuristic).
                if let Ok(rd) = std::fs::read_dir(&base) {
                    for ent in rd.filter_map(|e| e.ok()) {
                        let p = ent.path();
                        if p.is_dir() && p.file_name().map(|n| n.to_string_lossy().contains("agency")).unwrap_or(false) {
                            push(candidate_for(&p, "userClone"), &mut candidates, &mut seen);
                        }
                    }
                }
            }
        }
    }

    CatalogDetection { git_available, scanned: scan, candidates }
}

/// Ensure `~/.agency-agents` holds a catalog, cloning (git) or unpacking the
/// snapshot (no git) as needed. Returns the managed root path. Idempotent: if
/// it already looks like a catalog, this is a no-op (use pull to update).
async fn provision_managed() -> Result<PathBuf, AppError> {
    let path = home_agency_dir().ok_or_else(|| AppError::Io {
        message: "cannot resolve home directory".into(),
    })?;
    if looks_like_catalog(&path) {
        return Ok(path); // already provisioned
    }

    let empty = is_empty_dir(&path);
    if git_available().await && !path.exists() {
        // git clone into a fresh dir (clone requires absent/empty target).
        // Full clone (not shallow) so commit history is available for accurate
        // behind/ahead counts and diff stats in the Catalog status panel.
        run_git(&["clone", CATALOG_GIT_URL, &path.to_string_lossy()], None).await?;
    } else if git_available().await && empty {
        // Full clone (not shallow) so commit history is available for accurate
        // behind/ahead counts and diff stats in the Catalog status panel.
        run_git(&["clone", CATALOG_GIT_URL, &path.to_string_lossy()], None).await?;
    } else {
        // No git (or a non-empty target): drop the snapshot tarball in place.
        tokio::fs::create_dir_all(&path).await.map_err(|e| AppError::Io {
            message: format!("create {}: {e}", path.display()),
        })?;
        let bytes = download_corpus_tarball().await?;
        let categories = categories_from_tarball(&bytes)
            .unwrap_or_else(|| DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect());
        let written = extract_categories(&bytes, &path, &categories)?;
        if written == 0 {
            return Err(AppError::Internal {
                message: "provision: snapshot tarball contained no agent files".into(),
            });
        }
    }
    Ok(path)
}

/// Pull the active catalog root up to date. Git checkout → `git pull --ff-only`;
/// otherwise a tarball refresh into the root. Read-only sources are rejected by
/// the caller; Bundled refreshes its app-data copy.
async fn pull_active(app_data_dir: &Path) -> Result<(), AppError> {
    let source = load_catalog_source(app_data_dir).await;
    if matches!(&source, CatalogSource::UserClone { manage: false, .. }) {
        return Err(AppError::InvalidArgument {
            message: "catalog source is read-only (manage-with-permission is off)".into(),
        });
    }
    let root = catalog_root(app_data_dir, &source);
    if has_git_dir(&root) && git_available().await {
        run_git(&["-C", &root.to_string_lossy(), "pull", "--ff-only"], None).await?;
        Ok(())
    } else {
        // Tarball refresh writes into the active root (refresh() resolves it).
        refresh(app_data_dir).await.map(|_| ())
    }
}

// =====================================================================
// Tauri commands (contracts.md §C — corpus surface)
// =====================================================================

use crate::state::AppState;
use tauri::{AppHandle, Manager, State};

/// Resolve the bundled baseline dir from the Tauri resource dir. In dev
/// the resources live under the crate; in a bundled app they're inside
/// the `.app`. Tauri's `resource_dir()` resolves both.
fn baseline_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let res = app.path().resource_dir().map_err(|e| AppError::Internal {
        message: format!("resolve resource_dir: {e}"),
    })?;
    Ok(res.join("resources").join("corpus-baseline"))
}

/// Resolve the per-app data dir via Tauri's path resolver (honors the
/// bundle id `com.zerologic.agency-agents-app`).
pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path().app_data_dir().map_err(|e| AppError::Internal {
        message: format!("resolve app_data_dir: {e}"),
    })
}

/// Read the raw, byte-exact `.md` source of a seeded agent from the working
/// corpus copy (`<app_data>/corpus/<category>/<slug>.md`). Identity-tool
/// installs (claude-code, copilot) ship this verbatim, and provenance
/// reconciliation re-renders against it. Path is derived from app data +
/// the agent's own category/slug — never from IPC input.
pub(crate) async fn read_source(
    app: &AppHandle,
    category: &str,
    slug: &str,
) -> Result<String, AppError> {
    let adir = app_data_dir(app)?;
    let source = load_catalog_source(&adir).await;
    let cat_dir = catalog_root(&adir, &source).join(category);
    let fname = format!("{slug}.md");
    // Flat path first (the common case); fall back to a recursive search for
    // nested agents (e.g. game-development/godot/<slug>.md in a real clone).
    let flat = cat_dir.join(&fname);
    let path = if flat.exists() {
        flat
    } else {
        find_md_under(&cat_dir, &fname).unwrap_or(flat)
    };
    let bytes = read_capped(&path, MAX_AGENT_BYTES).await?;
    String::from_utf8(bytes).map_err(|e| AppError::Io {
        message: format!("agent source {slug}.md not UTF-8: {e}"),
    })
}

/// Ensure the in-memory corpus is built + memoized on `AppState`, then
/// return the shared `Arc`. First call seeds (if needed), parses, and
/// persists the index; subsequent calls are a cheap cache read.
pub(crate) async fn ensure_corpus(app: &AppHandle, state: &AppState) -> Result<Arc<Corpus>, AppError> {
    // Hold the cache lock across the ENTIRE init — check, seed, parse, store.
    // The frontend fires corpus_list + corpus_categories (+ corpus_status)
    // concurrently on mount; a released-lock double-check would let each run
    // `seed_from_baseline` at once, racing on the same `<file>.tmp` paths
    // (rename → ENOENT). Serializing the first load is correct and cheap:
    // it happens once, and every later call is a fast locked cache read.
    let mut cached = state.corpus_cache.lock().await;
    if let Some(c) = cached.as_ref() {
        return Ok(Arc::clone(c));
    }
    let adir = app_data_dir(app)?;
    let bdir = baseline_dir(app)?;
    let corpus = Arc::new(resolve_active(&adir, &bdir).await);
    *cached = Some(Arc::clone(&corpus));
    Ok(corpus)
}

/// `corpus_status()` — version / commit / fetched-at / count for the
/// active corpus.
#[tauri::command]
pub async fn corpus_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CorpusMeta, AppError> {
    let corpus = ensure_corpus(&app, &state).await?;
    Ok(corpus.meta())
}

/// `corpus_refresh()` — fetch the live tarball, re-index, swap the
/// memoized corpus, and return the fresh meta.
#[tauri::command]
pub async fn corpus_refresh(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CorpusMeta, AppError> {
    state.require_network("corpus_refresh").await?;

    // Single-flight: a second click fast-fails rather than queuing a
    // duplicate download.
    let _flight = match state.corpus_refresh_in_flight.try_lock() {
        Ok(g) => g,
        Err(_) => {
            return Err(AppError::InvalidArgument {
                message: "corpus refresh already in progress".into(),
            });
        }
    };

    let adir = app_data_dir(&app)?;
    refresh(&adir).await?;

    // Rebuild the in-memory copy from the freshly-written working tree and
    // swap the memoized Arc so subsequent reads see the new corpus.
    let bdir = baseline_dir(&app)?;
    let fresh = Arc::new(resolve_active(&adir, &bdir).await);
    let meta = fresh.meta();
    {
        let mut cached = state.corpus_cache.lock().await;
        *cached = Some(fresh);
    }
    Ok(meta)
}

/// `catalog_source_get()` — the persisted [`CatalogSource`] (default Bundled).
#[tauri::command]
pub async fn catalog_source_get(app: AppHandle) -> Result<CatalogSource, AppError> {
    let adir = app_data_dir(&app)?;
    Ok(load_catalog_source(&adir).await)
}

/// `catalog_configured()` — whether the user has made an explicit catalog-source
/// choice yet (i.e. `state/catalog.json` exists). Drives the first-run prompt:
/// `false` ⇒ show the catalog-source picker before anything else.
#[tauri::command]
pub async fn catalog_configured(app: AppHandle) -> Result<bool, AppError> {
    let adir = app_data_dir(&app)?;
    Ok(catalog_source_path(&adir).exists())
}

/// `catalog_source_set(source)` — switch where the catalog is read from, then
/// rebuild + swap the in-memory corpus so every view reflects the new source.
/// Validates that a `Managed`/`UserClone` path exists and looks like a catalog
/// (has at least one known category dir or `scripts/convert.sh`).
#[tauri::command]
pub async fn catalog_source_set(
    app: AppHandle,
    state: State<'_, AppState>,
    source: CatalogSource,
) -> Result<CorpusMeta, AppError> {
    // Validate non-bundled roots before committing to them.
    if let CatalogSource::Managed { path } | CatalogSource::UserClone { path, .. } = &source {
        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(AppError::InvalidArgument {
                message: format!("catalog path is not a directory: {path}"),
            });
        }
        if !looks_like_catalog(&root) {
            return Err(AppError::InvalidArgument {
                message: format!(
                    "{path} doesn't look like an agency-agents catalog (no scripts/convert.sh or category dirs)"
                ),
            });
        }
    }

    let adir = app_data_dir(&app)?;
    save_catalog_source(&adir, &source).await?;
    rebuild_corpus(&app, &state).await
}

/// Rebuild the in-memory corpus from the currently-persisted source and swap
/// the memoized `Arc`, so every view reflects the latest catalog state. Shared
/// by source switching, provisioning, and pull.
async fn rebuild_corpus(app: &AppHandle, state: &AppState) -> Result<CorpusMeta, AppError> {
    let adir = app_data_dir(app)?;
    let bdir = baseline_dir(app)?;
    let fresh = Arc::new(resolve_active(&adir, &bdir).await);
    let meta = fresh.meta();
    {
        let mut cached = state.corpus_cache.lock().await;
        *cached = Some(fresh);
    }
    Ok(meta)
}

/// `catalog_detect(scan)` — discover candidate catalogs (always checks
/// `~/.agency-agents`; `scan=true` also walks common dev roots).
#[tauri::command]
pub async fn catalog_detect(scan: bool) -> Result<CatalogDetection, AppError> {
    Ok(detect_catalogs(scan).await)
}

/// `catalog_provision_managed()` — clone/snapshot into `~/.agency-agents`, set
/// it as the managed source, and rebuild. The "set one up for me" path.
#[tauri::command]
pub async fn catalog_provision_managed(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CorpusMeta, AppError> {
    state.require_network("catalog_provision_managed").await?;
    let path = provision_managed().await?;
    let adir = app_data_dir(&app)?;
    save_catalog_source(&adir, &CatalogSource::Managed { path: path.to_string_lossy().to_string() }).await?;
    rebuild_corpus(&app, &state).await
}

/// `catalog_pull()` — update the active catalog root (git pull or tarball
/// refresh), then rebuild. Rejected for a read-only user clone.
#[tauri::command]
pub async fn catalog_pull(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CorpusMeta, AppError> {
    state.require_network("catalog_pull").await?;
    let adir = app_data_dir(&app)?;
    pull_active(&adir).await?;
    rebuild_corpus(&app, &state).await
}

/// `catalog_status()` — provenance + freshness of the active catalog (source,
/// git commit/branch/dirty, remote repo, version, agent count). Local-only (no
/// network); the git fields are empty for a bundled/snapshot source.
#[tauri::command]
pub async fn catalog_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CatalogStatus, AppError> {
    let adir = app_data_dir(&app)?;
    let source = load_catalog_source(&adir).await;
    let corpus = ensure_corpus(&app, &state).await?;
    let meta = corpus.meta();
    let root = catalog_root(&adir, &source);

    let is_git = has_git_dir(&root) && git_available().await;
    let mut branch = None;
    let mut commit = None;
    let mut last_commit_subject = None;
    let mut last_commit_date = None;
    let mut dirty_count = 0u32;
    let mut remote_url = None;
    let mut repo_slug = None;
    if is_git {
        let rs = root.to_string_lossy().to_string();
        branch = run_git(&["-C", &rs, "rev-parse", "--abbrev-ref", "HEAD"], None)
            .await
            .ok()
            .map(|s| s.trim().to_string());
        commit = run_git(&["-C", &rs, "rev-parse", "--short", "HEAD"], None)
            .await
            .ok()
            .map(|s| s.trim().to_string());
        if let Ok(log) = run_git(&["-C", &rs, "log", "-1", "--format=%s%x1f%cI"], None).await {
            let mut it = log.trim().splitn(2, '\u{1f}');
            last_commit_subject = it.next().map(|s| s.to_string()).filter(|s| !s.is_empty());
            last_commit_date = it.next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        }
        if let Ok(porcelain) = run_git(&["-C", &rs, "status", "--porcelain"], None).await {
            dirty_count = porcelain.lines().filter(|l| !l.trim().is_empty()).count() as u32;
        }
        remote_url = run_git(&["-C", &rs, "remote", "get-url", "origin"], None)
            .await
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        repo_slug = remote_url
            .as_deref()
            .and_then(extract_github_repo)
            .map(|r| format!("{}/{}", r.owner, r.repo));
    }

    let root_out = match source {
        CatalogSource::Bundled => None,
        _ => Some(root.to_string_lossy().to_string()),
    };

    Ok(CatalogStatus {
        source,
        root: root_out,
        is_git,
        branch,
        commit,
        last_commit_subject,
        last_commit_date,
        dirty_count,
        remote_url,
        repo_slug,
        version: meta.version,
        fetched_at: meta.fetched_at,
        agent_count: corpus.count(),
    })
}

/// `catalog_check_updates()` — fetch the active git catalog and report how far
/// behind/ahead upstream it is, plus a `git diff --stat` preview (the "stats on
/// diffs"). For a non-git source, returns `is_git=false` (the UI offers a plain
/// snapshot refresh instead). Network: runs `git fetch`.
#[tauri::command]
pub async fn catalog_check_updates(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CatalogUpdateCheck, AppError> {
    state.require_network("catalog_check_updates").await?;
    let adir = app_data_dir(&app)?;
    let source = load_catalog_source(&adir).await;
    let root = catalog_root(&adir, &source);

    if !(has_git_dir(&root) && git_available().await) {
        return Ok(CatalogUpdateCheck {
            is_git: false,
            behind: 0,
            ahead: 0,
            changed_files: 0,
            diffstat: String::new(),
            up_to_date: false,
        });
    }

    let rs = root.to_string_lossy().to_string();
    run_git(&["-C", &rs, "fetch", "--quiet"], None).await?;

    // "<ahead>\t<behind>" relative to the upstream tracking branch.
    let (mut ahead, mut behind) = (0u32, 0u32);
    if let Ok(counts) = run_git(&["-C", &rs, "rev-list", "--left-right", "--count", "HEAD...@{u}"], None).await {
        let mut it = counts.split_whitespace();
        ahead = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        behind = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    }

    let (mut diffstat, mut changed_files) = (String::new(), 0u32);
    if behind > 0 {
        diffstat = run_git(&["-C", &rs, "diff", "--stat", "HEAD..@{u}"], None).await.unwrap_or_default();
        if let Ok(names) = run_git(&["-C", &rs, "diff", "--name-only", "HEAD..@{u}"], None).await {
            changed_files = names.lines().filter(|l| !l.trim().is_empty()).count() as u32;
        }
    }

    Ok(CatalogUpdateCheck {
        is_git: true,
        behind,
        ahead,
        changed_files,
        diffstat,
        up_to_date: behind == 0,
    })
}

/// Heuristic: does `root` hold an agency-agents catalog? True if it has the
/// repo tooling or at least one of the canonical category dirs with agents.
fn looks_like_catalog(root: &Path) -> bool {
    if root.join("scripts").join("convert.sh").exists() {
        return true;
    }
    DEFAULT_CATEGORIES.iter().any(|c| root.join(c).is_dir())
}

/// `corpus_list(category?)` — list view (bodies omitted).
#[tauri::command]
pub async fn corpus_list(
    app: AppHandle,
    state: State<'_, AppState>,
    category: Option<String>,
) -> Result<Vec<Agent>, AppError> {
    let corpus = ensure_corpus(&app, &state).await?;
    Ok(corpus.list(category.as_deref()))
}

/// `corpus_get(slug)` — full agent incl. body.
#[tauri::command]
pub async fn corpus_get(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
) -> Result<Agent, AppError> {
    let corpus = ensure_corpus(&app, &state).await?;
    corpus.get(&slug).ok_or(AppError::InvalidArgument {
        message: format!("unknown agent slug: {slug}"),
    })
}

/// `corpus_categories()` — the Discover grid (one tile per division declared
/// by the active catalog's tooling) with per-category counts.
#[tauri::command]
pub async fn corpus_categories(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Category>, AppError> {
    let corpus = ensure_corpus(&app, &state).await?;
    Ok(corpus.categories())
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn write_agent(dir: &Path, category: &str, slug: &str, name: &str, body: &str) {
        let cat = dir.join(category);
        std::fs::create_dir_all(&cat).unwrap();
        let content = format!("---\nname: {name}\ndescription: d\n---\n{body}\n");
        std::fs::write(cat.join(format!("{slug}.md")), content).unwrap();
    }

    #[tokio::test]
    async fn build_indexes_agents_in_stable_order() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Write out of order across two categories.
        write_agent(dir, "engineering", "zeta", "Zeta", "z");
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "design", "mid", "Mid", "m");

        let corpus = build_from_dir(dir, "test", &discover_categories(dir)).await.unwrap();
        assert_eq!(corpus.count(), 3);
        // design < engineering, and within engineering alpha < zeta.
        let order: Vec<&str> = corpus.agents.iter().map(|a| a.slug.as_str()).collect();
        assert_eq!(order, vec!["mid", "alpha", "zeta"]);
    }

    #[tokio::test]
    async fn build_indexes_nested_agents() {
        // Real clones nest agents in subdirs (game-development/godot/<slug>.md).
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "flat-one", "Flat One", "x");
        let nested = dir.join("game-development").join("godot");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("godot-shader-developer.md"),
            "---\nname: Godot Shader Developer\ndescription: d\n---\nbody\n",
        )
        .unwrap();

        let corpus = build_from_dir(dir, "v", &discover_categories(dir)).await.unwrap();
        let nested_agent = corpus.get("godot-shader-developer");
        assert!(nested_agent.is_some(), "nested agent must be indexed");
        assert_eq!(
            nested_agent.unwrap().category,
            "game-development",
            "category is the top-level dir, not the subdir"
        );
        assert!(corpus.get("flat-one").is_some(), "flat agent still indexed");
    }

    #[tokio::test]
    async fn index_json_is_byte_stable_across_builds() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "design", "mid", "Mid", "m");

        let cats = discover_categories(dir);
        let a = build_from_dir(dir, "v", &cats).await.unwrap().index_json().unwrap();
        let b = build_from_dir(dir, "v", &cats).await.unwrap().index_json().unwrap();
        assert_eq!(a, b, "corpus-index.json must be deterministic");
    }

    #[tokio::test]
    async fn list_omits_body_get_includes_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "the persona body");
        let corpus = build_from_dir(dir, "v", &discover_categories(dir)).await.unwrap();

        let listed = corpus.list(None);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].body, "", "list view must omit body");

        let full = corpus.get("alpha").unwrap();
        assert!(full.body.contains("the persona body"), "get must include body");
    }

    #[tokio::test]
    async fn list_filters_by_category() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "design", "mid", "Mid", "m");
        let corpus = build_from_dir(dir, "v", &discover_categories(dir)).await.unwrap();

        let eng = corpus.list(Some("engineering"));
        assert_eq!(eng.len(), 1);
        assert_eq!(eng[0].slug, "alpha");
    }

    #[tokio::test]
    async fn categories_returns_all_16_with_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "engineering", "beta", "Beta", "b");
        // No scripts/ in this tempdir → discover falls back to DEFAULT_CATEGORIES.
        let corpus = build_from_dir(dir, "v", &discover_categories(dir)).await.unwrap();

        let cats = corpus.categories();
        assert_eq!(cats.len(), 16, "all 16 default categories always returned");
        let eng = cats.iter().find(|c| c.slug == "engineering").unwrap();
        assert_eq!(eng.count, 2);
        assert_eq!(eng.label, "Engineering");
        assert_eq!(eng.icon, "Code");
        // Empty category still present with count 0.
        let fin = cats.iter().find(|c| c.slug == "finance").unwrap();
        assert_eq!(fin.count, 0);
        // `strategy` is a canonical division; `integrations` is NOT (it's
        // convert.sh output), so it must never appear as a category.
        assert!(cats.iter().any(|c| c.slug == "strategy"), "strategy is canonical");
        assert!(!cats.iter().any(|c| c.slug == "integrations"), "integrations is not a category");
    }

    #[tokio::test]
    async fn non_agent_files_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "real", "Real", "x");
        // A README with no frontmatter.
        let cat = dir.join("engineering");
        std::fs::write(cat.join("README.md"), "# Examples\nnope\n").unwrap();
        // A workflow doc with no frontmatter.
        std::fs::write(cat.join("workflow.md"), "# Workflow\nnope\n").unwrap();

        let corpus = build_from_dir(dir, "v", &discover_categories(dir)).await.unwrap();
        assert_eq!(corpus.count(), 1);
        assert!(corpus.get("real").is_some());
        assert!(corpus.get("workflow").is_none());
    }

    #[tokio::test]
    async fn seed_then_build_round_trips() {
        let baseline = tempfile::tempdir().unwrap();
        write_agent(baseline.path(), "engineering", "alpha", "Alpha", "a");
        write_agent(baseline.path(), "design", "mid", "Mid", "m");

        let app_data = tempfile::tempdir().unwrap();
        let corpus = resolve_active(app_data.path(), baseline.path()).await;
        assert_eq!(corpus.count(), 2);
        // Working copy + index were written.
        assert!(corpus_dir(app_data.path()).join("engineering/alpha.md").exists());
        assert!(index_path(app_data.path()).exists());
        assert!(meta_path(app_data.path()).exists());
    }

    #[test]
    fn title_case_handles_hyphens() {
        assert_eq!(title_case("game-development"), "Game Development");
        assert_eq!(title_case("engineering"), "Engineering");
    }

    #[test]
    fn category_meta_resolves_from_bundled_json() {
        let bundled = bundled_division_meta();
        let (label, icon, color) = category_meta_from(&bundled, "engineering");
        assert_eq!(label, "Engineering");
        assert_eq!(icon, "Code");
        assert_eq!(color, "#3B82F6");
    }

    #[test]
    fn category_meta_falls_back_for_unknown_slug() {
        let bundled = bundled_division_meta();
        let (label, icon, color) = category_meta_from(&bundled, "made-up-division");
        assert_eq!(label, "Made Up Division");
        assert_eq!(icon, "Folder");
        assert_eq!(color, default_division_color());
    }

    #[test]
    fn load_division_meta_missing_file_uses_bundled() {
        // First-run / pre-#592 clone: no divisions.json at the root → bundled.
        let root = tempfile::tempdir().unwrap();
        let meta = load_division_meta(root.path());
        assert_eq!(meta.get("engineering").unwrap().color, "#3B82F6");
    }

    #[test]
    fn load_division_meta_overlays_catalog_divisions_json() {
        // A catalog divisions.json overrides a known division AND introduces a
        // brand-new one the bundled floor has never heard of (the whole point:
        // a new catalog division presents correctly without an app update).
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(DIVISIONS_FILENAME),
            r##"{ "divisions": {
                "engineering": { "label": "Engineering", "icon": "Cpu", "color": "#000000" },
                "robotics":    { "label": "Robotics",    "icon": "Bot", "color": "#FF00FF" }
            } }"##,
        )
        .unwrap();
        let meta = load_division_meta(root.path());
        // Overridden from the catalog.
        let eng = meta.get("engineering").unwrap();
        assert_eq!((eng.icon.as_str(), eng.color.as_str()), ("Cpu", "#000000"));
        // Net-new division, present only in the catalog.
        assert_eq!(meta.get("robotics").unwrap().color, "#FF00FF");
        // A bundled division the catalog file omitted is retained (overlay, not replace).
        assert_eq!(meta.get("marketing").unwrap().label, "Marketing");
    }

    #[test]
    fn load_division_meta_malformed_file_uses_bundled() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(DIVISIONS_FILENAME), "{ not valid json ").unwrap();
        let meta = load_division_meta(root.path());
        assert_eq!(meta.get("engineering").unwrap().color, "#3B82F6");
    }

    /// Parse the REAL bundled baseline corpus (not a synthetic tempdir) so a
    /// malformed real agent (bad frontmatter fence, missing `name`) fails CI
    /// rather than shipping. `cargo test` runs with cwd = crate root, so the
    /// relative resource path resolves. Categories come from the bundled
    /// tooling (`scripts/convert.sh`), so `integrations/` (convert.sh output)
    /// is NOT a category and `strategy/` IS. Counts are pinned to the
    /// agency-agents snapshot — bump them on a corpus refresh.
    #[tokio::test]
    async fn real_bundled_baseline_parses_completely() {
        let dir = Path::new("resources/corpus-baseline");
        if !dir.exists() {
            // Resources not present in this build context — skip rather than fail.
            return;
        }
        // Categories are discovered from the bundled scripts/convert.sh.
        let categories = discover_categories(dir);
        assert!(categories.iter().any(|c| c == "strategy"), "tooling declares strategy");
        assert!(!categories.iter().any(|c| c == "integrations"), "integrations is convert.sh output, not a category");

        let corpus = build_from_dir(dir, "baseline-test", &categories).await.unwrap();

        // 209 = 210 prior minus the lone `integrations/` artifact
        // (backend-architect-with-memory), which is convert.sh output, not a
        // catalog persona.
        assert_eq!(corpus.count(), 209, "all bundled agent personas indexed (integrations excluded)");

        // Every agent parsed real frontmatter: non-empty name + slug, real category.
        for a in &corpus.agents {
            assert!(!a.name.trim().is_empty(), "agent {} has empty name", a.slug);
            assert!(!a.slug.trim().is_empty(), "agent has empty slug");
            assert!(
                categories.contains(&a.category),
                "agent {} has unknown category {}",
                a.slug,
                a.category
            );
        }

        // Spot-check categories that nest agents in subdirs upstream — these are
        // the ones a flat seeding would silently undercount.
        let cats = corpus.categories();
        assert_eq!(cats.len(), 16, "16 agent categories");
        let count_of = |slug: &str| cats.iter().find(|c| c.slug == slug).map(|c| c.count).unwrap_or(0);
        assert_eq!(count_of("engineering"), 30);
        assert_eq!(count_of("specialized"), 46);
        // game-development nests agents in unity/, godot/, unreal-engine/ etc.
        // upstream; a flat seeding would silently undercount these.
        assert_eq!(count_of("game-development"), 20, "nested game-dev agents included");
        // strategy is a declared division but ships no frontmatter personas.
        assert_eq!(count_of("strategy"), 0, "strategy is an empty division in the baseline");
    }

    #[test]
    fn parse_agent_dirs_reads_the_bash_array() {
        let script = r#"
# preamble
ALL_TOOLS=(claude-code copilot)
AGENT_DIRS=(
  academic design engineering   # inline comment ignored
  finance strategy
)
echo done
"#;
        let cats = parse_agent_dirs(script).unwrap();
        assert_eq!(cats, vec!["academic", "design", "engineering", "finance", "strategy"]);
        assert!(!cats.contains(&"integrations".to_string()));
    }

    #[test]
    fn parse_agent_dirs_none_when_absent() {
        assert!(parse_agent_dirs("nothing here").is_none());
    }

    #[tokio::test]
    async fn conversion_slug_resolves_filename_prefixed_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("engineering")).unwrap();
        std::fs::write(
            dir.join("engineering/engineering-frontend-developer.md"),
            "---\nname: Frontend Developer\ndescription: Builds UIs.\n---\nBody\n",
        )
        .unwrap();
        let corpus = build_from_dir(dir, "v", &["engineering".into()])
            .await
            .unwrap();

        let agent = corpus
            .get_by_conversion_slug("frontend-developer")
            .expect("convert.sh filename resolves");
        assert_eq!(agent.slug, "engineering-frontend-developer");
    }

    #[tokio::test]
    async fn catalog_source_persists_and_defaults_bundled() {
        let app_data = tempfile::tempdir().unwrap();
        // No file yet → default Bundled.
        assert_eq!(load_catalog_source(app_data.path()).await, CatalogSource::Bundled);

        let src = CatalogSource::Managed { path: "/Users/x/.agency-agents".into() };
        save_catalog_source(app_data.path(), &src).await.unwrap();
        assert_eq!(load_catalog_source(app_data.path()).await, src);

        // catalog.json is valid camelCase-tagged JSON.
        let bytes = std::fs::read(catalog_source_path(app_data.path())).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("\"kind\": \"managed\""), "tagged on kind: {text}");
    }

    #[test]
    fn catalog_root_resolves_per_source() {
        let app_data = Path::new("/app/data");
        assert_eq!(
            catalog_root(app_data, &CatalogSource::Bundled),
            corpus_dir(app_data)
        );
        assert_eq!(
            catalog_root(app_data, &CatalogSource::Managed { path: "/home/x/.agency-agents".into() }),
            PathBuf::from("/home/x/.agency-agents")
        );
        assert_eq!(
            catalog_root(app_data, &CatalogSource::UserClone { path: "/src/aa".into(), manage: true }),
            PathBuf::from("/src/aa")
        );
    }

    #[test]
    fn looks_like_catalog_detects_tooling_or_categories() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!looks_like_catalog(tmp.path()), "empty dir is not a catalog");
        // A category dir is enough.
        std::fs::create_dir_all(tmp.path().join("engineering")).unwrap();
        assert!(looks_like_catalog(tmp.path()));
        // …or the tooling.
        let tmp2 = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp2.path().join("scripts")).unwrap();
        std::fs::write(tmp2.path().join("scripts/convert.sh"), "AGENT_DIRS=(engineering)\n").unwrap();
        assert!(looks_like_catalog(tmp2.path()));
    }

    #[test]
    fn quick_count_and_candidate_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        // Not a catalog yet.
        assert!(candidate_for(root, "userClone").is_none());

        write_agent(root, "engineering", "a", "A", "x");
        write_agent(root, "engineering", "b", "B", "y");
        write_agent(root, "design", "c", "C", "z");
        std::fs::write(root.join("engineering/README.md"), "# readme").unwrap();

        assert_eq!(quick_agent_count(root), 3, "README excluded; 3 real agents");
        let cand = candidate_for(root, "userClone").unwrap();
        assert_eq!(cand.kind, "userClone");
        assert_eq!(cand.agent_count, 3);
        assert!(!cand.has_git, "no .git in this tempdir");
    }

    #[test]
    fn discover_categories_falls_back_to_default_without_scripts() {
        let tmp = tempfile::tempdir().unwrap();
        let cats = discover_categories(tmp.path());
        assert_eq!(cats.len(), 16);
        assert_eq!(cats, DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn discover_categories_reads_bundled_tooling() {
        let dir = Path::new("resources/corpus-baseline");
        if !dir.join("scripts/convert.sh").exists() {
            return; // resources absent in this build context
        }
        let cats = discover_categories(dir);
        assert!(cats.contains(&"strategy".to_string()));
        assert!(!cats.contains(&"integrations".to_string()));
    }
}
