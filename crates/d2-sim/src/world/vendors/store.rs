// Spec: specs/world/vendors.md §2–§4, §6
//! Store generation (`0x00576980`), one store item (`0x00576330`), trade
//! and gamble open (`0x00579430`) and the refresh rules.

use super::trade::repair_item;
use super::{
    flag, gamble, stat, store_level, unit_flag, EventNode, VendorRecord, VendorTables, VendorWorld,
    AQV, CQV, CRACKED, FAIL_LIMIT, HIRE_CLASSES, NO_STORE_REFRESH, REFRESH_MS, TOWNS, XXX,
};
use crate::items::q;
use crate::rng::Seed;
use crate::units::UnitId;

/// Quality draw `0x00576900` (§3 step 4): one step, r = lo' mod 100.
pub fn quality_draw(seed: &mut Seed, ilvl: i32) -> u8 {
    let r = seed.step() % 100;
    match ilvl {
        i if i < 5 => {
            if r > 90 {
                q::LOW
            } else {
                q::NORMAL
            }
        }
        i if i < 10 => {
            if r > 85 {
                q::SUPERIOR
            } else {
                q::NORMAL
            }
        }
        _ => {
            if r > 74 {
                q::SUPERIOR
            } else {
                q::NORMAL
            }
        }
    }
}

/// range(min, max) `0x004BC500`: `min` with no draw if max ≤ min
/// (signed), else roll(max − min) + min.
pub fn range(seed: &mut Seed, min: i32, max: i32) -> i32 {
    seed.roll_range(min, max.wrapping_sub(min))
}

/// Whether a created item is an inferior `Cracked` (§3.1 rule 2).
pub fn is_cracked<W: VendorWorld>(t: &VendorTables, w: &W, item: UnitId) -> bool {
    w.item_quality(item) == q::LOW
        && usize::try_from(w.item_file_index(item))
            .ok()
            .and_then(|i| t.lowquality.get(i))
            .is_some_and(|n| n.as_slice() == CRACKED)
}

/// The NPC side of one store generation.
pub struct StoreCtx<'a> {
    pub tables: &'a VendorTables,
    /// The NPC-control seed (`npc.md` §1.1).
    pub seed: &'a mut Seed,
}

/// Upgrade code choice (§3.1 rule 1): one roll(100000) in Nightmare or
/// Hell when L_p > 25.
fn upgrade<W: VendorWorld>(
    t: &VendorTables,
    seed: &mut Seed,
    w: &W,
    record: usize,
    ilvl: i32,
    player_level: i32,
) -> usize {
    let d = w.difficulty();
    let Some(rec) = t.item(record) else {
        return record;
    };
    if d == 0 || player_level <= 25 {
        return record;
    }
    let r = seed.roll(100_000) as i32;
    let mut code = record;
    if d == 1 {
        if let (true, Some(u)) = (r < ilvl * 64 + 4000, t.valid_code(rec.ubercode)) {
            code = u;
        } else if rec.nightmare_upgrade != XXX {
            // TODO(specs/world/vendors.md §3.1 rule 1): an upgrade code not
            // in the code map is not described; the base code is kept.
            if let Some(i) = t.find_code(rec.nightmare_upgrade) {
                code = i;
            }
        }
    } else {
        if let (true, true, Some(x)) = (
            w.expansion(),
            r < ilvl * 16 + 1000,
            t.valid_code(rec.ultracode),
        ) {
            code = x;
        } else if let (true, Some(u)) = (r < ilvl * 128 + 5000, t.valid_code(rec.ubercode)) {
            code = u;
        }
        if rec.hell_upgrade != XXX {
            if let Some(i) = t.find_code(rec.hell_upgrade) {
                code = i;
            }
        }
    }
    code
}

/// Mark a store item (`0x005762C0`, §3.1 rule 5).
pub fn mark<W: VendorWorld>(w: &mut W, class: u16, item: UnitId) {
    let mut f = w.item_flags(item) | flag::IDENTIFIED;
    if w.has_filled_sockets(item) {
        f |= flag::NEW;
    }
    w.set_item_flags(item, f);
    w.or_unit_flags(item, unit_flag::VENDOR);
    w.add_trade_inventory(class, item);
}

/// Places an item in the store on its store page; page 1 retries on page
/// 2 (§3.1 rule 4). Returns false (item not destroyed) when there is no
/// room or no store page; `None` store page → the item is destroyed.
pub(crate) fn place_store_page<W: VendorWorld>(
    t: &VendorTables,
    w: &mut W,
    class: u16,
    item: UnitId,
) -> Option<bool> {
    let page = t.type_of(w.item_record(item)).map_or(0xFF, |y| y.storepage);
    if page == 0xFF {
        w.destroy_item(item);
        return None;
    }
    w.set_item_page(item, page);
    if w.place_in_store(class, item) {
        return Some(true);
    }
    if page == 1 {
        w.set_item_page(item, 2);
        if w.place_in_store(class, item) {
            return Some(true);
        }
    }
    Some(false)
}

/// One store item `0x00576330` (§3.1). Returns the placed item, or null.
pub fn make_store_item<W: VendorWorld>(
    c: &mut StoreCtx,
    rec: &mut VendorRecord,
    w: &mut W,
    record: usize,
    quality: u8,
    ilvl: i32,
    player_level: i32,
) -> Option<UnitId> {
    let t = c.tables;
    let chosen = upgrade(t, c.seed, w, record, ilvl, player_level);
    let mut q = quality;
    let mut made = None;
    'rounds: for round in 0..2 {
        let mut item = None;
        for _ in 0..5 {
            // TODO(specs/world/vendors.md §3.1 rule 2): a null creation is
            // read as a failed try (as a cracked one).
            match w.create_item(rec.class, chosen, q, ilvl) {
                Some(i) if is_cracked(t, w, i) => w.destroy_item(i),
                Some(i) => {
                    item = Some(i);
                    break;
                }
                None => {}
            }
        }
        let i = item?;
        if round == 0 && w.item_record(i) != chosen {
            w.destroy_item(i);
            q = q::NORMAL;
            continue 'rounds;
        }
        made = Some(i);
        break;
    }
    let item = made?;
    // Rule 3–4.
    let page = t.type_of(w.item_record(item)).map_or(0xFF, |y| y.storepage);
    if page == 0xFF {
        w.destroy_item(item);
        return None;
    }
    w.set_item_page(item, page);
    repair_item(t, w, item, None);
    let mut placed = w.place_in_store(rec.class, item);
    if !placed && page == 1 {
        w.set_item_page(item, 2);
        placed = w.place_in_store(rec.class, item);
    }
    if !placed {
        rec.events.push(EventNode {
            unit: item,
            arg: 0,
            kind: 0,
            deferred: true,
        });
        return None;
    }
    // Rule 5.
    mark(w, rec.class, item);
    rec.store.push(item);
    Some(item)
}

/// Store generation `0x00576980` (§3). `now` is the host's
/// `GetTickCount` (edge case 10).
pub fn generate<W: VendorWorld>(
    c: &mut StoreCtx,
    rec: &mut VendorRecord,
    w: &mut W,
    player: UnitId,
    now: u32,
) {
    let t = c.tables;
    rec.store_time = now;
    let lp = w.stat(player, stat::LEVEL, 0);
    let ilvl = store_level(lp, w.difficulty(), rec.act);
    let mut fails = 0u32;
    let entries = rec.items.clone();
    for e in &entries {
        // TODO(specs/world/vendors.md §3 step 1): a list code missing from
        // the code map is skipped without a draw (lists are built from the
        // records, so it does not occur with consistent tables).
        let Some(record) = t.find_code(e.code) else {
            continue;
        };
        let r = &t.items[record];
        // Step 1.
        if i32::from(r.level) > ilvl {
            continue;
        }
        // Step 2.
        let n_norm = if ilvl < 25 {
            range(c.seed, i32::from(e.min), i32::from(e.max) + 1)
        } else {
            0
        };
        // Step 3.
        if r.version >= 100 && w.item_format() < 100 {
            continue;
        }
        // Step 4.
        for _ in 0..n_norm {
            let q = quality_draw(c.seed, ilvl);
            if make_store_item(c, rec, w, record, q, ilvl, lp).is_none() {
                fails += 1;
                if fails > FAIL_LIMIT {
                    return;
                }
            }
        }
        // Step 5.
        if r.bitfield1 & 1 != 0 && i32::from(e.magic_lvl) <= ilvl {
            let k = if ilvl >= 25 {
                range(c.seed, 1, 3) + 1
            } else {
                1
            };
            let n_mag = range(c.seed, i32::from(e.magic_min), i32::from(e.magic_max) + k);
            for _ in 0..n_mag {
                if make_store_item(c, rec, w, record, q::MAGIC, ilvl, lp).is_none() {
                    fails += 1;
                }
            }
        }
    }
    let perm = rec.perm.clone();
    for code in perm {
        // TODO(specs/world/vendors.md §3): as above, a permanent code missing
        // from the code map is skipped.
        if let Some(record) = t.find_code(code) {
            match make_store_item(c, rec, w, record, q::NORMAL, ilvl, lp) {
                None => fails += 1,
                Some(item) => {
                    if code == CQV || code == AQV {
                        if let Some(it) = w.price_item(item) {
                            let m = super::price::max_stack(t, &it);
                            w.set_stat(item, stat::QUANTITY, 0, m);
                        }
                    }
                }
            }
        }
        if fails > FAIL_LIMIT {
            return;
        }
    }
}

/// Clearing a record's data `0x00536580` (§6 rule 4).
pub fn clear_record<W: VendorWorld>(rec: &mut VendorRecord, w: &mut W) {
    for g in std::mem::take(&mut rec.gamble_lists) {
        for item in g.items {
            w.remove_gamble_item(rec.class, g.player, item);
        }
    }
    for item in std::mem::take(&mut rec.store) {
        w.remove_store_item(rec.class, item);
    }
    for e in std::mem::take(&mut rec.events) {
        if e.deferred {
            w.destroy_item(e.unit);
        }
    }
}

/// Adds every store or gamble-list item to the NPC's trade inventory
/// (§4 rule 3, `0x00576C30`).
fn show_items<W: VendorWorld>(rec: &VendorRecord, w: &mut W, player: u32, gamble: bool) {
    let items: Vec<UnitId> = if gamble {
        rec.gamble_list(player)
            .map(|g| g.items.clone())
            .unwrap_or_default()
    } else {
        rec.store.clone()
    };
    for item in items {
        w.or_unit_flags(item, unit_flag::VENDOR);
        if w.has_filled_sockets(item) {
            let f = w.item_flags(item);
            w.set_item_flags(item, f | flag::NEW);
        }
        w.add_trade_inventory(rec.class, item);
    }
}

/// Trade or gamble open `0x00579430(npc, single, gamble)` (§4).
#[allow(clippy::too_many_arguments)]
pub fn open<W: VendorWorld>(
    c: &mut StoreCtx,
    rec: &mut VendorRecord,
    w: &mut W,
    npc: UnitId,
    player: UnitId,
    single: bool,
    is_gamble: bool,
    now: u32,
) {
    let pg = w.guid(player);
    // Rule 1.
    rec.has_traded = true;
    rec.last_npc = w.guid(npc);
    // Rule 2 (`0x00578B30`).
    if !is_gamble {
        if rec.trader {
            rec.chain_node_mut(pg).gamble_mode = false;
            if !rec.store_generated {
                generate(c, rec, w, player, now);
                rec.store_generated = true;
            }
            if !w.hire_list_made(rec.class) {
                if HIRE_CLASSES.contains(&rec.class) {
                    w.make_hire_list(rec.class, c.seed);
                }
                w.set_hire_list_made(rec.class);
            }
        }
    } else if rec.has_gamble {
        rec.chain_node_mut(pg).gamble_mode = true;
        if rec.gamble_list(pg).is_none() {
            gamble::make_list(c, rec, w, player);
        }
    }
    // Rule 3.
    if NO_STORE_REFRESH.contains(&rec.class) {
        return;
    }
    if !is_gamble && single && rec.refresh_pending {
        rec.refresh_pending = false;
        clear_record(rec, w);
        w.new_store_inventory(rec.class, None);
        generate(c, rec, w, player, now);
        rec.store_generated = true;
    }
    w.refresh_npc_inventory(npc);
    show_items(rec, w, pg, is_gamble);
}

/// Refresh rule 3 `0x00537230(act, empty)` over the game's records.
pub fn refresh_act<W: VendorWorld>(
    records: &mut [VendorRecord],
    w: &mut W,
    act: u8,
    empty: bool,
    now: u32,
) {
    for rec in records.iter_mut() {
        if rec.act != act || !rec.trader || !rec.has_traded || !rec.store_generated {
            continue;
        }
        if !empty {
            if rec.store_time.wrapping_add(REFRESH_MS) < now {
                rec.refresh_pending = true;
                rec.store_time = now;
            }
            continue;
        }
        let npc = w
            .npc_by_guid(rec.last_npc)
            .filter(|&n| w.npc_class(n) == rec.class);
        let reset = match npc {
            None => Some(None),
            Some(n) if w.interaction_empty(n) => Some(Some(n)),
            Some(_) => None,
        };
        match reset {
            Some(assign) => {
                clear_record(rec, w);
                rec.store_generated = false;
                rec.store_time = 0;
                w.new_store_inventory(rec.class, assign);
            }
            None => rec.refresh_pending = true,
        }
    }
}

/// The act of a town level (§6 rule 1).
pub fn town_act(level: u16) -> Option<u8> {
    TOWNS.iter().position(|&l| l == level).map(|a| a as u8)
}

/// Leaving a level `0x00537340(from, to)` (§6 rule 1).
pub fn level_changed<W: VendorWorld>(
    records: &mut [VendorRecord],
    w: &mut W,
    player: UnitId,
    from: u16,
    to: u16,
    now: u32,
) {
    if let Some(act) = town_act(from) {
        let empty = w.players_in_level(from) == 0;
        refresh_act(records, w, act, empty, now);
    }
    if matches!(to, 1 | 40 | 75 | 109) {
        w.town_entered(player, to);
    }
}

/// Client leaving the game `0x00537580` (§6 rule 2).
pub fn client_left<W: VendorWorld>(
    records: &mut [VendorRecord],
    w: &mut W,
    player: UnitId,
    now: u32,
) {
    if w.game_type() == 3 {
        return;
    }
    let level = w.player_level_id(player);
    if !matches!(level, 1 | 40 | 75 | 103) {
        return;
    }
    let Some(act) = town_act(level) else { return };
    let empty = w.players_in_level(level).wrapping_sub(1) == 0;
    refresh_act(records, w, act, empty, now);
}
