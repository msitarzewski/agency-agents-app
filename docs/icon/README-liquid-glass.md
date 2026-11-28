# Agency Agents app icon

The approved artwork and exports come from `~/Software/icon-lab/current`.
The app records the installed release and exact file hashes in
[`.icon-release.json`](../../.icon-release.json).

## Apply an approved release

Run these commands on macOS from the app repository root.

```sh
python3 ~/Software/icon-lab/sync.py agency
bash docs/icon/build-icons.sh
python3 ~/Software/icon-lab/check.py . --strict
```

`sync.py` updates the Icon Composer source, Apple asset catalog, Tauri
fallback icons, light in-app icon, and app favicon. `build-icons.sh` uses
the same release to update the dark in-app icon and the landing page's
light and dark artwork, favicons, and install icons. Pass a pinned Icon Lab
release directory to `build-icons.sh` when updating from a specific release.

The in-app brand component follows the app's resolved theme. For the landing
pages, refresh the social cards with the matching artwork and update the icon
URLs' version parameter. The catalog site's light and dark PNGs can reuse the
app landing page's exports.

## Verify the macOS bundle

Build with `npm run tauri -- build --bundles app` using the normal local
signing configuration. Check that `Contents/Resources` contains both
`Assets.car` and `icon.icns`, and that `Contents/Info.plist` sets
`CFBundleIconName` to `AppIcon`. macOS Tahoe uses the layered asset catalog
for Liquid Glass appearances. Older macOS versions use the ICNS fallback.
