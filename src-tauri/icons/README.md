# Icons

Generated from `source.svg` — a first-pass brand mark: orange (`#E54B1A`)
bracketed `B` on the brand black (`#04060A`), matching the in-app
header. "Real" brand artwork is a v0.2 task; this set at least means
Bifrost doesn't ship with the Tauri default logo.

## Regenerate the full set

The Tauri CLI takes a 1024×1024 PNG and produces every required size
(`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.ico`,
`icon.icns`, plus the Android / iOS sets if Bifrost ever ships there).

```sh
# 1. Rasterise source.svg → source.png at 1024×1024
#    Any SVG renderer works. A few common ones:
#
#    Inkscape:
#    inkscape source.svg --export-type=png --export-filename=source.png -w 1024
#
#    rsvg-convert:
#    rsvg-convert -w 1024 -h 1024 source.svg -o source.png
#
#    ImageMagick:
#    magick source.svg -resize 1024x1024 source.png
#
#    Or online: drop source.svg into https://cloudconvert.com/svg-to-png

# 2. Generate the full Tauri icon set (from the repo root)
cd ../..
pnpm tauri icon src-tauri/icons/source.png
```

The `pnpm tauri icon` command overwrites every file in this directory
except `source.svg` and this README.

## Replacing with proper brand artwork

When a designer hands over a finished mark:

1. Drop the new `source.png` (1024×1024, RGBA, transparent background
   preferred) over the existing one.
2. Re-run `pnpm tauri icon src-tauri/icons/source.png`.
3. Commit the regenerated PNG / ICO / ICNS set in one go.

If you want to keep an SVG as the canonical source, also update
`source.svg` and note the rasteriser used.
