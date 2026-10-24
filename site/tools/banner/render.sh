#!/usr/bin/env bash
# Renders banner.html with headless Edge: the GitHub social preview into
# Assets/ and the site's sharing card into public/img/. Needs bun.
# usage: render.sh [browser]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
bun build "$here/sky-entry.ts" --outfile "$here/sky.js" --format iife >/dev/null
trap 'rm -f "$here/sky.js"' EXIT
bun "$here/shot.ts" 1280 640 "$here/../../../Assets/social-preview.png" "$@"
bun "$here/shot.ts" 1200 630 "$here/../../public/img/og.png" "$@"
