//! Shared DTOs serialized across the Tauri IPC boundary.
//!
//! Every struct uses `#[serde(rename_all = "camelCase")]` so the
//! TypeScript side matches `src/lib/types.ts` exactly.

use serde::{Deserialize, Serialize};

// =========================================================
// Agency Agents — corpus subsystem (contracts.md §A)
// =========================================================
//
// Wire format mirrors `src/lib/types.ts`.

// ---------- Tools & scope ----------

/// An AI coding tool we can deploy an agent into. The 11 variants are
/// the authoritative install-target set from agency-agents'
/// `scripts/install.sh` (see contracts.md §B). Serialized as
/// camelCase strings (e.g. `"claudeCode"`, `"geminiCli"`) so the TS
/// `Tool` union matches exactly.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    ClaudeCode,
    Copilot,
    Cursor,
    GeminiCli,
    Codex,
    Opencode,
    Windsurf,
    Aider,
    Qwen,
    Openclaw,
    Antigravity,
}

/// Deployment scope. User-global tools write to fixed `~/…` dests;
/// project-scoped tools install into a tracked `project_path`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    User,
    Project,
}

// ---------- Catalog source (where the corpus lives) ----------

/// Where the active agent catalog lives on disk. The whole app reads/writes the
/// resolved root, so this is the one knob that says "be a respectful frontend
/// over the user's clone" vs "manage our own copy." Persisted to
/// `state/catalog.json`. Serialized tagged on `kind` so the TS side is a clean
/// discriminated union.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CatalogSource {
    /// App-managed copy seeded from the bundled baseline (`<app_data>/corpus`).
    /// The always-works default; never touches anything outside app data.
    Bundled,
    /// A clone the app provisioned and owns (default `~/.agency-agents`). The
    /// app may pull/refresh it; it's shared with the CLI.
    Managed { path: String },
    /// The user's own pre-existing clone. `manage` records whether the user
    /// granted permission to pull it (manage-with-permission); when false we
    /// only ever read from it.
    UserClone { path: String, manage: bool },
}

impl Default for CatalogSource {
    fn default() -> Self {
        CatalogSource::Bundled
    }
}

/// A catalog directory discovered on disk (for the first-run / Settings picker).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCandidate {
    /// Absolute path to the candidate catalog root.
    pub path: String,
    /// `"managed"` for `~/.agency-agents`, else `"userClone"`.
    pub kind: String,
    /// Whether it's a git checkout (has `.git`) — drives pull strategy.
    pub has_git: bool,
    /// Quick agent count (top-level `.md` across discovered categories).
    pub agent_count: u32,
}

/// Result of `catalog_detect` — what the app found, plus whether `git` is on
/// PATH (so the UI can explain clone vs snapshot provisioning).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDetection {
    pub git_available: bool,
    /// True when a filesystem scan of common dev roots was performed (the
    /// "Find Agency Agents" button), vs the cheap `~/.agency-agents`-only check.
    pub scanned: bool,
    pub candidates: Vec<CatalogCandidate>,
}

/// Live status of the active catalog — source, git provenance, and freshness.
/// Powers the Settings → Catalog panel ("manage the repo": which commit, how
/// far behind, what GitHub repo). All git fields are `None`/0 for a non-git
/// (bundled snapshot) source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    pub source: CatalogSource,
    /// Catalog root path (None for the bundled, app-data-internal source).
    pub root: Option<String>,
    pub is_git: bool,
    pub branch: Option<String>,
    /// Short commit SHA of HEAD.
    pub commit: Option<String>,
    pub last_commit_subject: Option<String>,
    pub last_commit_date: Option<String>,
    /// Count of uncommitted working-tree changes.
    pub dirty_count: u32,
    /// `origin` remote URL, if a git checkout.
    pub remote_url: Option<String>,
    /// `owner/repo` parsed from the remote (for GitHub repo stats), if it's a
    /// github.com remote.
    pub repo_slug: Option<String>,
    pub version: String,
    pub fetched_at: String,
    pub agent_count: u32,
}

/// Result of checking the active catalog for upstream updates — the "stats on
/// diffs" view. Git sources fetch + compare against the upstream branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogUpdateCheck {
    pub is_git: bool,
    /// Commits the upstream branch has that we don't (how far behind).
    pub behind: u32,
    /// Commits we have that upstream doesn't (local work).
    pub ahead: u32,
    /// Files that would change on pull.
    pub changed_files: u32,
    /// Human-readable `git diff --stat` of HEAD..upstream.
    pub diffstat: String,
    /// True when already at the upstream tip (git) — no-op pull.
    pub up_to_date: bool,
}

// ---------- Agent (parsed from the corpus) ----------

/// An agent as parsed from a single corpus `.md` file. `body` is the
/// markdown persona and is omitted/empty in list views (`corpus_list`)
/// to keep payloads small; `corpus_get` returns it populated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    /// Filename without `.md`, e.g. `"frontend-developer"`.
    pub slug: String,
    /// Frontmatter `name`.
    pub name: String,
    /// Frontmatter `description`.
    pub description: String,
    /// Parent directory, e.g. `"engineering"`.
    pub category: String,
    /// Frontmatter `emoji`.
    pub emoji: Option<String>,
    /// Frontmatter `color` (named or hex).
    pub color: Option<String>,
    /// Frontmatter `vibe`.
    pub vibe: Option<String>,
    /// Markdown body (persona) — lazy/optional in list views.
    pub body: String,
}

// ---------- Corpus index ----------

/// One row of `corpus-index.json`. The three split hashes let update
/// classification distinguish cosmetic (frontmatter-only) from
/// substantive (body) changes. Hash = SHA-256 lowercase hex of UTF-8
/// bytes (contracts.md §E).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusEntry {
    pub slug: String,
    pub name: String,
    pub category: String,
    pub emoji: Option<String>,
    pub color: Option<String>,
    pub vibe: Option<String>,
    pub description: String,
    /// SHA-256 of the full canonical `.md`.
    pub source_hash: String,
    /// SHA-256 of the frontmatter block.
    pub frontmatter_hash: String,
    /// SHA-256 of the body.
    pub body_hash: String,
}

/// Top-level metadata for the maintained corpus copy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusMeta {
    pub version: String,
    pub commit: Option<String>,
    pub fetched_at: String,
    pub count: u32,
}

// ---------- Install ledger ----------

/// One row of `installs.json` — the ledger of local install actions.
/// `source_hash` records the corpus version installed from;
/// `rendered_hash` is the SHA-256 of the exact bytes written after
/// per-tool conversion, used by reconciliation to classify state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecord {
    pub slug: String,
    pub tool: Tool,
    pub scope: Scope,
    pub project_path: Option<String>,
    /// Absolute path written.
    pub dest: String,
    pub source_hash: String,
    /// SHA-256 of the agent body at install time. Lets reconciliation label an
    /// available update cosmetic (body unchanged) vs substantive. `#[serde(default)]`
    /// so ledgers written before this field still parse (older rows get "").
    #[serde(default)]
    pub body_hash: String,
    pub rendered_hash: String,
    pub installed_at: String,
    pub corpus_version: String,
}

// ---------- Reconciliation ----------

/// The five reconciliation states (like a package manager's installed /
/// outdated states). See systemPatterns.md §4 for the disk ↔ ledger ↔ corpus
/// test that classifies each on-disk agent file.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum InstallState {
    Current,
    Outdated,
    Modified,
    Removed,
    Foreign,
}

/// Whether an available update is cosmetic (frontmatter/metadata only,
/// `body_hash` unchanged) or substantive (prompt body changed).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum UpdateKind {
    Cosmetic,
    Substantive,
}

/// Reconciled view-model for the Library — one on-disk agent file
/// resolved against the ledger and corpus-index. `update_kind` is
/// `Some(..)` only when `state == Outdated`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAgent {
    pub slug: String,
    pub name: String,
    pub tool: Tool,
    pub scope: Scope,
    pub project_path: Option<String>,
    pub dest: String,
    pub state: InstallState,
    pub update_kind: Option<UpdateKind>,
}

/// Result of `agent_diff` — what's on disk now vs the canonical render the app
/// would write. Powers "review before Update": the UI can show the user exactly
/// what an Update/Restore would change before any file is touched.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiff {
    pub slug: String,
    pub tool: Tool,
    pub project_path: Option<String>,
    pub dest: String,
    /// Current on-disk contents (None if the file is missing).
    pub on_disk: Option<String>,
    /// The canonical render the app would write.
    pub proposed: String,
    /// Whether the two differ (false ⇒ Update is a no-op).
    pub differs: bool,
}

// ---------- Tools / categories / projects ----------

/// View-model for the Tools section — a detected AI tool plus its
/// deployment surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub tool: Tool,
    pub label: String,
    pub detected: bool,
    pub scope: Scope,
    pub user_dest: Option<String>,
    pub installed_count: u32,
}

/// Best-effort detected version string for a tool, from probing `<bin>
/// --version`. `version` is `None` when the binary isn't on PATH, the probe
/// timed out, or the tool has no known version command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolVersion {
    pub tool: Tool,
    pub version: Option<String>,
}

/// One category for the Discover grid. `slug` is the corpus parent dir
/// (e.g. `"engineering"`); `icon` is a PascalCase Lucide icon name the
/// frontend resolves via its static icon map.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub slug: String,
    pub label: String,
    pub icon: String,
    /// Brand color (hex) for the division, from the catalog metadata.
    pub color: String,
    pub count: u32,
}

/// A registered project directory for project-scoped installs. The app
/// keeps a Projects list so Library/Tools can show per-project
/// deployment; one agent in five projects = five tracked rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    /// Absolute project root path.
    pub path: String,
    /// Display label (defaults to the directory name).
    pub label: String,
    /// Count of agents installed into this project across all
    /// project-scoped tools.
    pub installed_count: u32,
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Agency Agents: Tool enum ----------

    #[test]
    fn tool_serializes_camel_case() {
        // Multi-word variants are the drift risk — pin the exact wire
        // strings the TS `Tool` union depends on.
        assert_eq!(
            serde_json::to_string(&Tool::ClaudeCode).unwrap(),
            "\"claudeCode\""
        );
        assert_eq!(
            serde_json::to_string(&Tool::GeminiCli).unwrap(),
            "\"geminiCli\""
        );
        assert_eq!(serde_json::to_string(&Tool::Aider).unwrap(), "\"aider\"");
        assert_eq!(
            serde_json::to_string(&Tool::Antigravity).unwrap(),
            "\"antigravity\""
        );
    }

    #[test]
    fn tool_round_trips_all_eleven_variants() {
        for t in [
            Tool::ClaudeCode,
            Tool::Copilot,
            Tool::Cursor,
            Tool::GeminiCli,
            Tool::Codex,
            Tool::Opencode,
            Tool::Windsurf,
            Tool::Aider,
            Tool::Qwen,
            Tool::Openclaw,
            Tool::Antigravity,
        ] {
            let s = serde_json::to_string(&t).unwrap();
            let back: Tool = serde_json::from_str(&s).unwrap();
            assert_eq!(back, t);
        }
    }

    #[test]
    fn scope_and_states_serialize_camel_case() {
        assert_eq!(serde_json::to_string(&Scope::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Scope::Project).unwrap(),
            "\"project\""
        );
        assert_eq!(
            serde_json::to_string(&InstallState::Foreign).unwrap(),
            "\"foreign\""
        );
        assert_eq!(
            serde_json::to_string(&UpdateKind::Substantive).unwrap(),
            "\"substantive\""
        );
    }

    #[test]
    fn installed_agent_serializes_camel_case_fields() {
        let a = InstalledAgent {
            slug: "frontend-developer".into(),
            name: "Frontend Developer".into(),
            tool: Tool::ClaudeCode,
            scope: Scope::User,
            project_path: None,
            dest: "/Users/x/.claude/agents/frontend-developer.md".into(),
            state: InstallState::Outdated,
            update_kind: Some(UpdateKind::Cosmetic),
        };
        let v = serde_json::to_value(&a).unwrap();
        for k in [
            "slug",
            "name",
            "tool",
            "scope",
            "projectPath",
            "dest",
            "state",
            "updateKind",
        ] {
            assert!(v.get(k).is_some(), "InstalledAgent must have wire field {:?}", k);
        }
        for snake in ["project_path", "update_kind"] {
            assert!(v.get(snake).is_none(), "snake key {:?} must not leak", snake);
        }
        assert_eq!(v["tool"], "claudeCode");
        assert_eq!(v["state"], "outdated");
        assert_eq!(v["updateKind"], "cosmetic");
    }

    #[test]
    fn corpus_entry_serializes_split_hashes_camel_case() {
        let e = CorpusEntry {
            slug: "code-reviewer".into(),
            name: "Code Reviewer".into(),
            category: "engineering".into(),
            emoji: Some("🔍".into()),
            color: None,
            vibe: None,
            description: "Reviews code.".into(),
            source_hash: "a".repeat(64),
            frontmatter_hash: "b".repeat(64),
            body_hash: "c".repeat(64),
        };
        let v = serde_json::to_value(&e).unwrap();
        for k in ["sourceHash", "frontmatterHash", "bodyHash"] {
            assert!(v.get(k).is_some(), "CorpusEntry must have wire field {:?}", k);
        }
        for snake in ["source_hash", "frontmatter_hash", "body_hash"] {
            assert!(v.get(snake).is_none(), "snake key {:?} must not leak", snake);
        }
    }
}
