// Spec: specs/audio/sound-table.md (§6.1 sound tick, §6.4 r2, §6.5 r2, §8.1, §4 r5), specs/audio/triggers.md (§1 r5, §2 r2–r4, §3, §11), specs/client/bridge.md (§10 r5)
//! The sound layer driven from the client model: [`SoundDriver`] runs one
//! sound tick of the [`SoundSystem`] per server tick the bridge ran
//! (T and C advance once per client update, one per server tick in
//! single player: `triggers.md` §1 r5, `client/model.md` §5 r1), after
//! the frame's trigger requests, and hands every channel cue to the
//! audio core (the core then runs with [`crate::audio::Unlimited`] and
//! presents the sound tick, [`SoundDriver::tick`]).
//!
//! Trigger feeds wired, as one ordered request list ([`SoundRequest`]):
//! the UI sounds (`triggers.md` §11: the click sound of `ui/panels.md`
//! §10.2 and the sounds of the S→C 0x5D / 0x77 UI outputs), the server
//! sound events of the bridge's `ServerSound` outputs (S→C 0x2C,
//! `triggers.md` §2 r2–r4) and the player event sounds the UI asks for
//! (§3). Every other cause class needs input the client model does not
//! hold; each is in [`PENDING`]. The [`SoundWorld`] questions the model
//! cannot answer are not guessed: a request that asks one makes the frame
//! fail ([`DriverError::Pending`]). A rule part whose input is not held
//! (an event's record, a follow-up's owner not wired) is skipped and
//! named in [`SoundDriver::take_skipped`].

use std::cell::RefCell;

use crate::audio::sound_table::{SoundSystem, SoundWorld};
use crate::audio::triggers::events::{player_event, server_event, EventExtra, Followup};
use std::collections::BTreeMap;

use crate::audio::triggers::objects::object_mode;
use crate::audio::triggers::tables::ObjectSounds;
use crate::audio::triggers::{detach_all, ui, Ctx, Globals, TriggerError, Unit, UnitSound};
use crate::audio::{CueSource, TriggerQueue};
use crate::bridge::world::{ClientWorld, LevelRow, UnitKey, MONSTER};
use d2_sim::rng::Seed;

/// Trigger feeds not wired, each with the input it lacks (M02).
pub const PENDING: &[(&str, &str)] = &[
    (
        "server sound events 12, 16 (monsters), 17, 18 (§2 r2)",
        "event 12's `stsound` (open question 4), the `monsounds` rows, the per-unit sound \
         fields and the NPC greeting records are not held by the driver",
    ),
    (
        "event follow-ups: overhead text (`0x004A0200`), quest stingers and the stinger \
         re-arm (§2 r3, §3 r4, r7)",
        "the overhead text (`client/ui.md`) and the stinger machine (`environment.md` §3) \
         are not wired",
    ),
    (
        "mode sounds, footsteps, idle voices (§4–§6)",
        "per-unit animation frame / speed, weapon hit class, states, monsounds rows and the \
         floor material under the unit are not in the client model",
    ),
    (
        "skills, missiles, states, items (§8, §9)",
        "skill / missile / state / item rows and the S→C messages that start them are not \
         handled by the client",
    ),
    ("NPC speech (§10)", "no NPC interaction in the client model"),
    (
        "ambience, rain, music (`environment.md`)",
        "the player's level and its sound environment are known now (`model.md` §11 r5, \
         `environment.md` §1 r2), but every tick of the machines also reads the day phase \
         (`environment.md` open question 3: the act environment, S→C 0x53 unhandled) and the \
         weather (open question 4, `draw-order-2.md` §11 state not in the model)",
    ),
];

/// A [`SoundWorld`] question the model cannot answer yet, or a fatal
/// path of the original.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DriverError {
    #[error("sound world: {0} (pending: not in the client model)")]
    Pending(&'static str),
    #[error(transparent)]
    Trigger(#[from] TriggerError),
}

/// One sound request of a frame, in the order 1.14d makes the calls
/// (`client/bridge.md` §10 rules 2, 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundRequest {
    /// A UI sound: request(id, none), delay 0 (§11).
    Ui(i32),
    /// A `ServerSound` output (S→C 0x2C, §2 r4): the event unit's key,
    /// class and position captured at receive, and the event. The
    /// position is used when the key no longer resolves at delivery
    /// (`client/bridge.md` §10 r3.1 (b)).
    Server {
        unit: UnitKey,
        class: u32,
        at: Option<(u16, u16)>,
        event: u16,
    },
    /// A `UnitFreed` output (`client/bridge.md` §10 r3.1 (a)): the unit
    /// free's detach of every request of the unit without force
    /// (`audio/triggers-2.md` §19 r5); the sample-lock release
    /// `0x004CC160(U, −1)` is cache only (`sound-table.md` §10 r4).
    UnitFreed { unit: UnitKey },
    /// A player event sound `0x004CB9C0(unit, event)` (§3) the UI asked
    /// for (S→C 0x77 code 9 on the local player, `client/msg-ui.md` §3),
    /// or the client object code (`client/model.md` §8 rule 7).
    PlayerEvent { unit: UnitKey, event: u16 },
    /// The object mode sound call `0x004CB460(U)` (§7) of an
    /// `ObjectSound::Mode` output: U's key (`client_only`: in set C), its
    /// class, mode and path distance to P at the call
    /// (`audio/triggers-2.md` §20).
    ObjectMode {
        unit: UnitKey,
        client_only: bool,
        class: u32,
        mode: u32,
        local_dist: i32,
    },
    /// A sound request `0x004B9A00(id, U, 0, 0, 0)` (§1 r1): the client
    /// object functions (`world/objects-client.md` §26.18) and the shrine
    /// sound of 0x4D (`client/model.md` §15 rule 4 step 4).
    UnitRequest { id: i32, unit: UnitKey },
}

/// A rule part the driver skipped (its input is not held), named.
pub const SKIP_EVENT_12: &str = "S→C 0x2C event 12: the skill `stsound` (triggers.md OQ 4)";
pub const SKIP_EVENT_16: &str = "S→C 0x2C event 16 on a monster: its `monsounds` record";
pub const SKIP_EVENT_17: &str = "S→C 0x2C event 17: the monster flee voice (§6 r4)";
pub const SKIP_EVENT_18: &str = "S→C 0x2C event 18: the NPC greeting record (§10 r1)";
pub const SKIP_OVERHEAD: &str = "overhead text 0x004A0200 (client/ui.md)";
pub const SKIP_STINGER: &str = "quest stinger line (environment.md §3)";
pub const SKIP_REARM: &str = "stinger speech re-arm 0x004DCE10 (environment.md §3 r5)";
pub const SKIP_NO_UNIT: &str = "player event sound: the unit is not in the model";

/// The sound layer's view of the client model (`sound-table.md` §8.1,
/// §6.4 r2, §6.5 r2, §4 r5). Questions it cannot answer are recorded in
/// `asked` and answered neutrally; [`SoundDriver::frame`] then fails.
pub struct ModelSoundWorld<'a> {
    pub world: &'a ClientWorld,
    /// The `Levels.txt` rows by level id (`SoundEnv`).
    pub levels: &'a [LevelRow],
    /// `Indoors` of each `soundenviron` row (`sound-table.md` §2).
    pub env_indoors: &'a [u8],
    /// Pending questions asked, in order.
    pub asked: RefCell<Vec<&'static str>>,
    /// The positions `ServerSound` outputs captured, by unit, for a unit
    /// no longer in the model at delivery (`client/bridge.md` §10 r3.1
    /// (b)); the latest capture of a unit wins.
    pub captured: Option<&'a RefCell<BTreeMap<UnitKey, (i32, i32)>>>,
}

impl<'a> ModelSoundWorld<'a> {
    /// A view without level or environment rows.
    pub fn new(world: &'a ClientWorld) -> Self {
        Self::with_env(world, &[], &[])
    }

    pub fn with_env(world: &'a ClientWorld, levels: &'a [LevelRow], env_indoors: &'a [u8]) -> Self {
        Self {
            world,
            levels,
            env_indoors,
            asked: RefCell::new(Vec::new()),
            captured: None,
        }
    }

    fn ask(&self, q: &'static str) {
        self.asked.borrow_mut().push(q);
    }
}

impl SoundWorld for ModelSoundWorld<'_> {
    fn local_player(&self) -> Option<UnitKey> {
        self.world.local_player
    }

    /// The model's cell of the unit (`client/model.md` §1 r2); a unit no
    /// longer in the model: the position its `ServerSound` captured
    /// (`client/bridge.md` §10 r3.1 (b)). The units per type are
    /// `sound-table.md` open question 2.
    fn position(&self, unit: UnitKey) -> Option<(i32, i32)> {
        match self.world.units.get(&unit) {
            Some(u) => {
                let (x, y) = u.position?;
                Some((i32::from(x), i32::from(y)))
            }
            None => self.captured?.borrow().get(&unit).copied(),
        }
    }

    /// `0x00622AA0(player, unit, 2)` needs the client collision rooms (no
    /// client DRLG): pending.
    fn blocked(&self, _unit: UnitKey) -> bool {
        self.ask("line test 0x00622AA0 (§6.4 r2): no client collision rooms");
        false
    }

    /// `Indoors` of the current sound environment: the `soundenviron`
    /// row `SoundEnv` of the local player's level (`model.md` §11 r5,
    /// `audio/environment.md` §1 r2; a row outside the table reads 0).
    /// With no level (no local player or no room) the current environment
    /// is not stated: pending.
    fn indoors(&self) -> bool {
        let Some(level) = self.world.player_level() else {
            self.ask("sound environment Indoors (§6.4 r2): no player level");
            return false;
        };
        let Some(row) = self.levels.get(usize::from(level)) else {
            self.ask("sound environment Indoors (§6.4 r2): no Levels row for the player's level");
            return false;
        };
        self.env_indoors
            .get(usize::from(row.sound_env))
            .is_some_and(|&v| v != 0)
    }

    /// The `0x004BA640` condition (open question 6: a game state, likely
    /// a menu or pause). d2rs has neither a game menu nor a pause, so the
    /// condition cannot hold (reading listed in the handoff).
    fn state_duck(&self) -> bool {
        false
    }

    /// The local player's client seed (player + 0x20) is the model's,
    /// shared with the camera shake (`camera.md` open question 6) and
    /// read-only to the audio frame: pending.
    fn client_seed(&mut self) -> Option<&mut Seed> {
        self.ask("local player client seed (§4 r5): read-only model seed");
        None
    }
}

/// The sound system, its trigger globals and the cues not yet handed to
/// the audio core.
pub struct SoundDriver {
    system: SoundSystem,
    /// `Indoors` of each `soundenviron` row, copied from the table.
    env_indoors: Vec<u8>,
    globals: Globals,
    cues: TriggerQueue,
    /// The last server tick a sound tick ran for.
    last_server_tick: Option<u64>,
    /// Rule parts skipped since the last [`SoundDriver::take_skipped`].
    skipped: Vec<&'static str>,
    /// The per-unit sound fields +0x70 … +0x88 (`client/model.md` §18
    /// rule 1: zero at creation, written only by the audio rules), by
    /// (unit, in set C); a unit gone from the model drops its fields
    /// (§18 rule 2, the unit free).
    unit_sounds: BTreeMap<(UnitKey, bool), UnitSound>,
}

impl SoundDriver {
    pub fn new(system: SoundSystem) -> Self {
        Self {
            env_indoors: system.table().env_indoors().to_vec(),
            system,
            globals: Globals::sound_init(),
            cues: TriggerQueue::new(),
            last_server_tick: None,
            skipped: Vec::new(),
            unit_sounds: BTreeMap::new(),
        }
    }

    pub fn system(&self) -> &SoundSystem {
        &self.system
    }

    /// The sound tick T (ticks run so far): what the core presents.
    pub fn tick(&self) -> u32 {
        self.system.tick()
    }

    /// One audio frame: the frame's sound requests in order (UI sounds,
    /// server sound events, player event sounds), then one sound tick per
    /// server tick since the last frame (none before the first server
    /// tick). A pending [`SoundWorld`] question fails the frame after the
    /// tick that asked it. `levels` are the `Levels.txt` rows by level id
    /// (`SoundEnv`). P is the local player now (§2 r4: at delivery).
    pub fn frame(
        &mut self,
        world: &ClientWorld,
        levels: &[LevelRow],
        requests: &[SoundRequest],
    ) -> Result<(), DriverError> {
        let now = world.server_ticks;
        let ticks = match self.last_server_tick {
            _ if now == 0 => 0,
            None => 1,
            Some(last) => now.saturating_sub(last),
        };
        self.unit_sounds.retain(|&(k, c), _| {
            if c {
                world.objclient.set_c.contains_key(&k)
            } else {
                world.units.contains_key(&k)
            }
        });
        let captured = RefCell::new(BTreeMap::new());
        let mut sw = ModelSoundWorld::with_env(world, levels, &self.env_indoors);
        sw.captured = Some(&captured);
        if !requests.is_empty() {
            // C: one client update per server tick (§1 r5).
            let c = now as u32;
            let mut ctx = self.system.with(&mut sw);
            let mut cx = Ctx::new(&mut ctx, &mut self.globals, c);
            for r in requests {
                if let SoundRequest::Server {
                    unit, at: Some(at), ..
                } = *r
                {
                    let at = (i32::from(at.0), i32::from(at.1));
                    captured.borrow_mut().insert(unit, at);
                }
                request(&mut cx, world, r, &mut self.unit_sounds, &mut self.skipped)?;
            }
        }
        for _ in 0..ticks {
            self.system.run_tick(&mut sw, &mut self.cues);
        }
        if ticks > 0 {
            self.last_server_tick = Some(now);
        }
        if let Some(&q) = sw.asked.borrow().first() {
            return Err(DriverError::Pending(q));
        }
        Ok(())
    }

    /// The rule parts skipped since the last call, in order.
    pub fn take_skipped(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.skipped)
    }

    /// Errors the sound system recorded (table lookups, locks).
    pub fn take_errors(&mut self) -> Vec<crate::audio::sound_table::SoundError> {
        self.system.take_errors()
    }
}

/// The unit as the event rules read it: key and class (captured at
/// receive for 0x2C), whether it is P, and P's mode when it is (§3 r2's
/// dead check reads it; P is read at delivery).
fn event_unit(world: &ClientWorld, key: UnitKey, class: u32) -> Unit<'static> {
    let mut u = Unit::new(key, class as i32);
    u.is_local = world.local_player == Some(key);
    if u.is_local {
        if let Some(p) = world.units.get(&key) {
            u.mode = p.mode as u8;
        }
    }
    u
}

fn followups(out: Vec<Followup>, skipped: &mut Vec<&'static str>) {
    for f in out {
        skipped.push(match f {
            Followup::OverheadText(_) => SKIP_OVERHEAD,
            Followup::QuestStinger { .. } => SKIP_STINGER,
            Followup::StingerRearm => SKIP_REARM,
        });
    }
}

/// One request (§11, §2 r2–r4, §3).
fn request(
    cx: &mut Ctx,
    world: &ClientWorld,
    r: &SoundRequest,
    unit_sounds: &mut BTreeMap<(UnitKey, bool), UnitSound>,
    skipped: &mut Vec<&'static str>,
) -> Result<(), DriverError> {
    match *r {
        SoundRequest::Ui(id) => ui::ui_sound(cx, id),
        SoundRequest::ObjectMode {
            unit,
            client_only,
            class,
            mode,
            local_dist,
        } => {
            let mut u = Unit::new(unit, class as i32);
            u.mode = u8::try_from(mode).unwrap_or(u8::MAX);
            u.local_dist = local_dist;
            let us = unit_sounds.entry((unit, client_only)).or_default();
            object_mode(cx, ObjectSounds::spec(), &u, us)?;
        }
        SoundRequest::UnitRequest { id, unit } => {
            cx.unit_request(id, unit);
        }
        SoundRequest::UnitFreed { unit } => detach_all(cx.s, unit, false),
        SoundRequest::Server {
            unit, class, event, ..
        } => {
            let u = event_unit(world, unit, class);
            let skip = match event {
                12 => Some(SKIP_EVENT_12),
                16 if unit.unit_type == MONSTER => Some(SKIP_EVENT_16),
                17 => Some(SKIP_EVENT_17),
                18 => Some(SKIP_EVENT_18),
                _ => None,
            };
            if let Some(s) = skip {
                skipped.push(s);
                return Ok(());
            }
            let extra = EventExtra {
                local: world.local_player,
                event12_stsound: 0,
                greeting: None,
                day_phase: 0,
            };
            let out = server_event(cx, &u, &mut UnitSound::default(), event, extra)?;
            followups(out, skipped);
        }
        SoundRequest::PlayerEvent { unit, event } => {
            let Some(p) = world.units.get(&unit) else {
                skipped.push(SKIP_NO_UNIT);
                return Ok(());
            };
            let u = event_unit(world, unit, p.class);
            let out = player_event(cx, &u, event)?;
            followups(out, skipped);
        }
    }
    Ok(())
}

impl CueSource for SoundDriver {
    fn drain_cues(&mut self, queue: &mut TriggerQueue) {
        for (_, cue) in self.cues.take_due(u32::MAX) {
            queue.push(cue);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::audio::sound_table::SoundTableData;
    use crate::audio::{Sound, SoundBank, SoundId};
    use crate::bridge::world::{ClientUnit, MONSTER, PLAYER};

    struct Bank;

    impl SoundBank for Bank {
        fn file(&self, id: SoundId) -> Option<Arc<str>> {
            Some(format!("s{}.wav", id.0).into())
        }
        fn samples(&self, _: SoundId) -> Option<Arc<Sound>> {
            Some(Arc::new(Sound::new(1_000, 1, vec![0; 1_000]).unwrap()))
        }
    }

    /// Ids 0–3: `b` (2) heads a group of 2 (variants 2, 3).
    fn driver() -> SoundDriver {
        let s = "Sound\tIndex\tFileName\tVolume\tGroup Size\tBlock 1\tBlock 2\tBlock 3\r\n\
                 none\t0\tnone.wav\t0\t0\t-1\t-1\t-1\r\n\
                 a\t1\ta.wav\t255\t0\t-1\t-1\t-1\r\n\
                 b\t2\tb.wav\t255\t2\t-1\t-1\t-1\r\n\
                 c\t3\tc.wav\t255\t0\t-1\t-1\t-1\r\n";
        let e = "Handle\tIndex\tSong\r\nx\t0\t0\r\n";
        let st = d2_data::txt::TxtTable::parse("sounds.txt", s.as_bytes()).unwrap();
        let et = d2_data::txt::TxtTable::parse("soundenviron.txt", e.as_bytes()).unwrap();
        let t = SoundTableData::from_txt(&st, &et).unwrap();
        SoundDriver::new(SoundSystem::new(t, Box::new(Bank)))
    }

    fn at_tick(n: u64) -> ClientWorld {
        ClientWorld {
            server_ticks: n,
            ..ClientWorld::default()
        }
    }

    // Covers: specs/audio/sound-table.md §6.1; specs/audio/triggers.md §1 r5
    #[test]
    fn one_sound_tick_per_server_tick() {
        let mut d = driver();
        d.frame(&at_tick(0), &[], &[]).unwrap();
        assert_eq!(d.tick(), 0, "no server tick, no sound tick");
        d.frame(&at_tick(1), &[], &[]).unwrap();
        assert_eq!(d.tick(), 1);
        d.frame(&at_tick(1), &[], &[]).unwrap();
        assert_eq!(d.tick(), 1, "a frame without a server tick");
        d.frame(&at_tick(4), &[], &[]).unwrap();
        assert_eq!(d.tick(), 4);
    }

    // Covers: specs/audio/triggers.md §11
    #[test]
    fn ui_sounds_are_requested_and_their_cues_reach_the_core() {
        let mut d = driver();
        d.frame(&at_tick(1), &[], &[SoundRequest::Ui(1)]).unwrap();
        let mut q = TriggerQueue::new();
        d.drain_cues(&mut q);
        assert!(!q.is_empty(), "the request's channel start is a cue");
        let mut again = TriggerQueue::new();
        d.drain_cues(&mut again);
        assert!(again.is_empty(), "cues are handed over once");
    }

    // Covers: specs/audio/triggers.md §2 r2, §2 r3, §2 r4, §3 r5; specs/client/bridge.md §10 r2
    #[test]
    fn server_sounds_and_ui_sounds_run_in_list_order() {
        use crate::audio::sound_table::SoundError;
        use crate::bridge::world::OBJECT;
        let mut d = driver();
        let mut w = at_tick(1);
        let p = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(p);
        u.mode = 1;
        w.units.insert(p, u);
        w.local_player = Some(p);
        let obj = UnitKey::new(OBJECT, 9);
        let requests = [
            // Event 13 on an object: 2,634 on it.
            SoundRequest::Server {
                unit: obj,
                class: 5,
                at: None,
                event: 13,
            },
            SoundRequest::Ui(5000),
            // Event 2 on the local player: the player event sound (§3),
            // id 7, and its overhead text (r7, not wired).
            SoundRequest::Server {
                unit: p,
                class: 0,
                at: None,
                event: 2,
            },
            // Events whose record the driver does not hold.
            SoundRequest::Server {
                unit: obj,
                class: 5,
                at: None,
                event: 12,
            },
            SoundRequest::Server {
                unit: UnitKey::new(MONSTER, 3),
                class: 5,
                at: None,
                event: 18,
            },
            SoundRequest::PlayerEvent {
                unit: UnitKey::new(PLAYER, 77),
                event: 23,
            },
        ];
        d.frame(&w, &[], &requests).unwrap();
        // The ids are outside the 4-row test table: each request is
        // reported in order (`sound-table.md` §1 r4).
        assert_eq!(
            d.take_errors(),
            [
                SoundError::OutOfTable(2634),
                SoundError::OutOfTable(5000),
                SoundError::OutOfTable(7)
            ]
        );
        assert_eq!(
            d.take_skipped(),
            [SKIP_OVERHEAD, SKIP_EVENT_12, SKIP_EVENT_18, SKIP_NO_UNIT]
        );
        // The dead local player makes no player event sound (§3 r2).
        w.units.get_mut(&p).unwrap().mode = 17;
        d.frame(&w, &[], &[SoundRequest::PlayerEvent { unit: p, event: 2 }])
            .unwrap();
        assert!(d.take_errors().is_empty() && d.take_skipped().is_empty());
        // A player class outside the record table is fatal (§3 r1).
        let bad = SoundRequest::Server {
            unit: UnitKey::new(PLAYER, 2),
            class: 9,
            at: None,
            event: 2,
        };
        assert_eq!(
            d.frame(&w, &[], &[bad]),
            Err(DriverError::Trigger(TriggerError::PlayerClass(9)))
        );
    }

    // Covers: specs/client/bridge.md §10 r3; specs/audio/triggers-2.md §19 r5
    #[test]
    fn a_unit_freed_output_detaches_the_units_requests() {
        let mut d = driver();
        let mut w = at_tick(1);
        let m = UnitKey::new(MONSTER, 4);
        let mut u = ClientUnit::new(m);
        u.position = Some((100, 200));
        w.units.insert(m, u);
        // The request is made; its tick's line test needs the client
        // collision rooms (pending, not part of this rule).
        assert!(matches!(
            d.frame(&w, &[], &[SoundRequest::UnitRequest { id: 1, unit: m }]),
            Err(DriverError::Pending(_))
        ));
        assert_eq!(d.system().unit_requests(m).len(), 1);
        w.units.remove(&m);
        w.server_ticks = 2;
        d.frame(&w, &[], &[SoundRequest::UnitFreed { unit: m }])
            .unwrap();
        assert!(d.system().unit_requests(m).is_empty());
    }

    // Covers: specs/client/bridge.md §10 r3
    #[test]
    fn a_freed_units_position_is_the_one_its_sound_captured() {
        let w = ClientWorld::default();
        let gone = UnitKey::new(MONSTER, 4);
        let captured = RefCell::new(BTreeMap::from([(gone, (100, 200))]));
        let mut sw = ModelSoundWorld::new(&w);
        assert_eq!(sw.position(gone), None);
        sw.captured = Some(&captured);
        assert_eq!(sw.position(gone), Some((100, 200)));
        assert_eq!(sw.position(UnitKey::new(MONSTER, 5)), None);
    }

    // Covers: specs/audio/sound-table.md §4 r5
    #[test]
    fn a_question_the_model_cannot_answer_fails_the_frame() {
        let mut d = driver();
        // Id 2 heads a group: the variant roll needs the client seed (a
        // local player exists: a draw without one is an internal error,
        // `sound-table.md` §4 r6).
        let mut w = at_tick(1);
        w.local_player = Some(UnitKey::new(PLAYER, 1));
        assert_eq!(
            d.frame(&w, &[], &[SoundRequest::Ui(2)]),
            Err(DriverError::Pending(
                "local player client seed (§4 r5): read-only model seed"
            ))
        );
    }

    // Covers: specs/audio/sound-table.md §8.1 r1, §6.4 r2, §6.5 r2
    #[test]
    fn the_model_sound_world() {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 7);
        let mut u = ClientUnit::new(key);
        u.position = Some((0x1241, 0x11C4));
        w.units.insert(key, u);
        w.local_player = Some(key);
        let mut sw = ModelSoundWorld::new(&w);
        assert_eq!(sw.local_player(), Some(key));
        assert_eq!(sw.position(key), Some((0x1241, 0x11C4)));
        assert_eq!(sw.position(UnitKey::new(PLAYER, 8)), None);
        assert!(!sw.state_duck());
        assert!(sw.asked.borrow().is_empty());
        assert!(!sw.blocked(key) && !sw.indoors() && sw.client_seed().is_none());
        assert_eq!(sw.asked.borrow().len(), 3);
        assert!(PENDING.len() >= 6);
    }

    // Covers: specs/audio/sound-table.md §6.4 r2; specs/audio/environment.md §1 r2
    #[test]
    fn indoors_is_the_player_levels_environment() {
        use crate::bridge::drlg::DrlgRoomId;
        use crate::bridge::world::ActiveRoom;
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 7);
        let mut u = ClientUnit::new(key);
        u.position = Some((45, 5));
        w.units.insert(key, u);
        w.local_player = Some(key);
        // The player is in the room's unit list (`sim/unit-order.md` §5
        // rule 6: the client's only unit → room link).
        w.room_units.place(key, Some(DrlgRoomId(0)));
        let room = |level| ActiveRoom {
            x0: 40,
            y0: 0,
            w: 40,
            h: 40,
            level,
            room: DrlgRoomId(0),
        };
        // Level 1: SoundEnv 1 (Indoors 1); level 2: SoundEnv 0 (Indoors
        // 0); level 3: SoundEnv 9, outside the two rows (reads 0).
        let levels = [
            LevelRow::default(),
            LevelRow {
                sound_env: 1,
                ..LevelRow::default()
            },
            LevelRow::default(),
            LevelRow {
                sound_env: 9,
                ..LevelRow::default()
            },
        ];
        let env = [0u8, 1];
        for (level, indoors) in [(1, true), (2, false), (3, false)] {
            w.active_rooms = Some(vec![room(level)]);
            let sw = ModelSoundWorld::with_env(&w, &levels, &env);
            assert_eq!(sw.indoors(), indoors, "level {level}");
            assert!(sw.asked.borrow().is_empty());
        }
        // A level past the Levels rows: pending, not guessed.
        w.active_rooms = Some(vec![room(40)]);
        let sw = ModelSoundWorld::with_env(&w, &levels, &env);
        assert!(!sw.indoors());
        assert_eq!(sw.asked.borrow().len(), 1);
    }
}
