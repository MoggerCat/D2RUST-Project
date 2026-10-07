// Spec: specs/audio/environment.md
// Spec: specs/audio/triggers.md (§1 conventions: request, volume set, group stops, time bases, draw helpers)
// Spec: specs/audio/sound-table.md (§1 r5 song range, §2 soundenviron layout, §6 sound tick, §9 settings)
// Spec: specs/client/audio.md (§B4, §B5)
//! Sound environment state machines (`audio/environment.md`): music and
//! quest stingers (§2–§3), level-entry lines (§4), ambience bed, rain and
//! event cues (§5–§7), sample pins (§8).
//!
//! Plain Rust, no float, no I/O: [`Environment::tick`] runs once per sound
//! tick with every input passed explicitly ([`TickInput`]) and reaches the
//! sound layer only through [`EnvCalls`] (= [`SoundCalls`] plus the few
//! request queries these machines need). Things outside the sound layer
//! (the client quest check, the player event line of `triggers.md` §3 r4)
//! go through [`EnvHooks`].
//!
//! Inputs owned by other specs are plain parameters (`environment.md` §1
//! r5): the day phase is the period index of the client act's environment
//! record (§1 r3, `render/lighting.md` §9.1–§9.3); the weather flag and
//! intensity are the weather state of `render/draw-order-2.md`
//! §11.1–§11.3.

use std::collections::BTreeSet;

use super::calls::{jitter, uniform, Handle, SoundCalls, FLAG_NO_FADE_IN};
use crate::bridge::world::UnitKey;

#[cfg(test)]
mod tests;

/// The request queries the environment machines need beyond
/// [`SoundCalls`]. Each names its 1.14d entry point; the sound table
/// (`audio/sound-table.md`) owns what they read.
pub trait EnvCalls: SoundCalls {
    /// `0x004B9610(id)`: a request with this id exists (§2 r6).
    fn id_requested(&self, id: i32) -> bool;
    /// `0x004B9C60`: a request with an id in 4,657–4,698 is still active
    /// (not ended; a stop flag alone does not end it) (§2 r6).
    fn music_active(&self) -> bool;
    /// `0x004B9D50` → `0x004DF900`: the play position of the first
    /// request of the active list whose current id is `id`, if it is
    /// playing, else `None` (§1 r6; §2 r8, §3 r1). Units: 4-byte units of
    /// the stream (`sound-table.md` §7 r8), one frame of a 16-bit stereo
    /// song.
    fn play_position(&self, id: i32) -> Option<u32>;
    /// `Block 1`, `Block 2`, `Block 3` of sound row `id`
    /// (`sound-table.md` §1 r2; an empty cell reads −1).
    fn blocks(&self, id: i32) -> [i32; 3];
    /// Volume (+0x1C) of the request with this handle, `None` when no
    /// such request exists (§6 r3).
    fn volume(&self, h: Handle) -> Option<i32>;
    /// `0x004B99A0(h, x, y, z)` (`sound-table.md` §5 r8): the request's
    /// position := (x, y, z + 640.0) and its distance² from x, y (§7 r3).
    /// The original stores f32; every value passed here is integral.
    fn set_position(&mut self, h: Handle, x: i32, y: i32, z: i32);
}

/// Calls into client code outside the sound layer.
pub trait EnvHooks {
    /// `0x004A4180(q)` (§4 r2, `world/quests-status.md` §12): 1 when quest
    /// `q` is open and not done for this player: byte q of the last S→C
    /// 0x5E ≠ 0 (indexed directly, so q 12 / 13 read the bytes of chains 11
    /// / 12), the first quest-log entry whose chain is q found, its slot
    /// clear in the game record (bit 13) and the player record (bits 1, 0,
    /// 14), and for q = 1 the Den of Evil shown status < 5. Only called
    /// with q ≠ 0.
    fn quest_check(&self, q: u8) -> bool;
    /// Player event `e` on the local player P (`audio/triggers.md` §3 r4:
    /// the class quest line base + e − 33, delay per that rule) (§4 r2).
    fn player_event(&mut self, s: &mut dyn SoundCalls, e: u8);
}

/// The `soundenviron.txt` values these machines read (`sound-table.md`
/// §2 offsets; the EAX columns are not reproduced, §7 r5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EnvRow {
    /// `Song` (+0x00).
    pub song: i32,
    /// `Day Ambience` (+0x04).
    pub day_ambience: i32,
    /// `Night Ambience` (+0x08).
    pub night_ambience: i32,
    /// `Day Event` (+0x0C).
    pub day_event: i32,
    /// `Night Event` (+0x10).
    pub night_event: i32,
    /// `Event Delay` (+0x14).
    pub event_delay: i32,
    /// `Indoors` (+0x18, u8).
    pub indoors: u8,
    /// `Material 1` (+0x1C).
    pub material1: i32,
    /// `Material 2` (+0x20).
    pub material2: i32,
}

/// E for a level: row `SoundEnv` of `rows`; an index outside the table is
/// no row (§1 r2).
pub fn env_row(rows: &[EnvRow], sound_env: u8) -> Option<EnvRow> {
    rows.get(usize::from(sound_env)).copied()
}

/// Day: phase ∈ {1, 2, 3}; any other phase is night (§1 r3).
pub fn is_day(phase: u32) -> bool {
    (1..=3).contains(&phase)
}

/// The song range (`sound-table.md` §1 r5): minimum and maximum `Song` over
/// the `soundenviron` rows with `Song > 0`. Live: 4,657–4,684.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongRange {
    pub first: i32,
    pub last: i32,
}

impl SongRange {
    /// The live 1.14d range.
    pub const LIVE: SongRange = SongRange {
        first: 4657,
        last: 4684,
    };

    /// `None` when no row has `Song > 0`.
    pub fn from_rows(rows: &[EnvRow]) -> Option<SongRange> {
        let songs = rows.iter().map(|r| r.song).filter(|&s| s > 0);
        let first = songs.clone().min()?;
        let last = songs.max()?;
        Some(SongRange { first, last })
    }

    pub fn contains(&self, id: i32) -> bool {
        (self.first..=self.last).contains(&id)
    }

    fn len(&self) -> usize {
        usize::try_from(self.last - self.first + 1).unwrap_or(0)
    }
}

/// The local player as these machines see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerState {
    pub key: UnitKey,
    /// `0x00464820(P) = 0` (§3 r4).
    pub alive: bool,
    /// P+0x7C: last voice, in C (`triggers.md` §1 r6) (§4 r2).
    pub last_voice: u32,
}

/// Everything one sound tick reads besides the sound layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickInput {
    /// L: the local player's level id; 0 = no player or no room (§1 r1).
    pub level: u32,
    /// E: [`env_row`] of `levels[L].SoundEnv` (§1 r2).
    pub env: Option<EnvRow>,
    /// The period index (+0x00) of the client act's environment record
    /// (§1 r3, r5; answers open question 3): set by the client update's
    /// advance and S→C 0x53 (`render/lighting.md` §9.2–§9.3), index 2
    /// before the first 0x53 (§9.1). Day is 1–3.
    pub day_phase: u32,
    /// Weather active (`0x00473C40`; `render/draw-order-2.md` §11.1–§11.3,
    /// §1 r5).
    pub weather_active: bool,
    /// trunc(intensity × 255.0) of the weather intensity (f32
    /// `[0x007A89A0]` = target particles × 1/256, `render/draw-order-2.md`
    /// §11.1), computed by the weather owner so no float reaches this
    /// module (§6 r1).
    pub rain_level: i32,
    /// `Master Volume` 0–100 (`sound-table.md` §9).
    pub master_volume: i32,
    /// `Music Volume` 0–100 (`sound-table.md` §9).
    pub music_volume: i32,
    /// C: client update counter (`triggers.md` §1 r5).
    pub c: u32,
    /// P; `None` when there is no local player.
    pub player: Option<PlayerState>,
    /// Number of levels (`levels.txt` rows) (§4 r1).
    pub level_count: u32,
}

impl TickInput {
    fn audible(&self) -> bool {
        self.master_volume != 0 && self.music_volume != 0
    }

    fn day(&self) -> bool {
        is_day(self.day_phase)
    }
}

/// What §8 asks of the sample cache on a level change: lock the walk and
/// run footstep groups of these materials (`Material 1`, `Material 2`)
/// after clearing the locks of groups 2,720–2,876. Cache only
/// (`sound-table.md` §10 r4): no observable effect in d2rs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SamplePins {
    pub materials: [i32; 2],
}

/// Which `Block` a stinger stores as the interrupted song's resume point
/// (§3 r1, column k).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeBlock {
    /// k = 0: resume at 0.
    Start,
    /// k = 1..3: `Block k` of the song's row.
    Block(BlockIndex),
}

/// `Block 1`..`Block 3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockIndex {
    B1,
    B2,
    B3,
}

/// The arguments of `0x004DCD40(M, dM, H, k, S, dS, play)` (§3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StingerArgs {
    /// Quest track (0: none).
    pub m: i32,
    /// Delay of M, in C.
    pub dm: u32,
    /// Music hold, in C.
    pub h: u32,
    pub k: ResumeBlock,
    /// Hero quest line.
    pub s: i32,
    /// Delay of S, in C.
    pub ds: u32,
    /// Whether S is played.
    pub play: bool,
}

/// One row of the §3 r6 caller table (player events).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StingerRow {
    pub event: u8,
    pub m: i32,
    pub dm: u32,
    pub h: u32,
    pub k: ResumeBlock,
    pub ds: u32,
    pub play: bool,
}

const fn row(event: u8, m: i32, dm: u32, h: u32, k1: bool, ds: u32, play: bool) -> StingerRow {
    StingerRow {
        event,
        m,
        dm,
        h,
        k: if k1 {
            ResumeBlock::Block(BlockIndex::B1)
        } else {
            ResumeBlock::Start
        },
        ds,
        play,
    }
}

/// §3 r6: the player events that start a stinger.
pub const STINGER_ROWS: [StingerRow; 11] = [
    row(33, 4685, 0, 475, false, 425, true),
    row(34, 4686, 475, 800, true, 525, true),
    row(35, 4688, 0, 300, true, 250, true),
    row(37, 0, 0, 1500, false, 0, false),
    row(50, 4693, 475, 1100, true, 525, true),
    row(52, 4694, 0, 425, true, 375, true),
    row(66, 4692, 0, 425, false, 375, true),
    row(75, 4695, 0, 475, false, 325, true),
    row(80, 4698, 0, 500, true, 475, true),
    row(82, 4697, 0, 550, true, 400, true),
    row(83, 4696, 0, 525, true, 425, true),
];

/// §3 r6: 4,687 `music_quest_compelling` (`0x0046B850`), no speech.
pub const STINGER_COMPELLING: StingerArgs = StingerArgs {
    m: 4687,
    dm: 0,
    h: 350,
    k: ResumeBlock::Start,
    s: 0,
    ds: 0,
    play: false,
};

/// §3 r6: 4,691 `music_quest_izual` (`0x0046BC10`), no speech.
pub const STINGER_IZUAL: StingerArgs = StingerArgs {
    m: 4691,
    dm: 0,
    h: 250,
    k: ResumeBlock::Start,
    s: 0,
    ds: 0,
    play: false,
};

/// §3 r6: the stinger of player event `e`, with S = the class quest line
/// base + e − 33 (`triggers.md` §3 r4); `None` for an event without one.
pub fn stinger_for_event(e: u8, quest_base: i32) -> Option<StingerArgs> {
    STINGER_ROWS
        .iter()
        .find(|r| r.event == e)
        .map(|r| StingerArgs {
            m: r.m,
            dm: r.dm,
            h: r.h,
            k: r.k,
            s: quest_base + i32::from(e) - 33,
            ds: r.ds,
            play: r.play,
        })
}

/// One record of the level-entry table `0x0072A2C4` (§4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryRecord {
    pub levels: &'static [u32],
    /// Quest q (0: no quest check).
    pub quest: u8,
    /// Player event e (the line).
    pub event: u8,
}

/// §4: the 14 records, in table order.
pub const ENTRY_RECORDS: [EntryRecord; 14] = [
    EntryRecord {
        levels: &[17],
        quest: 2,
        event: 38,
    },
    EntryRecord {
        levels: &[34, 35, 36, 37],
        quest: 6,
        event: 40,
    },
    EntryRecord {
        levels: &[8],
        quest: 1,
        event: 41,
    },
    EntryRecord {
        levels: &[29, 30, 31],
        quest: 6,
        event: 42,
    },
    EntryRecord {
        levels: &[26, 27],
        quest: 3,
        event: 43,
    },
    EntryRecord {
        levels: &[20, 21, 22, 23, 24, 25],
        quest: 5,
        event: 44,
    },
    EntryRecord {
        levels: &[38],
        quest: 6,
        event: 46,
    },
    EntryRecord {
        levels: &[2, 3, 4, 5, 6, 7],
        quest: 1,
        event: 47,
    },
    EntryRecord {
        levels: &[74],
        quest: 12,
        event: 55,
    },
    EntryRecord {
        levels: &[58],
        quest: 13,
        event: 56,
    },
    EntryRecord {
        levels: &[110],
        quest: 31,
        event: 76,
    },
    EntryRecord {
        levels: &[120],
        quest: 35,
        event: 78,
    },
    EntryRecord {
        levels: &[121],
        quest: 34,
        event: 77,
    },
    EntryRecord {
        levels: &[132],
        quest: 36,
        event: 79,
    },
];

// ---------------------------------------------------------------------------
// §4 Level-entry lines
// ---------------------------------------------------------------------------

/// State of `0x004CC270`: last level checked `[0x007C88CC]` and the
/// per-level flags `[0x007C78B8]`. §4 r4 (answers open question 6): sound
/// init at every game start clears the flags ([`EntryLines::game_start`],
/// `0x004CA280`); the last level checked is never reset, so it carries
/// the previous game's last level into the next game (reproduced).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntryLines {
    last_checked: u32,
    flagged: BTreeSet<u32>,
}

impl EntryLines {
    pub fn is_flagged(&self, level: u32) -> bool {
        self.flagged.contains(&level)
    }

    /// The last level checked (`[0x007C88CC]`).
    pub fn last_checked(&self) -> u32 {
        self.last_checked
    }

    /// `0x004CA280` from sound init (§4 r4): the level flags := 0; the
    /// last level checked keeps its value.
    pub fn game_start(&mut self) {
        self.flagged.clear();
    }

    /// §4 r1–r3.
    fn check(
        &mut self,
        s: &mut dyn EnvCalls,
        hooks: &mut dyn EnvHooks,
        inp: &TickInput,
        level: u32,
    ) {
        if level >= inp.level_count || level == self.last_checked {
            return;
        }
        self.last_checked = level;
        if self.flagged.contains(&level) {
            return;
        }
        let Some(rec) = ENTRY_RECORDS.iter().find(|r| r.levels.contains(&level)) else {
            return;
        };
        self.flagged.extend(rec.levels.iter().copied());
        let Some(p) = inp.player else {
            return;
        };
        let quest_ok = rec.quest == 0 || hooks.quest_check(rec.quest);
        // C − P+0x7C, unsigned like every C difference here.
        if quest_ok && inp.c.wrapping_sub(p.last_voice) > 62 && !s.any_speech() {
            hooks.player_event(s, rec.event);
        }
    }
}

// ---------------------------------------------------------------------------
// §2 Music and §3 stingers
// ---------------------------------------------------------------------------

/// §3 state (`[0x007C89E4]`–`[0x007C8A00]`); times in C.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stinger {
    pub active: bool,
    pub m: i32,
    pub tm: u32,
    pub m_pending: bool,
    pub s: i32,
    pub ts: u32,
    pub s_pending: bool,
    pub th: u32,
}

/// §2 state: `cur` `[0x007C89D8]`, last level `[0x007C89DC]`, Tl
/// `[0x007C89D0]`, Ts `[0x007C89E0]`, last announced `[0x007C8A04]`, the
/// resume table `[0x007BC99C]`, and the §3 stinger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Music {
    range: SongRange,
    pub cur: i32,
    pub last_level: u32,
    pub tl: u32,
    pub ts: u32,
    pub last_announced: u32,
    /// One value per song id of the range (`Block` values, may be −1).
    resume: Vec<i32>,
    pub stinger: Stinger,
}

impl Music {
    pub fn new(range: SongRange) -> Self {
        Music {
            range,
            cur: 0,
            last_level: 0,
            tl: 0,
            ts: 0,
            last_announced: 0,
            resume: vec![0; range.len()],
            stinger: Stinger::default(),
        }
    }

    /// §2 r9 (`0x004DCA30`): every §2 and §3 variable and the resume
    /// table := 0. Run by [`Environment::sound_init`], which applies the
    /// `-ns` condition.
    pub fn reset(&mut self) {
        *self = Music::new(self.range);
    }

    /// resume[id]; 0 outside the song range.
    pub fn resume(&self, id: i32) -> i32 {
        self.slot(id).map_or(0, |i| self.resume[i])
    }

    pub fn set_resume(&mut self, id: i32, v: i32) {
        if let Some(i) = self.slot(id) {
            self.resume[i] = v;
        }
    }

    fn slot(&self, id: i32) -> Option<usize> {
        if self.range.contains(id) {
            usize::try_from(id - self.range.first).ok()
        } else {
            None
        }
    }

    /// §3 r1–r3: `0x004DCD40(M, dM, H, k, S, dS, play)`. T is
    /// `s.sound_tick()`, `c` is C.
    pub fn start_stinger(&mut self, s: &mut dyn EnvCalls, c: u32, a: StingerArgs) {
        if self.cur != 0 && s.play_position(self.cur).is_some() {
            let v = match a.k {
                ResumeBlock::Start => 0,
                ResumeBlock::Block(b) => s.blocks(self.cur)[b as usize],
            };
            self.set_resume(self.cur, v);
            self.ts = s.sound_tick();
        }
        s.stop_songs();
        self.stinger = Stinger {
            active: true,
            m: a.m,
            tm: c.wrapping_add(a.dm),
            m_pending: true,
            s: a.s,
            ts: c.wrapping_add(a.ds),
            s_pending: a.play,
            th: c.wrapping_add(a.h),
        };
    }

    /// §3 r5: `0x004DCE10(dt, play)` (0x2C event 92 with (25, 1)).
    pub fn rearm_stinger(&mut self, c: u32, dt: u32, play: bool) {
        let st = &mut self.stinger;
        if !st.active {
            return;
        }
        if play {
            st.ts = c.wrapping_add(dt);
            st.s_pending = true;
            st.th = c.wrapping_add(dt).wrapping_add(50);
        } else {
            st.th = st.ts.wrapping_add(50);
            st.s_pending = false;
        }
    }

    /// §3 r4; true while the stinger holds the rest of §2.
    fn stinger_tick(&mut self, s: &mut dyn EnvCalls, inp: &TickInput, audible: bool) -> bool {
        let st = &mut self.stinger;
        if st.m_pending && inp.c >= st.tm {
            s.request(st.m, None, 0, 0, 0);
            st.m_pending = false;
        }
        if st.s_pending && inp.c >= st.ts {
            if let Some(p) = inp.player.filter(|p| p.alive) {
                s.request(st.s, Some(p.key), 0, 0, 0);
            }
            st.s_pending = false;
        }
        if inp.c < st.th {
            return true;
        }
        st.active = false;
        if audible {
            s.stop_id(st.m);
        }
        false
    }

    /// §2 r1–r8 (`0x004DCAA0(T)`), for a tick with L ≠ 0.
    fn tick(
        &mut self,
        s: &mut dyn EnvCalls,
        hooks: &mut dyn EnvHooks,
        inp: &TickInput,
        entry: &mut EntryLines,
    ) {
        let t = s.sound_tick();
        let l = inp.level;
        // r1
        let song = inp
            .env
            .map(|e| e.song)
            .filter(|&id| self.range.contains(id))
            .unwrap_or(0);
        let audible = inp.audible();
        // r2
        if l != self.last_level {
            self.tl = t;
            self.last_level = l;
        }
        // r3
        if self.cur != song && (t.wrapping_sub(self.tl) > 74 || !audible || self.cur == 0) {
            self.cur = song;
            self.ts = t;
        }
        // r4
        if !audible || self.cur == 0 {
            s.stop_songs();
        }
        // r5
        if self.stinger.active && self.stinger_tick(s, inp, audible) {
            return;
        }
        // r6
        if self.cur != 0 && audible && !s.id_requested(self.cur) {
            let o = self.resume(self.cur);
            let flags = if !s.music_active() && o == 0 {
                FLAG_NO_FADE_IN
            } else {
                0
            };
            s.stop_songs();
            // The stored value is passed through as the offset (a −1
            // `Block` cell from §3 r1 included).
            s.request(self.cur, None, 0, flags, o as u32);
        }
        // r7
        if l != self.last_announced && t.wrapping_sub(self.tl) >= 62 {
            entry.check(s, hooks, inp, l);
            self.last_announced = l;
        }
        // r8
        if self.cur != 0 && t > self.ts.wrapping_add(125) {
            if let Some(p) = s.play_position(self.cur) {
                let r = resume_point(s.blocks(self.cur), p);
                self.set_resume(self.cur, r);
                self.ts = t;
            }
        }
    }
}

/// §2 r8: the next block boundary after play position `p`, or 0; −1 cells
/// never match.
pub fn resume_point(blocks: [i32; 3], p: u32) -> i32 {
    let p = i64::from(p);
    let [b1, b2, b3] = blocks.map(i64::from);
    if 0 <= p && p < b1 {
        blocks[0]
    } else if b1 <= p && p < b2 {
        blocks[1]
    } else if b2 <= p && p < b3 {
        blocks[2]
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// §5–§8 Ambience, rain, event cues, sample pins
// ---------------------------------------------------------------------------

/// `scene_rain` (§6 r1).
pub const SCENE_RAIN: i32 = 64;
/// Bed cross-fade length, in T (§5 r2–r3).
pub const BED_FADE: u32 = 250;
/// Rain volume step per tick (§6 r3).
pub const RAIN_STEP: i32 = 6;
/// The z argument of the cue's position call (§7 r3, r6:
/// `position(h, x, y, 0)`); `0x004B99A0` adds 640.0 (`sound-table.md` §5
/// r8), so the cue sits at z = 640.0.
pub const CUE_Z: i32 = 0;

/// `0x004E42E0` state: bed `[0x007C8C88]` and its handle `[0x007C8C80]`,
/// rain handle `[0x007C8C84]` and previous id `[0x007C8C8C]`, event id
/// `[0x007C8C90]`, gap `[0x007C8C94]`, last cue `[0x007C8C98]`, pinned
/// level `[0x007C8C7C]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ambience {
    pub bed: i32,
    pub bed_handle: Handle,
    pub rain_handle: Handle,
    pub rain_prev: i32,
    pub event_id: i32,
    pub gap: i32,
    pub last_cue: u32,
    pub pinned_level: u32,
}

impl Ambience {
    /// §5–§8 for a tick with L ≠ 0. Returns the §8 pins on a level change.
    fn tick(&mut self, s: &mut dyn EnvCalls, inp: &TickInput) -> Option<SamplePins> {
        self.bed(s, inp);
        self.rain(s, inp);
        self.cues(s, inp);
        // §7 r5: EAX room settings, mixer mode 2 only: not reproduced.
        self.pins(inp)
    }

    /// §5 r1–r3.
    fn bed(&mut self, s: &mut dyn EnvCalls, inp: &TickInput) {
        let day = inp.day();
        let a = inp.env.map_or(0, |e| {
            if day {
                e.day_ambience
            } else {
                e.night_ambience
            }
        });
        if a == self.bed {
            return;
        }
        let swap = inp.env.is_some_and(|e| {
            let (d, n) = (e.day_ambience, e.night_ambience);
            (a, self.bed) == (d, n) || (a, self.bed) == (n, d)
        });
        if swap {
            s.fade(self.bed_handle, 0, 0, BED_FADE);
        } else {
            // "when raining": the weather flag of this tick (§6 r1 r ≠ 0).
            let rain = if inp.weather_active { SCENE_RAIN } else { 0 };
            s.stop_52_71_except(a, rain);
            // §5 r2 (`0x004E4403`): the second exception of `0x004BA9D0`
            // is always 0.
            s.stop_72_201_except(event_of(inp), 0);
        }
        if a == 0 {
            self.bed = 0;
            self.bed_handle = 0;
        } else {
            self.bed_handle = s.request(a, None, 0, 0, 0);
            if swap {
                s.set_volume(self.bed_handle, 0);
                s.fade(self.bed_handle, 255, 0, BED_FADE);
            }
            self.bed = a;
        }
    }

    /// §6 r1–r3.
    fn rain(&mut self, s: &mut dyn EnvCalls, inp: &TickInput) {
        let r = if inp.weather_active { SCENE_RAIN } else { 0 };
        let v = if inp.weather_active {
            inp.rain_level
        } else {
            0
        };
        if r == 0 {
            s.stop_handle(self.rain_handle);
            self.rain_handle = 0;
            self.rain_prev = 0;
            return;
        }
        if v != 0 && self.rain_handle == 0 {
            // The previous tick's id: the first raining tick requests 0.
            self.rain_handle = s.request(self.rain_prev, None, 0, 0, 0);
            s.set_volume(self.rain_handle, v);
        }
        if self.rain_handle != 0 {
            // §6 r4: `0x004B9B20` reads 0 for a handle whose request is
            // gone; with v > 0 the step then writes min(6, v) to the
            // missing request (nothing happens) and the stale handle is
            // kept until v reaches 0 or the weather ends.
            let w = s.volume(self.rain_handle).unwrap_or(0);
            let w = if w < v {
                (w + RAIN_STEP).min(v)
            } else if w > v {
                (w - RAIN_STEP).max(v)
            } else {
                w
            };
            if w == 0 {
                s.stop_handle(self.rain_handle);
                self.rain_handle = 0;
            } else {
                s.set_volume(self.rain_handle, w);
            }
        }
        self.rain_prev = r;
    }

    /// §7 r1–r4.
    fn cues(&mut self, s: &mut dyn EnvCalls, inp: &TickInput) {
        let ev = event_of(inp);
        if ev == 0 {
            return;
        }
        let d = inp.env.map_or(0, |e| e.event_delay);
        let t = s.sound_tick();
        if ev != self.event_id {
            self.gap = d + jitter(s, d / 3);
            self.last_cue = t.wrapping_sub(s.roll(self.gap));
            self.event_id = ev;
        }
        if t.wrapping_sub(self.last_cue) >= self.gap as u32 {
            let h = s.request(ev, None, 0, 0, 0);
            if h != 0 {
                let sign = if s.roll(2) != 0 { 1 } else { -1 };
                let x = sign * uniform(s, 450, 750);
                let y = jitter(s, 100);
                s.set_position(h, x, y, CUE_Z);
                self.last_cue = t;
                self.gap = d + jitter(s, d / 3);
            }
        }
    }

    /// §8: on a level change, the footstep groups to pin.
    fn pins(&mut self, inp: &TickInput) -> Option<SamplePins> {
        if inp.level == self.pinned_level {
            return None;
        }
        self.pinned_level = inp.level;
        let e = inp.env.unwrap_or_default();
        Some(SamplePins {
            materials: [e.material1, e.material2],
        })
    }
}

/// E's `Day Event` by day, else `Night Event`; 0 without E (§7 r1).
fn event_of(inp: &TickInput) -> i32 {
    let day = inp.day();
    inp.env
        .map_or(0, |e| if day { e.day_event } else { e.night_event })
}

// ---------------------------------------------------------------------------
// The whole machine
// ---------------------------------------------------------------------------

/// The environment machines of one client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    pub ambience: Ambience,
    pub music: Music,
    pub entry: EntryLines,
}

impl Environment {
    pub fn new(range: SongRange) -> Self {
        Environment {
            ambience: Ambience::default(),
            music: Music::new(range),
            entry: EntryLines::default(),
        }
    }

    /// Sound init at a game start (`0x00482260`): the music reset of §2 r9
    /// runs unless the start-up configuration's `-ns` switch
    /// (`[0x007A0438]` +0x220) is set (answers open question 7); the
    /// level-entry flags are cleared (§4 r4).
    pub fn sound_init(&mut self, no_sound_switch: bool) {
        if !no_sound_switch {
            self.music.reset();
        }
        self.entry.game_start();
    }

    /// One sound tick (`0x00482C20`, before the request update): ambience
    /// (§5–§8), then music (§2–§4). L = 0 → nothing (§1 r1). Returns the
    /// §8 pins when the level changed.
    pub fn tick(
        &mut self,
        s: &mut dyn EnvCalls,
        hooks: &mut dyn EnvHooks,
        inp: &TickInput,
    ) -> Option<SamplePins> {
        if inp.level == 0 {
            return None;
        }
        let pins = self.ambience.tick(s, inp);
        self.music.tick(s, hooks, inp, &mut self.entry);
        pins
    }
}
