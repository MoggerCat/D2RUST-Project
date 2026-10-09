// Spec: specs/tools/soak.md (§3)
//! The soak's checks: state invariants of the server game (read on the
//! server thread between two frames, shared references only) and the
//! comparison of the client model with the server state (desync). Every
//! check reads; none changes the game.

use std::collections::{BTreeMap, BTreeSet};

use d2_sim::stats::{self, stat, StatLists};
use d2_sim::units::{UnitId, UnitType};

use crate::app::single_player::{self, Sim};
use crate::bridge::items as citems;
use crate::bridge::world::ClientWorld;

/// One broken rule. `sig` names the rule and the units it is about, with
/// no frame or step, so the same break found again (a replay, a reduced
/// log) has the same signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub kind: &'static str,
    pub sig: String,
    pub detail: String,
}

fn v(kind: &'static str, sig: String, detail: String) -> Violation {
    Violation { kind, sig, detail }
}

/// The local player's item as the server places it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerItem {
    pub guid: u32,
    /// Unit record mode.
    pub mode: u32,
    pub page: u8,
    pub body: u8,
    pub x: i32,
    pub y: i32,
}

/// What the desync check compares, read from the server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerView {
    pub frame: u32,
    pub player_guid: Option<u32>,
    pub player_pos: Option<(i32, i32)>,
    /// Life and mana, whole points.
    pub player_hp: Option<i32>,
    pub player_mp: Option<i32>,
    pub player_dead: bool,
    /// The local player's inventory items, by GUID.
    pub items: BTreeMap<u32, ServerItem>,
    pub cursor: Option<u32>,
    /// (type, guid) → class of every listed unit (GUID reuse).
    pub units: BTreeMap<(u8, u32), u32>,
}

fn stat_of(stats: &StatLists, list: Option<stats::ListId>, stat: u16) -> Option<i32> {
    let l = list.filter(|&l| stats.is_live(l))?;
    // Vitals live in the full array only (`debug::state`).
    if !stats.is_extended(l) {
        return None;
    }
    let entries = stats.full_entries(l);
    let k = stats::key(stat, 0);
    entries
        .binary_search_by_key(&k, |e| e.0)
        .ok()
        .map(|i| entries[i].1)
}

/// The invariants of §3 rules 1–5 on the server game, and the view the
/// desync check compares. Reads only.
pub fn server_check(sim: &Sim) -> (Vec<Violation>, ServerView) {
    let game = &sim.game;
    let sys = &sim.events.action.sys;
    let mut out = Vec::new();
    let mut view = ServerView {
        frame: game.frame.max(0) as u32,
        ..ServerView::default()
    };

    // Rule 1: GUIDs. Two listed units of one type with one GUID; a GUID
    // past its type's counter.
    let mut item_ids = BTreeSet::new();
    for ty in UnitType::ALL {
        let mut seen: BTreeMap<u32, UnitId> = BTreeMap::new();
        let counter = game.lists.guids.get(ty);
        for id in game.lists.units_of_type(ty) {
            let Some(e) = game.lists.unit(id) else {
                out.push(v(
                    "list",
                    format!("list:{ty:?}:dangling"),
                    format!("{ty:?} list holds {id:?} with no entry"),
                ));
                continue;
            };
            if let Some(prev) = seen.insert(e.guid, id) {
                out.push(v(
                    "guid",
                    format!("guid:dup:{ty:?}"),
                    format!("{ty:?} GUID {} on {prev:?} and {id:?}", e.guid),
                ));
            }
            if e.guid > counter && ty != UnitType::Tile {
                out.push(v(
                    "guid",
                    format!("guid:past-counter:{ty:?}"),
                    format!("{ty:?} GUID {} past the counter {counter}", e.guid),
                ));
            }
            let class = sys.units.get(id).map_or(u32::MAX, |r| r.class);
            if let Some(r) = sys.units.get(id) {
                if r.guid != e.guid {
                    out.push(v(
                        "list",
                        format!("list:{ty:?}:guid-mismatch"),
                        format!("{id:?}: list GUID {} record GUID {}", e.guid, r.guid),
                    ));
                }
            }
            view.units.insert((ty as u8, e.guid), class);
            if ty == UnitType::Item {
                item_ids.insert(id);
            }
        }
    }

    // Rule 2: vitals. Life, mana, stamina never negative; never above max
    // by more than a point (the max can drop under the value for a frame
    // while an item comes off: allow equality only).
    for ty in [UnitType::Player, UnitType::Monster] {
        for id in game.lists.units_of_type(ty) {
            let list = sys.stats.unit_list(id);
            let guid = game.lists.unit(id).map_or(0, |e| e.guid);
            for (stat, max, name) in [
                (stat::HITPOINTS, stat::MAXHP, "life"),
                (stat::MANA, stat::MAXMANA, "mana"),
                (stat::STAMINA, stat::MAXSTAMINA, "stamina"),
            ] {
                let Some(val) = stat_of(&sys.stats, list, stat) else {
                    continue;
                };
                if val < 0 {
                    out.push(v(
                        "vitals",
                        format!("vitals:negative-{name}:{ty:?}"),
                        format!("{ty:?} {guid}: {name} {val} (1/256) is negative"),
                    ));
                }
                if ty == UnitType::Player {
                    if let Some(m) = stat_of(&sys.stats, list, max) {
                        if m < 0 {
                            out.push(v(
                                "vitals",
                                format!("vitals:negative-max-{name}:{ty:?}"),
                                format!("{ty:?} {guid}: max {name} {m} is negative"),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Rule 3: rooms. A unit with a path stands in its path's room, and the
    // list's room is the path's.
    if let Some(paths) = sys.hooks.paths.as_ref() {
        for ty in [
            UnitType::Player,
            UnitType::Monster,
            UnitType::Object,
            UnitType::Item,
        ] {
            for id in game.lists.units_of_type(ty) {
                let Some(e) = game.lists.unit(id) else {
                    continue;
                };
                let Some(p) = paths.record(id) else { continue };
                let (Some(room), lroom) = (p.room(), e.room()) else {
                    continue;
                };
                let mode = sys.units.get(id).map_or(u32::MAX, |r| r.mode);
                if lroom != Some(room) {
                    out.push(v(
                        "room",
                        format!("room:list-vs-path:{ty:?}"),
                        format!(
                            "{ty:?} {} mode {mode}: path room {room:?}, list room {lroom:?}",
                            e.guid
                        ),
                    ));
                }
                let (x, y) = p.position();
                match sys.hooks.drlg.subtiles(game, room) {
                    Some(r) if !r.contains_closed(x, y) => out.push(v(
                        "room",
                        format!("room:outside:{ty:?}"),
                        format!(
                            "{ty:?} {} at ({x}, {y}) outside its room {room:?} {r:?}",
                            e.guid
                        ),
                    )),
                    _ => {}
                }
            }
        }
    }

    // Rule 4: items in one place. Each item in at most one inventory, at
    // most once in it (items list, cursor, one grid), never also in a
    // room; the item data's inventory is the one that lists it; the unit
    // mode matches the grid.
    if let Some(inv) = sim.world.inventory.as_ref() {
        let state = &inv.state;
        let mut owner_of: BTreeMap<UnitId, UnitId> = BTreeMap::new();
        for (owner, i) in &state.inventories {
            let mut here: BTreeMap<UnitId, Vec<String>> = BTreeMap::new();
            for &it in i.items() {
                here.entry(it).or_default().push("list".into());
            }
            if let Some(c) = i.cursor() {
                here.entry(c).or_default().push("cursor".into());
            }
            for g in 0..i.grid_count() {
                let Some(grid) = i.grid(g) else { continue };
                let cells: BTreeSet<UnitId> = grid.cells.iter().flatten().copied().collect();
                for &it in grid.items.iter().chain(cells.iter()) {
                    let tag = format!("grid{g}");
                    let e = here.entry(it).or_default();
                    if !e.contains(&tag) {
                        e.push(tag);
                    }
                }
                for &it in &cells {
                    if !grid.items.contains(&it) {
                        out.push(v(
                            "item",
                            format!("item:cell-not-in-grid-list:grid{g}"),
                            format!("item {it:?} fills grid {g} cells of {owner:?} but is not in its list"),
                        ));
                    }
                }
            }
            for (it, places) in &here {
                let guid = game.lists.unit(*it).map_or(0, |e| e.guid);
                if let Some(prev) = owner_of.insert(*it, *owner) {
                    if prev != *owner {
                        out.push(v(
                            "item",
                            "item:two-inventories".into(),
                            format!(
                                "item {guid} ({it:?}) in the inventories of {prev:?} and {owner:?}"
                            ),
                        ));
                    }
                }
                let grids: Vec<&String> = places.iter().filter(|p| p.starts_with("grid")).collect();
                if grids.len() > 1 || (places.iter().any(|p| p == "cursor") && !grids.is_empty()) {
                    out.push(v(
                        "item",
                        "item:two-places-in-inventory".into(),
                        format!("item {guid} ({it:?}) of {owner:?} in {places:?}"),
                    ));
                }
                if game.lists.unit(*it).and_then(|e| e.room()).is_some() {
                    out.push(v(
                        "item",
                        "item:inventory-and-room".into(),
                        format!("item {guid} ({it:?}) of {owner:?} is also in a room"),
                    ));
                }
                if let Some(d) = state.items.get(it) {
                    if d.inv.is_some_and(|o| o != *owner) {
                        out.push(v(
                            "item",
                            "item:data-owner".into(),
                            format!(
                                "item {guid}: data says inventory {:?}, listed by {owner:?}",
                                d.inv
                            ),
                        ));
                    }
                }
                if !item_ids.contains(it) && game.lists.unit(*it).is_none() {
                    out.push(v(
                        "item",
                        "item:freed-in-inventory".into(),
                        format!("inventory {owner:?} holds freed unit {it:?}"),
                    ));
                }
                let mode = sys.units.get(*it).map(|r| r.mode);
                let expect = match grids.first().map(|s| s.as_str()) {
                    Some("grid0") => Some(1),
                    Some("grid1") => Some(2),
                    Some(_) => Some(0),
                    None if places.iter().any(|p| p == "cursor") => Some(4),
                    None => None,
                };
                if let (Some(m), Some(x)) = (mode, expect) {
                    // Socketed fillers live in their item's inventory (mode 6).
                    if m != x && m != 6 {
                        out.push(v(
                            "item",
                            format!("item:mode-vs-place:{x}"),
                            format!("item {guid} of {owner:?} in {places:?} has mode {m}"),
                        ));
                    }
                }
            }
        }
        // Ground items (mode 3) in no inventory and in a room.
        for &it in &item_ids {
            let Some(r) = sys.units.get(it) else { continue };
            if r.mode == 3 && owner_of.contains_key(&it) {
                out.push(v(
                    "item",
                    "item:ground-and-inventory".into(),
                    format!(
                        "item {} is on the ground and in {:?}",
                        r.guid, owner_of[&it]
                    ),
                ));
            }
        }

        // The view: the local player's items.
        if let Some((pid, pguid)) = single_player::local_player(sim) {
            view.player_guid = Some(pguid);
            if let Some(i) = state.inventories.get(&pid) {
                view.cursor = i.cursor().and_then(|c| game.lists.unit(c)).map(|e| e.guid);
                for &it in i.items() {
                    let (Some(e), Some(d)) = (game.lists.unit(it), state.items.get(&it)) else {
                        continue;
                    };
                    view.items.insert(
                        e.guid,
                        ServerItem {
                            guid: e.guid,
                            mode: sys.units.get(it).map_or(u32::MAX, |r| r.mode),
                            page: d.page,
                            body: d.body_loc,
                            x: d.x,
                            y: d.y,
                        },
                    );
                }
            }
        }
    }

    if let Some((pid, pguid)) = single_player::local_player(sim) {
        view.player_guid = Some(pguid);
        view.player_pos = sys
            .hooks
            .paths
            .as_ref()
            .and_then(|p| p.record(pid))
            .map(|p| p.position());
        let list = sys.stats.unit_list(pid);
        view.player_hp = stat_of(&sys.stats, list, stat::HITPOINTS).map(|v| v >> 8);
        view.player_mp = stat_of(&sys.stats, list, stat::MANA).map(|v| v >> 8);
        view.player_dead = sys.units.get(pid).is_some_and(|r| r.is_dead());
    }
    (out, view)
}

/// GUID reuse across checks (§3 rule 1): a (type, GUID) that left the
/// lists and came back as another class.
#[derive(Default)]
pub struct GuidHistory {
    last: BTreeMap<(u8, u32), u32>,
    gone: BTreeMap<(u8, u32), u32>,
}

impl GuidHistory {
    pub fn observe(&mut self, now: &BTreeMap<(u8, u32), u32>) -> Vec<Violation> {
        let mut out = Vec::new();
        for (k, class) in now {
            if let Some(old) = self.gone.remove(k) {
                if old != *class {
                    out.push(v(
                        "guid",
                        format!("guid:reused:{}", k.0),
                        format!(
                            "type {} GUID {} left as class {old} and came back as class {class}",
                            k.0, k.1
                        ),
                    ));
                }
            }
        }
        for (k, class) in &self.last {
            if !now.contains_key(k) {
                self.gone.insert(*k, *class);
            }
        }
        self.last = now.clone();
        out
    }
}

/// How many consecutive checks a client/server difference must last
/// before it is a desync: the model trails the server by the message
/// round trip and the walk prediction leads it (`seams/movement-prediction.md`).
pub const DESYNC_CHECKS: u32 = 50;

/// Desync bookkeeping: each difference's signature and how many checks
/// in a row it held.
#[derive(Default)]
pub struct Desync {
    streak: BTreeMap<String, (u32, String)>,
}

impl Desync {
    /// Compares the model with the server view; returns the differences
    /// that have now held for [`DESYNC_CHECKS`] checks (once each).
    pub fn observe(&mut self, w: &ClientWorld, s: &ServerView) -> Vec<Violation> {
        let mut now: BTreeMap<String, String> = BTreeMap::new();
        let local = w.local_player.and_then(|k| w.units.get(&k));
        if let (Some(u), Some(guid)) = (local, s.player_guid) {
            if u.key.guid != guid {
                now.insert(
                    "desync:player-guid".into(),
                    format!("client player {} server {guid}", u.key.guid),
                );
            }
            // The local player's cell is its walk prediction's while the
            // client moves it (`seams/movement-prediction.md` §2.9 r2).
            let cell = w.predicted(u).map(|p| p.cell()).or(u.position);
            if let (Some((cx, cy)), Some((sx, sy))) = (cell, s.player_pos) {
                let d = (i32::from(cx) - sx).abs().max((i32::from(cy) - sy).abs());
                if d > 10 {
                    now.insert(
                        "desync:player-position".into(),
                        format!("client ({cx}, {cy}) server ({sx}, {sy}): {d} sub-tiles"),
                    );
                }
            }
            if let (Some(&chp), Some(shp)) = (u.stats.get(&stat::HITPOINTS), s.player_hp) {
                if chp >> 8 != shp && !s.player_dead {
                    now.insert(
                        "desync:player-life".into(),
                        format!("client {} server {shp}", chp >> 8),
                    );
                }
            }
        }
        if s.player_guid.is_some() {
            let client: BTreeMap<u32, citems::ItemView> = citems::local_items(w)
                .into_iter()
                .filter(|i| !i.store)
                .filter(|i| i.owner.is_none_or(|o| Some(o.guid) == s.player_guid))
                .map(|i| (i.key.guid, i))
                .collect();
            for (g, si) in &s.items {
                match client.get(g) {
                    None => {
                        let model = citems::item(
                            w,
                            crate::bridge::world::UnitKey {
                                unit_type: crate::bridge::world::ITEM,
                                guid: *g,
                            },
                        );
                        now.insert(
                            format!("desync:item-missing-in-client:mode{}", si.mode),
                            format!(
                                "server item {g} {si:?} not the model player's; model: {model:?}"
                            ),
                        );
                    }
                    Some(ci) => {
                        let same = u32::from(ci.mode) == si.mode
                            && match si.mode {
                                0 => {
                                    ci.page == si.page
                                        && i32::from(ci.x) == si.x
                                        && i32::from(ci.y) == si.y
                                }
                                1 => ci.body == si.body,
                                _ => true,
                            };
                        if !same {
                            now.insert(
                                format!("desync:item-place:mode{}", si.mode),
                                format!(
                                    "item {g}: server mode {} page {} body {} ({}, {}); client mode {} page {} body {} ({}, {})",
                                    si.mode, si.page, si.body, si.x, si.y, ci.mode, ci.page, ci.body, ci.x, ci.y
                                ),
                            );
                        }
                    }
                }
            }
            for (g, ci) in &client {
                if !s.items.contains_key(g) && matches!(ci.mode, 0..=2) {
                    now.insert(
                        format!("desync:item-only-in-client:mode{}", ci.mode),
                        format!("model item {g} mode {} not the server player's", ci.mode),
                    );
                }
            }
        }
        let mut out = Vec::new();
        self.streak.retain(|k, _| now.contains_key(k));
        for (k, d) in now {
            let e = self.streak.entry(k.clone()).or_insert((0, String::new()));
            e.0 += 1;
            e.1 = d;
            if e.0 == DESYNC_CHECKS {
                out.push(v("desync", k, e.1.clone()));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_guid_back_as_another_class_is_reuse() {
        let mut h = GuidHistory::default();
        let a: BTreeMap<_, _> = [((1, 5), 10)].into();
        assert!(h.observe(&a).is_empty());
        assert!(h.observe(&BTreeMap::new()).is_empty());
        // Back as the same class (an act change): not reuse.
        assert!(h.observe(&a).is_empty());
        assert!(h.observe(&BTreeMap::new()).is_empty());
        let b: BTreeMap<_, _> = [((1, 5), 11)].into();
        let r = h.observe(&b);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].sig, "guid:reused:1");
    }

    #[test]
    fn a_difference_is_a_desync_only_after_it_held() {
        let mut d = Desync::default();
        let w = ClientWorld::default();
        let s = ServerView {
            player_guid: Some(1),
            items: [(
                7,
                ServerItem {
                    guid: 7,
                    mode: 0,
                    page: 0,
                    body: 0,
                    x: 0,
                    y: 0,
                },
            )]
            .into(),
            ..ServerView::default()
        };
        for _ in 1..DESYNC_CHECKS {
            assert!(d.observe(&w, &s).is_empty());
        }
        let r = d.observe(&w, &s);
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!(r[0].sig, "desync:item-missing-in-client:mode0");
        // Reported once.
        assert!(d.observe(&w, &s).is_empty());
        // A gap resets the streak.
        assert!(d.observe(&w, &ServerView::default()).is_empty());
    }
}
