//! Built-in NSO patches carried for known builds, matching the C++ reference emulator's
//! `BUILTIN_PATCHES`. [`crate::ModOptions::builtin_patches`] disables them all when false;
//! individual add-ons can be toggled via [`crate::ModOptions::disabled`] (with `+Name` opting in to
//! `off_by_default` patches).

use std::ops::Range;

use crate::program::Module;

/// Prefix stored in `disabled` lists to enable an `off_by_default` built-in add-on.
pub const ENABLED_ADDON_PREFIX: char = '+';

/// Storage key in `disabled` lists that turns an `off_by_default` add-on on.
pub fn enabled_addon_key(name: &str) -> String {
    format!("{ENABLED_ADDON_PREFIX}{name}")
}

/// Summary of one built-in add-on shown in per-game configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltinAddon {
    pub name: &'static str,
    pub off_by_default: bool,
    pub replaces: Option<&'static str>,
    pub description: &'static str,
}

#[derive(Clone, Copy)]
struct Record {
    offset: usize,
    bytes: &'static [u8],
    original: &'static [u8],
    executable_padding: bool,
}

impl Record {
    const fn patch(offset: usize, bytes: &'static [u8], original: &'static [u8]) -> Self {
        Self {
            offset,
            bytes,
            original,
            executable_padding: false,
        }
    }

    const fn cave(offset: usize, bytes: &'static [u8]) -> Self {
        Self {
            offset,
            bytes,
            original: &[],
            executable_padding: true,
        }
    }

    const fn slot(offset: usize, bytes: &'static [u8], padding: &'static [u8]) -> Self {
        Self {
            offset,
            bytes,
            original: padding,
            executable_padding: false,
        }
    }
}

#[derive(Clone, Copy)]
struct Hook {
    name: &'static str,
    function_offset: usize,
    function_size: usize,
    pattern: &'static [u8],
    cave_offset: usize,
    cave: &'static [u8],
}

struct BuiltinPatch {
    name: &'static str,
    title_id: u64,
    module: &'static str,
    build_id: &'static [u8],
    records: &'static [Record],
    states: &'static [Range<usize>],
    hooks: &'static [Hook],
    replaces: Option<&'static str>,
    requires_patch: Option<&'static str>,
    off_by_default: bool,
    group: Option<&'static str>,
    description: &'static str,
}

const OCARINA_OF_TIME: u64 = 0x0400_4860_24DE_C000;
const OCARINA_MAIN_BUILD_ID: [u8; 20] = [
    0xE0, 0xF3, 0xCC, 0xFA, 0xF5, 0x00, 0x65, 0xAE, 0x4E, 0xA0, 0x1E, 0xB5, 0xAE, 0xB1, 0x3B, 0x1A,
    0x45, 0xDA, 0x33, 0x48,
];

const YOSHI_MYSTERIOUS_BOOK: u64 = 0x0400_2450_21D2_C000;
const YOSHI_MAIN_BUILD_ID: [u8; 16] = [
    0x89, 0x3C, 0x30, 0xE7, 0x55, 0x21, 0xAF, 0xB8, 0xB2, 0x33, 0x26, 0xEB, 0x6A, 0xA0, 0xE2, 0xDC,
];

const MARIO_WONDER: u64 = 0x0400_1510_0B51_4001;
const WONDER_MAIN_BUILD_ID: [u8; 20] = [
    0xEC, 0x47, 0xE6, 0xF8, 0x84, 0x6D, 0x2A, 0x7B, 0x68, 0x92, 0xE2, 0x68, 0x28, 0x45, 0xC7, 0x25,
    0x5F, 0x71, 0x17, 0xAE,
];

const MAX_RESOLUTION_NAME: &str = "Max Resolution (built-in)";
const FPS_UNLOCK_NAME: &str = "FPS Unlock (built-in)";
const DYNAMIC_FPS_NAME: &str = "Dynamic FPS by Dark (built-in)";
const TEXTURE_POOL_NAME: &str = "Texture Pool x2 (built-in)";
const TEXTURE_COMPACTION_NAME: &str = "Texture heap compaction interval (built-in)";
const SHADOW_STEPS_NAME: &str = "Shadow Size Steps (built-in)";
const OCCLUSION_OFF_NAME: &str = "Occlusion Off (built-in)";

const MAX_RESOLUTION_ORIGINAL: [u8; 4] = [0xE8, 0x41, 0x00, 0x36];
const MAX_RESOLUTION_BRANCH: [u8; 4] = [0x0F, 0x02, 0x00, 0x14];
const MAX_RESOLUTION_RECORDS: &[Record] = &[Record::patch(
    0x805BD0,
    &MAX_RESOLUTION_BRANCH,
    &MAX_RESOLUTION_ORIGINAL,
)];

const TEXTURE_POOL_ORIGINAL: [u8; 4] = [0xE8, 0x27, 0x01, 0xA9];
const TEXTURE_POOL_CALL: [u8; 4] = [0xD5, 0x6F, 0xC0, 0x94];
const TEXTURE_POOL_ROUTINE: [u8; 12] = [
    0x08, 0xF9, 0x7F, 0xD3, 0xE8, 0x27, 0x01, 0xA9, 0xC0, 0x03, 0x5F, 0xD6,
];
const TEXTURE_POOL_RECORDS: &[Record] = &[
    Record::patch(0xC2A36C, &TEXTURE_POOL_CALL, &TEXTURE_POOL_ORIGINAL),
    Record::cave(0x3C462C0, &TEXTURE_POOL_ROUTINE),
];

const DYNAMIC_FPS_ORIGINAL: [u8; 4] = [0x28, 0x18, 0x20, 0x1E];
const DYNAMIC_FPS_CALL: [u8; 4] = [0x4F, 0x7D, 0xD5, 0x94];

macro_rules! dynamic_fps_with_tail {
    ($($tail:expr),* $(,)?) => {
        [
            0x28, 0x18, 0x20, 0x1E, 0xCA, 0x92, 0x00, 0xB0, 0x4A, 0x01, 0x38, 0x91, 0x2B, 0xE0, 0x3B,
            0xD5, 0x4C, 0x01, 0x40, 0xF9, 0x4B, 0x01, 0x00, 0xF9, 0xCC, 0x02, 0x00, 0xB4, 0x6B, 0x01,
            0x0C, 0xCB, 0x0D, 0xE0, 0x3B, 0xD5, 0x4E, 0x01, 0x80, 0xD2, 0xAE, 0x09, 0xCE, 0x9A, 0x7F,
            0x01, 0x0E, 0xEB, 0xCB, 0x81, 0x8B, 0x9A, 0x4C, 0x05, 0x40, 0xF9, 0x4C, 0x00, 0x00, 0xB5,
            0xEC, 0x03, 0x0B, 0xAA, 0x6E, 0x01, 0x0C, 0xCB, 0x8C, 0x0D, 0x8E, 0x8B, 0x4C, 0x05, 0x00,
            0xF9, 0x82, 0x01, 0x23, 0x9E, 0xA3, 0x01, 0x23, 0x9E, 0x42, 0x18, 0x23, 0x1E, 0x42, 0x08,
            $($tail),*
        ]
    };
}

const DYNAMIC_FPS_ROUTINE: [u8; 120] = dynamic_fps_with_tail!(
    0x21, 0x1E, 0xC3, 0x00, 0x00, 0x1C, 0x03, 0x09, 0x23, 0x1E, 0x40, 0x20, 0x23, 0x1E, 0x4D, 0x00,
    0x00, 0x54, 0x48, 0x40, 0x20, 0x1E, 0xC0, 0x03, 0x5F, 0xD6, 0x0A, 0xD7, 0x83, 0x3F,
);

const FPS_UNLOCK_ROUTINE: [u8; 128] = dynamic_fps_with_tail!(
    0x21, 0x1E, 0xE3, 0x00, 0x00, 0x1C, 0x42, 0x68, 0x23, 0x1E, 0xC3, 0x00, 0x00, 0x1C, 0x42, 0x78,
    0x23, 0x1E, 0x48, 0x40, 0x20, 0x1E, 0xC0, 0x03, 0x5F, 0xD6, 0x1F, 0x20, 0x03, 0xD5, 0x0A, 0xD7,
    0xA3, 0x3C, 0x00, 0x00, 0xC0, 0x40,
);

const WATER_STEPS_ORIGINAL: [u8; 4] = [0x09, 0x00, 0x38, 0x1E];
const WATER_STEPS_CALL: [u8; 4] = [0x42, 0xFB, 0xEF, 0x94];
const WATER_MUL_ORIGINAL: [u8; 4] = [0x08, 0x7D, 0x09, 0x1B];
const WATER_MUL_NOP: [u8; 4] = [0x1F, 0x20, 0x03, 0xD5];
const WATER_STEPS_ROUTINE: [u8; 148] = [
    0x01, 0x01, 0x22, 0x1E, 0x00, 0x08, 0x21, 0x1E, 0x08, 0x20, 0x20, 0x1E, 0x0D, 0x04, 0x00, 0x54,
    0xC9, 0x92, 0x00, 0xB0, 0x29, 0x41, 0x38, 0x91, 0x2A, 0x01, 0x02, 0x91, 0xEB, 0x03, 0x09, 0xAA,
    0x6C, 0x01, 0x40, 0xF9, 0x9F, 0x01, 0x15, 0xEB, 0x60, 0x01, 0x00, 0x54, 0x6B, 0x41, 0x00, 0x91,
    0x7F, 0x01, 0x0A, 0xEB, 0x61, 0xFF, 0xFF, 0x54, 0x4C, 0x01, 0x40, 0xB9, 0x2B, 0x51, 0x2C, 0x8B,
    0x75, 0x01, 0x00, 0xF9, 0x7F, 0x09, 0x00, 0xB9, 0x8C, 0x05, 0x00, 0x11, 0x8C, 0x09, 0x00, 0x12,
    0x4C, 0x01, 0x00, 0xB9, 0x61, 0x09, 0x40, 0xBD, 0x00, 0x28, 0x21, 0x1E, 0x01, 0x10, 0x2C, 0x1E,
    0x02, 0x28, 0x21, 0x1E, 0x48, 0x00, 0x38, 0x1E, 0x1F, 0x05, 0x00, 0x71, 0x4A, 0x00, 0x00, 0x54,
    0x28, 0x00, 0x80, 0x52, 0x01, 0x01, 0x22, 0x1E, 0x00, 0x38, 0x21, 0x1E, 0x01, 0x10, 0x3C, 0x1E,
    0x00, 0x48, 0x21, 0x1E, 0x60, 0x09, 0x00, 0xBD, 0xC0, 0x03, 0x5F, 0xD6, 0xE8, 0x03, 0x1F, 0x2A,
    0xC0, 0x03, 0x5F, 0xD6,
];

const SOUND_ENTRY_ORIGINAL: [u8; 4] = [0x7F, 0x23, 0x03, 0xD5];
const SOUND_ENTRY_BRANCH: [u8; 4] = [0x40, 0xC5, 0xE0, 0x14];
const SOUND_STEP_ORIGINAL: [u8; 4] = [0x00, 0x11, 0x40, 0xBD];
const SOUND_STEP_CALL: [u8; 4] = [0xD3, 0xC3, 0xE0, 0x94];
const SOUND_GATE: [u8; 124] = [
    0xD0, 0x92, 0x00, 0xB0, 0x10, 0x82, 0x3A, 0x91, 0x02, 0x03, 0x00, 0x35, 0xC9, 0x81, 0x00, 0x90,
    0x29, 0x49, 0x44, 0xF9, 0x29, 0x01, 0x40, 0xF9, 0x30, 0x11, 0x40, 0xBD, 0x11, 0x10, 0x2E, 0x1E,
    0x00, 0x22, 0x31, 0x1E, 0xAA, 0x01, 0x00, 0x54, 0x12, 0x02, 0x40, 0xBD, 0x52, 0x2A, 0x30, 0x1E,
    0x40, 0x22, 0x31, 0x1E, 0xCB, 0x00, 0x00, 0x54, 0x52, 0x3A, 0x31, 0x1E, 0x12, 0x02, 0x00, 0xBD,
    0x29, 0x00, 0x80, 0x52, 0x09, 0x06, 0x00, 0xB9, 0x0A, 0x00, 0x00, 0x14, 0x12, 0x02, 0x00, 0xBD,
    0x1F, 0x06, 0x00, 0xB9, 0xC0, 0x03, 0x5F, 0xD6, 0x1F, 0x20, 0x03, 0xD5, 0x29, 0x00, 0x80, 0x52,
    0x09, 0x06, 0x00, 0xB9, 0x03, 0x00, 0x00, 0x14, 0x09, 0x06, 0x40, 0xB9, 0x69, 0x00, 0x00, 0x34,
    0x7F, 0x23, 0x03, 0xD5, 0xA4, 0x3A, 0x1F, 0x17, 0xC0, 0x03, 0x5F, 0xD6,
];
const SOUND_FEED: [u8; 16] = [
    0x00, 0x11, 0x40, 0xBD, 0x10, 0x10, 0x2E, 0x1E, 0x00, 0x68, 0x30, 0x1E, 0xC0, 0x03, 0x5F, 0xD6,
];

const DYNAMIC_FPS_RECORDS: &[Record] = &[
    Record::patch(0x6E6CC4, &DYNAMIC_FPS_CALL, &DYNAMIC_FPS_ORIGINAL),
    Record::cave(0x3C46200, &DYNAMIC_FPS_ROUTINE),
    Record::patch(0x475F8, &WATER_STEPS_CALL, &WATER_STEPS_ORIGINAL),
    Record::patch(0x4760C, &WATER_MUL_NOP, &WATER_MUL_ORIGINAL),
    Record::cave(0x3C46300, &WATER_STEPS_ROUTINE),
];

const STEP_HUD_ORIGINAL: [u8; 4] = [0x0C, 0x00, 0x38, 0x1E];
const STEP_HUD_HOOK: [u8; 4] = [0x80, 0x79, 0xC7, 0x14];
const STEP_HUD_ROUTINE: [u8; 40] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0x6D, 0x82, 0x0D, 0x91,
    0x1C, 0x00, 0x00, 0x94, 0xEC, 0x03, 0x0D, 0x2A, 0xFE, 0x13, 0x40, 0xF9, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC3, 0xA8, 0x78, 0x86, 0x38, 0x17,
];
const STEP_REPLAY_ORIGINAL: [u8; 4] = [0x08, 0x00, 0x39, 0x1E];
const STEP_REPLAY_HOOK: [u8; 4] = [0x46, 0xCD, 0x26, 0x14];
const STEP_REPLAY_ROUTINE: [u8; 40] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0x6D, 0x02, 0x14, 0x8B,
    0x12, 0x00, 0x00, 0x94, 0xE8, 0x03, 0x0D, 0x2A, 0xFE, 0x13, 0x40, 0xF9, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC3, 0xA8, 0xB2, 0x32, 0xD9, 0x17,
];
const STEP_HITSTOP_ORIGINAL: [u8; 44] = [
    0x00, 0xB9, 0xA1, 0x5E, 0x09, 0x01, 0x38, 0x1E, 0x00, 0xD8, 0x21, 0x5E, 0x00, 0x39, 0x20, 0x1E,
    0x00, 0x20, 0x30, 0x1E, 0xAA, 0x17, 0x00, 0x54, 0x00, 0x20, 0x31, 0x1E, 0x09, 0x18, 0x00, 0x54,
    0x0A, 0x9B, 0x40, 0xB9, 0x3F, 0x05, 0x00, 0x71, 0x29, 0xC5, 0x9F, 0x1A,
];
const STEP_HITSTOP_HOOK: [u8; 44] = [
    0xBA, 0xCD, 0x33, 0x14, 0x09, 0x01, 0x38, 0x1E, 0x00, 0xD8, 0x21, 0x5E, 0x00, 0x39, 0x20, 0x1E,
    0x00, 0x20, 0x30, 0x1E, 0xAA, 0x17, 0x00, 0x54, 0x00, 0x20, 0x31, 0x1E, 0x09, 0x18, 0x00, 0x54,
    0x0A, 0x9B, 0x40, 0xB9, 0x3F, 0x05, 0x00, 0x71, 0x29, 0xC5, 0x9F, 0x1A,
];
const STEP_HITSTOP_ROUTINE: [u8; 48] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0x0D, 0x63, 0x02, 0x91,
    0x00, 0x41, 0x20, 0x1E, 0x07, 0x00, 0x00, 0x94, 0xE9, 0x03, 0x0D, 0x2A, 0xFE, 0x13, 0x40, 0xF9,
    0xEF, 0x43, 0x41, 0xA9, 0xED, 0x3B, 0xC3, 0xA8, 0x0A, 0x9B, 0x40, 0xB9, 0x46, 0x32, 0xCC, 0x17,
];
const STEP_SHARED_ROUTINE: [u8; 128] = [
    0xE0, 0x07, 0xBF, 0xAD, 0xCE, 0x92, 0x00, 0x90, 0xCE, 0x41, 0x38, 0x91, 0x0F, 0x02, 0x80, 0x52,
    0xD0, 0x01, 0x40, 0xF9, 0x1F, 0x02, 0x0D, 0xEB, 0x40, 0x02, 0x00, 0x54, 0x70, 0x01, 0x00, 0xB4,
    0xCE, 0x41, 0x00, 0x91, 0xEF, 0x05, 0x00, 0x71, 0x41, 0xFF, 0xFF, 0x54, 0xCE, 0x41, 0x04, 0xD1,
    0xD0, 0x01, 0x40, 0xB9, 0x0F, 0x06, 0x00, 0x11, 0xEF, 0x0D, 0x00, 0x12, 0xCF, 0x01, 0x00, 0xB9,
    0xCE, 0x41, 0x00, 0x91, 0xCE, 0x11, 0x10, 0x8B, 0xCD, 0x01, 0x00, 0xF9, 0xDF, 0x09, 0x00, 0xB9,
    0x0D, 0x00, 0x38, 0x1E, 0xBF, 0x05, 0x00, 0x71, 0xAD, 0xA5, 0x9F, 0x1A, 0x07, 0x00, 0x00, 0x14,
    0xC1, 0x09, 0x40, 0xBD, 0x00, 0x28, 0x21, 0x1E, 0x01, 0xC0, 0x25, 0x1E, 0x00, 0x38, 0x21, 0x1E,
    0xC0, 0x09, 0x00, 0xBD, 0x2D, 0x00, 0x38, 0x1E, 0xE0, 0x07, 0xC1, 0xAC, 0xC0, 0x03, 0x5F, 0xD6,
];
const STEP_AMBIENT_ORIGINAL: [u8; 4] = [0x7A, 0x07, 0x00, 0x11];
const STEP_AMBIENT_HOOK: [u8; 4] = [0x93, 0x1C, 0xDC, 0x14];
const STEP_AMBIENT_ROUTINE: [u8; 60] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0x8D, 0x0B, 0x01, 0x8B,
    0xCE, 0x81, 0x00, 0x90, 0xCE, 0x49, 0x44, 0xF9, 0xCE, 0x01, 0x40, 0xF9, 0xC0, 0x11, 0x40, 0xBD,
    0x80, 0xFF, 0xFF, 0x97, 0x7A, 0x03, 0x0D, 0x0B, 0x00, 0x18, 0x40, 0xBD, 0xFE, 0x13, 0x40, 0xF9,
    0xEF, 0x43, 0x41, 0xA9, 0xED, 0x3B, 0xC3, 0xA8, 0x60, 0xE3, 0x23, 0x17,
];

const GROUND_VELOCITY_ORIGINAL_A: [u8; 4] = [0x6C, 0x19, 0x00, 0x54];
const GROUND_VELOCITY_ORIGINAL_B: [u8; 4] = [0x2C, 0x1E, 0x00, 0x54];
const GROUND_VELOCITY_ORIGINAL_C: [u8; 4] = [0xCC, 0x17, 0x00, 0x54];
const GROUND_VELOCITY_NOP: [u8; 4] = [0x1F, 0x20, 0x03, 0xD5];

const TURN_SPEED_ORIGINAL: [u8; 8] = [0x01, 0x08, 0x21, 0x1E, 0x00, 0x08, 0x28, 0x1E];
const TURN_SPEED_HOOK: [u8; 8] = [0x21, 0xA0, 0x58, 0x14, 0x00, 0x08, 0x28, 0x1E];
const TURN_SPEED_ROUTINE: [u8; 40] = [
    0x01, 0x10, 0x2E, 0x1E, 0x00, 0x21, 0x21, 0x1E, 0x8A, 0x00, 0x00, 0x54, 0x00, 0x18, 0x28, 0x1E,
    0x01, 0x40, 0x21, 0x1E, 0xDC, 0x5F, 0xA7, 0x17, 0xE1, 0x0F, 0x40, 0xBD, 0x01, 0x08, 0x21, 0x1E,
    0x00, 0x08, 0x28, 0x1E, 0xD8, 0x5F, 0xA7, 0x17,
];

const MOVIE_FRAMES_ORIGINAL: [u8; 4] = [0x08, 0x05, 0x00, 0x11];
const MOVIE_FRAMES_HOOK: [u8; 4] = [0xFF, 0xA8, 0x03, 0x14];
const MOVIE_FRAMES_ROUTINE: [u8; 56] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0x8D, 0xE2, 0x0B, 0x91,
    0xCE, 0x81, 0x00, 0x90, 0xCE, 0x49, 0x44, 0xF9, 0xCE, 0x01, 0x40, 0xF9, 0xC0, 0x11, 0x40, 0xBD,
    0x67, 0xFF, 0xFF, 0x97, 0x08, 0x01, 0x0D, 0x0B, 0xFE, 0x13, 0x40, 0xF9, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC3, 0xA8, 0xF5, 0x56, 0xFC, 0x17,
];

const SCARECROW_CUT_ORIGINAL: [u8; 4] = [0x08, 0x00, 0x39, 0x1E];
const SCARECROW_RECORD_HOOK: [u8; 4] = [0xEA, 0xBF, 0x26, 0x14];
const SCARECROW_LAST_HOOK: [u8; 4] = [0x04, 0xD1, 0x26, 0x14];
const SCARECROW_RECORD_ROUTINE: [u8; 40] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0xED, 0x03, 0x09, 0xAA,
    0xB0, 0xFF, 0xFF, 0x97, 0xE8, 0x03, 0x0D, 0x2A, 0xFE, 0x13, 0x40, 0xF9, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC3, 0xA8, 0x0E, 0x40, 0xD9, 0x17,
];
const SCARECROW_LAST_ROUTINE: [u8; 40] = [
    0xED, 0x3B, 0xBD, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xFE, 0x13, 0x00, 0xF9, 0x8D, 0xF2, 0x17, 0x91,
    0xA6, 0xFF, 0xFF, 0x97, 0xE8, 0x03, 0x0D, 0x2A, 0xFE, 0x13, 0x40, 0xF9, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC3, 0xA8, 0xF4, 0x2E, 0xD9, 0x17,
];

const CLOTH_DT_ORIGINAL_A: [u8; 4] = [0x00, 0x6D, 0x46, 0xBD];
const CLOTH_DT_ORIGINAL_B: [u8; 4] = [0x00, 0x01, 0x27, 0x1E];
const CLOTH_DT_ORIGINAL_D: [u8; 4] = [0x02, 0x6D, 0x46, 0xBD];
const CLOTH_DT_CALL_A: [u8; 4] = [0xF5, 0x01, 0xEC, 0x94];
const CLOTH_DT_CALL_B: [u8; 4] = [0xBF, 0x6C, 0xE6, 0x94];
const CLOTH_DT_CALL_C: [u8; 4] = [0xD7, 0x98, 0x40, 0x94];
const CLOTH_DT_CALL_D: [u8; 4] = [0xA9, 0x92, 0x40, 0x94];
const CLOTH_DT_ROUTINE_S0: [u8; 48] = [
    0xD0, 0x81, 0x00, 0x90, 0x10, 0x4A, 0x44, 0xF9, 0x10, 0x02, 0x40, 0xF9, 0x11, 0x1A, 0x40, 0xB9,
    0x10, 0x11, 0x91, 0x52, 0x10, 0xB1, 0xA7, 0x72, 0x31, 0x06, 0x00, 0x51, 0x3F, 0x02, 0x10, 0x6B,
    0x31, 0x32, 0x90, 0x1A, 0x31, 0x06, 0x00, 0x11, 0x20, 0x02, 0x27, 0x1E, 0xC0, 0x03, 0x5F, 0xD6,
];
const CLOTH_DT_ROUTINE_S2: [u8; 48] = [
    0xD0, 0x81, 0x00, 0x90, 0x10, 0x4A, 0x44, 0xF9, 0x10, 0x02, 0x40, 0xF9, 0x11, 0x1A, 0x40, 0xB9,
    0x10, 0x11, 0x91, 0x52, 0x10, 0xB1, 0xA7, 0x72, 0x31, 0x06, 0x00, 0x51, 0x3F, 0x02, 0x10, 0x6B,
    0x31, 0x32, 0x90, 0x1A, 0x31, 0x06, 0x00, 0x11, 0x22, 0x02, 0x27, 0x1E, 0xC0, 0x03, 0x5F, 0xD6,
];

const SOLVER_STEPS_FUNCTION_SIZE: usize = 0x1090;
const SOLVER_STEPS_SITE: [u8; 8] = [0x01, 0x00, 0x38, 0x1E, 0x01, 0xC8, 0x02, 0xB9];
const SOLVER_STEPS_ROUTINE: [u8; 124] = [
    0x01, 0x00, 0x38, 0x1E, 0xE2, 0x0F, 0xBF, 0xA9, 0xE2, 0x0F, 0xBF, 0xAD, 0x02, 0x42, 0x3B, 0xD5,
    0xE3, 0x4D, 0x82, 0x52, 0x63, 0x50, 0xA7, 0x72, 0x62, 0x00, 0x27, 0x1E, 0x02, 0x38, 0x22, 0x1E,
    0x48, 0x20, 0x20, 0x1E, 0x4D, 0x02, 0x00, 0x54, 0x28, 0x20, 0x20, 0x1E, 0x0D, 0x02, 0x00, 0x54,
    0x40, 0x20, 0x21, 0x1E, 0xCA, 0x01, 0x00, 0x54, 0x43, 0x8F, 0xA8, 0x52, 0x63, 0x00, 0x27, 0x1E,
    0x20, 0x20, 0x23, 0x1E, 0x4C, 0x01, 0x00, 0x54, 0xA3, 0x99, 0x99, 0x52, 0x83, 0xB9, 0xA7, 0x72,
    0x63, 0x00, 0x27, 0x1E, 0x42, 0x38, 0x23, 0x1E, 0x43, 0x00, 0x28, 0x1E, 0x7F, 0x04, 0x00, 0x71,
    0x63, 0xA4, 0x9F, 0x1A, 0x7F, 0x00, 0x01, 0x6B, 0x61, 0xC0, 0x81, 0x1A, 0x02, 0x42, 0x1B, 0xD5,
    0xE2, 0x0F, 0xC1, 0xAC, 0xE2, 0x0F, 0xC1, 0xA8, 0x00, 0x00, 0x00, 0x14,
];
const FPS_UNLOCK_HOOKS: &[Hook] = &[
    Hook {
        name: "solver steps",
        function_offset: 0x38AEB80,
        function_size: SOLVER_STEPS_FUNCTION_SIZE,
        pattern: &SOLVER_STEPS_SITE,
        cave_offset: 0x3C46400,
        cave: &SOLVER_STEPS_ROUTINE,
    },
    Hook {
        name: "solver steps",
        function_offset: 0x38AFC10,
        function_size: SOLVER_STEPS_FUNCTION_SIZE,
        pattern: &SOLVER_STEPS_SITE,
        cave_offset: 0x3C46480,
        cave: &SOLVER_STEPS_ROUTINE,
    },
];

const FPS_UNLOCK_RECORDS: &[Record] = &[
    Record::patch(0x6E6CC4, &DYNAMIC_FPS_CALL, &DYNAMIC_FPS_ORIGINAL),
    Record::cave(0x3C46200, &FPS_UNLOCK_ROUTINE),
    Record::patch(0x475F8, &WATER_STEPS_CALL, &WATER_STEPS_ORIGINAL),
    Record::patch(0x4760C, &WATER_MUL_NOP, &WATER_MUL_ORIGINAL),
    Record::cave(0x3C46300, &WATER_STEPS_ROUTINE),
    Record::patch(0x415300, &SOUND_ENTRY_BRANCH, &SOUND_ENTRY_ORIGINAL),
    Record::patch(0x415334, &SOUND_STEP_CALL, &SOUND_STEP_ORIGINAL),
    Record::cave(0x3C46800, &SOUND_GATE),
    Record::cave(0x3C46280, &SOUND_FEED),
    Record::patch(0xA68300, &STEP_HUD_HOOK, &STEP_HUD_ORIGINAL),
    Record::cave(0x3C46900, &STEP_HUD_ROUTINE),
    Record::patch(0x3293410, &STEP_REPLAY_HOOK, &STEP_REPLAY_ORIGINAL),
    Record::cave(0x3C46928, &STEP_REPLAY_ROUTINE),
    Record::patch(0x2F53268, &STEP_HITSTOP_HOOK, &STEP_HITSTOP_ORIGINAL),
    Record::cave(0x3C46950, &STEP_HITSTOP_ROUTINE),
    Record::patch(0x53F914, &STEP_AMBIENT_HOOK, &STEP_AMBIENT_ORIGINAL),
    Record::cave(0x3C46B60, &STEP_AMBIENT_ROUTINE),
    Record::cave(0x3C46980, &STEP_SHARED_ROUTINE),
    Record::patch(0x13A070, &GROUND_VELOCITY_NOP, &GROUND_VELOCITY_ORIGINAL_A),
    Record::patch(0x13D710, &GROUND_VELOCITY_NOP, &GROUND_VELOCITY_ORIGINAL_B),
    Record::patch(0x389AC2C, &GROUND_VELOCITY_NOP, &GROUND_VELOCITY_ORIGINAL_C),
    Record::patch(0x261EB18, &TURN_SPEED_HOOK, &TURN_SPEED_ORIGINAL),
    Record::cave(0x3C46B9C, &TURN_SPEED_ROUTINE),
    Record::patch(0x3B5C7C8, &MOVIE_FRAMES_HOOK, &MOVIE_FRAMES_ORIGINAL),
    Record::cave(0x3C46BC4, &MOVIE_FRAMES_ROUTINE),
    Record::patch(0x3296B08, &SCARECROW_RECORD_HOOK, &SCARECROW_CUT_ORIGINAL),
    Record::cave(0x3C46AB0, &SCARECROW_RECORD_ROUTINE),
    Record::patch(0x32926C8, &SCARECROW_LAST_HOOK, &SCARECROW_CUT_ORIGINAL),
    Record::cave(0x3C46AD8, &SCARECROW_LAST_ROUTINE),
    Record::patch(0x14632C, &CLOTH_DT_CALL_A, &CLOTH_DT_ORIGINAL_A),
    Record::patch(0x2AB804, &CLOTH_DT_CALL_B, &CLOTH_DT_ORIGINAL_B),
    Record::patch(0x2C207A4, &CLOTH_DT_CALL_C, &CLOTH_DT_ORIGINAL_A),
    Record::patch(0x2C2208C, &CLOTH_DT_CALL_D, &CLOTH_DT_ORIGINAL_D),
    Record::cave(0x3C46B00, &CLOTH_DT_ROUTINE_S0),
    Record::cave(0x3C46B30, &CLOTH_DT_ROUTINE_S2),
];

const EPONA_JUMP_ORIGINAL: [u8; 4] = [0x45, 0x01, 0x00, 0x54];
const EPONA_JUMP_BRANCH: [u8; 4] = [0x65, 0x30, 0x40, 0x14];
const EPONA_JUMP_ROUTINE: [u8; 72] = [
    0x25, 0x02, 0x00, 0x54, 0x69, 0x92, 0x00, 0xD0, 0x29, 0x91, 0x45, 0xF9, 0xA9, 0x01, 0x00, 0xB4,
    0x2A, 0x11, 0x40, 0xB9, 0x6A, 0x01, 0x00, 0x34, 0x09, 0xF0, 0xA7, 0x52, 0x5F, 0x01, 0x09, 0x6B,
    0x02, 0x01, 0x00, 0x54, 0x49, 0x01, 0x26, 0x1E, 0x3F, 0x01, 0x00, 0x71, 0xAD, 0x00, 0x00, 0x54,
    0x29, 0x01, 0x0A, 0x6B, 0x29, 0x55, 0x89, 0x5A, 0x3F, 0x09, 0x40, 0x71, 0x49, 0x00, 0x00, 0x54,
    0x8C, 0xCF, 0xBF, 0x17, 0x94, 0xCF, 0xBF, 0x17,
];
const EPONA_JUMP_RECORDS: &[Record] = &[
    Record::patch(0x2C3AE20, &EPONA_JUMP_BRANCH, &EPONA_JUMP_ORIGINAL),
    Record::cave(0x3C46FB4, &EPONA_JUMP_ROUTINE),
];

const ZERO_12: [u8; 12] = [0; 12];
const ZERO_16: [u8; 16] = [0; 16];
const ZERO_20: [u8; 20] = [0; 20];
const ZERO_24: [u8; 24] = [0; 24];
const ZERO_28: [u8; 28] = [0; 28];

const CLOCK_CARRY_ORIGINAL: [u8; 4] = [0x08, 0x00, 0x38, 0x1E];
const CLOCK_CARRY_HOOK: [u8; 4] = [0xC9, 0xBE, 0x4E, 0x14];
const CLOCK_CARRY_ENTRY: [u8; 28] = [
    0x08, 0x00, 0x38, 0x1E, 0xED, 0x3B, 0xBB, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xF0, 0x47, 0x01, 0xAD,
    0x0D, 0x42, 0x3B, 0xD5, 0xED, 0x23, 0x00, 0xF9, 0x53, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_RANGE_CHECK: [u8; 24] = [
    0x0D, 0x00, 0x26, 0x1E, 0xED, 0x0E, 0x00, 0x34, 0x0E, 0x00, 0xA8, 0x52, 0xBF, 0x01, 0x0E, 0x6B,
    0x82, 0x0E, 0x00, 0x54, 0x13, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_SETUP: [u8; 20] = [
    0x0E, 0xB9, 0x01, 0x90, 0xCE, 0x01, 0x00, 0x91, 0x6D, 0xF2, 0x00, 0x91, 0x0F, 0x00, 0x80, 0x52,
    0xB3, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_LOOKUP: [u8; 28] = [
    0xD0, 0x79, 0x6F, 0xF8, 0x1F, 0x02, 0x0D, 0xEB, 0xE0, 0xEB, 0xFF, 0x54, 0xEF, 0x05, 0x00, 0x91,
    0xFF, 0x21, 0x00, 0xF1, 0x63, 0xFF, 0xFF, 0x54, 0x3A, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_MISS: [u8; 28] = [
    0xCF, 0x81, 0x40, 0xB9, 0xF0, 0x05, 0x00, 0x11, 0x10, 0x0A, 0x00, 0x12, 0xD0, 0x81, 0x00, 0xB9,
    0xCD, 0x79, 0x2F, 0xF8, 0x10, 0xE4, 0x00, 0x2F, 0x33, 0xFF, 0xFF, 0x17,
];
const CLOCK_CARRY_HIT: [u8; 12] = [
    0xCD, 0x01, 0x01, 0x91, 0xB0, 0x79, 0x6F, 0xFC, 0x16, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_CALC: [u8; 24] = [
    0x11, 0xC0, 0x22, 0x1E, 0x10, 0x2A, 0x71, 0x1E, 0x08, 0x02, 0x78, 0x1E, 0x11, 0x01, 0x62, 0x1E,
    0x10, 0x3A, 0x71, 0x1E, 0x13, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_STORE: [u8; 12] = [
    0xCD, 0x01, 0x01, 0x91, 0xB0, 0x79, 0x2F, 0xFC, 0x16, 0x00, 0x00, 0x14,
];
const CLOCK_CARRY_DONE: [u8; 24] = [
    0xED, 0x23, 0x40, 0xF9, 0x0D, 0x42, 0x1B, 0xD5, 0xF0, 0x47, 0x41, 0xAD, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC5, 0xA8, 0x62, 0x40, 0xB1, 0x17,
];
const CLOCK_CARRY_RECORDS: &[Record] = &[
    Record::patch(0x3CDFC0, &CLOCK_CARRY_HOOK, &CLOCK_CARRY_ORIGINAL),
    Record::slot(0x177DAE4, &CLOCK_CARRY_ENTRY, &ZERO_28),
    Record::slot(0x177DC48, &CLOCK_CARRY_RANGE_CHECK, &ZERO_24),
    Record::slot(0x177DCA8, &CLOCK_CARRY_SETUP, &ZERO_20),
    Record::slot(0x177DF84, &CLOCK_CARRY_LOOKUP, &ZERO_28),
    Record::slot(0x177E084, &CLOCK_CARRY_MISS, &ZERO_28),
    Record::slot(0x177DD08, &CLOCK_CARRY_HIT, &ZERO_12),
    Record::slot(0x177DD68, &CLOCK_CARRY_CALC, &ZERO_24),
    Record::slot(0x177DDC8, &CLOCK_CARRY_STORE, &ZERO_12),
    Record::slot(0x177DE28, &CLOCK_CARRY_DONE, &ZERO_24),
];

const TEXT_SPEED_SITE: usize = 0xAFE1B4;
const TEXT_SPEED_CAVE: usize = 0x3C46F00;
const TEXT_SPEED_ORIGINAL: [u8; 4] = [0x69, 0xDE, 0x40, 0x79];
const TEXT_SPEED_BRANCH: [u8; 4] = [0x53, 0x23, 0xC5, 0x14];
macro_rules! text_speed_with_scale {
    ($b0:expr, $b1:expr, $b2:expr, $b3:expr) => {
        [
            0x90, 0x00, 0x00, 0x1C, 0x00, 0x08, 0x30, 0x1E, 0x69, 0xDE, 0x40, 0x79, 0xAB, 0xDC,
            0x3A, 0x17, $b0, $b1, $b2, $b3,
        ]
    };
}
const TEXT_SPEED_2X_CAVE: [u8; 20] = text_speed_with_scale!(0x00, 0x00, 0x00, 0x40);
const TEXT_SPEED_4X_CAVE: [u8; 20] = text_speed_with_scale!(0x00, 0x00, 0x80, 0x40);
const TEXT_SPEED_INSTANT_CAVE: [u8; 20] = text_speed_with_scale!(0x00, 0x24, 0x74, 0x49);
const TEXT_SPEED_2X_RECORDS: &[Record] = &[
    Record::patch(TEXT_SPEED_SITE, &TEXT_SPEED_BRANCH, &TEXT_SPEED_ORIGINAL),
    Record::cave(TEXT_SPEED_CAVE, &TEXT_SPEED_2X_CAVE),
];
const TEXT_SPEED_4X_RECORDS: &[Record] = &[
    Record::patch(TEXT_SPEED_SITE, &TEXT_SPEED_BRANCH, &TEXT_SPEED_ORIGINAL),
    Record::cave(TEXT_SPEED_CAVE, &TEXT_SPEED_4X_CAVE),
];
const TEXT_SPEED_INSTANT_RECORDS: &[Record] = &[
    Record::patch(TEXT_SPEED_SITE, &TEXT_SPEED_BRANCH, &TEXT_SPEED_ORIGINAL),
    Record::cave(TEXT_SPEED_CAVE, &TEXT_SPEED_INSTANT_CAVE),
];

const WEB_CARRY_ORIGINAL: [u8; 4] = [0x16, 0x00, 0x38, 0x1E];
const WEB_CARRY_HOOK: [u8; 4] = [0x51, 0x2D, 0x45, 0x14];
const WEB_CARRY_ENTRY: [u8; 28] = [
    0x16, 0x00, 0x38, 0x1E, 0xED, 0x3B, 0xBB, 0xA9, 0xEF, 0x43, 0x01, 0xA9, 0xF0, 0x47, 0x01, 0xAD,
    0x0D, 0x42, 0x3B, 0xD5, 0xED, 0x23, 0x00, 0xF9, 0x13, 0x00, 0x00, 0x14,
];
const WEB_CARRY_RANGE_CHECK: [u8; 24] = [
    0x0D, 0x00, 0x26, 0x1E, 0xED, 0x15, 0x00, 0x34, 0x0E, 0xF0, 0xA7, 0x52, 0xBF, 0x01, 0x0E, 0x6B,
    0x82, 0x15, 0x00, 0x54, 0x13, 0x00, 0x00, 0x14,
];
const WEB_CARRY_SETUP: [u8; 20] = [
    0xEE, 0xB8, 0x01, 0xF0, 0xCE, 0x01, 0x04, 0x91, 0x6D, 0xE2, 0x23, 0x91, 0x0F, 0x00, 0x80, 0x52,
    0x7B, 0x00, 0x00, 0x14,
];
const WEB_CARRY_LOOKUP: [u8; 28] = [
    0xD0, 0x79, 0x6F, 0xF8, 0x1F, 0x02, 0x0D, 0xEB, 0xE0, 0xF2, 0xFF, 0x54, 0xEF, 0x05, 0x00, 0x91,
    0xFF, 0x21, 0x00, 0xF1, 0x63, 0xFF, 0xFF, 0x54, 0x3A, 0x00, 0x00, 0x14,
];
const WEB_CARRY_MISS: [u8; 28] = [
    0xCF, 0x81, 0x40, 0xB9, 0xF0, 0x05, 0x00, 0x11, 0x10, 0x0A, 0x00, 0x12, 0xD0, 0x81, 0x00, 0xB9,
    0xCD, 0x79, 0x2F, 0xF8, 0x10, 0xE4, 0x00, 0x2F, 0x6B, 0xFF, 0xFF, 0x17,
];
const WEB_CARRY_HIT: [u8; 12] = [
    0xCD, 0x01, 0x01, 0x91, 0xB0, 0x79, 0x6F, 0xFC, 0x16, 0x00, 0x00, 0x14,
];
const WEB_CARRY_CALC: [u8; 24] = [
    0x11, 0xC0, 0x22, 0x1E, 0x10, 0x2A, 0x71, 0x1E, 0x16, 0x02, 0x78, 0x1E, 0xD1, 0x02, 0x62, 0x1E,
    0x10, 0x3A, 0x71, 0x1E, 0x13, 0x00, 0x00, 0x14,
];
const WEB_CARRY_STORE: [u8; 12] = [
    0xCD, 0x01, 0x01, 0x91, 0xB0, 0x79, 0x2F, 0xFC, 0x4E, 0x00, 0x00, 0x14,
];
const WEB_CARRY_DONE: [u8; 24] = [
    0xED, 0x23, 0x40, 0xF9, 0x0D, 0x42, 0x1B, 0xD5, 0xF0, 0x47, 0x41, 0xAD, 0xEF, 0x43, 0x41, 0xA9,
    0xED, 0x3B, 0xC5, 0xA8, 0xE2, 0xD1, 0xBA, 0x17,
];
const WEB_CARRY_RECORDS: &[Record] = &[
    Record::patch(0x632D00, &WEB_CARRY_HOOK, &WEB_CARRY_ORIGINAL),
    Record::slot(0x177E244, &WEB_CARRY_ENTRY, &ZERO_28),
    Record::slot(0x177E2A8, &WEB_CARRY_RANGE_CHECK, &ZERO_24),
    Record::slot(0x177E308, &WEB_CARRY_SETUP, &ZERO_20),
    Record::slot(0x177E504, &WEB_CARRY_LOOKUP, &ZERO_28),
    Record::slot(0x177E604, &WEB_CARRY_MISS, &ZERO_28),
    Record::slot(0x177E368, &WEB_CARRY_HIT, &ZERO_12),
    Record::slot(0x177E3C8, &WEB_CARRY_CALC, &ZERO_24),
    Record::slot(0x177E428, &WEB_CARRY_STORE, &ZERO_12),
    Record::slot(0x177E568, &WEB_CARRY_DONE, &ZERO_24),
];

const TEXTURE_COMPACTION_ORIGINAL: [u8; 4] = [0x88, 0x00, 0x08, 0x36];
const TEXTURE_COMPACTION_HOOK: [u8; 4] = [0xD3, 0x1E, 0x40, 0x14];
const TEXTURE_COMPACTION_ENTRY: [u8; 28] = [
    0x88, 0xDD, 0x0F, 0x36, 0xE8, 0x27, 0xBE, 0xA9, 0xEA, 0x2F, 0x01, 0xA9, 0x0B, 0x42, 0x3B, 0xD5,
    0x08, 0xE0, 0x3B, 0xD5, 0x88, 0xDF, 0xFF, 0xB4, 0x93, 0xFE, 0xFF, 0x17,
];
const TEXTURE_COMPACTION_SAMPLE: [u8; 24] = [
    0x29, 0xE0, 0x3B, 0xD5, 0xEA, 0xB8, 0x01, 0xF0, 0x4A, 0x01, 0x08, 0x91, 0x4A, 0x05, 0x40, 0xF9,
    0x8A, 0x06, 0x00, 0xB4, 0x1B, 0x00, 0x00, 0x14,
];
const TEXTURE_COMPACTION_ELAPSED: [u8; 24] = [
    0xEA, 0xB8, 0x01, 0xF0, 0x4A, 0x01, 0x41, 0xF9, 0x2A, 0x01, 0x0A, 0xCB, 0x5F, 0x01, 0x08, 0xEB,
    0x83, 0x05, 0x00, 0x54, 0x13, 0x00, 0x00, 0x14,
];
const TEXTURE_COMPACTION_RECORD: [u8; 20] = [
    0xEA, 0xB8, 0x01, 0xF0, 0x4A, 0x01, 0x08, 0x91, 0x28, 0x00, 0x80, 0xD2, 0x49, 0x21, 0x00, 0xA9,
    0x2C, 0x00, 0x00, 0x14,
];
const TEXTURE_COMPACTION_SKIP: [u8; 16] = [
    0x0B, 0x42, 0x1B, 0xD5, 0xEA, 0x2F, 0x41, 0xA9, 0xE8, 0x27, 0xC2, 0xA8, 0x45, 0xE2, 0xBF, 0x17,
];
const TEXTURE_COMPACTION_ALLOW: [u8; 16] = [
    0x0B, 0x42, 0x1B, 0xD5, 0xEA, 0x2F, 0x41, 0xA9, 0xE8, 0x27, 0xC2, 0xA8, 0x2A, 0xE2, 0xBF, 0x17,
];
const TEXTURE_COMPACTION_RECORDS: &[Record] = &[
    Record::patch(
        0x777278,
        &TEXTURE_COMPACTION_HOOK,
        &TEXTURE_COMPACTION_ORIGINAL,
    ),
    Record::slot(0x177EDC4, &TEXTURE_COMPACTION_ENTRY, &ZERO_28),
    Record::slot(0x177E828, &TEXTURE_COMPACTION_SAMPLE, &ZERO_24),
    Record::slot(0x177E8A8, &TEXTURE_COMPACTION_ELAPSED, &ZERO_24),
    Record::slot(0x177E908, &TEXTURE_COMPACTION_RECORD, &ZERO_20),
    Record::slot(0x177E968, &TEXTURE_COMPACTION_SKIP, &ZERO_16),
    Record::slot(0x177E9C8, &TEXTURE_COMPACTION_ALLOW, &ZERO_16),
];

const YOSHI_SCENE_PROFILE_ORIGINAL: [u8; 1] = [0x35];
const YOSHI_SCENE_PROFILE_FULL: [u8; 1] = [0x19];
const YOSHI_SCENE_PROFILE_RECORDS: &[Record] = &[Record::patch(
    0x7582677,
    &YOSHI_SCENE_PROFILE_FULL,
    &YOSHI_SCENE_PROFILE_ORIGINAL,
)];

const WONDER_RATE_DIVIDE: [u8; 4] = [0x01, 0x0D, 0xCA, 0x1A];
const WONDER_INTERVAL_LOAD: [u8; 4] = [0x81, 0x42, 0x40, 0xB9];
const WONDER_FALLBACK_LOAD: [u8; 4] = [0x81, 0x52, 0x40, 0xB9];
const WONDER_RATE_60: [u8; 4] = [0x81, 0x07, 0x80, 0x52];
const WONDER_INTERVAL_1: [u8; 4] = [0x21, 0x00, 0x80, 0x52];
const WONDER_60FPS_RECORDS: &[Record] = &[
    Record::patch(0x1BBFB0, &WONDER_RATE_60, &WONDER_RATE_DIVIDE),
    Record::patch(0x1BBFCC, &WONDER_INTERVAL_1, &WONDER_INTERVAL_LOAD),
    Record::patch(0x1BC04C, &WONDER_RATE_60, &WONDER_FALLBACK_LOAD),
];

const SHADOW_STEPS_ORIGINAL: [u8; 4] = [0xAA, 0xEA, 0x44, 0xB9];
const SHADOW_STEPS_BRANCH: [u8; 4] = [0x30, 0x0B, 0xE8, 0x14];
const SHADOW_STEPS_CAVE: [u8; 16] = [
    0xAA, 0xEA, 0x44, 0xB9, 0x4A, 0xFD, 0x00, 0x11, 0x4A, 0x65, 0x1A, 0x12, 0xCE, 0xF4, 0x17, 0x17,
];
const SHADOW_STEPS_RECORDS: &[Record] = &[
    Record::patch(0x244100, &SHADOW_STEPS_BRANCH, &SHADOW_STEPS_ORIGINAL),
    Record::cave(0x3C46DC0, &SHADOW_STEPS_CAVE),
];

const OCCLUSION_LDRB_ORIGINAL: [u8; 4] = [0x08, 0x00, 0x40, 0x39];
const OCCLUSION_MOV_WZR: [u8; 4] = [0xE8, 0x03, 0x1F, 0x2A];
const OCCLUSION_OFF_RECORDS: &[Record] = &[
    Record::patch(0x11ACE68, &OCCLUSION_MOV_WZR, &OCCLUSION_LDRB_ORIGINAL),
    Record::patch(0x11B828C, &OCCLUSION_MOV_WZR, &OCCLUSION_LDRB_ORIGINAL),
    Record::patch(0x11B82A4, &OCCLUSION_MOV_WZR, &OCCLUSION_LDRB_ORIGINAL),
    Record::patch(0x11ACEC8, &OCCLUSION_MOV_WZR, &OCCLUSION_LDRB_ORIGINAL),
    Record::patch(0x11B82DC, &OCCLUSION_MOV_WZR, &OCCLUSION_LDRB_ORIGINAL),
    Record::patch(0x11B82F4, &OCCLUSION_MOV_WZR, &OCCLUSION_LDRB_ORIGINAL),
];

const fn render_scale_bytes(float_top: u32) -> [u8; 4] {
    (0x52A0_0009u32 | (float_top << 5)).to_le_bytes()
}
const RENDER_SCALE_ONE: [u8; 4] = render_scale_bytes(0x3F80);
const RENDER_SCALE_360: [u8; 4] = render_scale_bytes(0x3E80);
const RENDER_SCALE_540: [u8; 4] = render_scale_bytes(0x3EC0);
const RENDER_SCALE_720: [u8; 4] = render_scale_bytes(0x3F00);
const RENDER_SCALE_810: [u8; 4] = render_scale_bytes(0x3F10);
const RENDER_SCALE_900: [u8; 4] = render_scale_bytes(0x3F20);
const RENDER_SCALE_1080: [u8; 4] = render_scale_bytes(0x3F40);
const RENDER_SCALE_1260: [u8; 4] = render_scale_bytes(0x3F60);

const RENDER_360_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_360,
    &RENDER_SCALE_ONE,
)];
const RENDER_540_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_540,
    &RENDER_SCALE_ONE,
)];
const RENDER_720_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_720,
    &RENDER_SCALE_ONE,
)];
const RENDER_810_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_810,
    &RENDER_SCALE_ONE,
)];
const RENDER_900_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_900,
    &RENDER_SCALE_ONE,
)];
const RENDER_1080_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_1080,
    &RENDER_SCALE_ONE,
)];
const RENDER_1260_RECORDS: &[Record] = &[Record::patch(
    0x806420,
    &RENDER_SCALE_1260,
    &RENDER_SCALE_ONE,
)];

#[allow(clippy::single_range_in_vec_init)]
const PATCHES: &[BuiltinPatch] = &[
    BuiltinPatch {
        name: MAX_RESOLUTION_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: MAX_RESOLUTION_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: TEXTURE_POOL_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: TEXTURE_POOL_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: FPS_UNLOCK_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: FPS_UNLOCK_RECORDS,
        states: &[0x4E9FE00..0x4E9FEA8, 0x4E9EE00..0x4E9EF10],
        hooks: FPS_UNLOCK_HOOKS,
        replaces: Some(DYNAMIC_FPS_NAME),
        requires_patch: None,
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: "Epona jump (FPS Unlock)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: EPONA_JUMP_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: Some(FPS_UNLOCK_NAME),
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: DYNAMIC_FPS_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: DYNAMIC_FPS_RECORDS,
        states: &[0x4E9FE00..0x4E9FE98],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: "OoT day/night clock (FPS Unlock)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: CLOCK_CARRY_RECORDS,
        states: &[0x4E9D000..0x4E9D088],
        hooks: &[],
        replaces: None,
        requires_patch: Some(FPS_UNLOCK_NAME),
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: "Text Speed Instant (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: TEXT_SPEED_INSTANT_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Text Speed"),
        description: "",
    },
    BuiltinPatch {
        name: "Text Speed 4x (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: TEXT_SPEED_4X_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Text Speed"),
        description: "",
    },
    BuiltinPatch {
        name: "Text Speed 2x (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: TEXT_SPEED_2X_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Text Speed"),
        description: "",
    },
    BuiltinPatch {
        name: "Deku Tree floor web (FPS Unlock)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: WEB_CARRY_RECORDS,
        states: &[0x4E9D100..0x4E9D188],
        hooks: &[],
        replaces: None,
        requires_patch: Some(FPS_UNLOCK_NAME),
        off_by_default: false,
        group: None,
        description: "",
    },
    BuiltinPatch {
        name: TEXTURE_COMPACTION_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: TEXTURE_COMPACTION_RECORDS,
        states: &[0x4E9D200..0x4E9D210],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: None,
        description: "Limits requests for new texture heap compaction rounds to once per guest \
                      second. This may increase heap fragmentation. Restart the game to apply.",
    },
    BuiltinPatch {
        name: SHADOW_STEPS_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: SHADOW_STEPS_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: false,
        group: None,
        description: "Rounds local shadow depth map sizes up to multiples of 64 so moving around \
                      reuses pooled depth images instead of creating thousands of new ones.",
    },
    BuiltinPatch {
        name: OCCLUSION_OFF_NAME,
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: OCCLUSION_OFF_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: None,
        description: "Disables z-prepass occlusion and samples-passed queries so every candidate \
                      object draws without waiting on occlusion readbacks.",
    },
    BuiltinPatch {
        name: "1260p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_1260_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      2240x1260 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "1080p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_1080_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      1920x1080 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "900p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_900_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      1600x900 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "810p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_810_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      1440x810 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "720p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_720_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      1280x720 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "540p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_540_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      960x540 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "360p render (built-in)",
        title_id: OCARINA_OF_TIME,
        module: "main",
        build_id: &OCARINA_MAIN_BUILD_ID,
        records: RENDER_360_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: Some("Render Height"),
        description: "Requires Max Resolution (built-in). Scales OoT's internal 3D render to \
                      640x360 while keeping emulator Resolution Scale at 1x.",
    },
    BuiltinPatch {
        name: "Full Scene Resolution with FXAA (built-in)",
        title_id: YOSHI_MYSTERIOUS_BOOK,
        module: "main",
        build_id: &YOSHI_MAIN_BUILD_ID,
        records: YOSHI_SCENE_PROFILE_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: true,
        group: None,
        description: "Yoshi and the Mysterious Book 2.0.0 only. Renders gameplay at full \
                      scene resolution with FXAA instead of temporal AA for a sharper image. \
                      Expect more aliasing in motion and likely higher GPU cost.",
    },
    BuiltinPatch {
        name: "60 FPS (built-in)",
        title_id: MARIO_WONDER,
        module: "main",
        build_id: &WONDER_MAIN_BUILD_ID,
        records: WONDER_60FPS_RECORDS,
        states: &[],
        hooks: &[],
        replaces: None,
        requires_patch: None,
        off_by_default: false,
        group: None,
        description: "Runs the game at 60 FPS, as on the console. A PC that can't hold 60 FPS \
                      plays slow frames in slow motion; turn this off for the game's own 30 FPS \
                      mode. Restart the game to apply.",
    },
];

/// Lists the user-toggleable built-in add-ons registered for `title_id` in UI display order.
pub(crate) fn list_for_title(title_id: u64) -> Vec<BuiltinAddon> {
    PATCHES
        .iter()
        .filter(|patch| patch.title_id == title_id && patch.requires_patch.is_none())
        .map(|patch| BuiltinAddon {
            name: patch.name,
            off_by_default: patch.off_by_default,
            replaces: PATCHES
                .iter()
                .find(|other| other.title_id == title_id && other.replaces == Some(patch.name))
                .map(|other| other.name),
            description: patch.description,
        })
        .collect()
}

fn matches(patch: &BuiltinPatch, title_id: u64, module: &Module) -> bool {
    patch.title_id == title_id
        && patch.module == module.name
        && module.build_id.starts_with(patch.build_id)
        && module.build_id[patch.build_id.len()..]
            .iter()
            .all(|&b| b == 0)
}

fn fits(patch: &BuiltinPatch, module: &Module, original: &[u8]) -> bool {
    let [text, rodata, data] = &module.segments;
    let text_end = (text.offset + text.size).min(rodata.offset) as usize;
    let records = patch.records.iter().all(|r| {
        let range = r.offset..r.offset + r.bytes.len();
        let unpatched = if r.executable_padding {
            range.start >= text.offset as usize
                && range.end <= text_end
                && original
                    .get(range.clone())
                    .is_some_and(|bytes| bytes.iter().all(|&b| b == 0))
        } else {
            original.get(r.offset..r.offset + r.original.len()) == Some(r.original)
        };
        unpatched && module.image.get(range.clone()) == original.get(range)
    });
    let zero = |image: &[u8], state: &Range<usize>| {
        image
            .get(state.start.min(image.len())..state.end.min(image.len()))
            .is_some_and(|bytes| bytes.iter().all(|&b| b == 0))
    };
    let states = patch.states.iter().all(|state| {
        !state.is_empty()
            && state.start >= data.offset as usize
            && state.end as u64 <= module.size
            && zero(original, state)
            && zero(&module.image, state)
    });
    records && states
}

fn encode_branch(from: usize, to: usize) -> [u8; 4] {
    let words = ((to as isize - from as isize) >> 2) as u32 & 0x03FF_FFFF;
    (0x1400_0000 | words).to_le_bytes()
}

fn apply_hook(patch: &BuiltinPatch, module: &mut Module, original: &[u8], hook: &Hook) {
    let [text, rodata, _] = &module.segments;
    let text_end = (text.offset + text.size).min(rodata.offset) as usize;
    let function_end = (hook.function_offset.saturating_add(hook.function_size)).min(text_end);
    let mut site = 0usize;
    let mut found = 0usize;
    let mut at = hook.function_offset;
    while at + hook.pattern.len() <= function_end {
        if original.get(at..at + hook.pattern.len()) == Some(hook.pattern) {
            site = at;
            found += 1;
        }
        at += 4;
    }
    let cave_range = hook.cave_offset..hook.cave_offset + hook.cave.len();
    let untouched = |range: Range<usize>| module.image.get(range.clone()) == original.get(range);
    let problem = if found == 0 {
        Some("its instruction is not in the function")
    } else if found != 1 {
        Some("its instruction is in the function more than once")
    } else if hook.cave.len() < 4
        || cave_range.start < text.offset as usize
        || cave_range.end > text_end
        || original
            .get(cave_range.clone())
            .is_none_or(|b| b.iter().any(|&x| x != 0))
    {
        Some("its code cave is not executable padding")
    } else if !untouched(cave_range.clone()) || !untouched(site..site + hook.pattern.len()) {
        Some("a user mod already patched it")
    } else {
        None
    };
    if let Some(problem) = problem {
        tracing::warn!(
            "{}: \"{}\" leaves out its {} hook at +{:#x}: {problem}",
            module.name,
            patch.name,
            hook.name,
            hook.function_offset
        );
        return;
    }
    let back = hook.cave_offset + hook.cave.len() - 4;
    let to_cave = encode_branch(site, hook.cave_offset);
    let to_site = encode_branch(back, site + 4);
    module.image[cave_range.start..back].copy_from_slice(&hook.cave[..hook.cave.len() - 4]);
    module.image[back..back + 4].copy_from_slice(&to_site);
    module.image[site..site + 4].copy_from_slice(&to_cave);
    tracing::info!(
        "{}: \"{}\": {} hook at +{site:#x}",
        module.name,
        patch.name,
        hook.name
    );
}

/// Returns whether a built-in patch is enabled under `disabled`.
pub(crate) fn is_enabled(patch_name: &str, off_by_default: bool, disabled: &[String]) -> bool {
    if off_by_default {
        disabled.iter().any(|d| d == &enabled_addon_key(patch_name))
    } else {
        !disabled.iter().any(|d| d == patch_name)
    }
}

/// Applies the built-in patches for `module` of `title_id`, respecting `disabled` add-ons
/// (`"Name"` disables a default-on patch; `"+Name"` enables an `off_by_default` patch).
/// `original` is the module's image before user patches changed it, `None` when none did.
pub(crate) fn apply(
    title_id: u64,
    module: &mut Module,
    original: Option<&[u8]>,
    disabled: &[String],
) {
    let owned_original;
    let original = match original {
        Some(o) => o,
        None => {
            owned_original = module.image.clone();
            &owned_original
        }
    };

    let mut applied: Vec<&'static str> = Vec::new();
    let mut replaced: Vec<&'static str> = Vec::new();
    let mut applied_groups: Vec<&'static str> = Vec::new();

    for patch in PATCHES {
        if patch.title_id != title_id || patch.module != module.name {
            continue;
        }
        if let Some(required) = patch.requires_patch
            && !applied.contains(&required)
        {
            continue;
        }
        if !is_enabled(patch.name, patch.off_by_default, disabled) {
            if !patch.off_by_default {
                tracing::info!(
                    "{}: skipping \"{}\": disabled in Add-Ons",
                    module.name,
                    patch.name
                );
            }
            continue;
        }
        if let Some(group) = patch.group
            && applied_groups.contains(&group)
        {
            tracing::info!(
                "{}: skipping \"{}\": another {group} built-in is applied",
                module.name,
                patch.name
            );
            continue;
        }
        if replaced.contains(&patch.name) {
            tracing::info!(
                "{}: skipping \"{}\": included in an applied built-in patch",
                module.name,
                patch.name
            );
            continue;
        }
        if !matches(patch, title_id, module) {
            tracing::info!(
                "{}: skipping \"{}\": build id mismatch",
                module.name,
                patch.name
            );
            continue;
        }
        if !fits(patch, module, original) {
            tracing::warn!(
                "{}: skipping built-in patch \"{}\": the module differs from the build it was \
                 made for, or a mod already changed it",
                module.name,
                patch.name
            );
            continue;
        }

        for r in patch.records {
            module.image[r.offset..r.offset + r.bytes.len()].copy_from_slice(r.bytes);
        }
        for hook in patch.hooks {
            apply_hook(patch, module, original, hook);
        }
        tracing::info!("{}: applied built-in patch \"{}\"", module.name, patch.name);
        applied.push(patch.name);
        if let Some(replaces) = patch.replaces {
            replaced.push(replaces);
        }
        if let Some(group) = patch.group {
            applied_groups.push(group);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exefs::load_exefs;
    use crate::npdm::tests::{NpdmSpec, build_npdm};
    use crate::nso::HashCheck;
    use crate::nso::tests::{Compression, build_nso, segments};
    use crate::patch::{ModOptions, Mods};
    use lichii_fs::MemDir;

    const CODE_END: usize = 0x3C46000;
    const MODULE_END: u32 = 0x4EA0000;

    fn fake_main(build_id: &[u8]) -> Module {
        let [_, rodata, data] = segments(1);
        let mut text = vec![0u8; 0x3C46400];
        for (i, byte) in text[..CODE_END].iter_mut().enumerate() {
            *byte = (i as u8) | 1;
        }
        for patch in PATCHES.iter().filter(|p| p.title_id == OCARINA_OF_TIME) {
            for r in patch.records.iter().filter(|r| !r.executable_padding) {
                text[r.offset..][..r.original.len()].copy_from_slice(r.original);
            }
            for hook in patch.hooks {
                let site = hook.function_offset + 0x40;
                text[site..site + hook.pattern.len()].copy_from_slice(hook.pattern);
            }
        }
        let data_end = 0x3C48000 + data.len() as u32;
        let mut nso = build_nso(
            &[text, rodata, data],
            Compression::None,
            MODULE_END - data_end,
            false,
        );
        nso[0x40..0x60].fill(0);
        nso[0x40..0x40 + build_id.len()].copy_from_slice(build_id);
        let exefs = MemDir::new()
            .with_bytes("main", nso)
            .with_bytes("main.npdm", build_npdm(&NpdmSpec::default()));
        let (_, mut modules) = load_exefs(&exefs, HashCheck::Skip).unwrap();
        let main = modules.remove(0);
        assert_eq!(main.segments[2].offset, 0x3C48000);
        assert_eq!(main.size, u64::from(MODULE_END));
        main
    }

    fn patched(module: &Module, builtin_patches: bool, disabled: &[&str]) -> Module {
        let options = ModOptions {
            builtin_patches,
            disabled: disabled.iter().map(|s| (*s).to_owned()).collect(),
            ..ModOptions::default()
        };
        let mut modules = vec![module.clone()];
        Mods::find(&options, OCARINA_OF_TIME)
            .unwrap()
            .patch_modules(&mut modules)
            .unwrap();
        modules.remove(0)
    }

    fn call_target(image: &[u8], offset: usize) -> usize {
        let word = u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap());
        assert_eq!(word >> 26, 0b100101, "bl at {offset:#x}");
        let delta = ((word << 6) as i32 >> 6) as isize * 4;
        offset.wrapping_add_signed(delta)
    }

    fn branch_target(image: &[u8], offset: usize) -> usize {
        let word = u32::from_le_bytes(image[offset..offset + 4].try_into().unwrap());
        assert_eq!(word >> 26, 0b000101, "b at {offset:#x}");
        let delta = ((word << 6) as i32 >> 6) as isize * 4;
        offset.wrapping_add_signed(delta)
    }

    fn adrp_add(bytes: &[u8], at: usize, offset: usize) -> usize {
        let word = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        let (adrp, add) = (word(at), word(at + 4));
        assert_eq!(adrp & 0x9F00_0000, 0x9000_0000, "adrp at {offset:#x}");
        assert_eq!(add & 0xFFC0_0000, 0x9100_0000, "add at {offset:#x}");
        let page = (((adrp >> 5) & 0x7FFFF) << 2 | (adrp >> 29) & 3) as usize;
        (offset & !0xFFF) + (page << 12) + ((add >> 10) & 0xFFF) as usize
    }

    #[test]
    fn fps_unlock_replaces_dynamic_fps_and_installs_hooks() {
        let main = patched(&fake_main(&OCARINA_MAIN_BUILD_ID), true, &[]);
        assert_eq!(call_target(&main.image, 0x6E6CC4), 0x3C46200);
        assert_eq!(main.image[0x3C46200..0x3C46280], FPS_UNLOCK_ROUTINE);
        assert_eq!(call_target(&main.image, 0x475F8), 0x3C46300);
        assert_eq!(&main.image[0x4760C..0x47610], &WATER_MUL_NOP);
        assert_eq!(call_target(&main.image, 0xC2A36C), 0x3C462C0);
        assert_eq!(&main.image[0x805BD0..0x805BD4], &MAX_RESOLUTION_BRANCH);
        assert_eq!(call_target(&main.image, 0x415334), 0x3C46280);
        assert_eq!(branch_target(&main.image, 0x415300), 0x3C46800);
        // Epona jump (FPS Unlock), OoT day/night clock, and Deku Tree floor web auto-apply.
        assert_eq!(&main.image[0x2C3AE20..0x2C3AE24], &EPONA_JUMP_BRANCH);
        assert_eq!(&main.image[0x3CDFC0..0x3CDFC4], &CLOCK_CARRY_HOOK);
        assert_eq!(&main.image[0x632D00..0x632D04], &WEB_CARRY_HOOK);
        // Both solver step hooks patched and jump back to site + 4.
        assert_eq!(branch_target(&main.image, 0x38AEBC0), 0x3C46400);
        assert_eq!(branch_target(&main.image, 0x3C46478), 0x38AEBC4);
        assert_eq!(branch_target(&main.image, 0x38AFC50), 0x3C46480);
        assert_eq!(branch_target(&main.image, 0x3C464F8), 0x38AFC54);
        // Off-by-default text speed and texture compaction stay unpatched.
        assert_eq!(&main.image[0xAFE1B4..0xAFE1B8], &TEXT_SPEED_ORIGINAL);
        assert_eq!(
            &main.image[0x777278..0x77727C],
            &TEXTURE_COMPACTION_ORIGINAL
        );
    }

    #[test]
    fn disabling_fps_unlock_falls_back_to_dynamic_fps_and_skips_dependents() {
        let base = fake_main(&OCARINA_MAIN_BUILD_ID);
        let main = patched(&base, true, &[FPS_UNLOCK_NAME]);
        assert_eq!(call_target(&main.image, 0x6E6CC4), 0x3C46200);
        assert_eq!(main.image[0x3C46200..0x3C46278], DYNAMIC_FPS_ROUTINE);
        assert_eq!(&main.image[0x415300..0x415304], &SOUND_ENTRY_ORIGINAL);
        assert_eq!(&main.image[0x2C3AE20..0x2C3AE24], &EPONA_JUMP_ORIGINAL);
        assert_eq!(&main.image[0x3CDFC0..0x3CDFC4], &CLOCK_CARRY_ORIGINAL);
        assert_eq!(&main.image[0x632D00..0x632D04], &WEB_CARRY_ORIGINAL);
    }

    #[test]
    fn off_by_default_and_text_speed_groups_behave_like_reference() {
        let base = fake_main(&OCARINA_MAIN_BUILD_ID);
        let instant = patched(
            &base,
            true,
            &[
                &enabled_addon_key("Text Speed 2x (built-in)"),
                &enabled_addon_key("Text Speed Instant (built-in)"),
                &enabled_addon_key(TEXTURE_COMPACTION_NAME),
            ],
        );
        // Instant precedes 2x in PATCHES, so Instant wins.
        assert_eq!(
            &instant.image[TEXT_SPEED_SITE..TEXT_SPEED_SITE + 4],
            &TEXT_SPEED_BRANCH
        );
        assert_eq!(
            &instant.image[TEXT_SPEED_CAVE..TEXT_SPEED_CAVE + 20],
            &TEXT_SPEED_INSTANT_CAVE
        );
        assert_eq!(&instant.image[0x777278..0x77727C], &TEXTURE_COMPACTION_HOOK);
    }

    #[test]
    fn dynamic_fps_routines_address_its_state() {
        assert_eq!(adrp_add(&DYNAMIC_FPS_ROUTINE, 4, 0x3C46200 + 4), 0x4E9FE00);
        let slots = adrp_add(&WATER_STEPS_ROUTINE, 16, 0x3C46300 + 16);
        assert_eq!(slots, 0x4E9FE10);
    }

    #[test]
    fn ocarina_patches_skip_a_changed_module_whole() {
        let base = fake_main(&OCARINA_MAIN_BUILD_ID);
        let mut modded = base.clone();
        modded.image[0x805BD0..0x805BD4].copy_from_slice(&[0x1F, 0x20, 0x03, 0xD5]);
        let mut applied = modded.clone();
        apply(OCARINA_OF_TIME, &mut applied, Some(&base.image), &[]);
        // Max Resolution skipped, Texture Pool x2 still applied.
        assert_eq!(
            &applied.image[0x805BD0..0x805BD4],
            &[0x1F, 0x20, 0x03, 0xD5]
        );
        assert_eq!(call_target(&applied.image, 0xC2A36C), 0x3C462C0);

        let all_off = patched(&base, false, &[]);
        assert_eq!(all_off.image, base.image);

        let mut other_id = OCARINA_MAIN_BUILD_ID;
        other_id[19] ^= 1;
        let other = fake_main(&other_id);
        assert_eq!(patched(&other, true, &[]).image, other.image);
    }

    #[test]
    fn dynamic_fps_needs_its_state_in_zero_bss() {
        let main = fake_main(&OCARINA_MAIN_BUILD_ID);
        let mut modded = main.clone();
        modded.image.resize(MODULE_END as usize, 0);
        modded.image[0x4E9FE40] = 7;
        let mut skipped = modded.clone();
        apply(OCARINA_OF_TIME, &mut skipped, Some(&main.image), &[]);
        // Both FPS Unlock and Dynamic FPS require 0x4E9FE00.. to be zero.
        assert_eq!(&skipped.image[0x6E6CC4..0x6E6CC8], &DYNAMIC_FPS_ORIGINAL);
        // Max Resolution still applied.
        assert_eq!(&skipped.image[0x805BD0..0x805BD4], &MAX_RESOLUTION_BRANCH);
    }

    #[test]
    fn shadow_steps_and_render_height_apply_cleanly() {
        let base = fake_main(&OCARINA_MAIN_BUILD_ID);
        let default_patched = patched(&base, true, &[]);
        assert_eq!(branch_target(&default_patched.image, 0x244100), 0x3C46DC0);
        assert_eq!(
            &default_patched.image[0x3C46DC0..0x3C46DD0],
            &SHADOW_STEPS_CAVE
        );
        assert_eq!(
            &default_patched.image[0x806420..0x806424],
            &RENDER_SCALE_ONE
        );

        let scaled = patched(&base, true, &[&enabled_addon_key("720p render (built-in)")]);
        assert_eq!(&scaled.image[0x806420..0x806424], &RENDER_SCALE_720);
    }
}
