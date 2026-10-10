// Spec: specs/items/inventory-moves.md
// Spec: specs/items/inventory.md (the sections other than §6–§11)
//! Pickup from the ground (§8), drop to the ground (§9) and gold (§10).

use super::deferred::{owner_refresh, send_item_page};
use super::seams::{MoveWorld, Spot};
use super::{
    add_cmd, add_iflags, add_uflags, changed_if_filled, clear_iflags, clear_uflags, cmd, exists,
    iflag, mode, page, sound, stat, ty, uflag, Guid, MoveFatal, Outcome, Owner, CUBE_CODE,
    DROP_MASK, DROP_MASK2, GOLD_PER_LEVEL, PILE_CAP,
};
use crate::items::inventory::pair_location;

/// "Leave the room": room delete notice, collision freed, room list
/// (§2.2; idempotent on the provider's side).
pub fn leave_room<W: MoveWorld>(w: &mut W, item: Guid) {
    // Announced once: an item whose grid / belt placement already left
    // the room is not announced again (REC-1403).
    if w.in_room(item) {
        w.room_delete_notice(item);
        w.free_collision(item);
    }
    w.remove_from_room(item);
}

// ------------------------------------------------------------------ §8

/// Auto pickup `0x00563560` (§8.1, cursor flag 0).
pub fn pickup_auto<W: MoveWorld>(
    w: &mut W,
    player: Owner,
    item: Guid,
) -> Result<Outcome, MoveFatal> {
    if w.cursor(player).is_some() || w.busy(player) {
        return Ok(Outcome::NOTHING);
    }
    if !exists(w, item) || w.mode(item) != mode::GROUND {
        return Ok(Outcome::REFUSED);
    }
    w.targeting_reset(player);
    if !can_pick(w, player, item) {
        refused_pickup(w, player, item, sound::REFUSED_PICKUP);
        return Ok(Outcome::NOTHING);
    }
    w.pickup_sound(player, item);
    if w.is_type(item, ty::GOLD) {
        gold_pickup(w, player, item);
        return Ok(Outcome::DONE);
    }
    if pickup_special(w, player, item)? {
        return Ok(Outcome::DONE);
    }
    if let Some(l) = w.auto_equip(player, item, false) {
        if w.equip_check(player, l, Some(item), false) != 1 {
            return Ok(Outcome::REFUSED);
        }
        leave_room(w, item);
        // §8.1 rule 5: a failed `0x00562E00` (unreachable) gives result 0,
        // out 0, the item out of its room in mode 3 and nothing sent.
        let ok = w.equip_picked(player, item);
        if ok {
            w.quest_item_picked(player, item);
        }
        return Ok(Outcome { ok, out: false });
    }
    if w.beltable(item) && w.auto_belt_gate(player, item) {
        if let Some(slot) = w.belt_free_slot(player, item) {
            if w.belt_place(player, item, u32::from(slot)) {
                leave_room(w, item);
                if !w.link_check(player, item, 2) {
                    return Err(MoveFatal::Link);
                }
                w.set_cursor(player, None);
                w.charm_relink(player, item);
                let active = w.is_active(player, item);
                if active {
                    w.stat_refresh(player);
                }
                clear_uflags(w, item, uflag::TARGETABLE);
                w.set_mode(item, mode::BELT);
                w.set_page(item, page::NONE);
                add_cmd(w, item, cmd::PICKED_TO_BELT);
                clear_uflags(w, item, uflag::ON_GROUND);
                w.update_list_add(player, item);
                owner_refresh(w, player);
                if active {
                    w.inventory_pass(player);
                }
                return Ok(Outcome::DONE);
            }
        }
    }
    if !grid_put(w, player, item, true)? {
        refused_pickup(w, player, item, sound::NO_ROOM);
        return Ok(Outcome::NOTHING);
    }
    Ok(Outcome::DONE)
}

/// Page-0 free position `0x005600A0(game, leave_room, page 0)` (§8.1 step
/// 7, §12.2 "grid put"): §2.3 + §2.2, then (with `leave`) the room step,
/// link kind 1 (failure fatal), cursor := none, item-skill link, stat
/// refresh, unit flag 0x2 cleared, mode 0, command flag 0x80, update
/// list, refresh, unit flag 0x2000000 cleared, page := 0, quest hook
/// ITEMPICKEDUP, inventory pass if active. False: no position.
pub fn grid_put<W: MoveWorld>(
    w: &mut W,
    player: Owner,
    item: Guid,
    leave: bool,
) -> Result<bool, MoveFatal> {
    let placed = match w.find_free(player, item, page::INVENTORY) {
        Some((x, y)) => w.place_at(player, item, page::INVENTORY, x, y),
        None => false,
    };
    if !placed {
        return Ok(false);
    }
    if leave {
        leave_room(w, item);
    }
    // The link (kind 1) failing in `0x005600A0` is a fatal assert (§8.1 r7).
    if !w.link_check(player, item, 1) {
        return Err(MoveFatal::Link);
    }
    w.set_cursor(player, None);
    w.charm_relink(player, item);
    w.stat_refresh(player);
    clear_uflags(w, item, uflag::TARGETABLE);
    w.set_mode(item, mode::STORED);
    add_cmd(w, item, cmd::PICKED_TO_PAGE);
    w.update_list_add(player, item);
    owner_refresh(w, player);
    clear_uflags(w, item, uflag::ON_GROUND);
    w.set_page(item, page::INVENTORY);
    w.quest_item_picked(player, item);
    if w.is_active(player, item) {
        w.inventory_pass(player);
    }
    Ok(true)
}

// ------------------------------------------------------------------ §12

/// Sound events of the corpse pickup (§12.1 steps 4–5).
pub const SOUND_CORPSE_LOOT: u32 = 93;
pub const SOUND_CANT_CARRY: u32 = 23;

/// The rest of `0x0057FB70` after its steps 1–2 passed (§12.1 steps
/// 3–5): the take-back §12.2, then the corpse's removal and sound 93, or
/// sound 23.
pub fn corpse_pickup_rest<W: MoveWorld>(
    w: &mut W,
    player: Owner,
    corpse: Owner,
) -> Result<(), MoveFatal> {
    if corpse_take_back(w, player, corpse)? {
        w.corpse_taken(player, corpse);
        w.sound(player, SOUND_CORPSE_LOOT);
    } else {
        w.sound(player, SOUND_CANT_CARRY);
    }
    Ok(())
}

/// S→C 0x0A for an item that left the player's inventory for the corpse,
/// so the client drops it (the take-back sends it again, §12.2).
/// d2rs-own, unverified (PROVISIONAL REC-141): no spec states the
/// client's view of the corpse creation's item moves.
fn gone_from<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) {
    w.send(
        p,
        crate::units::messages::remove_unit(Owner::ITEM, item).to_vec(),
    );
}

/// The item part of the corpse creation `0x0057F700` (`combat/vitals.md`
/// §4.7 rule 1.7): the cursor item goes into the corpse C's grid, then
/// each body item (locations 0–12) onto the same location of C; grid and
/// belt items stay on the player P. An item C cannot take is left on P
/// (the original drops it near P: the free-spot drop is the rest's,
/// PROVISIONAL REC-141). True when every item moved.
pub fn corpse_fill<W: MoveWorld>(w: &mut W, p: Owner, c: Owner) -> Result<bool, MoveFatal> {
    if !w.has_inventory(p) || !w.has_inventory(c) {
        return Ok(false);
    }
    let mut all = true;
    let mut moved = false;
    if let Some(x) = w.cursor(p) {
        moved = true;
        // Detaches the cursor item from P (`inventory.md` §1.4 rule 3).
        w.set_cursor(p, None);
        match w.find_free(c, x, page::INVENTORY) {
            Some((px, py)) if w.place_at(c, x, page::INVENTORY, px, py) => {
                clear_uflags(w, x, uflag::TARGETABLE);
                w.set_mode(x, mode::STORED);
                w.set_page(x, page::INVENTORY);
                gone_from(w, p, x);
            }
            _ => all = false,
        }
    }
    for loc in 0..=12u8 {
        let Some(x) = w.body_item(p, loc) else {
            continue;
        };
        moved = true;
        super::handlers::remove_from_body(w, p, x)?;
        if w.place_body(c, x, loc) {
            w.set_body_loc(x, loc);
            w.set_mode(x, mode::EQUIPPED);
            w.set_page(x, page::NONE);
            gone_from(w, p, x);
        } else {
            all = false;
        }
    }
    // The original's `0x0057F700` calls no refresh for P; with nothing
    // moved P's client hears nothing (recorded `items-drops-cha-00`
    // frame 96: no 0x47 / 0x48). PROVISIONAL (REC-2812): with items moved
    // the refresh stays d2rs-own.
    if moved {
        w.stat_refresh(p);
        owner_refresh(w, p);
        w.inventory_pass(p);
    }
    Ok(all)
}

/// Corpse take-back `0x00562F30(game, U, C)` (§12.2). True (result 1)
/// when the last sweep met no can-pick refusal and no failed grid put.
pub fn corpse_take_back<W: MoveWorld>(w: &mut W, u: Owner, c: Owner) -> Result<bool, MoveFatal> {
    if !w.has_inventory(u) || !w.has_inventory(c) {
        return Ok(false);
    }
    // Phase 1: body locations.
    loop {
        let mut present = 0u32;
        let mut moved = false;
        for b in 1..=12u8 {
            let Some(x) = w.body_item(c, b) else {
                continue;
            };
            present += 1;
            if !w.requirements(x, u, false) {
                continue;
            }
            let d = w.body_item(u, b);
            let pair = pair_location(b);
            let a = if pair != 0 { w.body_item(u, pair) } else { d };
            let (fit, l) = w.corpse_slot_fit(u, x, d, a, b);
            if !fit {
                if !grid_put(w, u, x, false)? {
                    continue;
                }
                w.replenish_timers(x);
            } else {
                if w.body_item(u, l).is_some() {
                    return Err(MoveFatal::SlotHeld);
                }
                if !w.place_body(u, x, l) {
                    continue;
                }
                let k = if l == 11 || l == 12 { 4 } else { 3 };
                if !w.link_check(u, x, k) {
                    w.clear_body_slot(u, l);
                    continue;
                }
                w.set_cursor(u, None);
                w.set_body_loc(x, l);
                clear_uflags(w, x, uflag::TARGETABLE);
                w.set_mode(x, mode::EQUIPPED);
                add_cmd(w, x, cmd::EQUIP);
                w.update_list_add(u, x);
                owner_refresh(w, u);
                clear_uflags(w, x, uflag::ON_GROUND);
                w.set_page(x, page::NONE);
                if k == 3 {
                    w.stat_link(u, x);
                    w.stat_refresh(u);
                    w.charm_relink(u, x);
                    w.weapon_bookkeeping(u);
                }
                w.quest_item_picked(u, x);
                w.replenish_timers(x);
            }
            // Step 7: X is no longer C's (the unlink returns none).
            add_iflags(w, x, iflag::CHANGED);
            let _ = w.unlink(c, x);
            w.clear_body_slot(c, b);
            present -= 1;
            moved = true;
        }
        if present == 0 || !moved {
            break;
        }
    }
    // Phase 2: every item left in C, list order.
    let mut pass = false;
    let r = loop {
        let mut placed = 0u32;
        let mut r = true;
        for x in w.items(c) {
            if !can_pick(w, u, x) {
                r = false;
                continue;
            }
            if w.equip_picked(u, x) {
                placed += 1;
                add_iflags(w, x, iflag::CHANGED);
                continue;
            }
            let belted = w.beltable(x)
                && w.belt_free_slot(u, x)
                    .is_some_and(|s| w.belt_place(u, x, u32::from(s)));
            if belted {
                if !w.link_check(u, x, 2) {
                    continue;
                }
                w.set_cursor(u, None);
                if u.is_player() {
                    w.charm_relink(u, x);
                }
                clear_uflags(w, x, uflag::TARGETABLE);
                if w.is_active(u, x) {
                    w.stat_refresh(u);
                }
                w.set_mode(x, mode::BELT);
                clear_uflags(w, x, uflag::ON_GROUND);
                add_cmd(w, x, cmd::PICKED_TO_BELT);
                w.set_page(x, page::NONE);
                w.update_list_add(u, x);
                owner_refresh(w, u);
                placed += 1;
                add_iflags(w, x, iflag::CHANGED);
                continue;
            }
            if pass {
                if grid_put(w, u, x, false)? {
                    placed += 1;
                    add_iflags(w, x, iflag::CHANGED);
                } else {
                    r = false;
                }
            }
        }
        if placed != 0 {
            continue;
        }
        if !pass {
            pass = true;
            continue;
        }
        break r;
    };
    w.inventory_pass(u);
    Ok(r)
}

/// Pickup to the cursor `0x0055CF50` (§8.2, cursor flag ≠ 0).
pub fn pickup_to_cursor<W: MoveWorld>(w: &mut W, player: Owner, item: Guid) -> Outcome {
    if w.cursor(player).is_some() || w.busy(player) {
        return Outcome::NOTHING;
    }
    w.targeting_reset(player);
    if !exists(w, item) {
        return Outcome::NOTHING;
    }
    if w.mode(item) != mode::GROUND {
        return Outcome::REFUSED;
    }
    if !can_pick(w, player, item) {
        refused_pickup(w, player, item, sound::REFUSED_PICKUP);
        return Outcome::NOTHING;
    }
    w.room_delete_notice(item);
    if w.is_type(item, ty::GOLD) {
        gold_pickup(w, player, item);
    } else {
        w.free_collision(item);
        w.remove_from_room(item);
        clear_uflags(w, item, uflag::TARGETABLE);
        w.set_cursor(player, Some(item));
        w.set_mode(item, mode::CURSOR);
        add_cmd(w, item, cmd::GROUND_TO_CURSOR);
        w.set_page(item, page::NONE);
        owner_refresh(w, player);
        clear_uflags(w, item, uflag::ON_GROUND);
        w.update_list_add(player, item);
        w.quest_item_picked(player, item);
    }
    w.pickup_sound(player, item);
    Outcome::DONE
}

/// Refused pickup `0x0055C9A0` (§8.3).
pub fn refused_pickup<W: MoveWorld>(w: &mut W, player: Owner, item: Guid, s: u32) {
    w.room_delete_notice(item);
    w.set_page(item, page::NONE);
    add_uflags(w, item, uflag::DROPPED);
    w.set_mode(item, mode::GROUND);
    w.sound(player, s);
}

/// Pickup specials `0x00560020` (§8.1 step 4): a scroll into a tome, a
/// book onto a tome, an auto-stack item onto existing stacks. True =
/// handled.
pub fn pickup_special<W: MoveWorld>(
    w: &mut W,
    player: Owner,
    item: Guid,
) -> Result<bool, MoveFatal> {
    if w.is_type(item, ty::SCRO) {
        let Some(t) = tome_for(w, player, item) else {
            return Ok(false);
        };
        return Ok(super::handlers::scroll_into_book(w, player, item, t)?.ok);
    }
    if w.is_type(item, ty::BOOK) {
        return book_onto_tome(w, player, item);
    }
    if w.stackable(item) && w.autostack(item) {
        return Ok(auto_stack(w, player, item));
    }
    Ok(false)
}

/// "Tome for P" `0x0063C3B0` (§8.1 step 4): the first item of page 0's
/// grid item list of primary type 18 whose spell equals P's and whose
/// stat 70 is below its total max stack.
pub fn tome_for<W: MoveWorld>(w: &W, player: Owner, p: Guid) -> Option<Guid> {
    let spell = w.spell(p);
    w.page_items(player, page::INVENTORY)
        .into_iter()
        .find(|&t| {
            w.primary_type(t) == ty::BOOK
                && w.spell(t) == spell
                && w.stat(Owner::item(t), stat::QUANTITY) < w.max_stack(t)
        })
}

/// P leaves its room and is freed, cursor := none (§8.1 step 4).
fn consume_picked<W: MoveWorld>(w: &mut W, player: Owner, p: Guid) {
    leave_room(w, p);
    clear_uflags(w, p, uflag::TARGETABLE);
    w.free_item(p);
    w.set_cursor(player, None);
}

/// Book onto a tome `0x0055D370` (§8.1 step 4).
fn book_onto_tome<W: MoveWorld>(w: &mut W, player: Owner, p: Guid) -> Result<bool, MoveFatal> {
    let Some(t) = tome_for(w, player, p) else {
        return Ok(false);
    };
    let (po, to) = (Owner::item(p), Owner::item(t));
    let (qp, qt, m) = (
        w.stat(po, stat::QUANTITY),
        w.stat(to, stat::QUANTITY),
        w.max_stack(t),
    );
    if qp < 0 || qt < 0 || m < 0 {
        return Err(MoveFatal::NegativeQuantity);
    }
    if qt.wrapping_add(qp) > m {
        w.set_stat(to, stat::QUANTITY, m);
        w.send_item_stat(player, t, stat::QUANTITY);
        w.set_stat(po, stat::QUANTITY, qt.wrapping_add(qp).wrapping_sub(m));
        w.send_item_stat(player, p, stat::QUANTITY);
        w.book_count_changed(player, t, m.wrapping_sub(qt));
    } else {
        w.set_stat(to, stat::QUANTITY, qt.wrapping_add(qp));
        w.send_item_stat(player, t, stat::QUANTITY);
        w.book_count_changed(player, t, qp);
        consume_picked(w, player, p);
    }
    Ok(true)
}

/// Auto-stack `0x0055D0D0` (§8.1 step 4): onto the stacks of the body
/// grid (when P's itemtype `quiver` ≠ 0), then of page 0's grid, in grid
/// list order, while P's quantity lasts. No candidate → not handled
/// (earlier partial merges stay).
fn auto_stack<W: MoveWorld>(w: &mut W, player: Owner, p: Guid) -> bool {
    let po = Owner::item(p);
    let mut cands = Vec::new();
    if w.quiver(p) {
        cands.extend(w.body_items(player));
    }
    cands.extend(w.page_items(player, page::INVENTORY));
    let books = |w: &W, d: Guid| w.primary_type(p) == ty::BOOK && w.primary_type(d) == ty::BOOK;
    let mut next = cands.into_iter();
    while w.stat(po, stat::QUANTITY) > 0 {
        let Some(d) = next.by_ref().find(|&d| {
            d != p && w.stack_test(p, d) && w.stat(Owner::item(d), stat::QUANTITY) < w.max_stack(d)
        }) else {
            return false;
        };
        let dd = Owner::item(d);
        let (q, qd, m) = (
            w.stat(po, stat::QUANTITY),
            w.stat(dd, stat::QUANTITY),
            w.max_stack(d),
        );
        if qd.wrapping_add(q) <= m {
            if w.merge_allowed(p) {
                let dp = w.stat(po, stat::DURABILITY);
                if dp < w.stat(dd, stat::DURABILITY) {
                    w.set_stat(dd, stat::DURABILITY, dp);
                    w.send_item_stat(player, d, stat::DURABILITY);
                }
            }
            w.set_stat(dd, stat::QUANTITY, qd.wrapping_add(q));
            w.send_item_stat(player, d, stat::QUANTITY);
            w.set_stat(po, stat::QUANTITY, 0);
            if books(w, d) {
                w.book_count_changed(player, d, q);
            }
            consume_picked(w, player, p);
            return true;
        }
        w.set_stat(dd, stat::QUANTITY, m);
        w.send_item_stat(player, d, stat::QUANTITY);
        w.set_stat(po, stat::QUANTITY, q.wrapping_add(qd).wrapping_sub(m));
        if books(w, d) {
            w.book_count_changed(player, d, m.wrapping_sub(qd));
        }
    }
    false
}

/// Code pairs the held test treats as equal, both orders (§8.4 rule 6,
/// `0x0055CA00`; the full list).
pub const HELD_PAIRS: [([u8; 4], [u8; 4]); 9] = [
    (*b"j34 ", *b"g34 "),
    (*b"bks ", *b"bkd "),
    (*b"d33 ", *b"g33 "),
    (*b"hst ", *b"msf "),
    (*b"hst ", *b"vip "),
    (*b"qf2 ", *b"qf1 "),
    (*b"qf2 ", *b"qhr "),
    (*b"qf2 ", *b"qey "),
    (*b"qf2 ", *b"qbr "),
];

/// Quest-pickup table `0x00731FEC`: (quest, `quest` value).
pub const QUEST_PICKUP: [(u8, u8); 4] = [(3, 4), (0x12, 0x11), (0x13, 0x12), (0x1B, 0x19)];

fn same_code(a: [u8; 4], b: [u8; 4]) -> bool {
    a == b
        || HELD_PAIRS
            .iter()
            .any(|&(x, y)| (a == x && b == y) || (a == y && b == x))
}

fn carry_one_unique<W: MoveWorld>(w: &W, item: Guid) -> bool {
    w.quality(item) == crate::items::q::UNIQUE && w.file_index(item) >= 0 && w.carry_one(item)
}

/// Held test `0x0055CA40` (§8.4 rule 6): true = the player already holds
/// a matching item. Walks the player's item list, then the item lists of
/// the player's corpses; each walk stops at the picked item P itself.
pub fn held<W: MoveWorld>(w: &W, player: Owner, item: Guid) -> bool {
    let unique = carry_one_unique(w, item);
    let quest = w.quest(item);
    let code = w.code(item);
    let mut units = vec![player];
    units.extend(w.held_test_units(player));
    units.into_iter().any(|u| {
        w.items(u).into_iter().take_while(|&h| h != item).any(|h| {
            w.page(h) != page::TRADE1
                && ((unique
                    && w.quality(h) == crate::items::q::UNIQUE
                    && w.file_index(h) == w.file_index(item))
                    || (w.quest(h) != 0 && w.quest(h) == quest && same_code(w.code(h), code)))
        })
    })
}

/// Can pick `0x0055CC90` (§8.4).
pub fn can_pick<W: MoveWorld>(w: &W, player: Owner, item: Guid) -> bool {
    if !w.has_inventory(player) {
        return false;
    }
    if carry_one_unique(w, item) && held(w, player, item) {
        return false;
    }
    let quest = w.quest(item);
    if quest == 0 {
        return true;
    }
    let f = |q, b| w.quest_flag(player, q, b);
    let code = w.code(item);
    let by_code = match &code {
        b"ass " => !f(9, 5),
        b"j34 " => !f(20, 0),
        b"xyz " => !f(20, 5),
        b"g33 " => !f(19, 7) && !f(19, 8),
        b"tr2 " => f(37, 8) && !f(37, 7),
        _ => true,
    };
    if !by_code {
        return false;
    }
    let by_value = match quest {
        10 => code == CUBE_CODE || !f(10, 0),
        5 => &code == b"leg " || !f(4, 0),
        v => QUEST_PICKUP
            .iter()
            .find(|&&(_, val)| val == v)
            .is_none_or(|&(q, _)| !f(q, 0)),
    };
    by_value && !held(w, player, item)
}

// ------------------------------------------------------------------ §9

/// Start point and free-spot search (§9.1 step 2; `last` is the search's
/// last argument: 1 for a drop as `items/treasure.md` §7 rule 2, 0 for
/// gold piles §10.2).
pub fn drop_spot<W: MoveWorld>(w: &W, unit: Owner, last: u32) -> Option<Spot> {
    let (x, y) = w.pos(unit);
    let start = if w.room_at(x + 2, y + 3) {
        (x + 2, y + 3)
    } else {
        (x, y)
    };
    w.free_spot(start, (x, y), 1, DROP_MASK, DROP_MASK2, last)
}

/// Ground expiry `0x00558A10` (§9.2): the value stored at item data +0x24.
/// Quest items store the absolute value 0 ("never"; not frame + 0).
pub fn ground_expiry<W: MoveWorld>(w: &W, item: Guid) -> i32 {
    if w.quest(item) != 0 {
        return 0;
    }
    let q = w.quality(item);
    let add = if q == 4 {
        30000
    } else if (5..=9).contains(&q)
        || (w.is_type(item, ty::GOLD) && w.stat(Owner::item(item), stat::GOLD) > 10000)
    {
        45000
    } else if w.socket_filler(item) {
        30000
    } else {
        15000
    };
    w.frame().wrapping_add(add)
}

/// Ground placement `0x00558AA0` (§9.1 step 3).
pub fn ground_place<W: MoveWorld>(w: &mut W, item: Guid, spot: Spot) {
    w.set_pos(Owner::item(item), spot.x, spot.y);
    w.add_to_room(item, spot);
    add_uflags(w, item, uflag::GROUND_PLACED);
    w.set_mode(item, mode::GROUND);
    w.set_page(item, page::NONE);
    add_uflags(w, item, uflag::ON_GROUND);
    let e = ground_expiry(w, item);
    w.set_expiry(item, e);
    if w.in_room(item) {
        w.quest_item_dropped(item);
    }
}

/// Drop the cursor item `0x00563C00` (§9.1).
pub fn drop_cursor_item<W: MoveWorld>(
    w: &mut W,
    player: Owner,
    item: Guid,
) -> Result<(), MoveFatal> {
    w.targeting_reset(player);
    if !exists(w, item) || w.mode(item) != mode::CURSOR {
        return Ok(());
    }
    let Some(spot) = drop_spot(w, player, 1) else {
        return Ok(());
    };
    ground_place(w, item, spot);
    w.set_cursor(player, None);
    changed_if_filled(w, item);
    clear_iflags(w, item, iflag::NOEQUIP);
    if w.code(item) == CUBE_CODE {
        cube_spill(w, player)?;
    }
    Ok(())
}

/// Cube spill `0x00563840` (§9.3).
pub fn cube_spill<W: MoveWorld>(w: &mut W, player: Owner) -> Result<(), MoveFatal> {
    let list: Vec<Guid> = w
        .items(player)
        .into_iter()
        .filter(|&i| w.page(i) == page::CUBE)
        .collect();
    for it in list {
        send_item_page(w, player, it, iflag::COPIED, page::CUBE)?;
        let (x, y) = w.pos(Owner::item(it));
        // The unlink not returning the item is a fatal assert (§9.3).
        if !w.unlink(player, it) {
            return Err(MoveFatal::Unlink);
        }
        w.room_change_notice(it, x, y);
        w.set_mode(it, mode::CURSOR);
        w.set_page(it, page::INVENTORY);
        if !w.place_in_page(player, it, 0, 0, true, true) {
            w.set_page(it, page::NONE);
            if let Some(spot) = drop_spot(w, player, 1) {
                ground_place(w, it, spot);
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ §10

/// Gold limit `0x00622E70`: level × 10000.
pub fn gold_limit<W: MoveWorld>(w: &W, player: Owner) -> i32 {
    w.stat(player, stat::LEVEL).wrapping_mul(GOLD_PER_LEVEL)
}

/// Gold pickup `0x0055C850` (§10.1).
pub fn gold_pickup<W: MoveWorld>(w: &mut W, player: Owner, pile: Guid) {
    let limit = i64::from(gold_limit(w, player));
    let g = i64::from(w.stat(player, stat::GOLD));
    let p = i64::from(w.stat(Owner::item(pile), stat::GOLD));
    let (take, rest) = if g + p > limit {
        (limit - g, p - (limit - g))
    } else {
        (p, 0)
    };
    match w.pile_owner(pile) {
        Some(o) if o.is_player() => w.owned_gold_pickup(player, pile, take as i32),
        _ => {
            if w.party_share_id(player) != -1 {
                w.party_share(player, take as i32);
            } else {
                let sum = g + take;
                let v = if sum < 0 || sum > limit { 0 } else { sum };
                w.set_stat(player, stat::GOLD, v as i32);
            }
        }
    }
    leave_room(w, pile);
    clear_uflags(w, pile, uflag::TARGETABLE);
    w.free_item(pile);
    if rest > 0 {
        w.rest_pile(player, rest as i32);
    }
}

/// Gold piles `0x0055A090` (§10.2): up to `max` piles of `amount` near
/// `unit`, in creation order.
pub fn gold_piles<W: MoveWorld>(w: &mut W, unit: Owner, amount: i32, max: usize) -> Vec<Guid> {
    let mut out = Vec::new();
    let mut placed: i64 = 0;
    let amount = i64::from(amount);
    while placed < amount && out.len() < max {
        let pile = (amount - placed).min(i64::from(PILE_CAP));
        let Some(spot) = drop_spot(w, unit, 0) else {
            break;
        };
        // A failed creation skips that pile and does not stop: its amount
        // already counts as placed (§10.2).
        let Some(g) = w.create_gold(unit, spot) else {
            placed += pile;
            continue;
        };
        w.set_stat(Owner::item(g), stat::GOLD, pile.max(0) as i32);
        ground_place(w, g, spot);
        out.push(g);
        placed += pile;
    }
    out
}

/// Interval of the ground expiry reader (§9.2: every 1,500 frames).
pub const EXPIRY_INTERVAL: i32 = 1500;

/// Ground expiry reader `0x00558B90(game, act)` (§9.2): of the units of
/// an act's rooms, in room order and room unit-list order, the items
/// whose expiry is ≠ 0 and ≤ `frame` (read once at entry). Returns them
/// in that order; removing each (room unit removal when in a room, unit
/// flag 0x2 cleared, `0x005538D0`, unit free `0x00555600`) is the
/// caller's.
pub fn expired_items(units: &[(Guid, i32)], frame: i32) -> Vec<Guid> {
    units
        .iter()
        .filter(|&&(_, e)| e != 0 && e <= frame)
        .map(|&(g, _)| g)
        .collect()
}
