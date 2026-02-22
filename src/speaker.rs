use std::thread;
use std::time::Duration;

use esp_idf_hal::gpio::{InputPin, OutputPin};
use esp_idf_hal::i2c::I2cDriver;
use esp_idf_hal::i2s::config::{
    Config as I2sChannelConfig, DataBitWidth, SlotMode, StdClkConfig, StdConfig, StdGpioConfig,
    StdSlotConfig,
};
use esp_idf_hal::i2s::{I2sDriver, I2sTx};
use esp_idf_hal::i2s::I2S0;
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_sys::{EspError, ESP_ERR_TIMEOUT};

#[derive(Clone, Copy, Debug)]
pub enum AlertTone {
    Advisory,
    Watch,
    Warning,
}

impl AlertTone {
    pub fn from_request(v: i8) -> Option<Self> {
        match v {
            0 => Some(Self::Advisory),
            1 => Some(Self::Watch),
            2 => Some(Self::Warning),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AlertTone::Advisory => "advisory",
            AlertTone::Watch => "watch",
            AlertTone::Warning => "warning",
        }
    }
}

pub struct Speaker<'d> {
    i2s: I2sDriver<'d, I2sTx>,
    sample_rate_hz: u32,
}

const ES8311_ADDR: u8 = 0x18;
const TCA9554_ADDR: u8 = 0x20;
const TCA9554_REG_OUTPUT: u8 = 0x01;
const TCA9554_REG_CONFIG: u8 = 0x03;
const TCA9554_PA_CTRL_BIT: u8 = 1 << 7;

fn write_reg(i2c: &mut I2cDriver<'_>, reg: u8, val: u8) -> Result<(), EspError> {
    i2c.write(ES8311_ADDR, &[reg, val], 100)
}

fn read_reg(i2c: &mut I2cDriver<'_>, reg: u8) -> Result<u8, EspError> {
    let mut v = [0u8; 1];
    i2c.write_read(ES8311_ADDR, &[reg], &mut v, 100)?;
    Ok(v[0])
}

fn tca9554_read_reg(i2c: &mut I2cDriver<'_>, reg: u8) -> Result<u8, EspError> {
    let mut v = [0u8; 1];
    i2c.write_read(TCA9554_ADDR, &[reg], &mut v, 100)?;
    Ok(v[0])
}

fn tca9554_write_reg(i2c: &mut I2cDriver<'_>, reg: u8, val: u8) -> Result<(), EspError> {
    i2c.write(TCA9554_ADDR, &[reg, val], 100)
}

pub fn enable_pa(i2c: &mut I2cDriver<'_>) -> Result<(), EspError> {
    let mut config = tca9554_read_reg(i2c, TCA9554_REG_CONFIG)?;
    config &= !TCA9554_PA_CTRL_BIT;
    tca9554_write_reg(i2c, TCA9554_REG_CONFIG, config)?;

    let mut output = tca9554_read_reg(i2c, TCA9554_REG_OUTPUT)?;
    output |= TCA9554_PA_CTRL_BIT;
    tca9554_write_reg(i2c, TCA9554_REG_OUTPUT, output)?;
    Ok(())
}

pub fn init_audio_path(i2c: &mut I2cDriver<'_>) -> Result<(), EspError> {
    enable_pa(i2c)?;
    init_es8311(i2c)?;
    Ok(())
}

pub fn init_es8311(i2c: &mut I2cDriver<'_>) -> Result<(), EspError> {
    // Based on Espressif esp_codec_dev ES8311 open/start flow.
    write_reg(i2c, 0x44, 0x08)?;
    write_reg(i2c, 0x44, 0x08)?;
    write_reg(i2c, 0x01, 0x30)?;
    write_reg(i2c, 0x02, 0x00)?;
    write_reg(i2c, 0x03, 0x10)?;
    write_reg(i2c, 0x16, 0x24)?;
    write_reg(i2c, 0x04, 0x10)?;
    write_reg(i2c, 0x05, 0x00)?;
    write_reg(i2c, 0x0B, 0x00)?;
    write_reg(i2c, 0x0C, 0x00)?;
    write_reg(i2c, 0x10, 0x1F)?;
    write_reg(i2c, 0x11, 0x7F)?;
    write_reg(i2c, 0x00, 0x80)?;

    // Slave mode, use external MCLK, non-inverted clocks.
    let mut reg00 = read_reg(i2c, 0x00)?;
    reg00 &= !0x40;
    write_reg(i2c, 0x00, reg00)?;
    write_reg(i2c, 0x01, 0x3F)?;
    let mut reg06 = read_reg(i2c, 0x06)?;
    reg06 &= !0x20;
    write_reg(i2c, 0x06, reg06)?;

    write_reg(i2c, 0x13, 0x10)?;
    write_reg(i2c, 0x1B, 0x0A)?;
    write_reg(i2c, 0x1C, 0x6A)?;
    write_reg(i2c, 0x44, 0x58)?;

    // Start path
    write_reg(i2c, 0x00, 0x80)?;
    write_reg(i2c, 0x01, 0x3F)?;
    let mut reg09 = read_reg(i2c, 0x09)?;
    reg09 &= !0x40;
    write_reg(i2c, 0x09, reg09)?;
    let mut reg0a = read_reg(i2c, 0x0A)?;
    reg0a &= !0x40;
    write_reg(i2c, 0x0A, reg0a)?;

    write_reg(i2c, 0x17, 0xBF)?;
    write_reg(i2c, 0x0E, 0x02)?;
    write_reg(i2c, 0x12, 0x00)?;
    write_reg(i2c, 0x14, 0x1A)?;
    write_reg(i2c, 0x0D, 0x01)?;
    write_reg(i2c, 0x15, 0x40)?;
    write_reg(i2c, 0x37, 0x08)?;
    write_reg(i2c, 0x45, 0x00)?;

    // Unmute + set a high output level for short alert tones.
    let mut reg31 = read_reg(i2c, 0x31)?;
    reg31 &= 0x9F;
    write_reg(i2c, 0x31, reg31)?;
    write_reg(i2c, 0x32, 0xFF)?;
    Ok(())
}

impl<'d> Speaker<'d> {
    pub fn new(
        i2s: impl Peripheral<P = I2S0> + 'd,
        bclk: impl Peripheral<P = impl InputPin + OutputPin> + 'd,
        dout: impl Peripheral<P = impl OutputPin> + 'd,
        ws: impl Peripheral<P = impl InputPin + OutputPin> + 'd,
        mclk: Option<impl Peripheral<P = impl InputPin + OutputPin> + 'd>,
    ) -> Result<Self, EspError> {
        // Factory ES8311 path runs at 48 kHz; keep I2S in sync.
        let sample_rate_hz = 48_000;
        let std_config = StdConfig::new(
            I2sChannelConfig::default().auto_clear(true),
            StdClkConfig::from_sample_rate_hz(sample_rate_hz),
            StdSlotConfig::philips_slot_default(DataBitWidth::Bits16, SlotMode::Stereo),
            StdGpioConfig::default(),
        );
        let mut i2s = I2sDriver::<I2sTx>::new_std_tx(i2s, &std_config, bclk, dout, mclk, ws)?;
        i2s.tx_enable()?;
        Ok(Self {
            i2s,
            sample_rate_hz,
        })
    }

    fn clear_output(&mut self) -> Result<(), EspError> {
        let silence = [0u8; 512];
        for _ in 0..6 {
            let mut offset = 0usize;
            while offset < silence.len() {
                match self.i2s.write(&silence[offset..], 20) {
                    Ok(written) => {
                        if written == 0 {
                            break;
                        }
                        offset += written;
                    }
                    Err(e) if e.code() == ESP_ERR_TIMEOUT => continue,
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    fn write_square_tone<F: FnMut() -> bool>(
        &mut self,
        freq_hz: u32,
        duration_ms: u64,
        amp: i16,
        should_stop: &mut F,
    ) -> Result<bool, EspError> {
        const FRAMES_PER_CHUNK: usize = 256;
        let mut phase: u32 = 0;
        let period = (self.sample_rate_hz / freq_hz.max(1)).max(2);
        let total_frames = (self.sample_rate_hz as u64 * duration_ms) / 1000;
        let mut sent_frames: u64 = 0;

        while sent_frames < total_frames {
            if should_stop() {
                return Ok(false);
            }
            let chunk_frames = (total_frames - sent_frames).min(FRAMES_PER_CHUNK as u64) as usize;
            let mut samples = [0i16; FRAMES_PER_CHUNK * 2];
            for i in 0..chunk_frames {
                let hi = (phase % period) < (period / 2);
                // Fade in/out a little to reduce click/pop at tone edges.
                let fade_len = ((self.sample_rate_hz / 200) as usize).max(1);
                let start_gain = if (sent_frames as usize + i) < fade_len {
                    (sent_frames as usize + i) as f32 / fade_len as f32
                } else {
                    1.0
                };
                let end_frames_left = total_frames.saturating_sub(sent_frames + i as u64) as usize;
                let end_gain = if end_frames_left < fade_len {
                    end_frames_left as f32 / fade_len as f32
                } else {
                    1.0
                };
                let gain = start_gain.min(end_gain);
                let a = (amp as f32 * gain) as i16;
                let s = if hi { a } else { -a };
                samples[i * 2] = s;
                samples[i * 2 + 1] = s;
                phase = phase.wrapping_add(1);
            }

            let sample_bytes = chunk_frames * 2 * core::mem::size_of::<i16>();
            let bytes = unsafe {
                core::slice::from_raw_parts(samples.as_ptr() as *const u8, sample_bytes)
            };
            let mut offset = 0usize;
            while offset < bytes.len() {
                if should_stop() {
                    return Ok(false);
                }
                match self.i2s.write(&bytes[offset..], 20) {
                    Ok(written) => {
                        if written == 0 {
                            continue;
                        }
                        offset += written;
                    }
                    Err(e) if e.code() == ESP_ERR_TIMEOUT => continue,
                    Err(e) => return Err(e),
                }
            }
            sent_frames += chunk_frames as u64;
        }

        Ok(true)
    }

    fn pause_ms<F: FnMut() -> bool>(duration_ms: u64, should_stop: &mut F) -> bool {
        let mut remaining = duration_ms;
        while remaining > 0 {
            if should_stop() {
                return false;
            }
            let step = remaining.min(10);
            thread::sleep(Duration::from_millis(step));
            remaining -= step;
        }
        true
    }

    pub fn play<F: FnMut() -> bool>(&mut self, tone: AlertTone, mut should_stop: F) -> Result<(), EspError> {
        match tone {
            AlertTone::Advisory => {
                // Soft double chirp: informative, low urgency.
                if !self.write_square_tone(820, 90, 9000, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
                if !Self::pause_ms(55, &mut should_stop) {
                    self.clear_output()?;
                    return Ok(());
                }
                if !self.write_square_tone(980, 90, 9000, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
            }
            AlertTone::Watch => {
                // Triple pulse: clear attention pattern.
                if !self.write_square_tone(1280, 85, 11500, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
                if !Self::pause_ms(45, &mut should_stop) {
                    self.clear_output()?;
                    return Ok(());
                }
                if !self.write_square_tone(1460, 85, 11500, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
                if !Self::pause_ms(45, &mut should_stop) {
                    self.clear_output()?;
                    return Ok(());
                }
                if !self.write_square_tone(1280, 95, 11500, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
            }
            AlertTone::Warning => {
                // Rising triad: urgent and distinct.
                if !self.write_square_tone(1250, 95, 12500, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
                if !Self::pause_ms(35, &mut should_stop) {
                    self.clear_output()?;
                    return Ok(());
                }
                if !self.write_square_tone(1750, 105, 12500, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
                if !Self::pause_ms(35, &mut should_stop) {
                    self.clear_output()?;
                    return Ok(());
                }
                if !self.write_square_tone(2450, 135, 12500, &mut should_stop)? {
                    self.clear_output()?;
                    return Ok(());
                }
            }
        }
        self.clear_output()?;
        Ok(())
    }
}
