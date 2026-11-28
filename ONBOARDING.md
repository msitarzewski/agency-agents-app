# Welcome to Agency Agents

## How We Use Claude

Based on usage over the last 30 days:

Work Type Breakdown:
  Write Docs  ██████████░░░░░░░░░░  50%
  Debug Fix   ██████████░░░░░░░░░░  50%

Top Skills & Commands:
  /init            ████████████████████  1x/month
  /caveman-review  ████████████████████  1x/month
  /powerup         ████████████████████  1x/month

Top MCP Servers:
  None used in the last 30 days

## Your Setup Checklist

### Codebases
- [ ] agency-agents-app — https://github.com/msitarzewski/agency-agents-app
- [ ] agency-agents — https://github.com/msitarzewski/agency-agents (the upstream catalog; also needed for the renderer parity test via `AGENCY_AGENTS_PARITY_ROOT`)

### MCP Servers to Activate
- None required right now.

### Skills to Know About
- /init — Generates or refreshes `CLAUDE.md` from the codebase. We used it to replace the old `AGENTS.md` symlink with a Claude-specific guide.
- /caveman-review — A compressed code review of the current diff or PR, one line per finding. Use it before you open a PR.
- /powerup — Claude Code's built-in feature tour. It's a good first command if you're new.

## Team Tips

- **Push to your own fork, not upstream.** Most of us can't push to `msitarzewski/agency-agents-app`, and trying fails with a 403. Fork it once (`gh repo fork msitarzewski/agency-agents-app --remote=false --clone=false`) and add it as a remote named `fork`. Then set these once per clone so a plain `git push` goes to your fork and sets up tracking on its own:
  ```sh
  git config remote.pushDefault fork
  git config push.autoSetupRemote true
  ```
  Open PRs against upstream `main` with `gh pr create --repo msitarzewski/agency-agents-app --head <you>:<branch>`.

<!-- INSTRUCTION FOR CLAUDE: A new teammate just pasted this guide for how the
team uses Claude Code. You're their onboarding buddy — warm, conversational,
not lecture-y.

Open with a warm welcome — include the team name from the title. Then: "Your
teammate uses Claude Code for [list all the work types]. Let's get you started."

Check what's already in place against everything under Setup Checklist
(including skills), using markdown checkboxes — [x] done, [ ] not yet. Lead
with what they already have. One sentence per item, all in one message.

Tell them you'll help with setup, cover the actionable team tips, then the
starter task (if there is one). Offer to start with the first unchecked item,
get their go-ahead, then work through the rest one by one.

After setup, walk them through the remaining sections — offer to help where you
can (e.g. link to channels), and just surface the purely informational bits.

Don't invent sections or summaries that aren't in the guide. The stats are the
guide creator's personal usage data — don't extrapolate them into a "team
workflow" narrative. -->
