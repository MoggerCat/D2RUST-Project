// Spec: specs/items/inventory.md
//! Pickup from the ground (§8), drop to the ground (§9) and gold (§10).

use super::deferred::{owner_refresh, send_item_page};
use super::seams::{MoveWorld, Spot};
use super::{
    add_cmd, add_uflags, changed_if_filled, clear_iflags, clear_uflags, cmd, exists, iflag, mode,
    page, sound, stat, ty, uflag, Guid, MoveFatal, Outcome, Owner, CUBE_CODE, DROP_MASK,
    DROP_MASK2, GOLD_PER_LEVEL, PILE_CAP,
};

/// "Leave the room": room delete notice, collision freed, room list
/// (§2.2; idempotent on the provider's side).
pub fn leave_room<W: MoveWorld>(w: &mut W, item: Guid) {
    w.room_delete_notice(item);
    w.free_collision(item);
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
    if w.pickup_special(player, item) {
        return Ok(Outcome::DONE);
    }
    if let Some(l) = w.auto_equip(player, item, false) {
        if w.equip_check(player, l, Some(item), false) != 1 {
            return Ok(Outcome::REFUSED);
        }
        leave_room(w, item);
        // TODO(spec: inventory.md §8.1 r5): the result of a failed
        // `0x00562E00` is not written; read as "nothing".
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
    let placed = match w.find_free(player, item, page::INVENTORY) {
        Some((x, y)) => w.place_at(player, item, page::INVENTORY, x, y),
        None => false,
    };
    if !placed {
        refused_pickup(w, player, item, sound::NO_ROOM);
        return Ok(Outcome::NOTHING);
    }
    leave_room(w, item);
    // TODO(spec: inventory.md §8.1 r7): the link's failure is not written
    // (`0x005600A0`); ignored.
    w.link_check(player, item, 1);
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
    Ok(Outcome::DONE)
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

/// Code pairs the held test treats as equal (§8.4 rule 6; more pairs not
/// read: OQ19).
pub const HELD_PAIRS: [([u8; 4], [u8; 4]); 4] = [
    (*b"j34 ", *b"g34 "),
    (*b"bks ", *b"bkd "),
    (*b"d33 ", *b"g33 "),
    (*b"hst ", *b"msf "),
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
/// a matching item.
pub fn held<W: MoveWorld>(w: &W, player: Owner, item: Guid) -> bool {
    let unique = carry_one_unique(w, item);
    let quest = w.quest(item);
    let code = w.code(item);
    let mut units = vec![player];
    units.extend(w.held_test_units(player));
    units.into_iter().any(|u| {
        w.items(u).into_iter().any(|h| {
            h != item
                && w.page(h) != page::TRADE1
                && ((unique && carry_one_unique(w, h) && w.file_index(h) == w.file_index(item))
                    || (quest != 0 && w.quest(h) == quest && same_code(w.code(h), code)))
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
/// Quest items store 0 ("never").
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
        // TODO(spec: inventory.md §9.3): an unlink failure here is not
        // written; ignored.
        w.unlink(player, it);
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
        // TODO(spec: inventory.md §10.2): a failed creation is not
        // written; read as "stop".
        let Some(g) = w.create_gold(unit, spot) else {
            break;
        };
        w.set_stat(Owner::item(g), stat::GOLD, pile.max(0) as i32);
        ground_place(w, g, spot);
        out.push(g);
        placed += pile;
    }
    out
}
