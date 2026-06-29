//! Install + reconcile — the cross-tool agent state layer (contracts.md §C,
//! systemPatterns.md §2–5). This is the differentiator: the AI tools have no
//! install registry, so the app IS the database.
//!
//! - **ledger** (`installs.json`): every install action we performed.
//! - **reconcile**: diff ledger ↔ disk ↔ corpus-index into the 5 states.
//! - **tools / projects**: detected tools and project-scoped install surfaces.
//!
//! Provenance is by hash-match only — we never mutate agent content. An
//! installed file is "ours/current" when its bytes equal a fresh render of its
//! slug for its tool (the deterministic `render/` layer makes that reproducible).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use tauri::{AppHandle, State};

use crate::corpus;
use crate::error::AppError;
use crate::registry;
use crate::render;
use crate::state::AppState;
use crate::types::{
    AgentDiff, InstallRecord, InstallState, InstalledAgent, ProjectInfo, Tool, ToolInfo, ToolVersion,
    UpdateKind,
};
use crate::util::fs::{atomic_write, read_capped};

/// Cap on an installed agent file we read back during reconciliation.
const MAX_INSTALLED_BYTES: u64 = 4 * 1024 * 1024;

// ---------- Ledger persistence ----------

fn ledger_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let adir = corpus::app_data_dir(app)?;
    Ok(corpus::state_dir(&adir).join("installs.json"))
}

async fn load_ledger(app: &AppHandle) -> Result<Vec<InstallRecord>, AppError> {
    let path = ledger_path(app)?;
    match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| AppError::Io {
            message: format!("parse installs.json: {e}"),
        }),
        Err(_) => Ok(Vec::new()), // no ledger yet — nothing installed
    }
}

async fn save_ledger(app: &AppHandle, records: &[InstallRecord]) -> Result<(), AppError> {
    let path = ledger_path(app)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::Io {
            message: format!("create state dir {}: {e}", parent.display()),
        })?;
    }
    let bytes = serde_json::to_vec_pretty(records).map_err(|e| AppError::Io {
        message: format!("serialize installs.json: {e}"),
    })?;
    atomic_write(&path, &bytes).await
}

fn home() -> Result<PathBuf, AppError> {
    dirs::home_dir().ok_or_else(|| AppError::Io {
        message: "cannot resolve home directory".into(),
    })
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Where overwritten files are preserved before a write replaces them. Lives
/// under app data, NOT inside any tool's agent dir — so the Foreign sweep never
/// mistakes a backup for an installed agent. Every destructive write copies the
/// prior bytes here first, making install/update/restore reversible.
fn backups_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let adir = corpus::app_data_dir(app)?;
    Ok(adir.join("backups"))
}

/// Filesystem-safe variant of an RFC3339 timestamp (no colons).
fn fs_stamp(iso: &str) -> String {
    iso.replace([':', '/'], "-")
}

/// Build the ledger record for a render. Shared by the write path
/// (`write_agent_files`) and the no-write Track path so both agree on what a
/// row looks like.
#[allow(clippy::too_many_arguments)]
fn record_for(
    agent: &crate::types::Agent,
    primary_dest: &Path,
    tool: &str,
    project_root: Option<&Path>,
    rendered_hash: String,
    source_hash: &str,
    body_hash: &str,
    corpus_version: &str,
    installed_at: &str,
) -> InstallRecord {
    InstallRecord {
        slug: agent.slug.clone(),
        tool: tool.to_string(),
        scope: render::scope_for(project_root),
        project_path: project_root.map(|p| p.to_string_lossy().to_string()),
        dest: primary_dest.to_string_lossy().to_string(),
        source_hash: source_hash.to_string(),
        body_hash: body_hash.to_string(),
        rendered_hash,
        installed_at: installed_at.to_string(),
        corpus_version: corpus_version.to_string(),
    }
}

/// Copy `dest`'s current bytes into `backup_dir` before it's overwritten, but
/// only if it exists AND differs from the incoming bytes (no-op writes leave no
/// litter). Backup name keeps the original filename + a timestamp so it's
/// human-recoverable. Best-effort within a still-fallible signature: a failed
/// backup aborts the write (we never overwrite what we couldn't preserve).
async fn backup_if_differs(
    dest: &Path,
    new_bytes: &[u8],
    backup_dir: &Path,
    stamp: &str,
) -> Result<(), AppError> {
    let existing = match tokio::fs::read(dest).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(AppError::Io {
                message: format!("read existing file {} before backup: {e}", dest.display()),
            })
        }
    };
    if existing == new_bytes {
        return Ok(()); // identical → not a destructive write
    }
    tokio::fs::create_dir_all(backup_dir).await.map_err(|e| AppError::Io {
        message: format!("create backups dir {}: {e}", backup_dir.display()),
    })?;
    let fname = dest.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "agent".into());
    let backup = backup_dir.join(format!("{fname}.{}.bak", fs_stamp(stamp)));
    atomic_write(&backup, &existing).await
}

// ---------- Install / update (shared core) ----------

async fn do_install(
    app: &AppHandle,
    state: &AppState,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<InstallRecord, AppError> {
    let corpus = corpus::ensure_corpus(app, state).await?;
    let agent = corpus.get(&slug).ok_or_else(|| AppError::Io {
        message: format!("unknown agent: {slug}"),
    })?;
    let entry = corpus.entry(&slug).ok_or_else(|| AppError::Io {
        message: format!("no corpus-index entry for {slug}"),
    })?;
    let raw = corpus::read_source(app, &agent.category, &slug).await?;

    let home = home()?;
    let proot = project_path.as_ref().map(PathBuf::from);
    let backups = backups_dir(app)?;
    let mut ledger = load_ledger(app).await?;
    let existing_dest = ledger
        .iter()
        .find(|r| r.slug == slug && r.tool == tool && r.project_path == project_path)
        .map(|r| PathBuf::from(&r.dest));
    let record = write_agent_files_to(
        &agent,
        &raw,
        &tool,
        &home,
        proot.as_deref(),
        Some(&backups),
        &entry.source_hash,
        &entry.body_hash,
        &corpus.version(),
        &now_iso(),
        existing_dest.as_deref(),
    )
    .await?;

    ledger.retain(|r| !(r.slug == slug && r.tool == tool && r.project_path == project_path));
    ledger.push(record.clone());
    save_ledger(app, &ledger).await?;
    Ok(record)
}

/// Track a recognized on-disk agent into the ledger **without writing anything**
/// (contrast `do_install`, which renders + overwrites). We record the canonical
/// render's hash + the current corpus source/body hashes, but leave the user's
/// file exactly as it is. Reconcile then tells the truth: if the on-disk bytes
/// match the canonical render it shows `Current`; if they differ (older catalog
/// version, or hand-edited) it shows `Modified`, and an explicit Update (which
/// backs up first) reconciles it. This is the safe replacement for "Adopt".
async fn do_track(
    app: &AppHandle,
    state: &AppState,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<InstallRecord, AppError> {
    let corpus = corpus::ensure_corpus(app, state).await?;
    let agent = corpus.get(&slug).ok_or_else(|| AppError::Io {
        message: format!("unknown agent: {slug}"),
    })?;
    let entry = corpus.entry(&slug).ok_or_else(|| AppError::Io {
        message: format!("no corpus-index entry for {slug}"),
    })?;
    let raw = corpus::read_source(app, &agent.category, &slug).await?;

    let home = home()?;
    let proot = project_path.as_ref().map(PathBuf::from);
    let record = track_agent_record(
        &agent,
        &raw,
        &tool,
        &home,
        proot.as_deref(),
        &entry.source_hash,
        &entry.body_hash,
        &corpus.version(),
        &now_iso(),
    )?;

    let mut ledger = load_ledger(app).await?;
    ledger.retain(|r| !(r.slug == slug && r.tool == tool && r.project_path == project_path));
    ledger.push(record.clone());
    save_ledger(app, &ledger).await?;
    Ok(record)
}

/// Build a ledger record for Track: compute the canonical render's hash and the
/// destination, but write NOTHING. Pure (Tauri-free) so it's unit-testable
/// against a tempdir — and the test can assert no file appears.
#[allow(clippy::too_many_arguments)]
fn track_agent_record(
    agent: &crate::types::Agent,
    raw: &str,
    tool: &str,
    home: &Path,
    project_root: Option<&Path>,
    source_hash: &str,
    body_hash: &str,
    corpus_version: &str,
    installed_at: &str,
) -> Result<InstallRecord, AppError> {
    let (_bytes, rendered_hash) = render::render_with_hash(agent, raw, tool)?;
    let paths = candidate_dests(agent, raw, tool, home, project_root)?;
    let primary = paths.iter().find(|p| p.exists()).unwrap_or(&paths[0]);
    Ok(record_for(
        agent,
        primary,
        tool,
        project_root,
        rendered_hash,
        source_hash,
        body_hash,
        corpus_version,
        installed_at,
    ))
}

/// Possible physical destinations for one logical install. App-authored files
/// historically used the catalog filename slug; upstream `convert.sh` uses
/// `slugify(name)` for transform tools. Recognize both without changing the
/// catalog's stable identity.
fn candidate_dests(
    agent: &crate::types::Agent,
    raw: &str,
    tool: &str,
    home: &Path,
    project_root: Option<&Path>,
) -> Result<Vec<PathBuf>, AppError> {
    let mut paths = render::dests(tool, &agent.slug, home, project_root)?;
    let conversion_slug = render::output_slug(agent, raw, tool);
    if conversion_slug != agent.slug {
        for path in render::dests(tool, &conversion_slug, home, project_root)? {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

/// Back up divergent files, then remove every existing physical destination.
/// Backup is a separate first pass so a preservation failure cannot occur after
/// an earlier destination has already been deleted.
async fn remove_agent_files(
    agent: &crate::types::Agent,
    raw: &str,
    tool: &str,
    home: &Path,
    project_root: Option<&Path>,
    ledger_dest: Option<&Path>,
    backup_dir: &Path,
    stamp: &str,
) -> Result<(), AppError> {
    let (canonical, _) = render::render_with_hash(agent, raw, tool)?;
    let mut paths = candidate_dests(agent, raw, tool, home, project_root)?;
    if let Some(path) = ledger_dest {
        let path = path.to_path_buf();
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    let existing: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
    for (index, path) in existing.iter().enumerate() {
        let backup_stamp = format!("{stamp}-{index}");
        backup_if_differs(path, canonical.as_bytes(), backup_dir, &backup_stamp).await?;
    }
    for path in existing {
        remove_file_strict(&path).await?;
    }
    Ok(())
}

async fn remove_file_strict(path: &Path) -> Result<(), AppError> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::Io {
            message: format!("remove agent file {}: {e}", path.display()),
        }),
    }
}

/// Render + write the agent file(s) and build the ledger record. Pure of Tauri
/// (`home`/`project_root` passed explicitly) so the full render→write→record
/// path is unit-testable against a tempdir. Returns the record; caller persists
/// it to the ledger.
///
/// When `backup_dir` is `Some`, any existing dest whose bytes differ from the
/// incoming render is copied there before being overwritten — every destructive
/// write is reversible. `None` skips backups (only for callers that have already
/// guaranteed there's nothing to preserve).
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
async fn write_agent_files(
    agent: &crate::types::Agent,
    raw: &str,
    tool: &str,
    home: &Path,
    project_root: Option<&Path>,
    backup_dir: Option<&Path>,
    source_hash: &str,
    body_hash: &str,
    corpus_version: &str,
    installed_at: &str,
) -> Result<InstallRecord, AppError> {
    write_agent_files_to(
        agent,
        raw,
        tool,
        home,
        project_root,
        backup_dir,
        source_hash,
        body_hash,
        corpus_version,
        installed_at,
        None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn write_agent_files_to(
    agent: &crate::types::Agent,
    raw: &str,
    tool: &str,
    home: &Path,
    project_root: Option<&Path>,
    backup_dir: Option<&Path>,
    source_hash: &str,
    body_hash: &str,
    corpus_version: &str,
    installed_at: &str,
    preferred_dest: Option<&Path>,
) -> Result<InstallRecord, AppError> {
    let (bytes, rendered_hash) = render::render_with_hash(agent, raw, tool)?;
    let mut paths = render::dests(tool, &agent.slug, home, project_root)?;
    if let Some(preferred) = preferred_dest {
        if paths.len() == 1 {
            paths[0] = preferred.to_path_buf();
        } else if let Some(index) = paths.iter().position(|p| p == preferred) {
            paths.swap(0, index);
        }
    }
    for dest in &paths {
        if let Some(bdir) = backup_dir {
            backup_if_differs(dest, bytes.as_bytes(), bdir, installed_at).await?;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| AppError::Io {
                message: format!("create {}: {e}", parent.display()),
            })?;
        }
        atomic_write(dest, bytes.as_bytes()).await?;
    }
    Ok(record_for(
        agent,
        &paths[0],
        tool,
        project_root,
        rendered_hash,
        source_hash,
        body_hash,
        corpus_version,
        installed_at,
    ))
}

// ---------- Reconciliation core (pure, testable) ----------

/// Classify one ledger row given what's on disk now and the current corpus
/// source hash for that slug. `disk` is `None` when the file is gone, else the
/// SHA-256 of its current bytes. See systemPatterns.md §4.
fn classify(
    disk: Option<&str>,
    rendered_hash: &str,
    record_source: &str,
    corpus_source: Option<&str>,
) -> InstallState {
    match disk {
        None => InstallState::Removed,
        Some(h) if h != rendered_hash => InstallState::Modified,
        Some(_) => match corpus_source {
            Some(cs) if cs == record_source => InstallState::Current,
            Some(_) => InstallState::Outdated,
            // Agent no longer in the corpus (e.g. removed upstream): the file
            // still matches what we wrote, so treat it as current, not stale.
            None => InstallState::Current,
        },
    }
}

/// True if `file_bytes` are byte-identical to the canonical render of `agent`
/// for `tool`. Pure (no I/O) so it's unit-testable. When they match, the file
/// on disk IS this agent verbatim — there's nothing to "adopt"; reconcile can
/// treat it as `Current` even if we didn't install it.
fn bytes_match_render(agent: &crate::types::Agent, raw: &str, tool: &str, file_bytes: &[u8]) -> bool {
    match render::render_with_hash(agent, raw, tool) {
        Ok((_, expected)) => render::sha256_hex(file_bytes) == expected,
        Err(_) => false,
    }
}

/// I/O wrapper for [`bytes_match_render`]: reads the agent's canonical source +
/// the on-disk file and compares. Returns `false` on any read/render failure
/// (can't prove identity ⇒ don't auto-claim it).
async fn is_canonical_on_disk(
    app: &AppHandle,
    agent: &crate::types::Agent,
    tool: &str,
    path: &Path,
) -> bool {
    let Ok(raw) = corpus::read_source(app, &agent.category, &agent.slug).await else {
        return false;
    };
    let Ok(bytes) = read_capped(path, MAX_INSTALLED_BYTES).await else {
        return false;
    };
    bytes_match_render(agent, &raw, tool, &bytes)
}

// ---------- Tool detection ----------

fn detect(tool: &str, home: &Path) -> (bool, Option<String>) {
    // Registry-driven: detected if ANY of the tool's `detect.dirs` exists under
    // `home`; the agents dir comes from `detect.agentsDir`. Recognized-only tools
    // (no `detect` block) → (false, None).
    let Some(det) = registry::get(tool).and_then(|m| m.detect.as_ref()) else {
        return (false, None);
    };
    let detected = det.dirs.iter().any(|d| home.join(d).exists());
    let agents_dir = det
        .agents_dir
        .as_ref()
        .map(|sub| home.join(sub).to_string_lossy().to_string());
    (detected, agents_dir)
}

/// The tools Phase 2 can install to — the wired (installable) registry ids, in
/// registry order. Sourced from the embedded JSON so adding a tool is adding a
/// file, not editing this list.
fn supported() -> Vec<&'static str> {
    registry::wired().map(|m| m.id.as_str()).collect()
}

// ---------- Tauri commands ----------

/// Install (or re-install) `slug` into `tool`. For project-scoped tools pass
/// the project root in `project_path`. Returns the ledger record.
#[tauri::command]
pub async fn install_agent(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<InstallRecord, AppError> {
    do_install(&app, &state, slug, tool, project_path).await
}

/// Update an install to the current corpus version (re-render + write). The
/// prior file is backed up first (see `do_install`), so an Update applied to a
/// Modified file preserves the user's edits in `backups/` before restoring the
/// canonical render. Separate command from install for intent + UX.
#[tauri::command]
pub async fn update_agent(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<InstallRecord, AppError> {
    do_install(&app, &state, slug, tool, project_path).await
}

/// Track a recognized Foreign install into the ledger **non-destructively** —
/// we record provenance but never write to the user's file. This is the safe
/// replacement for the old "Adopt" (which overwrote the on-disk file). After
/// tracking, reconcile shows `Current` if the file already matches the canonical
/// render, or `Modified` if it differs (then an explicit Update reconciles it).
#[tauri::command]
pub async fn track_agent(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<InstallRecord, AppError> {
    do_track(&app, &state, slug, tool, project_path).await
}

/// Diff what's on disk against the canonical render the app would write — powers
/// "review before Update" without touching any file.
#[tauri::command]
pub async fn agent_diff(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<AgentDiff, AppError> {
    let corpus = corpus::ensure_corpus(&app, &state).await?;
    let agent = corpus.get(&slug).ok_or_else(|| AppError::Io {
        message: format!("unknown agent: {slug}"),
    })?;
    let raw = corpus::read_source(&app, &agent.category, &slug).await?;
    let (proposed, _hash) = render::render_with_hash(&agent, &raw, &tool)?;

    let home = home()?;
    let proot = project_path.as_ref().map(PathBuf::from);
    let ledger = load_ledger(&app).await?;
    let ledger_dest = ledger
        .iter()
        .find(|r| r.slug == slug && r.tool == tool && r.project_path == project_path)
        .map(|r| PathBuf::from(&r.dest));
    let candidates = candidate_dests(&agent, &raw, &tool, &home, proot.as_deref())?;
    let dest = ledger_dest
        .as_ref()
        .or_else(|| candidates.iter().find(|p| p.exists()))
        .unwrap_or(&candidates[0]);
    let on_disk = match read_capped(dest, MAX_INSTALLED_BYTES).await {
        Ok(b) => Some(String::from_utf8_lossy(&b).into_owned()),
        Err(_) => None,
    };
    let differs = on_disk.as_deref() != Some(proposed.as_str());
    Ok(AgentDiff {
        slug,
        tool,
        project_path,
        dest: dest.to_string_lossy().to_string(),
        on_disk,
        proposed,
        differs,
    })
}

/// Uninstall: remove the written file(s) and the ledger row.
#[tauri::command]
pub async fn uninstall_agent(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    tool: Tool,
    project_path: Option<String>,
) -> Result<(), AppError> {
    let corpus = corpus::ensure_corpus(&app, &state).await?;
    let agent = corpus.get(&slug).ok_or_else(|| AppError::Io {
        message: format!("unknown agent: {slug}"),
    })?;
    let raw = corpus::read_source(&app, &agent.category, &slug).await?;
    let home = home()?;
    let proot = project_path.as_ref().map(PathBuf::from);
    let mut ledger = load_ledger(&app).await?;
    let ledger_dest = ledger
        .iter()
        .find(|r| r.slug == slug && r.tool == tool && r.project_path == project_path)
        .map(|r| PathBuf::from(&r.dest));
    remove_agent_files(
        &agent,
        &raw,
        &tool,
        &home,
        proot.as_deref(),
        ledger_dest.as_deref(),
        &backups_dir(&app)?,
        &now_iso(),
    )
    .await?;
    ledger.retain(|r| !(r.slug == slug && r.tool == tool && r.project_path == project_path));
    save_ledger(&app, &ledger).await?;
    Ok(())
}

/// The reconciled Library view — every ledger row resolved against disk +
/// corpus into one of the 5 states.
#[tauri::command]
pub async fn installs_reconcile(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<InstalledAgent>, AppError> {
    let corpus = corpus::ensure_corpus(&app, &state).await?;
    let ledger = load_ledger(&app).await?;
    let mut out = Vec::with_capacity(ledger.len());
    for r in &ledger {
        let dest = PathBuf::from(&r.dest);
        let disk_hash = if dest.exists() {
            read_capped(&dest, MAX_INSTALLED_BYTES)
                .await
                .ok()
                .map(|b| render::sha256_hex(&b))
        } else {
            None
        };
        let centry = corpus.entry(&r.slug);
        let corpus_source = centry.as_ref().map(|e| e.source_hash.as_str());
        let st = classify(disk_hash.as_deref(), &r.rendered_hash, &r.source_hash, corpus_source);
        // Cosmetic vs substantive: only meaningful when Outdated. Body unchanged
        // upstream → the update is metadata-only.
        let update_kind = if st == InstallState::Outdated {
            let cur_body = centry.as_ref().map(|e| e.body_hash.as_str());
            Some(if cur_body == Some(r.body_hash.as_str()) {
                UpdateKind::Cosmetic
            } else {
                UpdateKind::Substantive
            })
        } else {
            None
        };
        let name = corpus.get(&r.slug).map(|a| a.name).unwrap_or_else(|| r.slug.clone());
        out.push(InstalledAgent {
            slug: r.slug.clone(),
            name,
            tool: r.tool.clone(),
            scope: r.scope,
            project_path: r.project_path.clone(),
            dest: r.dest.clone(),
            state: st,
            update_kind,
            tracked: true,
        });
    }

    // Foreign sweep: files on disk we did NOT install but recognize as corpus
    // agents (slug matches a known agent). A file that is BYTE-IDENTICAL to the
    // canonical render IS that agent, verbatim — installed outside the app (e.g.
    // the CLI install.sh), but in sync. We surface it as `Current` (nothing to
    // decide). Only a recognized-but-DIFFERENT file (older version, or
    // hand-edited) stays `Foreign` and asks for a look. Scans each supported
    // tool's dir(s) — user dirs + every project dir in the ledger.
    let ledger_keys: std::collections::HashSet<(String, Tool, Option<String>)> = ledger
        .iter()
        .map(|r| (r.slug.clone(), r.tool.clone(), r.project_path.clone()))
        .collect();
    let project_dirs: Vec<PathBuf> = ledger
        .iter()
        .filter_map(|r| r.project_path.as_ref())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let home = home()?;
    for tool in supported() {
        // Some tools namespace the output slug (e.g. Osaurus dirs are
        // `agency-<slug>`); strip it before recognizing the agent.
        let prefix = crate::registry::get(tool)
            .and_then(|m| m.slug_prefix.as_deref())
            .unwrap_or("");
        // Dual-scope tools are scanned in BOTH places: the user-global dir (key
        // None) AND every project root the ledger knows about (key Some(path)).
        // Each entry: (scope-key, agents-root, suffix-after-`{slug}`).
        let mut scan_roots: Vec<(Option<String>, PathBuf, String)> = Vec::new();
        if render::supports_user(tool) {
            scan_roots
                .extend(agent_units(tool, &home, None).into_iter().map(|(d, s)| (None, d, s)));
        }
        if render::supports_project(tool) {
            scan_roots.extend(project_dirs.iter().flat_map(|p| {
                let key = Some(p.to_string_lossy().to_string());
                agent_units(tool, &home, Some(p)).into_iter().map(move |(d, s)| (key.clone(), d, s))
            }));
        }
        for (proj, agents_root, suffix) in scan_roots {
            let mut rd = match tokio::fs::read_dir(&agents_root).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            // A leading `/` in the suffix means the per-agent unit is a DIRECTORY
            // (e.g. `{slug}/SKILL.md`); otherwise it's a file (`{slug}.md`).
            let dir_unit = suffix.starts_with('/');
            while let Ok(Some(ent)) = rd.next_entry().await {
                let name = ent.file_name();
                let Some(name) = name.to_str() else { continue };
                // Recover the `{slug}` token + the file that holds the canonical
                // bytes. Dir unit: the entry IS the slug dir, bytes at <dir>/<leaf>.
                // File unit: the entry is `<slug><suffix>`.
                let (token, byte_path) = if dir_unit {
                    (name.to_string(), agents_root.join(name).join(suffix.trim_start_matches('/')))
                } else if name.ends_with(suffix.as_str()) && name.len() > suffix.len() {
                    (name[..name.len() - suffix.len()].to_string(), agents_root.join(name))
                } else {
                    continue; // not a unit for this template (stray file/dir)
                };
                let cand = token.strip_prefix(prefix).unwrap_or(&token);
                let Some(agent) = corpus.get(cand).or_else(|| corpus.get_by_conversion_slug(cand))
                else {
                    continue; // unrecognized → not ours to claim
                };
                let slug = agent.slug.clone();
                if ledger_keys.contains(&(slug.clone(), tool.to_string(), proj.clone())) {
                    continue; // already in the ledger
                }
                // Byte-identical to the catalog ⇒ in sync ⇒ Current. Otherwise a
                // recognized-but-divergent file ⇒ Foreign (worth a look).
                let state = if is_canonical_on_disk(&app, &agent, tool, &byte_path).await {
                    InstallState::Current
                } else {
                    InstallState::Foreign
                };
                out.push(InstalledAgent {
                    slug,
                    name: agent.name.clone(),
                    tool: tool.to_string(),
                    scope: render::scope_for(proj.as_deref().map(std::path::Path::new)),
                    project_path: proj.clone(),
                    dest: byte_path.to_string_lossy().to_string(),
                    state,
                    update_kind: None,
                    tracked: false,
                });
            }
        }
    }

    // Collapse to one row per LOGICAL install (slug, tool, project). Copilot
    // dual-writes to ~/.github and ~/.copilot, so the Foreign sweep finds the
    // same agent twice; other tools could too. One logical install = one row
    // (its Track/Update/Remove already cover every physical dest).
    let mut seen = std::collections::HashSet::new();
    out.retain(|a| seen.insert((a.slug.clone(), a.tool.clone(), a.project_path.clone())));

    Ok(out)
}

/// For the Foreign sweep: each scannable agents-root for a tool, paired with the
/// path suffix that follows `{slug}` in the dest template. The suffix tells the
/// sweep whether a per-agent UNIT is a file (e.g. `.md`) or a directory (e.g.
/// `/SKILL.md`), and where the canonical bytes live inside a dir unit.
///
/// Splitting the TEMPLATE (not a `{slug}`-substituted path) is what makes
/// dir-structured tools work: Osaurus's `.osaurus/skills/{slug}/SKILL.md` scans
/// `.osaurus/skills` instead of a bogus `.osaurus/skills/_probe`.
fn agent_units(tool: &str, home: &Path, project_root: Option<&Path>) -> Vec<(PathBuf, String)> {
    let Some(meta) = crate::registry::get(tool) else {
        return Vec::new();
    };
    let Some(dest) = meta.dest.as_ref() else {
        return Vec::new();
    };
    let (templates, root): (&[String], &Path) = match project_root {
        Some(p) => (&dest.project, p),
        None => (&dest.user, home),
    };
    templates
        .iter()
        .filter_map(|t| {
            t.split_once("{slug}")
                .map(|(before, after)| (root.join(before), after.to_string()))
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// All install records that match a given agent (for the persona detail panel).
#[tauri::command]
pub async fn installs_for_agent(
    app: AppHandle,
    slug: String,
) -> Result<Vec<InstallRecord>, AppError> {
    let ledger = load_ledger(&app).await?;
    Ok(ledger.into_iter().filter(|r| r.slug == slug).collect())
}

/// Detected AI tools + their deployment surface and installed counts.
#[tauri::command]
pub async fn tools_list(app: AppHandle) -> Result<Vec<ToolInfo>, AppError> {
    let ledger = load_ledger(&app).await?;
    let home = home()?;
    let supported = supported();
    let mut out = Vec::with_capacity(supported.len());
    for tool in supported {
        let installed_count = ledger.iter().filter(|r| r.tool == tool).count() as u32;
        let (detected, user_dest) = detect(tool, &home);
        out.push(ToolInfo {
            tool: tool.to_string(),
            label: render::label(tool),
            detected,
            // Primary/display scope: dual-scope tools read "user" (global-first);
            // Cursor is the project-only exception. Per-install scope is derived
            // from the chosen project root, not this field.
            scope: if render::supports_user(tool) {
                crate::types::Scope::User
            } else {
                crate::types::Scope::Project
            },
            user_dest,
            installed_count,
        });
    }
    Ok(out)
}

/// Open a path in the OS file manager (Finder / Explorer / xdg-open).
/// Best-effort: returns an error the UI can toast if the path is missing or no
/// opener is available. Used by the Tools panel's "Reveal" affordance.
#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), AppError> {
    tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        let program = "open";
        #[cfg(target_os = "windows")]
        let program = "explorer";
        #[cfg(all(unix, not(target_os = "macos")))]
        let program = "xdg-open";
        std::process::Command::new(program)
            .arg(&path)
            .status()
            .map(|_| ())
            .map_err(|e| AppError::Io {
                message: format!("could not open {path}: {e}"),
            })
    })
    .await
    .map_err(|e| AppError::Io {
        message: e.to_string(),
    })?
}

/// The `<bin> --version`-style probe command for a tool, or `None` when we don't
/// know one. Best-effort and uneven by nature — GUI tools may not ship a CLI.
fn version_cmd(tool: &str) -> Option<(&'static str, Vec<&'static str>)> {
    // The registry is cached for the process lifetime (`OnceLock`), so its
    // `&str`s are effectively `'static` — fine to hand to the version probe.
    let v = registry::get(tool)?.version.as_ref()?;
    Some((v.bin.as_str(), v.args.iter().map(String::as_str).collect()))
}

/// First non-empty trimmed line of version output, capped to a sane length.
fn first_version_line(s: &str) -> Option<String> {
    s.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| {
        let capped: String = l.chars().take(48).collect();
        capped
    })
}

async fn probe_version(tool: &str) -> Option<String> {
    let (bin, args) = version_cmd(tool)?;
    let fut = tokio::process::Command::new(bin).args(args).output();
    match tokio::time::timeout(std::time::Duration::from_secs(3), fut).await {
        Ok(Ok(o)) if o.status.success() => first_version_line(&String::from_utf8_lossy(&o.stdout))
            .or_else(|| first_version_line(&String::from_utf8_lossy(&o.stderr))),
        _ => None,
    }
}

/// Best-effort version probe across all supported tools, run concurrently with a
/// per-tool timeout. A tool whose binary isn't on PATH (or that has no known
/// version command) comes back as `version: None` — the UI just omits it.
#[tauri::command]
pub async fn tool_versions() -> Result<Vec<ToolVersion>, AppError> {
    let supported = supported();
    let mut handles = Vec::with_capacity(supported.len());
    for tool in supported {
        handles.push(tokio::spawn(
            async move { ToolVersion {
                tool: tool.to_string(),
                version: probe_version(tool).await,
            } },
        ));
    }
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(v) = h.await {
            out.push(v);
        }
    }
    Ok(out)
}

/// Project directories we've installed project-scoped agents into.
#[tauri::command]
pub async fn projects_list(app: AppHandle) -> Result<Vec<ProjectInfo>, AppError> {
    let ledger = load_ledger(&app).await?;
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for r in &ledger {
        if let Some(p) = &r.project_path {
            *counts.entry(p.clone()).or_default() += 1;
        }
    }
    Ok(counts
        .into_iter()
        .map(|(path, installed_count)| {
            let label = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            ProjectInfo { path, label, installed_count }
        })
        .collect())
}

// ---------- Loadouts (Agentfile) ----------

/// Portable manifest of an install set — "set up a new Mac in one click".
/// JSON so it's diffable + shareable; `tool` uses the camelCase wire value.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Agentfile {
    /// Format version.
    agentfile: u32,
    installs: Vec<LoadoutEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoadoutEntry {
    slug: String,
    tool: Tool,
    #[serde(default)]
    project_path: Option<String>,
}

/// Export the current ledger as an Agentfile written to `path`. Returns count.
#[tauri::command]
pub async fn loadout_export(app: AppHandle, path: String) -> Result<u32, AppError> {
    let ledger = load_ledger(&app).await?;
    let installs: Vec<LoadoutEntry> = ledger
        .iter()
        .map(|r| LoadoutEntry {
            slug: r.slug.clone(),
            tool: r.tool.clone(),
            project_path: r.project_path.clone(),
        })
        .collect();
    let n = installs.len() as u32;
    let af = Agentfile { agentfile: 1, installs };
    let bytes = serde_json::to_vec_pretty(&af).map_err(|e| AppError::Io {
        message: format!("serialize Agentfile: {e}"),
    })?;
    atomic_write(Path::new(&path), &bytes).await?;
    Ok(n)
}

/// Import an Agentfile from `path`, installing every entry. Returns the records
/// that installed successfully (entries that fail — e.g. a project tool whose
/// path no longer exists — are skipped, not fatal).
#[tauri::command]
pub async fn loadout_import(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<InstallRecord>, AppError> {
    let bytes = read_capped(Path::new(&path), MAX_INSTALLED_BYTES).await?;
    let af: Agentfile = serde_json::from_slice(&bytes).map_err(|e| AppError::Io {
        message: format!("parse Agentfile: {e}"),
    })?;
    let mut out = Vec::with_capacity(af.installs.len());
    for e in af.installs {
        if let Ok(rec) = do_install(&app, &state, e.slug, e.tool, e.project_path).await {
            out.push(rec);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agentfile_roundtrips() {
        let af = Agentfile {
            agentfile: 1,
            installs: vec![
                LoadoutEntry { slug: "a".into(), tool: "claudeCode".to_string(), project_path: None },
                LoadoutEntry {
                    slug: "b".into(),
                    tool: "cursor".to_string(),
                    project_path: Some("/proj".into()),
                },
            ],
        };
        let bytes = serde_json::to_vec(&af).unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("\"claudeCode\"") && s.contains("\"projectPath\":\"/proj\""));
        let back: Agentfile = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.installs.len(), 2);
        assert_eq!(back.installs[1].tool, "cursor");
    }

    #[test]
    fn classify_states() {
        // file gone
        assert_eq!(classify(None, "r", "s1", Some("s1")), InstallState::Removed);
        // bytes differ from what we wrote → user-edited
        assert_eq!(classify(Some("x"), "r", "s1", Some("s1")), InstallState::Modified);
        // matches our render, corpus unchanged → current
        assert_eq!(classify(Some("r"), "r", "s1", Some("s1")), InstallState::Current);
        // matches our render, corpus advanced → outdated
        assert_eq!(classify(Some("r"), "r", "s1", Some("s2")), InstallState::Outdated);
        // agent gone from corpus but file intact → current
        assert_eq!(classify(Some("r"), "r", "s1", None), InstallState::Current);
    }

    #[test]
    fn agent_units_handles_file_and_dir_tools() {
        let home = std::path::Path::new("/home/u");
        // File-per-agent (Claude): root = ~/.claude/agents, suffix = ".md".
        let claude = agent_units("claudeCode", home, None);
        assert!(
            claude.iter().any(|(d, s)| d.ends_with(".claude/agents") && s == ".md"),
            "claude: {claude:?}"
        );
        // Dir-per-agent (Osaurus): the bug was scanning `.osaurus/skills/_probe`.
        // It must scan `.osaurus/skills` with a `/SKILL.md` leaf.
        let osa = agent_units("osaurus", home, None);
        assert_eq!(osa.len(), 1, "osaurus: {osa:?}");
        assert!(osa[0].0.ends_with(".osaurus/skills"), "osaurus dir: {:?}", osa[0].0);
        assert_eq!(osa[0].1, "/SKILL.md");
    }

    fn sample_agent() -> crate::types::Agent {
        crate::types::Agent {
            slug: "frontend-developer".into(),
            name: "Frontend Developer".into(),
            description: "Builds UIs.".into(),
            category: "engineering".into(),
            emoji: None,
            color: Some("blue".into()),
            vibe: None,
            body: "You are a frontend dev.\n".into(),
        }
    }

    /// Full render → write-to-disk → reconcile loop against a tempdir "home".
    #[tokio::test]
    async fn install_writes_then_reconciles_through_states() {
        let home = tempfile::tempdir().unwrap();
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\n---\nORIGINAL\n";

        // Codex (user-scoped, TOML transform).
        let rec = write_agent_files(
            &agent, raw, "codex", home.path(), None, None, "src-1", "body-1", "v1",
            "2026-06-05T00:00:00Z",
        )
        .await
        .unwrap();

        let path = home.path().join(".codex").join("agents").join("frontend-developer.toml");
        assert!(path.exists(), "install wrote the file");
        let on_disk = std::fs::read(&path).unwrap();
        let disk_hash = render::sha256_hex(&on_disk);
        assert_eq!(disk_hash, rec.rendered_hash, "on-disk bytes match recorded render");

        // Reconcile classifications off the real bytes:
        assert_eq!(
            classify(Some(&disk_hash), &rec.rendered_hash, &rec.source_hash, Some("src-1")),
            InstallState::Current
        );
        assert_eq!(
            classify(Some(&disk_hash), &rec.rendered_hash, &rec.source_hash, Some("src-2")),
            InstallState::Outdated
        );
        assert_eq!(
            classify(Some("useredited"), &rec.rendered_hash, &rec.source_hash, Some("src-1")),
            InstallState::Modified
        );
        // delete → Removed
        std::fs::remove_file(&path).unwrap();
        let gone = if path.exists() { Some(disk_hash.as_str()) } else { None };
        assert_eq!(
            classify(gone, &rec.rendered_hash, &rec.source_hash, Some("src-1")),
            InstallState::Removed
        );
    }

    #[tokio::test]
    async fn claude_code_writes_raw_verbatim() {
        let home = tempfile::tempdir().unwrap();
        let raw = "---\nname: Frontend Developer\ncolor: blue\n---\nVERBATIM BODY\n";
        write_agent_files(
            &sample_agent(), raw, "claudeCode", home.path(), None, None, "s", "b", "v", "t",
        )
        .await
        .unwrap();
        let got = std::fs::read_to_string(home.path().join(".claude/agents/frontend-developer.md")).unwrap();
        assert_eq!(got, raw, "identity tool ships the source unchanged");
    }

    #[tokio::test]
    async fn project_tool_writes_into_project_root() {
        let home = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        let rec = write_agent_files(
            &sample_agent(), "raw", "cursor", home.path(), Some(proj.path()), None, "s", "b", "v", "t",
        )
        .await
        .unwrap();
        assert!(proj.path().join(".cursor/rules/frontend-developer.mdc").exists());
        assert_eq!(rec.project_path.as_deref(), Some(proj.path().to_string_lossy().as_ref()));
        assert_eq!(rec.scope, crate::types::Scope::Project);
    }

    /// A file byte-identical to the canonical render is recognized as in-sync
    /// (so the Foreign sweep can call it Current); any difference is not.
    #[test]
    fn canonical_render_is_recognized_byte_for_byte() {
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\ncolor: blue\n---\nBODY\n";
        // The exact canonical render matches…
        let (rendered, _h) = render::render_with_hash(&agent, raw, "codex").unwrap();
        assert!(bytes_match_render(&agent, raw, "codex", rendered.as_bytes()));
        // …a hand-edited / different file does not.
        assert!(!bytes_match_render(&agent, raw, "codex", b"different bytes"));
        // Identity tool (claude-code ships the source verbatim) also matches.
        let (raw_render, _h2) = render::render_with_hash(&agent, raw, "claudeCode").unwrap();
        assert!(bytes_match_render(&agent, raw, "claudeCode", raw_render.as_bytes()));
    }

    /// Track records provenance but must NOT create or touch any file.
    #[tokio::test]
    async fn track_writes_no_file() {
        let home = tempfile::tempdir().unwrap();
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\n---\nBODY\n";

        let rec = track_agent_record(
            &agent, raw, "codex", home.path(), None, "src-1", "body-1", "v1",
            "2026-06-06T00:00:00Z",
        )
        .unwrap();

        let path = home.path().join(".codex/agents").join("frontend-developer.toml");
        assert!(!path.exists(), "Track must not write the agent file");
        assert_eq!(std::path::Path::new(&rec.dest), path.as_path(), "record points at the canonical dest");

        // The recorded rendered_hash equals a real render — so if the user's file
        // happens to match it, reconcile yields Current; otherwise Modified.
        let (_b, render_hash) = render::render_with_hash(&agent, raw, "codex").unwrap();
        assert_eq!(rec.rendered_hash, render_hash);
        assert_eq!(
            classify(Some(&render_hash), &rec.rendered_hash, &rec.source_hash, Some("src-1")),
            InstallState::Current,
            "a tracked file that matches the canonical render reconciles as Current"
        );
        assert_eq!(
            classify(Some("hand-edited"), &rec.rendered_hash, &rec.source_hash, Some("src-1")),
            InstallState::Modified,
            "a tracked file that differs reconciles as Modified (never silently clobbered)"
        );
    }

    #[tokio::test]
    async fn tracked_conversion_slug_update_reuses_existing_destination() {
        let home = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let mut agent = sample_agent();
        agent.slug = "engineering-frontend-developer".into();
        let raw = "---\nname: Frontend Developer\ndescription: Builds UIs.\n---\nBODY\n";
        let conversion_dest = home.path().join(".codex/agents").join("frontend-developer.toml");
        std::fs::create_dir_all(conversion_dest.parent().unwrap()).unwrap();
        std::fs::write(&conversion_dest, b"OLDER CLI OUTPUT").unwrap();

        let tracked = track_agent_record(
            &agent,
            raw,
            "codex",
            home.path(),
            None,
            "src-1",
            "body-1",
            "v1",
            "2026-06-12T00:00:00Z",
        )
        .unwrap();
        assert_eq!(std::path::Path::new(&tracked.dest), conversion_dest.as_path());

        write_agent_files_to(
            &agent,
            raw,
            "codex",
            home.path(),
            None,
            Some(backups.path()),
            "src-2",
            "body-2",
            "v2",
            "2026-06-12T01:00:00Z",
            Some(&conversion_dest),
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&conversion_dest).unwrap(),
            render::render(&agent, raw, "codex").unwrap()
        );
        assert!(
            !home
                .path()
                .join(".codex/agents/engineering-frontend-developer.toml")
                .exists(),
            "update must not create a duplicate source-slug file"
        );
    }

    /// A write that overwrites an existing, DIFFERENT file must preserve the old
    /// bytes in the backups dir first; an identical (no-op) write must not.
    #[tokio::test]
    async fn write_backs_up_existing_differing_file() {
        let home = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let agent = sample_agent();
        let dest = home.path().join(".codex/agents/frontend-developer.toml");

        // Simulate a user-edited file already on disk at the dest.
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"USER EDITED CONTENT").unwrap();

        // Update over it (with backups enabled).
        write_agent_files(
            &agent, "---\nname: Frontend Developer\n---\nNEW\n", "codex", home.path(),
            None, Some(backups.path()), "src-2", "body-2", "v2", "2026-06-06T01:02:03Z",
        )
        .await
        .unwrap();

        // The old bytes were preserved before the overwrite.
        let saved: Vec<_> = std::fs::read_dir(backups.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| std::fs::read(e.path()).unwrap())
            .collect();
        assert_eq!(saved.len(), 1, "exactly one backup created");
        assert_eq!(saved[0], b"USER EDITED CONTENT", "backup holds the pre-overwrite bytes");

        // A second, byte-identical write makes no new backup (not destructive).
        let before = std::fs::read(&dest).unwrap();
        write_agent_files(
            &agent, "---\nname: Frontend Developer\n---\nNEW\n", "codex", home.path(),
            None, Some(backups.path()), "src-2", "body-2", "v2", "2026-06-06T02:02:03Z",
        )
        .await
        .unwrap();
        let after = std::fs::read(&dest).unwrap();
        assert_eq!(before, after, "identical render leaves the file unchanged");
        let count = std::fs::read_dir(backups.path()).unwrap().count();
        assert_eq!(count, 1, "no-op write adds no backup");
    }

    #[tokio::test]
    async fn uninstall_canonical_file_needs_no_backup() {
        let home = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\ndescription: Builds UIs.\n---\nBODY\n";
        let dest = home.path().join(".codex/agents/frontend-developer.toml");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, render::render(&agent, raw, "codex").unwrap()).unwrap();

        remove_agent_files(
            &agent,
            raw,
            "codex",
            home.path(),
            None,
            None,
            backups.path(),
            "2026-06-12T00:00:00Z",
        )
        .await
        .unwrap();

        assert!(!dest.exists());
        assert_eq!(std::fs::read_dir(backups.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn uninstall_modified_file_backs_up_before_delete() {
        let home = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\ndescription: Builds UIs.\n---\nBODY\n";
        let dest = home.path().join(".codex/agents/frontend-developer.toml");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"USER MODIFIED").unwrap();

        remove_agent_files(
            &agent,
            raw,
            "codex",
            home.path(),
            None,
            None,
            backups.path(),
            "2026-06-12T00:00:00Z",
        )
        .await
        .unwrap();

        assert!(!dest.exists());
        let saved: Vec<_> = std::fs::read_dir(backups.path())
            .unwrap()
            .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
            .collect();
        assert_eq!(saved, vec![b"USER MODIFIED".to_vec()]);
    }

    #[tokio::test]
    async fn uninstall_missing_file_is_successful() {
        let home = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        remove_agent_files(
            &sample_agent(),
            "---\nname: Frontend Developer\n---\nBODY\n",
            "codex",
            home.path(),
            None,
            None,
            backups.path(),
            "2026-06-12T00:00:00Z",
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn uninstall_copilot_removes_both_destinations() {
        let home = tempfile::tempdir().unwrap();
        let backups = tempfile::tempdir().unwrap();
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\n---\nBODY\n";
        for dest in render::dests("copilot", &agent.slug, home.path(), None).unwrap() {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::write(dest, raw).unwrap();
        }

        remove_agent_files(
            &agent,
            raw,
            "copilot",
            home.path(),
            None,
            None,
            backups.path(),
            "2026-06-12T00:00:00Z",
        )
        .await
        .unwrap();

        for dest in render::dests("copilot", &agent.slug, home.path(), None).unwrap() {
            assert!(!dest.exists());
        }
    }

    #[tokio::test]
    async fn uninstall_backup_failure_preserves_original() {
        let home = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let backup_path = scratch.path().join("not-a-directory");
        std::fs::write(&backup_path, b"occupied").unwrap();
        let agent = sample_agent();
        let raw = "---\nname: Frontend Developer\n---\nBODY\n";
        let dest = home.path().join(".codex/agents/frontend-developer.toml");
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, b"USER MODIFIED").unwrap();

        assert!(
            remove_agent_files(
                &agent,
                raw,
                "codex",
                home.path(),
                None,
                None,
                &backup_path,
                "2026-06-12T00:00:00Z",
            )
            .await
            .is_err()
        );
        assert_eq!(std::fs::read(&dest).unwrap(), b"USER MODIFIED");
    }

    #[tokio::test]
    async fn uninstall_removal_failure_is_reported() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join("directory");
        std::fs::create_dir(&directory).unwrap();
        assert!(remove_file_strict(&directory).await.is_err());
        assert!(directory.exists());
    }

    #[test]
    fn ledger_json_roundtrips() {
        let recs = vec![InstallRecord {
            slug: "a".into(),
            tool: "cursor".to_string(),
            scope: crate::types::Scope::Project,
            project_path: Some("/p".into()),
            dest: "/p/.cursor/rules/a.mdc".into(),
            source_hash: "sh".into(),
            body_hash: "bh".into(),
            rendered_hash: "rh".into(),
            installed_at: "2026-06-05T00:00:00Z".into(),
            corpus_version: "v".into(),
        }];
        let bytes = serde_json::to_vec(&recs).unwrap();
        // tool serializes camelCase per the wire contract.
        assert!(String::from_utf8_lossy(&bytes).contains("\"cursor\""));
        let back: Vec<InstallRecord> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].tool, "cursor");
    }
}
