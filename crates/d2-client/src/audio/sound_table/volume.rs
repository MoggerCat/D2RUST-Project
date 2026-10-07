// Spec: specs/audio/sound-table.md (§8 volume and pan, §9 settings)
//! Volume and pan (`sound-table.md` §8) and the player's sound settings
//! (§9).
//!
//! Floats: 1.14d computes positions, distance, linear falloff and the
//! mode-0 gain/pan in `f32` (§8.1 r1, §8.2 r8, r10). The exactness target
//! is the integer volume and pan sent to the channel (voice log), so the
//! `f32` steps are reproduced as written, one IEEE `f32` operation per
//! spec operation, truncated as the original's float→int conversion
//! ([`ftol`]). Rust's `f32::sqrt` is exact (IEEE); `log2`/`powf` come from
//! the platform libm and may differ from the x87 results in the last ulp,
//! which can move a truncation boundary: covered by the voice-log trace
//! (open question 1). This is client code (hard rule 6 binds `d2-sim`).

use crate::audio::{AudioError, GainCurve, Gains, GAIN_UNITY};

/// Falloff min and max (`0x004825B0`, `0x00482610`, §8.1 r2).
pub fn falloff(f: i32) -> (i32, i32) {
    match f {
        0 => (60, 400),
        1 => (60, 700),
        2 => (200, 1_000),
        3 => (400, 1_500),
        4 => (2_000, 2_000),
        _ => (60, 700),
    }
}

/// The original's float → int conversion: truncation toward zero; NaN
/// and out-of-range give `0x80000000` (§12 r2).
pub fn ftol(x: f32) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// Mixer mode (`Sound Mixer`, §9).
pub type MixerMode = u8;

/// The settings of §9 (defaults per the table). d2rs reproduces mixer
/// mode 0 only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundSettings {
    pub mixer_mode: MixerMode,
    pub master_volume: i32,
    pub music_volume: i32,
    pub positional_bias: i32,
    pub npc_speech: i32,
    pub options_music: i32,
    /// The flag `0x007A061C` gating `Tracking` (§6.4 r1): not an option
    /// but the client's game-loaded flag (set by S→C 0x04, cleared by 0x05
    /// and at game init / end). Every request update runs in game, so it
    /// is on (§6.4 r1, answered ST-6).
    pub game_loaded: bool,
}

impl Default for SoundSettings {
    fn default() -> Self {
        SoundSettings {
            mixer_mode: 0,
            master_volume: 100,
            music_volume: 50,
            positional_bias: 50,
            npc_speech: 2,
            options_music: 1,
            game_loaded: true,
        }
    }
}

impl SoundSettings {
    /// Reads the stored keys (`0x00514B60`, §9) through `get`; a missing
    /// or out-of-range value keeps the default.
    pub fn from_store(get: impl Fn(&str) -> Option<i64>) -> Self {
        let mut s = SoundSettings::default();
        let read = |key: &str, max: i64, slot: &mut i32| {
            if let Some(v) = get(key).filter(|v| (0..=max).contains(v)) {
                *slot = v as i32;
            }
        };
        let mut mode = i32::from(s.mixer_mode);
        read("Sound Mixer", 2, &mut mode);
        s.mixer_mode = mode as u8;
        read("Master Volume", 100, &mut s.master_volume);
        read("Music Volume", 100, &mut s.music_volume);
        read("Positional Bias", 100, &mut s.positional_bias);
        read("NPC Speech", 2, &mut s.npc_speech);
        read("Options Music", 1, &mut s.options_music);
        s
    }
}

/// Inputs of the volume chain that are not the request's own.
#[derive(Clone, Copy, Debug)]
pub struct ChainInputs {
    pub music_vol: bool,
    pub state_duck_applies: bool,
    pub solo: bool,
    pub state_duck: i32,
    pub solo_duck: i32,
}

/// §8.2 r1–r5: request volume through the settings and the ducks
/// (integer, truncating). The result is the `v` compared in r7.
pub fn chain(v: i32, s: &SoundSettings, c: &ChainInputs) -> i32 {
    let mut v = v;
    if c.music_vol {
        v = s.music_volume.wrapping_mul(v) / 100;
    }
    v = s.master_volume.wrapping_mul(v) / 100;
    if c.state_duck != 100 && c.state_duck_applies {
        v = c.state_duck.wrapping_mul(v) / 100;
    }
    if c.solo_duck != 100 && !c.solo {
        v = c.solo_duck.wrapping_mul(v) / 100;
    }
    v
}

/// §8.2 r6 (mixer modes 1–2 only; not wired: d2rs reproduces mode 0).
pub fn positional_bias(v: i32, b: i32, capable_3d: bool, is_3d: bool) -> i32 {
    if b == 50 {
        return v;
    }
    let c = (2 * b - 100) / 3;
    if capable_3d && c > 0 {
        (255 - 255 * c / 50).wrapping_mul(v) / 255
    } else if is_3d && c < 0 {
        (255 * c / 50 + 255).wrapping_mul(v) / 255
    } else {
        v
    }
}

/// §8.2 r8: linear falloff on the unclamped distance, in `f32`.
pub fn linear_falloff(v: i32, x: f32, y: f32, falloff_column: i32) -> i32 {
    let (min, max) = falloff(falloff_column);
    let d = (x * x + y * y).sqrt();
    if d > min as f32 {
        ftol((max as f32 - d) * v as f32 / (max - min) as f32)
    } else {
        v
    }
}

/// §8.2 r9: the record's `Volume` (wrapping 32-bit multiply).
pub fn record_volume(v: i32, volume: u8) -> i32 {
    i32::from(volume).wrapping_mul(v) / 255
}

/// §8.2 r10 (`0x00516830`): mode-0 gain and pan of a non-stereo voice at
/// position (x, y, z).
pub fn mode0_gain_pan(pos: [f32; 3]) -> (i32, i32) {
    const SCALE: f32 = 0.003_125;
    let (x, y, z) = (pos[0] * SCALE, pos[1] * SCALE, pos[2] * SCALE);
    let r = (x * x + y * y + z * z).sqrt().min(100.0);
    let t = if r < 2.0 { 0.0 } else { 6.0 * (r / 2.0).log2() };
    let gain = ftol(255.0 / 10f32.powf(t / 20.0)).clamp(0, 255);
    let pan = ftol(x * 127.0 / 1.25 + 128.0).clamp(0, 255);
    (gain, pan)
}

/// §8.3 (`0x005165F0`): hundredths of a dB for `x` of `full`.
pub fn device_db(x: f32, full: f32) -> i32 {
    if x <= 0.0001 {
        -10_000
    } else if x >= full - 0.0001 {
        0
    } else {
        ftol(-2000.0 * (full / x).log10())
    }
}

/// Device volume (`0x005157B0`, §8.3 r1).
pub fn device_volume(v: i32) -> i32 {
    device_db(v as f32, 255.0)
}

/// Device pan (`0x00515890`, §8.3 r2): (left, right) attenuation in
/// hundredths of a dB, full = 127.
pub fn device_pan(pan: i32) -> (i32, i32) {
    match pan {
        p if p < 128 => (0, device_db(p as f32, 127.0)),
        p if p > 128 => (device_db((255 - p) as f32, 127.0), 0),
        _ => (0, 0),
    }
}

/// The device gain `G` (`[0x0072F9B0]`) at every volume send of a game
/// sound tick (§8.3 r4).
pub const DEVICE_GAIN: i32 = 255;

/// The device volume after `G` and occlusion (`0x005157B0`, §8.3 r3):
/// `v1 = trunc(v × G / 255)`, `v2 = trunc((1 − occ) × v1)` (mixer mode 0:
/// no voice is an EAX voice).
pub fn device_occluded(v: i32, occlusion: f32) -> i32 {
    let v1 = v.wrapping_mul(DEVICE_GAIN) / 255;
    ftol((1.0 - occlusion) * v1 as f32)
}

/// The §8.3 ratios as the mixer's Q8 gains (`client/audio.md` §A4):
/// amplitude `v2 / 255` after `G` and occlusion (r3); `pan < 128` scales
/// the right side by `pan / 127`, `pan > 128` the left by `(255 − pan) /
/// 127` (r2). A volume at or below 0 is silent (r1: `x ≤ 0.0001` gives
/// −10,000, §12 r2's negative sends). The Q8 rounding is ours (the mix is
/// not compared, `client/audio.md` §A4).
#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceGain;

impl GainCurve for DeviceGain {
    fn gains(&self, vol: i32, pan: i32) -> Result<Gains, AudioError> {
        self.gains_occluded(vol, pan, 0.0)
    }

    fn gains_occluded(&self, vol: i32, pan: i32, occlusion: f32) -> Result<Gains, AudioError> {
        if vol > 255 || !(0..=255).contains(&pan) {
            return Err(AudioError::GainDomain { vol, pan });
        }
        let vol = device_occluded(vol, occlusion).max(0);
        let q = |n: i32, d: i32| (n * GAIN_UNITY / d).min(GAIN_UNITY);
        let (l, r) = match pan {
            p if p < 128 => (GAIN_UNITY, q(p, 127)),
            p if p > 128 => (q(255 - p, 127), GAIN_UNITY),
            _ => (GAIN_UNITY, GAIN_UNITY),
        };
        Gains::new(q(vol, 255), l, r)
    }
}
