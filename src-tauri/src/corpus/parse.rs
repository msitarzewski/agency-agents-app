//! Tolerant frontmatter parsing + canonical hashing for the corpus.
//!
//! Each agent lives in a single `.md` whose head is a YAML frontmatter
//! block fenced by `---` lines, followed by the markdown persona body:
//!
//! ```text
//! ---
//! name: Frontend Developer
//! description: "Builds delightful UIs."
//! color: blue
//! emoji: 🎨
//! vibe: Ships pixel-perfect interfaces.
//! ---
//! # Frontend Developer Agent
//! You are a …
//! ```
//!
//! Parsing mirrors the agency-agents `scripts/convert.sh` reference
//! (`get_field` / `get_body`): the frontmatter is everything between the
//! first two `---` fences; the body is everything after the second
//! fence. We parse the frontmatter with `serde_yaml` (tolerant of quoted
//! / multiline values) rather than the shell's line-grep so descriptions
//! that span lines or carry colons survive intact.
//!
//! Determinism (contracts.md §E): all three hashes are SHA-256 lowercase
//! hex of the UTF-8 bytes of a *canonical* slice of the source — no
//! timestamps, no re-serialization, no map reordering. We hash the raw
//! byte ranges of the original file so the same `.md` always yields the
//! same hashes regardless of platform or run.

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::types::{Agent, CorpusEntry, SkipReason};

/// The subset of frontmatter keys we surface. Unknown keys are ignored
/// (tolerant parse). `name` is the only required field — frontmatter
/// without it is reported as [`SkipReason::MissingName`].
#[derive(Debug, Deserialize)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
    emoji: Option<String>,
    color: Option<String>,
    vibe: Option<String>,
}

/// Result of splitting a raw `.md` into its three canonical regions.
/// Slices borrow from the source so hashing never copies.
struct Split<'a> {
    /// The frontmatter YAML, *between* the fences (no `---` lines).
    frontmatter: &'a str,
    /// The persona body, after the closing fence.
    body: &'a str,
}

/// Why [`split_frontmatter`] found no frontmatter block.
enum NoSplit {
    /// The first line isn't `---`: a README or plain doc, not an agent.
    NotAgent,
    /// The first line is `---`, but no closing fence follows.
    Unclosed,
}

/// Split a raw agent `.md` into frontmatter + body.
///
/// Fails with [`NoSplit::NotAgent`] when the file doesn't open with a `---`
/// fence, and with [`NoSplit::Unclosed`] when it opens one that never
/// closes. We accept a leading UTF-8 BOM and trailing whitespace on the
/// fence lines so files authored on different editors still parse.
fn split_frontmatter(source: &str) -> Result<Split<'_>, NoSplit> {
    // Tolerate a leading BOM.
    let s = source.strip_prefix('\u{feff}').unwrap_or(source);

    // The opening fence must be the very first line (after the optional
    // BOM). We match a line that is exactly `---` ignoring trailing
    // whitespace.
    let mut rest = s;
    let Some(first_line_end) = rest.find('\n') else {
        // One line only: a lone fence opens a block it can't close.
        return Err(if rest.trim_end() == "---" {
            NoSplit::Unclosed
        } else {
            NoSplit::NotAgent
        });
    };
    let first_line = rest[..first_line_end].trim_end();
    if first_line != "---" {
        return Err(NoSplit::NotAgent);
    }
    rest = &rest[first_line_end + 1..];

    // Walk lines until the closing fence. We track byte offsets so the
    // frontmatter slice is exact.
    let fm_start_offset = rest.as_ptr() as usize - s.as_ptr() as usize;
    let mut search_from = 0usize;
    loop {
        let line_end = rest[search_from..]
            .find('\n')
            .map(|i| search_from + i)
            .unwrap_or(rest.len());
        let line = rest[search_from..line_end].trim_end();
        if line == "---" {
            let fm_end_offset = fm_start_offset + search_from;
            let frontmatter = &s[fm_start_offset..fm_end_offset];
            // Body starts after this closing-fence line's newline (if any).
            let body_offset = if line_end < rest.len() {
                fm_start_offset + line_end + 1
            } else {
                s.len()
            };
            let body = &s[body_offset..];
            return Ok(Split { frontmatter, body });
        }
        if line_end >= rest.len() {
            // Hit EOF without a closing fence.
            return Err(NoSplit::Unclosed);
        }
        search_from = line_end + 1;
    }
}

/// SHA-256 lowercase hex of `bytes` (contracts.md §E rule 2).
fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Why a file that opens a frontmatter block still isn't an agent. The
/// indexer reports these rather than dropping the file silently (#79).
#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub reason: SkipReason,
    /// The YAML parser's message, for [`SkipReason::InvalidYaml`].
    pub detail: Option<String>,
}

impl ParseError {
    fn of(reason: SkipReason) -> Self {
        ParseError {
            reason,
            detail: None,
        }
    }
}

/// [`ParseError`] for frontmatter that `serde_yaml` rejected. The parser
/// counts lines from the top of the block, one below the file's own line
/// numbers; re-parsing with a leading newline makes its "at line N column M"
/// match the file, so a catalog author can go straight to the spot.
fn invalid_yaml(frontmatter: &str) -> ParseError {
    let detail = serde_yaml::from_str::<Frontmatter>(&format!("\n{frontmatter}"))
        .err()
        .map(|e| e.to_string());
    ParseError {
        reason: SkipReason::InvalidYaml,
        detail,
    }
}

/// Parse one agent `.md`.
///
/// `slug` is the filename without `.md` (contracts.md §A); `category` is
/// the parent directory name. Returns `Ok(None)` for files with no
/// frontmatter (READMEs, plain docs), which aren't agents. Returns `Err`
/// when the file opens a frontmatter block but can't be an agent: the block
/// never closes, isn't valid YAML, or has no `name`.
///
/// Both the [`Agent`] (with body) and the [`CorpusEntry`] (index row with
/// the three split hashes) are returned together so a single parse feeds
/// both list/detail views and the on-disk index.
pub fn parse_agent(
    slug: &str,
    category: &str,
    source: &str,
) -> Result<Option<(Agent, CorpusEntry)>, ParseError> {
    let split = match split_frontmatter(source) {
        Ok(split) => split,
        Err(NoSplit::NotAgent) => return Ok(None),
        Err(NoSplit::Unclosed) => return Err(ParseError::of(SkipReason::UnclosedFrontmatter)),
    };

    let fm: Frontmatter =
        serde_yaml::from_str(split.frontmatter).map_err(|_| invalid_yaml(split.frontmatter))?;

    // `name` is required.
    let Some(name) = fm.name.filter(|n| !n.trim().is_empty()) else {
        return Err(ParseError::of(SkipReason::MissingName));
    };

    let description = fm.description.unwrap_or_default();
    let body = split.body.to_string();

    // Hash the canonical byte regions of the *source* (not a
    // re-serialization) so the values are stable across runs/platforms.
    let source_hash = sha256_hex(source.as_bytes());
    let frontmatter_hash = sha256_hex(split.frontmatter.as_bytes());
    let body_hash = sha256_hex(split.body.as_bytes());

    let agent = Agent {
        slug: slug.to_string(),
        name: name.clone(),
        description: description.clone(),
        category: category.to_string(),
        emoji: fm.emoji.clone(),
        color: fm.color.clone(),
        vibe: fm.vibe.clone(),
        body,
    };

    let entry = CorpusEntry {
        slug: slug.to_string(),
        name,
        category: category.to_string(),
        emoji: fm.emoji,
        color: fm.color,
        vibe: fm.vibe,
        description,
        source_hash,
        frontmatter_hash,
        body_hash,
    };

    Ok(Some((agent, entry)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\nname: Frontend Developer\ndescription: \"Builds delightful UIs.\"\ncolor: blue\nemoji: 🎨\nvibe: Ships pixel-perfect interfaces.\n---\n# Frontend Developer Agent\n\nYou are a frontend developer.\n";

    #[test]
    fn parses_full_frontmatter() {
        let (agent, entry) = parse_agent("frontend-developer", "engineering", SAMPLE)
            .expect("ok")
            .expect("some");
        assert_eq!(agent.slug, "frontend-developer");
        assert_eq!(agent.name, "Frontend Developer");
        assert_eq!(agent.category, "engineering");
        assert_eq!(agent.description, "Builds delightful UIs.");
        assert_eq!(agent.color.as_deref(), Some("blue"));
        assert_eq!(agent.emoji.as_deref(), Some("🎨"));
        assert_eq!(
            agent.vibe.as_deref(),
            Some("Ships pixel-perfect interfaces.")
        );
        assert!(agent.body.starts_with("# Frontend Developer Agent"));
        // Index row mirrors the agent metadata.
        assert_eq!(entry.name, agent.name);
        assert_eq!(entry.description, agent.description);
    }

    #[test]
    fn hashes_are_deterministic_lowercase_hex_64() {
        let (_, e1) = parse_agent("x", "engineering", SAMPLE).unwrap().unwrap();
        let (_, e2) = parse_agent("x", "engineering", SAMPLE).unwrap().unwrap();
        // Stable across parses.
        assert_eq!(e1.source_hash, e2.source_hash);
        assert_eq!(e1.frontmatter_hash, e2.frontmatter_hash);
        assert_eq!(e1.body_hash, e2.body_hash);
        // 64 lowercase hex chars.
        for h in [&e1.source_hash, &e1.frontmatter_hash, &e1.body_hash] {
            assert_eq!(h.len(), 64);
            assert!(h
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
        // The three regions differ, so their hashes differ.
        assert_ne!(e1.frontmatter_hash, e1.body_hash);
        assert_ne!(e1.source_hash, e1.frontmatter_hash);
    }

    #[test]
    fn body_only_change_keeps_frontmatter_hash() {
        // Cosmetic vs substantive classification depends on this: a body
        // edit must move body_hash + source_hash but NOT frontmatter_hash.
        let edited = SAMPLE.replace(
            "You are a frontend developer.",
            "You are a senior frontend dev.",
        );
        let (_, base) = parse_agent("x", "engineering", SAMPLE).unwrap().unwrap();
        let (_, mutated) = parse_agent("x", "engineering", &edited).unwrap().unwrap();
        assert_eq!(base.frontmatter_hash, mutated.frontmatter_hash);
        assert_ne!(base.body_hash, mutated.body_hash);
        assert_ne!(base.source_hash, mutated.source_hash);
    }

    #[test]
    fn file_without_frontmatter_is_not_an_agent() {
        let md = "# Just a README\n\nNo frontmatter here.\n";
        assert!(parse_agent("readme", "examples", md).unwrap().is_none());
    }

    #[test]
    fn empty_file_is_not_an_agent() {
        assert!(parse_agent("empty", "examples", "").unwrap().is_none());
    }

    #[test]
    fn frontmatter_without_name_is_reported() {
        let md = "---\ndescription: orphan\n---\nbody\n";
        let err = parse_agent("orphan", "examples", md).unwrap_err();
        assert_eq!(err, ParseError::of(SkipReason::MissingName));
    }

    #[test]
    fn invalid_yaml_is_reported_with_the_files_line_number() {
        // #79: an unquoted colon inside a description. The description is
        // line 3 of the file (line 2 of the frontmatter block).
        let md = "---\nname: Tooling\ndescription: Builds CLIs: fast ones\n---\nbody\n";
        let err = parse_agent("tooling", "engineering", md).unwrap_err();
        assert_eq!(err.reason, SkipReason::InvalidYaml);
        let detail = err.detail.expect("parser message");
        assert!(detail.contains("at line 3 column"), "{detail}");
    }

    #[test]
    fn lone_fence_is_unclosed() {
        let err = parse_agent("x", "c", "---").unwrap_err();
        assert_eq!(err, ParseError::of(SkipReason::UnclosedFrontmatter));
    }

    #[test]
    fn tolerates_leading_bom_and_trailing_fence_whitespace() {
        let md = "\u{feff}---  \nname: X\n---  \nbody\n";
        let (agent, _) = parse_agent("x", "c", md).unwrap().unwrap();
        assert_eq!(agent.name, "X");
        assert_eq!(agent.body, "body\n");
    }

    #[test]
    fn unclosed_frontmatter_is_reported() {
        let md = "---\nname: X\nstill in frontmatter\n";
        let err = parse_agent("x", "c", md).unwrap_err();
        assert_eq!(err, ParseError::of(SkipReason::UnclosedFrontmatter));
    }

    #[test]
    fn missing_optional_fields_default_cleanly() {
        let md = "---\nname: Minimal\n---\nbody\n";
        let (agent, entry) = parse_agent("minimal", "c", md).unwrap().unwrap();
        assert_eq!(agent.description, "");
        assert!(agent.emoji.is_none());
        assert!(agent.color.is_none());
        assert!(agent.vibe.is_none());
        assert_eq!(entry.description, "");
    }
}
