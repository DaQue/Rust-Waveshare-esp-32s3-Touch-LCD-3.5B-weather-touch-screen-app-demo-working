#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct TimingConfig {
    pub reset_hold_ms: u32,
    pub post_reset_wait_ms: u32,
    pub sleep_out_wait_ms: u32,
    pub display_on_wait_ms: u32,
    pub power_rail_delay_ms: u32,
    pub backlight_delay_ms: u32,
    pub inter_command_delay_ms: u32,
}
