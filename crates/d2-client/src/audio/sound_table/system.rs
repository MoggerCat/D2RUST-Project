// Spec: specs/audio/sound-table.md (§5 requests, §6 sound tick, §7 channels, §8 volume and pan, §10 sample cache)
// Spec: specs/audio/triggers.md (§1 r2–r4, r8: volume set, detach, group stops, speaking)
//! The request layer and the 16 channels (`sound-table.md` §5–§7), driven
//! one sound tick at a time (§6). Every channel start, stop and volume/pan
//! send becomes a cue on the audio core's [`TriggerQueue`] stamped with the
//! sound tick, so the voice log records them (`client/audio.md` §A5); the
//! core's voice policy must then admit every start ([`crate::audio::Unlimited`]),
//! since channel limits and stealing are decided here.
//!
//! Unit positions, the listener, occlusion and the client RNG seed come
//! through the narrow [`SoundWorld`] trait, never from Bevy.

use crate::audio::calls::{Handle, SoundCalls, FLAG_EXACT, FLAG_NO_FADE_IN};
use crate::audio::{
    Cue, CueId, ParamChange, SoundBank, SoundId, Stop, StopTarget, Trigger, TriggerQueue,
    TriggerSource, VoiceParams,
};
use crate::bridge::world::UnitKey;
use d2_sim::rng::Seed;

use super::table::{is_music_path, is_speech, LoadState, SoundTableData, SoundTableError};
use super::volume::{
    chain, falloff, linear_falloff, mode0_gain_pan, record_volume, ChainInputs, SoundSettings,
};

/// Request pool size (`0x007C0ED0`, §5).
pub const REQUEST_SLOTS: usize = 200;
/// Channel slots (`0x007C8A78`, §7 r3).
pub const CHANNELS: usize = 16;
/// Solo duck floor (§6.5 r1).
pub const SOLO_DUCK_MIN: i32 = 70;
/// Fade-in length after a resume (§6.3 r3).
pub const RESUME_FADE: u32 = 3;
/// Async loads pending at most during a preload (§10 r3).
pub const MAX_PENDING_LOADS: usize = 15;
/// Preload period in sound ticks (§10 r3).
pub const PRELOAD_PERIOD: u32 = 25;
/// Duration of one sound tick in ms: one client tick (`render/camera.md`
/// §9), used only by d2rs's channel-end model ([`SoundSystem`] upkeep).
pub const TICK_MS: u64 = 40;
/// Distance clamp per axis for distance² (§8.1 r1).
const AXIS_CLAMP: f32 = 2000.0;
/// Group base of `event_thunder_*` (§6.4 r2).
const THUNDER_BASE: i32 = 202;

/// What the sound layer needs from the game world (§8.1, §6.4, §4 r5).
pub trait SoundWorld {
    /// The local player unit `[0x007A6A70]`.
    fn local_player(&self) -> Option<UnitKey>;
    /// A unit's client position (`0x00620900`).
    ///
    /// TODO(spec: audio/sound-table.md open question 2): the units of
    /// these coordinates per unit type.
    fn position(&self, unit: UnitKey) -> Option<(i32, i32)>;
    /// `0x00622AA0(player, unit, 2) ≠ 0` (§6.4 r2).
    fn blocked(&self, unit: UnitKey) -> bool;
    /// `Indoors` of the current sound environment (§6.4 r2).
    fn indoors(&self) -> bool;
    /// The state-duck condition at `0x004BA640` (§6.5 r2; open question 6).
    fn state_duck(&self) -> bool;
    /// The local player's client unit seed (player + 0x20, §4 r5).
    fn client_seed(&mut self) -> Option<&mut Seed>;
}

/// Request state (+0x30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestState {
    Waiting,
    Playing,
    Ended,
}

/// A volume fade (+0x45 … +0x55).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fade {
    pub start: i32,
    pub end: i32,
    pub t0: u32,
    pub t1: u32,
}

impl Fade {
    /// §6.3 r2: the volume at `now` and whether the fade has ended
    /// (signed 32-bit, truncating).
    pub fn volume_at(&self, now: u32) -> (i32, bool) {
        if now < self.t0 {
            (self.start, false)
        } else if now > self.t1 {
            (self.end, true)
        } else {
            let num = self
                .end
                .wrapping_sub(self.start)
                .wrapping_mul(now.wrapping_sub(self.t0) as i32);
            let v = self
                .start
                .wrapping_add(num / self.t1.wrapping_sub(self.t0) as i32);
            (v, false)
        }
    }
}

/// One request (§5).
#[derive(Clone, Debug)]
pub struct Request {
    /// Pool slot (its address order).
    pub slot: usize,
    /// +0x04, the requested id.
    pub id: i32,
    /// The variant chosen at the last start attempt (§7 r1); `id` before.
    pub variant: i32,
    /// +0x08.
    pub handle: Handle,
    /// +0x0C … +0x14.
    pub pos: [f32; 3],
    /// +0x18.
    pub dist2: f32,
    /// +0x1C.
    pub volume: i32,
    /// +0x20.
    pub occlusion: f32,
    /// +0x24, bytes.
    pub start_offset: u32,
    /// +0x28.
    pub units: Vec<UnitKey>,
    /// +0x2C.
    pub channel: Option<usize>,
    pub state: RequestState,
    /// +0x34.
    pub start_tick: u32,
    /// +0x38.
    pub flags: u32,
    /// +0x3C.
    pub priority: u8,
    /// +0x3D.
    pub stop: bool,
    /// +0x41, bytes (§7 r3).
    pub resume_offset: Option<u32>,
    pub fade: Option<Fade>,
}

/// Channel kind (§7 r3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKind {
    /// Takes any request. d2rs's layout until the mode-0 slot kinds are
    /// specified (TODO(spec: audio/sound-table.md §7 r3): the kinds
    /// `0x004E0050` gives the 16 slots in mixer mode 0).
    Any,
    Plain,
    Stereo,
    Positional,
    Eax,
}

/// One busy channel.
#[derive(Clone, Debug)]
struct Channel {
    request: usize,
    /// The variant playing on it.
    id: i32,
    cue: CueId,
    start_tick: u32,
    offset_frames: u64,
    frames: u64,
    rate: u64,
    block_align: u64,
    looped: bool,
    stereo: bool,
    /// Loop start in bytes (`Block 1` × 2, §7 r4), when the block count is 1.
    loop_start: Option<u32>,
    vol: i32,
    pan: i32,
    /// (v after §8.2 r5, occlusion, position) last sent (§8.2 r7).
    last_sent: (i32, f32, [f32; 3]),
}

/// A fatal condition of 1.14d, kept instead of aborting (M07).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SoundError {
    #[error(transparent)]
    Table(#[from] SoundTableError),
    /// §5 r5: `len = 0` with `delay ≠ 0` (fatal `0x25C`).
    #[error("fade of handle {handle} with len 0 and delay {delay}")]
    FadeDelay { handle: Handle, delay: u32 },
    /// A request for an id outside the table (1.14d reads a null record).
    #[error("sound id {0} outside the table")]
    OutOfTable(i32),
}

/// The sound table, the request pool and the channels (§1–§10).
pub struct SoundSystem {
    table: SoundTableData,
    bank: Box<dyn SoundBank + Send>,
    settings: SoundSettings,
    /// `0x007C545C`.
    enabled: bool,
    slots: Vec<Option<Request>>,
    /// Active list `0x007C5458`: slot indices.
    active: Vec<usize>,
    next_handle: u32,
    layout: [ChannelKind; CHANNELS],
    channels: [Option<Channel>; CHANNELS],
    /// `0x007BC9BC`.
    tick: u32,
    last_update: Option<u32>,
    solo_duck: i32,
    state_duck: i32,
    pending_loads: Vec<i32>,
    last_preload: Option<u32>,
    errors: Vec<SoundError>,
}

impl SoundSystem {
    pub fn new(table: SoundTableData, bank: Box<dyn SoundBank + Send>) -> Self {
        SoundSystem {
            table,
            bank,
            settings: SoundSettings::default(),
            enabled: true,
            slots: vec![None; REQUEST_SLOTS],
            active: Vec::new(),
            next_handle: 0,
            layout: [ChannelKind::Any; CHANNELS],
            channels: Default::default(),
            tick: 0,
            last_update: None,
            solo_duck: 100,
            state_duck: 100,
            pending_loads: Vec::new(),
            last_preload: None,
            errors: Vec::new(),
        }
    }

    pub fn table(&self) -> &SoundTableData {
        &self.table
    }

    pub fn table_mut(&mut self) -> &mut SoundTableData {
        &mut self.table
    }

    pub fn settings(&self) -> &SoundSettings {
        &self.settings
    }

    pub fn set_settings(&mut self, s: SoundSettings) {
        self.settings = s;
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    pub fn set_layout(&mut self, layout: [ChannelKind; CHANNELS]) {
        self.layout = layout;
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }

    pub fn solo_duck(&self) -> i32 {
        self.solo_duck
    }

    pub fn state_duck(&self) -> i32 {
        self.state_duck
    }

    pub fn take_errors(&mut self) -> Vec<SoundError> {
        std::mem::take(&mut self.errors)
    }

    /// The active requests in list order.
    pub fn requests(&self) -> impl Iterator<Item = &Request> {
        self.active.iter().filter_map(|&s| self.slots[s].as_ref())
    }

    pub fn request_by_handle(&self, h: Handle) -> Option<&Request> {
        self.requests().find(|r| r.handle == h && h != 0)
    }

    /// The request playing on channel `c`.
    pub fn channel_request(&self, c: usize) -> Option<&Request> {
        let ch = self.channels.get(c)?.as_ref()?;
        self.slots[ch.request].as_ref()
    }

    /// Volume and pan last sent on channel `c`.
    pub fn channel_output(&self, c: usize) -> Option<(i32, i32)> {
        let ch = self.channels.get(c)?.as_ref()?;
        Some((ch.vol, ch.pan))
    }

    /// Loop start in bytes of channel `c` (§7 r4).
    ///
    /// TODO(spec: audio/sound-table.md §7 r4): the core mixer has no loop
    /// start or start offset yet; the values are kept here.
    pub fn channel_loop_start(&self, c: usize) -> Option<u32> {
        self.channels.get(c)?.as_ref()?.loop_start
    }

    /// Wraps this system with a world as the [`SoundCalls`] surface.
    pub fn with<'a>(&'a mut self, world: &'a mut dyn SoundWorld) -> SoundCtx<'a> {
        SoundCtx { sys: self, world }
    }

    fn handle_slot(&self, h: Handle) -> Option<usize> {
        if h == 0 {
            return None;
        }
        self.active
            .iter()
            .copied()
            .find(|&s| self.slots[s].as_ref().is_some_and(|r| r.handle == h))
    }

    fn req(&self, s: usize) -> &Request {
        self.slots[s].as_ref().expect("active slot holds a request")
    }

    fn req_mut(&mut self, s: usize) -> &mut Request {
        self.slots[s].as_mut().expect("active slot holds a request")
    }

    // ------------------------------------------------------------ §5

    /// Relative position and distance² of `unit` (§8.1 r1).
    ///
    /// TODO(spec: audio/sound-table.md open question 8): sound 2599
    /// (`object_river`) uses a projected position; it gets this rule.
    fn unit_position(world: &dyn SoundWorld, unit: UnitKey) -> Option<([f32; 3], f32)> {
        let p = world.position(world.local_player()?)?;
        let u = world.position(unit)?;
        let x = u.0 as f32 - p.0 as f32;
        let y = 2.0 * (u.1 as f32 - p.1 as f32);
        let cx = x.clamp(-AXIS_CLAMP, AXIS_CLAMP);
        let cy = y.clamp(-AXIS_CLAMP, AXIS_CLAMP);
        Some(([x, y, 0.0], cx * cx + cy * cy))
    }

    /// A unit's occlusion value (`0x004B9890`, §6.4 r2).
    fn unit_occlusion(&self, world: &dyn SoundWorld, id: i32, unit: UnitKey) -> f32 {
        let Some(e) = self.table.get(id) else {
            return 0.0;
        };
        if e.row.falloff == 4 {
            0.0
        } else if e.group_base == THUNDER_BASE {
            if world.indoors() {
                0.5
            } else {
                0.0
            }
        } else if world.blocked(unit) {
            0.5
        } else {
            0.0
        }
    }

    /// `0x004B9A00` (§5 r1–r4).
    pub fn request(
        &mut self,
        world: &mut dyn SoundWorld,
        id: i32,
        unit: Option<UnitKey>,
        delay: u32,
        flags: u32,
        offset: u32,
    ) -> Handle {
        if !self.enabled || id < 1 {
            return 0;
        }
        let Some(e) = self.table.get(id) else {
            // 1.14d reads through a null record here (§1 r4).
            self.errors.push(SoundError::OutOfTable(id));
            return 0;
        };
        if e.row.volume == 0 {
            return 0;
        }
        let (compound, base, priority) = (e.row.compound, e.group_base, e.row.priority);
        let now = self.tick;
        let local = unit.is_some() && unit == world.local_player();
        // §5 r2. TODO(spec: audio/sound-table.md §5 r2): whether a merged
        // call attaches its unit to the existing request's unit list; d2rs
        // attaches nothing.
        if compound != 0 {
            let hit = self.active.iter().copied().find(|&s| {
                let r = self.req(s);
                self.table.base(r.id) == base
                    && !r.stop
                    && (compound < 0
                        || (now.wrapping_sub(r.start_tick) as i32) <= i32::from(compound))
            });
            if let Some(s) = hit {
                let r = self.req_mut(s);
                if local {
                    r.priority = r.priority.wrapping_add(80);
                }
                return r.handle;
            }
        }
        // §5 r3: first free slot.
        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            return 0;
        };
        self.next_handle = self.next_handle.wrapping_add(1);
        let (pos, dist2, occlusion) = match unit {
            Some(u) => {
                let (pos, d2) = Self::unit_position(world, u).unwrap_or(([0.0; 3], 0.0));
                (pos, d2, self.unit_occlusion(world, id, u))
            }
            None => ([0.0; 3], 0.0, 0.0),
        };
        let mut priority = priority;
        // §5 r4 (§12 r1): byte wrap.
        if local {
            priority = priority.wrapping_add(80);
        }
        self.slots[slot] = Some(Request {
            slot,
            id,
            variant: id,
            handle: self.next_handle,
            pos,
            dist2,
            volume: 255,
            occlusion,
            start_offset: offset,
            units: unit.into_iter().collect(),
            channel: None,
            state: RequestState::Waiting,
            start_tick: now.wrapping_add(delay),
            flags,
            priority,
            stop: false,
            resume_offset: None,
            fade: None,
        });
        self.active.push(slot);
        self.next_handle
    }

    /// `0x004B9B50(h, v)` (`triggers.md` §1 r2).
    pub fn set_volume(&mut self, h: Handle, v: i32) {
        if let Some(s) = self.handle_slot(h) {
            self.req_mut(s).volume = v;
        }
    }

    /// `0x004B9EF0(h, target, delay, len)` (§5 r5).
    pub fn fade(&mut self, h: Handle, target: i32, delay: u32, len: u32) {
        let Some(s) = self.handle_slot(h) else { return };
        let now = self.tick;
        let r = self.req(s);
        if r.units.len() > 1 {
            return;
        }
        let Some(e) = self.table.get(r.id) else {
            return;
        };
        let same = r.channel.is_some() && r.fade.is_some_and(|f| f.end == target);
        let mut len = len;
        if !same {
            if target == 255 {
                len = len.max(u32::from(e.row.fade_in));
            } else if target == 0 {
                len = len.max(u32::from(e.row.fade_out));
            }
        }
        if len == 0 && delay != 0 {
            self.errors.push(SoundError::FadeDelay { handle: h, delay });
            return;
        }
        let r = self.req_mut(s);
        if target == 0 {
            r.stop = true;
        }
        if len > 0 {
            let t0 = now.wrapping_add(delay);
            r.fade = Some(Fade {
                start: r.volume,
                end: target,
                t0,
                t1: t0.wrapping_add(len),
            });
        } else {
            r.volume = target;
        }
    }

    /// `0x004BA790(h, unit, force)` (`triggers.md` §1 r3).
    pub fn detach(&mut self, h: Handle, unit: UnitKey, force: bool) {
        let Some(s) = self.handle_slot(h) else { return };
        let r = self.req_mut(s);
        r.units.retain(|&u| u != unit);
        if !r.units.is_empty() {
            return;
        }
        let fading_out = r.fade.is_some_and(|f| f.end == 0);
        let id = r.id;
        let looped = self.table.get(id).is_some_and(|e| e.row.looped != 0);
        if force || (looped && !fading_out) {
            self.req_mut(s).stop = true;
            self.fade(h, 0, 0, 0);
        }
    }

    /// Group stop of one request (`triggers.md` §1 r4).
    fn group_stop(&mut self, s: usize) {
        let now = self.tick;
        let r = self.req(s);
        let fade_out = self
            .table
            .get(r.id)
            .map_or(0, |e| u32::from(e.row.fade_out));
        let r = self.req_mut(s);
        r.stop = true;
        if r.state == RequestState::Playing && fade_out > 0 {
            r.fade = Some(Fade {
                start: r.volume,
                end: 0,
                t0: now,
                t1: now.wrapping_add(fade_out),
            });
        }
    }

    fn stop_where(&mut self, pred: impl Fn(&SoundTableData, &Request) -> bool) {
        let hits: Vec<usize> = self
            .active
            .iter()
            .copied()
            .filter(|&s| pred(&self.table, self.req(s)))
            .collect();
        for s in hits {
            self.group_stop(s);
        }
    }

    pub fn stop_handle(&mut self, h: Handle) {
        if let Some(s) = self.handle_slot(h) {
            self.group_stop(s);
        }
    }

    pub fn stop_id(&mut self, id: i32) {
        self.stop_where(|_, r| r.id == id);
    }

    pub fn stop_songs(&mut self) {
        self.stop_where(|t, r| t.is_song(r.id));
    }

    pub fn stop_range_except(&mut self, lo: i32, hi: i32, a: i32, b: i32) {
        self.stop_where(|t, r| (lo..=hi).contains(&r.id) && t.base(r.id) != a && t.base(r.id) != b);
    }

    pub fn stop_speech(&mut self) {
        self.stop_where(|_, r| is_speech(r.id));
    }

    /// `speaking(U)` (`triggers.md` §1 r8).
    pub fn speaking(&self, unit: UnitKey) -> bool {
        self.requests()
            .any(|r| r.state == RequestState::Playing && is_speech(r.id) && r.units.contains(&unit))
    }

    /// `any_speech` (`triggers.md` §1 r8).
    pub fn any_speech(&self) -> bool {
        self.requests()
            .any(|r| r.state != RequestState::Ended && is_speech(r.id))
    }

    pub fn is_active(&self, h: Handle) -> bool {
        self.request_by_handle(h)
            .is_some_and(|r| r.state != RequestState::Ended)
    }

    /// `roll(n)` on the client seed (§4 r5).
    ///
    /// TODO(spec: audio/sound-table.md §4 r5): with no local player 1.14d
    /// reads a null unit; d2rs returns 0 without a step.
    pub fn roll(world: &mut dyn SoundWorld, n: i32) -> u32 {
        world.client_seed().map_or(0, |s| s.roll(n))
    }

    // ------------------------------------------------------------ §10

    /// Takes a lock on `id`'s group (§10 r4).
    pub fn lock(&mut self, id: i32, delta: i32) -> Result<(), SoundError> {
        Ok(self.table.lock(id, delta)?)
    }

    /// Collects finished async loads. d2rs has no load latency: a load
    /// started in one tick completes at the next tick's cache step.
    ///
    /// TODO(spec: audio/sound-table.md open question 10): the original's
    /// async latency.
    fn collect_loads(&mut self) {
        for id in std::mem::take(&mut self.pending_loads) {
            let sample = self.bank.samples(SoundId(id as u32));
            if let Some(e) = self.table.get_mut(id) {
                match sample {
                    Some(s) => {
                        e.sample = Some(s);
                        e.load = LoadState::Loaded;
                    }
                    None => {
                        e.failed = true;
                        e.load = LoadState::None;
                    }
                }
            }
        }
    }

    fn begin_async(&mut self, id: i32) {
        if let Some(e) = self.table.get_mut(id) {
            if e.load == LoadState::None && !e.failed {
                e.load = LoadState::Pending;
                self.pending_loads.push(id);
            }
        }
    }

    /// Preload (`0x00482B40`, §10 r3). No eviction fails in d2rs (no
    /// cache limit: TODO(spec: audio/sound-table.md §10 r2): eviction
    /// order), so the 250-tick hold never applies.
    ///
    /// TODO(spec: audio/sound-table.md §10 r3): the phase of the 25-tick
    /// period; d2rs runs at the first tick and every 25 after.
    fn preload(&mut self, now: u32) {
        if self
            .last_preload
            .is_some_and(|t| now.wrapping_sub(t) < PRELOAD_PERIOD)
        {
            return;
        }
        self.last_preload = Some(now);
        let wanted: Vec<i32> = self
            .table
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.load == LoadState::None && !e.failed && (e.locks > 0 || e.row.cache != 0)
            })
            .map(|(i, _)| i as i32)
            .collect();
        for id in wanted {
            if self.pending_loads.len() >= MAX_PENDING_LOADS {
                break;
            }
            self.begin_async(id);
        }
    }

    // ------------------------------------------------------------ §6

    /// One sound tick (`0x00482C20`, §6.1): cache work, preload, the
    /// request update at the current tick, channel upkeep, then tick + 1.
    pub fn run_tick(&mut self, world: &mut dyn SoundWorld, queue: &mut TriggerQueue) {
        let now = self.tick;
        self.collect_loads();
        self.preload(now);
        self.update(world, queue, now);
        self.upkeep(queue, now);
        self.tick = now.wrapping_add(1);
    }

    /// §6.2 r1 order: true when `a` is more important than `b`.
    fn more_important(a: &Request, b: &Request) -> bool {
        if a.priority != b.priority {
            return a.priority > b.priority;
        }
        if a.dist2 != b.dist2 {
            return a.dist2 < b.dist2;
        }
        if a.start_tick != b.start_tick {
            return a.start_tick > b.start_tick;
        }
        a.slot > b.slot
    }

    /// Bubble sort of the active list, most important first (§6.2 r1).
    fn sort_active(&mut self) {
        let n = self.active.len();
        for i in 0..n {
            let mut swapped = false;
            for j in 0..n - 1 - i {
                let (a, b) = (self.active[j], self.active[j + 1]);
                if Self::more_important(self.req(b), self.req(a)) {
                    self.active.swap(j, j + 1);
                    swapped = true;
                }
            }
            if !swapped {
                break;
            }
        }
    }

    fn audible(&self, id: i32) -> bool {
        self.settings.master_volume > 0
            && !(self.table.is_song(id) && self.settings.music_volume == 0)
    }

    fn max2(&self, id: i32) -> f32 {
        let (_, max) = falloff(self.table.get(id).map_or(0, |e| e.row.falloff));
        (max as f32) * (max as f32)
    }

    fn free(&mut self, s: usize) {
        self.slots[s] = None;
        self.active.retain(|&a| a != s);
    }

    /// The request update `0x004BA020(now)` (§6.2–§6.5).
    fn update(&mut self, world: &mut dyn SoundWorld, queue: &mut TriggerQueue, now: u32) {
        self.sort_active();
        // §6.2 r2.
        for s in self.active.clone() {
            let r = self.req(s);
            if r.stop && r.fade.is_none_or(|f| f.end != 0) {
                match r.state {
                    RequestState::Waiting => self.req_mut(s).state = RequestState::Ended,
                    RequestState::Playing => self.stop_channel(s, queue, now, "stop"),
                    RequestState::Ended => {}
                }
            }
        }
        let mut removal = Vec::new();
        for s in self.active.clone() {
            if self.slots[s].is_none() {
                continue;
            }
            self.update_one(world, queue, now, s, &mut removal);
        }
        for s in removal {
            if self.slots[s].as_ref().is_some_and(|r| r.channel.is_none()) {
                self.free(s);
            }
        }
        // §6.5.
        if self.last_update != Some(now) {
            let solo = self.requests().any(|r| {
                self.table.get(r.id).is_some_and(|e| e.row.solo != 0)
                    && !r.fade.is_some_and(|f| f.end == 0)
            });
            self.solo_duck = if solo {
                (self.solo_duck - 2).max(SOLO_DUCK_MIN)
            } else {
                (self.solo_duck + 2).min(100)
            };
            self.state_duck = if world.state_duck() {
                (self.state_duck - 5).max(0)
            } else {
                (self.state_duck + 5).min(100)
            };
        }
        self.last_update = Some(now);
    }

    /// §6.3 for one request, in list order. Reading: the rules run in
    /// sequence, so a request restarted by r1 can start in the same pass
    /// (r3), and r5 applies to requests playing before r3.
    fn update_one(
        &mut self,
        world: &mut dyn SoundWorld,
        queue: &mut TriggerQueue,
        now: u32,
        s: usize,
        removal: &mut Vec<usize>,
    ) {
        let id = self.req(s).id;
        let Some(e) = self.table.get(id) else { return };
        let row = e.row.clone();
        // r1. Reading: the pending fade ends with its end volume.
        if self.req(s).state == RequestState::Ended {
            let r = self.req_mut(s);
            if row.looped != 0 && row.duration == 0 && !r.stop {
                r.state = RequestState::Waiting;
                if let Some(f) = r.fade.take() {
                    r.volume = f.end;
                }
            } else {
                self.free(s);
                return;
            }
        }
        // §6.4 r1.
        if self.settings.tracking_option && row.tracking != 0 && !self.req(s).units.is_empty() {
            self.track(world, s);
        }
        // r2.
        let r = self.req_mut(s);
        if let Some(f) = r.fade {
            let (v, ended) = f.volume_at(now);
            r.volume = v;
            if ended {
                r.fade = None;
            }
        }
        let was_playing = r.state == RequestState::Playing;
        let due = now >= r.start_tick && r.volume != 0;
        let audible = self.audible(id);
        let in_range = self.req(s).dist2 <= self.max2(id);
        // r3.
        if self.req(s).state == RequestState::Waiting
            && due
            && audible
            && in_range
            && self.instance_rules(s, &row)
        {
            self.fade_in(s, &row, now);
            if self.start_channel(world, queue, now, s) {
                self.req_mut(s).state = RequestState::Playing;
            }
        }
        // r4.
        let r = self.req(s);
        if due && r.channel.is_none() && row.looped == 0 {
            let loading = row.async_only != 0
                && self
                    .table
                    .get(r.variant)
                    .is_some_and(|e| e.load == LoadState::Pending);
            if !loading {
                removal.push(s);
            }
        }
        // r5.
        if was_playing && self.req(s).state == RequestState::Playing {
            let r = self.req(s);
            let over = row.duration > 0 && now.wrapping_sub(r.start_tick) > u32::from(row.duration);
            if over || !audible || !in_range {
                self.stop_channel(s, queue, now, "end");
            } else {
                let v = r.variant;
                self.send(queue, now, s);
                if let Some(e) = self.table.get_mut(v) {
                    if e.load == LoadState::Loaded {
                        e.last_use = now;
                    }
                }
            }
        }
    }

    /// §6.4 r1: nearest unit's position; occlusion toward the units' mean
    /// by at most 0.05.
    fn track(&mut self, world: &mut dyn SoundWorld, s: usize) {
        let r = self.req(s);
        let id = r.id;
        let mut nearest: Option<([f32; 3], f32)> = None;
        let mut sum = 0.0f32;
        let units = r.units.clone();
        for &u in &units {
            if let Some((p, d2)) = Self::unit_position(world, u) {
                if nearest.is_none_or(|(_, n)| d2 < n) {
                    nearest = Some((p, d2));
                }
            }
            sum += self.unit_occlusion(world, id, u);
        }
        let target = sum / units.len() as f32;
        let r = self.req_mut(s);
        if let Some((p, d2)) = nearest {
            r.pos = p;
            r.dist2 = d2;
        }
        r.occlusion = if r.occlusion < target {
            (r.occlusion + 0.05).min(target)
        } else {
            (r.occlusion - 0.05).max(target)
        };
    }

    /// §6.3 r3.1–r3.2. Returns whether the request may start.
    fn instance_rules(&mut self, s: usize, row: &d2_data::sounds::SoundRow) -> bool {
        let r = self.req(s);
        let id = r.id;
        let unit = (r.units.len() == 1).then(|| r.units[0]);
        let found = if is_speech(id) && unit.is_some() {
            // `0x004B9700`: any playing speech request of that unit.
            self.active.iter().copied().find(|&o| {
                let q = self.req(o);
                o != s
                    && q.state == RequestState::Playing
                    && is_speech(q.id)
                    && q.units.contains(&unit.expect("one unit"))
            })
        } else if row.defer_inst != 0 || row.stop_inst != 0 {
            // `0x004B9690`: same group base, smallest start tick.
            let base = self.table.base(id);
            let mut best: Option<usize> = None;
            for &o in &self.active {
                let q = self.req(o);
                if o == s
                    || q.state != RequestState::Playing
                    || self.table.base(q.id) != base
                    || unit.is_some_and(|u| !q.units.contains(&u))
                {
                    continue;
                }
                if best.is_none_or(|b| q.start_tick < self.req(b).start_tick) {
                    best = Some(o);
                }
            }
            best
        } else {
            None
        };
        let Some(f) = found else { return true };
        let r = self.req(s);
        if row.defer_inst == 0 || !r.units.is_empty() || r.pos == [0.0; 3] {
            if row.stop_inst == 0 {
                self.req_mut(s).stop = true;
                return false;
            }
            self.req_mut(f).stop = true;
        }
        true
    }

    /// §6.3 r3.3.
    fn fade_in(&mut self, s: usize, row: &d2_data::sounds::SoundRow, now: u32) {
        let r = self.req_mut(s);
        let len = if let Some(off) = r.resume_offset.take() {
            r.start_offset = off;
            RESUME_FADE
        } else if row.fade_in > 0 && r.fade.is_none() && r.flags & FLAG_NO_FADE_IN == 0 {
            u32::from(row.fade_in)
        } else {
            return;
        };
        r.fade = Some(Fade {
            start: 0,
            end: r.volume,
            t0: now,
            t1: now.wrapping_add(len),
        });
        r.volume = 0;
    }

    // ------------------------------------------------------------ §7

    /// `0x004E01B0` (§7). Which record a variant start reads:
    /// TODO(spec: audio/sound-table.md §7): after a variant pick, d2rs
    /// reads the variant's record for the sample (load state, `Stream`,
    /// `Async Only`, `Loop`, blocks, `Stereo`) and the requested id's
    /// record for everything else.
    fn start_channel(
        &mut self,
        world: &mut dyn SoundWorld,
        queue: &mut TriggerQueue,
        now: u32,
        s: usize,
    ) -> bool {
        let r = self.req(s);
        let (id, flags, start_tick, priority) = (r.id, r.flags, r.start_tick, r.priority);
        // r1.
        let variant = if flags & FLAG_EXACT != 0 {
            id
        } else {
            self.table.pick_variant(id, &mut |n| Self::roll(world, n))
        };
        self.req_mut(s).variant = variant;
        let dup = self.channels.iter().flatten().any(|c| {
            let q = self.req(c.request);
            c.id == variant
                && !q.stop
                && start_tick.wrapping_sub(q.start_tick) <= 1
                && q.priority >= priority
        });
        if dup {
            return false;
        }
        // r2.
        self.table.record_history(id, variant);
        let Some(e) = self.table.get(variant) else {
            self.errors.push(SoundError::OutOfTable(variant));
            return false;
        };
        let vrow = e.row.clone();
        let block_count = e.block_count;
        if e.failed {
            return false;
        }
        let sample = if vrow.stream != 0 {
            // TODO(spec: audio/sound-table.md §7 r5): what a stream that
            // fails to open does; d2rs treats it as a failed file.
            let s = self.bank.samples(SoundId(variant as u32));
            if s.is_none() {
                self.table.get_mut(variant).expect("in table").failed = true;
            }
            s
        } else if e.load == LoadState::Loaded {
            e.sample.clone()
        } else if vrow.async_only != 0 {
            self.begin_async(variant);
            None
        } else {
            let s = self.bank.samples(SoundId(variant as u32));
            let e = self.table.get_mut(variant).expect("in table");
            match &s {
                Some(x) => {
                    e.sample = Some(x.clone());
                    e.load = LoadState::Loaded;
                }
                None => e.failed = true,
            }
            s
        };
        let Some(sample) = sample else { return false };
        let stereo = vrow.stereo != 0;
        // r3.
        let want = if stereo {
            ChannelKind::Stereo
        } else {
            ChannelKind::Plain
        };
        let fits = |k: ChannelKind| k == ChannelKind::Any || k == want;
        let mut slot = (0..CHANNELS).find(|&c| fits(self.layout[c]) && self.channels[c].is_none());
        if slot.is_none() {
            let mut victim: Option<usize> = None;
            for c in (0..CHANNELS).filter(|&c| fits(self.layout[c])) {
                let Some(ch) = &self.channels[c] else {
                    continue;
                };
                if victim.is_none_or(|v| {
                    let vr = self.req(self.channels[v].as_ref().expect("busy").request);
                    Self::more_important(vr, self.req(ch.request))
                }) {
                    victim = Some(c);
                }
            }
            if let Some(c) = victim {
                let vs = self.channels[c].as_ref().expect("busy").request;
                let v = self.req(vs);
                if priority > v.priority || (priority == v.priority && s > v.slot) {
                    let pos = self.play_position_at(c, now);
                    self.req_mut(vs).resume_offset = Some(pos);
                    self.stop_channel(vs, queue, now, "steal");
                    slot = Some(c);
                }
            }
        }
        let Some(c) = slot else { return false };
        // r4.
        let looped = vrow.looped != 0;
        let block_align = u64::from(sample.channels()) * 2;
        let r = self.req(s);
        let offset_frames = u64::from(r.start_offset) / block_align;
        let (pre, vol, pan) = self.compute(s, stereo);
        let r = self.req(s);
        let cause = format!("sound {id}->{variant} h{}", r.handle);
        let trigger = Trigger {
            tick: now,
            source: TriggerSource::Ui,
            sound: SoundId(variant as u32),
            params: VoiceParams {
                vol,
                pan,
                looped,
                priority: i32::from(r.priority),
                group: self.table.base(variant),
            },
            cause,
        };
        let last_sent = (pre, r.occlusion, r.pos);
        let cue = queue.push(Cue::Start(trigger));
        self.channels[c] = Some(Channel {
            request: s,
            id: variant,
            cue,
            start_tick: now,
            offset_frames,
            frames: sample.frames() as u64,
            rate: u64::from(sample.rate()),
            block_align,
            looped,
            stereo,
            loop_start: (block_count == 1).then(|| (vrow.blocks[0] as u32).wrapping_mul(2)),
            vol,
            pan,
            last_sent,
        });
        self.req_mut(s).channel = Some(c);
        true
    }

    /// Frames played on channel `c` by tick `now` (d2rs's model: elapsed
    /// sound ticks × 40 ms at the file rate).
    ///
    /// TODO(spec: audio/sound-table.md §6.1, §7 r3): when 1.14d's channel
    /// upkeep sees a buffer finish, and the play position it saves on a
    /// steal, come from the device; d2rs derives both from the tick.
    fn played_frames(ch: &Channel, now: u32) -> u64 {
        u64::from(now.wrapping_sub(ch.start_tick)) * TICK_MS * ch.rate / 1000
    }

    /// The play position in bytes saved as a resume offset (§7 r3).
    fn play_position_at(&self, c: usize, now: u32) -> u32 {
        let ch = self.channels[c].as_ref().expect("busy");
        let mut f = ch.offset_frames + Self::played_frames(ch, now);
        if ch.looped && ch.frames > 0 {
            f %= ch.frames;
        } else {
            f = f.min(ch.frames);
        }
        (f * ch.block_align) as u32
    }

    fn stop_channel(&mut self, s: usize, queue: &mut TriggerQueue, now: u32, why: &str) {
        let r = self.req_mut(s);
        r.state = RequestState::Ended;
        let Some(c) = r.channel.take() else { return };
        let handle = r.handle;
        if let Some(ch) = self.channels[c].take() {
            queue.push(Cue::Stop(Stop {
                tick: now,
                target: StopTarget::Voice(ch.cue),
                cause: format!("{why} h{handle}"),
            }));
        }
    }

    /// Channel upkeep: a one-shot channel whose sample has played to its
    /// end frees itself and its request ends (no stop cue: the voice ends
    /// by itself in the mixer).
    fn upkeep(&mut self, _queue: &mut TriggerQueue, now: u32) {
        for c in 0..CHANNELS {
            let Some(ch) = &self.channels[c] else {
                continue;
            };
            if ch.looped || ch.offset_frames + Self::played_frames(ch, now) < ch.frames {
                continue;
            }
            let s = ch.request;
            self.channels[c] = None;
            if let Some(r) = self.slots[s].as_mut() {
                r.channel = None;
                r.state = RequestState::Ended;
            }
        }
    }

    // ------------------------------------------------------------ §8

    /// The §8.2 chain for request `s`: (v after r5, sent volume, pan).
    /// Mixer mode 0 (§9: modes 1–2 are not reproduced; r6 is
    /// [`super::volume::positional_bias`], unused).
    fn compute(&self, s: usize, stereo: bool) -> (i32, i32, i32) {
        let r = self.req(s);
        let Some(e) = self.table.get(r.id) else {
            return (0, 0, 128);
        };
        let exempt = (1..=15).contains(&r.id) || (52..=71).contains(&r.id) || is_music_path(r.id);
        let pre = chain(
            r.volume,
            &self.settings,
            &ChainInputs {
                music_vol: e.row.music_vol != 0,
                state_duck_applies: !exempt,
                solo: e.row.solo != 0,
                state_duck: self.state_duck,
                solo_duck: self.solo_duck,
            },
        );
        let mut v = linear_falloff(pre, r.pos[0], r.pos[1], e.row.falloff);
        v = record_volume(v, e.row.volume);
        let mut pan = 128;
        if !stereo {
            let (gain, p) = mode0_gain_pan(r.pos);
            v = gain.wrapping_mul(v) / 255;
            pan = p;
        }
        (pre, v, pan)
    }

    /// §8.2 r7–r11 for a playing request: send unless unchanged.
    fn send(&mut self, queue: &mut TriggerQueue, now: u32, s: usize) {
        let Some(c) = self.req(s).channel else { return };
        let stereo = self.channels[c].as_ref().is_some_and(|ch| ch.stereo);
        let (pre, vol, pan) = self.compute(s, stereo);
        let r = self.req(s);
        let key = (pre, r.occlusion, r.pos);
        let handle = r.handle;
        let Some(ch) = self.channels[c].as_mut() else {
            return;
        };
        if ch.last_sent == key {
            return;
        }
        ch.last_sent = key;
        ch.vol = vol;
        if !stereo {
            ch.pan = pan;
        }
        queue.push(Cue::Param(ParamChange {
            tick: now,
            target: ch.cue,
            vol,
            pan: ch.pan,
            cause: format!("update h{handle}"),
        }));
    }
}

/// [`SoundSystem`] with a world: the [`SoundCalls`] surface the trigger
/// and environment rules use.
pub struct SoundCtx<'a> {
    pub sys: &'a mut SoundSystem,
    pub world: &'a mut dyn SoundWorld,
}

impl SoundCtx<'_> {
    /// One sound tick (§6.1).
    pub fn run_tick(&mut self, queue: &mut TriggerQueue) {
        self.sys.run_tick(self.world, queue);
    }
}

impl SoundCalls for SoundCtx<'_> {
    fn request(
        &mut self,
        id: i32,
        unit: Option<UnitKey>,
        delay: u32,
        flags: u32,
        offset: u32,
    ) -> Handle {
        self.sys.request(self.world, id, unit, delay, flags, offset)
    }

    fn set_volume(&mut self, h: Handle, v: i32) {
        self.sys.set_volume(h, v);
    }

    fn fade(&mut self, h: Handle, target: i32, delay: u32, len: u32) {
        self.sys.fade(h, target, delay, len);
    }

    fn detach(&mut self, h: Handle, unit: UnitKey, force: bool) {
        self.sys.detach(h, unit, force);
    }

    fn stop_handle(&mut self, h: Handle) {
        self.sys.stop_handle(h);
    }

    fn stop_id(&mut self, id: i32) {
        self.sys.stop_id(id);
    }

    fn stop_songs(&mut self) {
        self.sys.stop_songs();
    }

    fn stop_52_71_except(&mut self, a: i32, b: i32) {
        self.sys.stop_range_except(52, 71, a, b);
    }

    fn stop_72_201_except(&mut self, a: i32, b: i32) {
        self.sys.stop_range_except(72, 201, a, b);
    }

    fn stop_speech(&mut self) {
        self.sys.stop_speech();
    }

    fn speaking(&self, unit: UnitKey) -> bool {
        self.sys.speaking(unit)
    }

    fn any_speech(&self) -> bool {
        self.sys.any_speech()
    }

    fn is_active(&self, h: Handle) -> bool {
        self.sys.is_active(h)
    }

    fn sound_tick(&self) -> u32 {
        self.sys.tick()
    }

    fn roll(&mut self, n: i32) -> u32 {
        SoundSystem::roll(self.world, n)
    }
}

impl SoundSystem {
    /// `0x004B9610(id)`: a request of this id exists.
    pub fn id_requested(&self, id: i32) -> bool {
        self.requests().any(|r| r.id == id)
    }

    /// `0x004B9C60`: a request with an id in 4,657–4,698 is not ended.
    pub fn music_active(&self) -> bool {
        self.requests()
            .any(|r| is_music_path(r.id) && r.state != RequestState::Ended)
    }

    /// `0x004B9D50`: the play position (bytes, d2rs's tick model, see
    /// [`Self::channel_loop_start`]) of the first playing request of `id`
    /// in list order.
    ///
    /// TODO(spec: audio/environment.md open question 2): the units of the
    /// position and whether `id` is the requested id or the variant; d2rs
    /// matches the requested id.
    pub fn play_position(&self, id: i32) -> Option<u32> {
        let r = self
            .requests()
            .find(|r| r.id == id && r.state == RequestState::Playing)?;
        Some(self.play_position_at(r.channel?, self.tick))
    }

    /// `0x004B99A0(h, x, y, z)`: the request's position.
    ///
    /// TODO(spec: audio/sound-table.md §8.1): whether this also updates
    /// distance² (+0x18); d2rs writes the position only.
    pub fn set_position(&mut self, h: Handle, x: i32, y: i32, z: i32) {
        if let Some(s) = self.handle_slot(h) {
            self.req_mut(s).pos = [x as f32, y as f32, z as f32];
        }
    }

    /// The requests attached to `unit` that have not ended, as (handle,
    /// requested id).
    ///
    /// TODO(spec: audio/triggers.md §1 r6): the order of the unit's own
    /// list (+0x78); d2rs uses the active list order.
    pub fn unit_requests(&self, unit: UnitKey) -> Vec<(Handle, i32)> {
        self.requests()
            .filter(|r| r.state != RequestState::Ended && r.units.contains(&unit))
            .map(|r| (r.handle, r.id))
            .collect()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }
}

impl crate::audio::environment::EnvCalls for SoundCtx<'_> {
    fn id_requested(&self, id: i32) -> bool {
        self.sys.id_requested(id)
    }

    fn music_active(&self) -> bool {
        self.sys.music_active()
    }

    fn play_position(&self, id: i32) -> Option<u32> {
        self.sys.play_position(id)
    }

    fn blocks(&self, id: i32) -> [i32; 3] {
        self.sys.table.get(id).map_or([-1; 3], |e| e.row.blocks)
    }

    fn volume(&self, h: Handle) -> Option<i32> {
        self.sys.request_by_handle(h).map(|r| r.volume)
    }

    fn set_position(&mut self, h: Handle, x: i32, y: i32, z: i32) {
        self.sys.set_position(h, x, y, z);
    }
}

impl crate::audio::triggers::TriggerSound for SoundCtx<'_> {
    fn sound_on(&self) -> bool {
        self.sys.enabled
    }

    fn group_base(&self, id: i32) -> i32 {
        self.sys.table.base(id)
    }

    fn looping(&self, id: i32) -> bool {
        self.sys.table.get(id).is_some_and(|e| e.row.looped != 0)
    }

    fn unit_requests(&self, unit: UnitKey) -> Vec<(Handle, i32)> {
        self.sys.unit_requests(unit)
    }

    fn unit_count(&self, h: Handle) -> usize {
        self.sys.request_by_handle(h).map_or(0, |r| r.units.len())
    }

    fn variant(&mut self, id: i32) -> i32 {
        let world = &mut *self.world;
        self.sys
            .table
            .pick_variant(id, &mut |n| SoundSystem::roll(world, n))
    }
}
