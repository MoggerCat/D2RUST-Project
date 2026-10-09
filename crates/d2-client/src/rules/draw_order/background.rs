// Spec: specs/render/draw-order-2.md
//! Level backgrounds (pass 1, §12) and the dead pass 8 (§13).
//!
//! Level 74 (Arcane Sanctuary) draws 256 one-pixel stars, level 120
//! (Arreat Summit) a strip of mountain cels and 10 clouds. Each keeps its
//! own seed (`sim/rng.md` §5.5): the seed is an **input** (the capture's
//! recorded value), never the clock. `GetTickCount` is an input per call.
//! The draw functions return the pass-1 items as plain data in build
//! order; [`keyed`] gives them their `DrawKey` (`draw-order.md` §10: pass
//! 1, major 0, minor = build order).

use d2_formats::palette::Palette;
use d2_sim::rng::Seed;

use super::OrderKey;
use crate::rules::shading::nearest;
use crate::scene::order::pass;

const SPEC: &str = "render/draw-order-2.md";

/// Level ids that draw a background (§12).
pub const ARCANE_SANCTUARY: u32 = 74;
pub const ARREAT_SUMMIT: u32 = 120;

/// Stars (§12 r1).
pub const STARS: usize = 256;
pub const STAR_COLORS: usize = 8;
/// Star move period: `GetTickCount() − last > 40` (§12 r3).
pub const STAR_TICK_MS: u32 = 40;

/// Summit mountain anchor `0x274F` (§12 l2 r1).
pub const SUMMIT_ANCHOR_X: i32 = 10_063;
/// Mountain cel width (§12 l2 r2).
pub const SUMMIT_TILE: i32 = 256;
pub const SUMMIT_FILE: &str = "data\\global\\ui\\summit01";
pub const CLOUD_FILE: &str = "data\\global\\ui\\cloud01";
pub const CLOUDS: usize = 10;
/// Cloud wrap: `x16 > 16·(W + 368)` → `x16 := −5,888` (§12 l2 r3).
pub const CLOUD_WRAP: i32 = 368;
pub const CLOUD_RESET_X16: i32 = -5_888;
/// Mountain light −1 and draw mode 5; cloud light and draw mode 3.
pub const SUMMIT_LIGHT: u32 = 0xFFFF_FFFF;
pub const SUMMIT_DRAW_MODE: u8 = 5;
pub const CLOUD_LIGHT: u32 = 0xDDDD_DDDD;
pub const CLOUD_DRAW_MODE: u8 = 3;

/// Whether pass 8 (`0x00475B20`) runs: never in 1.14d (§13).
pub const PASS8_RUNS: bool = false;

/// Errors of the background pass: an input the frame lacks or a question
/// the spec leaves open (never defaulted, M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackgroundError {
    /// The first call of a background needs its recorded seed (§12: a
    /// capture records `[0x00712C4C]` / `[0x00712C50]`).
    #[error("TODO(spec: {SPEC} §12): level {level} background first use without a recorded seed")]
    NoSeed { level: u32 },
    /// The star colors need the palette `nearest` searches (§12 r1).
    #[error("TODO(spec: {SPEC} §12 r1): star colors need the palette `nearest` reads")]
    NoPalette,
}

/// One pass-1 draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundDraw {
    /// A line (`blend-modes.md` §8); the stars draw (x, y) → (x, y).
    Line {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u8,
        alpha: u8,
    },
    /// A cel of `file` (loaded once) at `(x, y)` with light and draw mode.
    Cel {
        file: &'static str,
        frame: u32,
        x: i32,
        y: i32,
        light: u32,
        draw_mode: u8,
    },
}

/// A pass-1 draw with its key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackgroundItem {
    pub key: OrderKey,
    pub draw: BackgroundDraw,
}

/// Keys pass-1 draws in build order (`draw-order.md` §10).
pub fn keyed(draws: Vec<BackgroundDraw>) -> Vec<BackgroundItem> {
    draws
        .into_iter()
        .enumerate()
        .map(|(i, draw)| BackgroundItem {
            key: OrderKey {
                pass: pass::LEVEL_BACKGROUND,
                major: 0,
                minor: i as u32,
            },
            draw,
        })
        .collect()
}

/// The items of pass 8: none, the pass never runs (§13).
pub fn pass8_items() -> Vec<BackgroundItem> {
    Vec::new()
}

// ---------------------------------------------------------------- stars

/// One star (`0x00476190`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Star {
    pub x: i32,
    pub y: i32,
    /// −1 … −5.
    pub speed: i32,
    pub color: u8,
}

/// The Arcane Sanctuary stars (`0x00476290`, §12 r1–r3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stars {
    pub seed: Seed,
    pub colors: [u8; STAR_COLORS],
    pub stars: [Star; STARS],
    /// `GetTickCount()` of the last move.
    pub last: u32,
}

impl Stars {
    /// The first call's set-up (r1): `seed` is the recorded time-seeded
    /// seed, `last` the tick of the last move (an input: §12 does not name
    /// its initial value is 0, §12 l74 r3). `w`, `h` are `W`, `H` (`camera.md` §1).
    pub fn new(seed: Seed, last: u32, palette: &Palette, w: i32, h: i32) -> Self {
        let mut s = Stars {
            seed,
            colors: [0; STAR_COLORS],
            stars: [Star::default(); STARS],
            last,
        };
        for i in 0..STAR_COLORS as u32 {
            let base = 128 + (128 * i) / 7;
            let mut c = [0u32; 3];
            for v in &mut c {
                // base ≥ 128, so the sum never goes below 96.
                *v = (base + (s.seed.step() & 63) - 32).min(255);
            }
            s.colors[i as usize] = nearest(palette, c[0], c[1], c[2]);
        }
        for j in 0..STARS {
            s.make(j, false, w, h);
        }
        s
    }

    /// `0x00476190(j, remake)`: x (r1: `roll(W)`, r3: `W − 1 + (step &
    /// 7)`), then y, speed, color.
    fn make(&mut self, j: usize, remake: bool, w: i32, h: i32) {
        let x = if remake {
            w - 1 + (self.seed.step() & 7) as i32
        } else {
            self.seed.roll(w) as i32
        };
        let y = self.seed.roll(h - 40) as i32;
        let speed = -1 - (self.seed.step() % 5) as i32;
        let color = self.colors[(self.seed.step() & 7) as usize];
        self.stars[j] = Star { x, y, speed, color };
    }

    /// Every call (r2, r3): one pixel line per star, then the move when
    /// more than 40 ms passed since `last` (unsigned difference).
    pub fn draw(&mut self, now: u32, w: i32, h: i32) -> Vec<BackgroundDraw> {
        let out = self
            .stars
            .iter()
            .map(|s| BackgroundDraw::Line {
                x0: s.x,
                y0: s.y,
                x1: s.x,
                y1: s.y,
                color: s.color,
                alpha: 0xFF,
            })
            .collect();
        if now.wrapping_sub(self.last) > STAR_TICK_MS {
            for j in 0..STARS {
                self.stars[j].x += self.stars[j].speed;
                if self.stars[j].x < 0 {
                    self.make(j, true, w, h);
                }
            }
            self.last = now;
        }
        out
    }
}

// ---------------------------------------------------------------- summit

/// One cloud: x in sixteenths of a pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Cloud {
    pub x16: i32,
    pub y: i32,
    /// 8 … 23.
    pub speed: i32,
}

/// The mountain strip position (§12 l2 r1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SummitScroll {
    pub q: i32,
    pub c: i32,
    /// 0…3.
    pub t: i32,
    pub x0: i32,
}

/// `d = x − 10,063`, `q = d / 8`, `c = q rem 256`, `t = (q / 256) mod 4`
/// made non-negative, `x0 = −c` (C division and remainder).
pub fn summit_scroll(player_x: i32) -> SummitScroll {
    let d = player_x.wrapping_sub(SUMMIT_ANCHOR_X);
    let q = d / 8;
    let c = q % SUMMIT_TILE;
    let mut t = (q / SUMMIT_TILE) % 4;
    if t < 0 {
        t += 4;
    }
    SummitScroll { q, c, t, x0: -c }
}

/// The Arreat Summit background (`0x00476460`, §12 l2 r1–r3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summit {
    pub seed: Seed,
    pub clouds: [Cloud; CLOUDS],
}

impl Summit {
    /// The clouds' first use (r3): `seed` is the recorded time-seeded
    /// seed; per cloud `x16 := roll(16·W)`, `y := step mod 250`, `speed :=
    /// (step & 15) + 8`.
    pub fn new(seed: Seed, w: i32) -> Self {
        let mut s = Summit {
            seed,
            clouds: [Cloud::default(); CLOUDS],
        };
        for c in &mut s.clouds {
            let x16 = s.seed.roll(16 * w) as i32;
            let y = (s.seed.step() % 250) as i32;
            let speed = (s.seed.step() & 15) as i32 + 8;
            *c = Cloud { x16, y, speed };
        }
        s
    }

    /// Every call (r1–r3): mountains row by row (y 256, 512, and 768 in
    /// resolution mode 2), then each cloud moved and drawn. The caller
    /// skips the call while the exit flag is set ([`Backgrounds::pass1`]).
    pub fn draw(&mut self, player_x: i32, w: i32, resolution_mode: u32) -> Vec<BackgroundDraw> {
        let mut out = Vec::new();
        let sc = summit_scroll(player_x);
        let rows: &[(i32, u32)] = if resolution_mode == 2 {
            &[(256, 0), (512, 4), (768, 8)]
        } else {
            &[(256, 0), (512, 4)]
        };
        let cel = |frame: u32, x: i32, y: i32| BackgroundDraw::Cel {
            file: SUMMIT_FILE,
            frame,
            x,
            y,
            light: SUMMIT_LIGHT,
            draw_mode: SUMMIT_DRAW_MODE,
        };
        for &(y, add) in rows {
            for k in 0..4 {
                let frame = ((sc.t + k) & 3) as u32 + add;
                out.push(cel(frame, sc.x0 + SUMMIT_TILE * k, y));
            }
            if sc.x0 > 0 {
                let frame = ((sc.t + 3) & 3) as u32 + add;
                out.push(cel(frame, sc.x0 - SUMMIT_TILE, y));
            }
        }
        for i in 0..CLOUDS {
            let mut c = self.clouds[i];
            c.x16 += c.speed;
            if c.x16 > 16 * (w + CLOUD_WRAP) {
                c.x16 = CLOUD_RESET_X16;
                c.y = (self.seed.step() % 300) as i32;
            }
            self.clouds[i] = c;
            let x = c.x16 / 16;
            for (frame, dx) in [(0, 0), (1, SUMMIT_TILE)] {
                out.push(BackgroundDraw::Cel {
                    file: CLOUD_FILE,
                    frame,
                    x: x + dx,
                    y: c.y,
                    light: CLOUD_LIGHT,
                    draw_mode: CLOUD_DRAW_MODE,
                });
            }
        }
        out
    }
}

// ---------------------------------------------------------------- pass 1

/// What pass 1 reads of a frame.
#[derive(Debug, Clone, Copy)]
pub struct BackgroundFrame<'a> {
    /// The local player's level id.
    pub level: u32,
    /// `W`, `H` (`camera.md` §1).
    pub w: i32,
    pub h: i32,
    /// Resolution mode `[0x007C8CB8]`.
    pub resolution_mode: u32,
    /// `GetTickCount()` of this call.
    pub now: u32,
    /// The client's exit flag `[0x007A0620]`.
    pub exiting: bool,
    /// The local player's client x.
    pub player_x: i32,
    /// The recorded seeds for a first use (level 74, level 120).
    pub stars_seed: Option<Seed>,
    pub summit_seed: Option<Seed>,
    /// The star tick `last` before the first call: `None` = 0, the
    /// zero-filled global `[0x007B57E0]` (§12 l74 r3).
    pub stars_last: Option<u32>,
    /// The palette `nearest` searches for the star colors.
    pub palette: Option<&'a Palette>,
}

/// The two backgrounds' state, kept across frames (first-call flags
/// `[0x007B955C]` and the summit's own).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Backgrounds {
    pub stars: Option<Stars>,
    pub summit: Option<Summit>,
}

impl Backgrounds {
    /// Pass 1 of a frame (`draw-order.md` §1 row 1): the background of
    /// level 74 or 120, nothing for other levels.
    pub fn pass1(&mut self, f: &BackgroundFrame) -> Result<Vec<BackgroundItem>, BackgroundError> {
        let draws = match f.level {
            ARCANE_SANCTUARY => {
                if self.stars.is_none() {
                    let seed = f
                        .stars_seed
                        .ok_or(BackgroundError::NoSeed { level: f.level })?;
                    let last = f.stars_last.unwrap_or(0);
                    let palette = f.palette.ok_or(BackgroundError::NoPalette)?;
                    self.stars = Some(Stars::new(seed, last, palette, f.w, f.h));
                }
                let stars = self.stars.as_mut().expect("set above");
                stars.draw(f.now, f.w, f.h)
            }
            ARREAT_SUMMIT if !f.exiting => {
                if self.summit.is_none() {
                    let seed = f
                        .summit_seed
                        .ok_or(BackgroundError::NoSeed { level: f.level })?;
                    self.summit = Some(Summit::new(seed, f.w));
                }
                let summit = self.summit.as_mut().expect("set above");
                summit.draw(f.player_x, f.w, f.resolution_mode)
            }
            _ => Vec::new(),
        };
        Ok(keyed(draws))
    }
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
