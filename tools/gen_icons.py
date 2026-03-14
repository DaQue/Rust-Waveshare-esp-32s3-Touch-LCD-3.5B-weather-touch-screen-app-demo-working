#!/usr/bin/env python3
"""
Generate RGB565 BMP weather icons for the ESP32-S3 display.

Usage:
    python3 tools/gen_icons.py           # generate BMPs + preview PNGs
    python3 tools/gen_icons.py --preview # preview only, no BMP output

Output BMPs: src/icons/<name>_80.bmp  (80×80)
             src/icons/<name>_48.bmp  (48×48)
             src/icons/<name>_36.bmp  (36×36)
Preview PNGs: tools/icon_preview.png
              tools/icon_preview_small.png

ICON_BG (20,25,35) is written as the background and treated as transparent
by the Rust weather_icons.rs draw_bmp_transparent() function.
"""

import sys, struct, math, os
from PIL import Image, ImageDraw

PREVIEW_ONLY = "--preview" in sys.argv

OUT_DIR = os.path.join(os.path.dirname(__file__), "../src/icons")

# ── Palette ──────────────────────────────────────────────────────────────────
BG       = (27,  31,  39)   # device background for preview
ICON_BG  = (20,  25,  35)   # transparent key — must match weather_icons.rs

CL_BRIGHT    = (248, 250, 255)   # specular tip
CL_HIGHLIGHT = (225, 230, 242)   # upper highlight
CL_MID       = (190, 196, 210)   # mid-tone gradient layer
CL_BODY      = (152, 158, 170)   # main body
CL_SHADOW    = ( 88,  92, 104)   # drop shadow
CL_DARK      = ( 64,  68,  80)   # underside / bottom shading
CL_OUTLINE   = ( 44,  48,  58)   # outline

SUN_GLOW      = (255, 200,  48)
SUN_DISC      = (255, 225,  70)
SUN_RAY       = (255, 180,  32)
SUN_LIMB      = (210, 135,  18)   # warm dark underside / limb darkening
SUN_HIGHLIGHT = (255, 243, 145)   # upper-left highlight
SUN_BRIGHT    = (255, 255, 225)   # specular tip

RAIN_DROP = (100, 170, 240)
RAIN_HI   = (200, 230, 255)

SNOW_BODY   = (200, 224, 255)
SNOW_CENTER = (255, 255, 255)

BOLT        = (255, 220,  40)
BOLT_SHADOW = (180, 140,  20)

MIST_BODY      = (125, 135, 150)
MIST_HIGHLIGHT = (188, 196, 210)
MIST_SHADOW    = ( 68,  76,  90)

# ── Drawing helpers ───────────────────────────────────────────────────────────

def circle(draw, cx, cy, r, fill, outline=None, outline_w=1):
    draw.ellipse([cx-r, cy-r, cx+r, cy+r], fill=fill,
                 outline=outline, width=outline_w)

def draw_cloud(draw, cx, cy, scale=1.0):
    s = scale
    # 5 bubbles: 3-bubble base row + 2 upper bumps → fluffy silhouette
    bubbles = [
        (cx,            cy,            int(18*s)),   # center (largest)
        (cx-int(14*s),  cy+int(4*s),   int(13*s)),  # left
        (cx+int(14*s),  cy+int(4*s),   int(12*s)),  # right
        (cx-int(8*s),   cy-int(7*s),   int(10*s)),  # upper-left bump
        (cx+int(8*s),   cy-int(6*s),   int(10*s)),  # upper-right bump
    ]
    # Pass 1: diagonal drop shadows (slightly right + down for depth)
    for bx, by, br in bubbles:
        circle(draw, bx+int(1*s), by+int(3*s), br, CL_SHADOW)
    # Pass 2: dark underside band (clouds are flat/dark on the bottom)
    for bx, by, br in bubbles:
        circle(draw, bx, by+int(br*0.15), int(br*0.85), CL_DARK)
    # Pass 3: main body + outline
    for bx, by, br in bubbles:
        circle(draw, bx, by, br, CL_BODY, CL_OUTLINE, max(1, int(s)))
    # Pass 4: mid-tone gradient (upper 58% of each bubble)
    for bx, by, br in bubbles:
        hr = int(br * 0.58)
        circle(draw, bx-int(br*0.12), by-int(br*0.18), hr, CL_MID)
    # Pass 5: bright highlight (upper 34%)
    for bx, by, br in bubbles:
        hr = int(br * 0.34)
        circle(draw, bx-int(br*0.18), by-int(br*0.28), hr, CL_HIGHLIGHT)
    # Pass 6: specular tip (tiny, upper 15%)
    for bx, by, br in bubbles:
        hr = int(br * 0.15)
        if hr >= 1:
            circle(draw, bx-int(br*0.22), by-int(br*0.38), hr, CL_BRIGHT)

def draw_sun(draw, cx, cy, r, rays=8):
    ray_inner = int(r * 1.15)
    ray_outer = int(r * 1.65)
    ray_w     = max(2, int(r * 0.18))
    for i in range(rays):
        ang = math.radians(i * 360 / rays)
        x1 = cx + ray_inner * math.cos(ang)
        y1 = cy + ray_inner * math.sin(ang)
        x2 = cx + ray_outer * math.cos(ang)
        y2 = cy + ray_outer * math.sin(ang)
        draw.line([x1, y1, x2, y2], fill=SUN_RAY, width=ray_w)
    # Glow halo
    circle(draw, cx, cy, r+2, SUN_GLOW)
    # Limb darkening base (warm dark, full radius)
    circle(draw, cx, cy, r, SUN_LIMB)
    # Main disc offset upper-left — reveals dark crescent at lower-right
    circle(draw, cx - int(r*0.08), cy - int(r*0.10), int(r*0.92), SUN_DISC)
    # Highlight upper-left (~50% radius)
    hr = int(r * 0.50)
    circle(draw, cx - int(r*0.18), cy - int(r*0.22), hr, SUN_HIGHLIGHT)
    # Specular tip (~20% radius)
    sr = int(r * 0.20)
    if sr >= 1:
        circle(draw, cx - int(r*0.28), cy - int(r*0.32), sr, SUN_BRIGHT)

def draw_raindrops(draw, cx, cy, scale=1.0, count=3):
    s = scale
    offsets = [(-int(8*s), 0), (0, int(6*s)), (int(8*s), 0)][:count]
    for dx, dy in offsets:
        x, y = cx+dx, cy+dy
        rw, rh = int(5*s), int(8*s)
        draw.ellipse([x-rw, y-rh//2, x+rw, y+rh//2], fill=RAIN_DROP)
        draw.polygon([(x, y-rh), (x-rw+1, y-rh//3), (x+rw-1, y-rh//3)],
                     fill=RAIN_DROP)
        draw.ellipse([x-rw//3-1, y-rh//3, x+rw//3-1, y], fill=RAIN_HI)

def draw_snowflake(draw, cx, cy, r):
    arm_w = max(2, int(r * 0.18))
    for i in range(6):
        ang = math.radians(i * 60)
        x2 = cx + r * math.cos(ang)
        y2 = cy + r * math.sin(ang)
        draw.line([cx, cy, x2, y2], fill=SNOW_BODY, width=arm_w)
        for frac in (0.55, 0.85):
            mx = cx + r * frac * math.cos(ang)
            my = cy + r * frac * math.sin(ang)
            perp = ang + math.pi/2
            bar = r * 0.2
            draw.line([mx + bar*math.cos(perp), my + bar*math.sin(perp),
                       mx - bar*math.cos(perp), my - bar*math.sin(perp)],
                      fill=SNOW_BODY, width=arm_w)
    circle(draw, cx, cy, int(r*0.18), SNOW_CENTER)

def draw_bolt(draw, cx, cy, scale=1.0):
    s = scale
    pts = [
        (cx+int(4*s),  cy-int(16*s)),
        (cx-int(2*s),  cy-int(2*s)),
        (cx+int(4*s),  cy-int(2*s)),
        (cx-int(4*s),  cy+int(16*s)),
        (cx+int(2*s),  cy+int(2*s)),
        (cx-int(4*s),  cy+int(2*s)),
    ]
    draw.polygon(pts, fill=BOLT_SHADOW)
    inner = [(x-1, y+1) for x,y in pts]
    draw.polygon(inner, fill=BOLT)

def draw_mist_wisps(draw, cx, cy, scale=1.0, lines=3, body=MIST_BODY,
                    highlight=MIST_HIGHLIGHT, shadow=MIST_SHADOW):
    """Flat tapering ellipses — look like actual fog/mist wisps."""
    s = scale
    spacing = int(10*s)
    w  = int(30*s)   # half-width
    bh = max(3, int(5*s))  # half-height
    y0 = cy - spacing * (lines - 1) // 2
    for i in range(lines):
        y  = y0 + i * spacing
        ox = int(6*s) if i % 2 else 0   # horizontal offset for alternating wisps
        # Drop shadow
        draw.ellipse([cx-w+ox+2, y-bh+2, cx+w-ox+2, y+bh+2], fill=shadow)
        # Main wisp body
        draw.ellipse([cx-w+ox, y-bh, cx+w-ox, y+bh], fill=body)
        # Highlight — upper half of wisp, inset slightly
        draw.ellipse([cx-w+ox+4, y-bh, cx+w-ox-4, y], fill=highlight)

# ── Icon draw functions (centred at cx,cy; s=scale 1.0→80px) ─────────────────

def icon_clear(draw, cx, cy, s):
    draw_sun(draw, cx, cy, int(22*s))

def icon_few_clouds(draw, cx, cy, s):
    draw_sun(draw, cx+int(12*s), cy-int(12*s), int(16*s), rays=7)
    draw_cloud(draw, cx-int(4*s), cy+int(8*s), s*0.85)

def icon_scattered_clouds(draw, cx, cy, s):
    draw_cloud(draw, cx+int(10*s), cy-int(6*s), s*0.7)
    draw_cloud(draw, cx-int(6*s),  cy+int(8*s), s)

def icon_broken_clouds(draw, cx, cy, s):
    draw_cloud(draw, cx+int(8*s),  cy-int(8*s), s*0.65)
    draw_cloud(draw, cx-int(4*s),  cy+int(6*s), s)

def icon_overcast(draw, cx, cy, s):
    draw_cloud(draw, cx, cy-int(4*s), s*0.8)
    draw_cloud(draw, cx, cy+int(8*s), s)

def icon_shower_rain(draw, cx, cy, s):
    draw_cloud(draw, cx, cy-int(10*s), s)
    draw_raindrops(draw, cx, cy+int(18*s), s, count=3)

def icon_rain(draw, cx, cy, s):
    draw_cloud(draw, cx, cy-int(8*s), s)
    draw_raindrops(draw, cx-int(6*s), cy+int(16*s), s, count=2)
    draw_raindrops(draw, cx+int(6*s), cy+int(20*s), s, count=1)

def icon_drizzle(draw, cx, cy, s):
    draw_cloud(draw, cx, cy-int(10*s), s*0.9)
    for dx, dy in [(-12, 16), (-4, 20), (4, 16), (12, 20)]:
        x, y = cx+int(dx*s), cy+int(dy*s)
        rw, rh = int(3*s), int(5*s)
        draw.ellipse([x-rw, y-rh, x+rw, y+rh], fill=RAIN_DROP)

def icon_thunderstorm(draw, cx, cy, s):
    draw_cloud(draw, cx, cy-int(10*s), s)
    draw_bolt(draw, cx, cy+int(14*s), s)

def icon_snow(draw, cx, cy, s):
    draw_cloud(draw, cx, cy-int(10*s), s)
    draw_snowflake(draw, cx, cy+int(20*s), int(12*s))

def icon_atmosphere(draw, cx, cy, s):
    # 4 wisps, slight warm/purple-grey haze tint
    draw_mist_wisps(draw, cx, cy, s, lines=4,
                    body=(132, 128, 145), highlight=(192, 188, 205), shadow=(70, 68, 82))

def icon_mist(draw, cx, cy, s):
    # 3 wisps, lighter and more airy
    draw_mist_wisps(draw, cx, cy, s*0.9, lines=3,
                    body=(148, 155, 170), highlight=(205, 212, 226), shadow=(80, 87, 102))

def icon_fog(draw, cx, cy, s):
    # Dense ground fog: 4 wisps that get wider and darker toward bottom
    bh = max(3, int(5*s))
    y_top = cy - int(14*s)
    wisps = [
        (y_top,           int(22*s), 0,         (125, 133, 148), (183, 191, 205), (65, 73, 87)),
        (y_top+int(10*s), int(26*s), int(6*s),  (112, 120, 135), (168, 176, 190), (58, 66, 80)),
        (y_top+int(20*s), int(29*s), 0,         ( 98, 106, 120), (150, 158, 173), (50, 58, 72)),
        (y_top+int(30*s), int(32*s), int(4*s),  ( 84,  92, 106), (132, 140, 155), (42, 50, 64)),
    ]
    for y, w, ox, body, hi, sh in wisps:
        draw.ellipse([cx-w+ox+2, y-bh+2, cx+w-ox+2, y+bh+2], fill=sh)
        draw.ellipse([cx-w+ox,   y-bh,   cx+w-ox,   y+bh  ], fill=body)
        draw.ellipse([cx-w+ox+4, y-bh,   cx+w-ox-4, y     ], fill=hi)

ICONS = [
    ("clear",            icon_clear),
    ("few_clouds",       icon_few_clouds),
    ("scattered_clouds", icon_scattered_clouds),
    ("broken_clouds",    icon_broken_clouds),
    ("overcast",         icon_overcast),
    ("shower_rain",      icon_shower_rain),
    ("rain",             icon_rain),
    ("drizzle",          icon_drizzle),
    ("thunderstorm",     icon_thunderstorm),
    ("snow",             icon_snow),
    ("atmosphere",       icon_atmosphere),
    ("mist",             icon_mist),
    ("fog",              icon_fog),
]

# ── 24-bit RGB BMP writer (BI_RGB, no compression) ───────────────────────────
# tinybmp 0.6.0 does not support BI_BITFIELDS (16-bit) — use standard 24-bit.
# File size for 80×80: 54 + 80*80*3 = 19254 bytes (matches original icons).

def save_bmp_rgb24(img, path):
    w, h = img.size
    row_bytes = w * 3
    pad = (4 - (row_bytes % 4)) % 4
    padded = row_bytes + pad
    pixel_data_size = padded * h
    data_offset = 14 + 40  # 54 bytes, no color masks
    file_size = data_offset + pixel_data_size

    buf = bytearray()
    # File header
    buf += b'BM'
    buf += struct.pack('<I', file_size)
    buf += struct.pack('<HH', 0, 0)
    buf += struct.pack('<I', data_offset)
    # BITMAPINFOHEADER
    buf += struct.pack('<I', 40)
    buf += struct.pack('<i', w)
    buf += struct.pack('<i', -h)   # negative = top-down
    buf += struct.pack('<H', 1)    # planes
    buf += struct.pack('<H', 24)   # 24bpp
    buf += struct.pack('<I', 0)    # BI_RGB (no compression)
    buf += struct.pack('<I', pixel_data_size)
    buf += struct.pack('<ii', 0, 0)
    buf += struct.pack('<II', 0, 0)
    # Pixel data — BGR order
    pix = img.load()
    for y in range(h):
        for x in range(w):
            r, g, b = pix[x, y][:3]
            buf += struct.pack('BBB', b, g, r)
        buf += b'\x00' * pad

    with open(path, 'wb') as f:
        f.write(buf)

# ── Render one icon to an Image ───────────────────────────────────────────────

def render_icon(fn, size):
    img = Image.new("RGB", (size, size), ICON_BG)
    draw = ImageDraw.Draw(img)
    scale = size / 80.0
    fn(draw, size // 2, size // 2, scale)
    return img

# ── Preview sheet ─────────────────────────────────────────────────────────────

def make_preview():
    CELL, COLS, PAD, LABEL_H = 100, 7, 8, 14
    ROWS = math.ceil(len(ICONS) / COLS)
    W = COLS * CELL + PAD*2
    H = ROWS * (CELL + LABEL_H) + PAD*2
    img  = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(img)
    for i, (name, fn) in enumerate(ICONS):
        col, row = i % COLS, i // COLS
        x0 = PAD + col * CELL
        y0 = PAD + row * (CELL + LABEL_H)
        cx, cy = x0 + CELL//2, y0 + CELL//2
        draw.rounded_rectangle([x0+2, y0+2, x0+CELL-2, y0+CELL-2],
                                radius=8, fill=ICON_BG)
        fn(draw, cx, cy, 1.0)
        draw.text((cx, y0+CELL+2), name, fill=(140,148,160), anchor="mt")
    img.save("tools/icon_preview.png")
    print("Saved tools/icon_preview.png")

    SMALL = 50
    SW = len(ICONS)*SMALL + PAD*2
    simg  = Image.new("RGB", (SW, SMALL + PAD*2 + LABEL_H), BG)
    sdraw = ImageDraw.Draw(simg)
    for i, (name, fn) in enumerate(ICONS):
        x0 = PAD + i*SMALL
        cx, cy = x0 + SMALL//2, PAD + SMALL//2
        sdraw.rounded_rectangle([x0+1, PAD+1, x0+SMALL-1, PAD+SMALL-1],
                                 radius=6, fill=ICON_BG)
        fn(sdraw, cx, cy, 0.45)
    simg.save("tools/icon_preview_small.png")
    print("Saved tools/icon_preview_small.png")

# ── Main ──────────────────────────────────────────────────────────────────────

make_preview()

if not PREVIEW_ONLY:
    os.makedirs(OUT_DIR, exist_ok=True)
    for name, fn in ICONS:
        for size, suffix in [(80, "80"), (48, "48"), (36, "36")]:
            img  = render_icon(fn, size)
            path = os.path.join(OUT_DIR, f"{name}_{suffix}.bmp")
            save_bmp_rgb24(img, path)
            print(f"  {path}")
    print(f"Generated {len(ICONS)*3} BMPs in {OUT_DIR}/")
