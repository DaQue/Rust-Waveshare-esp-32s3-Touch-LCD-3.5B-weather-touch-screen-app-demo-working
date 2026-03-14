#!/usr/bin/env python3
"""
Preview weather icons on dark background before generating BMPs.
Run:  python3 tools/preview_icons.py
Output: tools/icon_preview.png
"""

from PIL import Image, ImageDraw
import math

# ── Palette ─────────────────────────────────────────────────────────────────
BG          = (27,  31,  39)   # device background (BG_NOW)
ICON_BG     = (20,  25,  35)   # transparent key in BMP (CARD_FILL_NOW)

# Cloud greys
CL_HIGHLIGHT = (228, 232, 240) # top/left bubble highlight (near white)
CL_BODY      = (160, 164, 172) # main cloud body (medium grey)
CL_SHADOW    = ( 96, 100, 110) # bottom/overlap shadow
CL_OUTLINE   = ( 48,  52,  60) # thin outer ring

# Sun
SUN_GLOW     = (255, 200,  48) # outer ring / rays
SUN_DISC     = (255, 228,  80) # inner bright disc
SUN_RAY      = (255, 180,  32) # ray tips

# Rain
RAIN_DROP    = (100, 170, 240) # drop body (lighter blue)
RAIN_HI      = (200, 230, 255) # highlight spot

# Snow / ice
SNOW_BODY    = (200, 224, 255) # crystal arms
SNOW_CENTER  = (255, 255, 255) # bright center

# Thunder
BOLT         = (255, 220,  40) # lightning bolt
BOLT_SHADOW  = (180, 140,  20)

# Atmosphere / mist
MIST_LINE    = (140, 148, 160) # horizontal haze bands

# ── Drawing helpers ──────────────────────────────────────────────────────────

def circle(draw, cx, cy, r, fill, outline=None, outline_w=1):
    draw.ellipse([cx-r, cy-r, cx+r, cy+r], fill=fill,
                 outline=outline, width=outline_w)

def draw_cloud(draw, cx, cy, scale=1.0, flip=False):
    """Three overlapping circles forming a cloud with highlight/shadow shading."""
    s = scale
    # bubble centres (relative to cloud centre)
    bubbles = [
        (cx,          cy,          int(18*s)),   # main centre
        (cx-int(14*s), cy+int(4*s), int(13*s)),  # left
        (cx+int(14*s), cy+int(4*s), int(12*s)),  # right
    ]
    # shadow pass first
    for bx, by, br in bubbles:
        circle(draw, bx, by+int(2*s), br, CL_SHADOW)
    # body pass
    for bx, by, br in bubbles:
        circle(draw, bx, by, br, CL_BODY, CL_OUTLINE, max(1, int(s)))
    # highlight pass (top-left quadrant of each bubble)
    for bx, by, br in bubbles:
        hr = int(br * 0.55)
        circle(draw, bx-int(br*0.25), by-int(br*0.25), hr, CL_HIGHLIGHT)

def draw_sun(draw, cx, cy, r, rays=8):
    """Sun disc with tapered rays."""
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
    # glow ring
    circle(draw, cx, cy, r+2, SUN_GLOW)
    # bright disc
    circle(draw, cx, cy, r, SUN_DISC)

def draw_raindrops(draw, cx, cy, scale=1.0, count=3):
    """Teardrop rain drops with highlight."""
    s = scale
    offsets = [(-int(8*s), 0), (0, int(6*s)), (int(8*s), 0)][:count]
    for dx, dy in offsets:
        x, y = cx+dx, cy+dy
        rw, rh = int(5*s), int(8*s)
        # body ellipse (lower part)
        draw.ellipse([x-rw, y-rh//2, x+rw, y+rh//2], fill=RAIN_DROP)
        # pointed top — triangle
        draw.polygon([(x, y-rh), (x-rw+1, y-rh//3), (x+rw-1, y-rh//3)],
                     fill=RAIN_DROP)
        # highlight
        draw.ellipse([x-rw//3-1, y-rh//3, x+rw//3-1, y], fill=RAIN_HI)

def draw_snowflake(draw, cx, cy, r):
    """6-armed snowflake."""
    arms = 6
    arm_w = max(2, int(r * 0.18))
    for i in range(arms):
        ang = math.radians(i * 360 / arms)
        x2 = cx + r * math.cos(ang)
        y2 = cy + r * math.sin(ang)
        draw.line([cx, cy, x2, y2], fill=SNOW_BODY, width=arm_w)
        # small cross-bars at 60% and 90%
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
    """Lightning bolt polygon."""
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

def draw_mist_lines(draw, cx, cy, scale=1.0, lines=3):
    """Horizontal haze bands."""
    s = scale
    spacing = int(8*s)
    w = int(28*s)
    y0 = cy - spacing
    for i in range(lines):
        y = y0 + i*spacing
        offs = int(4*s) if i % 2 else 0
        draw.rounded_rectangle([cx-w+offs, y-int(2*s), cx+w-offs, y+int(2*s)],
                                radius=int(2*s), fill=MIST_LINE)

# ── Icon definitions ─────────────────────────────────────────────────────────

def icon_clear(draw, cx, cy, s):
    draw_sun(draw, cx, cy, int(22*s))

def icon_few_clouds(draw, cx, cy, s):
    # Sun behind cloud (sun offset up-right)
    draw_sun(draw, cx+int(12*s), cy-int(12*s), int(16*s), rays=7)
    draw_cloud(draw, cx-int(4*s), cy+int(8*s), s*0.85)

def icon_scattered_clouds(draw, cx, cy, s):
    # Small cloud behind, larger in front
    draw_cloud(draw, cx+int(10*s), cy-int(6*s), s*0.7)
    draw_cloud(draw, cx-int(6*s), cy+int(8*s), s)

def icon_broken_clouds(draw, cx, cy, s):
    draw_cloud(draw, cx+int(8*s), cy-int(8*s), s*0.65)
    draw_cloud(draw, cx-int(4*s), cy+int(6*s), s)

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
    # lighter scattered drops
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
    draw_mist_lines(draw, cx, cy, s, lines=4)

def icon_mist(draw, cx, cy, s):
    draw_mist_lines(draw, cx, cy, s*0.9, lines=3)

def icon_fog(draw, cx, cy, s):
    draw_mist_lines(draw, cx, cy, s, lines=4)
    # extra faded bottom band
    draw.rounded_rectangle([cx-int(20*s), cy+int(16*s), cx+int(20*s), cy+int(20*s)],
                            radius=int(2*s), fill=(80, 88, 100))

ICONS = [
    ("clear",           icon_clear),
    ("few_clouds",      icon_few_clouds),
    ("scattered_clouds",icon_scattered_clouds),
    ("broken_clouds",   icon_broken_clouds),
    ("overcast",        icon_overcast),
    ("shower_rain",     icon_shower_rain),
    ("rain",            icon_rain),
    ("drizzle",         icon_drizzle),
    ("thunderstorm",    icon_thunderstorm),
    ("snow",            icon_snow),
    ("atmosphere",      icon_atmosphere),
    ("mist",            icon_mist),
    ("fog",             icon_fog),
]

# ── Layout & render ──────────────────────────────────────────────────────────

CELL = 100        # cell size in preview
COLS = 7
ROWS = math.ceil(len(ICONS) / COLS)
PAD  = 8
LABEL_H = 14

W = COLS * CELL + PAD*2
H = int(ROWS * (CELL + LABEL_H)) + PAD*2

img = Image.new("RGB", (W, H), BG)
draw = ImageDraw.Draw(img)

for i, (name, fn) in enumerate(ICONS):
    col = i % COLS
    row = i // COLS
    x0 = PAD + col * CELL
    y0 = PAD + row * (CELL + LABEL_H)
    cx = x0 + CELL // 2
    cy = y0 + CELL // 2

    # cell background (card colour)
    draw.rounded_rectangle([x0+2, y0+2, x0+CELL-2, y0+CELL-2],
                            radius=8, fill=ICON_BG)

    # draw icon at 80px scale (s=1.0) centred
    fn(draw, cx, cy, 1.0)

    # label
    draw.text((cx, y0+CELL+2), name, fill=(140,148,160), anchor="mt")

out = "tools/icon_preview.png"
img.save(out)
print(f"Saved {out}  ({W}×{H})")

# Also render a 36px strip for the small size
SMALL = 50
SW = len(ICONS) * SMALL + PAD*2
SH = SMALL + PAD*2 + LABEL_H
simg = Image.new("RGB", (SW, SH), BG)
sdraw = ImageDraw.Draw(simg)
for i, (name, fn) in enumerate(ICONS):
    x0 = PAD + i*SMALL
    cx = x0 + SMALL//2
    cy = PAD + SMALL//2
    sdraw.rounded_rectangle([x0+1, PAD+1, x0+SMALL-1, PAD+SMALL-1],
                             radius=6, fill=ICON_BG)
    fn(sdraw, cx, cy, 0.45)  # ~36/80 scale

sout = "tools/icon_preview_small.png"
simg.save(sout)
print(f"Saved {sout}  ({SW}×{SH})")
