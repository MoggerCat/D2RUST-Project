// Spec: specs/world/quests.md §4.4, §4.5, §9; specs/world/quests-act1-rest.md §8; specs/sim/units.md §2; specs/world/npc.md §2 (the interact info); specs/sim/stat-lists.md §5
// Spec: specs/world/quests-helpers.md §8 (item search); specs/world/quests-act2-2.md §5.2–§5.4
//! [`QuestWorld`] on the real providers: game fields ([`GameFields`]),
//! the frame and unit lookups ([`crate::game::Game`]), unit records
//! (GUID, class, unit seed), stat lists (stat reads and adds) and the
//! item data (item codes, `quest` bytes). Everything else stays a seam:
//! [`QuestRest`].
//!
//! [`GameFields`]: super::GameFields

use super::Economy;
use crate::game::Game;
use crate::rng::Seed;
use crate::stats::StatLists;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{RoomId, UnitId, UnitType};
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
    /// `quests-act1.md` §10.5 J3's room test (DRLG rooms).
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
    /// The player's cursor item (`0x0063C1E0`). Default: none.
    fn quest_cursor_item(&self, player: UnitId) -> Option<UnitId> {
        let _ = player;
        None
    }
    /// `0x00544160` (`quests.md` §9.2: removal by item mode, inventory).
    fn delete_item(&mut self, player: UnitId, code: [u8; 4]);
    /// `0x005466B0` (`quests.md` §9.1) for a host that lends no inventory
    /// model: the wired host's runs on [`super::HostQuests`] with one
    /// ([`super::quest_reward`]).
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
    /// Act I quest seams (`quests-act1.md` §10.6–§10.8: paths, rooms,
    /// monsters, objects; `QuestWorld` documents each).
    fn unit_position(&self, unit: UnitId) -> Option<(i32, i32, RoomId)>;
    fn room_contains(&self, room: RoomId, x: i32, y: i32) -> bool;
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId>;
    #[allow(clippy::too_many_arguments)]
    fn free_spot_at(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32, RoomId)>;
    #[allow(clippy::too_many_arguments)]
    fn spawn_monster(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        r: u32,
    ) -> Option<UnitId>;
    fn or_unit_flags(&mut self, unit: UnitId, flags: u32);
    fn monsters(&self) -> Vec<UnitId>;
    fn npc_chat_clients(&self, npc: UnitId) -> Option<Vec<UnitId>>;
    fn remove_monster(&mut self, monster: UnitId);
    fn drop_preset_monster(&mut self, act: u8, class: u16);
    fn find_object_near(&self, object: UnitId, class: u16) -> Option<UnitId>;
    fn create_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId>;
    fn object_anim_length(&self, object: UnitId) -> i32;
    fn schedule_object_event(&mut self, object: UnitId, ev: u8, frame: i32);
    fn open_quest_message(&mut self, player: UnitId, object: UnitId, msg: u16);
    fn unhandled(&mut self, chain: u8, function: u32);

    // Act I remainder seams (`quests-act1-rest.md`; `QuestWorld`
    // documents each). A host that does not provide one reports the 1.14d
    // function through `unhandled` (chain 0xFF) and fails.
    #[allow(clippy::too_many_arguments)]
    fn spawn_monster_flags(
        &mut self,
        _room: RoomId,
        _x: i32,
        _y: i32,
        _class: u16,
        _mode: u8,
        _spread: i32,
        _flags: u32,
    ) -> Option<UnitId> {
        self.unhandled(0xFF, 0x005B_2F20);
        None
    }
    #[allow(clippy::too_many_arguments)]
    fn open_portal(
        &mut self,
        _owner: Option<UnitId>,
        _room: RoomId,
        _x: i32,
        _y: i32,
        _level: u32,
        _class: u16,
        _exact: bool,
    ) -> Option<UnitId> {
        self.unhandled(0xFF, 0x0056_D130);
        None
    }
    fn create_missile(
        &mut self,
        _owner: UnitId,
        _skill: u16,
        _level: u8,
        _class: u16,
        _x: i32,
        _y: i32,
    ) -> Option<UnitId> {
        self.unhandled(0xFF, 0x0056_EDE0);
        None
    }
    fn set_missile_target(&mut self, _missile: UnitId, _a: u32, _b: u32) {
        self.unhandled(0xFF, 0x0064_A710);
    }
    fn refresh_room(&mut self, _unit: UnitId) {
        self.unhandled(0xFF, 0x0061_AED0);
    }
    fn spawn_object(
        &mut self,
        _room: RoomId,
        _x: i32,
        _y: i32,
        _class: u16,
        _mode: i32,
    ) -> Option<UnitId> {
        self.unhandled(0xFF, 0x0055_5230);
        None
    }
    fn client_save_flags(&self, _player: UnitId) -> Option<u16> {
        None
    }
    fn set_client_save_flags(&mut self, _player: UnitId, _flags: u16) {
        self.unhandled(0xFF, 0x0053_8680);
    }
}

/// The quests' world: the economy plus the rest.
pub struct EconomyQuests<'e, 'a, H, R> {
    pub econ: &'e mut Economy<'a, H>,
    pub rest: &'e mut R,
    /// Where the mercenary rewards `0x00579180` go when the caller runs
    /// them on the NPC control block after the quest call
    /// ([`crate::wiring::interaction::Desk::quest_message`]), with the
    /// sends that follow a reward in the call; `None`:
    /// [`QuestRest::mercenary_reward`].
    pub deferred: Option<&'e mut Vec<QuestDeferred>>,
    /// The cursor item and item list of the host's inventory model, for
    /// the held-item tests ([`Self::find_item`]); `None`: the rest's.
    pub held: Option<(Option<UnitId>, Vec<UnitId>)>,
}

/// A quest-call effect the caller runs after the call, in list order
/// ([`EconomyQuests::deferred`]).
///
/// The reward needs the NPC control block, which the quest call cannot
/// reach, so it is queued; every send (`send`, `send_text_list`) the call
/// makes after a queued reward is queued behind it, so the reward's
/// messages (its 0x50 and the hireling's creation messages) precede them
/// as in 1.14d (`quests-act1-rest.md` §8 item 8: Kashya's message 92
/// sends 0x28, rewards, then refreshes the text, 0x27 / 0x29). Other
/// effects are not queued: none follows the reward in the 1.14d call,
/// and the reward reads no quest state, nor the refresh hireling state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestDeferred {
    /// `0x00579180(npc)` for `player`.
    Mercenary { player: UnitId, npc: u16 },
    /// [`QuestRest::send`].
    Send { player: UnitId, msg: Vec<u8> },
    /// [`QuestRest::send_text_list`].
    TextList {
        player: UnitId,
        npc: UnitId,
        list: Vec<(u16, u32)>,
    },
}

impl QuestDeferred {
    /// Sends a queued message through `rest`; a mercenary reward is
    /// handed back for the caller to run on the NPC control block.
    pub fn run<R: QuestRest + ?Sized>(self, rest: &mut R) -> Option<(UnitId, u16)> {
        match self {
            Self::Mercenary { player, npc } => return Some((player, npc)),
            Self::Send { player, msg } => rest.send(player, &msg),
            Self::TextList { player, npc, list } => rest.send_text_list(player, npc, &list),
        }
        None
    }
}

/// "Every player" of the quest code, `0x005537D0(game, 0, …)`
/// (`quests-act1-rest.md` §8 item 9): the player hash walk, buckets
/// 0 … 127 each from its head (`unit-order.md` §2 r4), without the
/// players in state 7 (`0x00639DF0`, §2 r5). The early stop at a callback
/// returning 1 is the caller's.
pub fn quest_players(game: &Game, stats: &StatLists) -> Vec<UnitId> {
    let mut out = game.lists.units_of_type(UnitType::Player);
    out.retain(|&u| !stats.has_state(u, 7));
    out
}

impl<'e, 'a, H, R> EconomyQuests<'e, 'a, H, R> {
    pub fn new(econ: &'e mut Economy<'a, H>, rest: &'e mut R) -> Self {
        Self {
            econ,
            rest,
            deferred: None,
            held: None,
        }
    }

    /// The queue when a reward is already in it (the sends then follow
    /// it, [`QuestDeferred`]).
    fn behind_reward(&mut self) -> Option<&mut Vec<QuestDeferred>> {
        self.deferred.as_deref_mut().filter(|q| !q.is_empty())
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

    /// `0x00558110(game, player, code)` (`quests-helpers.md` §8): the
    /// cursor item first (`questdiffcheck` not tested there), then the
    /// inventory list in order, items on page 1 (trade) skipped. A quest
    /// item passes when its record's `quest` is 0, or (list only)
    /// `questdiffcheck` is 0, or its stat 356 (`questitemdifficulty`,
    /// total value) ≥ the game difficulty.
    pub fn find_item(&self, player: UnitId, code: [u8; 4]) -> Option<UnitId> {
        if let Some((cursor, list)) = &self.held {
            return self.find_item_in(*cursor, list.clone(), code);
        }
        self.find_item_in(
            self.rest.quest_cursor_item(player),
            self.rest.inventory(player),
            code,
        )
    }

    /// [`Self::find_item`] over a given cursor item and item list (a host
    /// with an inventory model lends its own).
    pub fn find_item_in(
        &self,
        cursor: Option<UnitId>,
        list: Vec<UnitId>,
        code: [u8; 4],
    ) -> Option<UnitId> {
        const QUEST_ITEM_DIFFICULTY: u16 = 356;
        let t = self.econ.tables;
        let d = i32::from(self.econ.fields.difficulty);
        let rec = |i: UnitId| t.item(self.econ.items.get(i)?.record);
        let stat = |i: UnitId| self.econ.stats.unit_total(i, QUEST_ITEM_DIFFICULTY, 0);
        if let Some(c) = cursor {
            if let Some(r) = rec(c).filter(|r| r.code == code) {
                if r.quest == 0 || stat(c) >= d {
                    return Some(c);
                }
            }
        }
        list.into_iter().find(|&i| {
            let Some(item) = self.econ.items.get(i) else {
                return false;
            };
            if item.inv_page == 1 {
                return false;
            }
            rec(i).is_some_and(|r| {
                r.code == code && (r.quest == 0 || r.questdiffcheck == 0 || stat(i) >= d)
            })
        })
    }
}

/// `quests-act2-2.md` §5.2 `0x0061C450`: the Tainted Sun start on the act
/// environment record (speed 4; the 0x53 fields: index 0, ticks 300 × 4
/// from the period reset `0x0061BDF0` of the eclipse table's entry 0,
/// eclipse 1; type 3).
pub fn tainted_sun_start(e: &mut crate::world::environment::Environment) {
    use crate::world::environment::{ECLIPSE, SPEED_ECLIPSE};
    e.speed = SPEED_ECLIPSE;
    e.period = 0;
    e.kind = ECLIPSE[0].kind;
    e.ticks = ECLIPSE[0].start * SPEED_ECLIPSE;
    e.eclipse = true;
}

/// `quests-act2-2.md` §5.2 `0x0061C4D0`: the Tainted Sun end (speed 128,
/// index 2, ticks 0, the normal entry 2 type, eclipse 0; no period
/// reset).
pub fn tainted_sun_end(e: &mut crate::world::environment::Environment) {
    use crate::world::environment::{NORMAL, SPEED_NORMAL};
    e.speed = SPEED_NORMAL;
    e.period = 2;
    e.kind = NORMAL[2].kind;
    e.ticks = 0;
    e.eclipse = false;
}

impl<H: LifecycleHooks, R: QuestRest> QuestWorld for EconomyQuests<'_, '_, H, R> {
    fn frame(&self) -> i32 {
        self.econ.game.frame
    }
    /// `0x00554100` on the player's unit record (+0x64 / +0x68 / +0x6C,
    /// [`crate::units::record::InteractInfo`]); no record → none.
    fn interact_unit(&mut self, player: UnitId) -> Option<(u8, u32)> {
        self.econ.units.get(player)?.interact.get()
    }
    /// `0x00554120` (`Some`, ignored while active) / `0x00554190`
    /// (`None`) on the player's unit record.
    fn set_interact_unit(&mut self, player: UnitId, unit: Option<(u8, u32)>) {
        if let Some(r) = self.econ.units.get_mut(player) {
            match unit {
                Some((t, guid)) => r.interact.set(t, guid),
                None => r.interact.reset(),
            }
        }
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

    /// [`quest_players`] on the game's lists and stat lists.
    fn players(&self) -> Vec<UnitId> {
        quest_players(self.econ.game, self.econ.stats)
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
        match self.behind_reward() {
            Some(q) => q.push(QuestDeferred::Send {
                player,
                msg: msg.to_vec(),
            }),
            None => self.rest.send(player, msg),
        }
    }
    fn send_text_list(&mut self, player: UnitId, npc: UnitId, list: &[(u16, u32)]) {
        match self.behind_reward() {
            Some(q) => q.push(QuestDeferred::TextList {
                player,
                npc,
                list: list.to_vec(),
            }),
            None => self.rest.send_text_list(player, npc, list),
        }
    }

    /// `0x00558110` (`quests-helpers.md` §8): the player holds an item of
    /// `code` ([`EconomyQuests::find_item`]).
    fn has_item(&self, player: UnitId, code: [u8; 4]) -> bool {
        self.find_item(player, code).is_some()
    }
    /// The item's items record code (`0x00628590`).
    fn item_code(&self, item: UnitId) -> Option<[u8; 4]> {
        let rec = self.econ.items.get(item)?.record;
        Some(self.econ.tables.item(rec)?.code)
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
        match self.deferred.as_mut() {
            Some(q) => q.push(QuestDeferred::Mercenary { player, npc }),
            None => self.rest.mercenary_reward(player, npc),
        }
    }
    fn unit_position(&self, unit: UnitId) -> Option<(i32, i32, RoomId)> {
        self.rest.unit_position(unit)
    }
    fn room_contains(&self, room: RoomId, x: i32, y: i32) -> bool {
        self.rest.room_contains(room, x, y)
    }
    fn room_at(&self, room: RoomId, x: i32, y: i32) -> Option<RoomId> {
        self.rest.room_at(room, x, y)
    }
    #[allow(clippy::too_many_arguments)]
    fn free_spot_at(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        size: u32,
        mask: u32,
        radius: u32,
        limit: u32,
    ) -> Option<(i32, i32, RoomId)> {
        self.rest
            .free_spot_at(room, x, y, size, mask, radius, limit)
    }
    #[allow(clippy::too_many_arguments)]
    fn spawn_monster(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        r: u32,
    ) -> Option<UnitId> {
        self.rest.spawn_monster(room, x, y, class, mode, r)
    }
    fn or_unit_flags(&mut self, unit: UnitId, flags: u32) {
        self.rest.or_unit_flags(unit, flags)
    }
    fn monsters(&self) -> Vec<UnitId> {
        self.rest.monsters()
    }
    fn npc_chat_clients(&self, npc: UnitId) -> Option<Vec<UnitId>> {
        self.rest.npc_chat_clients(npc)
    }
    fn remove_monster(&mut self, monster: UnitId) {
        self.rest.remove_monster(monster)
    }
    fn drop_preset_monster(&mut self, act: u8, class: u16) {
        self.rest.drop_preset_monster(act, class)
    }
    fn find_object_near(&self, object: UnitId, class: u16) -> Option<UnitId> {
        self.rest.find_object_near(object, class)
    }
    fn create_object(&mut self, room: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        self.rest.create_object(room, x, y, class)
    }
    fn object_anim_length(&self, object: UnitId) -> i32 {
        self.rest.object_anim_length(object)
    }
    fn schedule_object_event(&mut self, object: UnitId, ev: u8, frame: i32) {
        self.rest.schedule_object_event(object, ev, frame)
    }
    /// `0x005456A0` (`quests-act2-2.md` §5.4): S→C 0x27 type 2 with the
    /// object's GUID and the string, to the player's client.
    fn open_quest_message(&mut self, player: UnitId, object: UnitId, msg: u16) {
        let g = self.guid(object);
        self.send(
            player,
            &crate::world::quests::helpers::msg_scroll_text(g, msg),
        );
    }
    fn unhandled(&mut self, chain: u8, function: u32) {
        self.rest.unhandled(chain, function)
    }
    fn spawn_monster_flags(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: u8,
        spread: i32,
        flags: u32,
    ) -> Option<UnitId> {
        self.rest
            .spawn_monster_flags(room, x, y, class, mode, spread, flags)
    }
    fn open_portal(
        &mut self,
        owner: Option<UnitId>,
        room: RoomId,
        x: i32,
        y: i32,
        level: u32,
        class: u16,
        exact: bool,
    ) -> Option<UnitId> {
        self.rest
            .open_portal(owner, room, x, y, level, class, exact)
    }
    fn create_missile(
        &mut self,
        owner: UnitId,
        skill: u16,
        level: u8,
        class: u16,
        x: i32,
        y: i32,
    ) -> Option<UnitId> {
        self.rest.create_missile(owner, skill, level, class, x, y)
    }
    fn set_missile_target(&mut self, missile: UnitId, a: u32, b: u32) {
        self.rest.set_missile_target(missile, a, b)
    }
    fn refresh_room(&mut self, unit: UnitId) {
        self.rest.refresh_room(unit)
    }
    fn spawn_object(
        &mut self,
        room: RoomId,
        x: i32,
        y: i32,
        class: u16,
        mode: i32,
    ) -> Option<UnitId> {
        self.rest.spawn_object(room, x, y, class, mode)
    }
    fn client_save_flags(&self, player: UnitId) -> Option<u16> {
        self.rest.client_save_flags(player)
    }
    fn set_client_save_flags(&mut self, player: UnitId, flags: u16) {
        self.rest.set_client_save_flags(player, flags)
    }

    /// `0x0061C450(act)` (`quests-act2-2.md` §5.2) on the act's
    /// environment record; a missing act (fatal 0x547) is reported.
    fn start_tainted_sun(&mut self, act: u8) {
        match self.econ.game.lists.act_mut(act) {
            Some(a) => tainted_sun_start(&mut a.environment),
            None => self.rest.unhandled(0xFF, 0x0061_C450),
        }
    }
    /// `0x0061C4D0(act)` on Act II's environment record.
    fn end_tainted_sun(&mut self) {
        match self.econ.game.lists.act_mut(1) {
            Some(a) => tainted_sun_end(&mut a.environment),
            None => self.rest.unhandled(0xFF, 0x0061_C4D0),
        }
    }
    /// `0x0052E050` (`quests-act2-2.md` §5.3): S→C 0x0A to every in-game
    /// client whose room's adjacent-room list (the room included) holds
    /// the unit's room (none for a missile), then the unit is freed
    /// (`0x00555600`).
    fn remove_unit(&mut self, unit: UnitId) {
        let lists = &self.econ.game.lists;
        let Some(entry) = lists.unit(unit) else {
            return;
        };
        let (ty, guid, room) = (entry.ty, entry.guid, entry.room());
        let mut to = Vec::new();
        let mut missing_room = false;
        for c in lists.clients() {
            let Some(ce) = lists.client(c) else { continue };
            if ce.state != crate::units::lists::client_state::IN_GAME {
                continue;
            }
            let Some(cr) = ce.room else {
                // Fatal 0x15BE in 1.14d.
                missing_room = true;
                continue;
            };
            // `0x00619790`: the client room's list, the room included.
            let near = room.is_some_and(|u| {
                lists
                    .room(cr)
                    .is_some_and(|r| u == cr || r.adjacent.contains(&u))
            });
            if near && ty != UnitType::Missile {
                if let Some(p) = ce.player {
                    to.push(p);
                }
            }
        }
        if missing_room {
            self.rest.unhandled(0xFF, 0x0052_DFB0);
        }
        for p in to {
            self.send(p, &crate::units::messages::remove_unit(ty as u8, guid));
        }
        let (mut sim, hooks) = self.econ.split();
        if crate::units::lifecycle::remove(&mut sim, hooks, unit).is_err() {
            self.rest.unhandled(0xFF, 0x0055_5600);
        }
    }
}

#[cfg(test)]
mod sun_tests {
    use super::{tainted_sun_end, tainted_sun_start};
    use crate::world::environment::Environment;

    // Covers: specs/world/quests-act2-2.md §5.2; specs/world/quests-act2.md §5.9 r2, §5.9 r4, §5.9 l2 r2, §5.9 l2 r3
    #[test]
    fn tainted_sun_start_and_end_on_the_record() {
        let mut e = Environment::CREATED;
        tainted_sun_start(&mut e);
        // Index 0, ticks 300 × 4 from the period reset, eclipse 1.
        assert_eq!(e.message(), [0x53, 0, 0, 0, 0, 0xB0, 0x04, 0, 0, 1]);
        tainted_sun_end(&mut e);
        assert_eq!(e, Environment::CREATED);
    }
}
