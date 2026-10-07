// Spec: specs/world/hirelings.md §3.2, §6 r1, §6 r4, §8 r2, §9 r3, §9 r4, §9 r5, §9 r6, §9 r7, §9 r8, §9 r9, §10 r1, §10 r2, §10 r3, §10 r4, §10 r7
//! The hireling's life: init (§3.2), following the player (§6 rule 1,
//! type 7), the classic act change (§6 rule 4, type 7), death (§8 rule
//! 2), revive (§9 rules 3–9, `0x00579AA0`) and restoring from a save
//! (§10 rules 1–4 and 7).
//!
//! The C→S 0x62 part of a resurrect (§9 rule 2: cost, flag, mode, life,
//! the 0x9B / 0x2A replies) is `npc.md` §7.4
//! ([`crate::world::npc::hire`]), which then calls [`revive`].

use super::pets::{self, broadcast_remove, merc_dead_message};
use super::{
    flags, level, resurrect_cost, stat, state, HirelingError, HirelingState, HirelingTables,
    HirelingWorld, Offer, Slot, NEW_HIRE, UNIT_PLAYER,
};
use crate::units::UnitId;

/// Unit mode: neutral (alive).
pub const MODE_NEUTRAL: u8 = 1;
/// Unit mode: dead (corpse).
pub const MODE_DEAD: u8 = 12;
/// `owner` type argument of `0x0058F030(old, −1, 1)` (§3.2 rule 4).
const CLEAR_OWNER_TYPE: u8 = 1;

/// §3.2 rule 4 / §9 rule 3: an old hireling unit leaves: owner cleared
/// (`0x0058F030(old, −1, 1)`), pet node removed (§5 rule 5, no kill),
/// room removal queued and the unit freed with its items.
fn discard<W: HirelingWorld>(
    w: &mut W,
    st: &mut HirelingState,
    player: UnitId,
    old: UnitId,
) -> Result<(), HirelingError> {
    let guid = w.guid(old);
    w.set_owner(old, u32::MAX, CLEAR_OWNER_TYPE);
    pets::remove(w, st, player, guid, false)?;
    w.free_unit(old);
    Ok(())
}

/// §3.2 (`0x00573270(game, player, merc, saved_id, slot, restore)`).
/// `saved_id`: the saved `Id`, or [`NEW_HIRE`]. Returns the offer of
/// rule 5, `None` when nothing happened (player not type 0) or there was
/// no offer (rule 5: the unit stays without owner or pet node).
#[allow(clippy::too_many_arguments)]
pub fn init<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &mut HirelingState,
    player: UnitId,
    merc: UnitId,
    saved_id: u16,
    slot: Slot,
    restore: bool,
) -> Result<Option<Offer>, HirelingError> {
    if w.unit_type(player) != UNIT_PLAYER {
        return Ok(None);
    }
    // Rule 1.
    let f = w.flags(merc);
    w.set_flags(merc, f | flags::INIT);
    // Rule 2: stat 172 := 2 in the stat list of state 105.
    w.set_state_stat(merc, state::ALIGNMENT, stat::ALIGNMENT, 2);
    // Rule 3.
    if !restore {
        w.join_team(merc, player);
    }
    // Rule 4: replace the first hireling node (living or dead) whose
    // unit exists.
    if let Some(old) = pets::any(w, st, player) {
        w.send(player, &merc_dead_message(0xFFFF, 0));
        discard(w, st, player, old)?;
    }
    // Rule 5.
    let act0 = t.rows.act_of_name(slot.name);
    let player_level = w.stat(player, stat::LEVEL);
    let Some(offer) = t.rows.offer(
        w.expansion(),
        player_level,
        slot.seed,
        act0,
        u32::from(w.difficulty()),
    ) else {
        return Ok(None);
    };
    // Rule 6. A `false` (maximum 0, unit dismissed) does not stop the
    // init in the spec text.
    // TODO(hirelings.md §3.2 r6): whether `0x00573270` reads the add's
    // result is not stated; the init continues.
    pets::add(w, t, st, player, merc, slot.seed, slot.name, offer.id)?;
    // Rule 7.
    let f = w.flags(merc);
    w.set_flags(merc, f | flags::OWNED);
    // Rule 8.
    let pg = w.guid(player);
    w.set_owner(merc, pg, UNIT_PLAYER);
    // Rule 9.
    if saved_id == NEW_HIRE {
        if t.rows
            .row_at(w.expansion(), offer.id, offer.level)
            .is_none()
        {
            return Ok(Some(offer));
        }
        w.set_base_stat(merc, stat::EXPERIENCE, offer.experience);
        level::apply_level(w, t, st, player, Some(merc), offer.level);
        level::send_stats(w, st, player)?;
    }
    // Rules 10, 11.
    w.hireling_ai(merc);
    Ok(Some(offer))
}

/// §8 rule 2 (`0x005751A0(game, owner, merc)`), hireling list part: the
/// node of the merc's GUID gets bit 0 (dead), the owner's client gets
/// 0x9B (name id, resurrect cost at the current level) and every client
/// 0x7A remove with only the GUID. `false`: the GUID is in no hireling
/// node.
///
/// TODO(hirelings.md §8 r2): the pet type lookup (`0x00574A20`) and the
/// type ≠ 7 node removal are `sim/pets.md`'s (`d2-sim::player::pets`).
/// The trigger (§8 rule 1: the `0x00457490` test, open question 8) is
/// the death path's.
pub fn death<W: HirelingWorld>(
    w: &mut W,
    st: &mut HirelingState,
    owner: UnitId,
    merc: UnitId,
) -> bool {
    let guid = w.guid(merc);
    let Some(node) = st
        .lists
        .get_mut(&owner)
        .and_then(|l| l.nodes.iter_mut().find(|n| n.guid == guid))
    else {
        return false;
    };
    node.dead = true;
    let name = node.name;
    let cost = resurrect_cost(w.stat(merc, stat::LEVEL));
    w.send(owner, &merc_dead_message(name, cost));
    broadcast_remove(w, guid);
    true
}

/// §9 rules 3–9 (`0x00579AA0(game, player, merc)`).
pub fn revive<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &mut HirelingState,
    player: UnitId,
    merc: UnitId,
) -> Result<(), HirelingError> {
    // Rule 3. Edge case 5: when the living hireling is `merc` itself the
    // original frees it here; `npc.md` §7.4 refuses that case (code 9).
    if let Some(old) = pets::living(w, st, player) {
        discard(w, st, player, old)?;
    }
    // Rule 4.
    w.set_mode(merc, MODE_NEUTRAL);
    let max = w.max_life(merc);
    w.set_base_stat(merc, stat::HITPOINTS, max);
    let guid = w.guid(merc);
    pets::mark_living(w, st, player, guid);
    w.join_team(merc, player);
    // Rule 5.
    for s in [state::FREEZE, state::SHATTER, state::RECYCLED] {
        w.remove_state(merc, s);
    }
    // Rule 6.
    let f = w.flags(merc);
    w.set_flags(merc, f | flags::REVIVE);
    let f2 = w.flags2(merc);
    w.set_flags2(merc, f2 & !flags::REVIVE_CLEAR2);
    // Rule 7.
    w.reapply_item_stats(merc);
    // Rule 8.
    follow(w, t, st, player);
    // Rule 9.
    level::send_stats(w, st, player)
}

/// §6 rule 1 (`0x005754B0(game, player, x, y)`), pet type 7: `warp` →
/// every living node's unit moves to the player (§6 rule 5, flags 2 |=
/// 0x10000); else `range` → see the TODO; else every node is freed
/// (`0x00574C60`).
///
/// TODO(sim/pets.md): pet types 1 … count − 1 other than 7 are
/// `d2-sim::player::pets`'s; the caller runs them in row order.
pub fn follow<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &mut HirelingState,
    player: UnitId,
) {
    if t.pet_flags & HirelingTables::WARP != 0 {
        let guids: Vec<u32> = st
            .list(player)
            .map(|l| l.nodes.iter().filter(|n| !n.dead).map(|n| n.guid).collect())
            .unwrap_or_default();
        for guid in guids {
            if let Some(unit) = w.monster_by_guid(guid) {
                w.warp_to(unit, player);
                let f2 = w.flags2(unit);
                w.set_flags2(unit, f2 | flags::WARP2);
            }
        }
    } else if t.pet_flags & HirelingTables::RANGE != 0 {
        // TODO(hirelings.md §6 r1): living pets farther than 1600
        // (`0x006492A0` distance) are removed with kill (§5 rule 5); the
        // distance is not in the seam. Unused by 1.14d row 7 (range 0).
    } else if let Some(list) = st.lists.get_mut(&player) {
        list.nodes.clear();
    }
}

/// §6 rule 4 (`0x00575BC0`), t = 7 (`0x00574570` with keep = keep_dead =
/// 1): for each node whose unit exists: broadcast 0x7A remove, queue the
/// room removal; a living node becomes dead, the death event runs when
/// the unit is in a room and the owner gets 0x9B (name id, resurrect
/// cost). The nodes stay.
///
/// TODO(sim/pets.md): t ≠ 7 (every pet unit killed, its node freed) is
/// `d2-sim::player::pets`'s.
pub fn classic_act_change<W: HirelingWorld>(w: &mut W, st: &mut HirelingState, player: UnitId) {
    let n = st.list(player).map_or(0, |l| l.nodes.len());
    for i in 0..n {
        let node = st.lists[&player].nodes[i];
        let Some(unit) = w.monster_by_guid(node.guid) else {
            continue;
        };
        broadcast_remove(w, node.guid);
        w.queue_room_removal(unit);
        if !node.dead {
            st.list_mut(player).nodes[i].dead = true;
            // TODO(hirelings.md §6 r4): "the list head := this node":
            // with `basemax` 1 the node is the only one (already the
            // head); a list holding earlier nodes is not modelled.
            if w.in_room(unit) {
                w.death_event(unit);
            }
            let cost = resurrect_cost(w.stat(unit, stat::LEVEL));
            w.send(player, &merc_dead_message(node.name, cost));
        }
    }
}

/// The saved hireling fields (save layout: open question 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SavedHireling {
    /// Dead bit 0x10000 of the saved flags.
    pub dead: bool,
    pub seed: u32,
    /// Name index (added to the row's `NameFirst`, §10 rule 2).
    pub name_index: u16,
    /// `hireling` `Id`.
    pub id: u16,
    pub experience: u32,
}

/// Why a save restores no hireling (§10 rules 1–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreSkip {
    /// Rule 1: seed, name and experience are all 0.
    NoHireling,
    /// Rule 2: no row of the `Id` ("Unable to load merc." in the old
    /// loader).
    NoRow,
    /// Rule 3: classic game and the name's act is not the player's act.
    OtherAct,
}

/// What [`restore_plan`] decides: the clamped name id and the mode the
/// caller creates the unit with (§3.1 rule 3: 12 dead, else 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestorePlan {
    pub name: u16,
    pub mode: u8,
}

/// §10 rules 1–3 up to the unit creation. `player_act`: the player's act
/// (client act `0x005382B0` when the player has a client, else unit
/// +0x18), read only in a classic game.
///
/// The caller then marks the hire slot of `name` hired in the seller's
/// list **only when it is not currently offered** (rule 3, edge case 8;
/// the hire list is `npc.md` §7's), creates the unit with `mode` and
/// calls [`restore`].
///
/// TODO(hirelings.md §10 r3): `Id` 0xFFFF (rule 1 by act and difficulty,
/// open question 3) is not handled: it gives [`RestoreSkip::NoRow`].
pub fn restore_plan(
    t: &HirelingTables,
    expansion: bool,
    saved: &SavedHireling,
    player_act: u32,
) -> Result<RestorePlan, RestoreSkip> {
    // Rule 1.
    if saved.seed == 0 && saved.name_index == 0 && saved.experience == 0 {
        return Err(RestoreSkip::NoHireling);
    }
    // Rule 2: the first bracket; name id := NameFirst + index, above
    // NameLast → NameFirst.
    let row = t
        .rows
        .row_at(expansion, u32::from(saved.id), 1)
        .ok_or(RestoreSkip::NoRow)?;
    let r = &t.rows.rows[row];
    let name = u32::from(r.name_first) + u32::from(saved.name_index);
    let name = if name > u32::from(r.name_last) {
        r.name_first
    } else {
        name as u16
    };
    // Rule 3: classic → only in the name's act. The row of rule 3 is the
    // same (Id, 1) row, so its name clamp leaves `name` unchanged.
    if !expansion && t.rows.act_of_name(name) != player_act {
        return Err(RestoreSkip::OtherAct);
    }
    let mode = if saved.dead { MODE_DEAD } else { MODE_NEUTRAL };
    Ok(RestorePlan { name, mode })
}

/// §10 rule 3 init and rule 4: init (§3.2 with `saved_id` = `Id`,
/// restore = 1: no offer stats, no experience reset), then the pet
/// node's seed / name / `Id` := the saved values (`0x005749B0`).
/// Returns whether the node exists (no offer → no node, §3.2 rule 5).
///
/// The caller then runs §10 rules 5–6 (experience and level:
/// [`super::level`]) and, when `saved.dead`, [`restore_dead`] (rule 7).
#[allow(clippy::too_many_arguments)]
pub fn restore<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &mut HirelingState,
    player: UnitId,
    merc: UnitId,
    plan: &RestorePlan,
    saved: &SavedHireling,
) -> Result<bool, HirelingError> {
    let slot = Slot {
        name: plan.name,
        seed: saved.seed,
    };
    init(w, t, st, player, merc, saved.id, slot, true)?;
    let guid = w.guid(merc);
    let Some(node) = st
        .lists
        .get_mut(&player)
        .and_then(|l| l.nodes.iter_mut().find(|n| n.guid == guid))
    else {
        return Ok(false);
    };
    node.seed = saved.seed;
    node.name = plan.name;
    node.id = u32::from(saved.id);
    Ok(true)
}

/// §10 rule 7: a dead saved hireling: §8 rule 2 (node dead, 0x9B to the
/// owner, 0x7A remove), flags |= 0x10000, mode 12.
///
/// TODO(hirelings.md §10 r7): `0x005738D0` and "no inventory → create
/// one" are not described / not in the seam.
pub fn restore_dead<W: HirelingWorld>(
    w: &mut W,
    st: &mut HirelingState,
    player: UnitId,
    merc: UnitId,
) {
    death(w, st, player, merc);
    let f = w.flags(merc);
    w.set_flags(merc, f | flags::DEAD);
    w.set_mode(merc, MODE_DEAD);
}
