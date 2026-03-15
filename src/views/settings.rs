use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::{Alignment, Text},
};
use profont::{PROFONT_10_POINT, PROFONT_14_POINT, PROFONT_18_POINT};

use crate::config::OrientationMode;
use crate::framebuffer::Framebuffer;
use crate::layout::{self, Orientation, *};
use crate::views::AppState;

/// Which settings button was tapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTap {
    TempF,
    TempC,
    OrientAuto,
    OrientToggle,   // toggles between Landscape and Portrait
}

const ROW1_Y: i32 = 70;
const ROW2_Y: i32 = 160;
const BTN_H: i32 = 50;
const BTN_GAP: i32 = 12;

/// Returns (x, y, w, h) for the four option buttons.
/// Indices 0-1: °F, °C   Indices 2-3: Auto, Toggle
fn button_rects(screen_w: i32) -> [(i32, i32, i32, i32); 4] {
    let btn_w = (screen_w - 2 * CARD_MARGIN - BTN_GAP) / 2;
    let bx0 = CARD_MARGIN;
    let bx1 = CARD_MARGIN + btn_w + BTN_GAP;
    [
        (bx0, ROW1_Y, btn_w, BTN_H),  // °F
        (bx1, ROW1_Y, btn_w, BTN_H),  // °C
        (bx0, ROW2_Y, btn_w, BTN_H),  // Auto
        (bx1, ROW2_Y, btn_w, BTN_H),  // Toggle (Landscape / Portrait)
    ]
}

pub fn hit_test(x: i16, y: i16, orientation: Orientation) -> Option<SettingsTap> {
    let screen_w = layout::screen_w(orientation);
    let rects = button_rects(screen_w);
    let actions = [
        SettingsTap::TempF, SettingsTap::TempC,
        SettingsTap::OrientAuto, SettingsTap::OrientToggle,
    ];
    for (i, (bx, by, bw, bh)) in rects.iter().enumerate() {
        if x >= *bx as i16 && x < (*bx + *bw) as i16
            && y >= *by as i16 && y < (*by + *bh) as i16
        {
            return Some(actions[i]);
        }
    }
    None
}

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    let (screen_w, screen_h) = screen_size(state.orientation);
    fb.clear_color(BG_ABOUT);
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_1);

    let header_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Settings", Point::new(14, 24), header_style)
        .draw(fb).ok();

    let rects = button_rects(screen_w);
    let section_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_TERTIARY);
    let btn_style     = MonoTextStyle::new(&PROFONT_18_POINT, TEXT_PRIMARY);
    let check_style   = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_CONDITION);

    // ── Temperature Units ────────────────────────────────────────────────────
    Text::new("Temperature Units", Point::new(CARD_MARGIN, ROW1_Y - 16), section_style)
        .draw(fb).ok();

    let labels_temp = ["\u{00B0}F", "\u{00B0}C"];
    let active_temp = if state.use_celsius { 1 } else { 0 };
    for (i, label) in labels_temp.iter().enumerate() {
        let (bx, by, bw, bh) = rects[i];
        let (fill, border) = if i == active_temp {
            (CARD_FILL_FORECAST, CARD_BORDER_FORECAST)
        } else {
            (CARD_FILL_I2C, LINE_COLOR_1)
        };
        draw_card(fb, bx, by, bw, bh, 8, fill, border, 1);
        let cx = bx + bw / 2;
        let cy = by + bh / 2 + 7;
        Text::with_alignment(label, Point::new(cx, cy), btn_style, Alignment::Center)
            .draw(fb).ok();
        if i == active_temp {
            Text::with_alignment("\u{2713}", Point::new(bx + bw - 8, by + 14), check_style, Alignment::Right)
                .draw(fb).ok();
        }
    }

    // ── Orientation ──────────────────────────────────────────────────────────
    Text::new("Orientation", Point::new(CARD_MARGIN, ROW2_Y - 16), section_style)
        .draw(fb).ok();

    // Toggle label shows the CURRENT lock state so the highlighted button is
    // always accurate.  Tapping it cycles Landscape ↔ Portrait.
    //   Auto or Landscape → shows "Landscape"
    //   Portrait          → shows "Portrait"
    let toggle_label = if state.orientation_mode == OrientationMode::Portrait {
        "Portrait"
    } else {
        "Landscape"
    };
    let labels_orient = ["Auto", toggle_label];
    // Auto button active when Auto; toggle button active when locked to anything
    let active_orient = if state.orientation_mode == OrientationMode::Auto { 0 } else { 1 };

    for (i, label) in labels_orient.iter().enumerate() {
        let (bx, by, bw, bh) = rects[i + 2];
        let (fill, border) = if i == active_orient {
            (CARD_FILL_FORECAST, CARD_BORDER_FORECAST)
        } else {
            (CARD_FILL_I2C, LINE_COLOR_1)
        };
        draw_card(fb, bx, by, bw, bh, 8, fill, border, 1);
        let cx = bx + bw / 2;
        let cy = by + bh / 2 + 7;
        Text::with_alignment(label, Point::new(cx, cy), btn_style, Alignment::Center)
            .draw(fb).ok();
        if i == active_orient {
            Text::with_alignment("\u{2713}", Point::new(bx + bw - 8, by + 14), check_style, Alignment::Right)
                .draw(fb).ok();
        }
    }

    // ── Console hint ─────────────────────────────────────────────────────────
    let hint_style  = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    let hint2_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_DETAIL);
    let console_y = ROW2_Y + BTN_H + 24;
    Text::new("WiFi / API credentials via serial console:", Point::new(CARD_MARGIN, console_y), hint2_style)
        .draw(fb).ok();
    Text::new("  wifi set <ssid> <pass>", Point::new(CARD_MARGIN, console_y + 14), hint2_style)
        .draw(fb).ok();
    Text::new("  api set-key <key>", Point::new(CARD_MARGIN, console_y + 26), hint2_style)
        .draw(fb).ok();

    Text::with_alignment(
        "swipe <-- About  |  hold = menu",
        Point::new(screen_w / 2, screen_h - 4),
        hint_style,
        Alignment::Center,
    ).draw(fb).ok();
}
