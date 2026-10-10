// Spec: specs/tools/state-snapshot.md
//! The `state-1` game-state snapshot (§1–§3): the game seed and every
//! server unit with the fields of the spec's §2 table, one JSON line per
//! server tick; plus the header and footer lines.
//!
//! [`snapshot`] reads a wired game ([`Game`] and the action wiring's
//! [`UnitSystem`]) through shared references only: no RNG step, no GUID,
//! no list or record change (spec Outputs). Running it or not gives the
//! same game.
//!
//! Where each d2rs value lives (the 1.14d read is the spec's §2 table):
//!
//! | Key | d2rs home |
//! |---|---|
//! | `ut`, `g` | [`crate::units::UnitLists`] (the five hash lists and the tile list) |
//! | `cl`, `m`, `act`, `s` | [`crate::units::record::UnitRecord`] `class`, `mode`, `act`, `seed` |
//! | `fr`, `fc`, `sp` | the record's [`crate::units::record::Anim`] `frame`, `frame_count`, `speed` |
//! | `x` … `d` | the path provider's record ([`crate::wiring::path::PathState`]): [`DynamicPath`] or [`StaticPath`] |
//! | `lv` | the path's room → its DRLG room's level ([`crate::wiring::action::DrlgWorld::level_id`]) |
//! | stats | the unit's list in [`crate::stats::StatLists`] (unit +0x5C): full array (6–11), base array (0–3, 12) |
//! | `seed` | [`ActionHooks::game_seed`] (game +0xD0) |
//!
//! `own` (spec §2, `sim/units.md` §2 "Owner links"): a monster's AI
//! control minion owner, a missile's stored owner ([`owner`]); an item's
//! holder is the inventory model's, which the host overlays (the export
//! in `d2-client` `state-dump`). The player and object types have none.
//!
//! `q` (the player's quest flag record, [`HOST_FIELDS`]) is not in the
//! wired game: the host keeps the players' records (`PlayerQuests`) and
//! fills it with [`StateSnapshot::set_quests`] from [`quest_words`].

use std::fmt::Write as _;

use crate::game::Game;
use crate::path::record::{DynamicPath, StaticPath};
use crate::path::UnitPath;
use crate::stats::{key, key_layer, key_stat, ListId, StatLists};
use crate::units::dispatch::UnitSystem;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::ActionHooks;
use crate::wiring::worldgen::WorldSim;
use crate::world::quests::{QuestFlags, SLOTS};

/// The format name of line 1 (§1 rule 1).
pub const FORMAT: &str = "state-1";

/// Every unit key of §2, in table order (the comparison order).
pub const FIELDS: [&str; 41] = [
    "ut", "g", "cl", "m", "x", "y", "xf", "yf", "tx", "ty", "d", "fr", "fc", "sp", "s", "act",
    "lv", "hp", "hpx", "mp", "mpx", "st", "stx", "str", "ene", "dex", "vit", "lvl", "own", "iq",
    "if", "fi", "il", "aa", "pf", "sf", "rp", "rs", "ik", "ss", "is",
];

/// Keys a host fills from records it keeps outside the wired game (§2:
/// `q`, the player's quest flag record of the game's difficulty). A host
/// that fills them adds them to the header `fields`; [`coverage`] does
/// not list them.
pub const HOST_FIELDS: [&str; 1] = ["q"];

/// The keys whose values need the path provider (path +0x00 … +0x64, and
/// the level of the path's room).
pub const PATH_FIELDS: [&str; 8] = ["x", "y", "xf", "yf", "tx", "ty", "d", "lv"];

/// The keys d2rs never fills, with why (header `gaps`, one line each).
pub const D2RS_GAPS: [(&str, &str); 0] = [];

/// One unit of a snapshot (§2). `None`: the key is absent from the line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitState {
    /// Unit type 0–5.
    pub ut: u8,
    /// GUID (per type).
    pub g: u32,
    pub cl: Option<u32>,
    pub m: Option<u32>,
    /// Sub-tile position: u16 (dynamic) or u32 (static), unsigned.
    pub x: Option<u32>,
    pub y: Option<u32>,
    /// Low 16 bits of the 16.16 position (dynamic paths).
    pub xf: Option<u16>,
    pub yf: Option<u16>,
    /// Path target (dynamic paths).
    pub tx: Option<u16>,
    pub ty: Option<u16>,
    /// Direction byte as stored.
    pub d: Option<u8>,
    /// Animation frame and frame count, 8.8 raw.
    pub fr: Option<i32>,
    pub fc: Option<i32>,
    /// Animation speed.
    pub sp: Option<i16>,
    /// Unit seed `[lo, hi]`.
    pub s: Option<[u32; 2]>,
    pub act: Option<u8>,
    /// Level id of the unit's room.
    pub lv: Option<u32>,
    /// Stats 6–11 layer 0, raw (8.8), full array.
    pub hp: Option<i32>,
    pub hpx: Option<i32>,
    pub mp: Option<i32>,
    pub mpx: Option<i32>,
    pub st: Option<i32>,
    pub stx: Option<i32>,
    /// Stats 0, 1, 2, 3, 12 layer 0, base array.
    pub str: Option<i32>,
    pub ene: Option<i32>,
    pub dex: Option<i32>,
    pub vit: Option<i32>,
    pub lvl: Option<i32>,
    /// Owner GUID.
    pub own: Option<u32>,
    /// Item data (§2, items only): quality +0x00, flags +0x18, file index
    /// +0x28, item level +0x2C, auto affix +0x36, magic prefixes +0x38…,
    /// suffixes +0x3E…, rare prefix / suffix +0x32 / +0x34, item seed
    /// +0x04, start seed +0x10.
    pub iq: Option<u8>,
    pub ifl: Option<u32>,
    pub fi: Option<i32>,
    pub il: Option<i32>,
    pub aa: Option<u16>,
    pub pf: Option<[u16; 3]>,
    pub sf: Option<[u16; 3]>,
    pub rp: Option<u16>,
    pub rs: Option<u16>,
    pub ik: Option<[u32; 2]>,
    pub ss: Option<u32>,
    /// The item's stat list base array: `[stat, layer, value]` in key
    /// order.
    pub is: Option<Vec<[i32; 3]>>,
    /// Players: the quest flag record of the game's difficulty as
    /// `[slot, word]` for every slot whose 16-bit word is non-zero, slots
    /// ascending ([`quest_words`]).
    pub q: Option<Vec<[u16; 2]>>,
}

/// One snapshot (§1 rule 2): the state after tick `frame` (§3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateSnapshot {
    pub frame: i32,
    /// The game seed `[lo, hi]`.
    pub seed: [u32; 2],
    /// Sorted by (`ut`, `g`) ([`StateSnapshot::sort_units`]).
    pub units: Vec<UnitState>,
}

impl StateSnapshot {
    /// Sets `q` of the player unit with GUID `guid` (no such unit:
    /// nothing).
    pub fn set_quests(&mut self, guid: u32, words: Vec<[u16; 2]>) {
        if let Some(u) = self
            .units
            .iter_mut()
            .find(|u| u.ut == UnitType::Player as u8 && u.g == guid)
        {
            u.q = Some(words);
        }
    }

    /// Sorts the units by (`ut`, `g`) ascending (§1 rule 2).
    pub fn sort_units(&mut self) {
        self.units.sort_by_key(|u| (u.ut, u.g));
    }

    /// The `snap` line (no newline): `{"k":"snap","f":..,"seed":[lo,hi],
    /// "units":[...]}`, units in their stored order, keys in §2 table
    /// order, absent keys omitted.
    pub fn to_json_line(&self) -> String {
        let mut o = String::with_capacity(64 + self.units.len() * 160);
        let _ = write!(
            o,
            "{{\"k\":\"snap\",\"f\":{},\"seed\":[{},{}],\"units\":[",
            self.frame, self.seed[0], self.seed[1]
        );
        for (i, u) in self.units.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            unit_json(&mut o, u);
        }
        o.push_str("]}");
        o
    }
}

fn unit_json(o: &mut String, u: &UnitState) {
    let _ = write!(o, "{{\"ut\":{},\"g\":{}", u.ut, u.g);
    fn num<T: std::fmt::Display>(o: &mut String, k: &str, v: Option<T>) {
        if let Some(v) = v {
            let _ = write!(o, ",\"{k}\":{v}");
        }
    }
    num(o, "cl", u.cl);
    num(o, "m", u.m);
    num(o, "x", u.x);
    num(o, "y", u.y);
    num(o, "xf", u.xf);
    num(o, "yf", u.yf);
    num(o, "tx", u.tx);
    num(o, "ty", u.ty);
    num(o, "d", u.d);
    num(o, "fr", u.fr);
    num(o, "fc", u.fc);
    num(o, "sp", u.sp);
    if let Some([lo, hi]) = u.s {
        let _ = write!(o, ",\"s\":[{lo},{hi}]");
    }
    num(o, "act", u.act);
    num(o, "lv", u.lv);
    num(o, "hp", u.hp);
    num(o, "hpx", u.hpx);
    num(o, "mp", u.mp);
    num(o, "mpx", u.mpx);
    num(o, "st", u.st);
    num(o, "stx", u.stx);
    num(o, "str", u.str);
    num(o, "ene", u.ene);
    num(o, "dex", u.dex);
    num(o, "vit", u.vit);
    num(o, "lvl", u.lvl);
    num(o, "own", u.own);
    num(o, "iq", u.iq);
    num(o, "if", u.ifl);
    num(o, "fi", u.fi);
    num(o, "il", u.il);
    num(o, "aa", u.aa);
    if let Some([a, b, c]) = u.pf {
        let _ = write!(o, ",\"pf\":[{a},{b},{c}]");
    }
    if let Some([a, b, c]) = u.sf {
        let _ = write!(o, ",\"sf\":[{a},{b},{c}]");
    }
    num(o, "rp", u.rp);
    num(o, "rs", u.rs);
    if let Some([lo, hi]) = u.ik {
        let _ = write!(o, ",\"ik\":[{lo},{hi}]");
    }
    num(o, "ss", u.ss);
    if let Some(list) = &u.is {
        o.push_str(",\"is\":[");
        for (i, [s, l, v]) in list.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            let _ = write!(o, "[{s},{l},{v}]");
        }
        o.push(']');
    }
    if let Some(q) = &u.q {
        o.push_str(",\"q\":[");
        for (i, [slot, word]) in q.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            let _ = write!(o, "[{slot},{word}]");
        }
        o.push(']');
    }
    o.push('}');
}

/// `q` of a quest flag record (§2): `[slot, word]` for slots 0..41 whose
/// little-endian word is non-zero (`world/quests.md` §1.1: bit b of slot
/// q is bit b of the word).
pub fn quest_words(flags: &QuestFlags) -> Vec<[u16; 2]> {
    (0..SLOTS)
        .map(|q| [u16::from(q), flags.word(q)])
        .filter(|w| w[1] != 0)
        .collect()
}

/// A JSON string literal of `s` (quotes, backslash and control
/// characters escaped).
pub fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", u32::from(c));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn string_array<S: AsRef<str>>(items: &[S]) -> String {
    let parts: Vec<String> = items.iter().map(|s| json_string(s.as_ref())).collect();
    format!("[{}]", parts.join(","))
}

/// The header of §1 rule 1. `save` and `seed` are the d2rs writer's
/// additions (written when present, after `gaps`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// `orig` or `d2rs`.
    pub side: String,
    /// `<name version>`.
    pub tool: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// The argv, joined.
    pub command: String,
    pub fields: Vec<String>,
    pub gaps: Vec<String>,
    pub save: Option<String>,
    pub seed: Option<u32>,
}

impl Header {
    /// The header line (no newline), keys in the order of §1 rule 1.
    pub fn to_json_line(&self) -> String {
        let mut o = format!(
            "{{\"k\":\"header\",\"format\":{},\"side\":{},\"tool\":{},\"date\":{},\"command\":{},\"fields\":{},\"gaps\":{}",
            json_string(FORMAT),
            json_string(&self.side),
            json_string(&self.tool),
            json_string(&self.date),
            json_string(&self.command),
            string_array(&self.fields),
            string_array(&self.gaps),
        );
        if let Some(s) = &self.save {
            let _ = write!(o, ",\"save\":{}", json_string(s));
        }
        if let Some(n) = self.seed {
            let _ = write!(o, ",\"seed\":{n}");
        }
        o.push('}');
        o
    }
}

/// The footer line of §1 rule 3 (no newline).
pub fn footer_line<S: AsRef<str>>(snaps: u64, notes: &[S]) -> String {
    format!(
        "{{\"k\":\"footer\",\"snaps\":{snaps},\"notes\":{}}}",
        string_array(notes)
    )
}

/// The header `fields` and `gaps` of a d2rs game: every §2 key but the
/// [`D2RS_GAPS`]; without the path provider ([`ActionHooks::paths`]
/// `None`) every unit reads as path 0, so [`PATH_FIELDS`] go to the gaps
/// as well.
pub fn coverage<X>(sys: &UnitSystem<ActionHooks<X>>) -> (Vec<String>, Vec<String>) {
    let no_paths = sys.hooks.paths.is_none();
    let mut fields = Vec::new();
    let mut gaps: Vec<String> = Vec::new();
    for k in FIELDS {
        if D2RS_GAPS.iter().any(|(g, _)| *g == k) {
            continue;
        }
        if no_paths && PATH_FIELDS.contains(&k) {
            continue;
        }
        fields.push(k.to_owned());
    }
    if no_paths {
        gaps.push(format!(
            "{}: the game has no path provider (ActionHooks::paths is None): no unit has a path record",
            PATH_FIELDS.join(", ")
        ));
    }
    gaps.extend(D2RS_GAPS.iter().map(|(_, why)| (*why).to_owned()));
    (fields, gaps)
}

/// The snapshot of a wired game (§2, §3): the game seed and every unit of
/// the server's unit lists, sorted by (`ut`, `g`). Reads only.
pub fn snapshot<X>(game: &Game, sys: &UnitSystem<ActionHooks<X>>) -> StateSnapshot {
    let mut units = Vec::new();
    for ty in UnitType::ALL {
        for id in game.lists.units_of_type(ty) {
            if let Some(e) = game.lists.unit(id) {
                units.push(unit_state(game, sys, id, ty, e.guid));
            }
        }
    }
    let seed = sys.hooks.game_seed;
    let mut s = StateSnapshot {
        frame: game.frame,
        seed: [seed.lo, seed.hi],
        units,
    };
    s.sort_units();
    s
}

/// [`snapshot`] of a [`WorldSim`] (the app's and the server's wired game).
pub fn snapshot_world<X>(game: &Game, sim: &WorldSim<X>) -> StateSnapshot {
    snapshot(game, &sim.action.sys)
}

/// The game's difficulty (game +0x6D) of a [`WorldSim`]: which of a
/// player's three quest records `q` reads (`world/quests.md` §1.4).
pub fn difficulty_world<X>(sim: &WorldSim<X>) -> u8 {
    sim.action.sys.data.difficulty
}

/// [`coverage`] of a [`WorldSim`].
pub fn coverage_world<X>(sim: &WorldSim<X>) -> (Vec<String>, Vec<String>) {
    coverage(&sim.action.sys)
}

fn unit_state<X>(
    game: &Game,
    sys: &UnitSystem<ActionHooks<X>>,
    id: UnitId,
    ty: UnitType,
    guid: u32,
) -> UnitState {
    let mut u = UnitState {
        ut: ty as u8,
        g: guid,
        ..UnitState::default()
    };
    if let Some(r) = sys.units.get(id) {
        u.cl = Some(r.class);
        u.m = Some(r.mode);
        u.fr = Some(r.anim.frame);
        u.fc = Some(r.anim.frame_count);
        u.sp = Some(r.anim.speed);
        u.s = Some([r.seed.lo, r.seed.hi]);
        u.act = Some(r.act);
    }
    let path = sys.hooks.paths.as_ref().and_then(|p| p.record(id));
    if let Some(path) = path {
        match path {
            UnitPath::Dynamic(p) => dynamic(&mut u, p),
            UnitPath::Static(p) => fixed(&mut u, p),
        }
        u.lv = path
            .room()
            .and_then(|room| sys.hooks.drlg.level_id(game, room));
    }
    u.own = owner(sys, id, ty);
    if let Some(l) = sys.stats.unit_list(id).filter(|&l| sys.stats.is_live(l)) {
        stats(&mut u, &sys.stats, l);
    }
    if ty == UnitType::Item {
        if let Some(it) = sys.hooks.items.get(id) {
            u.iq = Some(it.quality);
            u.ifl = Some(it.flags);
            u.fi = Some(it.file_index);
            u.il = Some(it.ilvl);
            u.aa = Some(it.auto_affix);
            u.pf = Some(it.prefix);
            u.sf = Some(it.suffix);
            u.rp = Some(it.rare_prefix);
            u.rs = Some(it.rare_suffix);
            u.ik = Some([it.item_seed.lo, it.item_seed.hi]);
            u.ss = Some(it.start_seed);
            let list = sys
                .stats
                .unit_list(id)
                .filter(|&l| sys.stats.is_live(l))
                .map(|l| {
                    sys.stats
                        .base_entries(l)
                        .into_iter()
                        .map(|(k, v)| [i32::from(key_stat(k)), i32::from(key_layer(k)), v])
                        .collect()
                })
                .unwrap_or_default();
            u.is = Some(list);
        }
    }
    u
}

/// `own` of a monster or a missile (§2): the AI control's minion owner
/// (`0x0058F0D0`; a released pack, GUID −1, is none) or the missile's
/// source-unit link (`0x00552FD0`). Types 0, 2, 5 have none; an item's
/// holder lives in the inventory model (overlaid by the host). 0 is
/// absent as on the 1.14d side.
fn owner<X>(sys: &UnitSystem<ActionHooks<X>>, id: UnitId, ty: UnitType) -> Option<u32> {
    let guid = match ty {
        UnitType::Monster => sys.hooks.ai.as_ref()?.control(id)?.minion_owner?.guid,
        UnitType::Missile => sys.hooks.missiles.as_ref()?.get(id)?.owner?.guid,
        // A monster's equipment: the monster has no inventory model, its
        // holdings are the host's (PROVISIONAL, REC-1030); the holder is
        // the monster the item is held for.
        UnitType::Item => {
            let (&holder, _) = sys
                .hooks
                .monster_equip
                .iter()
                .find(|(_, held)| held.values().any(|&i| i == id))?;
            sys.units.get(holder)?.guid
        }
        _ => return None,
    };
    (guid != 0 && guid != u32::MAX).then_some(guid)
}

/// Types 0, 1, 3 (§2): u16 sub-tile words, the fractions, the target and
/// the direction at +0x64.
fn dynamic(u: &mut UnitState, p: &DynamicPath) {
    u.x = Some(p.precise_x >> 16);
    u.y = Some(p.precise_y >> 16);
    u.xf = Some(p.precise_x as u16);
    u.yf = Some(p.precise_y as u16);
    u.tx = Some(p.target_x);
    u.ty = Some(p.target_y);
    u.d = Some(p.direction);
}

/// Types 2, 4, 5 (§2): u32 x, y and the direction at +0x1C.
fn fixed(u: &mut UnitState, p: &StaticPath) {
    u.x = Some(p.x as u32);
    u.y = Some(p.y as u32);
    u.d = Some(p.direction);
}

/// Stats 6–11 from the full array of an extended list, 0, 1, 2, 3, 12
/// from the base array (§2; raw values, no minimum rule: §2 rule 2). An
/// absent key reads 0; a plain list has no full array, so 6–11 stay
/// absent.
fn stats(u: &mut UnitState, lists: &StatLists, l: ListId) {
    let find = |entries: &[(i32, i32)], s: u16| {
        entries
            .binary_search_by_key(&key(s, 0), |e| e.0)
            .map_or(0, |i| entries[i].1)
    };
    let base = lists.base_entries(l);
    u.str = Some(find(&base, 0));
    u.ene = Some(find(&base, 1));
    u.dex = Some(find(&base, 2));
    u.vit = Some(find(&base, 3));
    u.lvl = Some(find(&base, 12));
    if lists.is_extended(l) {
        let full = lists.full_entries(l);
        u.hp = Some(find(&full, 6));
        u.hpx = Some(find(&full, 7));
        u.mp = Some(find(&full, 8));
        u.mpx = Some(find(&full, 9));
        u.st = Some(find(&full, 10));
        u.stx = Some(find(&full, 11));
    }
}

#[cfg(test)]
mod tests;
