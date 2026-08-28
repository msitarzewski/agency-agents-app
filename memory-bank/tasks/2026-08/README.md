# Tasks — 2026-08

## Completed

### 2026-08-28: Pi Subagent support

Added Pi as a dual-scope install target using native Subagent definitions:
`~/.pi/agent/agents/<slug>.md` globally and `.pi/agents/<slug>.md` per project. Reused the existing
safe Subagent renderer so Claude-specific `tools:` metadata cannot leak into Pi's tool allowlist.
The Tools UI uses Pi's official supplied SVG mark. Rust 273/0, production build green, actual
`pi-subagents` parser smoke-tested, and Tools UI verified.
See [260828_pi-subagent-support.md](./260828_pi-subagent-support.md).
