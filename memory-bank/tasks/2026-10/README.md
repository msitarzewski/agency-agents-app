# October 2026 — v0.3.2, signed Windows builds, a PR check, and no more silent catalog gaps

## Shipped to `main`
- **v0.3.2** (#110, 10-03) after renderer parity (#107), the tools.json sync (#108) and the 282-agent corpus refresh
  (#109); DMGs notarized and stapled (#111).
- **Windows signing** (#121): `scripts/release-windows.ps1` builds and signs both installers in the Windows VM with
  Azure Artifact Signing (`decisions.md` 2026-10-27). Prompted by agency-agents#1181 (Smart App Control).
- **`cargo fmt` + PR Check** (#122, replacing #100): the backend is formatted and `pr-check.yml` runs
  `cargo fmt --check` on every PR; #123 adds `.git-blame-ignore-revs` so blame skips the reformat.
- **Clone badge count** (#120): the Settings candidate badge counts nested agents like the indexer (#119).
- French locale (#118), landing icon (#112, #113), workflow actions pinned to SHAs (#116).

## Completed
### 2026-10-29: Report catalog files the app can't read (#79)
The indexer lists every file that looks like an agent but isn't indexed, with the reason; Settings → Catalog and
the sidebar say so. See [261029_catalog-skipped-files.md](./261029_catalog-skipped-files.md).

## Process notes worth remembering
- **Test a contributor PR by applying its reviewed diff to clean `main`**, not by checking out their branch: only
  code you've read runs.
- **After a batch of merges, test `main` itself** (fmt, `cargo test`, `npm run check`, parity), not just each PR.
- **Renderer parity needs a real catalog checkout** as `AGENCY_AGENTS_PARITY_ROOT`; `resources/corpus-baseline`
  has `convert.sh` but not the `lib.sh` it loads.
