#!/bin/bash
# 原图缩成像素块：从四角按相似色漫水抠底、裁边、缩到 16 格、限色，加一圈透明边成 18×18；
# 横幅缩成 192×80 全幅。成品进 frontend/town-2d/src/assets/gameModes/。用法见 Artifact-AGI-Tool.md。
set -eu
ROOT=$(cd "$(dirname "$0")/.." && pwd)
RAW="$ROOT/resource-generation/raw"
OUT="$ROOT/frontend/town-2d/src/assets/gameModes"

icon() { # icon <raw-prefix> <mode/name>
  local src dst="$OUT/$2.png"
  src=$(ls "$RAW/$1"_*.jpg "$RAW/$1"_*.png 2>/dev/null | head -1)
  [ -n "${src:-}" ] || { echo "MISSING $1"; return 1; }
  mkdir -p "$(dirname "$dst")"
  bg=$(magick "$src" -format '%[pixel:p{4,4}]' info:)
  magick "$src" -alpha set -fuzz 20% -fill none \
    -draw 'alpha 0,0 floodfill' -draw 'alpha %[fx:w-1],0 floodfill' \
    -draw 'alpha 0,%[fx:h-1] floodfill' -draw 'alpha %[fx:w-1],%[fx:h-1] floodfill' \
    ${3:+-fuzz 20% -transparent "$bg"} \
    -channel A -threshold 30% +channel -trim +repage \
    -resize 16x16 -dither None -colors 14 \
    -gravity center -background none -extent 18x18 -strip "$dst"
  echo "ok $2"
}

for name in clash fell crate boat flare; do icon "tide-$name" "tide/$name"; done
for name in wolf seer witch guard villager fire ballot moon sun; do
  icon "werewolf-$name" "werewolf/$name"
done
icon werewolf-hunter werewolf/hunter keyall
for name in sword podium ballot bell drop switch suns; do
  icon "swordholder-$name" "swordholder/$name"
done
icon swordholder-antenna swordholder/antenna keyall

banner() { # banner <raw-prefix> <dst-suffix>
  local src dst="$OUT/$2.png"
  src=$(ls "$RAW/$1"_*.jpg "$RAW/$1"_*.png 2>/dev/null | head -1)
  [ -n "${src:-}" ] || { echo "MISSING $1"; return 1; }
  mkdir -p "$(dirname "$dst")"
  magick "$src" -resize '192x80!' -dither None -colors 24 -strip "$dst"
  echo "ok $2"
}

banner storyline-banner storyline/banner
banner storyline-lighthouse-banner 'storyline/灯塔之约/banner'
echo PROCESSED
