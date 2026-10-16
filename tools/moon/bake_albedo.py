"""Bakes NASA's lunar albedo into the raw map Luna's moon renderer reads.

    blender -b --factory-startup -P tools/moon/bake_albedo.py -- <output file>

Source: lroc_color_2k.jpg from NASA's CGI Moon Kit (NASA's Scientific
Visualization Studio). Output: 1024x512 little-endian u16, row 0 at the
north pole, column 0 at longitude -180, values 0..65535 for albedo 0..1 with
the engraving contrast already applied.
"""

import os
import sys

import bpy
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
out = sys.argv[sys.argv.index("--") + 1]

img = bpy.data.images.load(os.path.join(HERE, "lroc_color_2k.jpg"))
img.colorspace_settings.name = "Non-Color"
w, h = img.size
px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)[::-1]  # north first
lum = (px[..., :3] @ np.array([0.2126, 0.7152, 0.0722])) ** 2.2
lum = lum / np.percentile(lum, 99.5)
# Engraving contrast: more separation between seas and highlands.
albedo = np.clip((lum - 0.18) / 0.82, 0, 1) ** 1.35
# Halve to 1024x512 by averaging 2x2 blocks.
albedo = albedo.reshape(h // 2, 2, w // 2, 2).mean(axis=(1, 3))
data = np.round(np.clip(albedo, 0, 1) * 65535).astype("<u2")
os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
data.tofile(out)
print("baked", data.shape, os.path.getsize(out))
