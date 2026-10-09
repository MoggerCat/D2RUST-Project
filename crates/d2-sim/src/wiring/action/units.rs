// Spec: specs/formats/d2s.md §2.3; specs/combat/vitals.md §4.8 r1, §4.8 r2; specs/sim/units.md §3, §4.1, §4.3, §4.5, §4.6, §5, §6; specs/monsters/init.md §5, §22; specs/monsters/umod-callbacks.md §2; specs/formats/animdata.md §3–§5; specs/sim/stat-lists.md §4, §8, §9; specs/monsters/ai.md §1; specs/missiles/missiles.md §R3
//! The unit side of the wiring: the unit hooks of [`ActionHooks`] (the
//! missile class handler for missile events, the AI think and reset for
//! monster events 2 and 10, the state-54 rule before a think is
//! scheduled, the town test, the combat list drop, the kind frees, the
//! skill events 5 / 8 / 9 through [`Pending::skill_event`], the player
//! action frame through [`Pending::action_frame`], the AnimData record
//! of a unit's mode (`formats/animdata.md` §5) and the monster death
//! start through [`Pending::monster_death_start`]; with a lent monster
//! world ([`super::monsters`]): the monster type init of the allocator,
//! the monster state's part of a free, event 7 and the mode change's
//! umod callbacks), and
//! the unit-field helpers of [`View`] the other adapters share (stats,
//! states, state lists, seeds).

use crate::game::Game;
use crate::missiles;
use crate::monsters::ai;
use crate::rng::Seed;
use crate::stats::lists::RemoveCallback;
use crate::stats::states::state;
use crate::stats::{ListId, StatHost, StatLists};
use crate::tick::events::event;
use crate::units::hooks::{Sim, UnitHooks};
use crate::units::lifecycle::{AllocRequest, LifecycleHooks};
use crate::units::modes::UnitError;
use crate::units::modes::MONSTER_MODES;
use crate::units::record::{flags2, AnimRecord, Sequence, ANIM_EVENTS};
use crate::units::{UnitId, UnitType};

use super::combat::HIRELING_CLASSES;
use super::monsters::umod_mode;
use super::{ActionHooks, Pending, SkillEvent, View, WiringError};

/// The client status word's dead bit (`formats/d2s.md` §2.3).
pub const STATUS_DEAD: u16 = 0x08;
/// The spread of the mercenary's creation `0x005B23C0(…, 4, 0)`
/// (`npc.md` §7.3 step 7).
const HIRE_SPREAD: i32 = 4;

/// Stat-list state of `justhit` (`missiles.md` §R5 step 6.1).
pub const STATE_JUSTHIT: u16 = 86;
/// State 92 (`death_delay`), cleared for players by `0x005544B0`.
pub const STATE_DEATH_DELAY: u16 = 92;

impl<X: Pending> StatHost for ActionHooks<X> {
    /// §8.2 rule 6: queue the callbacks this wiring runs after the expiry
    /// walk ([`UnitHooks::lists_expired`]): the default one, the shrine
    /// ones, and Inferno's / Blade Fury's (`skills/bodies.md` §6.16,
    /// `bodies-2b.md` §6.16: state off, unit flags |= 0x40; the timer 12
    /// expiry is what ends a player's Inferno / Arctic Blast channel). The
    /// others are run by their skill bodies.
    fn list_removed(
        &mut self,
        _lists: &mut StatLists,
        unit: UnitId,
        state: u32,
        _list: ListId,
        callback: RemoveCallback,
    ) {
        use crate::skills::use_::bodies::callback::{BLADE_FURY, DEFAULT, INFERNO};
        use crate::world::objects::shrines::{SKILL_REMOVE, STAMINA_REMOVE};
        if matches!(
            callback.0,
            DEFAULT | SKILL_REMOVE | STAMINA_REMOVE | INFERNO | BLADE_FURY
        ) {
            self.removed_lists.push((unit, state, callback.0));
        }
    }
    /// `0x0063A4A0`(unit, state) (`stat-lists.md` §8.8 rule 1): state in
    /// range and flag `monstaydeath` for a monster, `plrstaydeath` for any
    /// other unit.
    fn stays_on_death(&self, lists: &StatLists, unit: UnitId, state: u32) -> bool {
        use crate::stats::states::group;
        let monster = lists
            .unit_list(unit)
            .is_some_and(|r| lists.owner_type(r) == crate::stats::lists::owner::MONSTER);
        let g = if monster {
            group::MON_STAY_DEATH
        } else {
            group::PLR_STAY_DEATH
        };
        lists.data().states.has_flag(state, g)
    }
}

impl<X: Pending> ActionHooks<X> {
    /// `0x0066A9B0` (`animdata.md` §5): the record of the COF name the
    /// composer ([`Pending::anim_name`]) builds for the unit's type,
    /// class and mode, looked up by §4; a name the file lacks gets the
    /// default record (§3). `None` when no table is loaded, the unit has
    /// no record, or the composer gives no name.
    /// The draw identity `0x00645270` (`render/unit-composite.md` §1.1)
    /// of `unit`'s (type, class, mode +0x10): its own unless flag-ex bit 3
    /// ([`flags2::DISGUISE`]) is set; then the first state of
    /// [`crate::stats::states::StateTable::gfx_states`] the unit has gives
    /// the type and class, and a player shown as a monster (`gfxtype` 1)
    /// or a monster shown as a player (2) has its mode mapped. The
    /// animation lookup and rate read it (`units.md` §4.7).
    pub(crate) fn draw_identity(
        &self,
        sim: &Sim<'_>,
        unit: UnitId,
    ) -> Option<(UnitType, u32, u32)> {
        let r = sim.units.get(unit)?;
        let own = (r.ty, r.class, r.mode);
        if r.flags2 & flags2::DISGUISE == 0 {
            return Some(own);
        }
        let states = &sim.stats.data().states;
        let Some(&(_, gfx, class)) = states
            .gfx_states()
            .iter()
            .find(|&&(s, ..)| sim.stats.has_state(unit, s))
        else {
            return Some(own);
        };
        let class = u32::from(class);
        Some(match (gfx, r.ty) {
            (1, UnitType::Player) => {
                let combat = &self.tables.combat;
                let row = combat
                    .monstats
                    .get(class as usize)
                    .and_then(|m| combat.monstats2.get(usize::from(m.monstatsex)));
                let has = |m: u32| {
                    row.is_some_and(|r| {
                        [
                            r.mdt, r.mnu, r.mwl, r.mgh, r.ma1, r.ma2, r.mbl, r.msc, r.ms1, r.ms2,
                            r.ms3, r.ms4, r.mdd, r.mkb, r.msq, r.mrn,
                        ]
                        .get(m as usize)
                        .copied()
                        .unwrap_or(false)
                    })
                };
                let mut m = PLAYER_TO_MONSTER.get(r.mode as usize).copied().unwrap_or(1);
                while m != 1 && !has(m) {
                    m = mode_fallback(m);
                }
                (UnitType::Monster, class, m)
            }
            (1, _) => (UnitType::Monster, class, r.mode),
            (_, UnitType::Monster) => {
                let m = if class == 6 && r.mode == 4 {
                    12
                } else {
                    MONSTER_TO_PLAYER.get(r.mode as usize).copied().unwrap_or(1)
                };
                (UnitType::Player, class, m)
            }
            _ => (UnitType::Player, class, r.mode),
        })
    }

    /// Steps 3–5 and 8–10 of `0x00623F50` (`units.md` §4.7, through
    /// [`crate::units::anim_rate::anim_rate`]) on the draw identity, with
    /// `record` the AnimData record of that identity (s = its speed;
    /// its frames · 256 are the +0x48 the were-form speed reads, the
    /// record the mode start stores). Steps 6–7 are
    /// [`Self::movement_rate`]'s; a velocity mode without the path
    /// provider, or a type other than player and monster, is `None` (the
    /// host's [`Pending::anim_rate`]). The item/skill getter
    /// `0x00625500` is the unit total (`sim/stats.md`).
    // TODO(units.md §4.7 step 8.3): the dual-wield average is not
    // applied: the items at body locations 4 and 5 and their stat 68 are
    // not reachable from the action wiring.
    fn spec_anim_rate(
        &self,
        sim: &Sim<'_>,
        unit: UnitId,
        record: &d2_formats::animdata::AnimRecord,
        frame_count: i32,
    ) -> Option<i16> {
        use crate::units::anim_rate::{anim_rate, mode_row, Rate, RateInput};
        let r = sim.units.get(unit)?;
        let (ty, class, mode) = self.draw_identity(sim, unit)?;
        let t = match ty {
            UnitType::Player => 0,
            UnitType::Monster => 1,
            _ => return None,
        };
        if mode_row(t, class, mode).v && self.paths.is_none() {
            return None;
        }
        let total = |k: u16| sim.stats.unit_total(unit, k, 0);
        let used = self
            .used_skill_of(unit)
            .and_then(|e| self.tables.skills.skill(e.skill));
        // Step 8.6: `0x00646170`, a player in a were-form (`0x0063A400`:
        // flag-ex bit 3) with a state of group 38 `meleeonly`.
        let were_speed = (r.ty == UnitType::Player
            && r.flags2 & flags2::DISGUISE != 0
            && sim.stats.has_group(unit, MELEE_ONLY))
        .then(|| {
            let n = match self.x.attack_weapon(unit) {
                Some(w) => self.x.attack_frames(unit, w).unwrap_or(0),
                None => 19,
            };
            if n <= 0 {
                0
            } else {
                (frame_count & !0xFF) / n
            }
        });
        let input = RateInput {
            applies: true,
            is_object_or_missile: false,
            t,
            c: class,
            m: mode,
            s: record.speed as i32,
            item: [total(93), total(99), total(105), total(102), total(96)],
            velocitypercent: total(67),
            attackrate: total(68),
            other_animrate: total(69),
            used_seqtrans: used.map(|row| i32::from(row.seqtrans as i8)),
            use_attack_rate: used.is_some_and(|row| row.useattackrate),
            holyshield: sim.stats.has_state(unit, HOLY_SHIELD),
            has_path: self.path_has(unit),
            w: 0,
            velocity_mode: false,
            dual: None,
            player_mode_18: r.ty == UnitType::Player && r.mode == 18,
            were_speed,
        };
        match anim_rate(&input) {
            Ok(Rate::Set { speed, .. }) => Some(speed as i16),
            _ => None,
        }
    }

    /// The anim refresh `0x00623F50(unit)` outside a mode start
    /// (`skills/bodies.md` §2.6 "anim refresh", a state's stat fill):
    /// the rate of the unit's current mode on its draw identity, with the
    /// frame count (+0x48) as the mode start left it. `None`: no rate
    /// (nothing written).
    pub(crate) fn rate_refresh(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<i16> {
        if let Some(v) = self.movement_rate(sim, unit) {
            return Some(v);
        }
        let record = self.anim_lookup(sim, unit)?;
        let fc = sim.units.get(unit)?.anim.frame_count;
        self.spec_anim_rate(sim, unit, &record, fc)
    }

    fn anim_lookup(
        &mut self,
        sim: &Sim<'_>,
        unit: UnitId,
    ) -> Option<d2_formats::animdata::AnimRecord> {
        let data = self.anim_data.clone()?;
        let (ty, class, mode) = self.draw_identity(sim, unit)?;
        let name = self.x.anim_name(unit, ty, class, mode)?;
        match data.record(&name) {
            Ok(rec) => Some(rec.clone()),
            Err(e) => {
                self.errors.push(WiringError::AnimData(e));
                None
            }
        }
    }
}

/// State flag group 38 `meleeonly` (`units.md` §4.7 step 8.6).
const MELEE_ONLY: usize = 38;
/// State 101 `holyshield` (§4.7 step 4).
const HOLY_SHIELD: u32 = 101;

/// Player mode (0…19) → monster mode (`0x006EB348`,
/// `render/unit-composite.md` §1.1).
const PLAYER_TO_MONSTER: [u32; 20] = [
    0, 1, 2, 15, 3, 1, 2, 4, 5, 6, 7, 4, 11, 8, 9, 10, 11, 12, 14, 13,
];
/// Monster mode (0…15) → player mode (`0x006EB308`).
const MONSTER_TO_PLAYER: [u32; 16] = [0, 1, 2, 4, 7, 8, 9, 10, 13, 14, 15, 16, 17, 19, 18, 3];

/// The monster-mode fallback of the player → monster map
/// (`render/unit-composite.md` §1.1): WL, GH, A1 → NU; A2 → A1; BL → GH;
/// SC → A1; S1 → NU; S2, S3, S4 → S1; DD, KB, SQ → NU; RN → WL; DT and
/// anything else → NU.
fn mode_fallback(m: u32) -> u32 {
    match m {
        5 => 4,
        6 => 3,
        7 => 4,
        9..=11 => 8,
        15 => 2,
        _ => 1,
    }
}

/// The fields `units.md` §4.2 reads from an AnimData record: frames,
/// byte +0x0F (the speed's high byte, read as event index −1 by the
/// variants) and the 144 event bytes.
pub fn anim_record(r: &d2_formats::animdata::AnimRecord) -> AnimRecord {
    let mut events = [0u8; ANIM_EVENTS];
    events.copy_from_slice(&r.events[..ANIM_EVENTS]);
    AnimRecord {
        frames: r.frames,
        byte_0f: (r.speed >> 24) as u8,
        events,
    }
}

impl<X: Pending> UnitHooks for ActionHooks<X> {
    /// Runs the queued remove callbacks of the lists the expiry walk
    /// freed (`stat-lists.md` §8.2 rule 6, `skills/bodies.md` §2.8).
    // PROVISIONAL (REC-263; d2rs-own, unverified): the bodies of the shrine
    // callbacks `0x00583BD0` / `0x00583A40` are unwritten. Each runs the
    // default (state off) and the stamina one also clamps stamina to its
    // maximum (the shrine set stamina to 2v on the list); the skill one's
    // skill refresh has nothing to refresh here (levels read the stat).
    fn lists_expired(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        use crate::skills::use_::bodies::callback::{BLADE_FURY, INFERNO};
        use crate::skills::use_::bodies::helpers::FLAG_40;
        use crate::world::objects::shrines::STAMINA_REMOVE;
        for (u, state, cb) in std::mem::take(&mut self.removed_lists) {
            let t = sim.stats.toggle_state(u, state, false);
            // PROVISIONAL (REC-731): "state off" read as the toggle with
            // the update-queue insert `0x00639DB0` (`stat-lists.md` §9.2),
            // so the client pass sends S→C 0xA9. Recorded 2026-10-09: the
            // `manapot` list freed at full mana (no mana change that tick)
            // still gives 0xA9 106 the next frame
            // (`facts/items/a1-town-potions-low.tsv` n 35).
            if let Err(e) = sim.game.lists.queue_update(u) {
                self.errors
                    .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                        e.into(),
                    )));
            }
            if let (Some(d), Some(r)) = (t.disguise, sim.units.get_mut(u)) {
                if d {
                    r.flags2 |= flags2::DISGUISE;
                } else {
                    r.flags2 &= !flags2::DISGUISE;
                }
            }
            if cb == STAMINA_REMOVE {
                sim.stats.clamp_to_max(self, u);
            }
            // `0x005C8BF0` / `0x005D69B0`: unit flags (+0xC4) |= 0x40, so
            // the sequence's later do events do not run (`use.md` §5.2
            // rule 3).
            if matches!(cb, INFERNO | BLADE_FURY) {
                if let Some(r) = sim.units.get_mut(u) {
                    r.flags |= FLAG_40;
                }
            }
        }
        let _ = unit;
    }
    /// `0x00580EC0`: the death penalties at `0x00580F59`
    /// (`vitals.md` §4.6, [`super::death`]).
    /// Then the client status bit 0x08 at `0x00580F83`, softcore too
    /// (`formats/d2s.md` §2.3, `vitals.md` §4.8 rule 1.4).
    fn player_death(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.death_penalties(sim, unit);
        sim.game.lists.set_player_status(unit, STATUS_DEAD);
    }
    /// `0x0057FCA0`: the corpse creation `0x0057F700` at `0x0057FD1C`
    /// (`vitals.md` §4.7 rule 1, [`super::death`]), then `0x00575BC0`
    /// at `0x0057FD25` in every game type (`hirelings-2.md` §15 rule 1):
    /// queued for the host that holds the hireling lists
    /// ([`ActionHooks::owner_deaths`]).
    /// Then the client status bit 0x08 at `0x0057FD46` (`formats/d2s.md`
    /// §2.3, `vitals.md` §4.8 rule 2).
    fn player_corpse(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.corpse_creation(sim, unit);
        if let Some(q) = self.owner_deaths.as_mut() {
            q.push(unit);
        }
        sim.game.lists.set_player_status(unit, STATUS_DEAD);
    }
    /// `0x0057FB70` ([`super::death`]; the experience it returns is not
    /// read by the 0x16 caller).
    fn player_corpse_pickup(&mut self, sim: &mut Sim<'_>, player: UnitId, corpse: UnitId) -> bool {
        self.corpse_pickup(sim, player, corpse).is_some()
    }
    /// `0x00620F00`: the AnimData record of the unit's mode
    /// (`units.md` §4.1, `animdata.md` §5).
    fn anim_record(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<AnimRecord> {
        self.anim_lookup(sim, unit).map(|r| anim_record(&r))
    }

    /// `0x00621260` with the lookup `0x00663310` (`skills/sequences.md`
    /// §1–§2) for a player in mode 18: the used skill's `seqnum` and the
    /// unit's COF weapon class ([`Pending::composit_weapon_class`], the
    /// class the animation names use) pick the frame list; its event bytes,
    /// length · 256 and speed 256. A monster in mode 14: its class's slot
    /// of the used skill picks the `monseq` list
    /// ([`ActionHooks::monster_sequences`], §1 rules 2 and 5). `None`
    /// (the plain animation): no used skill, `seqnum` 0, a null list.
    fn load_sequence(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<Sequence> {
        let rec = sim.units.get(unit)?;
        if rec.ty == UnitType::Monster {
            let used = self.used_skill_of(unit)?;
            let class = usize::try_from(rec.class).ok()?;
            let frames = self.monster_sequences.as_ref()?.lookup(class, used.skill)?;
            return Some(Sequence {
                frame_count: (frames.len() as i32) * 256,
                speed: 256,
                pos: 0,
                events: frames.iter().map(|f| f.event).collect(),
                drawn: frames.iter().map(|f| f.frame).collect(),
            });
        }
        if rec.ty != UnitType::Player {
            return None;
        }
        let used = self.used_skill_of(unit)?;
        let row = self
            .tables
            .skills
            .skills
            .get(usize::try_from(used.skill).ok()?)?;
        let class = usize::try_from(self.x.composit_weapon_class(unit)).ok()?;
        let frames = crate::skills::sequences::lookup(row.seqnum, class)?;
        Some(Sequence {
            frame_count: (frames.len() as i32) * 256,
            speed: 256,
            pos: 0,
            events: frames.iter().map(|f| f.event).collect(),
            drawn: frames.iter().map(|f| f.frame).collect(),
        })
    }

    /// The animation re-init `0x00624390` of a mode change
    /// (`units.md` §4.1), for players and monsters: action frame +0x4E
    /// := 0, frame +0x44 := frame bonus · 256 (`world/objects-client.md`
    /// §26 r5), the AnimData record of the new mode (`0x00620F00`), its
    /// frame count +0x48 := frames · 256 and, for a unit with a path,
    /// the speed +0x4C := the rate `0x00623F50` (`units.md` §4.7).
    /// Other types: objects run their own branch
    /// ([`crate::world::objects`]), items and the rest nothing here.
    // PROVISIONAL (units.md §4.1, REC-592): the player/monster branch of
    // `0x00624390` is not written in a spec; it is taken to set +0x48 and
    // +0x4C as the prepare step `0x005533D0` does for a plain animation
    // (a 1.14d player standing in town reads +0x48 = AnimData frames · 256
    // and +0x4C = the AnimData speed with no animated start run; the
    // velocity half of `0x00623F50` and the sequence loads are left to
    // the mode starts).
    fn reinit_anim(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(ty) = sim.units.get(unit).map(|r| r.ty) else {
            return;
        };
        if !matches!(ty, UnitType::Player | UnitType::Monster) {
            return;
        }
        let rate = self
            .path_has(unit)
            .then(|| UnitHooks::anim_rate(self, sim, unit));
        let record = UnitHooks::anim_record(self, sim, unit);
        let bonus = self.x.frame_bonus(unit);
        let Some(r) = sim.units.get_mut(unit) else {
            return;
        };
        let a = &mut r.anim;
        a.action_frame = 0;
        a.frame = bonus.wrapping_mul(256);
        if let Some(rec) = record {
            a.frame_count = (rec.frames as i32).wrapping_mul(256);
        }
        a.record = record;
        if let Some(s) = rate {
            a.speed = s;
        }
    }

    /// `0x00623F50` (`units.md` §4.3, §4.7) from the record's speed
    /// (+0x0C): steps 6 (knockback) and 7 (velocity modes) here, with the
    /// path provider ([`ActionHooks::movement_rate`]); every other step
    /// is the host's ([`Pending::anim_rate`]).
    fn anim_rate(&mut self, sim: &Sim<'_>, unit: UnitId) -> i16 {
        if let Some(v) = self.movement_rate(sim, unit) {
            return v;
        }
        let record = self.anim_lookup(sim, unit);
        // The mode start stores the new record and its frame count before
        // the rate reads +0x48.
        if let Some(v) = record.as_ref().and_then(|r| {
            let fc = (r.frames as i32).wrapping_mul(256);
            self.spec_anim_rate(sim, unit, r, fc)
        }) {
            return v;
        }
        self.x.anim_rate(unit, record.map(|r| r.speed))
    }

    /// The velocity half of `0x00623F50` for monsters with the path
    /// provider ([`crate::wiring::path::monsters`]).
    fn anim_velocity(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        self.monster_mode_velocity(sim, unit);
    }

    /// The path part of `0x005A7C20` ([`crate::wiring::path::monsters`]);
    /// the requested mode is kept for the start function.
    fn monster_mode_bookkeeping(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {
        self.monster_request = mode;
        let left = sim.units.get(unit).map(|r| r.mode);
        if let Some(m) = left {
            self.leave_monster_mode(unit, m);
        }
        self.monster_path_setup(sim, unit, mode);
    }

    /// `0x005A4F50` ([`Pending::monster_mode_damage`]).
    fn monster_mode_damage(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {
        X::monster_mode_damage(self, sim, unit, mode);
    }

    /// `0x00623B10` (`units.md` §4.3).
    fn frame_bonus(&mut self, _: &Sim<'_>, unit: UnitId) -> i32 {
        self.x.frame_bonus(unit)
    }

    fn has_path(&mut self, _: &Sim<'_>, unit: UnitId) -> bool {
        self.path_has(unit)
    }

    /// The provider's static path record (`path-placement.md` §2.1).
    fn static_position(&self, unit: UnitId) -> Option<(i32, i32)> {
        match self.paths.as_ref()?.record(unit)? {
            crate::path::UnitPath::Static(s) => Some((s.x, s.y)),
            crate::path::UnitPath::Dynamic(_) => None,
        }
    }

    /// Player event 0 in modes 2, 3, 6, 19: the player step `0x00580C20`
    /// (`pathing.md` §9.2) with the path provider; the step result (2:
    /// stopped, the ENDANIM handler follows). Without the provider: the
    /// trait default (1, nothing moves).
    fn player_movement_step(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        let _ = (a1, a2);
        if self.paths.is_none() {
            return 1;
        }
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        crate::wiring::path::walk::player_step(&mut v, sim.game, unit)
    }

    /// Player event 0 in attack, cast and skill modes (`0x00580460`,
    /// `units.md` §4.5), through [`Pending::action_frame`].
    fn player_action_frame(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        X::action_frame(self, sim, unit, a1, a2)
    }

    /// Monster mode functions (`units.md` §4.6): the start and event
    /// functions of rules 5–14 ([`crate::wiring::path::monsters`]); the
    /// death start `0x005A6FF0` goes to [`Pending::monster_death_start`]
    /// with the mode change's target and the death clean-up
    /// ([`super::monster_death`]), the DD start `0x005A7390` runs rule 4
    /// there; DT's event functions `0x005A7350` /
    /// `0x005A72B0` end the death in mode 12 (`intents-events.md` §7.7
    /// rule 3, [`super::unit_update::death_function`]); every other
    /// function keeps the default (started, nothing done).
    fn monster_mode_function(&mut self, sim: &mut Sim<'_>, unit: UnitId, address: u32) -> bool {
        if let Some(started) = self.monster_motion_function(sim, unit, address) {
            return started;
        }
        if address == MONSTER_MODES[0].start {
            let target = self.mode_target;
            return self.monster_death(sim, unit, target);
        }
        if address == MONSTER_MODES[12].start {
            self.monster_dead_start(sim, unit);
            return true;
        }
        super::unit_update::death_function(self, sim, unit, address);
        true
    }

    /// `0x0057C980`: the unit's own entries leave its combat list
    /// (`damage.md` §3 step 3: entries are (attacker, defender) records).
    fn drop_combat_entries(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(e) = sim.game.lists.unit(unit) else {
            return;
        };
        let id = (e.ty, e.guid);
        if let Some(list) = self.combat_lists.get_mut(&unit) {
            list.retain(|c| c.attacker != id);
        }
    }

    /// `0x00648730` on the unit's dynamic path record
    /// (`crate::wiring::path::monsters::stop_path`).
    fn stop_path_now(&mut self, unit: UnitId) -> bool {
        crate::wiring::path::monsters::stop_path(self, unit).is_some()
    }

    /// AI param 0 of the unit's AI control (`ai.md` §3: `0x0058EC00(unit,
    /// 1, v)`); false while the store is lent or the unit has no AI.
    fn set_ai_param0(&mut self, unit: UnitId, v: i32) -> bool {
        self.ai
            .as_mut()
            .and_then(|a| a.control_mut(unit))
            .map(|c| c.params[0] = v)
            .is_some()
    }

    /// `0x0061AB00` on the unit's room.
    fn room_flag(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        sim.game
            .lists
            .unit(unit)
            .and_then(|e| e.room())
            .is_some_and(|r| self.drlg.in_town(sim.game, r))
    }

    /// `0x005544B0(unit, 0)` before a think is scheduled on a monster with
    /// state 54 (`tick.md` §5.2 rule 4, `ai.md` §1.1): state 54 off, the
    /// monster's type-2 events cancelled.
    fn uninterruptable_check(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let mut v = View::of(sim.units, sim.stats, sim.data, self);
        v.set_state(unit, state::UNINTERRUPTABLE as u16, false);
        sim.game
            .timers
            .cancel_unit_events(unit, event::AI_THINK, None);
    }

    /// Event 2 `0x005B1740` (`ai.md` §2). The freeze drop of `tick.md`
    /// §5.6 already ran in the unit dispatch.
    fn ai_think(&mut self, sim: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        let Some(mut store) = self.ai.take() else {
            self.errors.push(WiringError::Reentrant("ai"));
            return;
        };
        let t = self.tables.clone();
        let info = self.ai_info;
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                    skills: &t.skills.skills,
                    missiles: &t.skills.missiles,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            ai::think(sim.game, &mut cx, unit);
        }
        self.ai = Some(store);
    }

    /// The mode-set sites of the umod dispatcher (`umod-callbacks.md`
    /// §2 rules 1–2) on the lent monster world; without one, nothing.
    fn monster_umods(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u8) {
        self.run_umods(sim, unit, None, mode);
    }

    /// Event 7 `0x005A4370` → the umod dispatcher in mode 2 (`init.md`
    /// §22) on the lent monster world ([`super::monsters`]); without one,
    /// nothing (the trait default). The unit dispatch already ran the
    /// handler-table checks and the frozen-monster drop (`tick.md` §5.6).
    fn monster_umod(&mut self, sim: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        self.run_umods(sim, unit, None, umod_mode::EVENT7);
    }

    /// Event 10 `0x005A7F70` → `0x00573120` (`ai.md` §1; monster data).
    fn ai_reset(&mut self, _: &mut Sim<'_>, unit: UnitId, _: u32, _: u32) {
        self.x.ai_reset(unit);
    }

    /// Event 5 `0x0056D790` (`stat-lists.md` §10.2): the skills'
    /// active-state function, through [`Pending::skill_event`].
    fn active_state(&mut self, sim: &mut Sim<'_>, unit: UnitId, f: u16, skill: u32, a2: u32) {
        X::skill_event(
            self,
            sim,
            SkillEvent::ActiveState {
                unit,
                f,
                skill,
                arg2: a2,
            },
        );
    }

    /// Event 8 `0x0056FCB0` (`use.md` §7), through [`Pending::skill_event`].
    fn periodic_skills(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {
        X::skill_event(
            self,
            sim,
            SkillEvent::Periodic {
                unit,
                arg1: a1,
                arg2: a2,
            },
        );
    }

    /// Event 9 `0x0056FE40` after its checks (`stat-lists.md` §10.3), through
    /// [`Pending::skill_event`].
    fn apply_item_aura(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        a1: u32,
        skill: u32,
        level: i32,
    ) {
        X::skill_event(
            self,
            sim,
            SkillEvent::ItemAura {
                unit,
                arg1: a1,
                skill,
                level,
            },
        );
    }

    /// `0x00571A10`(unit, f) (`stat-lists.md` §10.1, `stats.md` §9.3): the
    /// 0xAB record {f} on the unit, the unit queued for update
    /// (`intents-events.md` §7.9 rule 2).
    fn send_life_fraction(&mut self, sim: &mut Sim<'_>, unit: UnitId, fraction: i32) {
        use super::event_records::EventRecord;
        let life = fraction.clamp(0, 255) as u8;
        self.event_records.push(unit, EventRecord::NpcHeal { life });
        let _ = sim.game.lists.queue_update(unit);
    }

    /// Object events (`units.md` §6.4) on the object state
    /// ([`super::objects`]); a game without one keeps the default.
    fn object_event(&mut self, sim: &mut Sim<'_>, unit: UnitId, event: u8) {
        View::of(sim.units, sim.stats, sim.data, self).object_event(sim.game, unit, event);
    }

    /// Missile events (`0x005ADBB0`, `missiles.md` §R3).
    fn missile_do(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let Some(mut store) = self.missiles.take() else {
            self.errors.push(WiringError::Reentrant("missiles"));
            return;
        };
        let t = self.tables.clone();
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut v,
            };
            missiles::class_handler(sim.game, &mut cx, unit);
        }
        self.missiles = Some(store);
    }
}

/// Flags 2 bit 0x10000 (`hirelings.md` §6 rule 5): the S→C 0x15 of the
/// next update (`intents-events.md` §7.3 rule 2 step 1).
const PET_WARP_REASSIGN: u32 = 0x10000;

impl<X: Pending> LifecycleHooks for ActionHooks<X> {
    fn request_act_change(&mut self, player: UnitId, level: u32, arg: u32) {
        self.act_changes.push((player, level, arg));
    }
    fn town_room(&self, game: &Game, room: crate::units::RoomId) -> bool {
        self.drlg.in_town(game, room)
    }
    fn room_level(&self, game: &Game, room: crate::units::RoomId) -> Option<u32> {
        self.drlg.level_id(game, room)
    }
    fn path_xy(&self, unit: UnitId) -> Option<(i32, i32)> {
        self.path_has(unit).then(|| self.path_position(unit))
    }
    /// The static path set at the spot (`0x00620AE0`); a dynamic path is
    /// not an item's. PROVISIONAL (REC-281): the footprint is not moved
    /// (the item's footprint is not removed when it leaves the ground
    /// either, `View::path_free`).
    fn ground_item_placed(&mut self, item: UnitId, room: crate::units::RoomId, x: i32, y: i32) {
        let Some(p) = self.paths.as_mut() else {
            return;
        };
        match p.records.get_mut(&item) {
            Some(crate::path::record::UnitPath::Static(s)) => s.set(Some(room), x, y),
            Some(_) => {}
            None => {
                let mut s = crate::path::record::StaticPath::default();
                s.set(Some(room), x, y);
                p.records
                    .insert(item, crate::path::record::UnitPath::Static(s));
            }
        }
    }
    /// [`View::town_portal_cast`] with the call's game seed lent to the
    /// hooks; no object state: `None`.
    fn town_portal_cast(
        &mut self,
        sim: &mut Sim<'_>,
        seed: &mut crate::rng::Seed,
        player: UnitId,
    ) -> Option<(u32, bool)> {
        self.objects.as_ref()?;
        self.game_seed = *seed;
        let r = View::of(sim.units, sim.stats, sim.data, self).town_portal_cast(sim.game, player);
        *seed = self.game_seed;
        Some(r)
    }
    /// The monster type init `0x00574250` (`init.md` §5, `units.md` §3.1
    /// table: the allocator's per-kind init of a monster) on the lent
    /// monster world ([`super::monsters`]); the object data and init
    /// `0x0054F5D0` of an object on the object state
    /// ([`View::object_init`], `objects.md` §3); other kinds, and a game
    /// without the lent world or the object state, keep the default
    /// (nothing).
    fn init_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId, req: &AllocRequest) {
        // The init's room is the allocation's (r7.2) until step 8.
        self.alloc_rooms.push((unit, req.room));
        if req.ty == UnitType::Player {
            // Player type init `0x005348C0`: unit flags |= 0x0E first
            // (`units.md` §1 row 0), so the missile target filter
            // (`missiles.md` §R4.2) accepts the player.
            if let Some(r) = sim.units.get_mut(unit) {
                r.flags |= 0x0E;
            }
        }
        if req.ty == UnitType::Monster {
            self.with_monster_world(|w, h| w.type_init(sim, h, unit));
        } else if req.ty == UnitType::Object {
            // Inside `View::allocate`: run once its seed step is back in
            // the hooks (the init may allocate and draw itself).
            if let Some(q) = self.deferred_inits.as_mut() {
                q.push(unit);
                return;
            }
            View::of(sim.units, sim.stats, sim.data, self).object_init(sim.game, unit);
        }
    }

    /// The mercenary's creation (`npc.md` §7.3 step 7, `hirelings.md`
    /// §3.1): `0x005B23C0(game, near, class, mode, 4, 0)`, the placement
    /// and creation of `population.md` §9 around `near`'s path position
    /// in its room (spread 4: rings 3 … 12 on the active-room seed),
    /// with the call's game seed lent to the hooks for the allocation's
    /// unit-seed step (`rng.md` §5.3). `None` when nothing was placed.
    /// Without the lent monster world: a plain allocation at (+2, +2)
    /// from the point (d2rs-own, unverified: hosts with no population
    /// state).
    fn spawn_near(
        &mut self,
        sim: &mut Sim<'_>,
        seed: &mut Seed,
        near: UnitId,
        class: u32,
        mode: u8,
    ) -> Option<UnitId> {
        let room = sim.game.lists.unit(near)?.room()?;
        let (x, y) = self.path_position(near);
        self.game_seed = *seed;
        let placed = self
            .with_monster_world(|w, h| {
                w.spawn_at(sim, h, room, x, y, class as i32, mode, HIRE_SPREAD, 0)
            })
            .flatten();
        let u = match placed {
            Some(placed) => placed,
            None => {
                let req = AllocRequest {
                    ty: UnitType::Monster,
                    class,
                    room: Some(room),
                    add: true,
                    fixed_guid: None,
                    mode: u32::from(mode),
                    allied: false,
                };
                View::of(sim.units, sim.stats, sim.data, self).allocate(
                    sim.game,
                    &req,
                    x + 2,
                    y + 2,
                )
            }
        };
        *seed = self.game_seed;
        u
    }

    /// The minion owner of the unit's AI control record (owner data
    /// `0x0058F030`): a unit without AI control keeps none. The lookup
    /// is by GUID at every use (`umod-callbacks.md` §1 rule 5), so an
    /// owner GUID that names no unit (−1) drops the link.
    fn set_ai_owner(&mut self, unit: UnitId, owner_type: u8, owner_guid: u32) {
        let Some(ty) = UnitType::ALL.get(usize::from(owner_type)).copied() else {
            return;
        };
        if let Some(c) = self.ai.as_mut().and_then(|s| s.control_mut(unit)) {
            c.minion_owner = Some(ai::UnitRef {
                ty,
                guid: owner_guid,
            });
        }
    }

    /// On the lent monster world (none: nothing).
    fn assign_umod(&mut self, sim: &mut Sim<'_>, unit: UnitId, umod: u8) {
        self.with_monster_world(|w, h| w.assign_umod(sim, h, unit, umod));
    }

    /// `0x00574CC0` (`hirelings.md` §6 rule 5): the pet placed at the
    /// player's room and point (`0x00650BE0`, the path provider's
    /// teleport, which also leaves the old room's list), queued for
    /// update with flags 2 |= 0x10000 (the reassign of `intents-events.md`
    /// §7.3 rule 2 step 1), then `0x00573780` (`ai.md` §1.5). No player
    /// room or no path provider: nothing.
    // TODO(hirelings.md §6 r5): the path reset `0x00648C30(path, 0x100)`
    // "when not moving" has no reading of "moving" in the spec; not run.
    fn warp_pet(&mut self, sim: &mut Sim<'_>, pet: UnitId, player: UnitId) {
        let Some(room) = sim.game.lists.unit(player).and_then(|e| e.room()) else {
            return;
        };
        let (x, y) = self.path_position(player);
        let placed = View::of(sim.units, sim.stats, sim.data, self).path_teleport_to(
            sim.game,
            pet,
            Some(room),
            x,
            y,
        );
        if placed.is_none() {
            return;
        }
        if let Err(e) = sim.game.lists.queue_update(pet) {
            self.errors
                .push(WiringError::Unit(UnitError::Game(e.into())));
        }
        if let Some(r) = sim.units.get_mut(pet) {
            r.flags2 |= PET_WARP_REASSIGN;
        }
        let Some(mut store) = self.ai.take() else {
            return;
        };
        let t = self.tables.clone();
        let info = self.ai_info;
        {
            let mut v = View::of(sim.units, sim.stats, sim.data, self);
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                    skills: &t.skills.skills,
                    missiles: &t.skills.missiles,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            ai::update_ai_callback(sim.game, &mut cx, pet);
        }
        self.ai = Some(store);
    }

    /// Step 8 linked the unit: its room is the list's from now on.
    fn added(&mut self, _: &mut Sim<'_>, unit: UnitId) {
        self.alloc_rooms.retain(|&(u, _)| u != unit);
    }

    /// The per-kind state of the action modules leaves with the unit:
    /// AI control (`AiStore::remove`), a summon's skill entries, missile
    /// data, combat list; then
    /// the lent monster world's part (monster data, minion list, owner
    /// link); an object's object data.
    fn free_kind(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        let (ty, class, mode) = sim
            .units
            .get(unit)
            .map_or((None, 0, 0), |r| (Some(r.ty), r.class, r.mode));
        // A ground item leaves the clients' rooms: its removal record
        // (REC-281, `ActionHooks::removed_items`), in the room its path
        // was in.
        if ty == Some(UnitType::Item) && mode == u32::from(crate::items::moves::mode::GROUND) {
            let room = self
                .paths
                .as_ref()
                .and_then(|p| p.record(unit))
                .and_then(|r| r.room())
                .or_else(|| sim.game.lists.unit(unit).and_then(|e| e.room()));
            let guid = sim.units.get(unit).map(|r| r.guid);
            if let (Some(room), Some(guid)) = (room, guid) {
                self.removed_items.push((guid, room));
                let act = sim.game.lists.room(room).map(|r| r.act);
                if let Some(a) = act.and_then(|a| sim.game.lists.act_mut(a)) {
                    a.pending_removals = true;
                }
            }
        }
        self.path_free(unit, ty, class, mode);
        self.monster_skills.remove(&unit);
        if let Some(ai) = self.ai.as_mut() {
            ai.remove(unit);
        }
        if let Some(m) = self.missiles.as_mut() {
            m.remove(unit);
        }
        self.combat_lists.remove(&unit);
        self.handlers.remove(&unit);
        if let Some(st) = self.objects.as_mut() {
            st.control.data.remove(&unit);
        }
        self.with_monster_world(|w, _| w.forget(unit));
    }
}

impl<X: Pending> View<'_, X> {
    /// Records an error of a unit operation.
    pub fn unit_error(&mut self, e: UnitError) {
        self.h.errors.push(WiringError::Unit(e));
    }

    /// The unit seed (unit +0x20). A unit without a record gets a scratch
    /// seed and an error (API misuse).
    pub fn seed(&mut self, u: UnitId) -> &mut Seed {
        if self.units.get(u).is_none() {
            self.h
                .errors
                .push(WiringError::Unit(UnitError::UnknownUnit(u)));
            self.h.orphan_seed = Seed::init();
            return &mut self.h.orphan_seed;
        }
        &mut self.units.get_mut(u).expect("checked").seed
    }

    /// Unit getter `0x00625480(unit, stat, 0)`.
    pub fn stat(&self, u: UnitId, s: u16) -> i32 {
        self.stats.unit_total(u, s, 0)
    }

    /// Unit set `0x00627260(unit, stat, value, 0)`.
    pub fn set_base(&mut self, u: UnitId, s: u16, value: i32) {
        self.stats.unit_set(&mut *self.h, u, s, value, 0);
    }

    /// Set a stat of a list (`0x006270B0`, layer 0).
    pub fn set_list_stat(&mut self, l: ListId, s: u16, value: i32) {
        self.stats.set(&mut *self.h, l, s, value, 0, None);
    }

    /// State toggle `0x00625A70` (`stat-lists.md` §9.2) with the disguise
    /// bit of unit +0xC8.
    pub fn set_state(&mut self, u: UnitId, s: u16, on: bool) {
        let t = self.stats.toggle_state(u, u32::from(s), on);
        if let (Some(d), Some(r)) = (t.disguise, self.units.get_mut(u)) {
            if d {
                r.flags2 |= flags2::DISGUISE;
            } else {
                r.flags2 &= !flags2::DISGUISE;
            }
        }
    }

    /// The unit's stat list of `state` (`0x006256B0`).
    pub fn state_list(&self, u: UnitId, s: u16) -> Option<ListId> {
        let r = self.stats.unit_list(u)?;
        self.stats.list_of_state(r, u32::from(s))
    }

    /// `stat` of the unit's list of `state` (its own base value).
    pub fn state_stat(&self, u: UnitId, s: u16, st: u16) -> Option<i32> {
        let l = self.state_list(u, s)?;
        Some(self.stats.base(l, st, 0))
    }

    /// A plain stat list for `state` attached to the unit: allocation
    /// `0x006251F0` with the owner's type and GUID, expire `0x00627440`
    /// (sets NEWLENGTH when > 0), state field, attach `0x00626E10`.
    ///
    /// TODO(stat-lists.md §4, §8.1): the callers' allocation flags and the
    /// attach `reset` argument are not stated for state lists; flags 0
    /// and reset = 1 (no DYNAMIC) are used. Without an owner the unit's
    /// own type and GUID are used.
    pub fn create_state_list(
        &mut self,
        u: UnitId,
        s: u16,
        owner: Option<(UnitType, u32)>,
        expire: i32,
    ) -> Option<ListId> {
        let (ty, guid) = match owner {
            Some(o) => o,
            None => {
                let r = self.units.get(u)?;
                (r.ty, r.guid)
            }
        };
        let l = self.stats.alloc(0, 0, ty.index() as u32, guid);
        self.stats.set_expire(l, expire);
        self.stats.set_state(l, u32::from(s));
        self.stats.attach(&mut *self.h, u, l, true);
        Some(l)
    }

    /// Alignment `0x005543B0(unit, a, ..)` (`skills/bodies-2.md` §2.22,
    /// `hirelings.md` §3.2 rule 2): stat 172 (`alignment`) := `a` in the
    /// unit's state-105 (`alignment`) list, allocated with flags 0 when
    /// missing (`stat-lists.md` §2), the state on, its state-changed bit
    /// set again (`0x0055448A`, `intents-events.md` §3.5 rule 6 "resend")
    /// and the unit queued for update. The game's allied mark follows a
    /// good (2) alignment. `a` > 2 is the original's fatal assertion:
    /// nothing here.
    pub fn set_alignment(&mut self, game: &mut Game, u: UnitId, a: u8) {
        const STATE: u16 = crate::combat::range::STATE_ALIGNMENT;
        if a > 2 || self.units.get(u).is_none() {
            return;
        }
        let Some(l) = self
            .state_list(u, STATE)
            .or_else(|| self.create_state_list(u, STATE, None, 0))
        else {
            return;
        };
        self.set_list_stat(l, crate::combat::range::STAT_ALIGNMENT, i32::from(a));
        self.set_state(u, STATE, true);
        self.stats.set_state_changed(u, u32::from(STATE), true);
        game.lists.set_allied(u, a == 2);
        let _ = game.lists.queue_update(u);
    }

    /// Hireling test `0x0063EE90`: a monster of a hireling class
    /// (`monsters/init.md` §6 step 4).
    pub fn is_hireling(&self, game: &Game, u: UnitId) -> bool {
        game.lists
            .unit(u)
            .is_some_and(|e| e.ty == UnitType::Monster)
            && self
                .units
                .get(u)
                .is_some_and(|r| HIRELING_CLASSES.contains(&r.class))
    }

    /// Unit allocation `0x00555230` (`units.md` §3.1) on the game seed:
    /// steps 1–7 with the per-kind init (an object's after the seed step
    /// is written back), then step 8: `SUNIT_Add`'s list part and its
    /// path part (`path-placement.md` §2.5, [`View::path_place`]; without
    /// the path provider [`Pending::place`]). An object allocated from
    /// inside an object call is left unlinked: its caller runs the init
    /// (`objects::allocate`) and then [`View::add_allocated`].
    pub fn allocate(
        &mut self,
        game: &mut Game,
        req: &AllocRequest,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        let u = self.allocate_unadded(game, req)?;
        if req.ty == UnitType::Object && self.h.objects_out {
            return Some(u);
        }
        self.add_allocated(game, u, req, x, y).then_some(u)
    }

    /// The player's unit seed `0x00552DF0` on the game seed
    /// (`units.md` §3.1 r4.1): what the character load runs on the player
    /// it allocated, before anything else of the load draws. `false`: no
    /// such unit (no draw).
    pub fn init_player_seed(&mut self, u: UnitId) -> bool {
        crate::units::lifecycle::init_player_seed(self.units, u, &mut self.h.game_seed)
    }

    /// [`View::allocate`] up to step 7 (the kind init included): the
    /// unit is not added yet. Its caller runs what the allocator runs
    /// before `SUNIT_Add` and then [`View::add_allocated`].
    pub fn allocate_unadded(&mut self, game: &mut Game, req: &AllocRequest) -> Option<UnitId> {
        let mut seed = self.h.game_seed;
        let outer = self.h.deferred_inits.replace(Vec::new());
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::allocate_unlinked(&mut sim, &mut *self.h, &mut seed, req)
        };
        self.h.game_seed = seed;
        // The object init is the allocation's last step before `SUNIT_Add`
        // (`units.md` §3.1 r7; `quests-act1-rest.md` §9 item 7: after the
        // unit's seed step): run it now that the step is in the hooks.
        let inits = std::mem::replace(&mut self.h.deferred_inits, outer).unwrap_or_default();
        for u in inits {
            self.object_init(game, u);
        }
        match r {
            Ok(Some(u)) => Some(u),
            Ok(None) => None,
            Err(e) => {
                self.unit_error(e);
                None
            }
        }
    }

    /// Step 8 of an allocation (`units.md` §3.1): `SUNIT_Add` in the
    /// allocation's room, then the path part at (x, y). `false`: the add
    /// failed (logged).
    pub fn add_allocated(
        &mut self,
        game: &mut Game,
        u: UnitId,
        req: &AllocRequest,
        x: i32,
        y: i32,
    ) -> bool {
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::add(&mut sim, &mut *self.h, u, req)
        };
        if let Err(e) = r {
            self.unit_error(e);
            return false;
        }
        self.path_place(game, u, x, y);
        if self.h.paths.is_some() {
            self.monster_added(game, u, x, y);
        }
        true
    }

    /// The monster branch of `SUNIT_Add` after the path
    /// (`monsters/init.md` §4.1 step 1): `0x005735A0` (path velocity :=
    /// monstats `Velocity` · 256, `0x00648690`; then the monster mode set
    /// `0x005A7C20` of the creation mode, whose start function
    /// schedules the first think, `ai.md` §1.3), then the think restart
    /// `0x00573780` (`ai.md` §1.5 r1: the recorded "+aidel, cancel, +2"
    /// pairs), gated by `0x00553160(unit)` (step 1.2): the unit has a
    /// room and that active room's client count (+0x78) is nonzero; else
    /// the room clean-up `0x00553220` ([`View::room_cleanup`]) and no
    /// think. Not a monster: nothing. The request's target point is
    /// (x, y) (step 1.1): the mode set writes it to path +0x10 / +0x12
    /// (`ai.md` §7.5 rule 2, `0x00648AD0`).
    fn monster_added(&mut self, game: &mut Game, u: UnitId, x: i32, y: i32) {
        let Some((class, mode)) = self
            .units
            .get(u)
            .filter(|r| r.ty == UnitType::Monster)
            .map(|r| (r.class, r.mode))
        else {
            return;
        };
        let velocity = self
            .h
            .tables
            .combat
            .monstats
            .get(class as usize)
            .map_or(0, |m| i32::from(m.velocity))
            << 8;
        self.h.path_set_velocity(u, velocity);
        crate::wiring::path::monsters::stage_request(
            self.h,
            u,
            crate::monsters::ai::ModeTarget::Point(x, y),
            None,
        );
        let r = {
            let mut sim = Sim {
                game: &mut *game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::modes::monster_set_mode(&mut sim, &mut *self.h, u, mode)
        };
        // A creation mode the path set-up skips (GH, rule 1) leaves the
        // staged target unread: it must not reach a later request.
        if let Some(p) = self.h.paths.as_mut() {
            if matches!(p.mode_request, Some((v, _)) if v == u) {
                p.mode_request = None;
            }
        }
        if let Err(e) = r {
            self.unit_error(e);
            return;
        }
        if self.room_has_clients(game, u) {
            self.think_restart(game, u);
        } else {
            self.room_cleanup(u);
        }
    }

    /// `0x00553160(unit)` (`monsters/init.md` §4.1 step 1.2): the unit's
    /// room exists and its active room's client count is nonzero.
    fn room_has_clients(&self, game: &Game, u: UnitId) -> bool {
        let Some(room) = game.lists.unit(u).and_then(|e| e.room()) else {
            return false;
        };
        self.h
            .drlg
            .drlg_room(game, room)
            .and_then(|(d, id)| d.active_room(id))
            .is_some_and(|a| !a.clients.is_empty())
    }

    /// The allocation room of a unit between steps 7 and 8 (`units.md`
    /// §3.1 r7.2), else the room it stands in.
    pub fn init_room(&self, game: &Game, u: UnitId) -> Option<crate::units::RoomId> {
        match self.h.alloc_rooms.iter().rev().find(|&&(v, _)| v == u) {
            Some(&(_, room)) => room,
            None => game.lists.unit(u).and_then(|e| e.room()),
        }
    }

    /// Unit removal `0x00555600` (`units.md` §3.2).
    pub fn remove(&mut self, game: &mut Game, u: UnitId) {
        let r = {
            let mut sim = Sim {
                game,
                units: self.units,
                stats: self.stats,
                data: self.data,
            };
            crate::units::lifecycle::remove(&mut sim, &mut *self.h, u)
        };
        if let Err(e) = r {
            self.unit_error(e);
        }
    }

    /// A monster mode change (`units.md` §4.6, `0x005A7C20`). Its umod
    /// callbacks run inside it ([`UnitHooks::monster_umods`]: mode 0
    /// before the start function, mode 1 after the animation prepare,
    /// `umod-callbacks.md` §2) on the lent monster world.
    pub fn monster_set_mode(&mut self, game: &mut Game, u: UnitId, mode: u32) -> bool {
        let mut sim = Sim {
            game,
            units: self.units,
            stats: self.stats,
            data: self.data,
        };
        let r = crate::units::modes::monster_set_mode(&mut sim, &mut *self.h, u, mode);
        match r {
            Ok(()) => true,
            Err(e) => {
                self.unit_error(e);
                false
            }
        }
    }
}

/// Clears state 54 and, for players, state 92 (`0x005544B0` minus the
/// timer part, `ai.md` §1.1).
pub fn clear_uninterruptable<X: Pending>(v: &mut View<'_, X>, game: &Game, u: UnitId) {
    v.set_state(u, state::UNINTERRUPTABLE as u16, false);
    if game.lists.unit(u).is_some_and(|e| e.ty == UnitType::Player) {
        v.set_state(u, STATE_DEATH_DELAY, false);
    }
}

/// Largest speed (`units.md` §4.7: at most 0x7FFF; `data/fixups.md` §8
/// steps 4, 7: at most 32,767).
const SPEED_MAX: u32 = 0x7FFF;

/// monstats rows below this take their run base from the walk speed
/// (`data/fixups.md` §8 step 5).
const RUN_BASE_ROWS: usize = 410;

impl<X: Pending> ActionHooks<X> {
    /// `0x00623F50` steps 6 and 7 (`units.md` §4.7): knockback (player
    /// mode 19, monster mode 13) → w clamped to 0..0x7FFF; a mode with the
    /// velocity modifier (`pathing.md` §8.1 rule 2) → w · p / 100 (i32,
    /// truncating; 0 if ≤ 0, at most 0x7FFF). w = `0x006213D0`: player 101
    /// in mode 3, else 213; monster the run speed (+0x38) in mode 15, else
    /// the walk speed (+0x36), both `data/fixups.md` §8. `None`: another
    /// step applies, the unit has no path (step 7: nothing), or there is
    /// no path provider.
    // TODO(units.md §4.7 Definitions): (T, C, M) are the unit's own; the
    // disguise substitution `0x00645270` is not applied here.
    pub(crate) fn movement_rate(&self, sim: &Sim<'_>, unit: UnitId) -> Option<i16> {
        use crate::path::walk::velocity::{velocity_percent, VelocityFacts, STAT_VELOCITYPERCENT};
        let paths = self.paths.as_ref()?;
        let (ty, class, mode) = self.draw_identity(sim, unit)?;
        let knockback = matches!((ty, mode), (UnitType::Player, 19) | (UnitType::Monster, 13));
        let w = |h: &Self| -> i32 {
            match ty {
                UnitType::Player => {
                    if mode == 3 {
                        101
                    } else {
                        213
                    }
                }
                _ if mode == 15 => h.monster_run_speed(unit, class as usize) as i32,
                _ => h.monster_walk_speed(unit, class as usize) as i32,
            }
        };
        if !matches!(ty, UnitType::Player | UnitType::Monster) {
            return None;
        }
        if knockback {
            return Some(w(self).clamp(0, SPEED_MAX as i32) as i16);
        }
        if !self.path_has(unit) {
            return None;
        }
        let [_, _, scale_stat] = paths.tables.animstat[4];
        let facts = VelocityFacts {
            ty,
            class,
            npc: ty == UnitType::Monster
                && self
                    .tables
                    .combat
                    .monstats
                    .get(class as usize)
                    .is_some_and(|m| m.npc),
            used_flags: self
                .used_skill_of(unit)
                .map(|e| self.x.entry_flags(unit, &e)),
            // The item/skill getter `0x00625500` is the unit total
            // (`sim/stats.md`): Burst of Speed's state list counts.
            item_fastermove: sim.stats.unit_total(unit, scale_stat as u16, 0),
            velocitypercent: sim.stats.unit_total(unit, STAT_VELOCITYPERCENT, 0),
        };
        let p = velocity_percent(&paths.tables, &facts, mode)?;
        let v = w(self).wrapping_mul(p) / 100;
        Some(if v <= 0 { 0 } else { v.min(SPEED_MAX as i32) } as i16)
    }

    /// The AnimData speed (+0x0C) of monster class `class` in `mode`
    /// (the COF name of [`Pending::anim_name`]); a missing name or table
    /// reads the default record's 256 (`data/fixups.md` §8 step 2).
    fn monster_anim_speed(&self, unit: UnitId, class: usize, mode: u32) -> u32 {
        let Some(data) = self.anim_data.as_ref() else {
            return 256;
        };
        self.x
            .anim_name(unit, UnitType::Monster, class as u32, mode)
            .and_then(|n| {
                let len = n.iter().position(|&b| b == 0).unwrap_or(n.len());
                data.record(&n[..len]).ok().map(|r| r.speed)
            })
            .unwrap_or(256)
    }

    /// monstats row `r`'s `BaseId` after the repair of `data/fixups.md`
    /// §8 step 1 (out of range → `r`).
    fn monster_base(&self, r: usize) -> usize {
        let ms = &self.tables.combat.monstats;
        let b = ms.get(r).map_or(-1, |m| m.baseid as i16);
        if b < 0 || b as usize >= ms.len() {
            r
        } else {
            b as usize
        }
    }

    /// The walk speed monstats +0x36 of row `r` (`data/fixups.md` §8
    /// steps 1–4).
    // PROVISIONAL (data/fixups.md §8, REC-593): the COF name of the
    // lookup is the host's composer ([`Pending::anim_name`]), not the
    // fixup's own monster composer (token + mode + monstats2 `BaseW`).
    fn monster_walk_speed(&self, unit: UnitId, r: usize) -> u32 {
        let ms = &self.tables.combat.monstats;
        let b = self.monster_base(r);
        let mut w = self.monster_anim_speed(unit, b, 2);
        let vel = |i: usize| ms.get(i).map_or(0, |m| m.velocity as i16);
        if b != r && vel(b) > 0 {
            w = (i32::from(vel(r)) as u32).wrapping_mul(w) / vel(b) as u32;
        }
        w.min(SPEED_MAX)
    }

    /// The run speed monstats +0x38 of row `r` (`data/fixups.md` §8
    /// steps 5–7; a base row after `r` reads its compiled +0x36, 0).
    fn monster_run_speed(&self, unit: UnitId, r: usize) -> u32 {
        let ms = &self.tables.combat.monstats;
        let b = self.monster_base(r);
        let mut run = if r < RUN_BASE_ROWS {
            if b > r {
                0
            } else {
                (i32::from(self.monster_walk_speed(unit, b) as i16) / 2) as u32
            }
        } else {
            self.monster_anim_speed(unit, b, 15)
        };
        let rn = |i: usize| ms.get(i).map_or(0, |m| m.run as i16);
        if b != r && rn(b) > 0 {
            run = (i32::from(rn(r)) as u32).wrapping_mul(run) / rn(b) as u32;
        }
        run.min(SPEED_MAX)
    }
}
