#!/usr/bin/env bash
# Rebuilds every generated image under site/public/img from Luna's own renderers.
# usage: build-assets.sh <graded frames dir>     (make the frames with frames.sh)
# Needs: cargo, ffmpeg, and python with pymupdf (PYTHONPATH may point at a
# `pip install --target` directory).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
site=$(cd "$here/.." && pwd)
repo=$(cd "$site/.." && pwd)
frames=$1
img="$site/public/img"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target}"
capture() { cargo run --quiet --release --manifest-path "$here/capture/Cargo.toml" -- "$@"; }
webp() { ffmpeg -v error -y -i "$1" -c:v libwebp -quality "${3:-88}" "$2"; }

mkdir -p "$img/report"

# The app's window, 2x, and where the Moon and the text lines are in it.
capture screens "$tmp/screens"
for f in "$tmp"/screens/app-*.ppm; do
  webp "$f" "$img/$(basename "$f" .ppm).webp" 90
done
cp "$tmp/screens/layout.json" "$site/src/data/layout.json"

# The star chart behind the hero.
capture sky "$tmp/sky"
webp "$tmp/sky/sky.ppm" "$img/sky.webp" 82

# The lunar albedo the Moon shader samples, lossless.
capture albedo "$tmp/albedo"
ffmpeg -v error -y -i "$tmp/albedo/moon-albedo.pgm" -c:v libwebp -lossless 1 "$img/moon-albedo.webp"

# The sample report, and its pages as pictures.
capture report "$frames" "$repo/crates/luna-app/ui/assets/luna-icon.png" "$tmp/report"
python - "$tmp/report" "$img/report" <<'EOF'
import sys, pymupdf
src, out = sys.argv[1], sys.argv[2]
for theme in ("light", "dark"):
    doc = pymupdf.open(f"{src}/sample-{theme}.pdf")
    for i, page in enumerate(doc):
        page.get_pixmap(dpi=180).save(f"{out}/{theme}-p{i + 1}.png")
    # The first page again at 300 dpi, for a 1:1 crop of one clip row.
    doc[0].get_pixmap(dpi=300).save(f"{out}/{theme}-p1-300.png")
    print(theme, len(doc), "pages")
EOF
for theme in light dark; do
  ffmpeg -v error -y -i "$img/report/$theme-p1-300.png" -vf "crop=3250:350:130:1530" "$img/report/detail-$theme.png"
  rm "$img/report/$theme-p1-300.png"
done
for f in "$img"/report/*.png; do
  webp "$f" "${f%.png}.webp" 86
  rm "$f"
done

# Sharing card: the done window on the canvas colour.
ffmpeg -v error -y -f lavfi -i "color=c=0x0c0f14:s=1200x630" -i "$img/app-done@2x.webp" \
  -filter_complex "[1]scale=1000:-1[w];[0][w]overlay=100:70" -frames:v 1 "$img/og.png"

echo "assets in $img"
