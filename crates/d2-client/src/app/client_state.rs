// Spec: specs/tools/state-snapshot.md (§3 rule 5: the client's own units)
//! `state-dump --client-out FILE`: the d2rs side of the 1.14d client unit
//! recording (`record_state.py --client-out`). One `snap` per server tick
//! that ran, holding the client model's units in format `state-1` with
//! header `side` = `d2rs-client`: set S ([`ClientWorld::units`]) and set C
//! (`objclient.set_c`), each unit tagged `"set":"S"` / `"set":"C"`, sorted
//! by (`set`, `ut`, `g`). Reads the model only; not game logic.
//!
//! Keys the client model does not hold are left out of the line and named
//! in the header `gaps` (`RUN_GAPS`), so a comparison reports them as gaps
//! and not as a match.

use std::fmt::Write as _;

use crate::bridge::world::{room_of_point, ClientUnit, ClientWorld, UnitKey};
use d2_sim::debug::state::{json_string, Header};

/// Keys never written by this dump, with the reason (header `gaps`).
pub const GAPS: [&str; 6] = [
    "tx: the client model holds no path record (client/model.md §1)",
    "act: the client unit's act is not kept per unit",
    "own: not read from the client unit",
    "q: the client model holds no quest record",
    "item keys (iq if fi il aa pf sf rp rs ik ss is): not dumped from the client item data yet",
    "ty: the client model holds no path record (client/model.md §1)",
];

/// The `fields` of the header.
pub const FIELDS: [&str; 22] = [
    "ut", "g", "cl", "m", "x", "y", "xf", "yf", "d", "fr", "fc", "sp", "s", "lv", "hp", "hpx",
    "mp", "mpx", "st", "stx", "str", "ene",
];

const DYNAMIC: [u8; 3] = [0, 1, 3];

/// The header line of the dump.
pub fn header(tool: &str, date: &str, command: &str) -> String {
    let h = Header {
        side: "d2rs-client".into(),
        tool: tool.into(),
        date: date.into(),
        command: command.into(),
        fields: FIELDS
            .iter()
            .chain(["dex", "vit", "lvl"].iter())
            .map(|k| (*k).to_owned())
            .collect(),
        gaps: GAPS.iter().map(|g| (*g).to_owned()).collect(),
        save: None,
        seed: None,
    };
    h.to_json_line()
}

/// One `snap` line for server frame `frame` (no newline); `units` is
/// [`units_json`] of the model as the tick end left it: the client has not
/// yet read that tick's messages (state-snapshot.md §3 rule 5).
pub fn snap_line(units: &str, frame: i32) -> String {
    format!("{{\"k\":\"snap\",\"f\":{frame},\"units\":{units}}}")
}

/// The `units` array of a `snap` line.
pub fn units_json(w: &ClientWorld) -> String {
    let mut rows: Vec<(char, &ClientUnit)> = w.units.values().map(|u| ('S', u)).collect();
    rows.extend(w.objclient.set_c.values().map(|u| ('C', u)));
    rows.sort_by_key(|(s, u)| (*s, u.key.unit_type, u.key.guid));
    let mut o = String::from("[");
    for (i, (set, u)) in rows.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        unit_json(&mut o, w, *set, u);
    }
    o.push(']');
    o
}

/// `d`: the path direction of a monster or player; the zeroed static path
/// of an object or item (`world/objects.md` §4 rule 5: allocated zeroed, no
/// model rule writes +0x1C).
fn direction(w: &ClientWorld, set: char, u: &ClientUnit) -> Option<u8> {
    match (set, u.key.unit_type) {
        ('C', 1) => crate::bridge::critter_path::direction(w, u.key),
        // A path record the model does not write is the zeroed allocation
        // (`path_dir` is set by the creation draw only, `monster_anim`).
        ('S', 0 | 1) => Some(u.path_dir.unwrap_or(0)),
        (_, 2 | 4 | 5) => Some(0),
        _ => None,
    }
}

fn unit_json(o: &mut String, w: &ClientWorld, set: char, u: &ClientUnit) {
    let UnitKey { unit_type, guid } = u.key;
    let (x, y) = u.position.unwrap_or((0, 0));
    let _ = write!(
        o,
        "{{\"ut\":{unit_type},\"g\":{guid},\"cl\":{},\"m\":{},\"x\":{x},\"y\":{y}",
        u.class, u.mode
    );
    if DYNAMIC.contains(&unit_type) {
        let (xf, yf) = u
            .precise
            .map_or((0x8000, 0x8000), |(px, py)| (px & 0xFFFF, py & 0xFFFF));
        let _ = write!(o, ",\"xf\":{xf},\"yf\":{yf}");
    }
    if let Some(d) = direction(w, set, u) {
        let _ = write!(o, ",\"d\":{d}");
    }
    let _ = write!(o, ",\"fr\":{},\"fc\":{}", u.frame, u.frame_count);
    // +0x4C is zero until an animation set-up writes it.
    let _ = write!(o, ",\"sp\":{}", u.speed.unwrap_or(0));
    if let Some((lo, hi)) = u.seed {
        let _ = write!(o, ",\"s\":[{lo},{hi}]");
    }
    if let Some(r) = w
        .active_rooms
        .as_deref()
        .and_then(|rooms| room_of_point(rooms, i32::from(x), i32::from(y)))
    {
        let _ = write!(o, ",\"lv\":{}", r.level);
    }
    if matches!(unit_type, 0 | 1) {
        for (k, id) in [
            ("hp", 6u16),
            ("hpx", 7),
            ("mp", 8),
            ("mpx", 9),
            ("st", 10),
            ("stx", 11),
        ] {
            let _ = write!(o, ",\"{k}\":{}", u.stats.get(&id).copied().unwrap_or(0));
        }
        for (k, id) in [
            ("str", 0u16),
            ("ene", 1),
            ("dex", 2),
            ("vit", 3),
            ("lvl", 12),
        ] {
            let _ = write!(o, ",\"{k}\":{}", u.stats.get(&id).copied().unwrap_or(0));
        }
    }
    let _ = write!(o, ",\"set\":{}}}", json_string(&set.to_string()));
}
