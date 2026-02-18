#!/usr/bin/env python3
"""Generate 26 weather icon BMP files (13 conditions x 2 sizes) in RGB565 format.

Style: Monotone grey clouds with selective color accents.
  - Clouds: shades of grey (no color)
  - Sun: warm gold/amber
  - Rain/drizzle: cool blue
  - Snow: bright white
  - Lightning: vivid yellow
  - Mist/fog: subtle grey bars

Output: src/icons/
Format: 16-bit BMP with BI_BITFIELDS (RGB565)
"""

import math
import os
import struct
from PIL import Image, ImageDraw

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
OUT_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                       "src", "icons")

# ---------------------------------------------------------------------------
# Colour palette — monotone clouds, color accents only
# ---------------------------------------------------------------------------
# Clouds: pure grey tones (no blue/warm tint)
CLOUD_BRIGHT = (180, 180, 180, 255)   # lightest cloud
CLOUD_MID    = (140, 140, 140, 255)   # medium cloud
CLOUD_DARK   = (95, 95, 95, 255)      # storm cloud
CLOUD_DEEP   = (70, 70, 70, 255)      # darkest overcast

# Color accents
SUN_GOLD     = (255, 200, 40, 255)    # warm gold sun body
SUN_RAY      = (255, 180, 20, 255)    # slightly deeper rays
SUN_CENTER   = (255, 220, 100, 255)   # bright center highlight
RAIN_BLUE    = (70, 150, 255, 255)    # cool blue rain
ICE_BLUE     = (130, 200, 255, 255)   # lighter ice blue for drizzle
LIGHTNING_Y  = (255, 230, 50, 255)    # vivid yellow bolt
SNOW_WHITE   = (230, 235, 245, 255)   # bright snow
MIST_GREY    = (160, 160, 160, 120)   # semi-transparent mist
FOG_GREY     = (120, 120, 120, 180)   # denser fog

# Background — matches icon transparency key
BG           = (0x14, 0x19, 0x23, 255)

SIZES = [80, 36]

# ---------------------------------------------------------------------------
# Scaling helpers (design at 80px, scale for 36px)
# ---------------------------------------------------------------------------

def s(val, sz):
    return val * sz / 80.0

def si(val, sz):
    return int(round(s(val, sz)))

# ---------------------------------------------------------------------------
# Drawing helpers
# ---------------------------------------------------------------------------

def draw_sun(draw, cx, cy, r_body, r_ray, n_rays, sz):
    """Sun with smooth body and clean rays."""
    ray_w = max(1, si(2.5, sz))
    # Rays first (behind body)
    for i in range(n_rays):
        angle = 2 * math.pi * i / n_rays
        x1 = cx + math.cos(angle) * (r_body + s(2, sz))
        y1 = cy + math.sin(angle) * (r_body + s(2, sz))
        x2 = cx + math.cos(angle) * r_ray
        y2 = cy + math.sin(angle) * r_ray
        draw.line([(x1, y1), (x2, y2)], fill=SUN_RAY, width=ray_w)
    # Body
    draw.ellipse([cx - r_body, cy - r_body, cx + r_body, cy + r_body],
                 fill=SUN_GOLD)
    # Center highlight
    hr = r_body * 0.5
    draw.ellipse([cx - hr, cy - hr, cx + hr, cy + hr], fill=SUN_CENTER)


def draw_cloud(draw, cx, cy, scale, color, sz):
    """Cloud from overlapping ellipses — clean rounded shape."""
    sc = scale * sz / 80.0
    def e(x, y, rx, ry):
        draw.ellipse([cx + x*sc - rx*sc, cy + y*sc - ry*sc,
                       cx + x*sc + rx*sc, cy + y*sc + ry*sc], fill=color)
    # Three bumps for natural cloud shape
    e(-14, 2, 18, 13)
    e(6, -5, 20, 16)
    e(24, 2, 14, 11)
    # Base to merge bumps smoothly
    draw.rectangle([cx - 32*sc, cy + 2*sc, cx + 38*sc, cy + 14*sc], fill=color)


def draw_rain_drops(draw, cx, cy, count, length, sz, color=RAIN_BLUE):
    """Angled rain lines."""
    spacing = s(12, sz)
    start_x = cx - spacing * (count - 1) / 2
    w = max(1, si(2, sz))
    for i in range(count):
        x = start_x + i * spacing
        draw.line([(x, cy), (x - s(4, sz), cy + s(length, sz))],
                  fill=color, width=w)


def draw_lightning(draw, cx, cy, sz):
    """Bold zigzag bolt."""
    pts = [
        (cx + s(3, sz), cy),
        (cx - s(5, sz), cy + s(12, sz)),
        (cx + s(3, sz), cy + s(10, sz)),
        (cx - s(7, sz), cy + s(26, sz)),
    ]
    draw.line(pts, fill=LIGHTNING_Y, width=max(2, si(3, sz)))


def draw_snowflake(draw, cx, cy, r, sz):
    """Six-armed snowflake."""
    w = max(1, si(1.5, sz))
    for angle_deg in [0, 60, 120]:
        a = math.radians(angle_deg)
        dx = math.cos(a) * r
        dy = math.sin(a) * r
        draw.line([(cx - dx, cy - dy), (cx + dx, cy + dy)],
                  fill=SNOW_WHITE, width=w)


# ---------------------------------------------------------------------------
# Icon drawing functions (each takes draw, sz)
# ---------------------------------------------------------------------------

def icon_clear(draw, sz):
    cx, cy = sz / 2, sz / 2
    draw_sun(draw, cx, cy, s(17, sz), s(30, sz), 12, sz)


def icon_few_clouds(draw, sz):
    # Sun upper-left, cloud lower-right overlapping
    sun_cx, sun_cy = s(26, sz), s(26, sz)
    draw_sun(draw, sun_cx, sun_cy, s(13, sz), s(23, sz), 10, sz)
    draw_cloud(draw, s(44, sz), s(48, sz), 1.0, CLOUD_BRIGHT, sz)


def icon_scattered_clouds(draw, sz):
    # Two grey clouds at different depths
    draw_cloud(draw, s(26, sz), s(28, sz), 0.8, CLOUD_MID, sz)
    draw_cloud(draw, s(44, sz), s(44, sz), 1.05, CLOUD_BRIGHT, sz)


def icon_broken_clouds(draw, sz):
    draw_cloud(draw, s(30, sz), s(30, sz), 0.7, CLOUD_DARK, sz)
    draw_cloud(draw, s(42, sz), s(42, sz), 1.15, CLOUD_MID, sz)


def icon_overcast(draw, sz):
    draw_cloud(draw, s(40, sz), s(38, sz), 1.5, CLOUD_DARK, sz)


def icon_shower_rain(draw, sz):
    cloud_cy = s(28, sz)
    draw_cloud(draw, s(40, sz), cloud_cy, 1.1, CLOUD_MID, sz)
    draw_rain_drops(draw, s(40, sz), cloud_cy + s(16, sz), 3, 16, sz)


def icon_rain(draw, sz):
    cloud_cy = s(26, sz)
    draw_cloud(draw, s(40, sz), cloud_cy, 1.2, CLOUD_DARK, sz)
    draw_rain_drops(draw, s(40, sz), cloud_cy + s(16, sz), 5, 20, sz)


def icon_drizzle(draw, sz):
    cloud_cy = s(28, sz)
    draw_cloud(draw, s(40, sz), cloud_cy, 1.0, CLOUD_BRIGHT, sz)
    # Small blue dots instead of lines
    spacing = s(14, sz)
    start_x = s(40, sz) - spacing
    dot_y = cloud_cy + s(20, sz)
    r = max(1, s(2.5, sz))
    for i in range(3):
        x = start_x + i * spacing
        draw.ellipse([x - r, dot_y - r, x + r, dot_y + r], fill=ICE_BLUE)


def icon_thunderstorm(draw, sz):
    cloud_cy = s(24, sz)
    draw_cloud(draw, s(40, sz), cloud_cy, 1.2, CLOUD_DEEP, sz)
    draw_lightning(draw, s(40, sz), cloud_cy + s(16, sz), sz)
    # A couple rain drops for context
    draw_rain_drops(draw, s(26, sz), cloud_cy + s(16, sz), 2, 14, sz)


def icon_snow(draw, sz):
    cloud_cy = s(26, sz)
    draw_cloud(draw, s(40, sz), cloud_cy, 1.1, CLOUD_MID, sz)
    # Row of snowflakes
    spacing = s(14, sz)
    n = 4
    start_x = s(40, sz) - spacing * (n - 1) / 2
    flake_y = cloud_cy + s(24, sz)
    flake_r = s(4.5, sz)
    for i in range(n):
        draw_snowflake(draw, start_x + i * spacing, flake_y, flake_r, sz)


def icon_atmosphere(draw, sz):
    """Wavy horizontal lines — haze."""
    for i, y_off in enumerate([18, 32, 46, 60]):
        y = s(y_off, sz)
        pts = []
        step = max(1, si(2, sz))
        for x in range(0, sz + 1, step):
            pts.append((x, y + math.sin(x / s(10, sz) + i) * s(4, sz)))
        if len(pts) >= 2:
            draw.line(pts, fill=CLOUD_MID, width=max(1, si(2, sz)))


def icon_mist(draw, sz):
    """Light semi-transparent horizontal bars."""
    for y_off in [14, 26, 38, 50, 62]:
        y = s(y_off, sz)
        h = max(1, s(4, sz))
        draw.rectangle([s(8, sz), y, s(72, sz), y + h], fill=MIST_GREY)


def icon_fog(draw, sz):
    """Dense horizontal gray bars."""
    for y_off in [10, 20, 30, 40, 50, 60]:
        y = s(y_off, sz)
        h = max(1, s(6, sz))
        draw.rectangle([s(6, sz), y, s(74, sz), y + h], fill=FOG_GREY)


# ---------------------------------------------------------------------------
# Map names -> draw funcs
# ---------------------------------------------------------------------------
ICONS = {
    "clear":             icon_clear,
    "few_clouds":        icon_few_clouds,
    "scattered_clouds":  icon_scattered_clouds,
    "broken_clouds":     icon_broken_clouds,
    "overcast":          icon_overcast,
    "shower_rain":       icon_shower_rain,
    "rain":              icon_rain,
    "drizzle":           icon_drizzle,
    "thunderstorm":      icon_thunderstorm,
    "snow":              icon_snow,
    "atmosphere":        icon_atmosphere,
    "mist":              icon_mist,
    "fog":               icon_fog,
}

# ---------------------------------------------------------------------------
# 24-bit BMP writer (tinybmp handles these reliably)
# ---------------------------------------------------------------------------

def write_24bit_bmp(img, path):
    """Write an RGBA PIL Image as a standard 24-bit uncompressed BMP."""
    w, h = img.size
    row_bytes = w * 3
    row_pad = (4 - (row_bytes % 4)) % 4
    headers_size = 14 + 40
    pixel_data_size = (row_bytes + row_pad) * h
    file_size = headers_size + pixel_data_size

    pixels = img.load()

    with open(path, "wb") as f:
        # BMP File Header (14 bytes)
        f.write(b'BM')
        f.write(struct.pack('<I', file_size))
        f.write(struct.pack('<HH', 0, 0))
        f.write(struct.pack('<I', headers_size))
        # DIB Header (40 bytes) — BITMAPINFOHEADER
        f.write(struct.pack('<I', 40))
        f.write(struct.pack('<i', w))
        f.write(struct.pack('<i', h))
        f.write(struct.pack('<H', 1))       # planes
        f.write(struct.pack('<H', 24))      # bpp
        f.write(struct.pack('<I', 0))       # BI_RGB (no compression)
        f.write(struct.pack('<I', pixel_data_size))
        f.write(struct.pack('<i', 2835))
        f.write(struct.pack('<i', 2835))
        f.write(struct.pack('<I', 0))
        f.write(struct.pack('<I', 0))
        # Pixel data (bottom-up, BGR byte order per BMP spec)
        pad_bytes = b'\x00' * row_pad
        for y in range(h - 1, -1, -1):
            row_data = bytearray(row_bytes)
            for x in range(w):
                r, g, b, a = pixels[x, y]
                if a < 255:
                    af = a / 255.0
                    r = int(r * af + BG[0] * (1 - af))
                    g = int(g * af + BG[1] * (1 - af))
                    b = int(b * af + BG[2] * (1 - af))
                off = x * 3
                row_data[off]     = b  # BMP stores BGR
                row_data[off + 1] = g
                row_data[off + 2] = r
            f.write(row_data)
            if row_pad:
                f.write(pad_bytes)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    os.makedirs(OUT_DIR, exist_ok=True)

    count = 0
    for name, draw_fn in ICONS.items():
        for sz in SIZES:
            img = Image.new("RGBA", (sz, sz), BG)
            draw = ImageDraw.Draw(img, "RGBA")
            draw_fn(draw, sz)

            fname = f"{name}_{sz}.bmp"
            fpath = os.path.join(OUT_DIR, fname)
            write_24bit_bmp(img, fpath)
            count += 1
            file_size = os.path.getsize(fpath)
            print(f"  {fname:<30s}  {file_size:>6d} bytes")

    print(f"\nGenerated {count} icon files in {OUT_DIR}")


if __name__ == "__main__":
    main()
