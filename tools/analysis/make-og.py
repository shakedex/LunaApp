# Disposable: composes apps/web/public/og.png (1200x630) = Luna logo centered
# on the app background color. Run once: python tools/analysis/make-og.py
from PIL import Image

W, H = 1200, 630
BG = (10, 10, 12, 255)  # #0a0a0c, matches --background

canvas = Image.new("RGBA", (W, H), BG)
logo = Image.open("Assets/luna-logo-lg.webp").convert("RGBA")
# scale logo to ~46% of height
target_h = int(H * 0.46)
scale = target_h / logo.height
logo = logo.resize((int(logo.width * scale), target_h), Image.LANCZOS)
canvas.alpha_composite(logo, ((W - logo.width) // 2, (H - logo.height) // 2))
canvas.convert("RGB").save("apps/web/public/og.png", "PNG")
print("wrote apps/web/public/og.png")
