# landing

Static landing page for Agency Agents, served at `agencyagents.app` via Caddy on the build host.

Same setup as the brew-browser landing: a self-contained static site (no build step) deployed with `rsync`.

## Files

- `index.html` — the page
- `style.css` — design tokens (dark-first, OKLCH, indigo accent)
- `agency-agents.png` — hero icon (Liquid Glass app icon)
- `favicon.ico`, `favicon-16x16.png`, `favicon-32x32.png`, `apple-touch-icon.png`, `icon-192.png`, `icon-512.png`
- `social-card.png` — 1200×630 Open Graph / Twitter card
- `manifest.json`, `robots.txt`, `sitemap.xml`

Icons are generated from `../docs/icon/agency-icon-*.png` via `sips`; `favicon.ico` is `../src-tauri/icons/icon.ico`.

> **TODO:** add an app screenshot to `screenshots/` and uncomment the `.section-shots`
> block in `index.html`. Capture the Dashboard (cross-tool coverage) for the strongest shot.

## Deploy

Set `DEPLOY_HOST` to your ssh alias for the build host (kept out of this repo).
From this directory:

```sh
rsync -avz --exclude README.md ./ "$DEPLOY_HOST":Sites/agency-agents/
```

> ⚠️ **Do NOT add a bare `--delete`.** Once auto-update is enabled this same web
> root also serves `updater.json` (and the signed updater artifacts), which do
> NOT live in this directory. A `--delete` sync from here would wipe the updater
> for every user. If you must prune stale landing files, add `--delete` **with**
> an `--exclude` for `updater.json` and any artifact paths.

Caddy config + DNS (`agencyagents.app`) are managed on the host.

## Update flow

1. Edit `index.html` / `style.css` locally
2. View locally: `python3 -m http.server -d . 8089` then open `http://localhost:8089`
3. `rsync` to the host when ready (command above)
4. Verify the page loads at `https://agencyagents.app/` (and, once auto-update ships,
   that `curl -s https://agencyagents.app/updater.json` still resolves)
