// Spec: specs/world/hirelings.md §3.2, §6 r1, §6 r4, §8 r2, §9 r3, §9 r4, §9 r5, §9 r6, §9 r7, §9 r8, §9 r9, §10 r1, §10 r2, §10 r3, §10 r4, §10 r7; specs/world/hirelings-2.md §15, §16, §18 r1, §18 r2, §18 r3
//! The hireling's life: init (§3.2), following the player (§6 rule 1,
//! type 7; `hirelings-2.md` §18), the classic act change and the
//! owner's death (§6 rule 4, `hirelings-2.md` §15, type 7), death (§8
//! rule 2), revive (§9 rules 3–9, `0x00579AA0`) and restoring from a
//! save (§10 rules 1–4 and 7, `hirelings-2.md` §16).
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
    let act0 = t.rows.act_of_name(w.expansion(), slot.name);
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
    // Rule 6. The add is void in 1.14d (`0x00575E90`, no test after the
    // call): a `false` (no node added) does not stop the init.
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

/// §8 rule 1, the hireling part of the kill `0x0057CCB0(game, unit,
/// killer, flag)`: the kill's own guards (monster mode not 0 / 12, the
/// `0x00457490(class, 15)` `killable` test) are the caller's; then with
/// `flag` ≠ 0 (1 at every caller except the expired-pet kill
/// `0x00574450`) and a player owner (`0x0058F0D0`, type 0),
/// `0x005751A0(game, owner, merc)` ([`death`]). Returns whether a node
/// was marked dead.
pub fn on_kill<W: HirelingWorld>(
    w: &mut W,
    st: &mut HirelingState,
    merc: UnitId,
    flag: bool,
) -> bool {
    if !flag {
        return false;
    }
    let Some((guid, UNIT_PLAYER)) = w.owner(merc) else {
        return false;
    };
    let Some(owner) = w.player_by_guid(guid) else {
        return false;
    };
    death(w, st, owner, merc)
}

/// §8 rule 2 (`0x005751A0(game, owner, merc)`), hireling list part: the
/// node of the merc's GUID gets bit 0 (dead), the owner's client gets
/// 0x9B (name id, resurrect cost at the current level) and every client
/// 0x7A remove with only the GUID. `false`: the GUID is in no hireling
/// node.
///
/// The pet type lookup (`0x00574A20`) and the type ≠ 7 node removal are
/// `sim/pets.md`'s (`d2-sim::player::pets`); the trigger is [`on_kill`].
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
    // Flags 2 bit 0x100 (§8 rule 5) is kept; no 1.14d reader reaches a
    // type-7 node (`hirelings-2.md` §18 rule 4).
    // Rule 7.
    w.reapply_item_stats(merc);
    // Rule 8.
    follow(w, t, st, player);
    // Rule 9.
    level::send_stats(w, st, player)
}

/// §6 rule 1 (`0x005754B0(game, player, x, y)`), pet type 7: `warp` →
/// every living node's unit moves to the player (§6 rule 5, flags 2 |=
/// 0x10000); else `range` → every living node whose unit exists and is
/// farther than squared distance 1600 from the player unit is removed
/// with kill (`hirelings-2.md` §18 rule 1); else every node is freed
/// with its unit (`0x00574C60`, §18 rule 2).
///
/// TODO(sim/pets.md): pet types 1 … count − 1 other than 7 are
/// `d2-sim::player::pets`'s; the caller runs them in row order.
pub fn follow<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &mut HirelingState,
    player: UnitId,
) {
    let living: Vec<u32> = st
        .list(player)
        .map(|l| l.nodes.iter().filter(|n| !n.dead).map(|n| n.guid).collect())
        .unwrap_or_default();
    if t.pet_flags & HirelingTables::WARP != 0 {
        for guid in living {
            if let Some(unit) = w.monster_by_guid(guid) {
                w.warp_to(unit, player);
                let f2 = w.flags2(unit);
                w.set_flags2(unit, f2 | flags::WARP2);
            }
        }
    } else if t.pet_flags & HirelingTables::RANGE != 0 {
        // §18 rule 1 (`0x00575380(game, player, list, 1600)`): the
        // player unit's position, not the follow's (x, y).
        let (px, py) = w.position(player);
        for guid in living {
            let Some(unit) = w.monster_by_guid(guid) else {
                continue;
            };
            if range_distance(w.position(unit), (px, py)) > RANGE_LIMIT {
                // `0x005750E0(game, player, GUID, 1)`, §5 rule 5. The
                // node is in the list (taken from it above), so the
                // remove cannot fail on a missing node.
                let _ = pets::remove(w, st, player, guid, true);
            }
        }
    } else {
        // §18 rule 2: every node of the type freed with its unit.
        // TODO(spec: sim/pets.md): `0x00574C60`'s unit free is not
        // written; [`HirelingWorld::free_unit`] stands for it. Unused by
        // 1.14d row 7 (warp 1).
        let guids: Vec<u32> = st
            .lists
            .get_mut(&player)
            .map(|l| l.nodes.drain(..).map(|n| n.guid).collect())
            .unwrap_or_default();
        for guid in guids {
            if let Some(unit) = w.monster_by_guid(guid) {
                w.free_unit(unit);
            }
        }
    }
}

/// The range pets' limit (`push 0x640` at `0x0057554A`,
/// `hirelings-2.md` §18 rule 1).
pub const RANGE_LIMIT: i32 = 1600;

/// `0x006492A0`: dx² + dy² (32-bit signed, wrapping), compared signed
/// against [`RANGE_LIMIT`] (`hirelings-2.md` §18 rule 1).
pub fn range_distance(pet: (i32, i32), player: (i32, i32)) -> i32 {
    let dx = pet.0.wrapping_sub(player.0);
    let dy = pet.1.wrapping_sub(player.1);
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}

/// `0x005773D0(game, player)` (`hirelings-2.md` §16 rule 3, from the
/// join placement `0x005394A0` at `0x005396C3`): the pet follow (§6
/// rule 1) to the player's position when the player has a living
/// hireling (`(7, 0)`); else nothing. A dead restored hireling stays
/// roomless (§8 rule 5).
///
/// Caller (`hirelings-2.md` §19): the join placement
/// (`wiring::path::place::game_entry`, `0x005394A0`) queues the call
/// (`wiring::action::HirelingCall::JoinFollow`) for the host holding the
/// hireling lists (`d2-server` `WiredWorld::hireling_calls`).
pub fn join_follow<W: HirelingWorld>(
    w: &mut W,
    t: &HirelingTables,
    st: &mut HirelingState,
    player: UnitId,
) {
    if pets::living(w, st, player).is_some() {
        follow(w, t, st, player);
    }
}

/// `0x00575BC0(game, player)`, t = 7 (`0x00574570` with keep =
/// keep_dead = 1; §6 rule 4, `hirelings-2.md` §15 rules 2–3, §18 rule 3):
/// for each node whose unit exists, in list order: broadcast 0x7A remove
/// with only the GUID, post the room removal notice {1, GUID} (the unit
/// is not unlinked from its room nor freed); a living node: bit 0 := 1,
/// list head := this node (earlier nodes leave the list, not freed), the
/// death mode request when the unit is in a room, and 0x9B (name id,
/// resurrect cost) to the player's client. A node whose unit does not
/// exist is left as it is. Count and max are unchanged.
///
/// TODO(sim/pets.md): t ≠ 7 (every pet unit killed, its node freed) and
/// the max recompute `0x00575900` after the walk are
/// `d2-sim::player::pets`'s.
pub fn kill_with_owner<W: HirelingWorld>(w: &mut W, st: &mut HirelingState, player: UnitId) {
    let mut i = 0;
    while i < st.list(player).map_or(0, |l| l.nodes.len()) {
        let node = st.lists[&player].nodes[i];
        let Some(unit) = w.monster_by_guid(node.guid) else {
            i += 1;
            continue;
        };
        broadcast_remove(w, node.guid);
        w.queue_room_removal(unit);
        if !node.dead {
            let list = st.list_mut(player);
            list.nodes[i].dead = true;
            // §18 rule 3: "list head := node". With type 7 the list holds
            // one node (§5 rule 3), so nothing is dropped with live data;
            // in 1.14d the dropped nodes stay counted, which a list held
            // as its nodes cannot show.
            list.nodes.drain(..i);
            i = 0;
            if w.in_room(unit) {
                w.death_event(unit);
            }
            let cost = resurrect_cost(w.stat(unit, stat::LEVEL));
            w.send(player, &merc_dead_message(node.name, cost));
        }
        i += 1;
    }
}

/// §6 rules 3–4: the classic act change (`0x0053ACC0`, classic game
/// only, before the player is placed in the new act) →
/// [`kill_with_owner`]. The caller then runs the pet follow ([`follow`])
/// to the new position (in both game types).
///
/// Caller (`hirelings-2.md` §19): the path wiring's `level_warp` meets
/// the act change (another act: `None`, the warp stays on its
/// `Pending::warp` route, owner: the act / level-change spec) and queues
/// it (`wiring::action::HirelingCall::ActChange`); the host holding the
/// hireling lists runs this, then [`follow`] (`d2-server`
/// `WiredWorld::hireling_calls`).
pub fn classic_act_change<W: HirelingWorld>(w: &mut W, st: &mut HirelingState, player: UnitId) {
    kill_with_owner(w, st, player);
}

/// `hirelings-2.md` §15 rule 1: the player mode-17 start `0x0057FCA0`
/// calls `0x00575BC0(game, player)` at `0x0057FD25`, after the corpse
/// creation, **in every game type**: the hireling dies with its owner
/// ([`kill_with_owner`]). Its messages follow the corpse's (rule 5).
pub fn player_death<W: HirelingWorld>(w: &mut W, st: &mut HirelingState, player: UnitId) {
    kill_with_owner(w, st, player);
}

/// The saved hireling fields (`formats/d2s.md` §2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SavedHireling {
    /// Dead bit 0x10000 of the saved flags.
    pub dead: bool,
    pub seed: u32,
    /// Name index (added to the row's `NameFirst`, §10 rule 2); the name
    /// word of a version-0x47 save ([`Loader::OldV47`]).
    pub name_index: u16,
    /// `hireling` `Id`.
    pub id: u16,
    pub experience: u32,
}

/// The 1.14d loader that restores the hireling (`hirelings-2.md` §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Loader {
    /// `0x0056AA50`, save version ≥ 0x5C.
    #[default]
    Current,
    /// `0x00533C70`, older saves, main path.
    Old,
    /// `0x00533C70`'s version-0x47 path (`0x00533CB1`, header word `JM`
    /// 0x4D4A): `Id` 0xFFFF, class 0, dead 0 (§16 rule 1).
    OldV47,
}

impl Loader {
    /// The old loader (`0x00533C70`, both paths).
    pub fn is_old(self) -> bool {
        matches!(self, Loader::Old | Loader::OldV47)
    }
}

/// Why a save restores no hireling (§10 rules 1–3, `hirelings-2.md`
/// §16 rules 1–4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreSkip {
    /// Rule 1: seed, name and experience are all 0 (version-0x47 path:
    /// seed or name word 0).
    NoHireling,
    /// Rule 2, ≥ 0x5C loader: no row (`Id`, level 1); the hireling is
    /// skipped and the load goes on (`0x0056AA93`, no error).
    NoRow,
    /// Rule 2, old loader: no row (`Id`, level 1): the load fails with
    /// "Unable to load merc." (error 0xE).
    LoadFailed,
    /// Rule 3: classic game and the name's act is not the player's act.
    OtherAct,
    /// A fatal error of 1.14d by string id: 0x744 (`Id` 0xFFFF and no
    /// candidate row, §16 rule 2).
    Fatal(u16),
}

/// The old loader's load error "Unable to load merc." (§16 rule 4).
pub const LOAD_ERROR_MERC: u32 = 0xE;
/// Fatal string of an `Id` 0xFFFF restore without a row (§16 rule 2).
pub const FATAL_NO_ROW: u16 = 0x744;
/// Fatal string of a null row in the old loader's level loop (§16 rule
/// 4).
pub use super::level::FATAL_LEVEL_ROW;

/// What [`restore_plan`] decides for `0x005774F0(game, player, name,
/// seed, Id, class, dead)` (§10 rule 3, `hirelings-2.md` §16 rules 1–3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestorePlan {
    /// The name id (clamped).
    pub name: u16,
    /// Mode of the unit: 12 dead, else 1 (§3.1 rule 3).
    pub mode: u8,
    /// The class argument the unit is allocated with (§16 rule 1): the
    /// `Class` of row (`Id`, 1), or 0 on the version-0x47 path.
    pub class: u32,
    /// The `Id` argument: the saved `Id`, or 0xFFFF on the version-0x47
    /// path.
    pub id: u16,
    /// `Seller` (+0x14) of the row 0x005774F0 looks up: the NPC record
    /// whose hire list gets the slot marked (`0x00535EA0`, §16 rule 3).
    pub seller: u32,
    pub loader: Loader,
}

/// §10 rules 1–3 up to the unit creation (`hirelings-2.md` §16 rules
/// 1–3). `player_act`: the player's act (client act `0x005382B0` when the
/// player has a client, else unit +0x18), read only in a classic game;
/// `difficulty`: game +0x6D (read only for `Id` 0xFFFF).
///
/// The caller then marks the hire slot of `name` hired in the hire list
/// of the NPC record of `seller` **only when it is not currently
/// offered** (rule 3, edge case 8; the hire list is `npc.md` §7's),
/// allocates the unit of class `class` with `mode` **with no room** at
/// point (0, 0) (`0x00555230(1, class, 0, 0, game, room 0, flag 1, mode,
/// 0)`, §16 rule 3) and calls [`restore`].
pub fn restore_plan(
    t: &HirelingTables,
    expansion: bool,
    difficulty: u8,
    saved: &SavedHireling,
    player_act: u32,
    loader: Loader,
) -> Result<RestorePlan, RestoreSkip> {
    let (name, id, class, dead, row) = if loader == Loader::OldV47 {
        // §16 rule 1: only when seed ≠ 0 and the name word ≠ 0; Id
        // 0xFFFF, class 0, dead 0. The name word goes in as it is.
        if saved.seed == 0 || saved.name_index == 0 {
            return Err(RestoreSkip::NoHireling);
        }
        // §16 rule 2: row := the first candidate of (expansion, act of
        // the name, difficulty) (§1.2 rule 1, after 0); none → fatal.
        let act0 = t.rows.act_of_name(expansion, saved.name_index);
        let row = *t
            .rows
            .candidates(expansion, act0, u32::from(difficulty))
            .first()
            .ok_or(RestoreSkip::Fatal(FATAL_NO_ROW))?;
        (saved.name_index, NEW_HIRE, 0, false, row)
    } else {
        // Rule 1.
        if saved.seed == 0 && saved.name_index == 0 && saved.experience == 0 {
            return Err(RestoreSkip::NoHireling);
        }
        // Rule 2: the first bracket; none → skipped (≥ 0x5C) or the load
        // fails (old, §16 rule 4). Name id := NameFirst + index, above
        // NameLast → NameFirst.
        let row = t
            .rows
            .row_at(expansion, u32::from(saved.id), 1)
            .ok_or(if loader.is_old() {
                RestoreSkip::LoadFailed
            } else {
                RestoreSkip::NoRow
            })?;
        let r = &t.rows.rows[row];
        let name = u32::from(r.name_first) + u32::from(saved.name_index);
        let name = if name > u32::from(r.name_last) {
            r.name_first
        } else {
            name as u16
        };
        (name, saved.id, r.class, saved.dead, row)
    };
    // Rule 3: classic → only in the name's act. On the main paths the
    // row of rule 3 is the same (Id, 1) row, so its name clamp leaves
    // `name` unchanged; on the 0x47 path the row is the first candidate
    // of the name's act, whose range holds the name when a row matched
    // it (§1.1 rule 3: one range per act).
    if !expansion && t.rows.act_of_name(expansion, name) != player_act {
        return Err(RestoreSkip::OtherAct);
    }
    // TODO(spec: hirelings.md §10 r3): the form of 0x005774F0's name
    // clamp for a name outside every row's range (0x47 path only) is not
    // written; the name is kept.
    let mode = if dead { MODE_DEAD } else { MODE_NEUTRAL };
    Ok(RestorePlan {
        name,
        mode,
        class,
        id,
        seller: t.rows.rows[row].seller,
        loader,
    })
}

/// §10 rule 3 init and rule 4: init (§3.2 with `saved_id` = `Id`,
/// restore = 1), then the pet node's seed / name / `Id` := the saved
/// values (`0x005749B0`, right after the unit exists and before the
/// experience step, `hirelings-2.md` §16 rule 7). Returns whether the
/// node exists (no offer → no node, §3.2 rule 5).
///
/// `Id` 0xFFFF (the version-0x47 path, §16 rule 2): the init takes the
/// new-hire branch (the node gets the offer's row `Id`, the unit the
/// offer's experience and level, from the saved seed and name) and the
/// path writes no node values of its own (original bug, reproduced).
///
/// The caller then runs §10 rules 5–6 ([`super::level::restore_experience`],
/// not on the 0x47 path) and [`restore_tail`] (rule 7).
///
/// Caller (`hirelings-2.md` §19): the save load `0x0056B180` →
/// `0x0056AA50` (`d2-server` `adapters::character`) queues the restore
/// (`wiring::action::HirelingCall::Restore`); the wired host
/// (`WiredWorld::restore_hireling`) allocates the roomless unit and runs
/// this with [`restore_plan`], the experience step and [`restore_tail`].
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
    init(w, t, st, player, merc, plan.id, slot, true)?;
    let guid = w.guid(merc);
    let Some(node) = st
        .lists
        .get_mut(&player)
        .and_then(|l| l.nodes.iter_mut().find(|n| n.guid == guid))
    else {
        return Ok(false);
    };
    if plan.loader != Loader::OldV47 {
        node.seed = saved.seed;
        node.name = plan.name;
        node.id = u32::from(saved.id);
    }
    Ok(true)
}

/// `0x005738D0(game, merc)` (`hirelings-2.md` §16 rule 6): cancel the
/// merc's pending timer events of type 2 (AI think), then type 3 (stat
/// regeneration), any argument.
pub fn cancel_ai_and_regen<W: HirelingWorld>(w: &mut W, merc: UnitId) {
    w.cancel_timers(merc, TIMER_AI, 0);
    w.cancel_timers(merc, TIMER_REGEN, 0);
}

/// Timer event type 2: AI think (`sim/tick.md` §5).
pub const TIMER_AI: u8 = 2;
/// Timer event type 3: stat regeneration (`sim/tick.md` §5).
pub const TIMER_REGEN: u8 = 3;

/// §10 rule 7, dead part: §8 rule 2 (node dead, 0x9B to the owner, 0x7A
/// remove), flags |= 0x10000, mode 12 (`0x00553570`), `0x005738D0`
/// ([`cancel_ai_and_regen`]).
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
    cancel_ai_and_regen(w, merc);
}

/// §10 rule 7 in each loader's order (`hirelings-2.md` §16 rule 5): ≥
/// 0x5C loader: the dead steps ([`restore_dead`], when `dead`), then "no
/// inventory → create one"; old loader: the inventory step first, then
/// the dead steps.
pub fn restore_tail<W: HirelingWorld>(
    w: &mut W,
    st: &mut HirelingState,
    player: UnitId,
    merc: UnitId,
    dead: bool,
    loader: Loader,
) {
    if loader.is_old() {
        w.ensure_inventory(merc);
    }
    if dead {
        restore_dead(w, st, player, merc);
    }
    if !loader.is_old() {
        w.ensure_inventory(merc);
    }
}
