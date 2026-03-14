#!/usr/bin/env python3
"""
Generate a pixel-accurate mockup of the Pressure+HVAC view.
Screen: 480x320 landscape
"""
from PIL import Image, ImageDraw, ImageFont
import os, math

BG               = (27,  31,  39)
LINE_COLOR       = (56,  63,  76)
CARD_FILL        = (20,  25,  35)
CARD_BORDER      = (63,  75,  95)
GRAPH_BG         = (15,  18,  26)
GRAPH_GRID       = (40,  46,  58)
COLOR_OWM        = (100, 180, 255)
COLOR_BME        = (120, 220, 140)
HVAC_HEAT        = (255, 140,  60)
HVAC_COOL        = ( 80, 160, 255)
HVAC_IDLE        = (100, 108, 120)
TEXT_HEADER      = (222, 225, 230)
TEXT_STATUS      = (182, 187, 196)
TEXT_LABEL       = (188, 196, 208)
TEXT_BOTTOM      = (140, 148, 160)
TEXT_VALUE       = (232, 235, 240)

W, H = 480, 320

def find_font(size, bold=False):
    if bold:
        candidates = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        ]
    else:
        candidates = [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ]
    for p in candidates:
        if os.path.exists(p): return ImageFont.truetype(p, size)
    return ImageFont.load_default()

f10 = find_font(10); f11 = find_font(11); f12 = find_font(12)
f13 = find_font(13); f14 = find_font(14); f16 = find_font(16)
f12b = find_font(12, bold=True); f14b = find_font(14, bold=True)

def tr(draw, text, rx, y, font, color):
    bb = draw.textbbox((0,0), text, font=font)
    draw.text((rx - (bb[2]-bb[0]), y), text, font=font, fill=color)

def tc(draw, text, cx, y, font, color):
    bb = draw.textbbox((0,0), text, font=font)
    draw.text((cx - (bb[2]-bb[0])//2, y), text, font=font, fill=color)

img  = Image.new("RGB", (W, H), BG)
draw = ImageDraw.Draw(img)

# ── Header ──────────────────────────────────────────────────────────
draw.text((10, 8), "10:42 AM", font=f14, fill=TEXT_HEADER)
tc(draw, "Pressure + HVAC (24h)  —  tap graph to toggle", W//2, 8, f11, TEXT_STATUS)
draw.line([(0,30),(W,30)], fill=LINE_COLOR, width=1)

# ── Readout row ─────────────────────────────────────────────────────
ry = 36
draw.text((10, ry), "BME:", font=f12b, fill=TEXT_LABEL)
draw.text((48, ry), "996.2 hPa", font=f12, fill=COLOR_BME)
draw.text((140, ry), "OWM:", font=f12b, fill=TEXT_LABEL)
draw.text((178, ry), "1013.4 hPa", font=f12, fill=COLOR_OWM)
# Trend readout (right-aligned) — new in v0.3.4
tr(draw, "3h: +1.2", W-14, ry, f12, TEXT_VALUE)

# ── Graph area ──────────────────────────────────────────────────────
gx, gt = 44, 64
gw = W - gx - 14     # ~422
gh = 320 - gt - 72 - 40  # ~144  (leave 72 HVAC box + 40 labels/hint)
gh = max(gh, 60)

# Background
draw.rectangle([gx, gt, gx+gw, gt+gh], fill=GRAPH_BG)

# Grid
for i in range(1,4):
    gy = gt + gh*i//4
    draw.line([(gx,gy),(gx+gw,gy)], fill=GRAPH_GRID, width=1)

# Y-axis labels
y_max, y_min = 1014.5, 1012.5   # after normalization
for i,v in enumerate([y_max, (y_max+y_min)/2, y_min]):
    gy = gt + gh*i//2
    tr(draw, f"{v:.0f}", gx-4, gy-6, f10, TEXT_LABEL)

# Simulate 24h pressure data — gentle sinusoidal drift
import random; random.seed(42)
n_owm = 96   # 24h / 15min OWM updates
n_bme = 480  # 24h / 3min

def pressure_curve(n, base, amp, noise):
    return [base + amp*math.sin(2*math.pi*i/n + 0.5) + random.gauss(0, noise)
            for i in range(n)]

owm_data = pressure_curve(n_owm, 1013.4, 0.6, 0.05)
bme_data = pressure_curve(n_bme, 1013.4, 0.6, 0.08)  # normalized = same baseline

def plot_line(data, color, width=1):
    pts = [(gx + int(i/(len(data)-1)*gw),
            gt + gh - int((v - y_min)/(y_max - y_min)*gh))
           for i,v in enumerate(data)]
    pts = [(x, max(gt, min(gt+gh, y))) for x,y in pts]
    for i in range(len(pts)-1):
        draw.line([pts[i], pts[i+1]], fill=color, width=width)

plot_line(owm_data, COLOR_OWM, 1)
plot_line(bme_data, COLOR_BME, 2)

# X-axis labels
xl_y = gt + gh + 4
draw.text((gx, xl_y), "-24h", font=f10, fill=TEXT_LABEL)
tc(draw, "-12h", gx + gw//2, xl_y, f10, TEXT_LABEL)
tr(draw, "now", gx+gw, xl_y, f10, TEXT_LABEL)

# Legend
draw.line([(gx, xl_y+14),(gx+18, xl_y+14)], fill=COLOR_BME, width=2)
draw.text((gx+22, xl_y+8), "BME280", font=f10, fill=COLOR_BME)
draw.line([(gx+80, xl_y+14),(gx+98, xl_y+14)], fill=COLOR_OWM, width=1)
draw.text((gx+102, xl_y+8), "OWM", font=f10, fill=COLOR_OWM)

# ── HVAC status box ─────────────────────────────────────────────────
hvac_y = gt + gh + 40
hvac_h = 72
draw.rounded_rectangle([8, hvac_y, W-8, hvac_y+hvac_h-1], radius=8,
                        fill=(18,22,32), outline=(56,70,90), width=1)

draw.text((18, hvac_y+6), "HVAC", font=f14b, fill=TEXT_HEADER)

# Mode indicator
mode_x = 90
draw.rounded_rectangle([mode_x, hvac_y+6, mode_x+60, hvac_y+24],
                        radius=4, fill=HVAC_HEAT, outline=None)
tc(draw, "HEAT", mode_x+30, hvac_y+8, f12b, (30,20,10))

# Stats
draw.text((18,  hvac_y+30), "On-time:", font=f12, fill=TEXT_LABEL)
draw.text((80,  hvac_y+30), "38%", font=f12, fill=HVAC_HEAT)
draw.text((120, hvac_y+30), "Cycles/24h:", font=f12, fill=TEXT_LABEL)
draw.text((212, hvac_y+30), "14", font=f12, fill=TEXT_VALUE)
draw.text((18,  hvac_y+46), "Last run:", font=f12, fill=TEXT_LABEL)
draw.text((80,  hvac_y+46), "12 min ago", font=f12, fill=TEXT_VALUE)
draw.text((200, hvac_y+46), "Avg run:", font=f12, fill=TEXT_LABEL)
draw.text((258, hvac_y+46), "8.4 min", font=f12, fill=TEXT_VALUE)

# ── Bottom hint ──────────────────────────────────────────────────────
tc(draw, "(swipe left/right to navigate   tap graph = 2h/24h toggle)", W//2, H-11, f10, TEXT_BOTTOM)

out = os.path.join(os.path.dirname(__file__), "../ws_s3_3p5_pressure_hvac_mock_v1.png")
img.save(out)
print(f"Saved: {out}")
