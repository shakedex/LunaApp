"""Prototype of Luna's engraved moon, for look development. Runs in
Blender's Python (for numpy and image I/O):

    blender -b --factory-startup -P tools/moon/engrave_proto.py -- <out dir> [size]

Writes still-*.png at several phases and, with SPIN_FRAMES set, spin-*.png
for a turntable. The app renders the same model live in Rust.

The look: a banknote-style line engraving of the real Moon (NASA's CGI Moon
Kit albedo, NASA's Scientific Visualization Studio).
- Lines: evenly spaced, bowed slightly by the sphere's depth so they wrap
  the form, thickening with the light on the surface.
- Cross-hatching: a second set across shadowed terrain near the terminator.
- Night side: a sparse stipple of earthshine in Luna blue.
- Halo: a soft glow off the lit limb.
- Orbit ring: an engraved dial of 72 ticks that light up with progress.
"""

import math
import os
import sys

import bpy
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
args = sys.argv[sys.argv.index("--") + 1 :]
OUT = args[0]
SIZE = int(args[1]) if len(args) > 1 else 480
SS = 2  # supersampling per axis
os.makedirs(OUT, exist_ok=True)

BG = np.array([0x0C, 0x0F, 0x14]) / 255.0
MOONLIGHT = np.array([0xEC, 0xEF, 0xF4]) / 255.0
LUNA_BLUE = np.array([0x79, 0xCA, 0xEA]) / 255.0
DIM_BLUE = np.array([0x2A, 0x4A, 0x5C]) / 255.0


def load_albedo():
    img = bpy.data.images.load(os.path.join(HERE, "lroc_color_2k.jpg"))
    img.colorspace_settings.name = "Non-Color"
    w, h = img.size
    px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)[::-1]
    lum = (px[..., :3] @ np.array([0.2126, 0.7152, 0.0722])) ** 2.2
    return lum / np.percentile(lum, 99.5)


ALBEDO = load_albedo()


def sample(lon, lat):
    h, w = ALBEDO.shape
    u = ((lon / (2 * math.pi)) + 0.5) % 1.0 * w - 0.5
    v = (0.5 - lat / math.pi) * h - 0.5
    x0 = np.floor(u).astype(int)
    y0 = np.clip(np.floor(v).astype(int), 0, h - 2)
    fx, fy = u - x0, np.clip(v - y0, 0, 1)
    x0 %= w
    x1 = (x0 + 1) % w
    a = ALBEDO[y0, x0] * (1 - fx) + ALBEDO[y0, x1] * fx
    b = ALBEDO[y0 + 1, x0] * (1 - fx) + ALBEDO[y0 + 1, x1] * fx
    return a * (1 - fy) + b * fy


def lines(param, per_px, width):
    """Anti-aliased lines at integer values of `param`; `per_px` is how many
    pixels one period spans; `width` is the share of the period inked."""
    dist = np.abs(param - np.round(param))
    edge = np.clip((width * 0.5 - dist) * per_px + 0.5, 0, 1)
    return edge * np.clip(width * per_px, 0, 1)


def hash01(ix, iy):
    h = (ix * 374761393 + iy * 668265263) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return (h & 0xFFFF) / 65535.0


def render(lit_fraction, spin, progress, size=SIZE):
    n = size * SS
    r = n * 0.36
    c = n / 2
    yy, xx = np.mgrid[0:n, 0:n].astype(np.float64) + 0.5
    vx = (xx - c) / r
    vy = (c - yy) / r
    rr = np.sqrt(vx * vx + vy * vy)
    inside = rr < 1.0
    vz = np.sqrt(np.clip(1.0 - rr * rr, 0, 1))

    # The moon leans a little; work in its frame.
    lean = math.radians(-16.0)
    cl, sl = math.cos(lean), math.sin(lean)
    sx = cl * vx - sl * vy
    sy = sl * vx + cl * vy
    cs, sn = math.cos(spin), math.sin(spin)
    bx = cs * sx + sn * vz
    bz = -sn * sx + cs * vz
    lon = np.arctan2(bx, bz)
    lat = np.arcsin(np.clip(sy, -1, 1))
    albedo = sample(lon, lat)
    # More contrast between the seas and the highlands, as an engraver would.
    albedo = np.clip((albedo - 0.18) / 0.82, 0, 1) ** 1.35

    phi = math.acos(1.0 - 2.0 * lit_fraction)
    s = (math.sin(phi), 0.0, -math.cos(phi))
    mu0 = vx * s[0] + vy * s[1] + vz * s[2]
    ls = np.where(mu0 > 0, 2.0 * mu0 / (mu0 + vz + 1e-6), 0.0)
    light = np.clip(albedo * ls * 0.62, 0, 1)

    period = 5.2 * SS  # pixels between lines at the centre
    per_px = period
    # Main lines: along the moon's frame, bowed by depth so they wrap the form.
    main = (sy * r + vz * 0.10 * r) / period
    main_ink = lines(main, per_px, np.clip(light, 0, 1) ** 0.85 * 0.78)

    # Cross-hatch across darker lit terrain near the terminator.
    ang = math.radians(58.0)
    cross_axis = math.cos(ang) * sx + math.sin(ang) * sy
    cross = (cross_axis * r + vz * 0.08 * r) / (period * 1.15)
    shade = np.clip((0.09 - light) / 0.09, 0, 1) * (mu0 > 0.0)
    cross_ink = lines(cross, per_px * 1.15, shade * 0.3) * np.clip(light * 10, 0, 1) * 0.8

    # Night side: sparse stipple in the dark, denser where the albedo is high.
    cell = 4.0 * SS
    ix = np.floor(xx / cell).astype(np.int64)
    iy = np.floor(yy / cell).astype(np.int64)
    jx, jy = hash01(ix, iy), hash01(iy + 91, ix + 17)
    dx = xx - (ix + 0.2 + 0.6 * jx) * cell
    dy = yy - (iy + 0.2 + 0.6 * jy) * cell
    dot = np.clip(1.2 * SS - np.sqrt(dx * dx + dy * dy), 0, 1)
    keep = hash01(ix + 5, iy + 3) < (0.015 + 0.06 * albedo) * (mu0 <= 0)
    stipple = dot * keep

    img = np.ones((n, n, 3)) * BG
    surface = np.clip(main_ink + cross_ink, 0, 1) * inside
    img += (MOONLIGHT - BG) * surface[..., None]
    img += (LUNA_BLUE - BG) * (stipple * 0.35 * inside)[..., None]

    # Limb hairline and a halo off the lit side.
    limb = np.clip(1.0 - np.abs(rr - 1.0) * r / (0.7 * SS), 0, 1)
    lit_side = np.clip((vx * s[0] + vy * s[1]) / np.maximum(rr, 1e-6) * 0.5 + 0.5, 0, 1) ** 2
    img += (DIM_BLUE - BG) * (limb * 0.8)[..., None]
    halo = np.exp(-np.clip(rr - 1.0, 0, None) * 9.0) * (rr >= 1.0) * lit_side * lit_fraction ** 0.5
    img += (MOONLIGHT * 0.5 + LUNA_BLUE * 0.5 - BG) * (halo * 0.10)[..., None]

    # Orbit ring: 72 engraved ticks; those up to `progress` light in Luna blue.
    ring_r = 1.17
    theta = (np.arctan2(vx, vy) / (2 * math.pi)) % 1.0  # 0 at top, clockwise
    tick = np.abs(theta * 72 - np.round(theta * 72)) * (2 * math.pi * ring_r * r / 72)
    in_band = (rr > ring_r - 0.035) & (rr < ring_r + 0.035)
    tick_ink = np.clip(1.0 * SS - tick, 0, 1) * in_band
    hair = np.clip(1.0 - np.abs(rr - ring_r) * r / (0.6 * SS), 0, 1)
    done = theta <= progress
    img += (DIM_BLUE - BG) * (hair * 0.45 + tick_ink * (~done) * 0.55)[..., None]
    img += (LUNA_BLUE - BG) * (tick_ink * done)[..., None]

    img = np.clip(img, 0, 1)
    return img.reshape(size, SS, size, SS, 3).mean(axis=(1, 3))


def save(path, rgb):
    h, w, _ = rgb.shape
    img = bpy.data.images.new(os.path.basename(path), w, h, alpha=False)
    rgba = np.concatenate([rgb, np.ones((h, w, 1))], axis=2)[::-1]
    img.pixels = rgba.astype(np.float32).ravel()
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()


for lit, prog in ((0.08, 0.0), (0.35, 0.3), (0.65, 0.62), (1.0, 1.0)):
    save(os.path.join(OUT, f"still-{int(lit * 100):03d}.png"), render(lit, 0.0, prog))
frames = int(os.environ.get("SPIN_FRAMES", "0"))
for i in range(frames):
    save(os.path.join(OUT, f"spin-{i:03d}.png"), render(0.6, 2 * math.pi * i / frames, 0.55, 360))
print("proto done")
