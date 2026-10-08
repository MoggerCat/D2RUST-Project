// Spec: specs/formats/d2s.md §7.2 (skills), §8.1 / §8.2 (items), specs/world/waypoints.md §3
//! The rest of a played character's save (q-save-full): items, skill
//! levels and waypoints, read from the running game at save time
//! ([`read_extra`]) and made again at the join ([`join_items`]). d2rs-own,
//! unverified: the placement of loaded items is PROVISIONAL (REC-115).

use d2_formats::d2s::{Body, Corpse, D2s, ItemEntry};
use d2_sim::skills::list::class_skills;
use d2_sim::units::UnitId;

use super::single_player::Sim;

/// What the running game says beyond the base stats and quests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extra {
    /// The player's item list (`None`: no inventory model, e.g. synthetic
    /// data; the loaded list passes through).
    pub items: Option<Vec<ItemEntry>>,
    /// Base level per class skill, in class-list order (`d2s.md` §7.2 r2).
    pub skills: Option<Vec<u8>>,
    /// The three 16-byte waypoint records (`waypoints.md` §3).
    pub waypoints: Option<[[u8; 16]; 3]>,
    /// The player's corpse with its items (`d2s.md` §8.3: the one with the
    /// highest score is saved; this preview has at most the newest per
    /// owner). `None`: no inventory model; the loaded section passes
    /// through. `Some(vec![])`: no corpse with items.
    pub corpses: Option<Vec<Corpse>>,
}

/// Reads [`Extra`] from the game's `player`.
pub fn read_extra(sim: &mut Sim, player: UnitId) -> Extra {
    let items = sim
        .world
        .save_items(&mut sim.game, &mut sim.events, player)
        .ok();
    let corpses = items.as_ref().map(|_| read_corpses(sim, player));
    let class = sim
        .events
        .action
        .sys
        .units
        .get(player)
        .map_or(-1, |u| u.class as i32);
    let h = sim.events.action.hooks();
    let skills = h.skill_lists.get(&player).and_then(|list| {
        let order = class_skills(&h.tables.skills.skills, class);
        if order.is_empty() {
            return None;
        }
        let levels = list.base_levels();
        Some(
            order
                .iter()
                .map(|&s| {
                    levels
                        .iter()
                        .find(|&&(k, _)| i32::from(k) == s)
                        .map_or(0, |&(_, l)| l)
                })
                .collect(),
        )
    });
    let waypoints = h.waypoints.get(&player).and_then(|w| {
        let mut w = *w;
        let mut out = [[0u8; 16]; 3];
        for (o, r) in out.iter_mut().zip(w.0.iter_mut()) {
            *o = r.out_copy().ok()?;
        }
        Some(out)
    });
    Extra {
        items,
        skills,
        waypoints,
        corpses,
    }
}

/// The corpses of `player` that lie dead (mode 17) and hold items, as
/// save sections (`d2s.md` §8.3). d2rs-own, unverified (REC-136): x and y
/// are 0, the value measured for a corpse saved after a town respawn
/// (rule 6); the unknown u32 is 0 (rule 3); of several corpses the
/// newest is kept (the spec scores them by repair cost).
fn read_corpses(sim: &mut Sim, player: UnitId) -> Vec<Corpse> {
    let Some(guid) = sim.events.action.sys.units.get(player).map(|u| u.guid) else {
        return Vec::new();
    };
    let sys = &sim.events.action.sys;
    let mut mine: Vec<UnitId> = sys
        .hooks
        .death
        .owners
        .iter()
        .filter(|&(&c, &o)| o == guid && sys.units.get(c).is_some_and(|r| r.mode == 17))
        .map(|(&c, _)| c)
        .collect();
    mine.sort();
    let mut out = Vec::new();
    for c in mine.into_iter().rev() {
        match sim.world.save_items(&mut sim.game, &mut sim.events, c) {
            Ok(items) if !items.is_empty() => {
                out.push(Corpse {
                    unk: 0,
                    x: 0,
                    y: 0,
                    items,
                });
                break;
            }
            _ => {}
        }
    }
    out
}

/// `body` with the [`Extra`] laid over it.
pub fn apply_extra(body: &mut Body, extra: &Extra) {
    if let Some(items) = &extra.items {
        body.items = items.clone();
    }
    if let Some(levels) = &extra.skills {
        for (slot, &l) in body.skills.iter_mut().zip(levels) {
            *slot = l;
        }
    }
    if let Some(records) = &extra.waypoints {
        body.waypoints.records = *records;
    }
    if let Some(c) = &extra.corpses {
        body.corpses = c.clone();
    }
}

/// The join's item load (`d2s.md` §8.2): makes the save's items on
/// `player`, queues their messages as the join's item messages and logs
/// what failed. True when nothing failed (the load report's "items" step
/// is then applied).
pub fn join_items(s: &mut Sim, player: UnitId, save: &D2s) -> bool {
    let Some(body) = &save.body else {
        return true;
    };
    if body.items.is_empty() {
        return true;
    }
    let loaded = s
        .world
        .load_items(&mut s.game, &mut s.events, player, &body.items);
    let own: Vec<Vec<u8>> = loaded
        .sent
        .iter()
        .filter(|(u, _)| *u == player)
        .map(|(_, b)| b.clone())
        .collect();
    if !own.is_empty() {
        s.events
            .action
            .sys
            .hooks
            .session
            .join_items
            .insert(player, own);
    }
    let log = &mut s.events.action.hooks().x.log;
    log.extend(
        loaded
            .faults
            .iter()
            .map(|f| format!("join: save load: items: {f}")),
    );
    loaded.faults.is_empty()
}
