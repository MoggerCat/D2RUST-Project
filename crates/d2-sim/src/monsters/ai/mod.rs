// Spec: specs/monsters/ai.md
//! Monster AI think: scheduling (§1), dispatch and prechecks (§2), the AI
//! control record and tables (§3), AI parameters (§4), target selection
//! (§5), distances (§6), tactics helpers (§7), commands (§8) and the
//! per-AI functions (§9) of the catalogue `ai-functions.tsv` (§10).
//!
//! Mode changes, movement, collision, target-node lists, skills and
//! damage are other specs'; they are reached through [`seams`]. AI timer
//! events (type 2 think, type 10 reset) plug into the tick through
//! [`MonsterDispatch`].

mod functions;
mod npc;
pub mod seams;
pub mod table;
mod tactics;
mod target;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_data::tables::{Levels, Monstats, Monstats2};

use crate::game::Game;
use crate::tick::timer::{TimerClass, TimerRun};
use crate::tick::EventDispatch;
use crate::units::{UnitId, UnitType};

pub use functions::{implemented, run_function};
pub use seams::{
    AiHost, AiModes, AiQuests, AiSkills, AiTargets, AiUnits, AiWorld, ModeTarget, PortalNpc,
};
pub use table::{AiRecord, AI_TABLE, SPECIAL_TABLE};
pub use tactics::*;
pub use target::{main_search, precheck_a, precheck_b, precheck_c};

/// Event types the AI handles (`tick.md` §5.6).
pub const EVENT_THINK: u8 = 2;
pub const EVENT_AI_RESET: u8 = 10;

/// Unit modes named by the AI (`sim/units.md`, §7.1).
pub mod mode {
    pub const DEATH: u8 = 0;
    pub const NEUTRAL: u8 = 1;
    pub const WALK: u8 = 2;
    pub const GETHIT: u8 = 3;
    pub const ATTACK1: u8 = 4;
    pub const ATTACK2: u8 = 5;
    pub const SKILL1: u8 = 8;
    pub const SKILL2: u8 = 9;
    pub const DEAD: u8 = 12;
    pub const SEQUENCE: u8 = 14;
    pub const RUN: u8 = 15;
}

/// State ids (`states.txt` rows) the AI reads.
pub mod state {
    pub const FREEZE: u16 = 1;
    pub const STUNNED: u16 = 21;
    pub const PREVENTHEAL: u16 = 52;
    pub const UNINTERRUPTABLE: u16 = 54;
    pub const DECREPIFY: u16 = 60;
}

/// AI control flags (control +0x08, §3.1).
pub mod flag {
    pub const TARGET_SEEN: u16 = 0x08;
    pub const BOSS_SOUND_DONE: u16 = 0x10;
    pub const MAY_TELEPORT: u16 = 0x20;
    pub const FORCE_LOS: u16 = 0x40;
}

/// A unit by (type, GUID), as the control stores owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitRef {
    pub ty: UnitType,
    pub guid: u32,
}

/// An AI command (D2MOO `D2AiCmdStrc`, §8): five params, param 0 = type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiCommand {
    pub params: [i32; 5],
}

/// The AI control record (monster data +0x28, D2MOO `D2AiControlStrc`,
/// §3.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiControl {
    /// +0x00.
    pub special_state: u32,
    /// +0x04: current function (1.14d address), 0 = none.
    pub function: u32,
    /// +0x08.
    pub flags: u16,
    /// +0x0C/+0x10: leash owner; `None` = GUID −1.
    pub owner: Option<UnitRef>,
    /// +0x14..+0x1C.
    pub params: [i32; 3],
    /// +0x20/+0x24: the command list; `commands[cur]` is the current one
    /// and the elements after it its successors.
    pub commands: Vec<AiCommand>,
    pub cur: usize,
    /// +0x2C/+0x30: minion owner (pack leader).
    pub minion_owner: Option<UnitRef>,
    /// +0x34: GUIDs of this leader's minions (written by population
    /// code, `monsters/population.md`).
    pub minions: Vec<u32>,
}

/// The AI param record's velocity request (monster data +0x2C, §7.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VelocityRequest {
    /// +0x18.
    pub method: i32,
    /// +0x1C.
    pub speed: i32,
    /// +0x20, capped at 77.
    pub steps: i32,
}

/// Per-monster AI state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MonsterAi {
    /// `None` until an AI is installed (monster data +0x28 = 0).
    pub control: Option<AiControl>,
    pub velocity: VelocityRequest,
}

/// A call the original makes that this code does not model yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unhandled {
    /// AI think/init/alternate function by address (stub; status other
    /// than `spec'd-here` in `ai-functions.tsv`, or a special-state
    /// function).
    Function { addr: u32, unit: UnitId },
    /// A think on a monster with no AI control.
    NoControl { unit: UnitId },
    /// A velocity request with a speed outside −126…126 (fatal assert in
    /// 1.14d).
    VelocityAssert { unit: UnitId, speed: i32 },
}

/// The AI system's state.
#[derive(Clone, Debug, Default)]
pub struct AiStore {
    units: BTreeMap<UnitId, MonsterAi>,
    pub unhandled: Vec<Unhandled>,
}

impl AiStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, u: UnitId) -> Option<&MonsterAi> {
        self.units.get(&u)
    }
    pub fn entry(&mut self, u: UnitId) -> &mut MonsterAi {
        self.units.entry(u).or_default()
    }
    pub fn control(&self, u: UnitId) -> Option<&AiControl> {
        self.units.get(&u)?.control.as_ref()
    }
    pub fn control_mut(&mut self, u: UnitId) -> Option<&mut AiControl> {
        self.units.get_mut(&u)?.control.as_mut()
    }
    /// Drops a removed unit's state.
    pub fn remove(&mut self, u: UnitId) {
        self.units.remove(&u);
    }
}

/// Game fields the AI reads (game +0x6A, +0x74, +0x6D).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameInfo {
    pub game_type: u8,
    pub game_type_ex: u32,
    /// 0 Normal, 1 Nightmare, 2 Hell.
    pub difficulty: u8,
}

/// The data tables the AI reads, typed `d2-data` records. `skill_modes`
/// are the compiled `Sk1mode..Sk3mode` bytes per monstats row (record
/// +0x180..+0x182; a callback column, so not in the typed record).
#[derive(Clone, Copy)]
pub struct AiTables<'a> {
    pub monstats: &'a [Monstats],
    pub monstats2: &'a [Monstats2],
    pub levels: &'a [Levels],
    pub skill_modes: &'a [[u8; 3]],
}

/// The `Sk1mode..Sk3mode` bytes of every `monstats.bin` record (+0x180).
pub fn skill_modes(t: &d2_data::bin::BinTable) -> Vec<[u8; 3]> {
    t.iter()
        .map(|r| {
            r.get(0x180..0x183)
                .and_then(|b| b.try_into().ok())
                .unwrap_or([0; 3])
        })
        .collect()
}

/// What AI code works with.
pub struct Ctx<'a, W: ?Sized> {
    pub tables: AiTables<'a>,
    pub info: GameInfo,
    pub store: &'a mut AiStore,
    pub world: &'a mut W,
}

/// The tick parameter record (D2MOO `D2AiTickParamStrc`, §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickParam {
    pub target: Option<UnitId>,
    pub distance: i32,
    pub combat: bool,
    /// Monstats row.
    pub class: usize,
    /// Monstats2 row.
    pub class2: usize,
}

impl<W: AiHost + ?Sized> Ctx<'_, W> {
    pub fn monstats(&self, class: i32) -> Option<&Monstats> {
        usize::try_from(class)
            .ok()
            .and_then(|c| self.tables.monstats.get(c))
    }

    /// `aipN` (1…8) of the difficulty (§4), signed.
    pub fn aip(&self, p: &TickParam, n: usize) -> i32 {
        let Some(r) = self.tables.monstats.get(p.class) else {
            return 0;
        };
        let v = match (n, self.info.difficulty) {
            (1, 0) => r.aip1,
            (1, 1) => r.aip1_n,
            (1, _) => r.aip1_h,
            (2, 0) => r.aip2,
            (2, 1) => r.aip2_n,
            (2, _) => r.aip2_h,
            (3, 0) => r.aip3,
            (3, 1) => r.aip3_n,
            (3, _) => r.aip3_h,
            (4, 0) => r.aip4,
            (4, 1) => r.aip4_n,
            (4, _) => r.aip4_h,
            (5, 0) => r.aip5,
            (5, 1) => r.aip5_n,
            (5, _) => r.aip5_h,
            (6, 0) => r.aip6,
            (6, 1) => r.aip6_n,
            (6, _) => r.aip6_h,
            (7, 0) => r.aip7,
            (7, 1) => r.aip7_n,
            (7, _) => r.aip7_h,
            (8, 0) => r.aip8,
            (8, 1) => r.aip8_n,
            (8, _) => r.aip8_h,
            _ => 0,
        };
        i32::from(v as i16)
    }

    /// `aidel` of §1.3: the Normal column unless game +0x6A or +0x74 is
    /// nonzero; 0 → 15.
    pub fn aidel(&self, class: i32) -> i32 {
        let Some(r) = self.monstats(class) else {
            return 15;
        };
        let v = if self.info.game_type == 0 && self.info.game_type_ex == 0 {
            r.aidel
        } else {
            match self.info.difficulty {
                0 => r.aidel,
                1 => r.aidel_n,
                _ => r.aidel_h,
            }
        };
        if v == 0 {
            15
        } else {
            i32::from(v)
        }
    }

    /// `aidist` of the difficulty (§4, no game-type gate); 0 → 35.
    pub fn aidist(&self, class: i32) -> i32 {
        let v = self
            .monstats(class)
            .map_or(0, |r| match self.info.difficulty {
                0 => r.aidist,
                1 => r.aidist_n,
                _ => r.aidist_h,
            });
        if v == 0 {
            35
        } else {
            i32::from(v)
        }
    }

    /// Base class (monstats `BaseId`).
    pub fn base_class(&self, unit: UnitId) -> i32 {
        let class = self.world.class(unit);
        self.monstats(class)
            .map_or(-1, |r| i32::from(r.baseid as i16))
    }

    /// Skill `n` (1…3) of the row as a signed id (< 0 = none) and its mode.
    pub fn skill(&self, p: &TickParam, n: usize) -> (i32, u8) {
        let Some(r) = self.tables.monstats.get(p.class) else {
            return (-1, 0);
        };
        let s = match n {
            1 => r.skill1,
            2 => r.skill2,
            _ => r.skill3,
        };
        let m = self.tables.skill_modes.get(p.class).map_or(0, |m| m[n - 1]);
        (i32::from(s as i16), m)
    }

    /// "P(v)": one step of the unit seed, `lo' % 100 < v` signed (§4).
    pub fn chance(&mut self, unit: UnitId, v: i32) -> bool {
        ((self.world.seed(unit).step() % 100) as i32) < v
    }

    /// AI state 3 or 19 (`0x005DD2B0`).
    pub fn ai_state_set(&self, unit: UnitId) -> bool {
        matches!(self.world.ai_state(unit), 3 | 19)
    }

    /// Whether the class can walk (`0x0046C140(class, 2)`).
    pub fn can_walk(&self, unit: UnitId) -> bool {
        let c = self.world.class(unit);
        self.world.class_has_mode(c, mode::WALK)
    }

    /// The AI table lookup `0x005B15D0` (§3.2).
    pub fn record(&self, unit: UnitId) -> AiRecord {
        let special = self.store.control(unit).map_or(0, |c| c.special_state);
        let class = self.world.class(unit);
        let row = self.monstats(class);
        if special != 0 && (special as usize) < SPECIAL_TABLE.len() {
            let gated = matches!(special, 10..=12);
            if !gated || row.is_some_and(|r| r.switchai) {
                return SPECIAL_TABLE[special as usize];
            }
        }
        let ai = row.map_or(0, |r| r.ai as usize);
        AI_TABLE.get(ai).copied().unwrap_or(AI_TABLE[0])
    }
}

/// Deletes the unit's pending thinks (`0x00540E60(2, 0)`).
pub fn delete_thinks(game: &mut Game, unit: UnitId) {
    game.timers.cancel_unit_events(unit, EVENT_THINK, None);
}

/// The first pending think's expire frame (D2MOO
/// `EVENT_GetEventFrame`), 0 when none.
pub fn pending_think(game: &Game, unit: UnitId) -> i32 {
    game.timers
        .unit_timers(unit)
        .into_iter()
        .find(|&t| game.timers.event(t).is_some_and(|e| e.0 == EVENT_THINK))
        .and_then(|t| game.timers.expire(t))
        .unwrap_or(0)
}

/// Schedules a think at `expire` (`0x005417D0`, type 2) with the
/// state-54 rule (`tick.md` §5.2 rule 4, §1.1): a monster with state 54
/// first runs `0x005544B0(unit, 0)`, which clears the state and cancels
/// all its type-2 events.
pub fn schedule_think<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    expire: i32,
) {
    if cx.world.has_state(unit, state::UNINTERRUPTABLE) {
        cx.world.clear_uninterruptable(game, unit);
        delete_thinks(game, unit);
    }
    // A monster has a timer class: the error cannot happen.
    let _ = game.schedule_event(unit, u32::from(EVENT_THINK), expire, None, 0, 0);
}

/// Delete thinks, then schedule one at frame + n.
fn reschedule<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId, n: i32) {
    delete_thinks(game, unit);
    let at = game.frame.wrapping_add(n);
    schedule_think(game, cx, unit, at);
}

/// `0x005DE080` `AITACTICS_IdleInNeutralMode` ("idle N", §1.2): N 0 → 1;
/// a non-neutral unit first gets a mode change to neutral targeting
/// itself; then delete + schedule.
pub fn idle<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId, n: i32) {
    let n = if n == 0 { 1 } else { n };
    if cx.world.anim_mode(unit) != mode::NEUTRAL {
        cx.world
            .change_mode(game, unit, mode::NEUTRAL, ModeTarget::Unit(unit));
    }
    reschedule(game, cx, unit, n);
}

/// `0x005DE0F0` `AITACTICS_Idle`: delete + schedule, mode unchanged.
pub fn idle_keep_mode<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    n: i32,
) {
    let n = if n == 0 { 1 } else { n };
    reschedule(game, cx, unit, n);
}

/// `0x005DE130`: as [`idle_keep_mode`] but only if no think is pending,
/// the pending one is due this frame, or it is due at frame + N or later.
pub fn idle_if_later<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    n: i32,
) {
    let n = if n == 0 { 1 } else { n };
    let pending = game
        .timers
        .unit_timers(unit)
        .into_iter()
        .find(|&t| game.timers.event(t).is_some_and(|e| e.0 == EVENT_THINK))
        .and_then(|t| game.timers.expire(t));
    let ok = match pending {
        None => true,
        Some(e) => e == game.frame || e >= game.frame.wrapping_add(n),
    };
    if ok {
        reschedule(game, cx, unit, n);
    }
}

/// The neutral mode's start `0x005A73E0` (§1.3), called by the mode
/// machinery when a monster enters neutral (also after a failed mode
/// start). Combat mode 1 is the caller's. Adds (no delete) a think at
/// frame + 45 when stunned, else + `aidel`, only if the pending think's
/// frame (0 when none) is ≤ the current frame.
pub fn neutral_mode_start<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId) {
    if pending_think(game, unit) > game.frame {
        return;
    }
    let delay = if cx.world.has_state(unit, state::STUNNED) {
        45
    } else {
        cx.aidel(cx.world.class(unit))
    };
    let at = game.frame.wrapping_add(delay);
    schedule_think(game, cx, unit, at);
}

/// The knockback mode's end `0x005A8520` (§1.2).
pub fn knockback_end<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId) {
    let class = cx.world.class(unit);
    let delay = if !cx.world.class_has_mode(class, mode::GETHIT) {
        1
    } else if cx.base_class(unit) == 78 {
        if cx.world.has_state(unit, state::STUNNED) {
            45
        } else {
            15
        }
    } else {
        cx.world.knockback_to_gethit(game, unit);
        return;
    };
    let at = game.frame.wrapping_add(delay);
    schedule_think(game, cx, unit, at);
}

/// `0x00573780` `MONSTER_UpdateAiCallbackEvent` (§1.2, §1.5).
pub fn update_ai_callback<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId) {
    delete_thinks(game, unit);
    let m = cx.world.anim_mode(unit);
    let schedule = match m {
        mode::NEUTRAL => true,
        mode::DEATH | mode::DEAD => false,
        _ => matches!(cx.base_class(unit), 110 | 118 | 136 | 247),
    };
    if schedule {
        let at = game.frame.wrapping_add(2);
        schedule_think(game, cx, unit, at);
    }
}

/// `0x0053A8E0` (§1.5 rule 2): the first client entered `room`; every
/// monster of the room's unit list gets [`update_ai_callback`]. The
/// room +0x78 < 2 test is the caller's.
pub fn client_entered_room<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    room: crate::units::RoomId,
) {
    for u in game.lists.room_units(room) {
        if game
            .lists
            .unit(u)
            .is_some_and(|e| e.ty == UnitType::Monster)
        {
            update_ai_callback(game, cx, u);
        }
    }
}

/// NPC interaction `0x00548B00` (`start`) / `0x0054CA10` (§1.2): for
/// classes with monstats `npc` and `interact`: stop the path; on start
/// AI param 0 := 40; delete + schedule +1.
pub fn npc_interaction<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    start: bool,
) {
    let class = cx.world.class(unit);
    if !cx.monstats(class).is_some_and(|r| r.npc && r.interact) {
        return;
    }
    cx.world.stop_path(unit);
    if start {
        if let Some(c) = cx.store.control_mut(unit) {
            c.params[0] = 40;
        }
    }
    reschedule(game, cx, unit, 1);
}

/// `0x0057B230` freeze apply (§1.2): delete + schedule at frame +
/// length + 1.
pub fn schedule_after_freeze<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: UnitId,
    length: i32,
) {
    reschedule(game, cx, unit, length.wrapping_add(1));
}

/// The freeze gate (§1.1, §1.4): state 1 on a living monster.
pub fn frozen<W: AiHost + ?Sized>(cx: &Ctx<'_, W>, unit: UnitId) -> bool {
    cx.world.has_state(unit, state::FREEZE) && !cx.world.is_dead(unit)
}

/// `0x005A8030`, the end of modes 3–9 and 14 (§1.4): walk/run ends and
/// the `SplEndGeneric` cases run the think inline; every other case
/// requests a mode change to neutral.
///
/// TODO(spec gap): for the `SplEndGeneric` cases the spec does not say
/// whether the anim mode is set to neutral first; it is not set here.
pub fn mode_end<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId, ended: u8) {
    const INLINE: [u8; 16] = [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
    if INLINE.get(ended as usize) == Some(&1) {
        cx.world.set_anim_mode(unit, mode::NEUTRAL);
        if !frozen(cx, unit) {
            think(game, cx, unit);
        }
        return;
    }
    let class = cx.world.class(unit);
    let generic = cx.monstats(class).is_some_and(|r| r.splendgeneric != 0);
    let base = cx.base_class(unit);
    let special = generic
        && match base {
            110 => ended == 8,
            247 => ended == 14,
            136 => matches!(ended, 10 | 11),
            230 | 231 | 403 => true,
            118 => ended == 2,
            _ => false,
        };
    if special {
        if !frozen(cx, unit) {
            think(game, cx, unit);
        }
        return;
    }
    cx.world
        .change_mode(game, unit, mode::NEUTRAL, ModeTarget::Unit(unit));
}

/// Installing an AI `0x005B0E00` (§3.3).
pub fn install<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId, state: u32) {
    // Step 1: fatal assert on state 54.
    assert!(
        !cx.world.has_state(unit, state::UNINTERRUPTABLE),
        "AI install on a unit with state 54 (ai.md §3.3 step 1)"
    );
    // Step 2.
    if state >= 18
        || game
            .lists
            .unit(unit)
            .is_none_or(|e| e.ty != UnitType::Monster)
    {
        return;
    }
    let class = cx.world.class(unit);
    let state = if usize::try_from(class).map_or(true, |c| c >= cx.tables.monstats.len()) {
        1
    } else {
        state
    };
    let Some(control) = cx.store.control(unit).cloned() else {
        // TODO(monsters/init.md): the control is allocated at creation;
        // without one there is nothing to install into.
        cx.store.unhandled.push(Unhandled::NoControl { unit });
        return;
    };
    // Step 3.
    if control.function != 0 {
        let cur = cx.record(unit);
        if cur.think == control.function && cur.alt != 0 {
            let c = cx.store.control_mut(unit).expect("control");
            c.special_state = state;
            c.function = cur.alt;
            return;
        }
    }
    // Step 4.
    {
        let c = cx.store.control_mut(unit).expect("control");
        c.params = [0; 3];
        c.commands.clear();
        c.cur = 0;
    }
    let rec = record_for(cx, unit, state);
    // The init function runs with a fresh tick record; nothing when either
    // record is missing. No init body is spec'd: logged as a stub.
    let rows = cx
        .monstats(class)
        .is_some_and(|r| (r.monstatsex as usize) < cx.tables.monstats2.len());
    if rec.init != 0 && rows {
        cx.store.unhandled.push(Unhandled::Function {
            addr: rec.init,
            unit,
        });
    }
    let c = cx.store.control_mut(unit).expect("control");
    c.special_state = state;
    c.function = if rec.think == 0 {
        table::IDLE_FN
    } else {
        rec.think
    };
}

/// The record for `state` (the lookup of §3.2 with a given state).
fn record_for<W: AiHost + ?Sized>(cx: &mut Ctx<'_, W>, unit: UnitId, state: u32) -> AiRecord {
    let saved = cx.store.control(unit).map_or(0, |c| c.special_state);
    if let Some(c) = cx.store.control_mut(unit) {
        c.special_state = state;
    }
    let r = cx.record(unit);
    if let Some(c) = cx.store.control_mut(unit) {
        c.special_state = saved;
    }
    r
}

/// The think handler `0x005B1740` (§2.1).
pub fn think<W: AiHost + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, unit: UnitId) {
    let class = cx.world.class(unit);
    let Some(row) = cx.monstats(class) else {
        return;
    };
    let class2 = row.monstatsex as usize;
    if class2 >= cx.tables.monstats2.len() {
        return;
    }
    let Some(function) = cx.store.control(unit).map(|c| c.function) else {
        cx.store.unhandled.push(Unhandled::NoControl { unit });
        return;
    };
    let mut p = TickParam {
        target: None,
        distance: 0,
        combat: false,
        class: class as usize,
        class2,
    };
    if precheck_a(game, cx, unit, &p) {
        return;
    }
    if precheck_b(game, cx, unit, &mut p) {
        return;
    }
    if precheck_c(game, cx, unit, &mut p) {
        return;
    }
    // The prechecks may have reinstalled the AI: call the current one.
    let f = cx.store.control(unit).map_or(function, |c| c.function);
    run_function(game, cx, f, unit, &p);
}

/// The monster class handler's AI events (`0x005A7F80`, `tick.md` §5.6):
/// type 2 runs [`think`] unless the monster is frozen and alive; type 10
/// runs the AI reset `0x005A7F70`.
pub fn handle_event<W: AiHost + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    run: &TimerRun,
) -> bool {
    let unit = run.owner.unit;
    match run.event {
        EVENT_THINK => {
            if !frozen(cx, unit) {
                think(game, cx, unit);
            }
            true
        }
        EVENT_AI_RESET => {
            cx.world.ai_reset(unit);
            true
        }
        _ => false,
    }
}

/// [`EventDispatch`] for the tick: monster-class type-2 and type-10
/// events without an explicit callback run the AI; everything else goes
/// to `next`.
pub struct MonsterDispatch<'a, W: ?Sized, N: ?Sized> {
    pub cx: Ctx<'a, W>,
    pub next: &'a mut N,
}

impl<W: AiHost + ?Sized, N: EventDispatch + ?Sized> EventDispatch for MonsterDispatch<'_, W, N> {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        if run.class == TimerClass::Monster
            && run.callback.is_none()
            && handle_event(game, &mut self.cx, run)
        {
            return;
        }
        self.next.run_event(game, run);
    }
}
