//! Deterministic per-tool agent renderers + destination-path resolution.
//!
//! Ports agency-agents `scripts/convert.sh`. Every renderer is a PURE function
//! of `(Agent, raw source)` — no timestamps, no randomness, stable key order —
//! so `rendered_hash` is reproducible. That reproducibility is the load-bearing
//! requirement for install-state reconciliation (`reconcile/`): we identify an
//! installed file as "ours" by re-rendering its slug for its tool and matching
//! bytes. See `memory-bank/contracts.md` §B/§E.
//!
//! Identity tools (claude-code, copilot) ship the agent `.md` verbatim, so their
//! "render" is the raw corpus source. Transform tools (cursor/.mdc, codex/TOML,
//! gemini-cli, opencode, qwen) rebuild the file from frontmatter fields + body.
//! The remaining tools (antigravity skill dirs, openclaw multi-file, aider /
//! windsurf accumulated files) are special multi-file shapes — not yet supported
//! here; `render`/`dests` return an error so the UI can disable them cleanly.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::types::{Agent, Scope, Tool};

impl Tool {
    /// User-global (`~/…`) vs project-scoped (`./…`) deployment.
    pub fn scope(self) -> Scope {
        match self {
            Tool::Cursor | Tool::Opencode | Tool::Windsurf | Tool::Aider => Scope::Project,
            _ => Scope::User,
        }
    }

    /// kebab id, matching `scripts/install.sh` tool names.
    pub fn id(self) -> &'static str {
        match self {
            Tool::ClaudeCode => "claude-code",
            Tool::Copilot => "copilot",
            Tool::Cursor => "cursor",
            Tool::GeminiCli => "gemini-cli",
            Tool::Codex => "codex",
            Tool::Opencode => "opencode",
            Tool::Windsurf => "windsurf",
            Tool::Aider => "aider",
            Tool::Qwen => "qwen",
            Tool::Openclaw => "openclaw",
            Tool::Antigravity => "antigravity",
        }
    }

    /// Human label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Tool::ClaudeCode => "Claude Code",
            Tool::Copilot => "GitHub Copilot",
            Tool::Cursor => "Cursor",
            Tool::GeminiCli => "Gemini CLI",
            Tool::Codex => "Codex",
            Tool::Opencode => "opencode",
            Tool::Windsurf => "Windsurf",
            Tool::Aider => "Aider",
            Tool::Qwen => "Qwen Code",
            Tool::Openclaw => "OpenClaw",
            Tool::Antigravity => "Antigravity",
        }
    }
}

/// SHA-256, lowercase hex — the canonical hash for the ledger + reconcile.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Match `scripts/lib.sh#get_field`: return the first literal `field: value`
/// line between exact `---` fences. The shell helper does not parse YAML, so
/// quotes and other source spelling must be preserved for byte parity.
fn source_field<'a>(source: &'a str, field: &str) -> &'a str {
    let prefix = format!("{field}: ");
    let mut fences = 0;
    for line in source.lines() {
        if line == "---" {
            fences += 1;
            continue;
        }
        if fences == 1 {
            if let Some(value) = line.strip_prefix(&prefix) {
                return value;
            }
        } else if fences >= 2 {
            break;
        }
    }
    ""
}

/// Match `body="$(get_body "$file")"` from the upstream converter. `awk`
/// emits one newline per body line and command substitution strips every
/// trailing newline before the heredoc adds exactly one back.
fn source_body(source: &str) -> String {
    let mut fences = 0;
    let mut body = String::new();
    for line in source.lines() {
        if line == "---" {
            fences += 1;
            continue;
        }
        if fences >= 2 {
            body.push_str(line);
            body.push('\n');
        }
    }
    while body.ends_with('\n') {
        body.pop();
    }
    body
}

/// Match `scripts/lib.sh#slugify`.
pub fn slugify(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
            previous_dash = false;
        } else if !out.is_empty() && !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Filename stem emitted by `convert.sh`. Identity tools preserve the source
/// filename; transform tools derive it from frontmatter `name`.
pub fn output_slug(agent: &Agent, raw_source: &str, tool: Tool) -> String {
    match tool {
        Tool::ClaudeCode | Tool::Copilot => agent.slug.clone(),
        _ => slugify(source_field(raw_source, "name")),
    }
}

fn unsupported(tool: Tool) -> AppError {
    AppError::Io {
        message: format!(
            "tool '{}' is not supported for install yet (multi-file format)",
            tool.id()
        ),
    }
}

/// Render the file content for `tool` from `agent` (+ the raw corpus `.md`
/// source, used verbatim by identity tools). Deterministic.
pub fn render(_agent: &Agent, raw_source: &str, tool: Tool) -> Result<String, AppError> {
    let name = source_field(raw_source, "name");
    let description = source_field(raw_source, "description");
    let body = source_body(raw_source);
    let slug = slugify(name);
    let out = match tool {
        // Identity — ship the corpus `.md` exactly as authored.
        Tool::ClaudeCode | Tool::Copilot => raw_source.to_string(),

        // Cursor `.mdc`: description + globs + alwaysApply frontmatter.
        Tool::Cursor => format!(
            "---\ndescription: {desc}\nglobs: \"\"\nalwaysApply: false\n---\n{body}\n",
            desc = description,
        ),

        // Codex TOML: minimal required fields, control chars escaped.
        Tool::Codex => format!(
            "name = \"{name}\"\ndescription = \"{desc}\"\ndeveloper_instructions = \"{body}\"\n",
            name = toml_escape(name),
            desc = toml_escape(description),
            body = toml_escape(&body),
        ),

        // Gemini CLI subagent `.md`: name(=slug) + description frontmatter.
        Tool::GeminiCli => format!(
            "---\nname: {slug}\ndescription: {desc}\n---\n{body}\n",
            desc = description,
        ),

        // Qwen Code SubAgent `.md`: optional tools line is preserved literally.
        Tool::Qwen => {
            let tools = source_field(raw_source, "tools");
            if tools.is_empty() {
                format!("---\nname: {slug}\ndescription: {description}\n---\n{body}\n")
            } else {
                format!(
                    "---\nname: {slug}\ndescription: {description}\ntools: {tools}\n---\n{body}\n"
                )
            }
        }

        // OpenCode `.md`: name + description + mode + hex color frontmatter.
        Tool::Opencode => format!(
            "---\nname: {name}\ndescription: {desc}\nmode: subagent\ncolor: '{color}'\n---\n{body}\n",
            desc = description,
            color = resolve_opencode_color(source_field(raw_source, "color")),
        ),

        Tool::Windsurf | Tool::Aider | Tool::Openclaw | Tool::Antigravity => {
            return Err(unsupported(tool))
        }
    };
    Ok(out)
}

/// Render + hash in one shot.
pub fn render_with_hash(
    agent: &Agent,
    raw_source: &str,
    tool: Tool,
) -> Result<(String, String), AppError> {
    let bytes = render(agent, raw_source, tool)?;
    let hash = sha256_hex(bytes.as_bytes());
    Ok((bytes, hash))
}

/// Absolute destination path(s) for an installed agent. Most tools write a
/// single file; Copilot dual-writes to `~/.github` and `~/.copilot`.
///
/// `home` is the user's home dir (user-scoped tools). `project_root` is required
/// for project-scoped tools (cursor, opencode) and ignored otherwise.
pub fn dests(
    tool: Tool,
    slug: &str,
    home: &Path,
    project_root: Option<&Path>,
) -> Result<Vec<PathBuf>, AppError> {
    let proj = || -> Result<&Path, AppError> {
        project_root.ok_or_else(|| AppError::Io {
            message: format!("tool '{}' is project-scoped; a project path is required", tool.id()),
        })
    };
    let v = match tool {
        Tool::ClaudeCode => vec![home.join(".claude/agents").join(format!("{slug}.md"))],
        Tool::Copilot => vec![
            home.join(".github/agents").join(format!("{slug}.md")),
            home.join(".copilot/agents").join(format!("{slug}.md")),
        ],
        Tool::Codex => vec![home.join(".codex/agents").join(format!("{slug}.toml"))],
        Tool::GeminiCli => vec![home.join(".gemini/agents").join(format!("{slug}.md"))],
        Tool::Qwen => vec![home.join(".qwen/agents").join(format!("{slug}.md"))],
        Tool::Cursor => vec![proj()?.join(".cursor/rules").join(format!("{slug}.mdc"))],
        Tool::Opencode => vec![proj()?.join(".opencode/agents").join(format!("{slug}.md"))],
        Tool::Windsurf | Tool::Aider | Tool::Openclaw | Tool::Antigravity => {
            return Err(unsupported(tool))
        }
    };
    Ok(v)
}

/// Map an agency-agents `color` (named or hex) to an OpenCode-safe `#RRGGBB`
/// (uppercase). Unknown → neutral gray. Ported from `resolve_opencode_color`.
fn resolve_opencode_color(color: &str) -> String {
    let c = color.trim().to_ascii_lowercase();
    let mapped = match c.as_str() {
        "cyan" => "#00FFFF",
        "blue" => "#3498DB",
        "green" => "#2ECC71",
        "red" => "#E74C3C",
        "purple" => "#9B59B6",
        "orange" => "#F39C12",
        "teal" => "#008080",
        "indigo" => "#6366F1",
        "pink" => "#E84393",
        "gold" => "#EAB308",
        "amber" => "#F59E0B",
        "neon-green" => "#10B981",
        "neon-cyan" => "#06B6D4",
        "metallic-blue" => "#3B82F6",
        "yellow" => "#EAB308",
        "violet" => "#8B5CF6",
        "rose" => "#F43F5E",
        "lime" => "#84CC16",
        "gray" => "#6B7280",
        "fuchsia" => "#D946EF",
        other => other,
    };
    let hex = mapped.strip_prefix('#').unwrap_or(mapped);
    let is_hex6 = hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit());
    if is_hex6 {
        format!("#{}", hex.to_ascii_uppercase())
    } else {
        "#6B7280".to_string()
    }
}

/// Escape a value for a TOML basic string (ported from `toml_escape_string`).
fn toml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) == 0x7F => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use std::process::Command;

    fn agent() -> Agent {
        Agent {
            slug: "frontend-developer".into(),
            name: "Frontend Developer".into(),
            description: "Builds UIs.".into(),
            category: "engineering".into(),
            emoji: Some("🎨".into()),
            color: Some("blue".into()),
            vibe: Some("Ships pixels.".into()),
            body: "You are a frontend dev.\n".into(),
        }
    }

    fn raw() -> &'static str {
        "---\nname: Frontend Developer\ndescription: Builds UIs.\ncolor: blue\nemoji: 🎨\nvibe: Ships pixels.\n---\nYou are a frontend dev.\n"
    }

    #[test]
    fn claude_code_is_identity() {
        let a = agent();
        let raw = "---\nname: Frontend Developer\n---\nORIGINAL BODY\n";
        assert_eq!(render(&a, raw, Tool::ClaudeCode).unwrap(), raw);
        assert_eq!(render(&a, raw, Tool::Copilot).unwrap(), raw);
    }

    #[test]
    fn cursor_mdc_shape() {
        let out = render(&agent(), raw(), Tool::Cursor).unwrap();
        assert!(out.starts_with("---\ndescription: Builds UIs.\nglobs: \"\"\nalwaysApply: false\n---\n"));
        assert!(out.contains("You are a frontend dev."));
    }

    #[test]
    fn codex_toml_escapes() {
        let mut a = agent();
        a.description = "has \"quotes\" and\nnewline".into();
        let source = "---\nname: Frontend Developer\ndescription: has \"quotes\" and\tcontrols\n---\nline 1\nline \"2\"\n";
        let out = render(&a, source, Tool::Codex).unwrap();
        assert!(out.contains("description = \"has \\\"quotes\\\" and\\tcontrols\""));
        assert!(out.contains("developer_instructions = \"line 1\\nline \\\"2\\\"\""));
        assert!(out.starts_with("name = \"Frontend Developer\""));
    }

    #[test]
    fn opencode_color_maps_to_hex() {
        let out = render(&agent(), raw(), Tool::Opencode).unwrap();
        assert!(out.contains("color: '#3498DB'"), "blue → #3498DB: {out}");
        assert!(out.contains("mode: subagent"));
    }

    #[test]
    fn opencode_unknown_color_falls_back() {
        let mut a = agent();
        a.color = None;
        let source = "---\nname: Frontend Developer\ndescription: Builds UIs.\n---\nBody\n";
        let out = render(&a, source, Tool::Opencode).unwrap();
        assert!(out.contains("color: '#6B7280'"));
    }

    #[test]
    fn gemini_uses_slug_as_name() {
        let out = render(&agent(), raw(), Tool::GeminiCli).unwrap();
        assert!(out.starts_with("---\nname: frontend-developer\ndescription: Builds UIs.\n---\n"));
    }

    #[test]
    fn render_is_deterministic() {
        for tool in [Tool::Cursor, Tool::Codex, Tool::Opencode, Tool::GeminiCli, Tool::Qwen] {
            let a = render(&agent(), raw(), tool).unwrap();
            let b = render(&agent(), raw(), tool).unwrap();
            assert_eq!(a, b, "{tool:?} must be deterministic");
        }
    }

    #[test]
    fn source_helpers_match_shell_semantics() {
        let source = "---\nname: \"Quoted Name\"\ndescription: has: colon\ntools: Read, Write\n---\nBody\n---\nTail\n\n";
        assert_eq!(source_field(source, "name"), "\"Quoted Name\"");
        assert_eq!(source_field(source, "description"), "has: colon");
        assert_eq!(source_body(source), "Body\nTail");
        assert_eq!(slugify("FP&A / QA"), "fp-a-qa");
    }

    #[test]
    fn qwen_preserves_optional_tools() {
        let source = "---\nname: Frontend Developer\ndescription: Builds UIs.\ntools: Read, Write\n---\nBody\n";
        let out = render(&agent(), source, Tool::Qwen).unwrap();
        assert!(out.contains("\ntools: Read, Write\n"));

        let without = render(&agent(), raw(), Tool::Qwen).unwrap();
        assert!(!without.contains("\ntools: "));
    }

    #[test]
    fn output_slug_matches_converter_identity_rules() {
        let mut a = agent();
        a.slug = "engineering-frontend-developer".into();
        assert_eq!(
            output_slug(&a, raw(), Tool::ClaudeCode),
            "engineering-frontend-developer"
        );
        assert_eq!(output_slug(&a, raw(), Tool::Codex), "frontend-developer");
    }

    fn collect_markdown(root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_markdown(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }

    #[test]
    #[ignore = "requires AGENCY_AGENTS_PARITY_ROOT and executes upstream convert.sh"]
    fn upstream_convert_sh_is_byte_identical_for_transform_tools() {
        let root = std::env::var("AGENCY_AGENTS_PARITY_ROOT")
            .expect("set AGENCY_AGENTS_PARITY_ROOT to an agency-agents clone");
        let root = PathBuf::from(root);
        let script = root.join("scripts/convert.sh");
        assert!(script.is_file(), "missing {}", script.display());

        let script_text = fs::read_to_string(&script).unwrap();
        let dirs_start = script_text.find("AGENT_DIRS=(").expect("AGENT_DIRS");
        let dirs_tail = &script_text[dirs_start + "AGENT_DIRS=(".len()..];
        let dirs_body = dirs_tail.split(')').next().expect("AGENT_DIRS close");
        let categories: Vec<&str> = dirs_body.split_whitespace().collect();

        let temp = tempfile::tempdir().unwrap();
        let tools = [
            (Tool::Cursor, "cursor/rules", "mdc"),
            (Tool::Codex, "codex/agents", "toml"),
            (Tool::GeminiCli, "gemini-cli/agents", "md"),
            (Tool::Opencode, "opencode/agents", "md"),
            (Tool::Qwen, "qwen/agents", "md"),
        ];
        for (_, tool_id, _) in tools {
            let tool = tool_id.split('/').next().unwrap();
            let status = Command::new("bash")
                .arg(&script)
                .args(["--tool", tool, "--out"])
                .arg(temp.path())
                .status()
                .unwrap();
            assert!(status.success(), "convert.sh failed for {tool}");
        }

        let mut files = Vec::new();
        for category in categories {
            collect_markdown(&root.join(category), &mut files);
        }
        files.sort();

        let mut conversion_slugs = HashSet::new();
        let mut compared = 0usize;
        for path in files {
            let raw = fs::read_to_string(&path).unwrap();
            let name = source_field(&raw, "name");
            if name.is_empty() || !raw.starts_with("---\n") {
                continue;
            }
            let source_slug = path.file_stem().unwrap().to_string_lossy().to_string();
            let agent = Agent {
                slug: source_slug,
                name: name.to_string(),
                description: String::new(),
                category: String::new(),
                emoji: None,
                color: None,
                vibe: None,
                body: String::new(),
            };
            let converted_slug = output_slug(&agent, &raw, Tool::Codex);
            assert!(
                conversion_slugs.insert(converted_slug.clone()),
                "duplicate conversion slug: {converted_slug}"
            );
            for (tool, subdir, ext) in tools {
                let expected_path =
                    temp.path().join(subdir).join(format!("{converted_slug}.{ext}"));
                let expected = fs::read(&expected_path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", expected_path.display()));
                let actual = render(&agent, &raw, tool).unwrap();
                assert_eq!(
                    actual.as_bytes(),
                    expected,
                    "{tool:?} parity mismatch for {}",
                    path.display()
                );
                compared += 1;
            }
        }
        assert!(compared > 0);
        eprintln!(
            "renderer parity: {} agents, {} byte comparisons",
            conversion_slugs.len(),
            compared
        );
    }

    #[test]
    fn unsupported_tools_error() {
        for tool in [Tool::Windsurf, Tool::Aider, Tool::Openclaw, Tool::Antigravity] {
            assert!(render(&agent(), "raw", tool).is_err());
            assert!(dests(tool, "x", Path::new("/home"), Some(Path::new("/p"))).is_err());
        }
    }

    #[test]
    fn dests_per_tool() {
        let home = Path::new("/Users/x");
        let proj = Path::new("/proj");
        assert_eq!(
            dests(Tool::ClaudeCode, "a", home, None).unwrap(),
            vec![PathBuf::from("/Users/x/.claude/agents/a.md")]
        );
        assert_eq!(dests(Tool::Copilot, "a", home, None).unwrap().len(), 2);
        assert_eq!(
            dests(Tool::Codex, "a", home, None).unwrap(),
            vec![PathBuf::from("/Users/x/.codex/agents/a.toml")]
        );
        assert_eq!(
            dests(Tool::Cursor, "a", home, Some(proj)).unwrap(),
            vec![PathBuf::from("/proj/.cursor/rules/a.mdc")]
        );
        // project-scoped without a project path → error
        assert!(dests(Tool::Cursor, "a", home, None).is_err());
    }

    #[test]
    fn scope_classification() {
        assert_eq!(Tool::ClaudeCode.scope(), Scope::User);
        assert_eq!(Tool::Cursor.scope(), Scope::Project);
        assert_eq!(Tool::Opencode.scope(), Scope::Project);
        assert_eq!(Tool::Codex.scope(), Scope::User);
    }
}
