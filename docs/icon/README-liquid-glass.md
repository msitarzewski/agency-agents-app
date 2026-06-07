# Liquid Glass app icon (macOS 26 Tahoe)

On macOS 26, the system renders app icons with **Liquid Glass** — the specular
edge highlight, depth, and Light/Dark/Clear/Tinted variants are applied *live by
the OS* from a **layered** icon authored in **Icon Composer** (`.icon` format).
A flat `.icns`/PNG (what `tauri icon` produces) is shown literally — no glass.

This folder has everything staged to give Agency Agents the native treatment.

## What's here

```
layers/
  Foreground.png         brain-circuit glyph, COLOR, transparent, 1024² (≈71% inset)
  Foreground-white.png   same glyph as a white silhouette (reads great on glass)
  Background.png         deep purple→teal gradient, 1024², full-bleed
  preview-flat.png       flat composite (reference only — NOT the glassed result)
AppIcon.icon/            a starter Icon Composer bundle (icon.json + Assets/)
```

`AppIcon.icon/icon.json` is hand-authored and **validates with `actool`** (zero
schema errors). It uses Background as the base layer and the colored glyph on
top. Swap in `Foreground-white.png` if you prefer the white glyph.

## Authoring (the 2-minute step — Icon Composer is installed)

Standalone `actool` won't compile a Liquid Glass `.icon` (Xcode 26 does it through
an internal build path the CLI doesn't expose), so author/export via the app:

1. Open **Icon Composer** (`/Applications/Xcode.app/Contents/Applications/Icon Composer.app`).
2. Either **open `docs/icon/AppIcon.icon`**, or **New → drag** `Background.png`
   then `Foreground.png` from `layers/` as two layers (background first).
3. Confirm it looks right (it renders the glass live). Tweak glyph scale/position,
   the background fill, and the Dark/Tinted variants if you like.
4. **Export** → choose the **`.icns`** export (Tahoe-compatible, carries the
   layered glass representation).

## Integration into the build

Replace the bundled icon with the Icon Composer export:

- Drop the exported icon at `src-tauri/icons/icon.icns`.
- `tauri build` ships it; macOS 26 renders the glass from it.
- If Icon Composer gives you a `.icon` (not `.icns`), keep it at
  `src-tauri/icons/AppIcon.icon` and add an `afterBundleCommand` that runs
  `actool` inside the Xcode build environment to emit `Assets.car` +
  `AppIcon.icns` into `…/Agency Agents.app/Contents/Resources/`, then set
  `CFBundleIconName = AppIcon` in the app's Info.plist. (Drop-in `.icns` is
  simpler — prefer it unless you need per-appearance variants.)

## Regenerating the layers from source art

Source: `brain-circuit.svg` (Affinity export). To rebuild the layers:

```sh
SVG="/path/to/brain-circuit.svg"
rsvg-convert -h 760 "$SVG" -o /tmp/g.png
magick -size 1024x1024 xc:none /tmp/g.png -gravity center -composite layers/Foreground.png
magick /tmp/g.png -channel RGB -evaluate set 100% +channel /tmp/gw.png
magick -size 1024x1024 xc:none /tmp/gw.png -gravity center -composite layers/Foreground-white.png
magick -size 1024x1024 gradient:'#4c1d95'-'#0e7490' layers/Background.png
```
