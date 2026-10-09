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
//! `own` is left out (spec §2: no 1.14d source; [`D2RS_GAPS`]).

use std::fmt::Write as _;

use crate::game::Game;
use crate::path::record::{DynamicPath, StaticPath};
use crate::path::UnitPath;
use crate::stats::{key, ListId, StatLists};
use crate::units::dispatch::UnitSystem;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::ActionHooks;
use crate::wiring::worldgen::WorldSim;

/// The format name of line 1 (§1 rule 1).
pub const FORMAT: &str = "state-1";

/// Every unit key of §2, in table order (the comparison order).
pub const FIELDS: [&str; 29] = [
    "ut", "g", "cl", "m", "x", "y", "xf", "yf", "tx", "ty", "d", "fr", "fc", "sp", "s", "act",
    "lv", "hp", "hpx", "mp", "mpx", "st", "stx", "str", "ene", "dex", "vit", "lvl", "own",
];

/// The keys whose values need the path provider (path +0x00 … +0x64, and
/// the level of the path's room).
pub const PATH_FIELDS: [&str; 8] = ["x", "y", "xf", "yf", "tx", "ty", "d", "lv"];

/// The keys d2rs never fills, with why (header `gaps`, one line each).
pub const D2RS_GAPS: [(&str, &str); 1] = [(
    "own",
    "own: not written; the spec gives it no 1.14d source (state-snapshot.md §2, open \
     question 1) and d2rs keeps owners per system (missile store, pet lists, item store), \
     with no single unit-owner field to map",
)];

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
    o.push('}');
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
    if let Some(l) = sys.stats.unit_list(id).filter(|&l| sys.stats.is_live(l)) {
        stats(&mut u, &sys.stats, l);
    }
    u
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
