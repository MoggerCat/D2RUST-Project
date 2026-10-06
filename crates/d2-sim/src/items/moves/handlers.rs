// Spec: specs/items/inventory.md
//! The item-move intents (§7): C→S 0x16–0x29, 0x50, 0x61 (expansion only)
//! and 0x63, each in the spec's validation order with its result codes.
//! 0x4C is `world/cube.md` §10's. Layouts: `sim/client-messages.tsv`.

use super::deferred::{mark, owner_refresh, send_item_page, send_to_belt};
use super::ground::{drop_cursor_item, gold_limit, gold_piles, pickup_auto, pickup_to_cursor};
use super::layouts;
use super::seams::MoveWorld;
use super::{
    add_cmd, add_iflags, changed_if_filled, clear_iflags, clear_uflags, cmd, exists, iflag, mode,
    page, res, stat, ty, uflag, Guid, MoveFatal, Outcome, Owner, CUBE_CODE, MAX_PILES,
    PICK_COLLISION_MASK, PICK_RANGE, PILE_CAP, USE_RANGE, WALK_RANGE,
};

/// The ids handled here with their exact handler size (`handler_size`
/// column of `client-messages.tsv`).
pub const HANDLED: [(u8, usize); 23] = [
    (0x16, 13),
    (0x17, 5),
    (0x18, 17),
    (0x19, 5),
    (0x1A, 9),
    (0x1B, 9),
    (0x1C, 3),
    (0x1D, 9),
    (0x1E, 9),
    (0x1F, 17),
    (0x20, 13),
    (0x21, 9),
    (0x22, 5),
    (0x23, 9),
    (0x24, 5),
    (0x25, 9),
    (0x26, 13),
    (0x27, 9),
    (0x28, 9),
    (0x29, 9),
    (0x50, 9),
    (0x61, 3),
    (0x63, 5),
];

fn u32_at(m: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([m[o], m[o + 1], m[o + 2], m[o + 3]])
}

fn u16_at(m: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([m[o], m[o + 1]])
}

/// Dispatches one C→S item message for `player`. `None` when the id is
/// not one of [`HANDLED`]. The size check (→ 3) comes first (§7 text).
pub fn handle<W: MoveWorld>(w: &mut W, player: Guid, msg: &[u8]) -> Option<Result<u32, MoveFatal>> {
    let id = *msg.first()?;
    let &(_, size) = HANDLED.iter().find(|&&(i, _)| i == id)?;
    if msg.len() != size {
        return Some(Ok(res::REFUSED));
    }
    let p = Owner::player(player);
    let m = msg;
    Some(match id {
        0x16 => pick_item(w, p, u32_at(m, 1), u32_at(m, 5), u32_at(m, 9)),
        0x17 => drop_item(w, p, u32_at(m, 1)),
        0x18 => Ok(insert_item(
            w,
            p,
            u32_at(m, 1),
            u32_at(m, 5),
            u32_at(m, 9),
            u32_at(m, 13),
        )),
        0x19 => remove_from_buffer(w, p, u32_at(m, 1)),
        0x1A => Ok(equip_item(w, p, u32_at(m, 1), m[5])),
        0x1B => swap_2handed(w, p, u32_at(m, 1), m[5]),
        0x1C => remove_body_item(w, p, u16_at(m, 1)),
        0x1D => Ok(swap_cursor_with_body(w, p, u32_at(m, 1), m[5])),
        0x1E => Ok(swap_1h_with_2h(w, p, u32_at(m, 1), m[5])),
        0x1F => swap_cursor_buffer(
            w,
            p,
            u32_at(m, 1),
            u32_at(m, 5),
            u32_at(m, 9),
            u32_at(m, 13),
        ),
        0x20 => Ok(use_grid_item(
            w,
            p,
            u32_at(m, 1),
            u32_at(m, 5),
            u32_at(m, 9),
        )),
        0x21 => Ok(stack_items(w, p, u32_at(m, 1), u32_at(m, 5))),
        0x22 => Ok(unstack_items(w, p, u32_at(m, 1))),
        0x23 => Ok(item_to_belt(w, p, u32_at(m, 1), u32_at(m, 5))),
        0x24 => item_from_belt(w, p, u32_at(m, 1)),
        0x25 => switch_belt_item(w, p, u32_at(m, 1), u32_at(m, 5)),
        0x26 => Ok(use_belt_item(w, p, u32_at(m, 1), u32_at(m, 5))),
        0x27 => Ok(use_item_action(w, p, u32_at(m, 1), u32_at(m, 5))),
        0x28 => socket_item(w, p, u32_at(m, 1), u32_at(m, 5)),
        0x29 => scroll_to_book(w, p, u32_at(m, 1), u32_at(m, 5)),
        0x50 => Ok(drop_gold(w, p, u32_at(m, 1), u32_at(m, 5))),
        0x61 => merc_item(w, p, u16_at(m, 1)),
        0x63 => item_to_belt_shift(w, p, u32_at(m, 1)),
        _ => unreachable!("HANDLED lists only the ids above"),
    })
}

fn valid_loc(loc: u32) -> bool {
    (1..=10).contains(&loc)
}

/// Chebyshev range test `0x00548EF0` around the player (`intents-events.md`
/// §2.4 rule 3).
fn within<W: MoveWorld>(w: &W, player: Owner, x: i32, y: i32, r: i32) -> bool {
    let (px, py) = w.pos(player);
    (i64::from(x) - i64::from(px)).abs() <= i64::from(r)
        && (i64::from(y) - i64::from(py)).abs() <= i64::from(r)
}

// ------------------------------------------------------------------ 0x16

/// 0x16 PickItem `0x0054AAD0` (§7.1).
pub fn pick_item<W: MoveWorld>(
    w: &mut W,
    p: Owner,
    unit_type: u32,
    guid: Guid,
    cursor: u32,
) -> Result<u32, MoveFatal> {
    if unit_type > 5 {
        return Ok(res::BAD);
    }
    if unit_type == 0 && guid == p.guid {
        return Ok(res::REFUSED);
    }
    match unit_type {
        1 => Ok(w.pick_npc(p, guid, cursor)),
        2 => Ok(w.pick_object(p, guid, cursor)),
        4 => {
            let it = Owner::item(guid);
            if !exists(w, guid) || w.mode(guid) != mode::GROUND || w.distance(p, it) > PICK_RANGE {
                return Ok(res::RANGE);
            }
            if w.distance(p, it) >= WALK_RANGE || w.collides(p, it, PICK_COLLISION_MASK) {
                w.walk_to_item(p, guid, cursor != 0);
                return Ok(res::OK);
            }
            let o = if cursor != 0 {
                pickup_to_cursor(w, p, guid)
            } else {
                pickup_auto(w, p, guid)?
            };
            Ok(o.result())
        }
        // TODO(spec: inventory.md §7.1 r2, OQ10): types 0, 3 and 5.
        t => Ok(w.pick_other(p, t, guid, cursor)),
    }
}

// ------------------------------------------------------------------ 0x17

/// 0x17 DropItem `0x0054AB40` (§7.2).
pub fn drop_item<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) -> Result<u32, MoveFatal> {
    let r = w.check_cursor_item(p, item);
    if r != 0 {
        return Ok(r);
    }
    if w.busy(p) && w.trading(p) {
        return Ok(res::REFUSED);
    }
    drop_cursor_item(w, p, item)?;
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x18

/// 0x18 InsertItemInBuffer `0x0054ABB0` (§7.3).
pub fn insert_item<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, x: u32, y: u32, pg: u32) -> u32 {
    let r = w.check_cursor_item(p, item);
    if r != 0 {
        return r;
    }
    if !w.unit_exists(p) || !p.is_player() {
        if pg > 4 {
            return res::BAD;
        }
    } else {
        // Step 3: "busy = 0 and page ≠ 0 → 2" is unreachable (the cursor
        // item makes the player busy); kept in its place.
        if !w.busy(p) && pg != 0 {
            return res::BAD;
        }
        match pg {
            4 if !w.in_town(p) => return res::REFUSED,
            2 if !w.trading(p) => return res::REFUSED,
            1 => return res::BAD,
            n if n >= 5 => return res::BAD,
            _ => {}
        }
    }
    if !exists(w, item) {
        return res::BAD;
    }
    w.set_page(item, pg as u8);
    // §2.4 step 1 (owner present; targeting reset), then steps 2–9.
    w.targeting_reset(p);
    if w.place_in_page(p, item, x as i32, y as i32, false, true) {
        res::OK
    } else {
        res::REFUSED
    }
}

// ------------------------------------------------------------------ 0x19

/// 0x19 RemoveItemFromBuffer `0x0054ACD0` (§7.4).
pub fn remove_from_buffer<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) -> Result<u32, MoveFatal> {
    let r = w.check_stored(p, item);
    if r != 0 {
        return Ok(r);
    }
    if w.cursor(p).is_some() {
        w.resync(p);
        return Ok(res::BAD);
    }
    if !exists(w, item) {
        return Ok(res::BAD);
    }
    if w.page(item) == page::TRADE1 {
        return Ok(res::REFUSED);
    }
    if !w.item_move_gate(p, Some(item)) {
        return Ok(res::OK);
    }
    Ok(to_cursor(w, p, item)?.result())
}

/// To cursor `0x00560420(item, &out, send 1, 0, 0, 0)` (§7.4).
pub fn to_cursor<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) -> Result<Outcome, MoveFatal> {
    if w.cursor(p).is_some() {
        return Ok(Outcome::NOTHING);
    }
    if !exists(w, item) {
        return Ok(Outcome::REFUSED);
    }
    let pg = w.page(item);
    // TODO(spec: inventory.md §7.4, OQ12): the busy player's case.
    if p.is_player() && !w.busy(p) && pg != page::INVENTORY {
        return Ok(Outcome::REFUSED);
    }
    if w.mode(item) != mode::STORED {
        return Ok(Outcome::REFUSED);
    }
    w.targeting_reset(p);
    clear_uflags(w, item, uflag::TARGETABLE);
    let (x, y) = w.pos(Owner::item(item));
    if !w.unlink(p, item) {
        return Err(MoveFatal::Unlink);
    }
    w.stat_refresh_unlink(p, 1);
    if w.is_active(p, item) {
        w.inventory_pass(p);
    }
    w.charm_unlink(p, item);
    w.room_change_notice(item, x, y);
    w.set_stored_page(item, pg);
    w.set_page(item, page::NONE);
    if !(p.is_player() && pg == page::TRADE1) {
        w.set_cursor(p, Some(item));
    }
    w.set_mode(item, mode::CURSOR);
    add_cmd(w, item, cmd::FROM_PAGE);
    changed_if_filled(w, item);
    clear_iflags(w, item, iflag::NOEQUIP);
    w.update_list_add(p, item);
    owner_refresh(w, p);
    Ok(Outcome::DONE)
}

// ------------------------------------------------------------------ 0x1A

/// 0x1A EquipItem `0x0054AD90` (§7.5).
pub fn equip_item<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, loc: u8) -> u32 {
    let r = w.check_cursor_item(p, item);
    if r != 0 {
        return r;
    }
    if !valid_loc(u32::from(loc)) {
        return res::BAD;
    }
    let (ok, out) = w.equip_from_cursor(p, item, loc, false);
    Outcome { ok, out }.result()
}

// ------------------------------------------------------------------ 0x1B

fn other_hand(loc: u8) -> u8 {
    if loc == 4 {
        5
    } else {
        4
    }
}

/// Removal from the body (§7.6): `0x0062A360`, `0x0063D2B0`, unlink, slot
/// cleared; a belt (type 19) then `0x005608C0`.
fn remove_from_body<W: MoveWorld>(w: &mut W, owner: Owner, item: Guid) -> Result<(), MoveFatal> {
    let loc = w.body_loc(item);
    w.body_leave_effects(owner, item);
    if !w.unlink(owner, item) {
        return Err(MoveFatal::Unlink);
    }
    w.clear_body_slot(owner, loc);
    if w.is_type(item, ty::BELT) {
        w.belt_unequip(owner, item);
    }
    Ok(())
}

/// §4.6 step 5 with command flag `c` (0x8 there; 0x10000 for 0x1B).
fn equip_at<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, loc: u8, c: u32) -> Outcome {
    let kind = if loc == 11 || loc == 12 { 4 } else { 3 };
    if !w.place_body(p, item, loc) || !w.link_check(p, item, kind) {
        return Outcome::REFUSED;
    }
    w.set_body_loc(item, loc);
    if loc != 11 && loc != 12 {
        w.stat_link(p, item);
        w.stat_refresh(p);
    }
    w.set_cursor(p, None);
    clear_uflags(w, item, uflag::TARGETABLE);
    w.set_mode(item, mode::EQUIPPED);
    w.set_page(item, page::NONE);
    add_cmd(w, item, c);
    add_iflags(w, item, iflag::CHANGED);
    clear_iflags(w, item, iflag::NOEQUIP);
    w.update_list_add(p, item);
    owner_refresh(w, p);
    w.weapon_bookkeeping(p);
    w.inventory_pass(p);
    Outcome::DONE
}

/// 0x1B Swap2HandedItem `0x0054AE30` → `0x00563D20` (§7.6).
pub fn swap_2handed<W: MoveWorld>(w: &mut W, p: Owner, n: Guid, loc: u8) -> Result<u32, MoveFatal> {
    let r = w.check_cursor_item(p, n);
    if r != 0 {
        return Ok(r);
    }
    if !valid_loc(u32::from(loc)) {
        return Ok(res::BAD);
    }
    if loc != 4 && loc != 5 {
        return Ok(res::REFUSED);
    }
    let Some(x) = w.body_item(p, other_hand(loc)) else {
        return Ok(res::REFUSED);
    };
    if w.equip_check(p, loc, Some(n), false) != 2 {
        return Ok(res::REFUSED);
    }
    if !w.requirements(n, p, false) {
        w.stat_refresh(p);
        w.requirement_sound(p);
        return Ok(res::OK);
    }
    remove_from_body(w, p, x)?;
    w.set_cursor(p, Some(x));
    w.set_mode(x, mode::CURSOR);
    clear_uflags(w, x, uflag::TARGETABLE);
    Ok(equip_at(w, p, n, loc, cmd::INDIRECT_SWAP).result())
}

// ------------------------------------------------------------------ 0x1C

/// 0x1C RemoveBodyItem `0x0054AEC0` → `0x00560CD0` (§7.7).
pub fn remove_body_item<W: MoveWorld>(w: &mut W, p: Owner, loc: u16) -> Result<u32, MoveFatal> {
    if !valid_loc(u32::from(loc)) {
        return Ok(res::BAD);
    }
    let loc = loc as u8;
    let at = w.body_item(p, loc);
    if !w.item_move_gate(p, at) {
        return Ok(res::OK);
    }
    if loc == 8 && !w.belt_remove_allowed(p) {
        return Ok(res::OK);
    }
    if w.cursor(p).is_some() || at.is_none() {
        return Ok(res::OK);
    }
    let r = w.equip_check(p, loc, None, false);
    if r != 3 && r != 4 {
        return Ok(res::REFUSED);
    }
    // TODO(spec: inventory.md §7.7): `0x0063E490` finding no item is not
    // written; read as "nothing".
    let Some(it) = w.item_to_remove(p, loc) else {
        return Ok(res::OK);
    };
    remove_from_body(w, p, it)?;
    w.set_cursor(p, Some(it));
    w.stat_refresh(p);
    clear_uflags(w, it, uflag::TARGETABLE);
    w.set_mode(it, mode::CURSOR);
    add_cmd(w, it, cmd::UNEQUIP);
    changed_if_filled(w, it);
    clear_iflags(w, it, iflag::NOEQUIP);
    w.update_list_add(p, it);
    owner_refresh(w, p);
    w.weapon_bookkeeping(p);
    w.inventory_pass(p);
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x1D

/// 0x1D SwapCursorWithBody `0x0054AF50` → `0x00560F00` (§7.8).
pub fn swap_cursor_with_body<W: MoveWorld>(w: &mut W, p: Owner, n: Guid, loc: u8) -> u32 {
    let r = w.check_cursor_item(p, n);
    if r != 0 {
        return r;
    }
    if !valid_loc(u32::from(loc)) {
        return res::BAD;
    }
    let Some(at) = w.body_item(p, loc) else {
        return res::RANGE;
    };
    if loc == 8 && !w.belt_remove_allowed(p) {
        return res::OK;
    }
    if !w.item_move_gate(p, Some(at)) {
        return res::OK;
    }
    if w.equip_check(p, loc, Some(n), false) != 5 {
        return res::OK;
    }
    w.weapon_in_use_update(p);
    // TODO(spec: inventory.md §7.8): the result when `0x0063E490` gives no
    // item or one not in mode 1 is not written; read as "nothing".
    let Some(e) = w
        .item_to_remove(p, loc)
        .filter(|&e| w.mode(e) == mode::EQUIPPED)
    else {
        return res::OK;
    };
    w.stat_refresh(p);
    if !w.requirements(n, p, false) {
        w.stat_refresh(p);
        w.requirement_sound(p);
        return res::OK;
    }
    if w.is_type(n, ty::BELT) {
        w.belt_unequip(p, n);
    }
    // E leaves the body (as §7.6, without its own belt step).
    let eloc = w.body_loc(e);
    w.body_leave_effects(p, e);
    // TODO(spec: inventory.md §7.8): an unlink failure here is not
    // written; ignored.
    w.unlink(p, e);
    w.clear_body_slot(p, eloc);
    w.set_cursor(p, Some(e));
    w.set_mode(e, mode::CURSOR);
    add_iflags(w, e, iflag::SWAP_OUT);
    add_cmd(w, e, cmd::SWAP_BODY);
    add_iflags(w, e, iflag::CHANGED);
    w.update_list_add(p, e);
    // N goes to the location.
    // TODO(spec: inventory.md §7.8): a failed placement of N is not
    // written; its result is ignored.
    w.place_body(p, n, loc);
    w.set_body_loc(n, loc);
    w.stat_link(p, n);
    w.set_mode(n, mode::EQUIPPED);
    w.set_page(n, page::NONE);
    add_iflags(w, n, iflag::SWAP_IN | iflag::CHANGED);
    add_cmd(w, n, cmd::SWAP_BODY);
    w.update_list_add(p, n);
    owner_refresh(w, p);
    w.weapon_bookkeeping(p);
    w.inventory_pass(p);
    res::OK
}

// ------------------------------------------------------------------ 0x1E

/// 0x1E Swap1HWith2H `0x0054B030` (§7.9; body `0x00561220` is OQ14).
pub fn swap_1h_with_2h<W: MoveWorld>(w: &mut W, p: Owner, n: Guid, loc: u8) -> u32 {
    let r = w.check_cursor_item(p, n);
    if r != 0 {
        return r;
    }
    if !valid_loc(u32::from(loc)) {
        return res::BAD;
    }
    if loc != 4 && loc != 5 {
        return res::REFUSED;
    }
    let Some(at) = w.body_item(p, loc) else {
        return res::RANGE;
    };
    if !w.item_move_gate(p, Some(at)) {
        return res::OK;
    }
    let (ok, out) = w.swap_1h_with_2h(p, n, loc);
    Outcome { ok, out }.result()
}

// ------------------------------------------------------------------ 0x1F

/// 0x1F SwapCursorBufferItem `0x0054B0F0` → `0x00561B00` (§7.10).
pub fn swap_cursor_buffer<W: MoveWorld>(
    w: &mut W,
    p: Owner,
    c: Guid,
    t: Guid,
    x: u32,
    y: u32,
) -> Result<u32, MoveFatal> {
    let r = w.check_cursor_item(p, c);
    if r != 0 {
        return Ok(r);
    }
    let r = w.check_stored(p, t);
    if r != 0 {
        return Ok(r);
    }
    if !exists(w, t) {
        return Ok(res::BAD);
    }
    let pg = w.page(t);
    if pg == page::TRADE1 || pg == page::TRADE2 {
        return Ok(res::REFUSED);
    }
    if !w.item_move_gate(p, Some(t)) {
        return Ok(res::OK);
    }
    if pg == page::CUBE && w.code(c) == CUBE_CODE {
        return Ok(res::OK);
    }
    if pg == page::TRADE2 {
        return Ok(res::REFUSED);
    }
    // TODO(spec: inventory.md §7.10 r3): the result for a target not in
    // mode 0 is not written; read as "nothing".
    if w.mode(t) != mode::STORED {
        return Ok(res::OK);
    }
    let t_active = w.is_active(p, t);
    let (tx, ty_) = w.pos(Owner::item(t));
    if !w.unlink(p, t) {
        return Err(MoveFatal::Unlink);
    }
    w.stat_refresh(p);
    w.room_change_notice(t, tx, ty_);
    w.set_page(t, page::NONE);
    w.set_cursor(p, Some(t));
    w.charm_unlink(p, t);
    w.set_stored_page(t, pg);
    clear_uflags(w, t, uflag::TARGETABLE);
    w.set_mode(t, mode::CURSOR);
    add_cmd(w, t, cmd::SWAP_IN_PAGE);
    changed_if_filled(w, t);
    w.update_list_add(p, t);
    // The cursor item C.
    if !w.place_at(p, c, pg, x as i32, y as i32) {
        return Ok(res::REFUSED);
    }
    w.set_stored_page(c, 0);
    w.set_page(c, pg);
    w.set_pos(Owner::item(c), x as i32, y as i32);
    // TODO(spec: inventory.md §7.10 r3): the "link" step's failure is not
    // written; ignored.
    w.link_check(p, c, 1);
    w.charm_relink(p, c);
    if w.is_active(p, c) {
        w.stat_refresh(p);
    }
    w.set_mode(c, mode::STORED);
    add_cmd(w, c, cmd::SWAP_IN_PAGE);
    w.update_list_add(p, c);
    owner_refresh(w, p);
    if t_active {
        w.inventory_pass(p);
    }
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x20

/// 0x20 UseGridItem `0x0054B1E0` (§7.11; body is the item-use spec's).
pub fn use_grid_item<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, x: u32, y: u32) -> u32 {
    let r = w.check_stored(p, item);
    if r != 0 {
        return r;
    }
    if !within(w, p, x as i32, y as i32, USE_RANGE) {
        return res::RANGE;
    }
    let (ok, out) = w.use_grid_item(p, item, x as i32, y as i32);
    Outcome { ok, out }.result()
}

// ------------------------------------------------------------------ 0x21

/// 0x21 StackItems `0x0054B300` → `0x0055E7C0` (§7.12).
pub fn stack_items<W: MoveWorld>(w: &mut W, p: Owner, src: Guid, dst: Guid) -> u32 {
    for g in [src, dst] {
        let r = w.check_owned(p, g);
        if r != 0 {
            return r;
        }
    }
    if src == dst {
        return res::REFUSED;
    }
    if !exists(w, src) || !exists(w, dst) || w.page(dst) == page::TRADE2 {
        return res::REFUSED;
    }
    if !w.stack_test(src, dst) {
        return res::OK;
    }
    let (s, d) = (Owner::item(src), Owner::item(dst));
    let qs = w.stat(s, stat::QUANTITY);
    let qd = w.stat(d, stat::QUANTITY);
    let m = w.max_stack(dst);
    let books = w.is_type(src, ty::BOOK) && w.is_type(dst, ty::BOOK);
    if i64::from(qs) + i64::from(qd) > i64::from(m) {
        w.set_stat(d, stat::QUANTITY, m);
        w.send_item_stat(p, dst, stat::QUANTITY);
        w.set_stat(s, stat::QUANTITY, qs.wrapping_add(qd).wrapping_sub(m));
        w.send_item_stat(p, src, stat::QUANTITY);
        if books {
            w.book_count_changed(p, m.wrapping_sub(qd));
        }
        add_iflags(w, dst, iflag::STACK_FULL);
    } else if w.merge_allowed(src) {
        let ds = w.stat(s, stat::DURABILITY);
        if ds < w.stat(d, stat::DURABILITY) {
            // TODO(spec: inventory.md §7.12, OQ15): stat 72's meaning here.
            w.set_stat(d, stat::DURABILITY, ds);
            w.send_item_stat(p, dst, stat::DURABILITY);
        }
        w.set_stat(d, stat::QUANTITY, qs.wrapping_add(qd));
        w.send_item_stat(p, dst, stat::QUANTITY);
        if books {
            w.book_count_changed(p, qs);
        }
        w.set_cursor(p, None);
        w.send(p, layouts::clear_cursor(Owner::ITEM, src));
        w.free_item(src);
    }
    mark(w, p, dst, cmd::ADD_QUANTITY);
    res::OK
}

// ------------------------------------------------------------------ 0x22

/// 0x22 UnstackItems `0x0054B380` (§7.13): 3 for every owned item
/// (`0x0055E9A0` returns 0 and leaves the size in the refusal flag).
pub fn unstack_items<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) -> u32 {
    let r = w.check_owned(p, item);
    if r != 0 {
        return r;
    }
    res::REFUSED
}

// ------------------------------------------------------------------ 0x23

/// 0x23 ItemToBelt `0x0054B3E0` → `0x0055E9B0` (§7.14).
pub fn item_to_belt<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, slot: u32) -> u32 {
    let r = w.check_cursor_item(p, item);
    if r != 0 {
        return r;
    }
    to_belt(w, p, item, slot).result()
}

/// `0x0055E9B0(item, slot, find 0)` (§7.14).
pub fn to_belt<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, slot: u32) -> Outcome {
    w.targeting_reset(p);
    if !exists(w, item) || w.mode(item) != mode::CURSOR {
        return Outcome::REFUSED;
    }
    if !w.beltable(item) {
        return Outcome::NOTHING;
    }
    if !w.belt_place(p, item, slot) {
        w.set_page(item, page::NONE);
        return Outcome::NOTHING;
    }
    if !w.link_check(p, item, 2) {
        return Outcome::REFUSED;
    }
    w.set_cursor(p, None);
    w.charm_relink(p, item);
    clear_uflags(w, item, uflag::TARGETABLE);
    if w.is_active(p, item) {
        w.stat_refresh(p);
    }
    w.set_mode(item, mode::BELT);
    w.set_page(item, page::NONE);
    add_cmd(w, item, cmd::TO_BELT);
    owner_refresh(w, p);
    w.update_list_add(p, item);
    Outcome::DONE
}

// ------------------------------------------------------------------ 0x24

/// 0x24 ItemFromBelt `0x0054B450` → `0x00562250` (§7.15).
pub fn item_from_belt<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) -> Result<u32, MoveFatal> {
    let r = w.check_belt(p, item);
    if r != 0 {
        return Ok(r);
    }
    if !w.item_move_gate(p, None) {
        return Ok(res::OK);
    }
    if w.cursor(p).is_some() {
        return Ok(res::BAD);
    }
    if !exists(w, item) || w.mode(item) != mode::BELT {
        return Ok(res::REFUSED);
    }
    w.targeting_reset(p);
    let slot = w.pos(Owner::item(item)).0;
    if !w.unlink(p, item) {
        return Err(MoveFatal::Unlink);
    }
    w.set_cursor(p, Some(item));
    w.charm_unlink(p, item);
    clear_uflags(w, item, uflag::TARGETABLE);
    w.stat_refresh(p);
    w.set_mode(item, mode::CURSOR);
    add_cmd(w, item, cmd::FROM_BELT);
    owner_refresh(w, p);
    w.update_list_add(p, item);
    w.belt_compact(p, slot as u8);
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x25

/// 0x25 SwitchBeltItem `0x0054B4E0` → `0x0055EB30` (§7.16).
pub fn switch_belt_item<W: MoveWorld>(
    w: &mut W,
    p: Owner,
    c: Guid,
    b: Guid,
) -> Result<u32, MoveFatal> {
    let r = w.check_cursor_item(p, c);
    if r != 0 {
        return Ok(r);
    }
    let r = w.check_belt(p, b);
    if r != 0 {
        return Ok(r);
    }
    // The cursor item check guarantees mode 4.
    if !w.beltable(c) || w.mode(c) != mode::CURSOR {
        return Ok(res::OK);
    }
    if !exists(w, b) || w.mode(b) != mode::BELT {
        return Ok(res::REFUSED);
    }
    let slot = w.pos(Owner::item(b)).0;
    if !w.unlink(p, b) {
        return Err(MoveFatal::Unlink);
    }
    w.set_cursor(p, Some(b));
    w.set_mode(b, mode::CURSOR);
    add_cmd(w, b, cmd::SWAP_BELT);
    w.update_list_add(p, b);
    if !w.belt_place(p, c, slot as u32) {
        return Err(MoveFatal::BeltSwitch);
    }
    w.set_pos(Owner::item(c), slot, 0);
    // TODO(spec: inventory.md §7.16): the link's failure is not written;
    // ignored.
    w.link_check(p, c, 2);
    w.set_mode(c, mode::BELT);
    w.set_page(c, page::NONE);
    add_cmd(w, c, cmd::SWAP_BELT);
    w.update_list_add(p, c);
    owner_refresh(w, p);
    w.inventory_pass(p);
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x26

/// 0x26 UseBeltItem `0x0054B560` → `0x00562390` (§7.17).
pub fn use_belt_item<W: MoveWorld>(w: &mut W, p: Owner, item: Guid, on_merc: u32) -> u32 {
    let r = w.check_belt(p, item);
    if r != 0 {
        return r;
    }
    // The player's own position passes the 50-subtile test.
    if !exists(w, item) {
        return res::REFUSED;
    }
    if w.cursor(p).is_some() {
        return res::OK;
    }
    if w.mode(item) != mode::BELT || !w.useable(item) {
        return res::REFUSED;
    }
    if w.trading(p) {
        return res::OK;
    }
    let mut target = p;
    if on_merc != 0 && p.is_player() && w.expansion() {
        if ![ty::HPOT, ty::APOT, ty::WPOT]
            .iter()
            .any(|&t| w.is_type(item, t))
        {
            return res::OK;
        }
        // TODO(spec: inventory.md §7.17): no hireling is not written; read
        // as "nothing".
        let Some(merc) = w.hireling(p) else {
            return res::OK;
        };
        target = merc;
    }
    let slot = w.pos(Owner::item(item)).0;
    if w.use_item(p, target, item) {
        w.charge_update(p, item);
        w.targeting_reset(p);
        w.remove_used(p, item);
        w.belt_compact(p, slot as u8);
    }
    res::OK
}

// ------------------------------------------------------------------ 0x27

/// 0x27 UseItemAction `0x0054B280` (§7.18; body `0x00561ED0` is OQ14).
pub fn use_item_action<W: MoveWorld>(w: &mut W, p: Owner, target: Guid, used: Guid) -> u32 {
    for g in [target, used] {
        let r = w.check_owned(p, g);
        if r != 0 {
            return r;
        }
    }
    let (ok, out) = w.use_item_action(p, target, used);
    Outcome { ok, out }.result()
}

// ------------------------------------------------------------------ 0x28

/// 0x28 SocketItem `0x0054B650` → `0x00562660` (§7.19).
pub fn socket_item<W: MoveWorld>(
    w: &mut W,
    p: Owner,
    filler: Guid,
    target: Guid,
) -> Result<u32, MoveFatal> {
    let r = w.check_cursor_item(p, filler);
    if r != 0 {
        return Ok(r);
    }
    let r = w.check_stored_or_equipped(p, target);
    if r != 0 {
        return Ok(r);
    }
    if w.trading(p) {
        return Ok(res::OK);
    }
    w.targeting_reset(p);
    if !exists(w, filler) {
        return Ok(res::REFUSED);
    }
    if w.mode(filler) != mode::CURSOR || !exists(w, target) {
        return Ok(res::REFUSED);
    }
    if w.item_flags(target) & iflag::IDENTIFIED == 0 {
        return Ok(res::OK);
    }
    // TODO(spec: inventory.md §7.19 r2): the results of the mode, filler and
    // socket checks are not written; read as "nothing".
    if w.mode(target) > mode::EQUIPPED {
        return Ok(res::OK);
    }
    if !w.socket_filler(filler) || w.item_flags(filler) & iflag::IDENTIFIED == 0 {
        return Ok(res::OK);
    }
    if w.item_flags(target) & iflag::SOCKETED == 0
        || w.fillers(target).len() as i64 >= i64::from(w.sockets(target))
    {
        return Ok(res::OK);
    }
    if !w.link_into_item(target, filler) {
        return Err(MoveFatal::Link);
    }
    w.set_cursor(p, None);
    clear_uflags(w, filler, uflag::TARGETABLE);
    w.filler_linked(filler, target);
    w.set_mode(filler, mode::SOCKETED);
    w.runeword(p, target);
    add_iflags(w, target, iflag::CHANGED);
    clear_iflags(w, target, iflag::NOEQUIP);
    owner_refresh(w, p);
    w.update_list_add(p, target);
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x29

/// 0x29 ScrollToBook `0x0054B710` → `0x0055EF20` (§7.20).
pub fn scroll_to_book<W: MoveWorld>(
    w: &mut W,
    p: Owner,
    scroll: Guid,
    book: Guid,
) -> Result<u32, MoveFatal> {
    let r = w.check_ground_or_owned(p, scroll);
    if r != 0 {
        return Ok(r);
    }
    let r = w.check_stored(p, book);
    if r != 0 {
        return Ok(r);
    }
    if !exists(w, scroll) {
        return Ok(res::REFUSED);
    }
    let sm = w.mode(scroll);
    if (sm != mode::GROUND && sm != mode::CURSOR) || !w.is_type(scroll, ty::SCRO) {
        return Ok(res::REFUSED);
    }
    if !exists(w, book) {
        return Ok(res::REFUSED);
    }
    if w.mode(book) != mode::STORED || !w.is_type(book, ty::BOOK) {
        return Ok(res::REFUSED);
    }
    if w.spell(book) != w.spell(scroll) {
        return Err(MoveFatal::SpellMismatch);
    }
    let b = Owner::item(book);
    let q = w.stat(b, stat::QUANTITY);
    if q >= w.max_stack(book) {
        return Ok(res::OK);
    }
    if !w.consume_one(scroll) {
        w.remove_from_room(scroll);
        w.free_item(scroll);
        w.set_cursor(p, None);
    }
    w.set_stat(b, stat::QUANTITY, q.wrapping_add(1));
    w.send_item_stat(p, book, stat::QUANTITY);
    w.book_count_changed(p, 1);
    Ok(res::OK)
}

// ------------------------------------------------------------------ 0x50

/// 0x50 DropGold `0x0054C800` → `0x00535510` (§7.22).
pub fn drop_gold<W: MoveWorld>(w: &mut W, p: Owner, unit: Guid, amount: u32) -> u32 {
    if w.busy(p) && w.trading(p) {
        return res::REFUSED;
    }
    if unit != p.guid {
        return res::REFUSED;
    }
    let amount = amount as i32;
    let gold = w.stat(p, stat::GOLD);
    if amount < 0 || amount > gold || amount > gold_limit(w, p) {
        return res::REFUSED;
    }
    if amount == 0 {
        return res::OK;
    }
    let amount = amount.min(PILE_CAP);
    for pile in gold_piles(w, p, amount, MAX_PILES) {
        if !w.query_0044be50() {
            w.set_owner(pile, p);
        }
        let g = w.stat(p, stat::GOLD) - w.stat(Owner::item(pile), stat::GOLD);
        // Setting gold (`0x00530EA0`): a negative value stores 0.
        w.set_stat(p, stat::GOLD, g.max(0));
    }
    res::OK
}

// ------------------------------------------------------------------ 0x61

const MERC_A1: u32 = 0x10F;
const MERC_A2: u32 = 0x152;
const MERC_A3: u32 = 0x167;
const MERC_A5A: u32 = 0x230;
const MERC_A5B: u32 = 0x231;

/// 0x61 MercItem `0x0054D430`, expansion only (§7.23).
pub fn merc_item<W: MoveWorld>(w: &mut W, p: Owner, loc: u16) -> Result<u32, MoveFatal> {
    if !w.expansion() {
        return Ok(res::REFUSED);
    }
    if w.busy(p) && w.trading(p) {
        return Ok(res::REFUSED);
    }
    if !w.not_dead(p) || !w.alive(p) {
        return Ok(res::OK);
    }
    let Some(merc) = w.hireling(p) else {
        return Ok(res::OK);
    };
    if !w.alive(merc) || !w.owns_hireling(p, merc) {
        return Ok(res::OK);
    }
    if !w.has_inventory(p) {
        return Ok(res::REFUSED);
    }
    if let Some(c) = w.cursor(p) {
        if w.quest(c) == 0 {
            merc_give(w, p, Some(merc), c);
        }
        return Ok(res::OK);
    }
    if loc != 0 {
        return merc_take(w, p, merc, loc);
    }
    Ok(res::OK)
}

/// Take from the hireling `0x0054D130` (§7.23).
pub fn merc_take<W: MoveWorld>(
    w: &mut W,
    p: Owner,
    merc: Owner,
    loc: u16,
) -> Result<u32, MoveFatal> {
    if !valid_loc(u32::from(loc)) {
        return Ok(res::BAD);
    }
    let loc = loc as u8;
    let Some(it) = w
        .body_item(merc, loc)
        .filter(|&i| w.mode(i) == mode::EQUIPPED)
    else {
        return Ok(res::BAD);
    };
    if !w.unlink(merc, it) {
        return Err(MoveFatal::Unlink);
    }
    w.clear_body_slot(merc, loc);
    w.stat_refresh_unlink(merc, 0);
    mark(w, merc, it, cmd::UNEQUIP);
    // TODO(spec: inventory.md §7.23): a failed copy is not written; read as
    // "no cursor item".
    if let Some(copy) = w.copy_item(it) {
        w.give_cursor_item(p, copy);
    }
    add_iflags(w, it, iflag::COPIED);
    w.merc_after_take(merc);
    Ok(res::OK)
}

/// Give to the hireling `0x0054D230` (§7.23); returns its result (the
/// caller ignores it).
pub fn merc_give<W: MoveWorld>(w: &mut W, p: Owner, merc: Option<Owner>, c: Guid) -> u32 {
    if !w.expansion() {
        return res::REFUSED;
    }
    let f = w.item_flags(c);
    if f & iflag::IDENTIFIED == 0 || f & iflag::BROKEN != 0 {
        return 0;
    }
    if [ty::HPOT, ty::WPOT, ty::APOT]
        .iter()
        .any(|&t| w.is_type(c, t))
    {
        let target = merc.unwrap_or(p);
        w.use_item(p, target, c);
        w.consume_one(c);
        w.set_cursor(p, None);
        return 1;
    }
    let one_handed = !w.two_handed(c);
    let allowed = w.is_type(c, ty::TORS)
        || w.is_type(c, ty::HELM)
        || match merc.map(|m| w.unit_class(m)) {
            Some(MERC_A1) => w.is_type(c, ty::BOW),
            Some(MERC_A2) => w.is_type(c, ty::SPEA) || w.is_type(c, ty::POLE),
            Some(MERC_A3) => w.is_type(c, ty::SHIE) || (w.is_type(c, ty::SWOR) && one_handed),
            Some(MERC_A5A) => (w.is_type(c, ty::AXE) && one_handed) || w.is_type(c, ty::PHLM),
            Some(MERC_A5B) => w.is_type(c, ty::SWOR) || w.is_type(c, ty::PHLM),
            _ => false,
        };
    if let Some(m) = merc {
        if allowed && w.requirements(c, m, false) {
            w.equip_on_merc(m, c);
        }
    }
    w.merc_sound(p);
    0
}

// ------------------------------------------------------------------ 0x63

/// 0x63 ItemToBeltShift `0x0054D520` (§7.24).
pub fn item_to_belt_shift<W: MoveWorld>(w: &mut W, p: Owner, item: Guid) -> Result<u32, MoveFatal> {
    let r = w.check_stored(p, item);
    if r != 0 {
        return Ok(r);
    }
    if w.cursor(p).is_some() {
        w.resync(p);
        return Ok(res::BAD);
    }
    if !exists(w, item) || !w.beltable(item) {
        return Ok(res::BAD);
    }
    if w.page(item) != page::INVENTORY {
        return Ok(res::REFUSED);
    }
    let Some(slot) = w.belt_free_slot(p, item) else {
        return Ok(res::OK);
    };
    if !w.item_move_gate(p, Some(item)) {
        return Ok(res::OK);
    }
    if !w.has_inventory(p) {
        return Err(MoveFatal::NoInventory);
    }
    let pg = w.page(item);
    if pg != page::INVENTORY || w.mode(item) != mode::STORED {
        return Ok(res::BAD);
    }
    let (x, y) = w.pos(Owner::item(item));
    if !w.unlink(p, item) {
        return Err(MoveFatal::Unlink);
    }
    w.room_change_notice(item, x, y);
    w.set_stored_page(item, pg);
    w.set_page(item, page::NONE);
    send_item_page(w, p, item, 0, pg)?;
    w.set_mode(item, mode::CURSOR);
    clear_uflags(w, item, uflag::TARGETABLE);
    if w.belt_place(p, item, u32::from(slot)) {
        w.set_page(item, page::NONE);
        w.link_check(p, item, 2);
        w.set_mode(item, mode::BELT);
        send_to_belt(w, p, item)?;
    }
    owner_refresh(w, p);
    Ok(res::OK)
}
