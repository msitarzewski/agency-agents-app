# 261105_liyan-mitra-office-preset

## Objective
Give the user (Liyan Mitra.co, a bookkeeping & tax services practice) an "office" inside the app:
a preset team staffed like a real bookkeeping and tax firm, deployable as one unit.

## Outcome
- ✅ `npm run check`: 0 errors, 0 warnings (324 files)
- ✅ `npm run build`: successful
- ⚠️ Not exercised in a running Tauri app (no desktop environment in the cloud session)
- ✅ Review: approved by the user; merged as **PR #1** on `ekafyudha-lang/agency-agents-app`
  (`main` @ `fe94cdc`, change commit `9c05aec`)

## Files Modified
- `src/lib/data/presetTeams.ts` — new `liyan-mitra-co` entry (Lucide `building-2`, `#0f766e`), 9 agents
- `src/lib/data/playbook.ts` — two `TEAM_EXAMPLES["liyan-mitra-co"]` starter prompts: client onboarding +
  month-end close, and an Indonesian PPh 21/23/4(2) / PPN / SPT compliance package
- `src/lib/i18n/locales/en.ts` — `preset.liyan-mitra-co.label` / `.description`

## Office roster (role → corpus slug)
| Role | Slug |
|---|---|
| Managing Partner | `chief-financial-officer` |
| Tax Consultant | `finance-tax-strategist` |
| Bookkeeping & Controller | `finance-bookkeeper-controller` |
| Accounts Payable | `accounts-payable-agent` |
| Financial Reporting Analyst | `finance-financial-analyst` |
| Compliance | `support-legal-compliance-checker` |
| Client Relations / Front Office | `customer-success-manager` |
| Office Operations | `operations-manager` |
| Document Desk | `specialized-document-generator` |

## Patterns Applied
- Extends the preset pattern documented at `src/lib/data/presetTeams.ts:1-11` (icon + color + verified slugs).
- Slugs are corpus file stems (`src-tauri/src/corpus/mod.rs:597`), so un-prefixed `specialized/*` files
  (e.g. `chief-financial-officer`) are referenced without a division prefix. All nine verified against
  `src-tauri/resources/corpus-baseline`.
- Only `en.ts` gets the keys: non-English locales are `Partial<Messages>` and fall back to English via
  `src/lib/stores/i18n.svelte.ts:66` (`local[key] ?? en[key] ?? fallback`).

## Integration Points
- `src/lib/components/Teams.svelte:401` and `src/lib/components/DeployBrowser.svelte:83` render every
  `PRESET_TEAMS` entry; no component changes needed.

## Architectural Decisions
- None. No new files or components; no `decisions.md` / `projectRules.md` entry needed.

## Related, outside the repo
- An interactive 3D office visual (8 rooms, 9 staff, animated client-file workflow) was published as a private
  claude.ai artifact: https://claude.ai/artifact/VfbVsdpExgQTXPSY9izJRU. Not part of the app or this repo.

## Artifacts
- PR: https://github.com/ekafyudha-lang/agency-agents-app/pull/1
