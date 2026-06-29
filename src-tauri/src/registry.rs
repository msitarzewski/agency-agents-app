//! Tool registry — the single source of truth for supported tools.
//!
//! Loaded from the embedded `data/tools.json`, a bundled baseline of the
//! canonical catalog the upstream `agency-agents` repo owns (alongside
//! `divisions.json`). It carries *upstream truth* — what the CLI converts +
//! installs — and nothing app-specific. Whether THIS app can install a tool is
//! derived, not stored: a tool is installable iff we ship a native renderer for
//! its `format` (see `IMPLEMENTED_FORMATS`). Mirrors the frontend
//! `toolRegistry.ts`, which reads the same file.

use std::sync::OnceLock;

use serde::Deserialize;

const TOOLS_JSON: &str = include_str!("../data/tools.json");

/// The render formats our native Rust renderer implements. A tool is installable
/// in this app iff its `format` is one of these — derived from the catalog's
/// `format`, never stored there (the catalog is upstream truth; renderer
/// coverage is our concern). Adding a renderer = adding its format here.
const IMPLEMENTED_FORMATS: &[&str] = &[
    "identity",
    "codex-toml",
    "gemini-md",
    "qwen-md",
    "cursor-mdc",
    "opencode-md",
    "skill-md",
];

/// Scope capabilities — whether a tool can deploy user-globally and/or per-project.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScopeCaps {
    #[serde(default)]
    pub user: bool,
    #[serde(default)]
    pub project: bool,
}

/// Detection hints: dirs whose presence implies the tool is installed.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Detect {
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub agents_dir: Option<String>,
}

/// `<bin> <args…>` probe for the installed version.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionCmd {
    pub bin: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Destination path templates (relative; `{slug}` substituted). User paths are
/// rooted at `$HOME`, project paths at the project root.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Dest {
    #[serde(default)]
    pub user: Vec<String>,
    #[serde(default)]
    pub project: Vec<String>,
}

/// One tool's definition, as authored in the canonical `tools.json`.
///
/// Some fields (`short`, `accent`, `icon`) exist to mirror the frontend
/// `toolRegistry.ts` and to validate that the bundled JSON parses cleanly; the
/// Rust backend doesn't consume them, hence `dead_code` is allowed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ToolMeta {
    /// camelCase id — the wire value used by the frontend + install ledger.
    pub id: String,
    /// Full display name, e.g. "Claude Code".
    pub label: String,
    #[serde(default)]
    pub short: String,
    /// kebab id matching the CLI install scripts, e.g. "claude-code".
    pub kebab: String,
    #[serde(default)]
    pub accent: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub order: Option<u32>,
    #[serde(default)]
    pub scope: Option<ScopeCaps>,
    #[serde(default)]
    pub detect: Option<Detect>,
    #[serde(default)]
    pub version: Option<VersionCmd>,
    /// Renderer key (e.g. "identity", "codex-toml", "skill-md"). The transform
    /// CONTRACT: same `format` ⇒ byte-identical output.
    #[serde(default)]
    pub format: Option<String>,
    /// Install MECHANISM, upstream truth: "per-agent" (N rendered files),
    /// "roster" (one aggregate file), or "plugin" (a router plugin — Hermes).
    /// A `plugin` tool is never app-installable; the CLI owns its install.
    #[serde(default)]
    pub install_kind: Option<String>,
    /// "source" (keep the corpus filename) or "name" (slugify frontmatter name);
    /// null for single-file roster formats (aider/windsurf).
    #[serde(default)]
    pub slug_from: Option<String>,
    /// Namespace prepended to the output slug AND the rendered `name` field —
    /// "agency-" for skill-dir tools that share a global skills folder.
    #[serde(default)]
    pub slug_prefix: Option<String>,
    #[serde(default)]
    pub dest: Option<Dest>,
}

impl ToolMeta {
    pub fn supports_user(&self) -> bool {
        self.scope.as_ref().is_some_and(|s| s.user)
    }
    pub fn supports_project(&self) -> bool {
        self.scope.as_ref().is_some_and(|s| s.project)
    }
    /// Installable in this app: we ship a native renderer for its `format` AND it
    /// installs as per-agent/roster files. Aggregate `plugin` integrations (e.g.
    /// Hermes) install one router plugin, not per-agent files — the CLI owns them,
    /// so they're never app-installable even if a renderer existed. (per-agent,
    /// roster, or a missing kind all fall through to the format check.)
    pub fn installable(&self) -> bool {
        if self.install_kind.as_deref() == Some("plugin") {
            return false;
        }
        self.format.as_deref().is_some_and(|f| IMPLEMENTED_FORMATS.contains(&f))
    }
}

/// The canonical catalog wrapper: `{ "_note": …, "tools": { "<kebab>": {…} } }`.
#[derive(Deserialize)]
struct Catalog {
    tools: std::collections::BTreeMap<String, ToolMeta>,
}

/// Parse + cache the registry on first access. Panics on malformed JSON — that's
/// a build-time authoring error in the bundled catalog, not a runtime condition.
fn registry() -> &'static Vec<ToolMeta> {
    static REG: OnceLock<Vec<ToolMeta>> = OnceLock::new();
    REG.get_or_init(|| {
        let cat: Catalog =
            serde_json::from_str(TOOLS_JSON).unwrap_or_else(|e| panic!("invalid tools.json: {e}"));
        let mut v: Vec<ToolMeta> = cat.tools.into_values().collect();
        v.sort_by(|a, b| {
            a.order
                .unwrap_or(999)
                .cmp(&b.order.unwrap_or(999))
                .then_with(|| a.label.cmp(&b.label))
        });
        v
    })
}

/// All tools, in registry order (install-menu order first, then by label).
/// Public surface mirroring the frontend's full list; the backend reaches tools
/// via `get`/`wired`.
#[allow(dead_code)]
pub fn all() -> &'static [ToolMeta] {
    registry().as_slice()
}

/// Look up a tool by its camelCase id.
pub fn get(id: &str) -> Option<&'static ToolMeta> {
    registry().iter().find(|t| t.id == id)
}

/// Iterator over the tools THIS app can install (has a native renderer for).
pub fn wired() -> impl Iterator<Item = &'static ToolMeta> {
    registry().iter().filter(|t| t.installable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_and_derives_installable() {
        assert_eq!(all().len(), 14, "expected the full bundled tool set");
        // The eight tools whose format we render are installable.
        for id in [
            "claudeCode", "codex", "geminiCli", "copilot", "qwen", "cursor", "opencode", "osaurus",
        ] {
            let m = get(id).unwrap_or_else(|| panic!("missing tool {id}"));
            assert!(m.installable(), "{id} should be installable");
            assert!(m.format.is_some() && m.dest.is_some(), "{id} needs format + dest");
        }
    }

    #[test]
    fn recognized_tools_are_not_installable() {
        // These carry a real (upstream) format the app doesn't render yet.
        for id in ["windsurf", "aider", "openclaw", "kimi"] {
            let m = get(id).unwrap();
            assert!(!m.installable(), "{id} is recognized-only in the app");
            assert!(m.format.is_some(), "{id} still has an upstream format");
        }
    }

    #[test]
    fn plugin_tools_are_never_installable() {
        // Hermes installs ONE router plugin (installKind "plugin"), not per-agent
        // files — recognized-only / CLI-only regardless of renderer coverage.
        let h = get("hermes").expect("hermes in the refreshed catalog");
        assert_eq!(h.install_kind.as_deref(), Some("plugin"));
        assert!(!h.installable(), "a plugin tool is never app-installable");
    }
}
