// Spec: specs/items/inventory-moves.md §6–§10; specs/sim/unit-order.md §5–§6; specs/sim/units.md §2, §3.2; specs/items/generation.md §3
// Spec: specs/items/inventory.md (the sections other than §6–§11)
//! [`MovePending`] on [`InvDesk`]. Wired: the room list (`0x0064C2C0`
//! insert, `0x0064C370` remove, `unit-order.md` §5), the update queue
//! (`0x0064C040`, §6), "alive" (`0x005541B0`, `units.md` §2), "has
//! durability" (`0x00629930`, `generation.md` §1.3), item
//! freeing (`units.md` §3.2 through the economy) and the creation of gold
//! piles (`generation.md` §3 through the economy, request from the rest),
//! the item bit stream (`items/bitstream.md`, [`super::bits`]).
//! Every other call goes to [`InvRest`] unchanged.

use super::{InvDesk, InvError, InvRest};
use crate::items::inventory::{
    active_inventory_item, belt_removal_allowed, corpse_slot_fit, InvWorld, UnitKind,
};
use crate::items::moves::{Guid, MovePending, Owner, Spot};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

/// Stat 152 `item_indesctructible` (`generation.md` §1.3).
const STAT_INDESTRUCTIBLE: u16 = 152;

impl<H: LifecycleHooks, R: InvRest + ?Sized> MovePending for InvDesk<'_, '_, H, R> {
    /// Room list removal `0x0064C370` (a unit in no room is left as is).
    fn remove_from_room(&mut self, item: Guid) {
        if let Some(u) = self.item_unit(item) {
            let r = self.econ.game.lists.room_remove(u);
            self.note_list(r);
        }
    }
    /// Ground placement's "room added" (`0x00558AA0`): the room list
    /// insert `0x0064C2C0`. An item already in the spot's room (the
    /// allocator added it) is left as is; one in another room is logged
    /// ([`InvError::OtherRoom`]).
    fn add_to_room(&mut self, item: Guid, spot: Spot) {
        let Some(u) = self.item_unit(item) else {
            return;
        };
        match self.econ.game.lists.unit(u).and_then(|e| e.room()) {
            None => {
                let r = self.econ.game.lists.room_insert(u, spot.room);
                self.note_list(r);
            }
            Some(r) if r == spot.room => {}
            Some(_) => self.state.errors.push(InvError::OtherRoom(u)),
        }
    }
    fn in_room(&self, item: Guid) -> bool {
        self.item_unit(item)
            .and_then(|u| self.econ.game.lists.unit(u))
            .is_some_and(|e| e.room().is_some())
    }
    /// `0x0064C040` (`unit-order.md` §6.2).
    fn queue_update(&mut self, u: Owner) {
        if let Some(id) = self.unit_of(u) {
            let r = self.econ.game.lists.queue_update(id);
            self.note_list(r);
        }
    }
    /// `0x00557FD0` (`world/cube.md` §8 "Exact" 1, `inventory-moves.md`
    /// §7.12): the item is unlinked from any player inventory list or
    /// cursor still holding it (callback `0x00557FA0` → `0x00557F50` over
    /// the players), then the unit removal `0x00555600` (`units.md` §3.2)
    /// frees the unit and its item data. Nothing else.
    fn free_item(&mut self, item: Guid) {
        let Some(u) = self.item_unit(item) else {
            return;
        };
        let holders: Vec<UnitId> = self
            .state
            .inventories
            .iter()
            .filter(|(&o, inv)| {
                matches!(self.kind_of(o), Some(UnitKind::Player { .. }))
                    && (inv.contains(u) || inv.cursor() == Some(u))
            })
            .map(|(&o, _)| o)
            .collect();
        for o in holders {
            self.unlink_from(o, u);
        }
        if let Err(e) = self.econ.free_item(u) {
            self.state.errors.push(InvError::Economy(e));
        }
        self.state.inventories.remove(&u);
        self.state.expiry.remove(&u);
        self.sync_in();
    }
    /// `0x00559CE0` for code `gld` (`0x00633640`): the request comes from
    /// [`InvRest::gold_request`] (layout unwritten), the item from
    /// [`crate::wiring::economy::Economy::create_item`].
    fn create_gold(&mut self, unit: Owner, _spot: Spot) -> Option<Guid> {
        let gld = self.tables.items.iter().position(|r| &r.code == b"gld ")?;
        let (mut rq, spawn) = self.rest.gold_request(unit, gld)?;
        match self.econ.create_item(&mut rq, false, spawn) {
            Ok(u) => {
                self.sync_in();
                Some(self.guid_of(u))
            }
            Err(e) => {
                self.state.errors.push(InvError::Economy(e));
                None
            }
        }
    }
    /// `0x00629930` "has durability" (`generation.md` §1.3): items
    /// `nodurability` = 0, `durability` ≠ 0, the item has a stat list and
    /// stat 152 (`item_indesctructible`) < 1.
    fn merge_allowed(&self, src: Guid) -> bool {
        let Some(u) = self.item_unit(src) else {
            return false;
        };
        let Some(r) = self
            .state
            .items
            .get(&u)
            .and_then(|d| self.econ.tables.item(d.record))
        else {
            return false;
        };
        r.nodurability == 0
            && r.durability != 0
            && self.econ.units.get(u).is_some_and(|x| x.stats.is_some())
            && self.econ.stats.unit_total(u, STAT_INDESTRUCTIBLE, 0) < 1
    }
    /// Not dead (`0x005541B0`, `units.md` §2).
    fn alive(&self, u: Owner) -> bool {
        self.unit_of(u)
            .and_then(|id| self.econ.units.get(id))
            .is_some_and(|r| !r.is_dead())
    }

    // ---- no provider: the rest ------------------------------------------

    fn distance(&self, a: Owner, b: Owner) -> i32 {
        self.rest.distance(a, b)
    }
    fn collides(&self, a: Owner, b: Owner, mask: u32) -> bool {
        self.rest.collides(a, b, mask)
    }
    fn walk_to_item(&mut self, player: Owner, item: Guid, cursor: bool) {
        self.rest.walk_to_item(player, item, cursor)
    }
    fn walk_to_unit(&mut self, player: Owner, target: Owner, cursor: bool) {
        self.rest.walk_to_unit(player, target, cursor)
    }
    fn tile_warp(&mut self, player: Owner, tile: Owner) {
        self.rest.tile_warp(player, tile)
    }
    /// `0x005BF240(I, I, x, y)` (`items/use.md` §1): the entries run here
    /// ([`InvDesk::item_use`]: the Town Portal cast); any other entry: the
    /// rest's.
    fn use_item_at(&mut self, player: Owner, item: Guid, x: i32, y: i32) -> bool {
        if let Some(r) = self.item_use(player, item) {
            return r != 0;
        }
        self.rest.use_item_at(player, item, x, y)
    }
    fn open_cube(&mut self, player: Owner, cube: Guid) -> bool {
        self.open_cube_desk(player, cube)
    }
    /// `0x0055E000` (§7.11 step 3, §7.18 step 9): the removal message
    /// (flag 0x20), then the item leaves its inventory and is freed
    /// ([`InvDesk::remove_used_item`]); an item without a unit: the
    /// rest's.
    fn consume_item(&mut self, player: Owner, item: Guid) {
        if self.item_unit(item).is_some() {
            return self.remove_used_item(player, item);
        }
        self.rest.consume_item(player, item)
    }
    /// `0x0055E050` on the books rows ([`InvDesk::books_item_skill`]);
    /// not a book or scroll, or no books table: the rest's.
    fn item_skill(&self, item: Guid) -> i32 {
        self.books_item_skill(item)
            .unwrap_or_else(|| self.rest.item_skill(item))
    }
    fn has_skill(&self, player: Owner, skill: i32) -> bool {
        self.rest.has_skill(player, skill)
    }
    fn skill_decrement(&mut self, player: Owner, skill: i32) {
        self.rest.skill_decrement(player, skill)
    }
    fn set_quest_flag(&mut self, player: Owner, quest: u8, flag: u8, on: bool) {
        self.rest.set_quest_flag(player, quest, flag, on)
    }
    fn quest_item_used(&mut self, player: Owner) {
        self.rest.quest_item_used(player)
    }
    fn quest_tr2_used(&mut self, player: Owner) {
        self.rest.quest_tr2_used(player)
    }
    fn reset_skills_stats(&mut self, player: Owner) {
        self.rest.reset_skills_stats(player)
    }
    fn has_used_skill(&self, player: Owner) -> bool {
        self.rest.has_used_skill(player)
    }
    /// `0x0057FB70` on the unit hooks
    /// ([`crate::units::hooks::UnitHooks::player_corpse_pickup`]: the
    /// action wiring's `ActionHooks::corpse_pickup`).
    fn corpse_pickup(&mut self, player: Owner, corpse: Owner) -> bool {
        let (Some(p), Some(c)) = (self.unit_of(player), self.unit_of(corpse)) else {
            return false;
        };
        let e = &mut *self.econ;
        let mut sim = crate::units::hooks::Sim {
            game: &mut *e.game,
            units: &mut *e.units,
            stats: &mut *e.stats,
            data: e.data,
        };
        e.hooks.player_corpse_pickup(&mut sim, p, c)
    }
    /// §12.3 on the inventory model ([`corpse_slot_fit`]).
    fn corpse_slot_fit(
        &self,
        unit: Owner,
        x: Guid,
        d: Option<Guid>,
        a: Option<Guid>,
        l: u8,
    ) -> (bool, u8) {
        let (Some(u), Some(xu)) = (self.unit_of(unit), self.item_unit(x)) else {
            return (false, l);
        };
        let d = d.and_then(|g| self.item_unit(g));
        let a = a.and_then(|g| self.item_unit(g));
        corpse_slot_fit(self, self.tables, u, xu, d, a, l)
    }
    /// The corpse list is not modelled by the desk (the rest's
    /// `corpse_taken` runs after).
    ///
    /// d2rs-own, unverified (PROVISIONAL REC-141): §12.1 step 4 on the
    /// wired side. The corpse unit leaves its room and is freed with its
    /// inventory (`0x0061A270`, `0x00555600`); the player is sent S→C 0x8E
    /// `CorpseAssign` [1] 0, the player's and the corpse's GUID, and 0x0A
    /// for the unit, so the client drops it. Other players' clients are
    /// not told (single-player preview); the rest's `corpse_taken` still
    /// runs.
    fn corpse_taken(&mut self, player: Owner, corpse: Owner) {
        if let Some(c) = self.unit_of(corpse) {
            let mut assign = vec![0x8E, 0];
            assign.extend_from_slice(&player.guid.to_le_bytes());
            assign.extend_from_slice(&corpse.guid.to_le_bytes());
            self.rest.send(player, assign);
            self.rest.send(
                player,
                crate::units::messages::remove_unit(0, corpse.guid).to_vec(),
            );
            let r = self.econ.game.lists.room_remove(c);
            self.note_list(r);
            self.state.inventories.remove(&c);
            let (mut sim, hooks) = self.econ.split();
            if let Err(e) = crate::units::lifecycle::remove(&mut sim, hooks, c) {
                self.state.errors.push(InvError::Economy(e.into()));
            }
            self.sync_in();
        }
        self.rest.corpse_taken(player, corpse)
    }
    fn replenish_timers(&mut self, item: Guid) {
        if let Some(u) = self.item_unit(item) {
            self.schedule_replenish(u);
        }
    }
    fn player_interact(&mut self, player: Owner, other: Owner) {
        self.rest.player_interact(player, other)
    }
    fn room_at(&self, x: i32, y: i32) -> bool {
        self.rest.room_at(x, y)
    }
    fn free_spot(
        &self,
        start: (i32, i32),
        origin: (i32, i32),
        size: u32,
        mask: u32,
        mask2: u32,
        last: u32,
    ) -> Option<Spot> {
        self.rest.free_spot(start, origin, size, mask, mask2, last)
    }
    fn in_town(&self, player: Owner) -> bool {
        self.rest.in_town(player)
    }
    fn room_delete_notice(&mut self, item: Guid) {
        self.rest.room_delete_notice(item)
    }
    fn free_collision(&mut self, item: Guid) {
        self.rest.free_collision(item)
    }
    fn room_change_notice(&mut self, item: Guid, x: i32, y: i32) {
        self.rest.room_change_notice(item, x, y)
    }
    fn stat_refresh(&mut self, u: Owner) {
        self.rest.stat_refresh(u)
    }
    fn stat_refresh_unlink(&mut self, u: Owner, b: u32) {
        self.rest.stat_refresh_unlink(u, b)
    }
    fn stat_link(&mut self, owner: Owner, item: Guid) {
        if let (Some(o), Some(i)) = (self.unit_of(owner), self.item_unit(item)) {
            self.link_item_stats(o, i);
        }
        self.rest.stat_link(owner, item)
    }
    /// §5.5 `0x0055C270` on the rules when [`InvState::equip_rules`] is on.
    ///
    /// [`InvState::equip_rules`]: super::InvState::equip_rules
    fn charm_relink(&mut self, owner: Owner, item: Guid) {
        match (
            self.state.equip_rules,
            self.unit_of(owner),
            self.item_unit(item),
        ) {
            (true, Some(o), Some(i)) => self.run_item_skill_link(o, i, true),
            _ => self.rest.charm_relink(owner, item),
        }
    }
    /// §5.5 `0x0055C6E0` on the rules when the equipment rules are on.
    fn charm_unlink(&mut self, owner: Owner, item: Guid) {
        match (
            self.state.equip_rules,
            self.unit_of(owner),
            self.item_unit(item),
        ) {
            (true, Some(o), Some(i)) => self.run_item_skill_link(o, i, false),
            _ => self.rest.charm_unlink(owner, item),
        }
        if let (Some(o), Some(i)) = (self.unit_of(owner), self.item_unit(item)) {
            self.unlink_item_stats(o, i);
        }
    }
    /// §5.6 (`0x0062FF70`).
    fn is_active(&self, owner: Owner, item: Guid) -> bool {
        match (self.unit_of(owner), self.item_unit(item)) {
            (Some(o), Some(i)) => active_inventory_item(self, self.tables, i, o),
            _ => false,
        }
    }
    /// §5.7 `0x0055DBC0(0)` on the rules when the equipment rules are on.
    fn inventory_pass(&mut self, owner: Owner) {
        match (self.state.equip_rules, self.unit_of(owner)) {
            (true, Some(o)) => self.run_inventory_pass(o, false),
            _ => self.rest.inventory_pass(owner),
        }
        if let Some(o) = self.unit_of(owner) {
            self.link_charms(o);
        }
    }
    fn weapon_in_use_update(&mut self, owner: Owner) {
        self.rest.weapon_in_use_update(owner)
    }
    /// §5.8 `0x0055C5C0` on the rules when the equipment rules are on.
    fn weapon_bookkeeping(&mut self, owner: Owner) {
        match (self.state.equip_rules, self.unit_of(owner)) {
            (true, Some(o)) => self.run_weapon_bookkeeping(o),
            _ => self.rest.weapon_bookkeeping(owner),
        }
    }
    fn body_leave_effects(&mut self, owner: Owner, item: Guid) {
        if let (Some(o), Some(i)) = (self.unit_of(owner), self.item_unit(item)) {
            self.unlink_item_stats(o, i);
        }
        self.rest.body_leave_effects(owner, item)
    }
    fn hireling_owner_pass(&mut self, owner: Owner) {
        self.rest.hireling_owner_pass(owner)
    }
    /// §3 rule 10 (`0x00567840`) on the player's inventory.
    fn belt_remove_allowed(&self, player: Owner) -> bool {
        belt_removal_allowed(&self.inv_or_empty(player), self)
    }
    fn sound(&mut self, u: Owner, id: u32) {
        self.rest.sound(u, id)
    }
    fn pickup_sound(&mut self, player: Owner, item: Guid) {
        self.rest.pickup_sound(player, item)
    }
    fn requirement_sound(&mut self, player: Owner) {
        self.rest.requirement_sound(player)
    }
    fn merc_sound(&mut self, player: Owner) {
        self.rest.merc_sound(player)
    }
    fn quest_flag(&self, player: Owner, quest: u8, flag: u8) -> bool {
        self.rest.quest_flag(player, quest, flag)
    }
    fn quest_item_picked(&mut self, player: Owner, item: Guid) {
        self.rest.quest_item_picked(player, item)
    }
    fn quest_item_dropped(&mut self, item: Guid) {
        self.rest.quest_item_dropped(item)
    }
    fn carry_one(&self, item: Guid) -> bool {
        self.rest.carry_one(item)
    }
    fn held_test_units(&self, player: Owner) -> Vec<Owner> {
        self.rest.held_test_units(player)
    }
    fn copy_item(&mut self, item: Guid) -> Option<Guid> {
        self.rest.copy_item(item)
    }
    fn give_cursor_item(&mut self, player: Owner, item: Guid) {
        self.rest.give_cursor_item(player, item)
    }
    fn consume_one(&mut self, item: Guid) -> bool {
        self.rest.consume_one(item)
    }
    fn set_owner(&mut self, item: Guid, owner: Owner) {
        self.rest.set_owner(item, owner)
    }
    fn pile_owner(&self, item: Guid) -> Option<Owner> {
        self.rest.pile_owner(item)
    }
    fn query_0044be50(&self) -> bool {
        self.rest.query_0044be50()
    }
    fn party_share_id(&self, player: Owner) -> i32 {
        self.rest.party_share_id(player)
    }
    fn party_share(&mut self, player: Owner, take: i32) {
        self.rest.party_share(player, take)
    }
    fn owned_gold_pickup(&mut self, player: Owner, pile: Guid, take: i32) {
        self.rest.owned_gold_pickup(player, pile, take)
    }
    fn rest_pile(&mut self, player: Owner, rest: i32) {
        self.rest.rest_pile(player, rest)
    }
    fn book_count_changed(&mut self, player: Owner, n: i32) {
        self.rest.book_count_changed(player, n)
    }
    fn use_item(&mut self, player: Owner, target: Owner, item: Guid) -> bool {
        // `0x005BF240(U, T, …)` for the entries run here (the Town Portal
        // cast, [`InvDesk::item_use`]).
        if let Some(r) = self.item_use(player, item) {
            return r != 0;
        }
        // PROVISIONAL (REC-102): potions on the player.
        if target == player && self.use_potion(player, item) {
            return true;
        }
        // PROVISIONAL (REC-113): identify scrolls and tomes on an item.
        if self.use_identify(player, target, item) {
            return true;
        }
        self.rest.use_item(player, target, item)
    }
    fn charge_update(&mut self, player: Owner, item: Guid) {
        self.rest.charge_update(player, item)
    }
    fn remove_used(&mut self, player: Owner, item: Guid) {
        if self.item_unit(item).is_some() {
            return self.remove_used_item(player, item);
        }
        self.rest.remove_used(player, item)
    }
    fn equip_picked(&mut self, player: Owner, item: Guid) -> bool {
        self.rest.equip_picked(player, item)
    }
    /// The filler's properties (`properties.md` §9,
    /// [`InvDesk::apply_filler_properties`]), then the owner link
    /// `0x006276C0` on the rest.
    fn filler_linked(&mut self, filler: Guid, target: Guid) {
        if let (Some(f), Some(t)) = (self.item_unit(filler), self.item_unit(target)) {
            self.apply_filler_properties(f, t);
        }
        self.rest.filler_linked(filler, target)
    }
    /// §7.19 step 3's runeword and recharge ([`InvDesk::socket_runeword`]);
    /// each recharged charged skill is announced to the player by S→C 0x3E
    /// stat 204 (`generation.md` §12.2 step 4). The rest's default is not
    /// asked.
    fn runeword(&mut self, player: Owner, target: Guid) -> bool {
        let Some(t) = self.item_unit(target) else {
            return false;
        };
        let (ran, recharged) = self.socket_runeword(t);
        if player.is_player() {
            for _ in recharged {
                self.rest
                    .send_item_stat(player, target, crate::items::recharge::CHARGED_SKILL);
            }
        }
        ran
    }
    /// `0x00574EC0(7, 0)` on the lent hireling lists
    /// ([`InvDesk::lent_hireling`]); none lent → the rest.
    fn hireling(&self, player: Owner) -> Option<Owner> {
        match self.lent_hireling(player) {
            Some(m) => m,
            None => self.rest.hireling(player),
        }
    }
    /// `0x0065A590` with the lent lists ([`InvDesk::lent_owns_hireling`]);
    /// none lent → the rest.
    fn owns_hireling(&self, player: Owner, merc: Owner) -> bool {
        match self.lent_owns_hireling(player, merc) {
            Some(b) => b,
            None => self.rest.owns_hireling(player, merc),
        }
    }
    /// `0x0054CED0` (`world/hirelings.md` §11) with the lent lists
    /// ([`InvDesk::lent_equip_on_merc`]); none lent → the rest.
    fn equip_on_merc(&mut self, merc: Owner, item: Guid) {
        if !self.lent_equip_on_merc(merc, item) {
            self.rest.equip_on_merc(merc, item)
        }
    }
    fn merc_after_take(&mut self, merc: Owner) {
        self.rest.merc_after_take(merc)
    }
    fn pick_npc(&mut self, player: Owner, guid: Guid, cursor: u32) -> u32 {
        self.rest.pick_npc(player, guid, cursor)
    }
    fn pick_object(&mut self, player: Owner, guid: Guid, cursor: u32) -> u32 {
        self.rest.pick_object(player, guid, cursor)
    }
    fn send(&mut self, player: Owner, bytes: Vec<u8>) {
        self.rest.send(player, bytes)
    }
    fn send_item_stat(&mut self, player: Owner, item: Guid, stat: u16) {
        self.rest.send_item_stat(player, item, stat)
    }
    /// The item bit stream (`items/bitstream.md`) of the real item
    /// ([`InvDesk::item_stream`]); the rest's default is not asked.
    fn item_bits(&self, item: Guid, flags: u32, page: u8) -> Vec<u8> {
        self.item_stream(item, flags, page)
    }
    fn store_messages(&mut self, client: Owner, item: Guid) -> Vec<Vec<u8>> {
        self.rest.store_messages(client, item)
    }
}
