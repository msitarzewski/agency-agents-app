## Agency Agents v0.1.0 - Unreleased

> Staging file. Rename to `docs/release-notes/<version>.md` when the next version is cut.

### Highlights

- Native app for browsing and installing agents from the `agency-agents` catalog.
- Catalog source picker supports bundled baseline, managed clone, and user-selected clone.
- Agents workspace combines catalog browsing, local install state, and per-tool deployment controls.
- Tools panel shows detected tools, install counts, project targets, and bulk operations.
- Dashboard charts summarize tool coverage, install health, and category distribution.
- Deterministic renderers for Claude Code, Codex, Gemini CLI, GitHub Copilot, Qwen Code, Cursor, and opencode.
- Install ledger and reconciliation classify files as current, outdated, modified, removed, or foreign.
- Modified-file uninstall backs up divergent files before deletion.
- macOS Liquid Glass icon path and cross-platform window chrome configuration.

### Quality Gates

- `cargo test --lib`
- `npm run check`
- `npm run build`
- ignored renderer parity test against the active `agency-agents` clone
- Phase C platform-config validation

### Known Release Notes

- The app is not an agent runtime.
- Some AA repo targets are not yet exposed as app installs because they need multi-file or aggregate renderer work.
- Windows/Linux builds need native runtime verification before support is claimed beyond build validation.
- **Windows: the build is not code-signed yet.** On first launch Microsoft Defender SmartScreen will show
  "Windows protected your PC" with an unknown-publisher warning. To run it: click **More info → Run anyway**.
  Code signing is deferred until there's enough Windows demand to justify a certificate (see
  `memory-bank/decisions.md` 2026-06-16). macOS builds are signed + notarized and launch without warnings.
