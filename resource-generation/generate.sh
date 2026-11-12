#!/bin/bash
# 生成玩法素材原图：mmx image-01，统一像素图标风格（平涂、近黑描边、品红底方便抠图）。
# 已有的原图跳过；删掉 resource-generation/raw/ 里某张再跑就会重新生成它。用法见 Artifact-AGI-Tool.md。
set -u
ROOT=$(cd "$(dirname "$0")/.." && pwd)
RAW="$ROOT/resource-generation/raw"
mkdir -p "$RAW"
STYLE="pixel art game icon, retro 16-bit style, flat colors, thick near-black outline, no shading, no gradients, single centered object, plain solid magenta background, hex ff00ff background, no text, no letters"

gen() { # gen <name> <prompt>
  local name="$1" prompt="$2"
  if ls "$RAW/$name".* >/dev/null 2>&1; then echo "skip $name"; return 0; fi
  echo "== $name"
  mmx image generate --prompt "$prompt, $STYLE" --width 512 --height 512 \
    --out-dir "$RAW" --out-prefix "$name" --non-interactive --quiet || echo "FAIL $name"
}

gen tide-clash "a bursting explosion starburst, yellow core with orange and red spikes"
gen tide-fell "a white skull, front view, simple and clean"
gen tide-crate "a wooden supply crate box with rope tied around it"
gen tide-boat "a small white sailboat with brown wooden hull sailing on blue waves"
gen tide-flare "a handheld red signal flare stick with white band, sparks flying from the top"

gen werewolf-wolf "a grey wolf head in side profile, red eyes, open jaw"
gen werewolf-seer "a blue crystal ball with small stars inside, on a wooden base"
gen werewolf-witch "a purple glass potion bottle with a cork, bubbling"
gen werewolf-hunter "a wooden crossbow with iron bolt"
gen werewolf-guard "a round iron shield with a simple emblem"
gen werewolf-villager "a simple straw farmer hat"
gen werewolf-fire "a small campfire with orange flames and logs"
gen werewolf-ballot "a wooden ballot box with a paper voting slip half inserted"
gen werewolf-moon "a yellow crescent moon"
gen werewolf-sun "a round yellow sun with short rays"

gen swordholder-sword "a long silver sword with golden guard and brown grip, pointing straight up"
gen swordholder-podium "a wooden speech podium with a small microphone"
gen swordholder-ballot "a dark wooden ballot box with a paper slip half inserted"
gen swordholder-bell "a brass bell with a small rope"
gen swordholder-drop "a sleek mirror-like silver teardrop shaped spaceship, smooth droplet"
gen swordholder-antenna "a giant radio telescope antenna dish on a tower"
gen swordholder-switch "a big red push button on a dark metal panel"
gen swordholder-suns "three suns in one sky, one yellow one orange one red"

echo "== banners"
if [ ! -f "$RAW/storyline-banner-0.png" ] && ! ls "$RAW/storyline-banner".* >/dev/null 2>&1; then
  mmx image generate --prompt "pixel art wide banner, an open storybook and a quill pen under a starry night sky, flat colors, thick near-black outline, no shading, no text, no letters" \
    --width 1536 --height 640 --out-dir "$RAW" --out-prefix storyline-banner --non-interactive --quiet || echo "FAIL storyline-banner"
fi
if ! ls "$RAW/storyline-lighthouse-banner".* >/dev/null 2>&1; then
  mmx image generate --prompt "pixel art wide banner, a white lighthouse on a rocky shore at dusk, warm beam of light sweeping over a dark blue sea, gulls, flat colors, thick near-black outline, no shading, no text, no letters" \
    --width 1536 --height 640 --out-dir "$RAW" --out-prefix storyline-lighthouse-banner --non-interactive --quiet || echo "FAIL storyline-lighthouse-banner"
fi
echo DONE
