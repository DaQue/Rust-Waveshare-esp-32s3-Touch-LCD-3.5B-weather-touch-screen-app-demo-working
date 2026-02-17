use embedded_graphics::{
    mono_font::MonoTextStyle,
    prelude::*,
    text::Text,
};
use profont::{PROFONT_10_POINT, PROFONT_14_POINT, PROFONT_18_POINT, PROFONT_24_POINT};

use crate::framebuffer::Framebuffer;
use crate::layout::*;
use crate::views::AppState;

pub fn draw(fb: &mut Framebuffer, state: &AppState) {
    // 1. Fill background
    fb.clear_color(BG_INDOOR);

    // 2. Header line
    draw_hline(fb, HEADER_LINE_Y, LINE_COLOR_3);

    // 3. Main card spanning most of the screen
    let card_h = SCREEN_H - 64 - INDOOR_CARD_Y;
    draw_card(
        fb,
        CARD_MARGIN,
        INDOOR_CARD_Y,
        SCREEN_W - 2 * CARD_MARGIN,
        card_h,
        16,
        CARD_FILL_INDOOR,
        CARD_BORDER_INDOOR,
        2,
    );

    // 4. Title
    let title_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_HEADER);
    Text::new("Indoor", Point::new(14, 26), title_style)
        .draw(fb)
        .ok();

    let primary_style = MonoTextStyle::new(&PROFONT_24_POINT, TEXT_PRIMARY);
    let secondary_style = MonoTextStyle::new(&PROFONT_18_POINT, TEXT_SECONDARY);
    let placeholder_style = MonoTextStyle::new(&PROFONT_14_POINT, TEXT_DETAIL);

    // 5. Temperature
    if let Some(temp) = state.indoor_temp {
        let text = format!("{:.1}°F", temp);
        Text::new(&text, Point::new(INDOOR_TEMP_X, INDOOR_TEMP_Y + 30), primary_style)
            .draw(fb)
            .ok();
    } else {
        Text::new("No sensor", Point::new(INDOOR_TEMP_X, INDOOR_TEMP_Y + 30), placeholder_style)
            .draw(fb)
            .ok();
    }

    // 6. Humidity
    if let Some(hum) = state.indoor_humidity {
        let text = format!("{:.0}% RH", hum);
        Text::new(&text, Point::new(INDOOR_HUM_X, INDOOR_HUM_Y + 30), primary_style)
            .draw(fb)
            .ok();
    } else {
        Text::new("No sensor", Point::new(INDOOR_HUM_X, INDOOR_HUM_Y + 30), placeholder_style)
            .draw(fb)
            .ok();
    }

    // 7. Pressure
    if let Some(press) = state.indoor_pressure {
        let text = format!("{:.0} hPa", press);
        Text::new(&text, Point::new(INDOOR_PRESS_X, INDOOR_PRESS_Y + 30), secondary_style)
            .draw(fb)
            .ok();
    } else {
        Text::new("No sensor", Point::new(INDOOR_PRESS_X, INDOOR_PRESS_Y + 30), placeholder_style)
            .draw(fb)
            .ok();
    }

    // 9. Bottom text
    let bottom_style = MonoTextStyle::new(&PROFONT_10_POINT, TEXT_BOTTOM);
    Text::new(&state.bottom_text, Point::new(BOTTOM_X, BOTTOM_Y), bottom_style)
        .draw(fb)
        .ok();
}
