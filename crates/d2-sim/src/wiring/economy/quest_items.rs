// Spec: specs/world/quests.md §4.4, §4.5, §9; specs/sim/units.md §2; specs/sim/stat-lists.md §5
//! [`QuestWorld`] on the real providers: game fields ([`GameFields`]),
//! the frame and unit lookups ([`crate::game::Game`]), unit records
//! (GUID, class, unit seed), stat lists (stat reads and adds) and the
//! item data (item codes, `quest` bytes). Everything else stays a seam:
//! [`QuestRest`].
//!
//! [`GameFields`]: super::GameFields

use super::Economy;
use crate::rng::Seed;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};
use crate::world::quests::{PlayerQuests, QuestChain, QuestWorld, UnitKind};

/// The quest calls no written spec provides yet, each with its expected
/// provider (`docs/handoff/impl-world.md` "Seams").
pub trait QuestRest {
    /// Game +0xC0 (DRLG).
    fn has_act2(&self) -> bool;
    /// Players in `unit-order.md` §7 order (units / clients).
    fn players(&self) -> Vec<UnitId>;
    fn first_client_player(&self) -> Option<UnitId>;
    /// Player data (not in d2-sim yet).
    fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests>;
    fn player_byte_4c(&self, player: UnitId) -> u8;
    fn set_player_byte_4c(&mut self, player: UnitId, v: u8);
    /// Unit +0x74 (not in the unit record yet).
    fn quest_chain(&mut self, unit: UnitId) -> Option<&mut QuestChain>;
    /// Room level and act (DRLG).
    fn unit_act(&self, unit: UnitId) -> Option<u8>;
    fn unit_level(&self, unit: UnitId) -> Option<u32>;
    /// Superunique hcIdx and minion owner (monsters spec).
    fn unit_kind(&self, unit: UnitId) -> UnitKind;
    /// `quests.md` §10.5 J3's room test (DRLG rooms).
    fn players_near(&self, unit: UnitId) -> Vec<UnitId>;
    /// The party list at game +0x1D2C (no party spec; `quests.md` open
    /// question 7).
    fn party_members(&self, player: UnitId) -> Option<Vec<UnitId>>;
    fn attach_sound(&mut self, player: UnitId, sound: u16);
    fn send(&mut self, player: UnitId, msg: &[u8]);
    fn send_text_list(&mut self, player: UnitId, npc: UnitId, list: &[(u16, u32)]);
    /// The player's inventory items in list order (inventory spec;
    /// `cube.md` open question 5).
    fn inventory(&self, player: UnitId) -> Vec<UnitId>;
    /// `0x00544160` (`quests.md` §9.2: removal by item mode, inventory).
    fn delete_item(&mut self, player: UnitId, code: [u8; 4]);
    /// `0x005466B0` (`quests.md` §9.1). TODO(quests.md §9.1): its
    /// creation call `0x00559CE0` and the level default `0x00558200` have
    /// no request layout in the items specs; placement is the inventory
    /// spec's. Not wired.
    fn reward_item(
        &mut self,
        player: UnitId,
        code: [u8; 4],
        level: i32,
        quality: u8,
        droppable: bool,
    ) -> Option<UnitId>;
    /// `0x00559A30` (not in the items specs).
    fn drop_item_at(&mut self, unit: UnitId, code: [u8; 4], quality: u8) -> bool;
    fn den_region(&self) -> (u32, u32, u32, u32);
    fn true_tomb_level(&self) -> u32;
    fn free_spot(
        &mut self,
        player: UnitId,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32)>;
    fn create_portal(&mut self, player: UnitId, x: i32, y: i32, class: u16, level: u32) -> bool;
    fn schedule_quest_event(&mut self, object: UnitId, frame: i32);
    /// Object mode (+0x10) and `0x00624690` (objects spec).
    fn object_mode(&self, object: UnitId) -> i32;
    fn set_object_mode(&mut self, object: UnitId, mode: i32);
    fn mercenary_reward(&mut self, player: UnitId, npc: u16);
    fn unhandled(&mut self, chain: u8, function: u32);
}

/// The quests' world: the economy plus the rest.
pub struct EconomyQuests<'e, 'a, H, R> {
    pub econ: &'e mut Economy<'a, H>,
    pub rest: &'e mut R,
    /// Where the mercenary rewards `0x00579180` go when the caller runs
    /// them on the NPC control block after the quest call
    /// ([`crate::wiring::interaction::Desk::quest_message`]); `None`:
    /// [`QuestRest::mercenary_reward`].
    pub mercenaries: Option<&'e mut Vec<(UnitId, u16)>>,
}

impl<'e, 'a, H, R> EconomyQuests<'e, 'a, H, R> {
    pub fn new(econ: &'e mut Economy<'a, H>, rest: &'e mut R) -> Self {
        Self {
            econ,
            rest,
            mercenaries: None,
        }
    }
}

impl<H: LifecycleHooks, R: QuestRest> EconomyQuests<'_, '_, H, R> {
    /// The inventory items with their items records.
    fn inventory_records(&self, player: UnitId) -> Vec<(UnitId, &crate::items::tables::ItemRec)> {
        let t = self.econ.tables;
        self.rest
            .inventory(player)
            .into_iter()
            .filter_map(|i| Some((i, t.item(self.econ.items.get(i)?.record)?)))
            .collect()
    }

    fn of_type(&self, unit: UnitId, ty: UnitType) -> Option<&crate::units::record::UnitRecord> {
        self.econ.units.get(unit).filter(|r| r.ty == ty)
    }
}

impl<H: LifecycleHooks, R: QuestRest> QuestWorld for EconomyQuests<'_, '_, H, R> {
    fn frame(&self) -> i32 {
        self.econ.game.frame
    }
    fn difficulty(&self) -> u8 {
        self.econ.fields.difficulty
    }
    fn expansion(&self) -> bool {
        self.econ.fields.expansion
    }
    fn game_type(&self) -> u8 {
        self.econ.fields.game_type
    }
    fn has_act2(&self) -> bool {
        self.rest.has_act2()
    }

    fn players(&self) -> Vec<UnitId> {
        self.rest.players()
    }
    fn first_client_player(&self) -> Option<UnitId> {
        self.rest.first_client_player()
    }
    /// Unit +0x0C; 0 for a unit without a record.
    fn guid(&self, unit: UnitId) -> u32 {
        self.econ.units.get(unit).map_or(0, |r| r.guid)
    }
    fn player_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.econ.game.lists.find_unit(UnitType::Player, guid)
    }
    fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests> {
        self.rest.quests(player)
    }
    fn unit_act(&self, unit: UnitId) -> Option<u8> {
        self.rest.unit_act(unit)
    }
    fn unit_level(&self, unit: UnitId) -> Option<u32> {
        self.rest.unit_level(unit)
    }
    /// Unit +0x04 of a player.
    fn player_class(&self, player: UnitId) -> u8 {
        self.of_type(player, UnitType::Player)
            .map_or(0, |r| r.class as u8)
    }
    /// Unit +0x20 in the unit record.
    ///
    /// Panics when the unit has no record (the quest code asks only for
    /// live units).
    fn unit_seed(&mut self, unit: UnitId) -> &mut Seed {
        &mut self
            .econ
            .units
            .get_mut(unit)
            .expect("quest unit without a unit record")
            .seed
    }
    /// The unit total, layer 0 (`0x00625480`).
    fn stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.econ.stats.unit_total(unit, stat, 0)
    }
    /// `0x006253B0`, layer 0.
    fn base_stat(&self, unit: UnitId, stat: u16) -> i32 {
        self.econ.stats.unit_base(unit, stat, 0)
    }
    /// `0x006272B0`, layer 0.
    fn add_stat(&mut self, unit: UnitId, stat: u16, delta: i32) {
        let e = &mut *self.econ;
        e.stats.unit_add(e.hooks, unit, stat, delta, 0);
    }
    fn attach_sound(&mut self, player: UnitId, sound: u16) {
        self.rest.attach_sound(player, sound)
    }
    fn player_byte_4c(&self, player: UnitId) -> u8 {
        self.rest.player_byte_4c(player)
    }
    fn set_player_byte_4c(&mut self, player: UnitId, v: u8) {
        self.rest.set_player_byte_4c(player, v)
    }
    fn quest_chain(&mut self, unit: UnitId) -> Option<&mut QuestChain> {
        self.rest.quest_chain(unit)
    }
    fn unit_kind(&self, unit: UnitId) -> UnitKind {
        self.rest.unit_kind(unit)
    }
    fn monster_by_guid(&self, guid: u32) -> Option<(UnitId, u16)> {
        let u = self.econ.game.lists.find_unit(UnitType::Monster, guid)?;
        Some((u, self.monster_class(u)?))
    }
    fn monster_class(&self, unit: UnitId) -> Option<u16> {
        self.of_type(unit, UnitType::Monster)
            .map(|r| r.class as u16)
    }
    fn players_near(&self, unit: UnitId) -> Vec<UnitId> {
        self.rest.players_near(unit)
    }
    fn party_members(&self, player: UnitId) -> Option<Vec<UnitId>> {
        self.rest.party_members(player)
    }

    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.rest.send(player, msg)
    }
    fn send_text_list(&mut self, player: UnitId, npc: UnitId, list: &[(u16, u32)]) {
        self.rest.send_text_list(player, npc, list)
    }

    /// An inventory item whose items record `code` is `code`.
    ///
    /// TODO(quests.md §9.2 `0x00558110`): which of the player's items the
    /// search covers (inventory only, or also equipped / cursor / belt)
    /// is not written; the rest's inventory list is searched.
    fn has_item(&self, player: UnitId, code: [u8; 4]) -> bool {
        self.inventory_records(player)
            .iter()
            .any(|(_, r)| r.code == code)
    }
    fn delete_item(&mut self, player: UnitId, code: [u8; 4]) {
        self.rest.delete_item(player, code)
    }
    fn reward_item(
        &mut self,
        player: UnitId,
        code: [u8; 4],
        level: i32,
        quality: u8,
        droppable: bool,
    ) -> Option<UnitId> {
        self.rest
            .reward_item(player, code, level, quality, droppable)
    }
    fn drop_item_at(&mut self, unit: UnitId, code: [u8; 4], quality: u8) -> bool {
        self.rest.drop_item_at(unit, code, quality)
    }
    /// §4.5: inventory items whose items record `quest` byte ≠ 0, in
    /// inventory order.
    fn quest_items(&self, player: UnitId) -> Vec<(UnitId, u8)> {
        self.inventory_records(player)
            .into_iter()
            .filter(|(_, r)| r.quest != 0)
            .map(|(i, r)| (i, r.quest))
            .collect()
    }

    fn den_region(&self) -> (u32, u32, u32, u32) {
        self.rest.den_region()
    }
    fn true_tomb_level(&self) -> u32 {
        self.rest.true_tomb_level()
    }
    fn free_spot(
        &mut self,
        player: UnitId,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32)> {
        self.rest.free_spot(player, size, mask, radius, limit)
    }
    fn create_portal(&mut self, player: UnitId, x: i32, y: i32, class: u16, level: u32) -> bool {
        self.rest.create_portal(player, x, y, class, level)
    }
    fn schedule_quest_event(&mut self, object: UnitId, frame: i32) {
        self.rest.schedule_quest_event(object, frame)
    }
    fn object_mode(&self, object: UnitId) -> i32 {
        self.rest.object_mode(object)
    }
    fn set_object_mode(&mut self, object: UnitId, mode: i32) {
        self.rest.set_object_mode(object, mode)
    }
    /// `0x00552F60` type 2: the game's unit lists, class from the record.
    fn object_by_guid(&self, guid: u32) -> Option<(UnitId, u16)> {
        let u = self.econ.game.lists.find_unit(UnitType::Object, guid)?;
        Some((u, self.of_type(u, UnitType::Object)?.class as u16))
    }
    fn mercenary_reward(&mut self, player: UnitId, npc: u16) {
        match self.mercenaries.as_mut() {
            Some(q) => q.push((player, npc)),
            None => self.rest.mercenary_reward(player, npc),
        }
    }
    fn unhandled(&mut self, chain: u8, function: u32) {
        self.rest.unhandled(chain, function)
    }
}
