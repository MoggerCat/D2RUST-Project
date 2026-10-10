// Spec: specs/audio/triggers.md (§2 r2, §4–§6, §8 r3–r4, §9 r1–r2), specs/audio/triggers-2.md (§18, §21)
//! The unit sounds of the client model: the table rows the rules read
//! ([`UnitSoundRows`]: `monsounds`, `monstats`, `monstats2`,
//! `superuniques`, `skills`, `states`, `missiles`, the item tables,
//! `AnimData`) and the per-unit pass ([`UnitFeed`]) that turns the model
//! into the calls of `Unit\UnitSnd.cpp`:
//!
//! - a **mode change** of a player or monster runs the mode sounds
//!   (§4 hit / death, swings, blocks, kicks, monster attack / skill
//!   voices, the dead-mode voice stop),
//! - a **new monster** runs the `Init` voice (§6 r2),
//! - **every client update** runs the monster `Neutral` voice (§6 r1) and
//!   the footsteps (§5),
//! - a state bit turning on or off runs the state sounds (§8 r4),
//! - a **new missile** requests its `TravelSound` (§8 r3),
//! - an item **dropping** (mode 5) or reaching the cursor (mode 4) runs
//!   the item sounds (§9 r1, r2),
//! - S→C 0x2C events 12, 16, 17 read the record the pass chooses
//!   ([`UnitFeed::event_unit`], [`UnitFeed::event12_stsound`]).
//!
//! PROVISIONAL guesses, each with what settles it (M25):
//!
//! - **REC-430** settled (Wine recording, `facts/client/anim/
//!   a1-town-walk-ama.tsv`): f := 0 at a mode change, wrapping at F
//!   (frames × 256); the speed is the rate of `sim/units.md` §4.7 (a
//!   monster from the client model, `bridge::monster_anim`, which also
//!   rolls a new monster's first frame; a player through
//!   [`player_rate`]); a monster's update advances f before its
//!   footstep, a player's footstep reads f before the advance. A monster
//!   the model does not animate (no 0xAC set-up: synthetic fixtures)
//!   keeps the raw AnimData speed from f = 0.
//! - **REC-431** (floor material k): the DT1 tile flags under the unit
//!   need the client room tile lists; k is the `soundenviron` `Material
//!   1` default (`footstep_material`, `Floor::NotFound`). Settles: a
//!   footstep request log on a stone floor (e.g. the Cathedral).
//! - **REC-432** (order of units in one update): ascending unit key; the
//!   original walks the room unit lists. Settles: a request log of two
//!   monsters idling in one update.
//! - **REC-433** (weapon hit class of a player): the right-hand body item
//!   (body location 4) by its code in `weapons` `hit class`; the weapon
//!   swap slots and a 2nd-hand weapon are not read. Settles: a swing
//!   request log with a bow and a sword.
//! - **REC-434** (the first sight of a unit): a monster seen for the
//!   first time plays `Init` and the sounds of the states it already has;
//!   an item first seen dropping (mode 5) plays the drop sound. The
//!   original runs them at the creation call the model does not
//!   distinguish from a level load. Settles: a request log of a level
//!   entry.
//! - **REC-435** (missile `HitSound`, `ProgSound`): they need the result
//!   of the client hit / progressive function, which the model does not
//!   run; not requested. Settles: the missile client functions in the
//!   model.
//! - **REC-1683** (skill start sounds, `triggers.md` §8 r1): a unit's
//!   mode request with code 0x15 or 0x16 (`model.md` §8 r4: the client
//!   skill start, record entry 0 = the skill) plays the skill's start
//!   sounds after the mode sounds, the start function taken as having
//!   returned non-zero (the model does not run `cltstfunc`). Settles: a
//!   request log of a cast with a start function that fails.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use d2_data::bin::TableFiles;
use d2_data::tables::{
    decode_all, Armor, Misc, Missiles, Monsounds, Monstats, Monstats2, Skills, States,
    Superuniques, Weapons,
};
use d2_formats::animdata::AnimData;

use crate::audio::triggers::identity::{monsounds_row, RecordInputs};
use crate::audio::triggers::modes::mode_set;
use crate::audio::triggers::movement::{
    footstep, footstep_called, footstep_material, init_voice, neutral, Floor, TOWN_LEVELS,
};
use crate::audio::triggers::skills::{skill_start, state_off, state_on, SkillStart};
use crate::audio::triggers::{within_700, Ctx, TriggerError, Unit, UnitSound, MONSTER, PLAYER};
use crate::bridge::items::{self, mode as item_mode};
use crate::bridge::world::{ClientUnit, ClientWorld, KindData, UnitKey, ITEM, MISSILE};
use crate::world_view::unit_assets::{unit_cof, UnitLooks};

/// The `monstats` columns the sound identity reads (`triggers-2.md`
/// §18 r3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MonsterSound {
    base_id: i32,
    monsound: i32,
    umonsound: i32,
    /// The `monstats2` row (`MonStatsEx`).
    ex: usize,
}

/// The sound columns of a `skills` row used by event 12.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StateSound {
    on: i32,
    off: i32,
    notondead: bool,
}

/// The sound columns of an item row (`dropsound`, `dropsfxframe`,
/// `usesound`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ItemSoundRow {
    pub dropsound: i32,
    pub dropsfxframe: u32,
    pub usesound: i32,
}

/// A link16 as the signed id the rules compare (an unset link is all
/// ones, so −1).
fn s16(v: u16) -> i32 {
    i32::from(v as i16)
}

/// The tables the unit sounds read, from the user's own files.
#[derive(Clone, Default)]
pub struct UnitSoundRows {
    monsounds: Vec<Monsounds>,
    monstats: Vec<MonsterSound>,
    critter: Vec<bool>,
    superunique_monsound: Vec<i32>,
    skill_stsound: Vec<i32>,
    /// The start-sound columns of each `skills` row (`triggers.md` §8 r1).
    skill_start: Vec<SkillStart>,
    states: Vec<StateSound>,
    /// `TravelSound` of each missile row.
    missile_travel: Vec<i32>,
    /// `hit class` of each weapon, by code.
    weapon_hit_class: BTreeMap<[u8; 4], u8>,
    item_sounds: BTreeMap<[u8; 4], ItemSoundRow>,
    looks: Option<Arc<UnitLooks>>,
    anim: Option<Arc<AnimData>>,
}

impl std::fmt::Debug for UnitSoundRows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UnitSoundRows({} monsounds, {} monstats)",
            self.monsounds.len(),
            self.monstats.len()
        )
    }
}

impl UnitSoundRows {
    /// The rows of the install behind `archives`; `looks` and `anim` give
    /// the animation (frames, speed) of a unit's mode (REC-430).
    pub fn live(
        archives: &dyn TableFiles,
        looks: Arc<UnitLooks>,
        anim: Arc<AnimData>,
    ) -> Result<Self, String> {
        let set = d2_data::bin::load_from(archives, d2_data::bin::DEFAULT_LANGUAGE)
            .map_err(|e| e.to_string())?;
        let table = |name: &str| set.table(name).ok_or(format!("{name} not loaded"));
        let monsounds: Vec<Monsounds> =
            decode_all(table("monsounds")?).map_err(|e| e.to_string())?;
        let monstats: Vec<Monstats> = decode_all(table("monstats")?).map_err(|e| e.to_string())?;
        let monstats2: Vec<Monstats2> =
            decode_all(table("monstats2")?).map_err(|e| e.to_string())?;
        let supers: Vec<Superuniques> =
            decode_all(table("superuniques")?).map_err(|e| e.to_string())?;
        let skills: Vec<Skills> = decode_all(table("skills")?).map_err(|e| e.to_string())?;
        let states: Vec<States> = decode_all(table("states")?).map_err(|e| e.to_string())?;
        let missiles: Vec<Missiles> = decode_all(table("missiles")?).map_err(|e| e.to_string())?;
        let weapons: Vec<Weapons> = decode_all(table("weapons")?).map_err(|e| e.to_string())?;
        let armor: Vec<Armor> = decode_all(table("armor")?).map_err(|e| e.to_string())?;
        let misc: Vec<Misc> = decode_all(table("misc")?).map_err(|e| e.to_string())?;
        let mut item_sounds = BTreeMap::new();
        let mut put = |code: [u8; 4], d: u16, f: u8, u: u16| {
            item_sounds.insert(
                code,
                ItemSoundRow {
                    dropsound: s16(d),
                    dropsfxframe: u32::from(f),
                    usesound: s16(u),
                },
            );
        };
        for w in &weapons {
            put(w.code, w.dropsound, w.dropsfxframe, w.usesound);
        }
        for a in &armor {
            put(a.code, a.dropsound, a.dropsfxframe, a.usesound);
        }
        for m in &misc {
            put(m.code, m.dropsound, m.dropsfxframe, m.usesound);
        }
        Ok(Self {
            monsounds,
            monstats: monstats
                .iter()
                .map(|m| MonsterSound {
                    base_id: i32::from(m.baseid),
                    monsound: s16(m.monsound),
                    umonsound: s16(m.umonsound),
                    ex: usize::from(m.monstatsex),
                })
                .collect(),
            critter: monstats2.iter().map(|m| m.critter).collect(),
            superunique_monsound: supers.iter().map(|s| s.monsound as i32).collect(),
            skill_stsound: skills.iter().map(|s| s16(s.stsound)).collect(),
            skill_start: skills
                .iter()
                .map(|s| SkillStart {
                    stsound: s16(s.stsound),
                    stsoundclass: s16(s.stsoundclass),
                    stsounddelay: s.stsounddelay,
                    weaponsnd: s.weaponsnd,
                    stsuccessonly: s.stsuccessonly,
                    charclass: i32::from(s.charclass as i8),
                    item_cast_sound: None,
                })
                .collect(),
            states: states
                .iter()
                .map(|s| StateSound {
                    on: s16(s.onsound),
                    off: s16(s.offsound),
                    notondead: s.notondead,
                })
                .collect(),
            missile_travel: missiles.iter().map(|m| s16(m.travelsound)).collect(),
            weapon_hit_class: weapons.iter().map(|w| (w.code, w.hit_class)).collect(),
            item_sounds,
            looks: Some(looks),
            anim: Some(anim),
        })
    }

    /// `0x004CA410`: the `monsounds` row of a monster (`triggers-2.md`
    /// §18 r3). The sound identity of a monster is its own type and class.
    pub fn record(&self, class: u32, flags: u8, hc_idx: u16) -> Option<&Monsounds> {
        self.monsounds.get(self.record_index(class, flags, hc_idx)?)
    }

    /// The row index of [`Self::record`].
    pub fn record_index(&self, class: u32, flags: u8, hc_idx: u16) -> Option<usize> {
        let row = self.monstats.get(class as usize);
        let inputs = RecordInputs {
            raw_type: MONSTER,
            class: class as i32,
            type_flags: flags,
            superunique_monsound: self.superunique_monsound.get(usize::from(hc_idx)).copied(),
            umonsound: row.map_or(0, |r| r.umonsound),
            monsound: row.map_or(-1, |r| r.monsound),
            monstats_rows: self.monstats.len() as i32,
            monsounds_rows: self.monsounds.len() as i32,
        };
        usize::try_from(monsounds_row(&inputs)?).ok()
    }

    fn base_class(&self, class: u32) -> i32 {
        self.monstats
            .get(class as usize)
            .map_or(class as i32, |m| m.base_id)
    }

    fn critter(&self, class: u32) -> bool {
        self.monstats
            .get(class as usize)
            .and_then(|m| self.critter.get(m.ex))
            .copied()
            .unwrap_or(false)
    }

    /// (`onsound`, `offsound`) of a state.
    pub fn state_sounds(&self, state: u8) -> Option<(i32, i32)> {
        self.states.get(usize::from(state)).map(|s| (s.on, s.off))
    }

    /// `TravelSound` of a missile row.
    pub fn missile_travel(&self, class: u32) -> Option<i32> {
        self.missile_travel.get(class as usize).copied()
    }

    /// The sound columns of an item code.
    pub fn item_sound(&self, code: [u8; 4]) -> Option<ItemSoundRow> {
        self.item_sounds.get(&code).copied()
    }

    /// The start-sound columns of a skill (`triggers.md` §8 r1).
    pub fn skill_start_row(&self, skill: u16) -> Option<SkillStart> {
        self.skill_start.get(usize::from(skill)).copied()
    }

    /// `stsound` of a skill (event 12).
    pub fn skill_stsound(&self, skill: u16) -> i32 {
        self.skill_stsound
            .get(usize::from(skill))
            .copied()
            .unwrap_or(0)
    }

    /// The (frame count F, speed) of the animation of `u` in its mode, as
    /// the 8.8 values the footstep rule reads (REC-430).
    pub fn animation(&self, u: &ClientUnit) -> Option<(u32, i32)> {
        let (looks, anim) = (self.looks.as_ref()?, self.anim.as_ref()?);
        let name = unit_cof(looks, u)?.short().to_ascii_uppercase();
        let b = name.as_bytes();
        if b.len() > 8 {
            return None;
        }
        let mut key = [0u8; 8];
        key[..b.len()].copy_from_slice(b);
        let info = anim.info(&key).ok()?;
        info.found
            .then(|| (info.frames.wrapping_mul(256), i32::from(info.speed as i16)))
    }
}

/// A player's animation speed +0x4C (`sim/units.md` §4.7, through
/// `d2_sim::units::anim_rate`): draw type 0, the AnimData speed `s`, the
/// unit's totals of stats 67–69, w = 101 in mode 3 (run) else 213 (step
/// 6), the mode row's V column as the velocity-mode test (`pathing.md`
/// §8.1 r2). Recorded: the amazon's TW at 213 (AnimData 256).
fn player_rate(world: &ClientWorld, key: UnitKey, u: &ClientUnit, s: i32) -> i32 {
    use d2_sim::units::anim_rate::{anim_rate, mode_row, Rate, RateInput};
    let m = u.mode;
    // A model player without a base stat reads the creation base 100
    // under its lists, as the walk prediction (`bridge::predict`).
    let total = |stat: u16| {
        let base = if u.stats.contains_key(&stat) { 0 } else { 100 };
        base + world.total(key, stat, 0)
    };
    let i = RateInput {
        applies: true,
        t: 0,
        c: u.class,
        m,
        s,
        velocitypercent: total(67),
        attackrate: total(68),
        other_animrate: total(69),
        has_path: true,
        w: if m == 3 { 101 } else { 213 },
        velocity_mode: mode_row(0, u.class, m).v,
        ..RateInput::default()
    };
    match anim_rate(&i) {
        Ok(Rate::Set { speed, .. }) => speed,
        _ => s,
    }
}

/// What the pass remembers of a unit between frames.
#[derive(Clone, Debug, Default)]
struct Track {
    mode: u32,
    /// +0x44 / +0x48 / +0x4C (REC-430).
    frame: u32,
    frame_count: u32,
    speed: i32,
    states: BTreeSet<u8>,
    /// `ClientUnit::mode_requests` at the last pass (REC-1683).
    mode_requests: u32,
}

/// One unit of a frame with the inputs the rules read.
struct Planned {
    key: UnitKey,
    first: bool,
    mode_changed: bool,
    states_on: Vec<u8>,
    states_off: Vec<u8>,
    is_local: bool,
    base_class: i32,
    critter: bool,
    dying: bool,
    state_146: bool,
    frozen: bool,
    weapon_hit_class: u8,
    near_local: bool,
    local_dist: i32,
    record: Option<usize>,
    item: Option<(Option<ItemSoundRow>, u8)>,
    /// The skill of a client skill start request (code 0x15 / 0x16, record
    /// entry 0) the unit received since the last pass (REC-1683).
    skill_request: Option<u16>,
}

/// The per-unit sound pass state.
#[derive(Default)]
pub struct UnitFeed {
    rows: Option<Arc<UnitSoundRows>>,
    tracks: BTreeMap<UnitKey, Track>,
}

impl UnitFeed {
    pub fn set_rows(&mut self, rows: Arc<UnitSoundRows>) {
        self.rows = Some(rows);
    }

    pub fn rows(&self) -> Option<&Arc<UnitSoundRows>> {
        self.rows.as_ref()
    }

    /// The monster's `monsounds` record for the event rules (events 16,
    /// 17).
    pub fn event_record(&self, world: &ClientWorld, key: UnitKey) -> Option<Monsounds> {
        let rows = self.rows.as_ref()?;
        let u = world.units.get(&key)?;
        if key.unit_type != MONSTER {
            return None;
        }
        let KindData::Monster(m) = &u.kind else {
            return rows.record(u.class, 0, 0).cloned();
        };
        rows.record(u.class, m.flags, m.hc_idx).cloned()
    }

    /// Event 12 (`triggers.md` OQ 4): the `stsound` of the skill in stat
    /// 350 of U's state-68 stat list, when U has that skill; 0 otherwise.
    pub fn event12_stsound(&self, world: &ClientWorld, key: UnitKey) -> i32 {
        let Some(rows) = self.rows.as_ref() else {
            return 0;
        };
        let Some(u) = world.units.get(&key) else {
            return 0;
        };
        let Some(list) = u.state_lists.get(&68) else {
            return 0;
        };
        let skill = list
            .iter()
            .find(|((stat, _), _)| *stat == 350)
            .map(|(_, &v)| v);
        let Some(skill) = skill.and_then(|v| u16::try_from(v).ok()) else {
            return 0;
        };
        let has = u
            .skills
            .as_ref()
            .is_some_and(|l| l.entries.iter().any(|e| e.skill == skill));
        if has {
            rows.skill_stsound(skill)
        } else {
            0
        }
    }

    /// The unit as the rules of an event read it (`triggers.md` §2 r2).
    pub fn event_unit<'a>(
        &self,
        world: &ClientWorld,
        key: UnitKey,
        class: u32,
        record: Option<&'a Monsounds>,
    ) -> Unit<'a> {
        let mut u = Unit::new(key, class as i32);
        u.is_local = world.local_player == Some(key);
        if let Some(c) = world.units.get(&key) {
            if u.is_local {
                u.mode = c.mode as u8;
            }
        }
        u.monsounds = record;
        u
    }

    /// Plans a frame: every sound-relevant unit with the changes since the
    /// last frame. `position` is the sound world's client pixel point.
    fn plan(
        &mut self,
        world: &ClientWorld,
        position: &dyn Fn(UnitKey) -> Option<(i32, i32)>,
        drawn: Option<(UnitKey, u32)>,
    ) -> Vec<Planned> {
        let Some(rows) = self.rows.clone() else {
            return Vec::new();
        };
        let local = world.local_player;
        let local_at = local.and_then(position);
        let item_views: BTreeMap<UnitKey, items::ItemView> = items::items(world)
            .into_iter()
            .map(|v| (v.key, v))
            .collect();
        // REC-433: the right-hand body item of the local player.
        let local_weapon = local.and_then(|p| {
            item_views
                .values()
                .find(|v| v.owner == Some(p) && v.mode == item_mode::BODY && v.body == 4)
                .and_then(|v| v.code)
                .and_then(|c| rows.weapon_hit_class.get(&c).copied())
        });
        let mut out = Vec::new();
        let mut live = BTreeSet::new();
        for (&key, u) in &world.units {
            if !matches!(key.unit_type, PLAYER | MONSTER | MISSILE | ITEM) {
                continue;
            }
            live.insert(key);
            let first = !self.tracks.contains_key(&key);
            let t = self.tracks.entry(key).or_default();
            let mode = effective_mode(u, drawn);
            let mode_changed = !first && t.mode != mode;
            let states_on: Vec<u8> = u
                .states
                .iter()
                .copied()
                .filter(|s| !t.states.contains(s))
                .collect();
            let states_off: Vec<u8> = t
                .states
                .iter()
                .copied()
                .filter(|s| !u.states.contains(s))
                .collect();
            let is_local = local == Some(key);
            let (near_local, local_dist) = match (position(key), local_at) {
                (Some((x, y)), Some((px, py))) => (
                    within_700(i64::from(x - px), i64::from(y - py)),
                    ((x - px).abs()).max((y - py).abs()),
                ),
                _ => (false, 0),
            };
            let mut p = Planned {
                key,
                first,
                mode_changed,
                states_on,
                states_off,
                is_local,
                base_class: u.class as i32,
                critter: false,
                dying: u.is_dead(),
                state_146: u.states.contains(&146),
                frozen: u.states.contains(&1),
                weapon_hit_class: 0,
                near_local,
                local_dist,
                record: None,
                item: None,
                skill_request: skill_start_request(t.mode_requests, u),
            };
            match key.unit_type {
                MONSTER => {
                    p.base_class = rows.base_class(u.class);
                    p.critter = rows.critter(u.class);
                    let (flags, hc) = match &u.kind {
                        KindData::Monster(m) => (m.flags, m.hc_idx),
                        _ => (0, 0),
                    };
                    p.record = rows.record_index(u.class, flags, hc);
                }
                PLAYER if is_local => p.weapon_hit_class = local_weapon.unwrap_or(0),
                ITEM => {
                    let row = item_views
                        .get(&key)
                        .and_then(|v| v.code)
                        .and_then(|c| rows.item_sounds.get(&c).copied());
                    p.item = Some((row, u.mode as u8));
                }
                _ => {}
            }
            // Advance the remembered state (the animation restarts with a
            // new mode, REC-430).
            if first || mode_changed {
                if key.unit_type == MONSTER && u.frame_count > 0 && u.mode == mode {
                    // The model's own animation (`bridge::monster_anim`):
                    // the §4.7 rate and, for a monster first seen, the
                    // first frame rolled on its seed (`msg-units.md`
                    // §1.2 r6.5).
                    t.frame = u.frame as u32;
                    t.frame_count = u.frame_count as u32;
                    t.speed = u.speed.unwrap_or(0);
                } else {
                    let mut shown = u.clone();
                    shown.mode = mode;
                    let (f, s) = rows.animation(&shown).unwrap_or((0, 0));
                    t.frame = 0;
                    t.frame_count = f;
                    t.speed = if key.unit_type == PLAYER {
                        player_rate(world, key, &shown, s)
                    } else {
                        s
                    };
                }
            }
            t.mode = mode;
            t.mode_requests = u.mode_requests;
            t.states = u.states.clone();
            out.push(p);
        }
        self.tracks.retain(|k, _| live.contains(k));
        out
    }

    /// One audio frame's unit pass: the changes since the last frame at
    /// `cs[0]` (the frame's first client update, or its only one), then
    /// the per-update voices and footsteps for each C in `cs`.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        cx: &mut Ctx,
        world: &ClientWorld,
        position: &dyn Fn(UnitKey) -> Option<(i32, i32)>,
        drawn: Option<(UnitKey, u32)>,
        material1: i32,
        updates: &[u32],
        unit_sounds: &mut BTreeMap<(UnitKey, bool), UnitSound>,
    ) -> Result<(), TriggerError> {
        let plan = self.plan(world, position, drawn);
        let Some(rows) = self.rows.clone() else {
            return Ok(());
        };
        let in_town = world
            .player_level()
            .is_some_and(|l| TOWN_LEVELS.contains(&i32::from(l)));
        let c0 = cx.c;
        for p in &plan {
            let Some(cu) = world.units.get(&p.key) else {
                continue;
            };
            let record = p.record.and_then(|r| rows.monsounds.get(r));
            let mode = effective_mode(cu, drawn) as u8;
            let mut u = Unit::new(p.key, cu.class as i32);
            u.base_class = p.base_class;
            u.mode = mode;
            u.is_local = p.is_local;
            u.monsounds = record;
            u.critter = p.critter;
            u.state_146 = p.state_146;
            u.frozen = p.frozen;
            u.dying = p.dying;
            u.weapon_hit_class = p.weapon_hit_class;
            u.in_town = in_town;
            u.near_local = p.near_local;
            u.local_dist = p.local_dist;
            let t = self.tracks.entry(p.key).or_default();
            u.frame = t.frame;
            u.frame_count = t.frame_count;
            u.speed = t.speed;
            let us = unit_sounds.entry((p.key, false)).or_default();
            cx.c = updates.first().copied().unwrap_or(c0);
            match p.key.unit_type {
                PLAYER | MONSTER => {
                    if p.key.unit_type == MONSTER && p.first {
                        init_voice(cx, &u, us);
                    }
                    let start = p.skill_request.and_then(|k| rows.skill_start_row(k));
                    if p.mode_changed {
                        mode_set(cx, &u, us, mode, start.as_ref().map(|k| (k, true)))?;
                    } else if let Some(k) = start {
                        skill_start(cx, &u, &k, true)?;
                    }
                }
                MISSILE if p.first => {
                    if let Some(id) = rows.missile_travel(cu.class) {
                        cx.unit_request(id, p.key);
                    }
                }
                ITEM => item_sound(cx, p),
                _ => {}
            }
            for &s in &p.states_on {
                let st = rows.states.get(usize::from(s)).copied().unwrap_or_default();
                let dead = cu.is_dead();
                state_on(cx, p.key, false, st.notondead, dead, st.on);
            }
            for &s in &p.states_off {
                let st = rows.states.get(usize::from(s)).copied().unwrap_or_default();
                state_off(cx, p.key, true, st.off);
            }
            if !matches!(p.key.unit_type, PLAYER | MONSTER) {
                continue;
            }
            // Per client update (recorded, REC-430 settled): a monster's
            // update advances f before its idle voice and footstep; a
            // player's footstep (`0x004CAF60` from `0x00463390`) reads f
            // before that update's advance.
            for &c in updates {
                cx.c = c;
                let t = self.tracks.entry(p.key).or_default();
                let before = t.frame;
                if t.frame_count > 0 {
                    t.frame = (t.frame as i32).wrapping_add(t.speed) as u32;
                    if t.frame >= t.frame_count {
                        t.frame %= t.frame_count;
                    }
                }
                u.frame = if p.key.unit_type == PLAYER {
                    before
                } else {
                    t.frame
                };
                let us = unit_sounds.entry((p.key, false)).or_default();
                if p.key.unit_type == MONSTER {
                    neutral(cx, &u, us);
                }
                // PROVISIONAL (REC-1682): a monster's footstep reads f
                // before the update's advance like a player's (measured:
                // audio-town-ambience-ama, the NPC footsteps came one
                // update early after the sound tick base moved to T 0).
                u.frame = before;
                if footstep_called(p.key.unit_type, cu.class as i32, mode)
                    && !(p.key.unit_type == PLAYER && cu.class >= 7)
                {
                    // REC-431.
                    let k = footstep_material(material1, Floor::NotFound);
                    footstep(cx, &u, us, k)?;
                }
            }
        }
        cx.c = c0;
        Ok(())
    }
}

/// The mode the unit sounds read: the model's, except the local player's
/// drawn mode while the preview walks it (REC-51: the client's own path
/// code is not in the model, so the prediction's walk / run mode stands
/// for the client's mode 2 / 3).
fn effective_mode(u: &ClientUnit, drawn: Option<(UnitKey, u32)>) -> u32 {
    match drawn {
        Some((k, m)) if k == u.key => m,
        _ => u.mode,
    }
}

/// The item sounds of a mode change (§9 r1, r2): the cursor pickup and the
/// drop (REC-434 for a first sight).
fn item_sound(cx: &mut Ctx, p: &Planned) {
    let Some((row, mode)) = p.item else {
        return;
    };
    let entered = p.mode_changed || p.first;
    if !entered {
        return;
    }
    match mode {
        // On the cursor: 235 `item_pickup`, none (a change only).
        4 if p.mode_changed => {
            cx.s.request(235, None, 0, 0, 0);
        }
        // Dropping: 216 `item_flippy`, then the base row's drop sound with
        // its frame delay (0 → 12), volume 180.
        5 => {
            cx.unit_request(216, p.key);
            if let Some(r) = row.filter(|r| r.dropsound > 0) {
                let d = if r.dropsfxframe == 0 {
                    12
                } else {
                    r.dropsfxframe
                };
                let h = cx.s.request(r.dropsound, Some(p.key), d, 0, 0);
                cx.s.set_volume(h, 180);
            }
        }
        _ => {}
    }
}

/// The skill whose start sounds a unit's new mode request plays
/// (REC-1683, `triggers.md` §8 r1): the last request when the count moved
/// and its code is 0x15 / 0x16, record entry 0 the skill. The local
/// player's own click request (level 0) is the one the original starts
/// from (`skills/sequences.md` local player rules 1-2); the server sends
/// the own client no second request (rule 3), so no request is skipped.
fn skill_start_request(seen: u32, u: &ClientUnit) -> Option<u16> {
    (seen != u.mode_requests)
        .then_some(u.last_mode_request)
        .flatten()
        .filter(|r| matches!(r.code, 0x15 | 0x16))
        .and_then(|r| u16::try_from(r.record[0]).ok())
}

#[cfg(test)]
mod rate_tests {
    use super::*;

    // Covers: specs/sim/units.md §4.7
    #[test]
    fn a_players_town_walk_steps_at_213_as_recorded() {
        // facts/client/anim/a1-town-walk-ama.tsv (Wine, REC-430): the
        // amazon's TW (mode 6, AnimData 256, F 2048) from its mode set:
        // f = 0, 213, 426, 639, …, 1917, then 82 (wrap); TN (mode 5,
        // AnimData speed 80) at 80.
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.mode = 6;
        let mut w = ClientWorld::default();
        w.units.insert(key, u.clone());
        let s = player_rate(&w, key, &u, 256);
        assert_eq!(s, 213);
        let (mut f, mut seen) = (0i32, vec![0]);
        for _ in 0..10 {
            f = (f + s) % 2048;
            seen.push(f);
        }
        assert_eq!(&seen[..4], &[0, 213, 426, 639]);
        assert_eq!(&seen[9..], &[1917, 82]);
        u.mode = 5;
        assert_eq!(player_rate(&w, key, &u, 80), 80);
    }
}

#[cfg(test)]
mod skill_start_tests {
    use super::*;
    use crate::bridge::world::ModeRequest;

    // Covers: specs/skills/sequences.md §3
    #[test]
    fn the_local_players_click_request_at_level_0_plays_its_start() {
        let mut u = ClientUnit::new(UnitKey::new(PLAYER, 1));
        u.mode_requests = 1;
        u.last_mode_request = Some(ModeRequest {
            code: 0x15,
            record: [44, 1, 5, 5, 0, 0, 0],
        });
        assert_eq!(skill_start_request(0, &u), Some(44));
        assert_eq!(skill_start_request(1, &u), None);
    }
}
