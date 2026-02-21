#!/usr/bin/env python3
import math
import os
import struct
from typing import Callable, Dict, List, Tuple

Color = Tuple[int, int, int]

BG: Color = (20, 25, 35)  # #141923
SUN: Color = (255, 197, 61)
SUN_SOFT: Color = (255, 219, 128)
CLOUD_LIGHT: Color = (214, 223, 235)
CLOUD_MID: Color = (176, 190, 208)
CLOUD_DARK: Color = (126, 142, 164)
RAIN: Color = (91, 173, 255)
RAIN_HEAVY: Color = (58, 146, 235)
STORM: Color = (255, 224, 88)
SNOW: Color = (230, 244, 255)
FOG_LIGHT: Color = (152, 170, 192)
FOG_DARK: Color = (102, 122, 148)

ICON_KEYS = [
    "clear",
    "few_clouds",
    "scattered_clouds",
    "broken_clouds",
    "overcast",
    "shower_rain",
    "rain",
    "drizzle",
    "thunderstorm",
    "snow",
    "atmosphere",
    "mist",
    "fog",
]


class Canvas:
    def __init__(self, width: int, height: int, bg: Color):
        self.w = width
        self.h = height
        self.pixels: List[List[Color]] = [[bg for _ in range(width)] for _ in range(height)]

    def set_px(self, x: int, y: int, color: Color):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.pixels[y][x] = color

    def fill_rect(self, x: int, y: int, w: int, h: int, color: Color):
        x0 = max(0, x)
        y0 = max(0, y)
        x1 = min(self.w, x + w)
        y1 = min(self.h, y + h)
        for yy in range(y0, y1):
            row = self.pixels[yy]
            for xx in range(x0, x1):
                row[xx] = color

    def fill_circle(self, cx: int, cy: int, r: int, color: Color):
        r2 = r * r
        x0 = max(0, cx - r)
        y0 = max(0, cy - r)
        x1 = min(self.w - 1, cx + r)
        y1 = min(self.h - 1, cy + r)
        for y in range(y0, y1 + 1):
            dy = y - cy
            for x in range(x0, x1 + 1):
                dx = x - cx
                if dx * dx + dy * dy <= r2:
                    self.pixels[y][x] = color

    def fill_ellipse(self, cx: int, cy: int, rx: int, ry: int, color: Color):
        if rx <= 0 or ry <= 0:
            return
        x0 = max(0, cx - rx)
        y0 = max(0, cy - ry)
        x1 = min(self.w - 1, cx + rx)
        y1 = min(self.h - 1, cy + ry)
        rx2 = rx * rx
        ry2 = ry * ry
        rr = rx2 * ry2
        for y in range(y0, y1 + 1):
            dy2 = (y - cy) * (y - cy)
            for x in range(x0, x1 + 1):
                dx2 = (x - cx) * (x - cx)
                if dx2 * ry2 + dy2 * rx2 <= rr:
                    self.pixels[y][x] = color

    def draw_line(self, x0: int, y0: int, x1: int, y1: int, color: Color, thickness: int = 1):
        dx = x1 - x0
        dy = y1 - y0
        steps = max(abs(dx), abs(dy))
        if steps == 0:
            self.fill_circle(x0, y0, max(1, thickness // 2), color)
            return
        radius = max(1, thickness // 2)
        for i in range(steps + 1):
            t = i / steps
            x = int(round(x0 + dx * t))
            y = int(round(y0 + dy * t))
            self.fill_circle(x, y, radius, color)


# --- Shared icon components ---

def cloud(c: Canvas, s: int, x: float = 0.0, y: float = 0.0, dense: int = 0, overcast: bool = False):
    cx = int((0.50 + x) * s)
    cy = int((0.56 + y) * s)
    w = int(0.54 * s)
    h = int(0.22 * s)

    col_base = CLOUD_MID
    col_top = CLOUD_LIGHT
    if dense == 1:
        col_base = CLOUD_DARK
        col_top = CLOUD_MID
    elif dense >= 2:
        col_base = (98, 115, 136)
        col_top = CLOUD_DARK

    c.fill_ellipse(cx - int(0.20 * s), cy - int(0.03 * s), int(0.14 * s), int(0.11 * s), col_top)
    c.fill_ellipse(cx, cy - int(0.08 * s), int(0.18 * s), int(0.13 * s), col_top)
    c.fill_ellipse(cx + int(0.18 * s), cy - int(0.02 * s), int(0.14 * s), int(0.10 * s), col_top)
    c.fill_rect(cx - w // 2, cy - int(0.02 * s), w, h, col_base)

    if overcast:
        c.fill_ellipse(cx - int(0.18 * s), cy + int(0.10 * s), int(0.24 * s), int(0.08 * s), col_base)
        c.fill_ellipse(cx + int(0.20 * s), cy + int(0.11 * s), int(0.22 * s), int(0.07 * s), col_base)


def sun(c: Canvas, s: int, x: float = 0.0, y: float = 0.0, small: bool = False):
    cx = int((0.33 + x) * s)
    cy = int((0.34 + y) * s)
    r = int((0.14 if small else 0.17) * s)

    ray_inner = r + int(0.04 * s)
    ray_outer = r + int(0.12 * s)
    for i in range(8):
        a = i * math.pi / 4.0
        x0 = int(cx + math.cos(a) * ray_inner)
        y0 = int(cy + math.sin(a) * ray_inner)
        x1 = int(cx + math.cos(a) * ray_outer)
        y1 = int(cy + math.sin(a) * ray_outer)
        c.draw_line(x0, y0, x1, y1, SUN_SOFT, thickness=max(1, s // 28))

    c.fill_circle(cx, cy, r, SUN)


def rain(c: Canvas, s: int, count: int, heavy: bool = False, drizzle: bool = False):
    x_start = int(0.30 * s)
    y_start = int(0.66 * s)
    spacing = int(0.11 * s)
    length = int((0.15 if heavy else 0.11 if drizzle else 0.13) * s)
    thickness = max(1, int((0.045 if heavy else 0.03) * s))
    col = RAIN_HEAVY if heavy else RAIN
    for i in range(count):
        x = x_start + i * spacing
        y = y_start + (i % 2) * int(0.02 * s)
        c.draw_line(x, y, x - int(0.03 * s), y + length, col, thickness=thickness)


def snow(c: Canvas, s: int, count: int = 4):
    x_start = int(0.29 * s)
    y_start = int(0.71 * s)
    spacing = int(0.12 * s)
    size = max(2, int(0.03 * s))
    for i in range(count):
        cx = x_start + i * spacing
        cy = y_start + (i % 2) * int(0.03 * s)
        c.draw_line(cx - size, cy, cx + size, cy, SNOW, thickness=1)
        c.draw_line(cx, cy - size, cx, cy + size, SNOW, thickness=1)
        c.draw_line(cx - size, cy - size, cx + size, cy + size, SNOW, thickness=1)
        c.draw_line(cx - size, cy + size, cx + size, cy - size, SNOW, thickness=1)


def lightning(c: Canvas, s: int):
    pts = [
        (0.53, 0.56),
        (0.45, 0.72),
        (0.54, 0.72),
        (0.48, 0.88),
        (0.65, 0.66),
        (0.56, 0.66),
    ]
    thick = max(1, s // 18)
    for i in range(len(pts) - 1):
        x0 = int(pts[i][0] * s)
        y0 = int(pts[i][1] * s)
        x1 = int(pts[i + 1][0] * s)
        y1 = int(pts[i + 1][1] * s)
        c.draw_line(x0, y0, x1, y1, STORM, thickness=thick)


def fog_lines(c: Canvas, s: int, dense: bool = False, light: bool = False):
    rows = [0.48, 0.60, 0.72]
    if dense:
        rows = [0.42, 0.52, 0.62, 0.72]
    if light:
        rows = [0.56, 0.68]

    amp = max(1, int(0.018 * s))
    wave = max(8, int(0.20 * s))
    for idx, rf in enumerate(rows):
        y0 = int(rf * s)
        col = FOG_LIGHT if idx % 2 == 0 else FOG_DARK
        x_prev = int(0.16 * s)
        y_prev = y0
        for x in range(int(0.16 * s), int(0.84 * s) + 1):
            y = y0 + int(round(math.sin((x / wave) * 2 * math.pi) * amp))
            c.draw_line(x_prev, y_prev, x, y, col, thickness=max(1, s // 30))
            x_prev, y_prev = x, y


# --- Icon definitions ---

def draw_clear(c: Canvas, s: int):
    sun(c, s)


def draw_few_clouds(c: Canvas, s: int):
    sun(c, s, x=-0.01, y=-0.01, small=True)
    cloud(c, s, x=0.08, y=0.06, dense=0)


def draw_scattered_clouds(c: Canvas, s: int):
    cloud(c, s, x=-0.08, y=0.03, dense=0)
    cloud(c, s, x=0.10, y=0.12, dense=1)


def draw_broken_clouds(c: Canvas, s: int):
    cloud(c, s, x=-0.10, y=0.02, dense=1)
    cloud(c, s, x=0.12, y=0.10, dense=2)


def draw_overcast(c: Canvas, s: int):
    cloud(c, s, x=0.00, y=0.02, dense=2, overcast=True)
    cloud(c, s, x=0.00, y=0.14, dense=2, overcast=True)


def draw_shower_rain(c: Canvas, s: int):
    cloud(c, s, dense=1)
    rain(c, s, count=4, heavy=False)


def draw_rain(c: Canvas, s: int):
    cloud(c, s, dense=2)
    rain(c, s, count=5, heavy=True)


def draw_drizzle(c: Canvas, s: int):
    cloud(c, s, dense=0)
    rain(c, s, count=3, drizzle=True)


def draw_thunderstorm(c: Canvas, s: int):
    cloud(c, s, dense=2)
    lightning(c, s)
    rain(c, s, count=3, heavy=True)


def draw_snow(c: Canvas, s: int):
    cloud(c, s, dense=1)
    snow(c, s, count=4)


def draw_atmosphere(c: Canvas, s: int):
    fog_lines(c, s, dense=False)


def draw_mist(c: Canvas, s: int):
    fog_lines(c, s, light=True)


def draw_fog(c: Canvas, s: int):
    fog_lines(c, s, dense=True)


DRAWERS: Dict[str, Callable[[Canvas, int], None]] = {
    "clear": draw_clear,
    "few_clouds": draw_few_clouds,
    "scattered_clouds": draw_scattered_clouds,
    "broken_clouds": draw_broken_clouds,
    "overcast": draw_overcast,
    "shower_rain": draw_shower_rain,
    "rain": draw_rain,
    "drizzle": draw_drizzle,
    "thunderstorm": draw_thunderstorm,
    "snow": draw_snow,
    "atmosphere": draw_atmosphere,
    "mist": draw_mist,
    "fog": draw_fog,
}


def rgb_to_565(color: Color) -> int:
    r, g, b = color
    r5 = (r * 31 + 127) // 255
    g6 = (g * 63 + 127) // 255
    b5 = (b * 31 + 127) // 255
    return (r5 << 11) | (g6 << 5) | b5


def write_bmp565(path: str, canvas: Canvas):
    w, h = canvas.w, canvas.h
    row_size = ((w * 2 + 3) // 4) * 4
    pixel_data_size = row_size * h

    file_header_size = 14
    dib_header_size = 40
    masks_size = 12
    pixel_offset = file_header_size + dib_header_size + masks_size
    file_size = pixel_offset + pixel_data_size

    with open(path, "wb") as f:
        # BITMAPFILEHEADER
        f.write(b"BM")
        f.write(struct.pack("<IHHI", file_size, 0, 0, pixel_offset))

        # BITMAPINFOHEADER
        f.write(
            struct.pack(
                "<IIIHHIIIIII",
                dib_header_size,
                w,
                h,
                1,
                16,
                3,  # BI_BITFIELDS
                pixel_data_size,
                2835,
                2835,
                0,
                0,
            )
        )

        # RGB565 masks
        f.write(struct.pack("<III", 0xF800, 0x07E0, 0x001F))

        pad = b"\x00" * (row_size - w * 2)
        # BMP rows are bottom-up
        for y in range(h - 1, -1, -1):
            row = canvas.pixels[y]
            for x in range(w):
                f.write(struct.pack("<H", rgb_to_565(row[x])))
            if pad:
                f.write(pad)


def render_icon(name: str, size: int, out_dir: str):
    c = Canvas(size, size, BG)
    DRAWERS[name](c, size)
    out_path = os.path.join(out_dir, f"{name}_{size}.bmp")
    write_bmp565(out_path, c)


def main():
    out_dir = os.path.join("src", "icons")
    os.makedirs(out_dir, exist_ok=True)

    for size in (80, 36):
        for key in ICON_KEYS:
            render_icon(key, size, out_dir)

    print(f"Generated {len(ICON_KEYS) * 2} icons in {out_dir}")


if __name__ == "__main__":
    main()
