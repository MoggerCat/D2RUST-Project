// Spec: specs/sim/units.md §1–§6; specs/sim/stat-lists.md §10
//! The seams of the unit code: [`Sim`], the state a unit operation
//! works on; [`UnitData`], the tables it reads; and [`UnitHooks`], every
//! call into a system another spec owns (animation rate, path, AI,
//! skills, objects, items, missiles, trade, messages). Each hook's
//! default is the narrowest reading of the spec: nothing happens, or
//! the value that makes the caller do nothing. The provider of each is
//! named in its doc.

use d2_data::bin::BinTable;
use d2_data::tables::{decode_all, Monstats, Monstats2, WrongTable};

use crate::game::Game;
use crate::stats::{StatHost, StatLists};

use super::record::{AnimRecord, Sequence, Units};
use super::UnitId;

/// The monstats facts the unit code reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MonsterInfo {
    /// `enabled` (bit 25 of +0x0C).
    pub enabled: bool,
    /// `aidel`, `aidel(N)`, `aidel(H)` (+0x4F, +0x50, +0x51).
    pub aidel: [u8; 3],
    /// The monstats2 move bits (+0x104, bit = mode) of the class's
    /// `MonStatsEx` row.
    pub moves: u32,
}

/// Tables and game settings the unit code reads.
#[derive(Clone, Debug, Default)]
pub struct UnitData {
    /// monstats rows by class.
    pub monsters: Vec<MonsterInfo>,
    /// Game +0x6D.
    pub difficulty: u8,
    /// Game +0x6A or game +0x74 non-zero (§4.6; open question 7).
    pub aidel_by_difficulty: bool,
    /// Game +0x70 non-zero.
    pub expansion: bool,
}

impl UnitData {
    /// From the monstats and monstats2 tables.
    pub fn new(monstats: &BinTable, monstats2: &BinTable) -> Result<Self, WrongTable> {
        let m2 = decode_all::<Monstats2>(monstats2)?;
        let monsters = decode_all::<Monstats>(monstats)?
            .iter()
            .map(|m| {
                let moves = m2.get(usize::from(m.monstatsex)).map_or(0, |x| {
                    [
                        (4, x.a1mv),
                        (5, x.a2mv),
                        (7, x.scmv),
                        (8, x.s1mv),
                        (9, x.s2mv),
                        (10, x.s3mv),
                        (11, x.s4mv),
                    ]
                    .iter()
                    .filter(|(_, b)| *b)
                    .fold(0u32, |a, (bit, _)| a | 1 << bit)
                });
                MonsterInfo {
                    enabled: m.enabled,
                    aidel: [m.aidel, m.aidel_n, m.aidel_h],
                    moves,
                }
            })
            .collect();
        Ok(Self {
            monsters,
            ..Self::default()
        })
    }

    pub fn monster(&self, class: u32) -> Option<&MonsterInfo> {
        self.monsters.get(class as usize)
    }
}

/// What a unit operation works on.
pub struct Sim<'a> {
    pub game: &'a mut Game,
    pub units: &'a mut Units,
    pub stats: &'a mut StatLists,
    pub data: &'a UnitData,
}

/// Calls into systems other specs own. Also the [`StatHost`] of the
/// stat writes the unit code makes.
#[allow(unused_variables)]
pub trait UnitHooks: StatHost {
    // ---- animation (animation-rate and sequence specs; units.md OQ2) ----

    /// `0x00620F00`: the unit's AnimData record for its mode.
    fn anim_record(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<AnimRecord> {
        None
    }

    /// `0x00623F50`: the animation rate (+0x4C).
    fn anim_rate(&mut self, sim: &Sim<'_>, unit: UnitId) -> i16 {
        0
    }

    /// The path-velocity half of `0x00623F50` (`sim/pathing.md` §8.1),
    /// run right after [`UnitHooks::anim_rate`]. Provider: path spec.
    fn anim_velocity(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// `0x00623B10`: the frame bonus (start index of §4.2).
    fn frame_bonus(&mut self, sim: &Sim<'_>, unit: UnitId) -> i32 {
        0
    }

    /// Sequence load for player mode 18 / monster mode 14
    /// (`0x006634C0`); `None` when the unit has no sequence.
    fn load_sequence(&mut self, sim: &Sim<'_>, unit: UnitId) -> Option<Sequence> {
        None
    }

    /// The unit has a path (+0x2C). Provider: path spec.
    fn has_path(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        false
    }

    /// `0x006272E0`, `0x00624390`: the rest of the animation-field
    /// re-initialisation at a mode change. Provider: animation spec.
    fn reinit_anim(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    // ---- combat, rooms ------------------------------------------------

    /// `0x0057C980`: drop the unit's own entries from its combat list.
    /// Provider: combat.
    fn drop_combat_entries(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// `0x0061AB00` on the unit's room (town test for player modes;
    /// monster regeneration, stat-lists.md OQ2). Provider: DRLG/world.
    fn room_flag(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        false
    }

    // ---- players (units.md §4.5, §6.1) --------------------------------

    /// `0x0057EDD0` / `0x0057EEC0`: the mode request check. Provider:
    /// player/path spec. Default: accepted.
    fn player_request_check(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) -> bool {
        true
    }

    /// `0x00580EC0` before the death animation: death bookkeeping.
    /// Provider: player/combat spec.
    fn player_death(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// `0x0057FCA0` corpse mode: corpse and character save. Provider:
    /// player spec.
    fn player_corpse(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// `0x0057FB70(game, player, corpse)`: the corpse pickup of C→S 0x16
    /// type 0 (`items/inventory-moves.md` §7.1, `combat/vitals.md` §4.7
    /// rule 2). Provider: player/combat spec.
    fn player_corpse_pickup(&mut self, sim: &mut Sim<'_>, player: UnitId, corpse: UnitId) {}

    /// Knockback start: path values 8 and 5. Provider: path spec.
    fn player_knockback_path(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// `0x0056FAF0` after an attack/cast/skill mode start. Provider:
    /// skills.
    fn player_skill_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// `0x00580C20`: one movement step; the action result (2 runs the
    /// ENDANIM handler at once). Provider: path spec.
    fn player_movement_step(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        1
    }

    /// `0x00580460`: the action frame of attack, cast and skill modes;
    /// the action result. Provider: skills.
    fn player_action_frame(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) -> u32 {
        1
    }

    /// `0x00643CE0`: the unit's item row is flagged (attack-mode cleanup
    /// at the end of the animation). Provider: items.
    fn player_item_row_flagged(&mut self, sim: &Sim<'_>, unit: UnitId) -> bool {
        false
    }

    /// `0x00580310`, `0x00580380`: attack-mode cleanup. Provider: skills.
    fn player_attack_cleanup(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// Event 11: party refresh `0x005406A0` and pet refresh
    /// `0x00575630`. Provider: party and pet specs.
    fn player_refresh(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// Event 13 `0x005689D0`. Provider: trade spec.
    fn update_trade(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {}

    // ---- monsters (units.md §4.6, §6.2) -------------------------------

    /// `0x005A7C20`: path and AI-state bookkeeping before a mode start
    /// (every mode but GH). Provider: monster spec.
    fn monster_mode_bookkeeping(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u32) {}

    /// Per-class mode records (`0x006E22D0`–`0x006E23A0`, classes with
    /// monstats +0x1A5): the record to use instead of the table's.
    /// Provider: monster spec.
    fn monster_class_record(
        &mut self,
        sim: &Sim<'_>,
        unit: UnitId,
        mode: u32,
    ) -> Option<super::modes::MonsterModeRecord> {
        None
    }

    /// A monster mode function at `address` (start functions return
    /// whether the mode started; event functions' results are
    /// ignored). The neutral start `0x005A73E0` is d2rs's own.
    /// Provider: monster spec. Default: started / nothing.
    fn monster_mode_function(&mut self, sim: &mut Sim<'_>, unit: UnitId, address: u32) -> bool {
        true
    }

    /// `0x005544B0(unit, 0)`, the state-54 check before an AI event is
    /// scheduled (`tick.md` §5.2 rule 4). Provider: monster spec.
    fn uninterruptable_check(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// Event 2 `0x005B1740`: AI think. Provider: AI spec.
    fn ai_think(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {}

    /// The umod dispatcher's mode-set sites inside `0x005A7C20`
    /// (`monsters/umod-callbacks.md` §2 rules 1–2): `mode` 0
    /// (`0x005A4350`, before the start function, never for GH) and 1
    /// (`0x005A4360`, after the animation prepare, before the cancel of
    /// events 0 / 1). Provider: monster spec (`monsters::init::dispatch`).
    fn monster_umods(&mut self, sim: &mut Sim<'_>, unit: UnitId, mode: u8) {}

    /// Event 7 `0x005A4370`. Provider: monster spec.
    fn monster_umod(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {}

    /// Event 10 `0x005A7F70`. Provider: AI spec.
    fn ai_reset(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {}

    /// Monster death by regeneration: `0x0057CCB0`(game, unit, killer)
    /// and the death events `0x005C0C30`. Provider: monster spec.
    fn monster_death(&mut self, sim: &mut Sim<'_>, unit: UnitId, killer: Option<UnitId>) {}

    // ---- skills (stat-lists.md §10.2, §10.3; units.md §6.1) -----------

    /// Event 5: active-state function `f` (< 191) of table
    /// `0x007322B0` (null entries: nothing). Provider: skills.
    fn active_state(&mut self, sim: &mut Sim<'_>, unit: UnitId, f: u16, skill: u32, a2: u32) {}

    /// Event 8 `0x0056FCB0`. Provider: skills.
    fn periodic_skills(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {}

    /// Event 9: aura application `0x0056F7F0`(game, unit, skill, l, 1, 1,
    /// 0) and `0x0056CE70`(game, unit, a1, skill, l, 0). Provider: skills.
    fn apply_item_aura(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        a1: u32,
        skill: u32,
        level: i32,
    ) {
    }

    /// Event 14 callback `0x00554570`: skill cooldown end. Provider:
    /// skills.
    fn cooldown_end(&mut self, sim: &mut Sim<'_>, unit: UnitId, a1: u32, a2: u32) {}

    // ---- other kinds ----------------------------------------------------

    /// Missile events (`0x005ADBB0`, every type): the server-do function.
    /// Provider: missile spec.
    fn missile_do(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// Object events 0–11 (handlers of `unit-handlers.tsv`). Provider:
    /// objects spec.
    fn object_event(&mut self, sim: &mut Sim<'_>, unit: UnitId, event: u8) {}

    /// Item event 3 `0x00562D30` (replenish, units.md §6.5). Provider:
    /// items; [`super::dispatch::replenish_delay`] gives the reschedule.
    fn item_replenish(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}

    /// Message `0x00571A10`(unit, fraction): the life fraction.
    /// Provider: messages (d2-server).
    fn send_life_fraction(&mut self, sim: &mut Sim<'_>, unit: UnitId, fraction: i32) {}

    /// Hover free (`0x00580B70`, `0x005A7F00` when timed out). Provider:
    /// hover/chat spec.
    fn free_hover(&mut self, sim: &mut Sim<'_>, unit: UnitId) {}
}
