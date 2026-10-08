// Spec: specs/formats/d2s.md §7.2 (skills), §8.1 / §8.2 (items), specs/world/waypoints.md §3
//! The rest of a played character's save (q-save-full): items, skill
//! levels and waypoints, read from the running game at save time
//! ([`read_extra`]) and made again at the join ([`join_items`]). d2rs-own,
//! unverified: the placement of loaded items is PROVISIONAL (REC-115).

use d2_formats::d2s::{Body, D2s, ItemEntry};
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
}

/// Reads [`Extra`] from the game's `player`.
pub fn read_extra(sim: &mut Sim, player: UnitId) -> Extra {
    let items = sim
        .world
        .save_items(&mut sim.game, &mut sim.events, player)
        .ok();
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
    }
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
