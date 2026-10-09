// Spec: specs/tools/soak.md (§5)
//! Save/load round trip: what a reload must keep. The comparison is of
//! the live state the server reads at save time ([`Live`]) before the
//! save and after the reload, and of the `.d2s` the reloaded game saves
//! against the first file, field by field, so each difference has its
//! own signature.

use std::fmt::Debug;

use d2_formats::d2s::{D2s, ItemEntry};

use super::super::save::Live;
use super::checks::Violation;

fn diff<T: Debug + PartialEq>(out: &mut Vec<Violation>, what: &str, a: &T, b: &T) {
    if a != b {
        let (a, b) = (format!("{a:?}"), format!("{b:?}"));
        let cut = |s: &str| -> String {
            if s.len() > 600 {
                format!("{}…", &s[..s.floor_char_boundary(600)])
            } else {
                s.to_owned()
            }
        };
        out.push(Violation {
            kind: "roundtrip",
            sig: format!("roundtrip:{what}"),
            detail: format!("before {} / after {}", cut(&a), cut(&b)),
        });
    }
}

/// An item list compared as a multiset of item bytes in list order, and
/// item by item, so a lost, added or changed item names itself.
fn diff_items(out: &mut Vec<Violation>, what: &str, a: &[ItemEntry], b: &[ItemEntry]) {
    if a == b {
        return;
    }
    if a.len() != b.len() {
        diff(out, &format!("{what}.count"), &a.len(), &b.len());
    }
    let mut sa: Vec<&ItemEntry> = a.iter().collect();
    let mut sb: Vec<&ItemEntry> = b.iter().collect();
    sa.sort_by(|x, y| x.bytes.cmp(&y.bytes));
    sb.sort_by(|x, y| x.bytes.cmp(&y.bytes));
    if sa == sb {
        diff(out, &format!("{what}.order"), &a, &b);
        return;
    }
    if let Some((x, y)) = sa.iter().zip(&sb).find(|(x, y)| x != y) {
        diff(out, &format!("{what}.item"), x, y);
    }
}

/// `live` as a load leaves it: every item's 0x2000 (instore) cleared
/// (`d2s.md` §8.2 rule 7: "a file's 0x2000 never survives a load"); the
/// flags are the 32 bits after `JM`, 0x2000 is bit 5 of byte 3.
pub fn loaded_live(mut live: Live) -> Live {
    let clear = |items: &mut Vec<ItemEntry>| {
        for e in items {
            if e.bytes.len() > 3 {
                e.bytes[3] &= !0x20;
            }
        }
    };
    if let Some(items) = &mut live.extra.items {
        clear(items);
    }
    if let Some(items) = &mut live.gaps.hireling_items {
        clear(items);
    }
    for c in live.extra.corpses.iter_mut().flatten() {
        clear(&mut c.items);
    }
    live
}

/// The differences between the live state before the save and after the
/// reload (the load's own rules applied to `before`: [`loaded_live`];
/// stamina is full after a load, `d2s-load.md` §9 r4, so stats 10 and 11
/// are compared as the maximum).
pub fn diff_live(before: &Live, after: &Live) -> Vec<Violation> {
    let before = loaded_live(before.clone());
    let mut out = Vec::new();
    let stats = |l: &Live| -> Vec<(u16, i32)> {
        let max_st = l.stats.iter().find(|s| s.0 == 11).map(|s| s.1);
        l.stats
            .iter()
            .map(|&(id, v)| match (id, max_st) {
                (10, Some(m)) => (id, m),
                _ => (id, v),
            })
            .collect()
    };
    let (sa, sb) = (stats(&before), stats(after));
    for ((ia, va), (ib, vb)) in sa.iter().zip(&sb) {
        if ia != ib || va != vb {
            diff(&mut out, &format!("live.stat{ia}"), &(ia, va), &(ib, vb));
        }
    }
    if sa.len() != sb.len() {
        diff(&mut out, "live.stats.count", &sa.len(), &sb.len());
    }
    diff(&mut out, "live.quests", &before.quests, &after.quests);
    diff(
        &mut out,
        "live.skills",
        &before.extra.skills,
        &after.extra.skills,
    );
    diff(
        &mut out,
        "live.waypoints",
        &before.extra.waypoints,
        &after.extra.waypoints,
    );
    match (&before.extra.items, &after.extra.items) {
        (Some(a), Some(b)) => diff_items(&mut out, "live.items", a, b),
        (a, b) => diff(&mut out, "live.items.present", &a.is_some(), &b.is_some()),
    }
    diff(
        &mut out,
        "live.corpses",
        &before.extra.corpses,
        &after.extra.corpses,
    );
    diff(
        &mut out,
        "live.mouse",
        &before.gaps.mouse,
        &after.gaps.mouse,
    );
    diff(&mut out, "live.town", &before.gaps.town, &after.gaps.town);
    match (&before.gaps.hireling_items, &after.gaps.hireling_items) {
        (Some(a), Some(b)) => diff_items(&mut out, "live.hireling_items", a, b),
        (a, b) => diff(
            &mut out,
            "live.hireling_items.present",
            &a.is_some(),
            &b.is_some(),
        ),
    }
    diff(
        &mut out,
        "live.golem",
        &before.gaps.golem,
        &after.gaps.golem,
    );
    diff(&mut out, "live.swap", &before.gaps.swap, &after.gaps.swap);
    diff(
        &mut out,
        "live.hireling",
        &before.gaps.hireling,
        &after.gaps.hireling,
    );
    diff(
        &mut out,
        "live.status",
        &before.gaps.status,
        &after.gaps.status,
    );
    diff(
        &mut out,
        "live.hotkeys",
        &before.gaps.hotkeys,
        &after.gaps.hotkeys,
    );
    diff(&mut out, "live.hardcore", &before.hardcore, &after.hardcore);
    diff(&mut out, "live.map_seed", &before.map_seed, &after.map_seed);
    diff(&mut out, "live.npcs", &before.npcs, &after.npcs);
    out
}

/// The save without its time stamp and the checksum over it (the second
/// save is a later one), with the load-cleared 0x2000 item flag, and with
/// stamina at its maximum (a load fills it, `d2s-load.md` §9 r4).
pub fn timeless(mut s: D2s) -> D2s {
    s.header.save_time = 0;
    s.header.checksum = 0;
    if let Some(d2_formats::d2s::Stats::Bits(v)) = s.body.as_mut().map(|b| &mut b.stats) {
        let max = v.iter().find(|e| e.id == 11 && e.layer == 0).map(|e| e.value);
        if let (Some(m), Some(st)) = (max, v.iter_mut().find(|e| e.id == 10 && e.layer == 0)) {
            st.value = m;
        }
    }
    if let Some(b) = &mut s.body {
        for e in &mut b.items {
            if e.bytes.len() > 3 {
                e.bytes[3] &= !0x20;
            }
        }
    }
    s
}

/// The differences between two saves, field by field (after [`timeless`]).
pub fn diff_saves(first: &D2s, second: &D2s) -> Vec<Violation> {
    let (a, b) = (timeless(first.clone()), timeless(second.clone()));
    let mut out = Vec::new();
    if a == b {
        return out;
    }
    let (ha, hb) = (&a.header, &b.header);
    macro_rules! h {
        ($($f:ident),*) => { $( diff(&mut out, concat!("d2s.header.", stringify!($f)), &ha.$f, &hb.$f); )* };
    }
    h!(
        version,
        file_size,
        weapon_switch,
        name,
        status,
        unk26,
        class,
        stat_count,
        skill_count,
        level,
        create_time,
        unk34,
        hotkeys,
        mouse,
        components,
        colours,
        towns,
        map_seed,
        hireling,
        client_cf
    );
    diff(
        &mut out,
        "d2s.header.tail",
        &ha.tail.to_vec(),
        &hb.tail.to_vec(),
    );
    match (&a.body, &b.body) {
        (Some(x), Some(y)) => {
            macro_rules! b {
                ($($f:ident),*) => { $( diff(&mut out, concat!("d2s.body.", stringify!($f)), &x.$f, &y.$f); )* };
            }
            b!(quests, waypoints, npcs, stats, skills, corpses, golem, trailing);
            diff_items(&mut out, "d2s.body.items", &x.items, &y.items);
            match (&x.hireling_items, &y.hireling_items) {
                (Some(Some(p)), Some(Some(q))) => {
                    diff_items(&mut out, "d2s.body.hireling_items", p, q)
                }
                (p, q) => diff(&mut out, "d2s.body.hireling_items.present", p, q),
            }
        }
        (x, y) => diff(&mut out, "d2s.body.present", &x.is_some(), &y.is_some()),
    }
    if out.is_empty() {
        diff(&mut out, "d2s.other", &a, &b);
    }
    out
}

/// Reads a save the way `play --save` does (the game's context).
pub fn read_file(
    live: &super::super::single_player::LiveData,
    path: &std::path::Path,
    difficulty: u8,
) -> Result<D2s, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    live.read_save(&bytes, difficulty)
        .map_err(|e| format!("{}: {e:?}", path.display()))
}
