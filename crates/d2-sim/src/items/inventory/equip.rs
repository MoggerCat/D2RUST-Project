// Spec: specs/items/inventory.md §4
//! Equipping: body-location compatibility (§4.1), requirements (§4.2), the
//! equip check (§4.3), hands compatible (§4.4), the stack test (§4.5),
//! equip from the cursor (§4.6) and auto-equip on pickup (§4.7).

use super::{
    body, cmd, grid::place_at_body, iflag, mode, page, stat, targeting_reset, ty, InvTables,
    InvWorld, Inventory, UnitKind, NO_GUID,
};
use crate::units::UnitId;

/// Itemtypes `class` "none" (§4.2 step 6).
pub const CLASS_NONE: u8 = 7;
/// Barbarian / assassin player classes (§4.2, §4.4).
pub const BARBARIAN: u8 = 4;
pub const ASSASSIN: u8 = 6;
/// Act 5 hireling monster classes (§4.2 step 6).
pub const ACT5_HIRELINGS: [u32; 2] = [0x230, 0x231];
/// Monster classes that dual-wield `h2h` (§4.4 step 6).
pub const H2H_MONSTERS: [u32; 2] = [0x1A1, 0x1A2];
/// Monster classes that dual-wield any weapons (§4.4 step 6).
pub const DUAL_MONSTERS: [u32; 3] = [0x21C, 0x21D, 0x21E];

/// Equip-check results (§4.3).
pub mod res {
    pub const NO: u8 = 0;
    pub const FREE: u8 = 1;
    pub const OTHER_HAND_BLOCKS: u8 = 2;
    pub const REMOVABLE: u8 = 3;
    pub const OTHER_HAND_TWO_HANDED: u8 = 4;
    pub const SWAP: u8 = 5;
    pub const STACK: u8 = 6;
    pub const SWAP_OTHER_TO_PAGE: u8 = 7;
}

/// Body-location compatibility (`0x0062ED50`, §4.1): `loc` equals the
/// item type's `bodyloc1` or `bodyloc2` (`0x0062EA80`), or is 11 / 12 when
/// either is 4 or 5.
pub fn body_location_allowed(t: &InvTables, record: usize, loc: u8) -> bool {
    let Some(r) = t.itype_of(record) else {
        return false;
    };
    let (a, b) = (r.bodyloc1, r.bodyloc2);
    if loc == a || loc == b {
        return true;
    }
    let hand = |l: u8| l == body::RIGHT_HAND || l == body::LEFT_HAND;
    matches!(loc, body::SWAP_RIGHT | body::SWAP_LEFT) && (hand(a) || hand(b))
}

fn stat_ok<W: InvWorld + ?Sized>(
    w: &W,
    item: UnitId,
    unit: UnitId,
    equipping: bool,
    id: u16,
    req: i32,
) -> bool {
    let v = w.unit_stat(unit, id);
    if v < 1 || v < req {
        return false;
    }
    if equipping && w.item_active_on(item, unit) {
        let v = v - w.own_contribution(item, unit, id);
        if v < 1 || v < req {
            return false;
        }
    }
    true
}

/// Requirements (`0x0062EAF0`, §4.2) of `item` for `unit`.
pub fn requirements_met<W: InvWorld + ?Sized>(
    w: &W,
    t: &InvTables,
    item: Option<UnitId>,
    unit: UnitId,
    equipping: bool,
) -> bool {
    // Step 1.
    let Some(item) = item else { return false };
    let Some(d) = w.item(item).copied() else {
        return false;
    };
    let Some(rec) = t.item(d.record) else {
        return false;
    };
    // Step 2.
    let (reqstr, reqdex) = (i32::from(rec.reqstr), i32::from(rec.reqdex));
    let p = w.req_percent(item);
    let (mut bonus_str, mut bonus_dex) = (0, 0);
    if p != 0 {
        bonus_str = w.percent_of(reqstr, p);
        bonus_dex = w.percent_of(reqdex, p);
    }
    if d.flags & iflag::ETHEREAL != 0 {
        bonus_str -= 10;
        bonus_dex -= 10;
    }
    // Steps 3–5.
    if !stat_ok(w, item, unit, equipping, stat::STRENGTH, reqstr + bonus_str) {
        return false;
    }
    if !stat_ok(
        w,
        item,
        unit,
        equipping,
        stat::DEXTERITY,
        reqdex + bonus_dex,
    ) {
        return false;
    }
    if w.unit_stat(unit, stat::LEVEL) < w.level_requirement(item, unit) {
        return false;
    }
    // Step 6.
    if d.flags & iflag::IDENTIFIED == 0 {
        return false;
    }
    if rec.type_ == ty::BOOK && w.item_stat(item, stat::QUANTITY) <= 0 {
        return false;
    }
    let class = t.itype_of(d.record).map_or(CLASS_NONE, |r| r.class);
    if class == CLASS_NONE {
        return true;
    }
    match w.unit_kind(unit) {
        Some(UnitKind::Player { class: c }) => c == class,
        Some(UnitKind::Monster { class: m }) => ACT5_HIRELINGS.contains(&m) && class == BARBARIAN,
        _ => false,
    }
}

fn is_type<W: InvWorld + ?Sized>(w: &W, t: &InvTables, item: UnitId, ty: i16) -> bool {
    w.item(item).is_some_and(|d| t.is_type(d.record, ty))
}

/// Hands compatible (`0x0063DBC0`, §4.4) of `a` and `b` for `unit`.
pub fn hands_compatible<W: InvWorld + ?Sized>(
    w: &W,
    t: &InvTables,
    unit: UnitId,
    a: Option<UnitId>,
    b: Option<UnitId>,
) -> bool {
    // Step 1.
    let (Some(a), Some(b)) = (a, b) else {
        return true;
    };
    // Step 2.
    let ammo = |x: UnitId, y: UnitId| w.ammo_type(x).is_some_and(|am| is_type(w, t, y, am));
    if ammo(a, b) || ammo(b, a) {
        return true;
    }
    // Step 3.
    if w.quiver_type(t, a) || w.quiver_type(t, b) {
        return false;
    }
    // Step 4.
    for x in [a, b] {
        if w.two_handed(x) && !w.one_or_two_handed(unit, x) {
            return false;
        }
    }
    // Step 5.
    let (wa, wb) = (is_type(w, t, a, ty::WEAP), is_type(w, t, b, ty::WEAP));
    if !(wa && wb) {
        return wa || wb;
    }
    // Step 6.
    let both_h2h = is_type(w, t, a, ty::H2H) && is_type(w, t, b, ty::H2H);
    match w.unit_kind(unit) {
        None => false,
        Some(UnitKind::Player { class }) => match class {
            BARBARIAN => true,
            ASSASSIN => both_h2h,
            _ => false,
        },
        Some(UnitKind::Monster { class }) => {
            (H2H_MONSTERS.contains(&class) && both_h2h) || DUAL_MONSTERS.contains(&class)
        }
        // TODO(spec: §4.4 step 6 names players and monsters only)
        Some(_) => false,
    }
}

/// Stack test (`0x0062C850`, §4.5).
pub fn stack_test<W: InvWorld + ?Sized>(w: &W, t: &InvTables, a: UnitId, b: UnitId) -> bool {
    let (Some(da), Some(db)) = (w.item(a), w.item(b)) else {
        return false;
    };
    let stackable = |r: usize| t.item(r).is_some_and(|r| r.stackable != 0);
    const DAMAGE: [u16; 6] = [
        stat::MINDAMAGE,
        stat::MAXDAMAGE,
        stat::SECONDARY_MINDAMAGE,
        stat::SECONDARY_MAXDAMAGE,
        stat::THROW_MINDAMAGE,
        stat::THROW_MAXDAMAGE,
    ];
    da.record == db.record
        && w.quality(a) == w.quality(b)
        && w.stack_file_index(a) == w.stack_file_index(b)
        && stackable(da.record)
        && w.stack_value(a) == w.stack_value(b)
        && w.stack_quality_ok(a)
        && w.stack_quality_ok(b)
        && DAMAGE
            .iter()
            .all(|&s| w.item_stat(a, s) == w.item_stat(b, s))
        && !w.has_sockets(a)
        && !w.has_sockets(b)
}

/// The other hand of a hand location (§4.3 step 4).
pub fn other_hand(loc: u8) -> Option<u8> {
    match loc {
        body::RIGHT_HAND => Some(body::LEFT_HAND),
        body::LEFT_HAND => Some(body::RIGHT_HAND),
        body::SWAP_RIGHT => Some(body::SWAP_LEFT),
        body::SWAP_LEFT => Some(body::SWAP_RIGHT),
        _ => None,
    }
}

/// Equip check (`0x0063DE60`, §4.3): may `item` (none: "may `loc` be
/// emptied") go to `loc` of `unit`, whose inventory is `inv`. Returns a
/// [`res`] code.
pub fn equip_check<W: InvWorld + ?Sized>(
    inv: &Inventory,
    w: &W,
    t: &InvTables,
    unit: UnitId,
    loc: u8,
    item: Option<UnitId>,
    skip: bool,
) -> u8 {
    // Step 1.
    if let Some(n) = item {
        let Some(d) = w.item(n) else { return res::NO };
        if !body_location_allowed(t, d.record, loc) {
            return res::NO;
        }
        if !skip && !requirements_met(w, t, Some(n), unit, true) {
            return res::NO;
        }
    }
    // Step 2.
    if loc == body::NONE {
        return res::NO;
    }
    let target = inv.body_item(loc);
    // Step 3.
    let Some(o) = other_hand(loc) else {
        return match (item, target) {
            (Some(_), Some(_)) => res::SWAP,
            (Some(_), None) => res::FREE,
            (None, Some(_)) => res::REMOVABLE,
            (None, None) => res::NO,
        };
    };
    // Step 4.
    let other = inv.body_item(o);
    match (item, target, other) {
        (None, Some(_), _) => res::REMOVABLE,
        (None, None, Some(x)) if w.two_handed(x) => res::OTHER_HAND_TWO_HANDED,
        (None, None, _) => res::NO,
        (Some(n), Some(tt), x) => {
            if stack_test(w, t, tt, n) {
                res::STACK
            } else if hands_compatible(w, t, unit, Some(n), x) {
                res::SWAP
            } else if x.is_some_and(|x| w.fits_free_page0(inv, x)) {
                res::SWAP_OTHER_TO_PAGE
            } else {
                res::NO
            }
        }
        (Some(n), None, Some(x)) => {
            if hands_compatible(w, t, unit, Some(n), Some(x)) {
                res::FREE
            } else {
                res::OTHER_HAND_BLOCKS
            }
        }
        (Some(_), None, None) => res::FREE,
    }
}

/// A helper's result and its refusal flag "out" (§7: result 0 with out ≠ 0
/// → handler 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquipOutcome {
    pub ok: bool,
    pub out: bool,
}

impl EquipOutcome {
    const REFUSED: EquipOutcome = EquipOutcome {
        ok: false,
        out: true,
    };
    const NO: EquipOutcome = EquipOutcome {
        ok: false,
        out: false,
    };
    const OK: EquipOutcome = EquipOutcome {
        ok: true,
        out: false,
    };
}

/// §4.6 step 5: put `item` at `loc` of the inventory's owner and mark it
/// with command flag `flag` (0x8 for §4.6; 0x10000 for 0x1B, §7.6).
/// False when the put or the link check fails (out := 1).
pub fn equip_put<W: InvWorld + ?Sized>(
    inv: &mut Inventory,
    w: &mut W,
    item: UnitId,
    loc: u8,
    flag: u32,
) -> bool {
    let unit = inv.owner;
    let swap = matches!(loc, body::SWAP_RIGHT | body::SWAP_LEFT);
    if !place_at_body(inv, w, item, loc) {
        return false;
    }
    if !w.link_check(unit, item, if swap { 4 } else { 3 }) {
        return false;
    }
    if let Some(d) = w.item_mut(item) {
        d.body_loc = loc;
    }
    if !swap {
        w.stat_link(unit, item);
        w.stat_refresh(unit);
    }
    inv.set_cursor(None);
    w.clear_targetable(item);
    let mut guid = NO_GUID;
    if let Some(d) = w.item_mut(item) {
        d.mode = mode::EQUIPPED;
        d.page = page::NONE;
        d.cmd_flags |= flag;
        d.flags |= iflag::CHANGED;
        d.flags &= !iflag::F4000;
        guid = d.guid;
    }
    inv.push_update(guid);
    w.owner_refresh(unit);
    w.weapon_bookkeeping(unit, item);
    w.inventory_pass(unit);
    true
}

/// Equip from the cursor (`0x005606B0`, §4.6): the cursor item `item` of
/// the inventory's owner to body location `loc`.
pub fn equip_from_cursor<W: InvWorld + ?Sized>(
    inv: &mut Inventory,
    w: &mut W,
    t: &InvTables,
    item: Option<UnitId>,
    loc: u8,
    skip: bool,
) -> EquipOutcome {
    let unit = inv.owner;
    // Step 1.
    targeting_reset(inv, w);
    let Some(item) = item else {
        return EquipOutcome::REFUSED;
    };
    let Some(d) = w.item(item).copied() else {
        return EquipOutcome::NO;
    };
    if d.mode != mode::CURSOR {
        return EquipOutcome::REFUSED;
    }
    // Step 2.
    if equip_check(inv, w, t, unit, loc, Some(item), skip) != res::FREE {
        return EquipOutcome::NO;
    }
    // Step 3.
    if !skip && !requirements_met(w, t, Some(item), unit, false) {
        return EquipOutcome::REFUSED;
    }
    // Step 4.
    if !matches!(loc, body::SWAP_RIGHT | body::SWAP_LEFT) && inv.weapon_guid != NO_GUID {
        w.weapon_in_use_update(unit);
    }
    // Step 5.
    if !equip_put(inv, w, item, loc, cmd::EQUIP) {
        return EquipOutcome::REFUSED;
    }
    EquipOutcome::OK
}

/// Auto-equip on pickup (`0x0055D710`, §4.7): the body location `item`
/// goes to on the inventory owner's body, none = no.
pub fn auto_equip_location<W: InvWorld + ?Sized>(
    inv: &Inventory,
    w: &W,
    t: &InvTables,
    item: UnitId,
    skip: bool,
) -> Option<u8> {
    let unit = inv.owner;
    let d = *w.item(item)?;
    let rec = t.item(d.record)?;
    let ityp = t.itype_of(d.record)?;
    // Step 1.
    if !skip && !requirements_met(w, t, Some(item), unit, false) {
        return None;
    }
    if ityp.body == 0 || !w.has_allowed_location(item) {
        return None;
    }
    if d.flags & iflag::IDENTIFIED == 0 {
        return None;
    }
    // `0x0062A4E0`: not broken and no item flag 0x4000.
    if d.flags & (iflag::BROKEN | iflag::F4000) != 0 {
        return None;
    }
    // TODO(spec: whether "type ≠ 38" is the primary type or the equivalence test; primary used)
    if rec.type_ == ty::TPOT {
        return None;
    }
    // Step 2.
    if w.quiver_kind(item) {
        // TODO(spec: which hand "an equipped hand weapon" means; both hands checked, right first)
        let fed = [body::RIGHT_HAND, body::LEFT_HAND].iter().any(|&l| {
            inv.body_item(l)
                .and_then(|h| w.ammo_type(h))
                .is_some_and(|am| t.is_type(d.record, am))
        });
        if !fed {
            return None;
        }
    }
    // Step 3.
    let (l1, l2) = (ityp.bodyloc1, ityp.bodyloc2);
    if l1 == l2 {
        return inv.body_item(l1).is_none().then_some(l1);
    }
    // Step 4.
    match (inv.body_item(l1).is_some(), inv.body_item(l2).is_some()) {
        (false, false) => Some(l1),
        (false, true) => w.auto_equip_allows(unit, item, l1).then_some(l1),
        (true, false) => w.auto_equip_allows(unit, item, l2).then_some(l2),
        (true, true) => None,
    }
}
