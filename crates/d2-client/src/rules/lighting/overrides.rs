// Spec: specs/render/lighting.md (§10, edge case 9)
//! The scripted ambient overrides (`0x0046BDD0`, §10): the Den of Evil
//! glow, the levels 107/108 glow, the darkness event, and the S→C 0x89
//! UniqueEvent dispatcher that triggers them.
//!
//! The sine table and the override intensities use doubles as the original
//! does; client-only state (§13).

use std::sync::OnceLock;

use d2_sim::rng::Seed;
use thiserror::Error;

use super::environment::{Ambient, PI_F};

/// Den of Evil (§10 r1).
pub const LEVEL_DEN_OF_EVIL: u32 = 8;
/// The red glow levels (§10 r2).
pub const LEVELS_RED_GLOW: [u32; 2] = [107, 108];

/// The sine table `W` (`0x00707800`, 512 floats): `W[i]` =
/// float(`sin(i · π_f / 256)`) (§10 r1).
pub fn sine_table() -> &'static [f32; 512] {
    static W: OnceLock<[f32; 512]> = OnceLock::new();
    W.get_or_init(|| std::array::from_fn(|i| (i as f64 * PI_F / 256.0).sin() as f32))
}

/// `trunc(W[(a + 128) & 511] · base)` (§10 r1, r3).
fn wave(a: i32, base: f64) -> i32 {
    let w = sine_table()[((a + 128) & 511) as usize];
    (f64::from(w) * base) as i32
}

/// The red glow color of §10 r1 and r2.
const RED_GLOW: (u8, u8, u8) = (255, 64, 48);

/// A running darkness event (`0x0046AE50`, §10 r3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DarknessEvent {
    pub fade_in: i32,
    pub hold: i32,
    /// `out`, 0 replaced by 25.
    pub fade_out: i32,
    /// The event's level id.
    pub level: u32,
    /// Counter `c`.
    pub c: i32,
    /// Angle `a`.
    pub a: i32,
    /// `[0x007A7430..36]`: the last computed `I`, R, G, B.
    pub out: Ambient,
}

impl DarknessEvent {
    /// `0x0046AE50(in, hold, out, level)`: `out` = 0 → 25; `c`, `a` from 0.
    pub fn new(fade_in: i32, hold: i32, fade_out: i32, level: u32) -> Self {
        DarknessEvent {
            fade_in,
            hold,
            fade_out: if fade_out == 0 { 25 } else { fade_out },
            level,
            c: 0,
            a: 0,
            out: Ambient::ZERO,
        }
    }

    fn total(&self) -> i32 {
        self.fade_in + self.hold + self.fade_out
    }
}

/// What the S→C 0x89 dispatcher did beyond its state changes (§10 r4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UniqueEventEffect {
    /// Id 0: the Den counter was set to 0.
    DenCounterStarted,
    /// Id 12: the levels 107/108 flag was set. `in_level_108`: the
    /// original also creates client missile 372 at the local player and
    /// calls `0x0046F870(243, 1)`; the caller owns those.
    RedGlowStarted { in_level_108: bool },
    /// Id 13: the levels 107/108 counter was set to 0 and `[0x007A7464]`.
    RedGlowCounterStarted,
    /// Ids 1, 3, 6, 14, 16, 17, 19: a client effect of its own
    /// (`0x0046AEE0`, `0x0046B100`, `0x0046B1E0`, `0x0046B300`,
    /// `0x0046B440`, `0x0046B4A0`, `0x0046B520`). Recorded only: no
    /// behavior is implemented here (id 1, the cairn stones, starts a
    /// darkness event from missile 288's fields, which this module does
    /// not read).
    OtherClientEffect(u8),
    /// The other ids below 20: the bit only.
    None,
}

/// A fatal S→C 0x89 (§10 r4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum OverrideError {
    #[error("S→C 0x89: id {0} ≥ 32 (fatal 0x1D9)")]
    Fatal1D9(u8),
    #[error("S→C 0x89: id {0} ≥ 20 (fatal 0x1DA)")]
    Fatal1DA(u8),
}

/// The client globals of the overrides (§10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Overrides {
    /// `[0x007A745C]`: Den lights placed.
    pub den_flag: bool,
    /// `[0x007129CC]`: the Den counter, −1 when idle.
    pub den_counter: i32,
    /// `[0x007A7460]`: the levels 107/108 flag.
    pub glow_flag: bool,
    /// `[0x007129D0]`: the levels 107/108 counter, −1 when idle.
    pub glow_counter: i32,
    /// `[0x007A7464]`: set by id 13.
    pub glow_value: i32,
    /// `[0x007A7458]`: the received UniqueEvent ids.
    pub unique_bits: u32,
    /// The darkness event, `None` when none runs.
    pub darkness: Option<DarknessEvent>,
}

impl Default for Overrides {
    fn default() -> Self {
        Overrides {
            den_flag: false,
            den_counter: -1,
            glow_flag: false,
            glow_counter: -1,
            glow_value: 0,
            unique_bits: 0,
            darkness: None,
        }
    }
}

impl Overrides {
    /// Game start (`0x0046BF90`): flags 0, counters −1. The spec names no
    /// other reset; the rest is kept.
    pub fn game_start(&mut self) {
        self.den_flag = false;
        self.den_counter = -1;
        self.glow_flag = false;
        self.glow_counter = -1;
    }

    /// The override of a room of level `level` (`0x0046BDD0`, §10 r1–r3).
    /// `quest_byte1`: client quest byte 1 (`[0x007C0EA5]`). A result with
    /// R, G, B all 0 falls through (§3.1).
    pub fn ambient(&self, level: u32, quest_byte1: u8) -> Ambient {
        let (r, g, b) = RED_GLOW;
        if level == LEVEL_DEN_OF_EVIL {
            if !self.den_flag && quest_byte1 != 0 {
                let i = if self.den_counter == -1 {
                    80
                } else {
                    let a = self.den_counter * 128 / 30;
                    wave(a, 80.0)
                };
                return Ambient {
                    i: i as u8,
                    r,
                    g,
                    b,
                };
            }
            return Ambient::ZERO;
        }
        if LEVELS_RED_GLOW.contains(&level) {
            if self.glow_flag && self.glow_counter == -1 {
                return Ambient { i: 160, r, g, b };
            }
            return Ambient::ZERO;
        }
        self.darkness.map_or(Ambient::ZERO, |e| e.out)
    }

    /// The counters per client update (`0x0046BEB0`, §10 r1–r2). Returns
    /// `true` when the Den counter passed 29 on this update: the flag is
    /// set and every loaded level-8 room gets Den lights (`0x0046B0D0`,
    /// [`den_light_points`]).
    ///
    /// The levels 107/108 counter rises while ≥ 0 and at > 29 resets flag
    /// and counter (read as their game-start values, 0 and −1).
    pub fn update_counters(&mut self) -> bool {
        let mut den_lights = false;
        if self.den_counter >= 0 && !self.den_flag {
            self.den_counter += 1;
            if self.den_counter > 29 {
                self.den_flag = true;
                den_lights = true;
            }
        }
        if self.glow_counter >= 0 {
            self.glow_counter += 1;
            if self.glow_counter > 29 {
                self.glow_flag = false;
                self.glow_counter = -1;
            }
        }
        den_lights
    }

    /// Whether a level-8 room loaded now gets Den lights (`0x0046BE60`):
    /// once the flag is set.
    pub fn room_load_gets_den_lights(&self, level: u32) -> bool {
        level == LEVEL_DEN_OF_EVIL && self.den_flag
    }

    /// Starts a darkness event (`0x0046AE50`, §10 r3).
    pub fn start_darkness(&mut self, fade_in: i32, hold: i32, fade_out: i32, level: u32) {
        self.darkness = Some(DarknessEvent::new(fade_in, hold, fade_out, level));
    }

    /// The darkness step per client update (`0x0046AD10`, §10 r3).
    /// `base`: the player room's ambient without override (§3.1 r2–r3);
    /// `level`: the player room's level id.
    pub fn update_darkness(&mut self, base: Ambient, level: u32) {
        let Some(e) = self.darkness.as_mut() else {
            return;
        };
        let i0 = f64::from(base.i);
        let total = e.total();
        let i = if e.c < e.fade_in {
            let i = wave(e.a, i0);
            e.a = e.c * 128 / e.fade_in;
            i
        } else if e.c < total - e.fade_out {
            0
        } else {
            let i = wave(e.a, i0);
            e.a = 128 - (e.c - total + e.fade_out) * 128 / e.fade_out;
            i
        };
        e.out = Ambient {
            i: i as u8,
            r: base.r,
            g: base.g,
            b: base.b,
        };
        e.c += 1;
        if e.c > total || level != e.level {
            self.darkness = None;
        }
    }

    /// S→C 0x89 UniqueEvent (`0x0045EA30` → `0x0046B630`, §10 r4).
    /// `level`: the local player's level id (id 12); `draw_0x00410a80`:
    /// the value `0x00410A80` returns, called only for id 13 (the spec
    /// does not describe that function).
    pub fn unique_event(
        &mut self,
        id: u8,
        level: u32,
        draw_0x00410a80: impl FnOnce() -> i32,
    ) -> Result<UniqueEventEffect, OverrideError> {
        if id >= 32 {
            return Err(OverrideError::Fatal1D9(id));
        }
        if id >= 20 {
            return Err(OverrideError::Fatal1DA(id));
        }
        self.unique_bits |= 1 << id;
        Ok(match id {
            0 => {
                self.den_counter = 0;
                UniqueEventEffect::DenCounterStarted
            }
            12 => {
                self.glow_flag = true;
                UniqueEventEffect::RedGlowStarted {
                    in_level_108: level == 108,
                }
            }
            13 => {
                self.glow_counter = 0;
                self.glow_value = draw_0x00410a80().wrapping_add(90);
                UniqueEventEffect::RedGlowCounterStarted
            }
            1 | 3 | 6 | 14 | 16 | 17 | 19 => UniqueEventEffect::OtherClientEffect(id),
            _ => UniqueEventEffect::None,
        })
    }
}

/// Den lights placement tries per room (§10 r1).
pub const DEN_LIGHT_TRIES: u32 = 25;
/// Den lights placed per room at most (§10 r1).
pub const DEN_LIGHTS_PER_ROOM: usize = 3;
/// The client missile a Den light is (`denofevillight`, §8).
pub const DEN_LIGHT_MISSILE: u32 = 287;

/// The Den lights of one room (`0x0046AF70`, §10 r1, edge case 9): up to
/// 25 tries until 3 are placed; each try draws x = room x + rnd(w), then
/// y = room y + rnd(h) from the local player unit's seed (`roll`,
/// `sim/rng.md` §3); the point is placed when `point_test(x, y)` (the
/// collision point test with mask 5) is 0. Returns the placed points in
/// order; each becomes client missile 287.
pub fn den_light_points(
    room: (i32, i32, i32, i32),
    seed: &mut Seed,
    mut point_test: impl FnMut(i32, i32) -> u32,
) -> Vec<(i32, i32)> {
    let (rx, ry, w, h) = room;
    let mut placed = Vec::with_capacity(DEN_LIGHTS_PER_ROOM);
    for _ in 0..DEN_LIGHT_TRIES {
        if placed.len() >= DEN_LIGHTS_PER_ROOM {
            break;
        }
        let x = rx.wrapping_add(seed.roll(w) as i32);
        let y = ry.wrapping_add(seed.roll(h) as i32);
        if point_test(x, y) == 0 {
            placed.push((x, y));
        }
    }
    placed
}
