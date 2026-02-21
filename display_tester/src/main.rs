mod app;
mod display_config;
mod esp_display;
mod test_generator;
mod test_patterns;
mod timing;

use anyhow::Result;

fn main() -> Result<()> {
    app::run()
}
