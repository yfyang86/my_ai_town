# Artifact-AGI-Tool

The game-mode art pipeline: AI-generated source images (MiniMax via the `mmx` CLI) turned into
the pixel tiles the HD-2D client serves as per-mode, per-script resources.

## Layout

```
resource-generation/
  generate.sh        # prompts -> raw/ (mmx image-01, 512x512 icons, 1536x640 banners)
  process.sh         # raw/ -> frontend/town-2d/src/assets/gameModes/ (ImageMagick)
  raw/               # generated source images, one <mode>-<name>_001.jpg each; kept so
                     # re-processing never needs the model again
  review/            # montage sheets of the current set for eyeballing
Artifact-AGI-Tool.md # this file
```

Output tiles land in `frontend/town-2d/src/assets/gameModes/<mode>/<name>.png`, with script packs
one level deeper (`storyline/灯塔之约/banner.png`). At runtime `gameModeArt(mode, name, pack?)`
(`frontend/town-2d/src/map/hd2d/gameModes/art.ts`) resolves pack first, then the mode default;
anything missing falls back to the icons drawn in code (`pixelIcons.ts`), so assets are always
optional. See `docs/wiki/game-modes.md` → "Generated art".

## Requirements

- `mmx` CLI authenticated (`mmx auth status`); image model `image-01`.
- ImageMagick 7 (`magick` on PATH).

## Usage

```bash
bash resource-generation/generate.sh   # fill in missing raw images (existing ones are skipped)
bash resource-generation/process.sh    # rebuild all tiles into the frontend assets
```

- **Regenerate one image:** delete its file under `raw/`, rerun both scripts.
- **Add an icon:** add a `gen <mode>-<name> "<prompt>"` line to `generate.sh` and a matching
  `icon <mode>-<name> <mode>/<name>` line to `process.sh`.
- **Add a script pack** (art that switches in when a storyline script is chosen): generate a wide
  image, then in `process.sh` add `banner <raw-prefix> 'storyline/<剧本名>/banner'`; the pack
  directory name must equal the script's `title`.
- **Review the set:** `magick montage frontend/town-2d/src/assets/gameModes/*/*.png \
  -background '#888' -geometry 90x90+4+4 -tile 8x out.png`

## Conventions

- **Prompt style** (`STYLE` in `generate.sh`): pixel-art game icon, flat colors, thick near-black
  outline, no shading/gradients/text, one centered object on a solid magenta (#ff00ff) background —
  magenta is keyed out later, so never let the subject contain it. Banners are full-bleed scenes
  (no keying).
- **Icon recipe** (`process.sh`): flood-fill transparency from the four corners at 20% fuzz
  (connected background only), alpha threshold, trim, scale into a 16-cell grid, quantize to 14
  colors, then pad with a 1-cell transparent margin → 18×18, the same canvas `pixelIcons.ts`
  paints. Icons whose subject encloses background pockets (lattice, bowstrings) get the `keyall`
  flag, which additionally keys the sampled corner color globally — only safe when subject colors
  are far from the background.
- **Banner recipe:** scale to 192×80 full-bleed, quantize to 24 colors, no transparency.
- **Determinism:** `-strip` removes timestamp chunks, so re-running `process.sh` on unchanged
  raws is byte-identical.
- Raw images are committed. They are the source of truth; the model is only needed for new or
  replaced art.
