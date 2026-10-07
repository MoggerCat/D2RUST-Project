// Spec: specs/audio/sound-table.md (§6.1 sound tick, §6.4 r2, §6.5 r2, §8.1, §4 r5), specs/audio/triggers.md (§1 r5, §11)
//! The sound layer driven from the client model: [`SoundDriver`] runs one
//! sound tick of the [`SoundSystem`] per server tick the bridge ran
//! (T and C advance once per client update, one per server tick in
//! single player: `triggers.md` §1 r5, `client/model.md` §5 r1), after
//! the frame's trigger requests, and hands every channel cue to the
//! audio core (the core then runs with [`crate::audio::Unlimited`] and
//! presents the sound tick, [`SoundDriver::tick`]).
//!
//! Trigger feeds wired: the UI sounds (`triggers.md` §11: the click
//! sound of `ui/panels.md` §10.2). Every other cause class needs input
//! the client model does not hold; each is in [`PENDING`]. The
//! [`SoundWorld`] questions the model cannot answer are not guessed: a
//! request that asks one makes the frame fail ([`DriverError::Pending`]).

use std::cell::RefCell;

use crate::audio::sound_table::{SoundSystem, SoundWorld};
use crate::audio::triggers::{ui, Ctx, Globals};
use crate::audio::{CueSource, TriggerQueue};
use crate::bridge::world::{ClientWorld, UnitKey};
use d2_sim::rng::Seed;

/// Trigger feeds not wired, each with the input it lacks (M02).
pub const PENDING: &[(&str, &str)] = &[
    (
        "server sound events, S→C 0x2C (§2) and player event sounds (§3)",
        "S→C 0x2C has no client handler (`client/msg-*` specs)",
    ),
    (
        "mode sounds, footsteps, idle voices (§4–§6)",
        "per-unit animation frame / speed, weapon hit class, states, monsounds rows and the \
         floor material under the unit are not in the client model",
    ),
    (
        "object mode sounds (§7)",
        "object mode changes are not reported by the bridge as events; distance needs the \
         client unit positions per type (`sound-table.md` open question 2)",
    ),
    (
        "skills, missiles, states, items (§8, §9)",
        "skill / missile / state / item rows and the S→C messages that start them are not \
         handled by the client",
    ),
    ("NPC speech (§10)", "no NPC interaction in the client model"),
    (
        "S→C 0x5D sound actions (§11)",
        "S→C 0x5D has no client handler",
    ),
    (
        "ambience, rain, music (`environment.md`)",
        "the sound environment of the player's level (no client DRLG, so no level), the day \
         phase and weather (`render/lighting.md`, `draw-order-2.md` §11 state not in the model)",
    ),
];

/// A [`SoundWorld`] question the model cannot answer yet.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DriverError {
    #[error("sound world: {0} (pending: not in the client model)")]
    Pending(&'static str),
}

/// The sound layer's view of the client model (`sound-table.md` §8.1,
/// §6.4 r2, §6.5 r2, §4 r5). Questions it cannot answer are recorded in
/// `asked` and answered neutrally; [`SoundDriver::frame`] then fails.
pub struct ModelSoundWorld<'a> {
    pub world: &'a ClientWorld,
    /// Pending questions asked, in order.
    pub asked: RefCell<Vec<&'static str>>,
}

impl<'a> ModelSoundWorld<'a> {
    pub fn new(world: &'a ClientWorld) -> Self {
        Self {
            world,
            asked: RefCell::new(Vec::new()),
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

    /// The model's cell of the unit (`client/model.md` §1 r2). The units
    /// per type are `sound-table.md` open question 2.
    fn position(&self, unit: UnitKey) -> Option<(i32, i32)> {
        let (x, y) = self.world.units.get(&unit)?.position?;
        Some((i32::from(x), i32::from(y)))
    }

    /// `0x00622AA0(player, unit, 2)` needs the client collision rooms (no
    /// client DRLG): pending.
    fn blocked(&self, _unit: UnitKey) -> bool {
        self.ask("line test 0x00622AA0 (§6.4 r2): no client collision rooms");
        false
    }

    /// `Indoors` of the sound environment of the player's level: no level
    /// without the client DRLG: pending.
    fn indoors(&self) -> bool {
        self.ask("sound environment Indoors (§6.4 r2): no player level");
        false
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
    globals: Globals,
    cues: TriggerQueue,
    /// The last server tick a sound tick ran for.
    last_server_tick: Option<u64>,
}

impl SoundDriver {
    pub fn new(system: SoundSystem) -> Self {
        Self {
            system,
            globals: Globals::default(),
            cues: TriggerQueue::new(),
            last_server_tick: None,
        }
    }

    pub fn system(&self) -> &SoundSystem {
        &self.system
    }

    /// The sound tick T (ticks run so far): what the core presents.
    pub fn tick(&self) -> u32 {
        self.system.tick()
    }

    /// One audio frame: the UI sound requests (`triggers.md` §11), then
    /// one sound tick per server tick since the last frame (none before
    /// the first server tick). A pending [`SoundWorld`] question fails
    /// the frame after the tick that asked it.
    pub fn frame(&mut self, world: &ClientWorld, ui_sounds: &[i32]) -> Result<(), DriverError> {
        let now = world.server_ticks;
        let ticks = match self.last_server_tick {
            _ if now == 0 => 0,
            None => 1,
            Some(last) => now.saturating_sub(last),
        };
        let mut sw = ModelSoundWorld::new(world);
        if !ui_sounds.is_empty() {
            // C: one client update per server tick (§1 r5).
            let c = now as u32;
            let mut ctx = self.system.with(&mut sw);
            let mut cx = Ctx::new(&mut ctx, &mut self.globals, c);
            for &id in ui_sounds {
                ui::ui_sound(&mut cx, id);
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

    /// Errors the sound system recorded (table lookups, locks).
    pub fn take_errors(&mut self) -> Vec<crate::audio::sound_table::SoundError> {
        self.system.take_errors()
    }
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
    use crate::bridge::world::{ClientUnit, PLAYER};

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
        d.frame(&at_tick(0), &[]).unwrap();
        assert_eq!(d.tick(), 0, "no server tick, no sound tick");
        d.frame(&at_tick(1), &[]).unwrap();
        assert_eq!(d.tick(), 1);
        d.frame(&at_tick(1), &[]).unwrap();
        assert_eq!(d.tick(), 1, "a frame without a server tick");
        d.frame(&at_tick(4), &[]).unwrap();
        assert_eq!(d.tick(), 4);
    }

    // Covers: specs/audio/triggers.md §11
    #[test]
    fn ui_sounds_are_requested_and_their_cues_reach_the_core() {
        let mut d = driver();
        d.frame(&at_tick(1), &[1]).unwrap();
        let mut q = TriggerQueue::new();
        d.drain_cues(&mut q);
        assert!(!q.is_empty(), "the request's channel start is a cue");
        let mut again = TriggerQueue::new();
        d.drain_cues(&mut again);
        assert!(again.is_empty(), "cues are handed over once");
    }

    // Covers: specs/audio/sound-table.md §4 r5
    #[test]
    fn a_question_the_model_cannot_answer_fails_the_frame() {
        let mut d = driver();
        // Id 2 heads a group: the variant roll needs the client seed.
        assert_eq!(
            d.frame(&at_tick(1), &[2]),
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
        assert!(PENDING.len() >= 7);
    }
}
