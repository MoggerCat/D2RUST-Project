// Spec: specs/sim/units.md §4, §6.1 (join), §6.3 (missile setup)
//! Modes: setting a mode (§4.1), preparing the animation, the player
//! mode starts and their event-0/event-1 handlers (§4.5), the monster
//! mode set with its mode table and the neutral start (§4.6), the player
//! join sequence (§6.1) and the missile setup (§6.3).

use thiserror::Error;

use crate::game::GameError;
use crate::stats::stat;
use crate::stats::states::state;
use crate::tick::events::event;

use super::anim::{self, AnimError, Form};
use super::hooks::{Sim, UnitHooks};
use super::record::flags;
use super::{UnitId, UnitType};

/// Player modes (plrmode.txt rows, §1).
pub mod player_mode {
    pub const DT: u32 = 0;
    pub const NU: u32 = 1;
    pub const WL: u32 = 2;
    pub const RN: u32 = 3;
    pub const GH: u32 = 4;
    pub const TN: u32 = 5;
    pub const TW: u32 = 6;
    pub const A1: u32 = 7;
    pub const A2: u32 = 8;
    pub const BL: u32 = 9;
    pub const TH: u32 = 11;
    pub const DD: u32 = 17;
    pub const SEQUENCE: u32 = 18;
    pub const KB: u32 = 19;
    pub const COUNT: u32 = 20;
}

/// Monster modes (monmode.txt rows, §1).
pub mod monster_mode {
    pub const DT: u32 = 0;
    pub const NU: u32 = 1;
    pub const GH: u32 = 3;
    pub const DD: u32 = 12;
    pub const SEQUENCE: u32 = 14;
    pub const COUNT: u32 = 16;
}

/// Errors of the unit operations (d2rs API misuse or a fatal assertion
/// of the original).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UnitError {
    #[error("unknown unit {0:?}")]
    UnknownUnit(UnitId),
    #[error("mode {mode} out of range for {ty:?}")]
    BadMode { ty: UnitType, mode: u32 },
    /// `0x005A7C20` asserts the monster has no state 54.
    #[error("monster mode set with state 54 (fatal assertion in 1.14d)")]
    Uninterruptable,
    /// A null object or item handler (fatal in 1.14d, §5 rule 2).
    #[error("null {ty:?} handler for event {event} (fatal in 1.14d)")]
    NullHandler { ty: UnitType, event: u8 },
    /// A timer callback d2rs does not know.
    #[error("unknown timer callback {0:#x}")]
    UnknownCallback(u32),
    #[error(transparent)]
    Anim(#[from] AnimError),
    #[error(transparent)]
    Game(#[from] GameError),
    /// Stat-list expiry an expired extended list stops (`stat-lists.md`
    /// edge case 4: endless in 1.14d).
    #[error(transparent)]
    Stats(#[from] crate::stats::lists::StatListError),
}

fn record_mut<'a>(
    sim: &'a mut Sim<'_>,
    unit: UnitId,
) -> Result<&'a mut super::record::UnitRecord, UnitError> {
    sim.units.get_mut(unit).ok_or(UnitError::UnknownUnit(unit))
}

/// `0x00553570`(game, unit, mode) (§4.1): drop the unit's combat
/// entries, then `0x00624690`(unit, mode).
pub fn set_mode<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    mode: u32,
) -> Result<(), UnitError> {
    hooks.drop_combat_entries(sim, unit);
    write_mode(sim, hooks, unit, mode)
}

/// `0x00624690`(unit, mode) (§4.1), the part of [`set_mode`] after the
/// combat drop: the mode written, the unit queued for update, flag 0x1,
/// and on a new mode the TEMPONLY lists freed and the animation fields
/// re-initialised. A monster staying in mode 1 and a tile: nothing.
pub fn write_mode<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    mode: u32,
) -> Result<(), UnitError> {
    let rec = record_mut(sim, unit)?;
    if rec.ty == UnitType::Tile {
        return Ok(());
    }
    let changed = rec.mode != mode;
    if !changed && rec.ty == UnitType::Monster && mode == monster_mode::NU {
        return Ok(());
    }
    rec.mode = mode;
    rec.flags |= flags::CHANGED;
    sim.game.lists.queue_update(unit).map_err(GameError::from)?;
    if changed {
        // `0x006272E0` (`stat-lists.md` §8.9): a new mode frees the
        // unit's TEMPONLY lists (the run list of `pathing.md` §8.2, the
        // attack-rate lists of the skill bodies).
        sim.stats.remove_temporary_lists(hooks, unit);
        hooks.reinit_anim(sim, unit);
    }
    Ok(())
}

/// Prepare animation `0x005533D0` (§4.1). The frame count +0x48: the
/// sequence's count · 256 in the sequence branch (`skills/sequences.md`
/// §2 step 2; that branch leaves it, `0x0055341A`–`0x0055343B`), the
/// AnimData record's frames · 256 otherwise.
pub fn prepare_animation<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    let rec = sim.units.get(unit).ok_or(UnitError::UnknownUnit(unit))?;
    let sequence_mode = match rec.ty {
        UnitType::Player => rec.mode == player_mode::SEQUENCE,
        UnitType::Monster => rec.mode == monster_mode::SEQUENCE,
        _ => false,
    };
    let sequence = if sequence_mode {
        hooks.load_sequence(sim, unit)
    } else {
        None
    };
    let rate = if sequence.is_none() && hooks.has_path(sim, unit) {
        let r = hooks.anim_rate(sim, unit);
        hooks.anim_velocity(sim, unit);
        Some(r)
    } else {
        None
    };
    let record = hooks.anim_record(sim, unit);
    let a = &mut record_mut(sim, unit)?.anim;
    a.action_frame = 0;
    if let Some(seq) = sequence {
        a.frame_count = seq.frame_count;
        // Load step 3 (`0x00621210(unit, 0)`): +0x44 := the drawn frame of
        // frame 0 · 256 when the list carries the drawn frames.
        if let Some(&d) = seq.drawn.first() {
            a.frame = i32::from(d) << 8;
        }
        a.sequence = Some(seq);
    } else {
        a.sequence = None;
        a.frame = 0;
        if let Some(r) = rate {
            a.speed = r;
        }
    }
    a.record = record;
    if let (None, Some(r)) = (&a.sequence, record) {
        a.frame_count = (r.frames as i32).wrapping_mul(256);
    }
    Ok(())
}

/// The animated mode start tail (§4.1): prepare, cancel 0/1, §4.2 main
/// form with the frame bonus.
pub fn animate<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    prepare_animation(sim, hooks, unit)?;
    anim::cancel_mode_events(sim.game, unit);
    let bonus = hooks.frame_bonus(sim, unit);
    let rec = sim
        .units
        .get_mut(unit)
        .ok_or(UnitError::UnknownUnit(unit))?;
    anim::run(sim.game, unit, &mut rec.anim, Form::Main { bonus })?;
    Ok(())
}

// ---- players (§4.5) --------------------------------------------------------

/// The player's start-function class (table `0x006E1740`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerStart {
    /// 0 DT (`0x00580EC0`).
    Death,
    /// 1 NU, 5 TN (`0x0057F020`).
    Neutral,
    /// 2 WL, 3 RN, 6 TW, 19 KB (`0x0057F1F0`, `0x0057F190` →
    /// `0x0057F090`).
    Move,
    /// 4 GH (`0x0057FE30`), 9 BL (`0x0057FDC0`).
    Animate,
    /// 7, 8, 10–16, 18 (`0x0057FE90`, `0x0057FEF0`).
    Skill,
    /// 17 DD (`0x0057FCA0`).
    Corpse,
}

/// The start function class of a player mode.
pub fn player_start_kind(mode: u32) -> Option<PlayerStart> {
    Some(match mode {
        0 => PlayerStart::Death,
        1 | 5 => PlayerStart::Neutral,
        2 | 3 | 6 | 19 => PlayerStart::Move,
        4 | 9 => PlayerStart::Animate,
        7 | 8 | 10..=16 | 18 => PlayerStart::Skill,
        17 => PlayerStart::Corpse,
        _ => return None,
    })
}

/// `0x005809D0` / `0x00580A70` (§4.5): the request check, then the
/// mode's start function. Returns whether the request was accepted.
pub fn player_start<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    mode: u32,
) -> Result<bool, UnitError> {
    let kind = player_start_kind(mode).ok_or(UnitError::BadMode {
        ty: UnitType::Player,
        mode,
    })?;
    if !hooks.player_request_check(sim, unit, mode) {
        return Ok(false);
    }
    player_start_function(sim, hooks, unit, mode, kind)?;
    Ok(true)
}

fn player_start_function<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    mode: u32,
    kind: PlayerStart,
) -> Result<(), UnitError> {
    match kind {
        PlayerStart::Death => {
            hooks.player_death(sim, unit);
            set_mode(sim, hooks, unit, mode)?;
            animate(sim, hooks, unit)?;
        }
        PlayerStart::Neutral => player_neutral(sim, hooks, unit)?,
        PlayerStart::Move => {
            let town = hooks.room_flag(sim, unit);
            let mut m = mode;
            if m == player_mode::RN && sim.stats.unit_total(unit, stat::STAMINA, 0) == 0 {
                m = player_mode::WL;
            }
            if matches!(m, player_mode::WL | player_mode::TW) {
                m = if town {
                    player_mode::TW
                } else {
                    player_mode::WL
                };
            }
            if m == player_mode::KB {
                hooks.player_knockback_path(sim, unit);
            }
            set_mode(sim, hooks, unit, m)?;
            anim::cancel_mode_events(sim.game, unit);
            anim::every_tick_movement(sim.game, unit)?;
        }
        PlayerStart::Animate => {
            set_mode(sim, hooks, unit, mode)?;
            animate(sim, hooks, unit)?;
        }
        PlayerStart::Skill => {
            set_mode(sim, hooks, unit, mode)?;
            animate(sim, hooks, unit)?;
            record_mut(sim, unit)?.flags &= !flags::ATTACK_PENDING;
            hooks.player_skill_start(sim, unit);
        }
        PlayerStart::Corpse => {
            set_mode(sim, hooks, unit, player_mode::DD)?;
            hooks.player_corpse(sim, unit);
        }
    }
    Ok(())
}

/// `0x0057F020`: cancel 0/1, mode 1, or 5 in town.
fn player_neutral<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    anim::cancel_mode_events(sim.game, unit);
    let m = if hooks.room_flag(sim, unit) {
        player_mode::TN
    } else {
        player_mode::NU
    };
    set_mode(sim, hooks, unit, m)
}

/// Player event 0 `0x005811D0`: the mode's action function (table
/// `0x00732C10`); result 2 runs the ENDANIM handler at once.
pub fn player_event0<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    a1: u32,
    a2: u32,
) -> Result<(), UnitError> {
    let mode = sim
        .units
        .get(unit)
        .ok_or(UnitError::UnknownUnit(unit))?
        .mode;
    let result = match player_start_kind(mode) {
        Some(PlayerStart::Move) => hooks.player_movement_step(sim, unit, a1, a2),
        Some(PlayerStart::Skill) => hooks.player_action_frame(sim, unit, a1, a2),
        Some(_) => 1,
        None => {
            return Err(UnitError::BadMode {
                ty: UnitType::Player,
                mode,
            })
        }
    };
    if result == 2 {
        player_event1(sim, hooks, unit)?;
    }
    Ok(())
}

/// Player event 1 `0x00581020` (ENDANIM).
pub fn player_event1<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    let mode = sim
        .units
        .get(unit)
        .ok_or(UnitError::UnknownUnit(unit))?
        .mode;
    if matches!(mode, player_mode::A1 | player_mode::A2 | player_mode::TH)
        || hooks.player_item_row_flagged(sim, unit)
    {
        hooks.player_attack_cleanup(sim, unit);
    }
    match mode {
        player_mode::DT => {
            player_start_function(sim, hooks, unit, player_mode::DD, PlayerStart::Corpse)
        }
        player_mode::KB => {
            // TODO(units.md §4.5): what a failed check 0x0057EEC0(4, 1, 0)
            // does is not written; nothing here.
            if hooks.player_request_check(sim, unit, player_mode::GH) {
                player_start_function(sim, hooks, unit, player_mode::GH, PlayerStart::Animate)?;
            }
            Ok(())
        }
        _ => player_neutral(sim, hooks, unit),
    }
}

/// Join `0x00534AD0` (§6.1): neutral mode start (`0x005809D0`, mode 1),
/// then event 3 at f + 1, then event 11 at f + 250.
pub fn player_join<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    player_start(sim, hooks, unit, player_mode::NU)?;
    let f = sim.game.frame;
    sim.game.schedule_event(
        unit,
        u32::from(event::STAT_REGEN),
        f.wrapping_add(1),
        None,
        0,
        0,
    )?;
    sim.game.schedule_event(
        unit,
        u32::from(event::DELAYED_PORTAL),
        f.wrapping_add(250),
        None,
        0,
        0,
    )?;
    Ok(())
}

// ---- monsters (§4.6) ---------------------------------------------------------

/// A monster mode record (`0x006E2260` + 16·mode): start, event-0 and
/// event-1 function addresses (0: null) and the schedule flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterModeRecord {
    pub start: u32,
    pub event0: u32,
    pub event1: u32,
    pub schedules: bool,
}

/// Fallback for a null record or function (`0x005A7B30`).
pub const MONSTER_MODE_FALLBACK: u32 = 0x005A_7B30;

/// The neutral start, implemented here.
pub const MONSTER_NEUTRAL_START: u32 = 0x005A_73E0;

const fn rec(start: u32, event0: u32, event1: u32, schedules: bool) -> MonsterModeRecord {
    MonsterModeRecord {
        start,
        event0,
        event1,
        schedules,
    }
}

/// The monster mode table (`0x005A78A0`, §4.6).
pub const MONSTER_MODES: [MonsterModeRecord; 16] = [
    rec(0x005A6FF0, 0x005A7350, 0x005A72B0, true),
    rec(0x005A73E0, 0x005A6DE0, 0, false),
    rec(0x005A7520, 0x005A8490, 0, true),
    rec(0x005A7580, 0x005A6DE0, 0x005A8030, true),
    rec(0x005A75C0, 0x005A7670, 0x005A8030, true),
    rec(0x005A75C0, 0x005A7670, 0x005A8030, true),
    rec(0x005A77C0, 0x005A6DE0, 0x005A8030, true),
    rec(0x005A75C0, 0x005A7670, 0x005A8030, true),
    rec(0x005A75C0, 0x005A7670, 0x005A8030, true),
    rec(0x005A75C0, 0x005A7670, 0x005A8030, true),
    rec(0x005A7490, 0x005A74A0, 0x005A74D0, false),
    rec(0x005A74E0, 0x005A6DE0, 0, false),
    rec(0x005A7390, 0x005A6DE0, 0, false),
    rec(0x005A77D0, 0x005A8630, 0x005A8520, true),
    rec(0x005A7870, 0x005A8670, 0x005A8030, true),
    rec(0x005A7550, 0x005A84F0, 0, true),
];

/// Whether a monster mode moves (`0x005A6B10`, table `0x006E23D0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moves {
    No,
    Always,
    /// The monstats2 `*mv` bit of the mode.
    Bit,
}

/// The moving table (§4.6).
pub const MONSTER_MOVES: [Moves; 16] = [
    Moves::No,
    Moves::No,
    Moves::Always,
    Moves::No,
    Moves::Bit,
    Moves::Bit,
    Moves::No,
    Moves::Bit,
    Moves::Bit,
    Moves::Bit,
    Moves::Bit,
    Moves::Bit,
    Moves::No,
    Moves::Always,
    Moves::Always,
    Moves::Always,
];

fn monster_record<H: UnitHooks>(
    sim: &Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    mode: u32,
) -> Option<MonsterModeRecord> {
    hooks
        .monster_class_record(sim, unit, mode)
        .or_else(|| MONSTER_MODES.get(mode as usize).copied())
}

/// `0x005A6B10`: the mode moves.
pub fn monster_moves(sim: &Sim<'_>, unit: UnitId, mode: u32) -> bool {
    match MONSTER_MOVES.get(mode as usize) {
        Some(Moves::Always) => true,
        Some(Moves::Bit) => sim
            .units
            .get(unit)
            .and_then(|r| sim.data.monster(r.class))
            .is_some_and(|m| m.moves & 1 << mode != 0),
        _ => false,
    }
}

/// Monster mode set `0x005A7C20` (§4.6).
pub fn monster_set_mode<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    mode: u32,
) -> Result<(), UnitError> {
    if mode >= monster_mode::COUNT {
        return Err(UnitError::BadMode {
            ty: UnitType::Monster,
            mode,
        });
    }
    if sim.stats.has_state(unit, state::UNINTERRUPTABLE) {
        return Err(UnitError::Uninterruptable);
    }
    if mode != monster_mode::GH {
        hooks.monster_mode_bookkeeping(sim, unit, mode);
        // `umod-callbacks.md` §2 rule 1: the mode damage rewrite, then
        // umod mode 0, old mode still set.
        hooks.monster_mode_damage(sim, unit, mode);
        hooks.monster_umods(sim, unit, 0);
    }
    let start = monster_record(sim, hooks, unit, mode)
        .map(|r| r.start)
        .filter(|&s| s != 0)
        .unwrap_or(MONSTER_MODE_FALLBACK);
    let started = if start == MONSTER_NEUTRAL_START {
        monster_neutral(sim, hooks, unit)?;
        true
    } else {
        hooks.monster_mode_function(sim, unit, start)
    };
    if !started {
        monster_neutral(sim, hooks, unit)?;
    }
    record_mut(sim, unit)?.flags |= flags::MODE_CHANGING;
    prepare_animation(sim, hooks, unit)?;
    // `umod-callbacks.md` §2 rule 2: umod mode 1, new mode set; before
    // the cancel of events 0 / 1 and the animation schedule.
    hooks.monster_umods(sim, unit, 1);
    anim::cancel_mode_events(sim.game, unit);
    let now = record_mut(sim, unit)?.mode;
    if monster_record(sim, hooks, unit, now).is_some_and(|r| r.schedules) {
        if monster_moves(sim, unit, now) {
            anim::every_tick_movement(sim.game, unit)?;
        } else {
            let bonus = hooks.frame_bonus(sim, unit);
            let rec = sim
                .units
                .get_mut(unit)
                .ok_or(UnitError::UnknownUnit(unit))?;
            anim::run(sim.game, unit, &mut rec.anim, Form::Main { bonus })?;
        }
    }
    Ok(())
}

/// The neutral AI delay of §4.6: `aidel` (Normal column, or the
/// difficulty's column when game +0x6A / +0x74 is set), 0 → 15.
pub fn neutral_ai_delay(sim: &Sim<'_>, class: u32) -> i32 {
    let col = if sim.data.aidel_by_difficulty {
        usize::from(sim.data.difficulty).min(2)
    } else {
        0
    };
    match sim.data.monster(class).map_or(0, |m| m.aidel[col]) {
        0 => 15,
        d => i32::from(d),
    }
}

/// Neutral start `0x005A73E0` (§4.6): mode 1 and, unless the unit has a
/// type-2 timer with expire > f, event 2 at f + 45 (state 21) or
/// f + aidel.
pub fn monster_neutral<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    set_mode(sim, hooks, unit, monster_mode::NU)?;
    let f = sim.game.frame;
    let pending = sim
        .game
        .timers
        .unit_timers(unit)
        .into_iter()
        .filter(|&t| {
            sim.game
                .timers
                .event(t)
                .is_some_and(|e| e.0 == event::AI_THINK)
        })
        .filter_map(|t| sim.game.timers.expire(t))
        .filter(|&e| e > 0)
        .min()
        .unwrap_or(0);
    if pending > f {
        return Ok(());
    }
    let expire = if sim.stats.has_state(unit, state::AI_DELAY) {
        f.wrapping_add(45)
    } else {
        let class = sim
            .units
            .get(unit)
            .ok_or(UnitError::UnknownUnit(unit))?
            .class;
        f.wrapping_add(neutral_ai_delay(sim, class))
    };
    if sim.stats.has_state(unit, state::UNINTERRUPTABLE) {
        hooks.uninterruptable_check(sim, unit);
    }
    sim.game
        .schedule_event(unit, u32::from(event::AI_THINK), expire, None, 0, 0)?;
    Ok(())
}

/// Monster events 0 and 1 (`0x005A7BA0`, `0x005A7BE0`): the current
/// mode's function (null: `0x005A7B30`).
pub fn monster_event<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    end_anim: bool,
) -> Result<(), UnitError> {
    let mode = sim
        .units
        .get(unit)
        .ok_or(UnitError::UnknownUnit(unit))?
        .mode;
    let f = monster_record(sim, hooks, unit, mode)
        .map(|r| if end_anim { r.event1 } else { r.event0 })
        .filter(|&a| a != 0)
        .unwrap_or(MONSTER_MODE_FALLBACK);
    hooks.monster_mode_function(sim, unit, f);
    Ok(())
}

// ---- missiles (§6.3) -----------------------------------------------------------

/// Missile setup `0x0059F8A0`: cancel all the missile's timers, then the
/// every-tick type-0 event.
pub fn missile_init(sim: &mut Sim<'_>, unit: UnitId) -> Result<(), UnitError> {
    sim.game.timers.cancel_unit_timers(unit);
    let owner = sim.game.owner(unit)?;
    sim.game
        .timers
        .schedule_every_tick(owner, u32::from(event::MODE_CHANGE), None, 0, 0)
        .map_err(GameError::from)?;
    Ok(())
}
