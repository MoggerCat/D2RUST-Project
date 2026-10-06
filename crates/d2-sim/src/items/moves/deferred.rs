// Spec: specs/items/inventory.md
//! Deferred item messages (§6): marking (§6.1), the per-client dispatcher
//! `0x005973F0` driven by `items/item-actions.tsv` (§6.2), ground items
//! (§6.3) and the direct sends of §6.4. Message bytes per §11
//! ([`super::layouts`]).

use super::layouts;
use super::seams::MoveWorld;
use super::{iflag, mode, uflag, Guid, MoveFatal, Owner};

/// Which flags a row tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    /// Command flags (item data +0x14).
    Cmd,
    /// Item flags (item data +0x18).
    Item,
}

/// Recipients of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    /// Only the owner's client.
    Owner,
    /// Every client that processes the player.
    All,
    /// The owner, or any client when the item is in mode 1.
    OwnerOrMode1,
}

/// Extra condition of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    None,
    /// Item flag 0x100 clear.
    NotBroken,
    /// Mode in 0..2; neither 0x100 nor 0x200; the owner's client gets
    /// nothing while the item has item flag 0x40000.
    UpdateStats,
}

/// One row of `items/item-actions.tsv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemAction {
    pub order: u8,
    pub test: Test,
    pub flags: u32,
    pub to: To,
    pub cond: Cond,
    pub message: u8,
    /// 0x9C / 0x9D action byte, or the 0x7D flag value.
    pub action: u32,
    pub sender: u32,
    pub d2moo: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn row(
    order: u8,
    test: Test,
    flags: u32,
    to: To,
    cond: Cond,
    message: u8,
    action: u32,
    sender: u32,
    d2moo: &'static str,
) -> ItemAction {
    ItemAction {
        order,
        test,
        flags,
        to,
        cond,
        message,
        action,
        sender,
        d2moo,
    }
}

use Cond as C;
use Test as T;
use To as O;

/// The dispatcher's rows in test order (`items/item-actions.tsv`).
pub const ITEM_ACTIONS: [ItemAction; 20] = [
    row(
        1,
        T::Cmd,
        0x40,
        O::Owner,
        C::None,
        0x9C,
        0x01,
        0x0053EC20,
        "GroundToCursor",
    ),
    row(
        2,
        T::Cmd,
        0x80000,
        O::Owner,
        C::None,
        0x9D,
        0x16,
        0x0053D4B0,
        "Unknown0x16",
    ),
    row(
        3,
        T::Cmd,
        0x2 | 0x80,
        O::Owner,
        C::None,
        0x9C,
        0x04,
        0x0053ED50,
        "PutInContainer",
    ),
    row(
        4,
        T::Cmd,
        0x4,
        O::Owner,
        C::None,
        0x9D,
        0x05,
        0x0053D010,
        "RemoveFromContainer",
    ),
    row(
        5,
        T::Cmd,
        0x8 | 0x200,
        O::All,
        C::None,
        0x9D,
        0x06,
        0x0053D090,
        "Equip",
    ),
    row(
        6,
        T::Cmd,
        0x10000,
        O::All,
        C::None,
        0x9D,
        0x07,
        0x0053D0B0,
        "IndirectlySwapBodyItem",
    ),
    row(
        7,
        T::Cmd,
        0x10,
        O::All,
        C::None,
        0x9D,
        0x08,
        0x0053D0D0,
        "Unequip",
    ),
    row(
        8,
        T::Cmd,
        0x20,
        O::All,
        C::None,
        0x9D,
        0x09,
        0x0053D0F0,
        "SwapBodyItem",
    ),
    row(
        9,
        T::Cmd,
        0x100,
        O::Owner,
        C::None,
        0x9C,
        0x0A,
        0x0053EDB0,
        "AddQuantity",
    ),
    row(
        10,
        T::Cmd,
        0x40000,
        O::Owner,
        C::None,
        0x9C,
        0x0D,
        0x0053EE10,
        "SwapInContainer",
    ),
    row(
        11,
        T::Cmd,
        0x400 | 0x2000,
        O::Owner,
        C::None,
        0x9C,
        0x0E,
        0x0053EE70,
        "PutInBelt",
    ),
    row(
        12,
        T::Cmd,
        0x800,
        O::Owner,
        C::None,
        0x9C,
        0x0F,
        0x0053EED0,
        "RemoveFromBelt",
    ),
    row(
        13,
        T::Cmd,
        0x1000,
        O::Owner,
        C::None,
        0x9C,
        0x10,
        0x0053EEF0,
        "SwapInBelt",
    ),
    row(
        14,
        T::Cmd,
        0x100000,
        O::Owner,
        C::None,
        0x9C,
        0x12,
        0x0053EF10,
        "ToCursor",
    ),
    row(
        15,
        T::Cmd,
        0x200000,
        O::All,
        C::None,
        0x9D,
        0x17,
        0x0053D110,
        "WeaponSwitch",
    ),
    row(
        16,
        T::Cmd,
        0x4000,
        O::All,
        C::None,
        0x9D,
        0x11,
        0x0053D2C0,
        "AutoUnequip",
    ),
    row(
        17,
        T::Cmd,
        0x8000,
        O::All,
        C::None,
        0x9D,
        0x14,
        0x0053D350,
        "Unknown0x14",
    ),
    row(
        18,
        T::Item,
        0x100,
        O::OwnerOrMode1,
        C::None,
        0x7D,
        0x100,
        0x0053D440,
        "broken",
    ),
    row(
        19,
        T::Item,
        0x200,
        O::OwnerOrMode1,
        C::NotBroken,
        0x7D,
        0x200,
        0x0053D440,
        "repaired",
    ),
    row(
        20,
        T::Item,
        0x1,
        O::OwnerOrMode1,
        C::UpdateStats,
        0x9D,
        0x15,
        0x0053D420,
        "UpdateStats",
    ),
];

/// Owner refresh `0x00621000(unit, 1)` (§6.1 rule 1): queue the unit for
/// update, set unit +0xC8 bit 0 and, for players, bit 1.
pub fn owner_refresh<W: MoveWorld>(w: &mut W, u: Owner) {
    w.queue_update(u);
    let bits = if u.is_player() { 0x3 } else { 0x1 };
    let v = w.update_bits(u);
    w.set_update_bits(u, v | bits);
}

/// Marking (§6.1 rule 1): command flags, update list, owner refresh.
pub fn mark<W: MoveWorld>(w: &mut W, owner: Owner, item: Guid, cmd: u32) {
    let v = w.cmd_flags(item);
    w.set_cmd_flags(item, v | cmd);
    w.update_list_add(owner, item);
    owner_refresh(w, owner);
}

/// Category byte (§11, `0x00623D60`).
pub fn category<W: MoveWorld>(w: &W, item: Guid) -> u8 {
    let c = w.component(item);
    let loc = w.body_loc(item);
    if loc != 4 && loc != 5 {
        return c;
    }
    let Some(o) = w.item_owner(item) else {
        return c;
    };
    let hand = |l| {
        w.body_item(o, l)
            .filter(|&h| w.item_flags(h) & iflag::BROKEN == 0 && !w.two_handed(h))
    };
    let class = w.unit_class(o);
    let dual = match o.ty {
        Owner::PLAYER => class == 4 || class == 6,
        Owner::MONSTER => class == 0x1A1 || class == 0x1A2,
        _ => false,
    };
    if hand(4).is_some() && hand(5).is_some() && dual && w.weapon_in_use(o) != Some(item) {
        6
    } else {
        c
    }
}

/// Builds a 0x9C / 0x9D for `item` and, for an item with sockets, one 0x9D
/// action 0x13 per filler after it (§11 "Bit stream").
fn item_message<W: MoveWorld>(
    w: &W,
    message: u8,
    action: u8,
    owner: Owner,
    item: Guid,
    flags: u32,
    page: u8,
) -> Result<Vec<Vec<u8>>, MoveFatal> {
    let bits = w.item_bits(item, flags, page);
    let cat = category(w, item);
    let first = if message == 0x9C {
        layouts::item_world(action, cat, item, &bits)?
    } else {
        layouts::item_owned(action, cat, item, owner.ty, owner.guid, &bits)?
    };
    let mut out = vec![first];
    // TODO(spec: inventory.md §11): the owner fields of a filler's 0x9D
    // action 0x13 and the flag argument of its bit stream are not written.
    for f in w.fillers(item) {
        let fo = w.filler_owner(item);
        let fb = w.item_bits(f, 0, w.page(f));
        out.push(layouts::item_owned(
            0x13,
            category(w, f),
            f,
            fo.ty,
            fo.guid,
            &fb,
        )?);
    }
    Ok(out)
}

/// The dispatcher `0x005973F0` (§6.2) for one item of `inv`'s update list,
/// as seen by the client of `client` (a player). Returns the messages in
/// send order.
pub fn dispatch<W: MoveWorld>(
    w: &mut W,
    client: Guid,
    inv: Owner,
    item: Guid,
) -> Result<Vec<Vec<u8>>, MoveFatal> {
    let client = Owner::player(client);
    let mut out = w.store_messages(client, item);
    let is_owner = inv == client;
    let cmdf = w.cmd_flags(item);
    let itf = w.item_flags(item);
    let m = w.mode(item);
    for r in &ITEM_ACTIONS {
        let v = match r.test {
            Test::Cmd => cmdf,
            Test::Item => itf,
        };
        if v & r.flags == 0 {
            continue;
        }
        let cond = match r.cond {
            Cond::None => true,
            Cond::NotBroken => itf & iflag::BROKEN == 0,
            Cond::UpdateStats => {
                m <= mode::BELT
                    && itf & (iflag::BROKEN | iflag::REPAIRED) == 0
                    && !(is_owner && itf & iflag::NO_UPDATE != 0)
            }
        };
        if !cond {
            continue;
        }
        // TODO(spec: inventory.md §6.2): a row whose flags match but whose
        // `to` excludes this client is read as ending the walk (nothing
        // sent, later rows skipped).
        let to = match r.to {
            To::Owner => is_owner,
            To::All => true,
            To::OwnerOrMode1 => is_owner || m == mode::EQUIPPED,
        };
        if to {
            if r.message == 0x7D {
                out.push(layouts::item_state(
                    inv.ty,
                    inv.guid,
                    item,
                    r.action,
                    itf & r.action,
                ));
            } else {
                if r.order == 1 {
                    // 0x9C action 1 first sets the item's x, y to 0 (§8.2).
                    w.set_pos(Owner::item(item), 0, 0);
                }
                // TODO(spec: inventory.md §11): the sender's flag argument
                // for the dispatcher's sends is not written; 0 is used.
                let page = w.page(item);
                out.extend(item_message(
                    w,
                    r.message,
                    r.action as u8,
                    inv,
                    item,
                    0,
                    page,
                )?);
            }
        }
        break;
    }
    Ok(out)
}

/// Player unit update `0x00580860` (§6.1 rules 2–3) for the client of
/// `client`: when `player` has +0xC8 bit 0, the update-list pass
/// `0x00597890`, then S→C 0x47 and 0x48 for the player.
pub fn player_update<W: MoveWorld>(
    w: &mut W,
    client: Guid,
    player: Guid,
) -> Result<Vec<Vec<u8>>, MoveFatal> {
    let p = Owner::player(player);
    let mut out = Vec::new();
    if w.update_bits(p) & 1 == 0 {
        return Ok(out);
    }
    for g in w.update_list(p) {
        if !w.unit_exists(Owner::item(g)) {
            continue;
        }
        out.extend(dispatch(w, client, p, g)?);
        let io = Owner::item(g);
        if w.update_bits(io) & 1 != 0 {
            for h in w.update_list(io) {
                out.extend(dispatch(w, client, io, h)?);
            }
        }
    }
    w.hireling_owner_pass(p);
    out.push(layouts::relator1(Owner::PLAYER, player));
    out.push(layouts::relator2(Owner::PLAYER, 0, player));
    // TODO(spec: inventory.md §6.1 rule 4, OQ9): the per-item reset of
    // command flags and the freeing of update lists after the pass.
    Ok(out)
}

/// Ground item update (§6.3, `0x0055BED0`): a mode-3 item without unit
/// flag 0x10 sends 0x9C action 2 (unit flag 0x1000) or 3. The caller is
/// the item unit update (`unit-order.md` §6).
pub fn ground_update<W: MoveWorld>(w: &W, item: Guid) -> Result<Option<Vec<u8>>, MoveFatal> {
    let u = Owner::item(item);
    if w.mode(item) != mode::GROUND || w.unit_flags(u) & uflag::NO_GROUND_MSG != 0 {
        return Ok(None);
    }
    let action = if w.unit_flags(u) & uflag::DROPPED != 0 {
        2
    } else {
        3
    };
    let bits = w.item_bits(item, 0, w.page(item));
    Ok(Some(layouts::item_world(
        action,
        category(w, item),
        item,
        &bits,
    )?))
}

/// Direct 0x9D action 5 (`0x0053D010`, §6.4): the page shown as `shown`
/// (the stored page; 3 for the cube spill), item flags OR-ed with
/// `flags`; queued to the owner's client now.
pub fn send_item_page<W: MoveWorld>(
    w: &mut W,
    player: Owner,
    item: Guid,
    flags: u32,
    shown: u8,
) -> Result<(), MoveFatal> {
    for m in item_message(w, 0x9D, 0x05, player, item, flags, shown)? {
        w.send(player, m);
    }
    Ok(())
}

/// Direct 0x9C action 0xE (`0x0053EE70`, §6.4), queued now.
pub fn send_to_belt<W: MoveWorld>(w: &mut W, player: Owner, item: Guid) -> Result<(), MoveFatal> {
    let page = w.page(item);
    for m in item_message(w, 0x9C, 0x0E, player, item, 0, page)? {
        w.send(player, m);
    }
    Ok(())
}
