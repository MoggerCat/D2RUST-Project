// Spec: specs/sim/pathing.md §1 (walk and run requests)
//! C→S 0x01–0x04 after transport validation (`intents-events.md` §2.4
//! rules 3–4 own the parsing and range checks): the player mode request,
//! the mode and interrupt checks, and the movement start.

use super::find::{compute, path_type, reset_type, set_type};
use super::seams::{PathWorld, Point, StartTarget, TargetUnit, WalkError, WalkPath, WalkUnits};
use super::step::STAT_STAMINA;
use super::tables::PathTables;
use super::velocity::{mode_velocity, run_velocity_bonus, set_velocity};
use crate::game::Game;
use crate::units::{UnitId, UnitType};

/// Player modes used here.
pub mod mode {
    pub const DEATH: u32 = 0;
    pub const NEUTRAL: u32 = 1;
    pub const WALK: u32 = 2;
    pub const RUN: u32 = 3;
    pub const TOWN_NEUTRAL: u32 = 5;
    pub const TOWN_WALK: u32 = 6;
    pub const DEAD: u32 = 17;
    pub const KNOCKBACK: u32 = 19;
}

/// States read here.
pub mod state {
    pub const MOVE_STUCK_54: u16 = 54;
    pub const CONCENTRATION: u16 = 42;
    pub const STATE_15: u16 = 15;
}

/// Stat of the concentration state list read by §1.4 rule 5.
pub const STAT_164: u16 = 164;

/// The request target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkTarget {
    /// Point form (`0x005809D0`).
    Point(Point),
    /// Unit form (`0x00580A70`): type and GUID from the message.
    Unit { ty: UnitType, guid: u32 },
}

/// What a request did (for callers and tests; the dispatcher result is
/// 0 in every case, §1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Unit form: no unit of that type and GUID (a log line).
    NoTargetUnit,
    /// Mode check `0x0057EDD0` refused.
    ModeRefused,
    /// Interrupt check `0x0057EEC0` refused.
    InterruptRefused,
    /// Knockback on a unit already in mode 19: nothing.
    KnockbackIgnored,
    /// The mode's start function is not a walk mode (seam).
    OtherMode,
    /// Path empty: neutral start (§1.5 step 5).
    Neutral,
    /// Mode set with a path of this many points.
    Moving(i32),
}

/// C→S message ids 0x01–0x04 → (mode, unit form).
pub fn message_request(id: u8) -> Option<(u32, bool)> {
    match id {
        0x01 => Some((mode::WALK, false)),
        0x02 => Some((mode::WALK, true)),
        0x03 => Some((mode::RUN, false)),
        0x04 => Some((mode::RUN, true)),
        _ => None,
    }
}

/// Handler of C→S 0x01–0x04 (§1.1) after `intents-events.md` §2.4 accepts
/// it: `a`, `b` are (x, y) for the point forms and (type, GUID) for the
/// unit forms. Returns the dispatcher result (always 0) and the outcome.
#[allow(clippy::too_many_arguments)]
pub fn handle_message<W: PathWorld + ?Sized, U: WalkUnits + ?Sized>(
    t: &PathTables,
    w: &mut W,
    u: &mut U,
    game: &mut Game,
    player: UnitId,
    id: u8,
    a: u32,
    b: u32,
) -> Result<(u32, Option<Outcome>), WalkError> {
    let Some((m, unit_form)) = message_request(id) else {
        return Ok((0, None));
    };
    let target = if unit_form {
        let Some(ty) = UnitType::ALL.get(a as usize).copied() else {
            return Ok((0, Some(Outcome::NoTargetUnit)));
        };
        WalkTarget::Unit { ty, guid: b }
    } else {
        WalkTarget::Point(Point::new(a as i32, b as i32))
    };
    let o = request(t, w, u, game, player, None, m, target, false)?;
    Ok((0, Some(o)))
}

/// Player mode request `0x005809D0` / `0x00580A70` (§1.2).
#[allow(clippy::too_many_arguments)]
pub fn request<W: PathWorld + ?Sized, U: WalkUnits + ?Sized>(
    t: &PathTables,
    w: &mut W,
    u: &mut U,
    game: &mut Game,
    unit: UnitId,
    skill: Option<u16>,
    m: u32,
    target: WalkTarget,
    reentry: bool,
) -> Result<Outcome, WalkError> {
    // Step 1.
    let target = match target {
        WalkTarget::Point(p) => StartTarget::Point(p),
        WalkTarget::Unit { ty, guid } => match u.find_unit(ty, guid) {
            Some(t) => StartTarget::Unit(t),
            None => return Ok(Outcome::NoTargetUnit),
        },
    };
    // Step 2.
    if !mode_check(u, game, unit, m) {
        return Ok(Outcome::ModeRefused);
    }
    // Step 3.
    if !reentry && !interrupt_check(t, w, u, game, unit, m, skill)? {
        return Ok(Outcome::InterruptRefused);
    }
    // Step 4.
    if matches!(target, StartTarget::Unit(_)) && u.unit_type(unit) == UnitType::Player {
        u.clear_queued_action(unit);
    }
    // Step 5.
    if let Some(s) = skill {
        u.set_used_skill(unit, Some(s));
    }
    if !matches!(
        m,
        mode::WALK | mode::RUN | mode::TOWN_WALK | mode::KNOCKBACK
    ) {
        u.start_other_mode(game, unit, m, target);
        return Ok(Outcome::OtherMode);
    }
    let Some(mut path) = w.load_path(unit) else {
        return Err(WalkError::Fatal("walk request on a unit without a path"));
    };
    match target {
        StartTarget::Point(p) => {
            // `0x00648AD0`.
            path.target = p;
            path.target_unit = None;
        }
        StartTarget::Unit(tu) => {
            if m == mode::KNOCKBACK && u.mode(tu) == mode::KNOCKBACK {
                return Ok(Outcome::KnockbackIgnored);
            }
            // `0x00648B90`.
            path.target_unit = Some(TargetUnit {
                unit: tu,
                ty: u.unit_type(tu),
                guid: u.guid(tu),
            });
        }
    }
    let r = start_movement(t, w, u, game, unit, &mut path, m);
    w.store_path(unit, &path);
    let n = r?;
    if n == 0 {
        neutral_start(w, u, game, unit, &path);
        Ok(Outcome::Neutral)
    } else {
        Ok(Outcome::Moving(n))
    }
}

/// Mode check `0x0057EDD0` (§1.3).
pub fn mode_check<U: WalkUnits + ?Sized>(u: &U, game: &Game, unit: UnitId, m: u32) -> bool {
    if u.has_cursor_item(unit) && m != 0 {
        return m == 17;
    }
    if matches!(m, 0 | 1 | 5 | 17) {
        return true;
    }
    let frame = game.frame;
    let e = u.first_type1_expire(game, unit);
    let early = frame <= e + 5;
    match u.mode(unit) {
        0 | 4 | 9 | 17 => false,
        7 | 8 | 10 | 11 => early || m == 4 || m == 9,
        12 => early,
        13 => u.class(unit) != 0,
        15 => u.class(unit) != 5,
        18 => match u.used_skill(unit) {
            Some(s) if s.seq_input > 0 => true,
            _ => early,
        },
        _ => true,
    }
}

/// Interrupt check `0x0057EEC0(mode, 1, skill)` (§1.4).
#[allow(clippy::too_many_arguments)]
pub fn interrupt_check<W: PathWorld + ?Sized, U: WalkUnits + ?Sized>(
    t: &PathTables,
    w: &mut W,
    u: &mut U,
    game: &mut Game,
    unit: UnitId,
    m: u32,
    skill: Option<u16>,
) -> Result<bool, WalkError> {
    let cur = u.mode(unit);
    // Rule 1.
    if u.has_state(unit, state::MOVE_STUCK_54) || cur == mode::DEATH || cur == mode::DEAD {
        return Ok(false);
    }
    // Rule 2.
    if m == 0 {
        return Ok(true);
    }
    let Some(used) = u.used_skill(unit) else {
        return Ok(true);
    };
    // Rule 3.
    if skill == Some(used.id) && matches!(used.srvdofunc, 67 | 76) {
        return Ok(true);
    }
    // Rule 4.
    if !used.interrupt {
        if cur == mode::NEUTRAL {
            reenter_neutral(t, w, u, game, unit)?;
            return Ok(true);
        }
        let e = u.first_type1_expire(game, unit);
        return Ok(matches!(m, 7 | 8 | 10 | 11 | 13 | 18) && game.frame <= e + 5);
    }
    // Rule 5. TODO(spec: pathing.md §1.4 rule 5, whether a failed
    // concentration roll falls through to the state 15 test): read as
    // "state 42 → roll; else state 15".
    let to_rule6 = if u.has_state(unit, state::CONCENTRATION) {
        let v = u.state_stat(unit, state::CONCENTRATION, STAT_164);
        let r = u.seed(unit).roll(100);
        (r as i64) < v as i64
    } else {
        u.has_state(unit, state::STATE_15)
    };
    if !to_rule6 {
        return Ok(true);
    }
    // Rule 6.
    if cur == mode::NEUTRAL {
        reenter_neutral(t, w, u, game, unit)?;
        return Ok(true);
    }
    Ok(false)
}

/// `0x005809D0(no skill, 1, 0, 0, re-entry 1)`.
fn reenter_neutral<W: PathWorld + ?Sized, U: WalkUnits + ?Sized>(
    t: &PathTables,
    w: &mut W,
    u: &mut U,
    game: &mut Game,
    unit: UnitId,
) -> Result<(), WalkError> {
    request(
        t,
        w,
        u,
        game,
        unit,
        None,
        mode::NEUTRAL,
        WalkTarget::Point(Point::default()),
        true,
    )?;
    Ok(())
}

/// Starting the movement `0x0057F090(mode)` (§1.5) on a loaded path,
/// steps 1–4 and 6; returns the point count (0: the caller runs the
/// neutral start of step 5).
pub fn start_movement<W: PathWorld + ?Sized, U: WalkUnits + ?Sized>(
    t: &PathTables,
    w: &mut W,
    u: &mut U,
    game: &mut Game,
    unit: UnitId,
    path: &mut WalkPath,
    m: u32,
) -> Result<i32, WalkError> {
    let ty = u.unit_type(unit);
    // Step 1.
    reset_type(t, path, ty)?;
    // Step 2.
    let mut m = m;
    let stamina_less_run = m == mode::RUN && u.stat(unit, STAT_STAMINA) == 0;
    if m == mode::WALK || m == mode::TOWN_WALK || stamina_less_run {
        m = if path.room.is_some_and(|r| w.room_in_town(r)) {
            mode::TOWN_WALK
        } else {
            mode::WALK
        };
    } else if m == mode::KNOCKBACK {
        set_type(t, path, ty, path_type::KNOCKBACK)?;
        path.distance_budget = 5;
    }
    // Step 3.
    let n = compute(t, w, u, path, unit, false)?;
    // Step 4.
    u.set_used_skill(unit, None);
    // Step 5.
    if path.count == 0 {
        return Ok(0);
    }
    // Step 6.
    set_mode_and_velocity(t, u, game, unit, path, m);
    u.cancel_events(game, unit, 0);
    u.cancel_events(game, unit, 1);
    u.schedule_event0(game, unit);
    Ok(n)
}

/// Mode set plus the velocity half of `0x00623F50` (§8.1, §8.2).
pub fn set_mode_and_velocity<U: WalkUnits + ?Sized>(
    t: &PathTables,
    u: &mut U,
    game: &mut Game,
    unit: UnitId,
    path: &mut WalkPath,
    m: u32,
) {
    u.set_mode(game, unit, m);
    if (m == mode::RUN || m == mode::KNOCKBACK) && u.unit_type(unit) == UnitType::Player {
        let (walk, run, _) = u.charstats_velocity(unit);
        if let Some(v) = run_velocity_bonus(walk, run) {
            u.attach_run_stats(game, unit, v);
        }
    }
    if let Some(v) = mode_velocity(t, u, unit, m) {
        set_velocity(path, v);
    }
}

/// Neutral start `0x0057F020`: cancel event types 0 and 1, mode 5 in a
/// town room else 1, used skill none.
pub fn neutral_start<W: PathWorld + ?Sized, U: WalkUnits + ?Sized>(
    w: &mut W,
    u: &mut U,
    game: &mut Game,
    unit: UnitId,
    path: &WalkPath,
) {
    u.cancel_events(game, unit, 0);
    u.cancel_events(game, unit, 1);
    let town = path.room.is_some_and(|r| w.room_in_town(r));
    u.set_mode(
        game,
        unit,
        if town {
            mode::TOWN_NEUTRAL
        } else {
            mode::NEUTRAL
        },
    );
    u.set_used_skill(unit, None);
}
