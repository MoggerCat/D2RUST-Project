// Spec: specs/render/draw-order-2.md (§11, Randomness, Edge cases)
//! Weather: the three environment pools and the scalars of §11.1, the
//! once-per-client-update weather update (§11.2–§11.4), the water-floor
//! splash and bubble spawns of the floor pass (§11.5), the draw items of
//! pass 4 (§11.6) and pass 9 (§11.7), and the act-load and level-entry
//! presets (§11.8). Plain Rust, integer math, every draw on the local
//! player's seed in the original's order.
//!
//! Only the GDI path (perspective 0) is modelled, as the spec describes it.
//! The float parts of the particles are the spec's Open question 3: a
//! spawn records its shape values as pending (`None`), and moving or
//! drawing a falling particle returns [`WeatherError::Open`] instead of a
//! guessed value. The snow spawn's draw list, its color (`0x00472FB0`) and
//! the snow line table `0x006D6E78` are not in the spec yet and return
//! [`WeatherError::Unspecified`]. The particle color tables are built at
//! act load from ramps the spec does not list, so they are an input
//! ([`ColorTables`]).

use d2_sim::rng::Seed;

use crate::rules::camera::{FrameSize, OpenMode};

/// Particle pool slots (§11.1, `Environment Particles`, 256 × 0x28).
pub const PARTICLE_SLOTS: usize = 256;
/// Splash pool slots (§11.1, `Environment Splashes`, 512 × 0x18).
pub const SPLASH_SLOTS: usize = 512;
/// Bubble pool slots (§11.1, `Environment Bubbles`, 128 × 0x18).
pub const BUBBLE_SLOTS: usize = 128;

/// Rain cycle tables at act load (§11.3 r1): `Min[p]`, `N[p]`.
pub const CYCLE_MIN: [i32; 4] = [7_500, 250, 3_000, 125];
pub const CYCLE_N: [i32; 4] = [7_500, 250, 3_000, 50];
/// Snow wind goal roll `(Gmin, Gn)` from client start (§11.4 r3).
pub const SNOW_GOAL_START: (i32, i32) = (42, 170);
/// Sound of a thunder strike (§11.7 r2).
pub const THUNDER_SOUND: u16 = 202;
/// The play-area bottom margin of passes 4 and 9: `y < H − 47`.
pub const BOTTOM_MARGIN: i32 = 47;

/// A fatal or unresolved condition of the weather code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WeatherError {
    /// A fatal error of the original (`0x547`, `0x573`, `0x390`, `0x15B`).
    #[error("weather fatal 0x{0:X} (render/draw-order-2.md §11)")]
    Fatal(u16),
    /// A rule waiting on an open question of `render/draw-order-2.md`.
    #[error("render/draw-order-2.md open question {question}: {what}")]
    Open { question: u8, what: &'static str },
    /// A behavior the spec names but does not describe yet.
    #[error("render/draw-order-2.md §11 does not specify {what}")]
    Unspecified { what: &'static str },
    /// The act resources (§11.8) are not loaded.
    #[error("weather used before act load (render/draw-order-2.md §11.8)")]
    NotLoaded,
}

/// A fixed-size slot pool (§11.1, `0x006BCA10`): first free slot from the
/// lowest index (`0x006BCB40`), highest used index (+0x110), live count
/// (+0x114), clear (`0x006BCAC0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool<T> {
    slots: Vec<Option<T>>,
    highest: Option<usize>,
    live: usize,
}

impl<T: Copy> Pool<T> {
    pub fn new(capacity: usize) -> Self {
        Pool {
            slots: vec![None; capacity],
            highest: None,
            live: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Live count (+0x114).
    pub fn live(&self) -> usize {
        self.live
    }

    /// Highest index allocated since the last clear (+0x110).
    pub fn highest(&self) -> Option<usize> {
        self.highest
    }

    pub fn is_full(&self) -> bool {
        self.live == self.slots.len()
    }

    /// Takes the lowest free slot; `None` when full.
    pub fn alloc(&mut self, rec: T) -> Option<usize> {
        let i = self.slots.iter().position(Option::is_none)?;
        self.slots[i] = Some(rec);
        self.live += 1;
        self.highest = Some(self.highest.map_or(i, |h| h.max(i)));
        Some(i)
    }

    pub fn free(&mut self, i: usize) {
        if self.slots[i].take().is_some() {
            self.live -= 1;
        }
    }

    /// Frees every slot.
    pub fn clear(&mut self) {
        self.slots.iter_mut().for_each(|s| *s = None);
        self.highest = None;
        self.live = 0;
    }

    pub fn get(&self, i: usize) -> Option<&T> {
        self.slots.get(i).and_then(Option::as_ref)
    }

    /// Live records in slot order 0 … highest used.
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        let end = self.highest.map_or(0, |h| h + 1);
        self.slots[..end]
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|r| (i, r)))
    }

    fn slot_mut(&mut self, i: usize) -> Option<&mut T> {
        self.slots.get_mut(i).and_then(Option::as_mut)
    }

    fn end(&self) -> usize {
        self.highest.map_or(0, |h| h + 1)
    }
}

/// A splash or bubble record (§11.1, 0x18 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolRecord {
    /// Screen pixels (+0x04, +0x08).
    pub x: i32,
    pub y: i32,
    /// Cel kind `k` 0–3 (+0x0C): file `Rain{k+1}` / `bubble{k+1}`.
    pub kind: u8,
    /// Cel frame (+0x10).
    pub frame: u32,
    /// Countdown (+0x14).
    pub countdown: i32,
}

/// A rain drop or snow flake (§11.1, 0x28 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Particle {
    pub x: i32,
    pub y: i32,
    /// Ground y (+0x0C).
    pub ground_y: i32,
    /// Shape values +0x10 and +0x14: rain `4 − f₁`, `15 − f₂`; `None`
    /// while the float parts are Open question 3.
    pub shape_10: Option<i32>,
    pub shape_14: Option<i32>,
    /// Phase 0–511 (+0x18).
    pub phase: u32,
    /// Landed flag (+0x1C).
    pub landed: bool,
    /// Bounce count (+0x20).
    pub bounces: u32,
    /// Color index (+0x24) and alpha byte (+0x25).
    pub color: u8,
    pub alpha: u8,
}

/// Which pass-4 pool a record belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvPool {
    Splashes,
    Bubbles,
}

impl EnvPool {
    /// The cel file of kind `k` (`DATA\GLOBAL\UncompOverlays\`, §11.1).
    pub fn cel_file(self, kind: u8) -> String {
        let stem = match self {
            EnvPool::Splashes => "Rain",
            EnvPool::Bubbles => "bubble",
        };
        format!("DATA\\GLOBAL\\UncompOverlays\\{stem}{}", kind + 1)
    }

    /// `DrawKey` major of pass 4 (`draw-order.md` §10): 0 splashes, 1 bubbles.
    pub fn major(self) -> u32 {
        match self {
            EnvPool::Splashes => 0,
            EnvPool::Bubbles => 1,
        }
    }
}

/// The three 12-entry particle color tables (§11.4 r5), built at act load
/// from `nearest` (`shading.md` §5) of ramps the spec does not list yet:
/// supplied by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTables {
    /// `[0x007A8980]`: day period 0 (alpha 0x7F).
    pub period0: [u8; 12],
    /// `[0x007A894C]`: day periods 1 and 3.
    pub period13: [u8; 12],
    /// `[0x007A89A4]`: day period 2.
    pub period2: [u8; 12],
}

/// What act load reads from files (§11.8): the eight cel files' frame
/// counts (`0x006019F0`) and the color tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActResources {
    /// Frame counts of `Rain1`–`Rain4` (`[0x007A89C8]`).
    pub splash_frames: [u32; 4],
    /// Frame counts of `bubble1`–`bubble4` (`[0x007A892C]`).
    pub bubble_frames: [u32; 4],
    pub colors: ColorTables,
}

/// The local player's level as the weather reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelWeather {
    /// `levels.txt` row id.
    pub level_id: u32,
    /// Act index 0–4 (`0x006427F0`).
    pub act: u8,
    /// `Rain` (+0x05), `Mud` (+0x06).
    pub rain: bool,
    pub mud: bool,
}

/// The local player unit: its seed (`unit +0x20`) and level.
#[derive(Debug)]
pub struct LocalPlayer<'a> {
    pub seed: &'a mut Seed,
    pub level: LevelWeather,
}

/// Frame inputs of the weather update (§11.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateInput {
    /// Client update count (`0x0044DB00`).
    pub update_count: u32,
    /// `W`, `H` (`camera.md` §1).
    pub frame: FrameSize,
    /// Camera origin delta of the last drawn frame (`[0x007A0678]`,
    /// `[0x007A067C]`).
    pub camera_delta: (i32, i32),
    /// The act's day period 0–3 (`0x0061C100`).
    pub day_period: u8,
}

/// Frame inputs of pass 9 (§11.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pass9Input {
    pub frame: FrameSize,
    pub mode: OpenMode,
    /// Frames counted in the last 1,000 ms (`[0x007BB390]`).
    pub frame_rate: u32,
    /// Low-quality setting `[0x0072DA50]` ≠ 0.
    pub low_quality: bool,
    /// Whether the audio layer starts sound 202 (`0x004B9A00`); the two
    /// position rolls happen only then.
    pub thunder_sound_starts: bool,
}

/// A pass-4 draw (§11.6): cel `frame` of the kind's file at (`x`, `y`),
/// `D2GFX_DrawCelContext`, light −1, draw mode 3, palette 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolDraw {
    pub pool: EnvPool,
    /// Slot index: the `DrawKey` minor (`draw-order.md` §10).
    pub slot: usize,
    pub kind: u8,
    pub frame: u32,
    pub x: i32,
    pub y: i32,
}

impl PoolDraw {
    /// Light −1: unlit.
    pub const LIGHT: i32 = -1;
    /// Draw mode 3 (`blend-modes.md` §1).
    pub const DRAW_MODE: u8 = 3;
    pub const PALETTE: u8 = 0;
}

/// A pass-9 draw (§11.7), in build order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkyDraw {
    /// `D2GFX_DrawRectangle` (`blend-modes.md` §8 r2), color 255, mode 5.
    Flash { x0: i32, y0: i32, x1: i32, y1: i32 },
    /// `D2GFX_DrawLine` (`blend-modes.md` §8 r1).
    Line {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u8,
        alpha: u8,
    },
}

impl SkyDraw {
    pub const FLASH_COLOR: u8 = 255;
    pub const FLASH_DRAW_MODE: u8 = 5;
}

/// Sound 202 of a thunder strike (§11.7 r2): emitted, not played here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thunder {
    pub sound: u16,
    /// `roll_range(25, 50)`.
    pub volume: i32,
    /// The two `roll_range(−200, 400)` of `0x004B99A0`, when it started.
    pub position: Option<(i32, i32)>,
}

/// The output of pass 9.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pass9 {
    pub draws: Vec<SkyDraw>,
    pub thunder: Option<Thunder>,
}

/// The floor pass's per-frame context (`0x004DE730`, §11.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FloorContext {
    /// Update count of the last splash frame (+0x14) and bubble frame
    /// (+0x18); persist between frames.
    pub last_s: u32,
    pub last_b: u32,
    /// This frame's flags; cleared at the start of each frame.
    pub splash: bool,
    pub bubble: bool,
}

impl FloorContext {
    /// Start of a frame's floor pass: clears and sets the two flags.
    /// `mud` is the local player's level's `Mud`.
    pub fn begin_frame(&mut self, update_count: u32, weather: &Weather, mud: bool) {
        let c = update_count;
        self.splash = false;
        self.bubble = false;
        if c > self.last_s.wrapping_add(3) && weather.intensity_int() != 0 {
            self.splash = true;
            self.last_s = c;
        }
        if mud && c > self.last_b.wrapping_add(25) {
            self.bubble = true;
            self.last_b = c;
        }
    }
}

/// `L`, `R` of passes 4 and 9 (§11.6) for frame width `W` and open mode.
pub fn span(frame: FrameSize, mode: OpenMode) -> (i32, i32) {
    let w = frame.width;
    match mode.get() {
        1 => (0, w - 2 * (w / 4)),
        2 => (2 * (w / 4), w),
        _ => (0, w),
    }
}

/// The weather state (§11.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Weather {
    particles: Pool<Particle>,
    splashes: Pool<PoolRecord>,
    bubbles: Pool<PoolRecord>,
    /// Rain / mud flags of the previous update (`[0x007A8A44]`, `[0x007A8A40]`).
    pub rain_flag: bool,
    pub mud_flag: bool,
    /// Snow mode `[0x007A8A14]`.
    pub snow_mode: bool,
    /// Snow lock `[0x007A8A1C]`.
    pub snow_lock: bool,
    /// `[0x007A8A20]`: read by §11.3 with the lock; no writer is in the
    /// spec (question W1).
    pub snow_lock_hold: u32,
    /// Size bump `[0x007A8A18]`.
    pub size_bump: bool,
    /// Rain cycle phase `[0x007A8A24]`, length `[0x007A8A38]`, countdown
    /// `[0x007A8A3C]`.
    pub phase: u32,
    pub length: u32,
    pub countdown: u32,
    /// `Min[p]`, `N[p]` of §11.3 r1 (set at act load).
    pub cycle_min: [i32; 4],
    pub cycle_n: [i32; 4],
    /// Peak `[0x007A89C0]` and target `[0x007A89E0]` particle counts.
    pub peak: u32,
    pub target: u32,
    /// Intensity `[0x007A89A0]` = float(`intensity_256`) × 1/256.
    pub intensity_256: u32,
    /// Wind `[0x007A89C4]`, goal `[0x007A89F8]`, retarget countdown
    /// `[0x007A89E4]`.
    pub wind: i32,
    pub wind_goal: i32,
    pub wind_countdown: i32,
    /// Snow goal roll (`[0x007A89D8]`, `[0x007A8998]`).
    pub snow_goal_min: i32,
    pub snow_goal_n: i32,
    /// Lightning on `[0x007A8A08]`, countdown `[0x007A8968]`, phase
    /// `[0x007A896C]`, frame trigger `[0x007A89E8]`, thunder flag
    /// `[0x00712B4C]`.
    pub lightning_on: bool,
    pub lightning_countdown: i32,
    pub lightning_phase: u32,
    pub lightning_trigger: i32,
    pub thunder: bool,
    /// Weather update mark `[0x007A8A0C]`.
    pub mark: u32,
    resources: Option<ActResources>,
}

impl Default for Weather {
    fn default() -> Self {
        Self::new()
    }
}

impl Weather {
    /// The state at client start (`0x00472320`): empty pools, the thunder
    /// flag set (`.data` 1), snow goal (42, 170); every other scalar 0.
    pub fn new() -> Self {
        Weather {
            particles: Pool::new(PARTICLE_SLOTS),
            splashes: Pool::new(SPLASH_SLOTS),
            bubbles: Pool::new(BUBBLE_SLOTS),
            rain_flag: false,
            mud_flag: false,
            snow_mode: false,
            snow_lock: false,
            snow_lock_hold: 0,
            size_bump: false,
            phase: 0,
            length: 0,
            countdown: 0,
            cycle_min: [0; 4],
            cycle_n: [0; 4],
            peak: 0,
            target: 0,
            intensity_256: 0,
            wind: 0,
            wind_goal: 0,
            wind_countdown: 0,
            snow_goal_min: SNOW_GOAL_START.0,
            snow_goal_n: SNOW_GOAL_START.1,
            lightning_on: false,
            lightning_countdown: 0,
            lightning_phase: 0,
            lightning_trigger: 0,
            thunder: true,
            mark: 0,
            resources: None,
        }
    }

    pub fn particles(&self) -> &Pool<Particle> {
        &self.particles
    }

    pub fn splashes(&self) -> &Pool<PoolRecord> {
        &self.splashes
    }

    pub fn bubbles(&self) -> &Pool<PoolRecord> {
        &self.bubbles
    }

    /// Mutable pools, for restoring a captured state.
    pub fn pools_mut(
        &mut self,
    ) -> (
        &mut Pool<Particle>,
        &mut Pool<PoolRecord>,
        &mut Pool<PoolRecord>,
    ) {
        (&mut self.particles, &mut self.splashes, &mut self.bubbles)
    }

    pub fn resources(&self) -> Option<&ActResources> {
        self.resources.as_ref()
    }

    /// int(intensity) (`0x00682FD0`, truncation): `intensity_256` / 256,
    /// exact for every target the rules produce (≤ 256).
    pub fn intensity_int(&self) -> u32 {
        self.intensity_256 / 256
    }

    // ---------------------------------------------------------------- §11.8

    /// Act load (`0x00454811` → `0x00472890`): `0x004726F0(1)`, the cycle
    /// tables (`Min[2]` × 3 in acts III and V), the color tables and cel
    /// frame counts, intensity := 0.
    pub fn act_load(
        &mut self,
        player: LocalPlayer<'_>,
        resources: ActResources,
    ) -> Result<(), WeatherError> {
        let level = player.level;
        self.level_entry(1, player.seed, &level)?;
        self.cycle_min = CYCLE_MIN;
        self.cycle_n = CYCLE_N;
        if level.act == 2 || level.act == 4 {
            self.cycle_min[2] *= 3;
        }
        self.resources = Some(resources);
        self.intensity_256 = 0;
        Ok(())
    }

    /// `0x004726F0(arg)`: the snow and lightning re-check of a level entry
    /// (`arg` ≠ 0 at act load) and of rain phase 1 (`arg` = 0).
    pub fn level_entry(
        &mut self,
        arg: u32,
        seed: &mut Seed,
        level: &LevelWeather,
    ) -> Result<(), WeatherError> {
        if level.act != 4 {
            self.snow_off()?;
            self.reset_wind_lightning(seed);
            return Ok(());
        }
        self.reset_wind_lightning(seed);
        let mut t = 0;
        let mut lock = false;
        match level.level_id {
            109..=112 => {
                t = 25;
                self.size_bump = false;
            }
            117 => self.size_bump = true,
            120 | 121 => {
                self.size_bump = true;
                lock = true;
            }
            _ => {}
        }
        if seed.step() % 100 < t && arg == 0 {
            return self.snow_off();
        }
        self.snow_lock = lock;
        if lock {
            self.phase_entry(2, seed, level)?;
        }
        // 0x00472590
        self.lightning_on = false;
        if !self.snow_mode || arg != 0 {
            self.clear_particles()?;
        }
        self.snow_mode = true;
        Ok(())
    }

    /// Snow off (§11.8 r1 without `0x00472610`); a level change also
    /// clears lightning (§11.7).
    fn snow_off(&mut self) -> Result<(), WeatherError> {
        self.snow_lock = false;
        if self.snow_mode {
            self.clear_particles()?;
        }
        self.snow_mode = false;
        self.lightning_on = false;
        Ok(())
    }

    fn clear_particles(&mut self) -> Result<(), WeatherError> {
        self.particles.clear();
        if self.particles.live() != 0 {
            return Err(WeatherError::Fatal(0x15B));
        }
        Ok(())
    }

    /// `0x00472610`: three raw steps.
    fn reset_wind_lightning(&mut self, seed: &mut Seed) {
        self.wind_countdown = 125 + (seed.step() % 375) as i32;
        self.lightning_countdown = 500 + (seed.step() % 1_500) as i32;
        let w = 92 + (seed.step() % 71) as i32;
        self.wind = w;
        self.wind_goal = w;
    }

    /// `0x00472400`: the per-level presets of rain phase 2.
    fn level_presets(&mut self, seed: &mut Seed, level: &LevelWeather) {
        match level.level_id {
            109..=112 => {
                (self.snow_goal_min, self.snow_goal_n) = (42, 170);
                self.peak = seed.roll_range(32, 56) as u32;
            }
            117 => {
                (self.snow_goal_min, self.snow_goal_n) = (170, 56);
                self.peak = seed.roll_range(40, 112) as u32;
            }
            120 | 121 => {
                (self.snow_goal_min, self.snow_goal_n) = (28, 28);
                self.wind = 28;
                self.wind_goal = 28;
                self.peak = 256;
            }
            _ => {
                self.lightning_on = false;
                if !self.snow_mode {
                    seed.step();
                }
            }
        }
        self.target = self.peak;
    }

    /// `0x00473D00(p)`: the rain phase entries.
    fn phase_entry(
        &mut self,
        p: u32,
        seed: &mut Seed,
        level: &LevelWeather,
    ) -> Result<(), WeatherError> {
        match p {
            0 => self.target = 0,
            1 => {
                self.level_entry(0, seed, level)?;
                self.peak = seed.roll_range(32, 224) as u32;
            }
            2 => self.level_presets(seed, level),
            _ => self.lightning_on = false,
        }
        Ok(())
    }

    // ---------------------------------------------------------------- §11.2

    /// The weather update (`0x00473F50`), once per frame before
    /// `StartDraw`; advances at most once per client update.
    pub fn update(
        &mut self,
        player: Option<LocalPlayer<'_>>,
        input: &UpdateInput,
    ) -> Result<(), WeatherError> {
        let (r, m) = player
            .as_ref()
            .map_or((false, false), |p| (p.level.rain, p.level.mud));
        if !r && self.rain_flag {
            if self.particles.live() != 0 {
                self.particles.clear();
            }
            if self.splashes.live() != 0 {
                self.splashes.clear();
            }
        }
        if !m && self.mud_flag && self.bubbles.live() != 0 {
            self.bubbles.clear();
        }
        self.rain_flag = r;
        self.mud_flag = m;

        let c = input.update_count;
        let Some(player) = player else {
            return Err(WeatherError::Fatal(0x547));
        };
        if c <= self.mark {
            return Ok(());
        }
        self.mark = c;
        let (seed, level) = (player.seed, player.level);
        if !r {
            self.target = 0;
        } else {
            if self.particles.live() != 0 {
                return Err(WeatherError::Open {
                    question: 3,
                    what: "particle move 0x004732C0",
                });
            }
            let frames = self.resources.ok_or(WeatherError::NotLoaded)?.splash_frames;
            update_pool(&mut self.splashes, &frames, input.camera_delta);
            self.rain_cycle(seed, &level)?;
            self.top_up(seed, input)?;
        }
        if m {
            let frames = self.resources.ok_or(WeatherError::NotLoaded)?.bubble_frames;
            update_pool(&mut self.bubbles, &frames, input.camera_delta);
        }
        Ok(())
    }

    /// The rain cycle (§11.3, `0x00473E50`).
    fn rain_cycle(&mut self, seed: &mut Seed, level: &LevelWeather) -> Result<(), WeatherError> {
        if self.snow_lock && self.phase != 2 {
            if self.snow_lock_hold == 0 {
                self.phase = 2;
                self.level_presets(seed, level);
            }
            return Ok(());
        }
        if self.countdown == 0 {
            let p = (self.phase + 1) % 4;
            self.phase = p;
            let d = seed.roll_range(self.cycle_min[p as usize], self.cycle_n[p as usize]);
            self.length = d as u32;
            self.countdown = d as u32;
            self.phase_entry(p, seed, level)?;
        }
        self.countdown = self.countdown.wrapping_sub(1);
        let d = self.length;
        match self.phase {
            1 => self.target = self.peak.wrapping_mul(d.wrapping_sub(self.countdown)) / d,
            3 => self.target = self.peak.wrapping_mul(self.countdown) / d,
            _ => {}
        }
        self.intensity_256 = self.target;
        Ok(())
    }

    /// Top-up, wind and lightning timer (§11.4, `0x004737B0`).
    fn top_up(&mut self, seed: &mut Seed, input: &UpdateInput) -> Result<(), WeatherError> {
        while (self.particles.live() as u64) < u64::from(self.target) {
            if !self.spawn_particle(seed, input)? {
                break;
            }
        }
        let s = if self.snow_mode { 1 } else { 2 };
        if self.wind < self.wind_goal {
            self.wind = (self.wind + s).min(self.wind_goal);
        } else if self.wind > self.wind_goal {
            self.wind = (self.wind - s).max(self.wind_goal);
        }
        self.wind_countdown = self.wind_countdown.wrapping_sub(1);
        if self.wind_countdown == 0 {
            if self.snow_mode {
                self.wind_countdown = 250 + (seed.step() % 875) as i32;
                seed.roll_range(self.snow_goal_min, self.snow_goal_n);
                return Err(WeatherError::Open {
                    question: 3,
                    what: "snow wind goal (§11.4 r3)",
                });
            }
            self.wind_countdown = 125 + (seed.step() % 375) as i32;
            self.wind_goal = 92 + (seed.step() % 71) as i32;
        }
        if self.lightning_on {
            self.lightning_countdown = self.lightning_countdown.wrapping_sub(1);
            if self.lightning_countdown == 0 {
                self.lightning_trigger = 1;
            }
        }
        Ok(())
    }

    /// One particle spawn (`0x00473090`, §11.4 r5); `false` when the pool
    /// is full.
    fn spawn_particle(
        &mut self,
        seed: &mut Seed,
        input: &UpdateInput,
    ) -> Result<bool, WeatherError> {
        if self.snow_mode {
            return Err(WeatherError::Unspecified {
                what: "the snow spawn's draw list and color 0x00472FB0 (§11.4 r5)",
            });
        }
        let colors = self.resources.ok_or(WeatherError::NotLoaded)?.colors;
        if self.particles.is_full() {
            return Ok(false);
        }
        let (w, h) = (input.frame.width, input.frame.height);
        let x = seed.roll_range(0, w);
        let g = seed.roll_range(40, h - 87);
        let y = seed.roll_range(-20, g + 20);
        let phase = seed.step() & 511;
        let i = (seed.step() % 12) as usize;
        let (color, alpha) = match input.day_period {
            0 => (colors.period0[i], 0x7F),
            2 => (colors.period2[i], 0xFF),
            _ => (colors.period13[i], 0xFF),
        };
        self.particles.alloc(Particle {
            x,
            y,
            ground_y: g,
            shape_10: None,
            shape_14: None,
            phase,
            landed: false,
            bounces: 3,
            color,
            alpha,
        });
        Ok(true)
    }

    // ---------------------------------------------------------------- §11.5

    /// After every drawn floor whose DT1 material has bit 0x2 (water), at
    /// its screen position (`camera.md` §6): one `roll_range(0, 1000)`,
    /// then the gated splash and bubble spawns.
    pub fn water_floor(&mut self, ctx: &FloorContext, x: i32, y: i32, seed: &mut Seed) {
        let r = seed.roll_range(0, 1_000);
        if ctx.splash && (r as i64) < i64::from(self.intensity_int()) {
            self.spawn_splash(x, y, seed);
        }
        if ctx.bubble && r < 100 {
            self.spawn_bubble(x, y, seed);
        }
    }

    /// `0x00472DA0`; nothing when the pool is full.
    pub fn spawn_splash(&mut self, x: i32, y: i32, seed: &mut Seed) -> Option<usize> {
        if self.splashes.is_full() {
            return None;
        }
        let (dx, a) = spawn_offset(seed);
        let kind = 2 + (seed.step() & 1) as u8;
        self.splashes.alloc(PoolRecord {
            x: x + dx,
            y: y + a,
            kind,
            frame: 0,
            countdown: 2,
        })
    }

    /// `0x00472EC0`; nothing when the pool is full.
    pub fn spawn_bubble(&mut self, x: i32, y: i32, seed: &mut Seed) -> Option<usize> {
        if self.bubbles.is_full() {
            return None;
        }
        let (dx, a) = spawn_offset(seed);
        self.bubbles.alloc(PoolRecord {
            x: x + dx,
            y: y + a - 30,
            kind: 2,
            frame: 0,
            countdown: 2,
        })
    }

    // ---------------------------------------------------------------- §11.6

    /// Pass 4 (`0x00473C00` → `0x00473A70`): splashes, then bubbles, each
    /// in slot order; `shift_x` is `camera.md` §1's.
    pub fn pass4(&self, frame: FrameSize, mode: OpenMode, shift_x: i32) -> Vec<PoolDraw> {
        let (l, r) = span(frame, mode);
        let bottom = frame.height - BOTTOM_MARGIN;
        let mut out = Vec::new();
        for (pool, recs) in [
            (EnvPool::Splashes, &self.splashes),
            (EnvPool::Bubbles, &self.bubbles),
        ] {
            if recs.live() == 0 {
                continue;
            }
            for (slot, rec) in recs.iter() {
                let x = rec.x + shift_x;
                if x >= l && x < r && rec.y >= 0 && rec.y < bottom {
                    out.push(PoolDraw {
                        pool,
                        slot,
                        kind: rec.kind,
                        frame: rec.frame,
                        x,
                        y: rec.y,
                    });
                }
            }
        }
        out
    }

    // ---------------------------------------------------------------- §11.7

    /// `0x00472C50(thunder)`: lightning on, thunder flag := `thunder`,
    /// frame trigger 1. Its caller is Open question 4.
    pub fn start_lightning(&mut self, thunder: bool) {
        self.lightning_on = true;
        self.thunder = thunder;
        self.lightning_trigger = 1;
    }

    /// Pass 9 (`0x00473910`): the lightning flash, else the particles.
    pub fn pass9(
        &mut self,
        player: Option<LocalPlayer<'_>>,
        input: &Pass9Input,
    ) -> Result<Pass9, WeatherError> {
        let player = player.ok_or(WeatherError::Fatal(0x573))?;
        let seed = player.seed;
        let (l, r) = span(input.frame, input.mode);
        let bottom = input.frame.height - BOTTOM_MARGIN;
        let mut out = Pass9::default();
        if self.lightning_on && self.lightning_trigger != 0 {
            self.lightning_trigger -= 1;
            if self.lightning_trigger == 0 {
                if self.lightning_phase == 0 {
                    self.lightning_countdown = 3;
                    self.lightning_phase = 1;
                } else {
                    self.lightning_countdown = seed.roll_range(500, 1_500);
                    self.lightning_phase = 0;
                    if self.thunder {
                        let volume = seed.roll_range(25, 50);
                        let position = input.thunder_sound_starts.then(|| {
                            let px = seed.roll_range(-200, 400);
                            let py = seed.roll_range(-200, 400);
                            (px, py)
                        });
                        out.thunder = Some(Thunder {
                            sound: THUNDER_SOUND,
                            volume,
                            position,
                        });
                    } else {
                        self.thunder = true;
                    }
                }
            }
            if input.frame_rate > 9 {
                out.draws.push(SkyDraw::Flash {
                    x0: l,
                    y0: 0,
                    x1: r,
                    y1: bottom,
                });
                return Ok(out);
            }
        }
        if !player.level.rain || self.particles.live() == 0 || input.low_quality {
            return Ok(out);
        }
        let in_range = |x: i32, y: i32| x >= l && x < r && y >= 0 && y < bottom;
        for (_, p) in self.particles.iter() {
            if self.snow_mode {
                if p.shape_10.is_some_and(|s| (0..=7).contains(&s)) && in_range(p.x, p.y) {
                    return Err(WeatherError::Unspecified {
                        what: "the snow line table 0x006D6E78 (§11.7 r3)",
                    });
                }
            } else if p.landed {
                if in_range(p.x, p.y) {
                    out.draws.push(SkyDraw::Line {
                        x0: p.x,
                        y0: p.y,
                        x1: p.x,
                        y1: p.y,
                        color: p.color,
                        alpha: p.alpha,
                    });
                }
            } else {
                return Err(WeatherError::Open {
                    question: 3,
                    what: "falling drop vector (§11.7 r3)",
                });
            }
        }
        Ok(out)
    }
}

/// The spawn offset of `0x00472DA0` / `0x00472EC0`: `a` := `roll_range(0,
/// 80)`, `b` := 80 − 2·|a − 40|, dx := `roll_range(−b, 2b)` (no draw when
/// `b` = 0). Returns (dx, a).
fn spawn_offset(seed: &mut Seed) -> (i32, i32) {
    let a = seed.roll_range(0, 80);
    let b = 80 - 2 * (a - 40).abs();
    (seed.roll_range(-b, 2 * b), a)
}

/// Splash / bubble update (`0x00472C80`, `0x00472D10`, §11.5).
fn update_pool(pool: &mut Pool<PoolRecord>, frames: &[u32; 4], delta: (i32, i32)) {
    for i in 0..pool.end() {
        let Some(rec) = pool.slot_mut(i) else {
            continue;
        };
        rec.x -= delta.0;
        rec.y -= delta.1;
        rec.countdown -= 1;
        if rec.countdown < 1 {
            rec.frame += 1;
            rec.countdown = 2;
            if rec.frame >= frames[usize::from(rec.kind & 3)] {
                pool.free(i);
            }
        }
    }
}

#[cfg(test)]
#[path = "weather_tests.rs"]
mod tests;
