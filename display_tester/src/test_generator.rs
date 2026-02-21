use crate::display_config::{BitOrder, ByteOrder, ColorFormat, DisplayConfig, QspiMode};
use crate::timing::TimingConfig;

#[derive(Clone, Copy, Debug)]
pub enum InitVariant {
    Standard,
    DoubleReset,
    ResetHeldDuringPower,
    AltCommandOrder,
    MinimalInit,
}

#[derive(Clone, Debug)]
pub struct TestCase {
    pub id: usize,
    pub phase: &'static str,
    pub name: &'static str,
    pub variant: InitVariant,
    pub config: DisplayConfig,
}

fn timing(reset: u32, post: u32, sleep: u32, display: u32) -> TimingConfig {
    TimingConfig {
        reset_hold_ms: reset,
        post_reset_wait_ms: post,
        sleep_out_wait_ms: sleep,
        display_on_wait_ms: display,
        power_rail_delay_ms: 100,
        backlight_delay_ms: 100,
        inter_command_delay_ms: 20,
    }
}

pub fn build_tests() -> Vec<TestCase> {
    let phase1 = "Phase 1: Timing Validation";
    let phase2 = "Phase 2: Color Format Tests";
    let phase3 = "Phase 3: Reset/Sequence Variations";

    let t1 = timing(500, 500, 500, 200);
    let t2 = timing(200, 300, 300, 100);
    let t3 = timing(100, 300, 300, 100);
    let t4 = timing(50, 200, 200, 50);
    let t5 = timing(10, 120, 120, 10);

    let base_phase1 = DisplayConfig {
        format: ColorFormat::RGB666,
        byte_order: ByteOrder::LittleEndian,
        bit_order: BitOrder::MSBFirst,
        qspi_mode: QspiMode::Quad,
        timing: t1,
    };

    let mut tests = Vec::new();
    tests.push(TestCase {
        id: 1,
        phase: phase1,
        name: "Ultra-safe",
        variant: InitVariant::Standard,
        config: DisplayConfig { timing: t1, ..base_phase1 },
    });
    tests.push(TestCase {
        id: 2,
        phase: phase1,
        name: "Very safe",
        variant: InitVariant::Standard,
        config: DisplayConfig { timing: t2, ..base_phase1 },
    });
    tests.push(TestCase {
        id: 3,
        phase: phase1,
        name: "Conservative",
        variant: InitVariant::Standard,
        config: DisplayConfig { timing: t3, ..base_phase1 },
    });
    tests.push(TestCase {
        id: 4,
        phase: phase1,
        name: "Moderate",
        variant: InitVariant::Standard,
        config: DisplayConfig { timing: t4, ..base_phase1 },
    });
    tests.push(TestCase {
        id: 5,
        phase: phase1,
        name: "Datasheet minimums",
        variant: InitVariant::Standard,
        config: DisplayConfig { timing: t5, ..base_phase1 },
    });

    let phase2_timing = t2;
    let base_phase2 = DisplayConfig {
        timing: phase2_timing,
        ..base_phase1
    };

    tests.push(TestCase {
        id: 6,
        phase: phase2,
        name: "RGB666 LE MSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig { ..base_phase2 },
    });
    tests.push(TestCase {
        id: 7,
        phase: phase2,
        name: "RGB565 LE MSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig { format: ColorFormat::RGB565, ..base_phase2 },
    });
    tests.push(TestCase {
        id: 8,
        phase: phase2,
        name: "BGR666 LE MSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig { format: ColorFormat::BGR666, ..base_phase2 },
    });
    tests.push(TestCase {
        id: 9,
        phase: phase2,
        name: "RGB888 LE MSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig { format: ColorFormat::RGB888, ..base_phase2 },
    });
    tests.push(TestCase {
        id: 10,
        phase: phase2,
        name: "RGB666 BE MSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig { byte_order: ByteOrder::BigEndian, ..base_phase2 },
    });
    tests.push(TestCase {
        id: 11,
        phase: phase2,
        name: "RGB565 BE MSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig {
            format: ColorFormat::RGB565,
            byte_order: ByteOrder::BigEndian,
            ..base_phase2
        },
    });
    tests.push(TestCase {
        id: 12,
        phase: phase2,
        name: "RGB666 LE LSB Quad",
        variant: InitVariant::Standard,
        config: DisplayConfig { bit_order: BitOrder::LSBFirst, ..base_phase2 },
    });
    tests.push(TestCase {
        id: 13,
        phase: phase2,
        name: "RGB666 LE MSB Dual",
        variant: InitVariant::Standard,
        config: DisplayConfig { qspi_mode: QspiMode::Dual, ..base_phase2 },
    });
    tests.push(TestCase {
        id: 14,
        phase: phase2,
        name: "RGB565 LE MSB Dual",
        variant: InitVariant::Standard,
        config: DisplayConfig {
            format: ColorFormat::RGB565,
            qspi_mode: QspiMode::Dual,
            ..base_phase2
        },
    });
    tests.push(TestCase {
        id: 15,
        phase: phase2,
        name: "RGB666 LE MSB Single",
        variant: InitVariant::Standard,
        config: DisplayConfig { qspi_mode: QspiMode::Single, ..base_phase2 },
    });

    let phase3_base = DisplayConfig { timing: t2, ..base_phase1 };
    tests.push(TestCase {
        id: 16,
        phase: phase3,
        name: "Double software reset (0x01)",
        variant: InitVariant::DoubleReset,
        config: DisplayConfig { timing: t2, ..phase3_base },
    });
    tests.push(TestCase {
        id: 17,
        phase: phase3,
        name: "Long rail delays",
        variant: InitVariant::Standard,
        config: DisplayConfig {
            timing: TimingConfig {
                power_rail_delay_ms: 500,
                ..t2
            },
            ..phase3_base
        },
    });
    tests.push(TestCase {
        id: 18,
        phase: phase3,
        name: "Software reset before init",
        variant: InitVariant::ResetHeldDuringPower,
        config: DisplayConfig { timing: t2, ..phase3_base },
    });
    tests.push(TestCase {
        id: 19,
        phase: phase3,
        name: "Alt command order",
        variant: InitVariant::AltCommandOrder,
        config: DisplayConfig { timing: t2, ..phase3_base },
    });
    tests.push(TestCase {
        id: 20,
        phase: phase3,
        name: "Minimal init",
        variant: InitVariant::MinimalInit,
        config: DisplayConfig { timing: t2, ..phase3_base },
    });

    tests
}
