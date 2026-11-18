"""Composes docs/img/brand-preview.png from the generated brand files (visual QA sheet)."""
from PIL import Image, ImageDraw
B = 'assets/brand/'
W, H = 1600, 1180
sheet = Image.new('RGB', (W, H), '#0B0F19')
d = ImageDraw.Draw(sheet)

def fit(path, w, h):
    im = Image.open(path).convert('RGBA'); im.thumbnail((w, h), Image.LANCZOS); return im

def panel(x, y, w, h, bg, path, pad=40):
    d.rounded_rectangle((x, y, x + w, y + h), 18, fill=bg)
    im = fit(path, w - pad * 2, h - pad * 2)
    sheet.paste(im, (x + (w - im.width) // 2, y + (h - im.height) // 2), im)

panel(20, 20, 770, 260, '#F4F7FC', B + 'logo-light.png', 60)
panel(810, 20, 770, 260, '#101521', B + 'logo-dark.png', 60)
panel(20, 300, 370, 300, '#F4F7FC', B + 'symbol.png')
panel(410, 300, 370, 300, '#101521', B + 'symbol-dark.png')
panel(800, 300, 370, 300, '#E9EDF5', B + 'symbol-monochrome-black.png')
panel(1190, 300, 390, 300, '#202B40', B + 'symbol-monochrome-white.png')
panel(20, 620, 770, 200, '#E9EDF5', B + 'logo-monochrome-black.png', 50)
panel(810, 620, 770, 200, '#202B40', B + 'logo-monochrome-white.png', 50)

# App icon from the real .ico at native sizes + 4x nearest zoom of small ones
d.rounded_rectangle((20, 840, 1580, 1160), 18, fill='#1B2130')
ico = Image.open(B + 'app-icon.ico')
x = 50
for s in [256, 128, 48, 32, 24, 16]:
    ico.size = (s, s); im = ico.copy().convert('RGBA')
    sheet.paste(im, (x, 870 + (256 - s) // 2), im); x += s + 30
for s in [32, 16]:
    ico.size = (s, s); im = ico.copy().convert('RGBA').resize((s * 4, s * 4), Image.NEAREST)
    sheet.paste(im, (x, 870 + (256 - s * 4) // 2), im); x += s * 4 + 30
# Tray icon on dark & light taskbar
tray = Image.open('src-tauri/icons/tray-icon.png').convert('RGBA').resize((16, 16), Image.LANCZOS)
for i, bg in enumerate(['#1F1F1F', '#F3F3F3']):
    tx, ty = x + 10, 900 + i * 90
    d.rectangle((tx, ty, tx + 140, ty + 48), fill=bg)
    sheet.paste(tray, (tx + 16, ty + 16), tray)
    big = tray.resize((64, 64), Image.NEAREST); bgim = Image.new('RGBA', (64, 64), bg); bgim.alpha_composite(big)
    sheet.paste(bgim, (tx + 60, ty - 8 if i == 0 else ty - 8))
sheet.save('docs/img/brand-preview.png')
print('ok')
