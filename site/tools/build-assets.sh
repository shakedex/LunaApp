#!/usr/bin/env bash
# Rebuilds every generated image under site/public/img from Luna's own renderers.
# usage: build-assets.sh <graded frames dir>   (make it with frames.sh)
# Needs: cargo, ffmpeg, python with pymupdf (PYTHONPATH may point at a --target install).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
site=$(cd "$here/.." && pwd)
repo=$(cd "$site/.." && pwd)
frames=$1
img="$site/public/img"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

capture() { cargo run --quiet --release --manifest-path "$here/capture/Cargo.toml" -- "$@"; }

capture screens "$tmp"
capture moon "$tmp"
capture report "$frames" "$repo/crates/luna-app/ui/assets/luna-icon.png" "$tmp/report"

webp() { ffmpeg -v error -y -i "$1" -c:v libwebp -quality "${3:-88}" "$2"; }

mkdir -p "$img/report"
for f in "$tmp"/app-*.ppm; do
  name=$(basename "$f" .ppm)
  webp "$f" "$img/$name.webp"
done
cp "$tmp/layout.json" "$img/layout.json"
ffmpeg -v error -y -i "$tmp/moon-still.pam" -c:v libwebp -lossless 0 -quality 90 -pix_fmt yuva420p "$img/moon-still.webp"

cp "$tmp/report/luna-sample.html" "$tmp/report/luna-sample-dark.html" "$tmp/report/luna-sample.csv" "$img/report/"
python - "$tmp/report" <<'EOF'
import sys, pymupdf
d = sys.argv[1]
pymupdf.open(f"{d}/luna-sample.pdf")[0].get_pixmap(dpi=200).save(f"{d}/report-light-p1.png")
pymupdf.open(f"{d}/luna-sample-dark.pdf")[1].get_pixmap(dpi=200).save(f"{d}/report-dark-p2.png")
EOF
webp "$tmp/report/report-light-p1.png" "$img/report-light-p1.webp" 86
webp "$tmp/report/report-dark-p2.png" "$img/report-dark-p2.webp" 86

# The frame strips the set piece flies out of the window.
mkdir -p "$img/strips"
for clip in P1066462 P1066468 P1066471; do
  for i in 0 1 2; do
    webp "$frames/${clip}_$i.jpg" "$img/strips/${clip}_$i.webp" 82
  done
done
echo "assets in $img"
