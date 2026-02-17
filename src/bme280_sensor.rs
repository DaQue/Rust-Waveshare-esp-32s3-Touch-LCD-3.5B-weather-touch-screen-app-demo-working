use esp_idf_hal::i2c::I2cDriver;
use log::{info, warn};

const BME280_ADDR: u8 = 0x76;
const BME280_ADDR_ALT: u8 = 0x77;
const BME280_CHIP_ID_REG: u8 = 0xD0;
const BME280_CHIP_ID: u8 = 0x60;

// BME280 registers
const REG_CTRL_HUM: u8 = 0xF2;
const REG_CTRL_MEAS: u8 = 0xF4;
const REG_CONFIG: u8 = 0xF5;
const REG_PRESS_MSB: u8 = 0xF7;
const REG_CALIB_00: u8 = 0x88;
const REG_CALIB_26: u8 = 0xE1;

pub struct Bme280Reading {
    pub temperature_f: f32,
    pub humidity: f32,
    pub pressure_hpa: f32,
}

pub struct Bme280 {
    addr: u8,
    // Calibration data
    dig_t1: u16,
    dig_t2: i16,
    dig_t3: i16,
    dig_p1: u16,
    dig_p2: i16,
    dig_p3: i16,
    dig_p4: i16,
    dig_p5: i16,
    dig_p6: i16,
    dig_p7: i16,
    dig_p8: i16,
    dig_p9: i16,
    dig_h1: u8,
    dig_h2: i16,
    dig_h3: u8,
    dig_h4: i16,
    dig_h5: i16,
    dig_h6: i8,
}

impl Bme280 {
    /// Try to initialize BME280 on the I2C bus. Returns None if not found.
    pub fn init(i2c: &mut I2cDriver<'_>) -> Option<Self> {
        // Try both addresses
        for &addr in &[BME280_ADDR, BME280_ADDR_ALT] {
            let mut chip_id = [0u8];
            if i2c.write_read(addr, &[BME280_CHIP_ID_REG], &mut chip_id, 100).is_ok()
                && chip_id[0] == BME280_CHIP_ID {
                    info!("BME280 found at 0x{:02X}", addr);
                    let mut sensor = Self::new_uncalibrated(addr);
                    if sensor.read_calibration(i2c).is_ok() {
                        sensor.configure(i2c);
                        return Some(sensor);
                    }
                }
        }
        warn!("BME280 not found on I2C bus");
        None
    }

    fn new_uncalibrated(addr: u8) -> Self {
        Self {
            addr,
            dig_t1: 0, dig_t2: 0, dig_t3: 0,
            dig_p1: 0, dig_p2: 0, dig_p3: 0, dig_p4: 0, dig_p5: 0,
            dig_p6: 0, dig_p7: 0, dig_p8: 0, dig_p9: 0,
            dig_h1: 0, dig_h2: 0, dig_h3: 0, dig_h4: 0, dig_h5: 0, dig_h6: 0,
        }
    }

    fn read_calibration(&mut self, i2c: &mut I2cDriver<'_>) -> Result<(), esp_idf_sys::EspError> {
        // Read calibration block 1 (0x88..0x9F, 26 bytes)
        let mut cal1 = [0u8; 26];
        i2c.write_read(self.addr, &[REG_CALIB_00], &mut cal1, 100)?;

        self.dig_t1 = u16::from_le_bytes([cal1[0], cal1[1]]);
        self.dig_t2 = i16::from_le_bytes([cal1[2], cal1[3]]);
        self.dig_t3 = i16::from_le_bytes([cal1[4], cal1[5]]);
        self.dig_p1 = u16::from_le_bytes([cal1[6], cal1[7]]);
        self.dig_p2 = i16::from_le_bytes([cal1[8], cal1[9]]);
        self.dig_p3 = i16::from_le_bytes([cal1[10], cal1[11]]);
        self.dig_p4 = i16::from_le_bytes([cal1[12], cal1[13]]);
        self.dig_p5 = i16::from_le_bytes([cal1[14], cal1[15]]);
        self.dig_p6 = i16::from_le_bytes([cal1[16], cal1[17]]);
        self.dig_p7 = i16::from_le_bytes([cal1[18], cal1[19]]);
        self.dig_p8 = i16::from_le_bytes([cal1[20], cal1[21]]);
        self.dig_p9 = i16::from_le_bytes([cal1[22], cal1[23]]);

        // dig_h1 is at 0xA1
        let mut h1 = [0u8];
        i2c.write_read(self.addr, &[0xA1], &mut h1, 100)?;
        self.dig_h1 = h1[0];

        // Read calibration block 2 (0xE1..0xE7, 7 bytes)
        let mut cal2 = [0u8; 7];
        i2c.write_read(self.addr, &[REG_CALIB_26], &mut cal2, 100)?;

        self.dig_h2 = i16::from_le_bytes([cal2[0], cal2[1]]);
        self.dig_h3 = cal2[2];
        self.dig_h4 = ((cal2[3] as i16) << 4) | ((cal2[4] as i16) & 0x0F);
        self.dig_h5 = ((cal2[5] as i16) << 4) | ((cal2[4] as i16) >> 4);
        self.dig_h6 = cal2[6] as i8;

        info!("BME280 hum cal: H1={} H2={} H3={} H4={} H5={} H6={}",
            self.dig_h1, self.dig_h2, self.dig_h3, self.dig_h4, self.dig_h5, self.dig_h6);

        Ok(())
    }

    fn configure(&self, i2c: &mut I2cDriver<'_>) {
        // BME280 datasheet: ctrl_hum changes only take effect after writing ctrl_meas.
        // Write order: config (sleep mode first) → ctrl_hum → ctrl_meas (activates all).

        // 1. Put sensor in sleep mode first (ctrl_meas with mode=00)
        let _ = i2c.write(self.addr, &[REG_CTRL_MEAS, 0x00], 100);
        // 2. Set config (standby 1000ms, filter off)
        let _ = i2c.write(self.addr, &[REG_CONFIG, 0xA0], 100);
        // 3. Set humidity oversampling x1
        let _ = i2c.write(self.addr, &[REG_CTRL_HUM, 0x01], 100);
        // 4. Set temp x1, press x1, normal mode — this activates ctrl_hum setting
        let _ = i2c.write(self.addr, &[REG_CTRL_MEAS, 0x27], 100);

        // Verify ctrl_hum was set
        let mut check = [0u8];
        if i2c.write_read(self.addr, &[REG_CTRL_HUM], &mut check, 100).is_ok() {
            info!("BME280 ctrl_hum readback: 0x{:02X} (expect 0x01)", check[0]);
        }
        if i2c.write_read(self.addr, &[REG_CTRL_MEAS], &mut check, 100).is_ok() {
            info!("BME280 ctrl_meas readback: 0x{:02X} (expect 0x27)", check[0]);
        }
    }

    /// Read temperature, humidity, and pressure.
    pub fn read(&self, i2c: &mut I2cDriver<'_>) -> Option<Bme280Reading> {
        let mut raw = [0u8; 8];
        if i2c.write_read(self.addr, &[REG_PRESS_MSB], &mut raw, 100).is_err() {
            return None;
        }

        let adc_p = ((raw[0] as i32) << 12) | ((raw[1] as i32) << 4) | ((raw[2] as i32) >> 4);
        let adc_t = ((raw[3] as i32) << 12) | ((raw[4] as i32) << 4) | ((raw[5] as i32) >> 4);
        let adc_h = ((raw[6] as i32) << 8) | (raw[7] as i32);

        // Temperature compensation
        let var1 = (((adc_t >> 3) - ((self.dig_t1 as i32) << 1)) * (self.dig_t2 as i32)) >> 11;
        let var2 = (((((adc_t >> 4) - (self.dig_t1 as i32)) * ((adc_t >> 4) - (self.dig_t1 as i32))) >> 12) * (self.dig_t3 as i32)) >> 14;
        let t_fine = var1 + var2;
        let temp_c = ((t_fine * 5 + 128) >> 8) as f32 / 100.0;
        let temp_f = temp_c * 9.0 / 5.0 + 32.0;

        // Pressure compensation
        let pressure_hpa = self.compensate_pressure(adc_p, t_fine);

        // Humidity compensation
        let humidity = self.compensate_humidity(adc_h, t_fine);

        Some(Bme280Reading {
            temperature_f: temp_f,
            humidity,
            pressure_hpa,
        })
    }

    #[allow(clippy::precedence)]
    fn compensate_pressure(&self, adc_p: i32, t_fine: i32) -> f32 {
        let mut var1 = (t_fine as i64) - 128000;
        let mut var2 = var1 * var1 * (self.dig_p6 as i64);
        var2 += (var1 * (self.dig_p5 as i64)) << 17;
        var2 += (self.dig_p4 as i64) << 35;
        var1 = ((var1 * var1 * (self.dig_p3 as i64)) >> 8) + ((var1 * (self.dig_p2 as i64)) << 12);
        var1 = (((1i64 << 47) + var1) * (self.dig_p1 as i64)) >> 33;
        if var1 == 0 {
            return 0.0;
        }
        let mut p: i64 = 1048576 - adc_p as i64;
        p = (((p << 31) - var2) * 3125) / var1;
        var1 = ((self.dig_p9 as i64) * (p >> 13) * (p >> 13)) >> 25;
        var2 = ((self.dig_p8 as i64) * p) >> 19;
        p = ((p + var1 + var2) >> 8) + ((self.dig_p7 as i64) << 4);
        (p as f32) / 25600.0
    }

    fn compensate_humidity(&self, adc_h: i32, t_fine: i32) -> f32 {
        // BME280 datasheet Section 4.2.3 — integer compensation formula (i64 to avoid overflow)
        let v = (t_fine - 76800) as i64;

        let x1 = ((adc_h as i64) << 14)
            - ((self.dig_h4 as i64) << 20)
            - ((self.dig_h5 as i64) * v);
        let x1 = (x1 + 16384) >> 15;

        let x2 = (v * (self.dig_h6 as i64)) >> 10;
        let x3 = (v * (self.dig_h3 as i64)) >> 11;
        let x2 = ((x2 * (x3 + 32768)) >> 10) + 2097152;
        let x2 = ((x2 * (self.dig_h2 as i64)) + 8192) >> 14;

        let mut var = x1 * x2;
        var -= (((var >> 15) * (var >> 15)) >> 7) * (self.dig_h1 as i64) >> 4;
        var = var.clamp(0, 419430400);

        (var >> 12) as f32 / 1024.0
    }
}
