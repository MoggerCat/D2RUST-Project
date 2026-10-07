// Spec: specs/render/lighting.md (§5)
//! The light quality `q` and the measured draw rate (§5).
//!
//! Wall-clock time is always an input (milliseconds, the original's
//! `GetTickCount()` values): nothing here reads the OS clock. Verify runs
//! feed `q` from the recording instead of measuring it (§13).

/// The lighting option flags (§5): low-quality `[0x0072DA50]` and
/// missile-lights `[0x0072A348]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightOptions {
    /// `[0x0072DA50]`, static 0 (the settings `+0x08` of
    /// `render/shading.md` §4).
    pub low_quality: bool,
    /// `[0x0072A348]`, static 1. Missile lights exist only when set (§8).
    pub missile_lights: bool,
}

impl Default for LightOptions {
    /// The static values: low-quality 0, missile-lights 1.
    fn default() -> Self {
        LightOptions {
            low_quality: false,
            missile_lights: true,
        }
    }
}

/// The options-menu "lighting quality" item (`0x0047CFE0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityOption {
    Low,
    Medium,
    High,
}

impl LightOptions {
    /// The flags the menu item sets (§5): low → 1, 0; medium → 0, 0;
    /// high → 0, 1.
    pub fn from_menu(option: QualityOption) -> Self {
        let (low_quality, missile_lights) = match option {
            QualityOption::Low => (true, false),
            QualityOption::Medium => (false, false),
            QualityOption::High => (false, true),
        };
        LightOptions {
            low_quality,
            missile_lights,
        }
    }
}

/// The hysteresis of §5 r2: a new candidate is taken only when more than
/// this many ms passed since the last change.
pub const HYSTERESIS_MS: u32 = 2_000;

/// The light quality state (`0x00475780`): `q` `[0x007B567C]` and the last
/// change time `[0x007B5678]`, both 0 at process start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Quality {
    /// `q` ∈ {0, 1, 2}.
    pub q: u8,
    /// `GetTickCount()` of the last change.
    pub last_ms: u32,
}

impl Quality {
    /// The candidate of §5 r1 for draw rate `d`. `Err(q)` is the 13–15
    /// early return with the current `q` (> 0): no timer update follows.
    fn candidate(&self, options: LightOptions, d: i32) -> Result<u8, u8> {
        if options.low_quality && !options.missile_lights {
            return Ok(0);
        }
        let start = if options.low_quality && options.missile_lights {
            1
        } else {
            2
        };
        match d {
            i32::MIN..=9 => Ok(0),
            10..=12 => Ok(1),
            13..=15 => {
                if self.q > 0 {
                    Err(self.q)
                } else {
                    Ok(1)
                }
            }
            _ => Ok(start),
        }
    }

    /// One quality step (`0x00475780`, §5 r1–r2) at wall-clock `now_ms`
    /// with the measured draw rate `d`; returns the new `q`. The elapsed
    /// time is `now − last` in u32 arithmetic, as `GetTickCount()`
    /// differences are.
    pub fn update(&mut self, options: LightOptions, d: i32, now_ms: u32) -> u8 {
        let candidate = match self.candidate(options, d) {
            Ok(c) => c,
            Err(q) => return q,
        };
        if candidate != self.q && now_ms.wrapping_sub(self.last_ms) > HYSTERESIS_MS {
            self.q = candidate;
            self.last_ms = now_ms;
        }
        self.q
    }
}

/// The draw-rate window (§5): 3,000 ms.
pub const RATE_WINDOW_MS: u32 = 3_000;
/// `D` at game start (`0x0044F100`).
pub const START_RATE: i32 = 25;

/// The measured draw rate `D` (`[0x007A04A8]`) and its window (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawRateMeter {
    /// `D`.
    pub d: i32,
    /// Draws with a player room in the window (`[0x007A04AC]`).
    pub count: u32,
    /// The window start (loop wall-clock time, ms).
    pub window_start_ms: u32,
}

impl DrawRateMeter {
    /// `D` = 25 (`0x0044F100`), count 0, window starting at
    /// `window_start_ms`. The spec does not say what the window start is
    /// at game start: the caller supplies it.
    pub fn new(window_start_ms: u32) -> Self {
        DrawRateMeter {
            d: START_RATE,
            count: 0,
            window_start_ms,
        }
    }

    /// One in-game draw that ran with a player room (`0x0044F29C`).
    pub fn count_draw(&mut self) {
        self.count = self.count.wrapping_add(1);
    }

    /// One client loop pass (`0x0044CCE0`) at loop time `now_ms`: more
    /// than 3,000 ms past the window start → `D` := count / 3, window
    /// restarted, count zeroed. Returns `D`.
    pub fn pass(&mut self, now_ms: u32) -> i32 {
        if now_ms.wrapping_sub(self.window_start_ms) > RATE_WINDOW_MS {
            self.d = (self.count / 3) as i32;
            self.window_start_ms = now_ms;
            self.count = 0;
        }
        self.d
    }
}
