// Spec: specs/sim/units.md §4.6 (rules 1.2, 1.3, 3.1, 4; "What keeps a dead monster dead"); specs/sim/stat-lists.md §8.8; specs/monsters/ai.md §3.1
//! The monster death clean-up `0x005A6520` of the DT start `0x005A6FF0`
//! and the DD start `0x005A7390` (`units.md` §4.6 rules 1.2 and 4), and
//! the room's dead-GUID ring `0x0061AFA0` of rule 1.3.
//!
//! The clean-up runs in rule 1.2's order: the overhead record freed,
//! the pack-leader handover `0x0058F6C0`, the target-node list leave
//! `0x005B1A90`, flags &= ~0x800C, the stat-list death `0x00627540`,
//! the state keep mask `0x00639FB0`, the `hide` flag, the dead-body
//! footprint `0x00649F70` (unless monstats2 flag 0x13), the path
//! direction snap `0x006488A0` and, last, `0x005738D0`: U's type-2
//! (think) and type-3 events cancelled, so no think runs on the dead
//! unit (the cause of "killed monsters return to mode 1 with hp 0").
//!
//! A host's DT start ([`Pending::monster_death_start`]) sets mode 0 and
//! calls [`death_cleanup`] before its treasure gate, as 1.14d does; for
//! a host that does not, the wiring runs it after the host's start
//! ([`ActionHooks::monster_death`]).
//!
//! Seams: the target-node lists are the host's
//! ([`Pending::target_nodes`]): only unit +0xD0 is written here; the
//! pack handover acts on the AI store's controls and is skipped while
//! the store is lent (a kill inside a think).

use crate::game::Game;
use crate::monsters::ai::{self, AiStore, UnitRef};
use crate::stats::states::group;
use crate::tick::events::event;
use crate::units::hooks::{Sim, UnitHooks};
use crate::units::modes::{self, monster_mode};
use crate::units::record::{flags, INITIAL_NODE_INDEX};
use crate::units::{UnitId, UnitType};

use super::{ActionHooks, Pending, View, WiringError};

/// Unit flags the clean-up clears (`units.md` §4.6 rule 1.2).
const CLEARED_FLAGS: u32 = 0x800C;
/// AI control flag bits the handover reads (`0x0058F6C0`).
const CONTROL_LEADER: u16 = 0x1;
const CONTROL_HANDOVER: u16 = 0x2;

impl<X: Pending> ActionHooks<X> {
    /// The DT start through the host ([`Pending::monster_death_start`])
    /// with the clean-up guaranteed and the dead-GUID ring after it
    /// (`units.md` §4.6 rules 1.2–1.3).
    pub(super) fn monster_death(
        &mut self,
        sim: &mut Sim<'_>,
        unit: UnitId,
        target: Option<UnitId>,
    ) -> bool {
        self.death_cleaned = None;
        let started = X::monster_death_start(self, sim, unit, target);
        if started {
            if self.death_cleaned != Some(unit) {
                death_cleanup(self, sim, unit);
            }
            record_dead(sim.game, unit);
        }
        self.death_cleaned = None;
        started
    }

    /// The DD start `0x005A7390` (`units.md` §4.6 rule 4): U in a mode
    /// other than 0 → the clean-up; mode 12 by the plain mode set; U's
    /// events of types 8 and 9 cancelled.
    pub(super) fn monster_dead_start(&mut self, sim: &mut Sim<'_>, unit: UnitId) {
        if sim
            .units
            .get(unit)
            .is_some_and(|r| r.mode != monster_mode::DT)
        {
            death_cleanup(self, sim, unit);
        }
        if let Err(e) = modes::set_mode(sim, self, unit, monster_mode::DD) {
            self.errors.push(WiringError::Unit(e));
        }
        for ty in [event::PERIODIC_SKILLS, event::PERIODIC_STATS] {
            sim.game.timers.cancel_unit_events(unit, ty, None);
        }
    }
}

/// The death clean-up `0x005A6520(U, R byte +0x14)` (`units.md` §4.6
/// rule 1.2), with R's direction byte the one the kill writes (`damage.md`
/// §7.2 rule 3): toward the mode change's target
/// ([`ActionHooks::mode_target`]) when there is one, else U's current
/// direction.
pub fn death_cleanup<X: Pending>(h: &mut ActionHooks<X>, sim: &mut Sim<'_>, unit: UnitId) {
    h.death_cleaned = Some(unit);
    let dir = death_direction(h, unit);
    // Overhead record (+0xA4).
    if sim.units.get(unit).is_some_and(|r| r.hover.is_some()) {
        h.free_hover(sim, unit);
        h.session.overheads.remove(&unit);
        if let Some(r) = sim.units.get_mut(unit) {
            r.hover = None;
            r.flags |= flags::HOVER_FREED;
        }
        queue_update(h, sim.game, unit);
    }
    // `0x0058F6C0`: pack-leader handover.
    if let Some(store) = h.ai.as_mut() {
        pack_handover(store, sim.game, unit);
    }
    // `0x005B1A90`: the target-node list leave.
    if let Some(r) = sim.units.get_mut(unit) {
        r.node_index = INITIAL_NODE_INDEX;
        r.flags &= !CLEARED_FLAGS;
    }
    // `0x00627540`, then the remove callbacks of the freed lists.
    sim.stats.death(h, unit);
    h.lists_expired(sim, unit);
    // `0x00639FB0(U, boss)`.
    let keep = match sim.units.get(unit).map(|r| r.ty) {
        Some(UnitType::Monster) if h.x.is_boss(unit) => group::BOSS_STAY_DEATH,
        Some(UnitType::Monster) => group::MON_STAY_DEATH,
        _ => group::PLR_STAY_DEATH,
    };
    sim.stats.clear_states_except(unit, keep);
    queue_update(h, sim.game, unit);
    // `hide` (`0x0063A320`).
    if sim.stats.has_group(unit, group::HIDE) {
        if let Some(r) = sim.units.get_mut(unit) {
            r.flags &= !flags::TILE;
        }
    }
    // `0x00649F70(U, 1)` unless monstats2 flag 0x13 (inside).
    View::of(sim.units, sim.stats, sim.data, h).death_footprint(unit);
    // `0x006488A0(path, dir)`.
    if let (Some(d), Some(p)) = (dir, h.paths.as_mut().and_then(|p| p.dynamic_mut(unit))) {
        p.direction = d;
        p.new_direction = d;
    }
    // `0x005738D0(game, U)`.
    ai::cancel_think_and_regen(sim.game, unit);
}

/// `0x0061AFA0(U's room, U's GUID)` (`units.md` §4.6 rule 3.1); no room
/// → nothing.
pub fn record_dead(game: &mut Game, unit: UnitId) {
    let Some((room, guid)) = game
        .lists
        .unit(unit)
        .and_then(|e| Some((e.room()?, e.guid)))
    else {
        return;
    };
    if let Some(r) = game.lists.room_mut(room) {
        r.record_dead(guid);
    }
}

/// R byte +0x14 of the kill's request: `0x00621DC0(U, A x, A y)` toward
/// the target A, else `0x006487F0` (U's path direction). `None` without
/// a dynamic path (nothing to snap).
// PROVISIONAL (units.md §4.6 rule 3.1, REC-1090): the direction is taken
// between the two path positions in sub-tiles (the coordinates
// `0x0064FDC0` feeds are not stated, `skills/bodies-3.md` §3.8); settled
// by the d byte of a kill's S→C 0x69 code 8 with known positions.
fn death_direction<X: Pending>(h: &ActionHooks<X>, unit: UnitId) -> Option<u8> {
    let current = h.paths.as_ref()?.dynamic(unit)?.direction;
    let Some(a) = h.mode_target.filter(|&a| a != unit) else {
        return Some(current);
    };
    let p = h.paths.as_ref()?;
    let (from, at) = (h.path_position(unit), h.path_position(a));
    let (_, d) = crate::path::walk::geom::direction_vector(
        &p.tables,
        (from.0 as u32, from.1 as u32),
        (at.0 as u32, at.1 as u32),
    );
    Some(d & 63)
}

fn queue_update<X: Pending>(h: &mut ActionHooks<X>, game: &mut Game, unit: UnitId) {
    if let Err(e) = game.lists.queue_update(unit) {
        h.errors
            .push(WiringError::Unit(crate::units::modes::UnitError::Game(
                e.into(),
            )));
    }
}

/// `0x0058F6C0(U)` (`units.md` §4.6 rule 3.1). The minion list is read
/// from its head: 1.14d pushes new minions at the head (`0x0058F100`,
/// `ai.md` §3.1) where d2rs appends, so the head is the vector's end.
fn pack_handover(store: &mut AiStore, game: &Game, unit: UnitId) {
    let Some(c) = store.control(unit) else {
        return;
    };
    if c.flags & CONTROL_LEADER == 0 {
        return;
    }
    let list: Vec<u32> = c.minions.iter().rev().copied().collect();
    let resolve = |g: u32| game.lists.find_unit(UnitType::Monster, g);
    if c.flags & CONTROL_HANDOVER == 0 {
        // `0x0058F660`: the pack is released.
        for m in list.iter().filter_map(|&g| resolve(g)) {
            if let Some(mc) = store.control_mut(m) {
                mc.minion_owner = None;
            }
        }
        return;
    }
    // `0x0058F530`: the first resolving entry N leads.
    let Some(n) = list.iter().find_map(|&g| resolve(g)) else {
        return;
    };
    let Some(n_guid) = game.lists.unit(n).map(|e| e.guid) else {
        return;
    };
    let n_ref = Some(UnitRef {
        ty: UnitType::Monster,
        guid: n_guid,
    });
    if let Some(nc) = store.control_mut(n) {
        nc.flags |= CONTROL_HANDOVER | CONTROL_LEADER;
        nc.minion_owner = n_ref;
    }
    if let Some(c) = store.control_mut(unit) {
        c.minion_owner = n_ref;
    }
    let mut joining = Vec::new();
    if let Some(g) = game.lists.unit(unit).map(|e| e.guid) {
        joining.push(g);
    }
    for &g in &list {
        let Some(m) = resolve(g).filter(|&m| m != n) else {
            continue;
        };
        if let Some(mc) = store.control_mut(m) {
            mc.minion_owner = n_ref;
        }
        joining.push(g);
    }
    if let Some(nc) = store.control_mut(n) {
        nc.minions.extend(joining);
    }
}
