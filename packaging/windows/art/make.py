"""Builds the Windows installer art and Luna's icon from the source images.

    cargo run --release -p luna-app --example installer_art -- <tmp>
    python packaging/windows/art/make.py <tmp>

Writes, next to this script:
  wizard-<w>x<h>.png  the wizard panel (Luna's sky and Moon), one per DPI
  small-<n>x<n>.png   the logo for the wizard's top corner, one per DPI
and Assets/luna-logo.ico (16 to 256 px) for Luna.exe, Setup and shortcuts.
Needs Pillow.
"""

import pathlib
import sys

from PIL import Image

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[2]
LOGO = ROOT / "Assets" / "luna-logo-lg.webp"

# Inno's small image area is 58 x 58 at 100%; these are 100% to 250%.
SMALL = [58, 73, 87, 102, 116, 131, 145]
ICON = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def square_logo(pad: float) -> Image.Image:
    """The logo centred on a transparent square, `pad` of the side each way."""
    logo = Image.open(LOGO).convert("RGBA")
    side = round(max(logo.size) / (1 - 2 * pad))
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.alpha_composite(logo, ((side - logo.width) // 2, (side - logo.height) // 2))
    return canvas


def main(renders: pathlib.Path) -> None:
    for ppm in sorted(renders.glob("wizard-*.ppm")):
        Image.open(ppm).save(HERE / (ppm.stem + ".png"), optimize=True)
    small = square_logo(0.08)
    for n in SMALL:
        small.resize((n, n), Image.LANCZOS).save(HERE / f"small-{n}x{n}.png", optimize=True)
    square_logo(0.02).save(ROOT / "Assets" / "luna-logo.ico", sizes=[(n, n) for n in ICON])
    print("ok")


if __name__ == "__main__":
    main(pathlib.Path(sys.argv[1]))
