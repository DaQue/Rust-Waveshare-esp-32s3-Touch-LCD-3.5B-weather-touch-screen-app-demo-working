#!/usr/bin/env python3
"""
Generate a pixel-accurate mockup of the proposed NOW view layout.
Screen: 480x320 landscape
v2: bigger icon, bigger fonts, Indoor in right side of weather card, full-width forecast bottom.
"""
from PIL import Image, ImageDraw, ImageFont
import os, struct

# ── Colors (matching layout.rs) ──────────────────────────────────────────────
BG_NOW                   = (27,  31,  39)
LINE_COLOR_1             = (56,  63,  76)
CARD_FILL_NOW            = (20,  25,  35)
CARD_BORDER_NOW          = (63,  75,  95)
CARD_FILL_FORECAST_PREVIEW = (23, 29, 40)
CARD_BORDER_FORECAST_PREVIEW = (66, 86, 108)
TEXT_HEADER              = (222, 225, 230)
TEXT_STATUS              = (182, 187, 196)
TEXT_PRIMARY             = (232, 235, 240)
TEXT_SECONDARY           = (225, 228, 233)
TEXT_TERTIARY            = (188, 196, 208)
TEXT_DETAIL              = (184, 189, 198)
TEXT_CONDITION           = (166, 208, 255)
TEXT_BOTTOM              = (140, 148, 160)
DIVIDER_COLOR            = (56,  68,  88)

CARD_MARGIN = 8
CARD_RADIUS = 12
W, H = 480, 320

# ── Font helpers ─────────────────────────────────────────────────────────────
def find_font(size, bold=False):
    if bold:
        candidates = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSansBold.ttf",
        ]
    else:
        candidates = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ]
    for p in candidates:
        if os.path.exists(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()

f10  = find_font(10)
f11  = find_font(11)
f12  = find_font(12)
f13  = find_font(13)
f14  = find_font(14)
f16  = find_font(16)
f18  = find_font(18)
f40  = find_font(40)
f14b = find_font(14, bold=True)
f16b = find_font(16, bold=True)
f12b = find_font(12, bold=True)

# ── Draw helpers ──────────────────────────────────────────────────────────────
def draw_rounded_rect(draw, x, y, w, h, r, fill, border, border_w=1):
    draw.rounded_rectangle([x, y, x+w-1, y+h-1], radius=r, fill=fill, outline=border, width=border_w)

def text_center(draw, text, cx, y, font, color):
    bb = draw.textbbox((0, 0), text, font=font)
    tw = bb[2] - bb[0]
    draw.text((cx - tw//2, y), text, font=font, fill=color)

def text_right(draw, text, rx, y, font, color):
    bb = draw.textbbox((0, 0), text, font=font)
    tw = bb[2] - bb[0]
    draw.text((rx - tw, y), text, font=font, fill=color)

def load_bmp_as_rgba(path):
    """Load a 24-bit BGR BMP and return RGBA image with ICON_BG transparent."""
    ICON_BG = (20, 25, 35)
    try:
        img = Image.open(path).convert("RGB")
        data = img.load()
        rgba = Image.new("RGBA", img.size)
        rd = rgba.load()
        for py in range(img.height):
            for px in range(img.width):
                r, g, b = data[px, py]
                r5 = (r >> 3) << 3
                g6 = (g >> 2) << 2
                b5 = (b >> 3) << 3
                if (r5, g6, b5) == ((ICON_BG[0]>>3)<<3, (ICON_BG[1]>>2)<<2, (ICON_BG[2]>>3)<<3):
                    rd[px, py] = (0, 0, 0, 0)
                else:
                    rd[px, py] = (r, g, b, 255)
        return rgba
    except Exception:
        return None

# ── WiFi icon ─────────────────────────────────────────────────────────────────
def draw_wifi_icon(draw, cx, cy, color):
    for r, w in [(10,2),(7,2),(4,2)]:
        draw.arc([cx-r, cy-r, cx+r, cy+r], start=210, end=330, fill=color, width=w)
    draw.ellipse([cx-2, cy+2, cx+2, cy+6], fill=color)

# ── Main ──────────────────────────────────────────────────────────────────────
img  = Image.new("RGB", (W, H), BG_NOW)
draw = ImageDraw.Draw(img)

# ── Header ────────────────────────────────────────────────────────────────────
draw.text((10, 8), "10:42 AM", font=f14, fill=TEXT_HEADER)
text_center(draw, "St. Charles, MO", W//2, 8, f14, TEXT_HEADER)
draw_wifi_icon(draw, W-42, 14, TEXT_STATUS)
text_right(draw, "-36", W-8, 9, f11, TEXT_STATUS)
draw.line([(0, 30), (W, 30)], fill=LINE_COLOR_1, width=1)

# ── Weather card ──────────────────────────────────────────────────────────────
card_top = 36
card_h   = 150   # tall enough for 96px icon + margins
INDOOR_DIVIDER_X = 298   # vertical split inside card

draw_rounded_rect(draw, CARD_MARGIN, card_top, W - 2*CARD_MARGIN, card_h,
                  CARD_RADIUS, CARD_FILL_NOW, CARD_BORDER_NOW)

# Icon: scale 80px BMP → 96px
ICONS_DIR = os.path.join(os.path.dirname(__file__), "../src/icons")
icon = load_bmp_as_rgba(os.path.join(ICONS_DIR, "partly_cloudy_80.bmp"))
if icon is None:
    icon = load_bmp_as_rgba(os.path.join(ICONS_DIR, "few_clouds_80.bmp"))
if icon:
    icon = icon.resize((96, 96), Image.LANCZOS)
    img.paste(icon, (16, card_top + 8), icon)
else:
    draw.rectangle([16, card_top+8, 112, card_top+104], fill=(40,50,70))

# Temp (bigger font)
temp_x = 120
draw.text((temp_x, card_top + 8), "72°F", font=f40, fill=TEXT_PRIMARY)

# Trend triangle (rising — green), positioned right of temp text
bb = draw.textbbox((0, 0), "72°F", font=f40)
tx = temp_x + bb[2] - bb[0] + 8
ty = card_top + 30
draw.polygon([(tx, ty-10), (tx-9, ty+8), (tx+9, ty+8)], fill=(100,220,100))

# Feels like (bigger)
draw.text((temp_x, card_top + 60), "FEELS 69°", font=f18, fill=TEXT_CONDITION)

# Condition (bigger)
draw.text((temp_x, card_top + 84), "Partly Cloudy", font=f16, fill=TEXT_DETAIL)

# Vertical divider between weather info and indoor
draw.line([(INDOOR_DIVIDER_X, card_top+12), (INDOOR_DIVIDER_X, card_top+card_h-12)],
          fill=DIVIDER_COLOR, width=1)

# Indoor section (right side of card)
ind_x = INDOOR_DIVIDER_X + 12
draw.text((ind_x, card_top + 10), "Indoor", font=f16b, fill=TEXT_HEADER)
draw.text((ind_x, card_top + 36), "71.4°F", font=f18, fill=TEXT_PRIMARY)
draw.text((ind_x, card_top + 60), "43% RH", font=f14, fill=TEXT_TERTIARY)
draw.text((ind_x, card_top + 82), "1009.3 hPa", font=f14, fill=TEXT_TERTIARY)

# Hint row (bottom of card)
hint_y = card_top + card_h - 16
draw.text((CARD_MARGIN + 10, hint_y), "(tap temp = °F/°C)", font=f10, fill=TEXT_BOTTOM)
text_right(draw, "(tap icon = refresh)", W - CARD_MARGIN - 10, hint_y, f10, TEXT_BOTTOM)

# ── Bottom: full-width forecast ───────────────────────────────────────────────
split_y = card_top + card_h + 6   # 192
split_h = H - split_y - 16        # ~112 (leaves room for bottom hint)

draw_rounded_rect(draw, CARD_MARGIN, split_y, W - 2*CARD_MARGIN, split_h,
                  10, CARD_FILL_FORECAST_PREVIEW, CARD_BORDER_FORECAST_PREVIEW)

draw.text((CARD_MARGIN + 10, split_y + 7), "Forecast  ▶", font=f14b, fill=TEXT_HEADER)

# 4 forecast columns, full card width
days  = ["Mon", "Tue", "Wed", "Thu"]
highs = ["68°", "72°", "58°", "63°"]
fc_inner_w = W - 2*CARD_MARGIN
col_w = fc_inner_w // 4

icon_names = ["partly_cloudy_36.bmp", "clear_36.bmp", "drizzle_36.bmp", "few_clouds_36.bmp"]
icons36 = []
for name in icon_names:
    ic = load_bmp_as_rgba(os.path.join(ICONS_DIR, name))
    if ic is None:
        ic = load_bmp_as_rgba(os.path.join(ICONS_DIR, "few_clouds_36.bmp"))
    # Scale 36px → 44px for more visual weight
    if ic:
        ic = ic.resize((44, 44), Image.LANCZOS)
    icons36.append(ic)

for i in range(4):
    cx = CARD_MARGIN + i * col_w + col_w // 2
    text_center(draw, days[i], cx, split_y + 26, f13, TEXT_TERTIARY)
    if icons36[i]:
        img.paste(icons36[i], (cx - 22, split_y + 42), icons36[i])
    else:
        draw.rectangle([cx-22, split_y+42, cx+22, split_y+86], fill=(40,50,70))
    text_center(draw, highs[i], cx, split_y + 90, f16, TEXT_SECONDARY)

# ── Bottom hint ───────────────────────────────────────────────────────────────
text_center(draw, "(swipe ◀/▶ or tap header to switch pages)", W//2, H-11, f10, TEXT_BOTTOM)

# ── Save ──────────────────────────────────────────────────────────────────────
out = os.path.join(os.path.dirname(__file__), "../ws_s3_3p5_now_proposed_v2.png")
img.save(out)
print(f"Saved: {out}")
