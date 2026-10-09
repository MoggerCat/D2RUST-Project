// Spec: specs/sim/units.md §5, §6; specs/sim/unit-handlers.tsv; specs/sim/stat-lists.md §10
//! Timer event dispatch per unit kind (§5): the default-handler tables of
//! `unit-handlers.tsv` ([`HANDLERS`], checked against the TSV by a
//! test), the handlers this spec group owns — modes (events 0, 1),
//! regeneration (3), active state (5), hover (6), periodic stats (9),
//! the player refresh (11), expiry (12) — and the hooks for the rest.
//! [`UnitSystem`] is the [`EventDispatch`] the tick runs.

use std::sync::Arc;

use crate::game::Game;
use crate::stats::states::{group, state};
use crate::stats::{fraction_changed, life_fraction, muldiv, stat, StatData, StatLists};
use crate::tick::events::{event, monster_dropped_when_frozen};
use crate::tick::timer::{CallbackId, TimerClass, TimerRun};
use crate::tick::EventDispatch;

use super::hooks::{Sim, UnitData, UnitHooks};
use super::modes::{self, UnitError};
use super::record::{flags, flags2, Units};
use super::{UnitId, UnitType};

/// Default handler addresses per class and event type (0: null), from
/// `unit-handlers.tsv` (tables `0x006E1810`, `0x006E2490`, `0x006E19B0`,
/// `0x006E117C`; missiles ignore the type).
pub const HANDLERS: [(TimerClass, [u32; 15]); 5] = [
    (
        TimerClass::Player,
        [
            0x005811D0, 0x00581020, 0, 0x00580810, 0, 0x0056D790, 0x00580B70, 0, 0x0056FCB0,
            0x0056FE40, 0, 0x00580BE0, 0x00580800, 0x005689D0, 0,
        ],
    ),
    (
        TimerClass::Monster,
        [
            0x005A7BA0, 0x005A7BE0, 0x005B1740, 0x005A6920, 0, 0x0056D790, 0x005A7F00, 0x005A4370,
            0x0056FCB0, 0x0056FE40, 0x005A7F70, 0, 0x005A7EF0, 0, 0,
        ],
    ),
    (
        TimerClass::Object,
        [
            0x00581700, 0x00581490, 0x00581510, 0x005818B0, 0x005817A0, 0x005814D0, 0x00581620,
            0x00581A10, 0x00581250, 0x00585CE0, 0x00586850, 0x00581410, 0, 0, 0,
        ],
    ),
    (
        TimerClass::Item,
        [
            0, 0, 0, 0x00562D30, 0x0055F120, 0, 0, 0, 0, 0, 0, 0, 0x0055F130, 0, 0,
        ],
    ),
    (TimerClass::Missile, [0x005ADBB0; 15]),
];

/// The default handler of (class, event type), 0 when null.
pub fn handler(class: TimerClass, ev: u8) -> u32 {
    HANDLERS
        .iter()
        .find(|(c, _)| *c == class)
        .and_then(|(_, t)| t.get(usize::from(ev)).copied())
        .unwrap_or(0)
}

/// The event-14 callback `0x00554570` (skill cooldown end, §6.1).
pub const SKILL_COOLDOWN: CallbackId = CallbackId(0x0055_4570);

/// Number of active-state functions (`0x007322B0`, §10.2).
pub const ACTIVE_STATE_FUNCTIONS: u16 = 191;

/// Item replenish delay at the start (`0x00558530`, `0x00558580`,
/// `0x0055A2A0`): f + 2500 / r + 1, integer division, no minimum.
/// `r ≠ 0` (callers skip r = 0).
pub fn replenish_start_delay(r: i32) -> i32 {
    (2500 / r).wrapping_add(1)
}

/// Item replenish delay in the handler (`0x00562C40`): f + max(2500 / r
/// + 1, 125).
pub fn replenish_delay(r: i32) -> i32 {
    replenish_start_delay(r).max(125)
}

/// The unit records, stat lists and tables of one game, with the hooks
/// into other systems: the [`EventDispatch`] for timer events.
pub struct UnitSystem<H> {
    pub units: Units,
    pub stats: StatLists,
    pub data: UnitData,
    pub hooks: H,
    /// Handler errors (fatal assertions of 1.14d, API misuse), in order.
    pub errors: Vec<(TimerRun, UnitError)>,
}

impl<H: UnitHooks> UnitSystem<H> {
    pub fn new(stat_data: Arc<StatData>, data: UnitData, hooks: H) -> Self {
        Self {
            units: Units::new(),
            stats: StatLists::new(stat_data),
            data,
            hooks,
            errors: Vec::new(),
        }
    }

    /// The [`Sim`] view on `game` and this system, with the hooks.
    pub fn with<R>(&mut self, game: &mut Game, f: impl FnOnce(&mut Sim<'_>, &mut H) -> R) -> R {
        let mut sim = Sim {
            game,
            units: &mut self.units,
            stats: &mut self.stats,
            data: &self.data,
        };
        f(&mut sim, &mut self.hooks)
    }
}

impl<H: UnitHooks> EventDispatch for UnitSystem<H> {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        let r = self.with(game, |sim, hooks| dispatch(sim, hooks, run));
        if let Err(e) = r {
            self.errors.push((*run, e));
        }
    }
}

/// One timer event (`tick.md` §5.5 "Running", §5.6; units.md §5).
pub fn dispatch<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    run: &TimerRun,
) -> Result<(), UnitError> {
    let unit = run.owner.unit;
    let (ev, a1, a2) = (run.event, run.arg1, run.arg2);
    if let Some(cb) = run.callback {
        if cb != SKILL_COOLDOWN {
            return Err(UnitError::UnknownCallback(cb.0));
        }
        hooks.cooldown_end(sim, unit, a1, a2);
        return Ok(());
    }
    if run.class != TimerClass::Missile && handler(run.class, ev) == 0 {
        return match run.class {
            TimerClass::Object | TimerClass::Item => Err(UnitError::NullHandler {
                ty: run.owner.unit_type,
                event: ev,
            }),
            _ => Ok(()),
        };
    }
    match run.class {
        TimerClass::Player => player(sim, hooks, unit, ev, a1, a2),
        TimerClass::Monster => {
            if monster_dropped_when_frozen(ev)
                && sim.stats.has_state(unit, state::FREEZE)
                && !sim.units.is_dead(unit)
            {
                return Ok(());
            }
            monster(sim, hooks, unit, ev, a1, a2)
        }
        TimerClass::Missile => {
            hooks.missile_do(sim, unit);
            Ok(())
        }
        TimerClass::Object => {
            hooks.object_event(sim, unit, ev);
            Ok(())
        }
        TimerClass::Item => {
            match ev {
                event::STAT_REGEN => hooks.item_replenish(sim, unit),
                event::REMOVE_STATE => remove_state(sim, hooks, unit)?,
                // 0x0055F120 returns at once.
                _ => {}
            }
            Ok(())
        }
    }
}

fn player<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    ev: u8,
    a1: u32,
    a2: u32,
) -> Result<(), UnitError> {
    match ev {
        event::MODE_CHANGE => modes::player_event0(sim, hooks, unit, a1, a2)?,
        event::END_ANIM => modes::player_event1(sim, hooks, unit)?,
        event::STAT_REGEN => player_regen(sim, hooks, unit, a1, a2)?,
        event::ACTIVE_STATE => active_state(sim, hooks, unit, a1, a2),
        event::FREE_HOVER => hover(sim, hooks, unit)?,
        event::PERIODIC_SKILLS => hooks.periodic_skills(sim, unit, a1, a2),
        event::PERIODIC_STATS => periodic_stats(sim, hooks, unit, a1, a2),
        event::DELAYED_PORTAL => {
            hooks.player_refresh(sim, unit);
            let f = sim.game.frame;
            sim.game.schedule_event(
                unit,
                u32::from(event::DELAYED_PORTAL),
                f.wrapping_add(30),
                None,
                0,
                0,
            )?;
        }
        event::REMOVE_STATE => remove_state(sim, hooks, unit)?,
        event::UPDATE_TRADE => hooks.update_trade(sim, unit, a1, a2),
        _ => {}
    }
    Ok(())
}

fn monster<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    ev: u8,
    a1: u32,
    a2: u32,
) -> Result<(), UnitError> {
    match ev {
        event::MODE_CHANGE => {
            // trigger(U) of the attack-family event 0 `0x005A7670` reads
            // unit +0x4E = 1 (`skills/use.md` §5.2 "Monsters").
            // PROVISIONAL (sim/units.md §4.2; REC-701): the frame code of
            // the type-0 timer (arg 1) is stored in +0x4E here; which code
            // writes it is not specified. The every-tick event of a moving
            // mode (§4.4, args 0) is no frame event: +0x4E stays the frame
            // advance's (SQ, `skills/sequences.md` §3).
            if a1 != 0 {
                if let Some(r) = sim.units.get_mut(unit) {
                    r.anim.action_frame = a1 as u8;
                }
            }
            modes::monster_event(sim, hooks, unit, false)?
        }
        event::END_ANIM => modes::monster_event(sim, hooks, unit, true)?,
        event::AI_THINK => hooks.ai_think(sim, unit, a1, a2),
        event::STAT_REGEN => monster_regen(sim, hooks, unit)?,
        event::ACTIVE_STATE => active_state(sim, hooks, unit, a1, a2),
        event::FREE_HOVER => hover(sim, hooks, unit)?,
        event::MON_UMOD => hooks.monster_umod(sim, unit, a1, a2),
        event::PERIODIC_SKILLS => hooks.periodic_skills(sim, unit, a1, a2),
        event::PERIODIC_STATS => periodic_stats(sim, hooks, unit, a1, a2),
        event::AI_RESET => hooks.ai_reset(sim, unit, a1, a2),
        event::REMOVE_STATE => remove_state(sim, hooks, unit)?,
        _ => {}
    }
    Ok(())
}

/// Event 12 (`0x00580800`, `0x005A7EF0`, `0x0055F130`): `0x00627460`(unit,
/// frame), `stat-lists.md` §10.4.
fn remove_state<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    let f = sim.game.frame;
    sim.stats.expire_lists(hooks, unit, f)?;
    hooks.lists_expired(sim, unit);
    Ok(())
}

/// Event 6 (`0x00580B70`, `0x005A7F00`, units.md §6.1).
fn hover<H: UnitHooks>(sim: &mut Sim<'_>, hooks: &mut H, unit: UnitId) -> Result<(), UnitError> {
    let f = sim.game.frame;
    // TODO(units.md §6.1): the handler without a hover is not described;
    // nothing here.
    let Some(timeout) = sim.units.get(unit).and_then(|r| r.hover) else {
        return Ok(());
    };
    if timeout <= f {
        hooks.free_hover(sim, unit);
        let rec = sim
            .units
            .get_mut(unit)
            .ok_or(UnitError::UnknownUnit(unit))?;
        rec.hover = None;
        rec.flags |= flags::HOVER_FREED;
        sim.game
            .lists
            .queue_update(unit)
            .map_err(crate::game::GameError::from)?;
    } else {
        sim.game
            .schedule_event(unit, u32::from(event::FREE_HOVER), timeout, None, 0, 0)?;
    }
    Ok(())
}

/// Event 5 `0x0056D790` (`stat-lists.md` §10.2).
fn active_state<H: UnitHooks>(sim: &mut Sim<'_>, hooks: &mut H, unit: UnitId, skill: u32, a2: u32) {
    let d = sim.stats.data();
    if skill == 0 || skill as usize >= d.aurastate.len() {
        return;
    }
    let s = d.aurastate[skill as usize];
    let Some(f) = d.states.srvactivefunc(u32::from(s)) else {
        return;
    };
    if f < ACTIVE_STATE_FUNCTIONS {
        hooks.active_state(sim, unit, f, skill, a2);
    }
}

/// Event 9 `0x0056FE40` (`stat-lists.md` §10.3).
fn periodic_stats<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    a1: u32,
    skill: u32,
) {
    let d = sim.stats.data();
    // TODO(stat-lists.md §10.3): "invalid" skill read as ≥ the skills
    // count (event 5 also rejects 0).
    let valid = (skill as usize) < d.aurastate.len()
        && (d.aurastate[skill as usize] as usize) < d.states.count();
    let cancel = |sim: &mut Sim<'_>| {
        let arg = (a1 != 0).then_some(a1);
        sim.game
            .timers
            .cancel_unit_events(unit, event::PERIODIC_STATS, arg);
    };
    if !valid || sim.units.is_dead(unit) {
        cancel(sim);
        return;
    }
    let l = sim.stats.unit_total(unit, stat::ITEM_AURA, skill as u16);
    if l <= 0 {
        cancel(sim);
        return;
    }
    hooks.apply_item_aura(sim, unit, a1, skill, l);
}

/// The life-fraction update of `stat-lists.md` §10.1 (players step 3,
/// monsters step 5).
fn fraction_update<H: UnitHooks>(sim: &mut Sim<'_>, hooks: &mut H, unit: UnitId) {
    let hp = sim.stats.unit_total(unit, stat::HITPOINTS, 0);
    let f = life_fraction(hp, sim.stats.max_life(unit));
    let last = sim.stats.unit_total(unit, stat::LAST_SENT_HP_PCT, 0);
    if fraction_changed(f, last) {
        hooks.send_life_fraction(sim, unit, f);
        sim.stats
            .unit_set(hooks, unit, stat::LAST_SENT_HP_PCT, f, 0);
    }
}

/// Player regeneration `0x00580810` (`stat-lists.md` §10.1).
///
/// TODO(units.md §6.1 vs stat-lists.md §10.1): units.md says the
/// stamina and mana steps run "if `0x00580610`"; stat-lists.md lists the
/// three steps unconditionally. d2rs follows stat-lists.md (the owner of
/// the internals). The fraction update is read as part of the r ≠ 0
/// branch.
pub fn player_regen<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
    a1: u32,
    a2: u32,
) -> Result<(), UnitError> {
    let f = sim.game.frame;
    sim.game.schedule_event(
        unit,
        u32::from(event::STAT_REGEN),
        f.wrapping_add(1),
        None,
        a1,
        a2,
    )?;
    if sim.units.is_dead(unit) {
        return Ok(());
    }
    // Life (0x00580610).
    let r = sim.stats.unit_total(unit, stat::HPREGEN, 0);
    if r != 0 {
        let mut hp = sim
            .stats
            .unit_total(unit, stat::HITPOINTS, 0)
            .wrapping_add(r);
        let m = sim.stats.max_life(unit);
        if hp > m {
            hp = m;
            sim.stats.free_state_list(hooks, unit, state::HEALTHPOT);
            // The list's remove callback (`0x0056E900`: the state off)
            // runs with the free (`stat-lists.md` §8.2 rule 6).
            hooks.lists_expired(sim, unit);
        }
        if hp < 256 {
            hp = 256;
        }
        sim.stats.unit_set(hooks, unit, stat::HITPOINTS, hp, 0);
        fraction_update(sim, hooks, unit);
    }
    stamina_regen(sim, hooks, unit)?;
    mana_regen(sim, hooks, unit)?;
    Ok(())
}

/// Stamina `0x00580500`.
fn stamina_regen<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    let mode = sim
        .units
        .get(unit)
        .ok_or(UnitError::UnknownUnit(unit))?
        .mode;
    let s = sim.stats.unit_total(unit, stat::STAMINA, 0);
    let b = sim.stats.unit_total(unit, stat::STAMINARECOVERYBONUS, 0);
    let shift = match mode {
        1 | 5 => 8,
        2 if s as u32 & 0xFFFF_FF00 != 0 => 9,
        2 => return Ok(()),
        6 => 9,
        _ if b >= 1000 => 8,
        _ => return Ok(()),
    };
    let m = sim.stats.max_stamina(unit);
    if s >= m {
        return Ok(());
    }
    let mut i = m >> shift;
    if b != 0 {
        i = i.wrapping_add(i.wrapping_mul(b) / 100);
    }
    let s = s.wrapping_add(i).min(m);
    sim.stats.unit_set(hooks, unit, stat::STAMINA, s, 0);
    Ok(())
}

/// Mana `0x005806F0`.
fn mana_regen<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    let class = sim
        .units
        .get(unit)
        .ok_or(UnitError::UnknownUnit(unit))?
        .class;
    let v = sim.stats.unit_total(unit, stat::MANA, 0);
    let m = sim.stats.max_mana(unit);
    let mut i = 0i32;
    if !sim.stats.has_state(unit, state::NOMANAREGEN) {
        let regen = sim
            .stats
            .data()
            .classes
            .get(class as usize)
            .map_or(0, |c| i32::from(c.mana_regen));
        let q = match regen * 25 {
            0 => 7500,
            q => q,
        };
        i = (m / q).max(1);
        let pct = sim
            .stats
            .unit_total(unit, stat::MANARECOVERYBONUS, 0)
            .wrapping_add(100);
        i = muldiv(i, pct, 100);
    }
    i = i.wrapping_add(sim.stats.unit_total(unit, stat::MANARECOVERY, 0));
    if v >= m && i > 0 {
        sim.stats.free_state_list(hooks, unit, state::MANAPOT);
        hooks.lists_expired(sim, unit);
    }
    i = i.min(m.wrapping_sub(v));
    i = i.max(v.wrapping_neg());
    if i != 0 {
        sim.stats.unit_add(hooks, unit, stat::MANA, i, 0);
    }
    Ok(())
}

/// Monster regeneration `0x005A6920` (`stat-lists.md` §10.1).
pub fn monster_regen<H: UnitHooks>(
    sim: &mut Sim<'_>,
    hooks: &mut H,
    unit: UnitId,
) -> Result<(), UnitError> {
    let mut r = sim.stats.unit_total(unit, stat::HPREGEN, 0);
    if sim.stats.has_group(unit, group::LIFE) {
        r = r.wrapping_sub(sim.stats.unit_base(unit, stat::HPREGEN, 0));
    }
    if sim.stats.has_state(unit, state::PREVENTHEAL) && r >= 0 {
        return Ok(());
    }
    let f = sim.game.frame;
    sim.game.schedule_event(
        unit,
        u32::from(event::STAT_REGEN),
        f.wrapping_add(1),
        None,
        0,
        0,
    )?;
    if r == 0 {
        sim.game
            .timers
            .cancel_unit_events(unit, event::STAT_REGEN, None);
        return Ok(());
    }
    let mut hp = sim.stats.unit_total(unit, stat::HITPOINTS, 0);
    let m = sim.stats.max_life(unit);
    if r < 0 && hp < 256 {
        let has_room = sim.game.lists.unit(unit).and_then(|u| u.room()).is_some();
        if !has_room || hooks.room_flag(sim, unit) {
            return Ok(());
        }
    }
    hp = hp.wrapping_add(r);
    if hp > m {
        sim.game
            .timers
            .cancel_unit_events(unit, event::STAT_REGEN, None);
        hp = m;
    }
    if hp < 1 {
        hp = 0;
    }
    sim.stats.unit_set(hooks, unit, stat::HITPOINTS, hp, 0);
    fraction_update(sim, hooks, unit);
    let mode = sim
        .units
        .get(unit)
        .ok_or(UnitError::UnknownUnit(unit))?
        .mode;
    if hp == 0 && !matches!(mode, 0 | 12) {
        let owner = sim
            .stats
            .state_list_owner(unit, state::POISON)
            .or_else(|| sim.stats.state_list_owner(unit, state::OPENWOUNDS));
        let killer = owner.and_then(|(ty, guid)| {
            let ty = UnitType::ALL.get(ty as usize).copied()?;
            sim.game.lists.find_unit(ty, guid)
        });
        // TODO(stat-lists.md §10.1 step 6): unit +0xB0 := 0 (field not
        // described) is not modelled.
        if sim.stats.has_state(unit, state::UNINTERRUPTABLE) {
            let t = sim.stats.toggle_state(unit, state::DEATH_DELAY, true);
            if let (Some(d), Some(rec)) = (t.disguise, sim.units.get_mut(unit)) {
                if d {
                    rec.flags2 |= flags2::DISGUISE;
                } else {
                    rec.flags2 &= !flags2::DISGUISE;
                }
            }
            return Ok(());
        }
        hooks.monster_death(sim, unit, killer);
    }
    Ok(())
}
