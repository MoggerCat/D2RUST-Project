// Spec: specs/items/inventory-moves.md
// Spec: specs/items/inventory.md (the sections other than §6–§11)
//! Seams of the item-move code (`inventory-moves.md` §6–§11).
//!
//! - [`InventoryOps`]: exactly the `inventory.md` §1–§5 operations the
//!   handlers call (provider: `items::inventory`, parallel session). No
//!   defaults: every method is a rule of that spec.
//! - [`MoveUnits`]: unit record fields (`sim/units.md`, `items/generation.md`
//!   §1.3–§1.4, `sim/stats.md`). No defaults (provider: the units and item
//!   records through the wiring).
//! - [`MovePending`]: calls into systems whose owner spec is unwritten or
//!   not wired yet. Each default is the narrowest reading (nothing happens,
//!   or the value that makes the caller do nothing); when a provider lands,
//!   its methods move into the adapter that wires it.

use crate::units::RoomId;

use super::{Guid, Owner};

/// A ground position found by the free-spot search (`sim/path-placement.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub room: RoomId,
    pub x: i32,
    pub y: i32,
}

/// The `inventory.md` §1–§5 operations this module calls, by the spec's
/// names. Item checks return the check's result code (0 = pass).
pub trait InventoryOps {
    // ---- §1.4 item list, update list, cursor ---------------------------

    /// Whether the unit has an inventory record.
    fn has_inventory(&self, owner: Owner) -> bool;
    /// Cursor item (`0x0063C1E0`).
    fn cursor(&self, owner: Owner) -> Option<Guid>;
    /// Set the cursor (`0x0063C180`).
    fn set_cursor(&mut self, owner: Owner, item: Option<Guid>);
    /// The item list in link order (§1.4 rule 1).
    fn items(&self, owner: Owner) -> Vec<Guid>;
    /// Unlink (`0x0063AAF0`); false when the item is missing or not in
    /// `owner`'s inventory (callers treat that as fatal where the spec says).
    fn unlink(&mut self, owner: Owner, item: Guid) -> bool;
    /// The update list, in order (§1.4 rule 2).
    fn update_list(&self, owner: Owner) -> Vec<Guid>;
    /// Append to the update list unless listed (`0x0063CC70`).
    fn update_list_add(&mut self, owner: Owner, item: Guid);
    /// Free the update list (`0x0063CBD0`, §6.1 rule 4).
    fn update_list_free(&mut self, owner: Owner);
    /// GUID of the weapon in use (inventory +0x1C, `0x0063BEF0`).
    fn weapon_in_use(&self, owner: Owner) -> Option<Guid>;

    // ---- §2 grid placement ----------------------------------------------

    /// Place at a position (§2.2, `0x0063BCC0`) on `page`.
    fn place_at(&mut self, owner: Owner, item: Guid, page: u8, x: i32, y: i32) -> bool;
    /// Free-position search (§2.3) on `page`.
    fn find_free(&self, owner: Owner, item: Guid, page: u8) -> Option<(i32, i32)>;
    /// Item placement into a page (§2.4 `0x00560200`), steps 2–9 (the
    /// caller does step 1: owner check and targeting reset). The page is
    /// the item's page field.
    fn place_in_page(
        &mut self,
        owner: Owner,
        item: Guid,
        x: i32,
        y: i32,
        find: bool,
        send: bool,
    ) -> bool;
    /// Link check `0x0063B210(inv, item, kind)` (§2.4 step 5).
    fn link_check(&mut self, owner: Owner, item: Guid, kind: u8) -> bool;
    /// Link a socket filler into an item's inventory (`0x0063B210` on
    /// the item's inventory, created first; §7.19 step 3).
    fn link_into_item(&mut self, target: Guid, filler: Guid) -> bool;

    // ---- §3 belt --------------------------------------------------------

    /// Beltable (§3.3).
    fn beltable(&self, item: Guid) -> bool;
    /// Auto-belt gate (§3.6).
    fn auto_belt_gate(&self, owner: Owner, item: Guid) -> bool;
    /// Free slot for an item (§3.5).
    fn belt_free_slot(&self, owner: Owner, item: Guid) -> Option<u8>;
    /// Place in a slot (§3.7).
    fn belt_place(&mut self, owner: Owner, item: Guid, slot: u32) -> bool;
    /// Compaction after a slot is emptied (§3.8).
    fn belt_compact(&mut self, owner: Owner, slot: u8);
    /// The item in a belt slot (`0x0063C7F0`, grid 1).
    fn belt_item(&self, owner: Owner, slot: u8) -> Option<Guid>;
    /// `numboxes` of the belt type of `belt` (`0x00621ED0`), or of record
    /// 2 without one (§3 rule 9).
    fn belt_boxes(&self, belt: Option<Guid>) -> u8;
    /// The grid item list of page `page` (grid page + 2), in grid list
    /// order (item data +0x70 next; §8.1 step 4).
    fn page_items(&self, owner: Owner, page: u8) -> Vec<Guid>;
    /// The body-location grid's item list (grid 0, `0x0063C2F0`), in grid
    /// list order.
    fn body_items(&self, owner: Owner) -> Vec<Guid>;

    // ---- §4 equipping ---------------------------------------------------

    /// The item at a body location (`0x0063DD90`).
    fn body_item(&self, owner: Owner, loc: u8) -> Option<Guid>;
    /// Put at a body location (`0x0063BDB0` = §2.2 on grid 0).
    fn place_body(&mut self, owner: Owner, item: Guid, loc: u8) -> bool;
    /// Clear a body slot (`0x0063BE30`).
    fn clear_body_slot(&mut self, owner: Owner, loc: u8);
    /// The item to remove from a location (`0x0063E490`; for equip check
    /// result 4: the two-handed item in the other hand).
    fn item_to_remove(&self, owner: Owner, loc: u8) -> Option<Guid>;
    /// Two-handed (`0x006289C0`).
    fn two_handed(&self, item: Guid) -> bool;
    /// Requirements (§4.2).
    fn requirements(&self, item: Guid, unit: Owner, equipping: bool) -> bool;
    /// Equip check (§4.3).
    fn equip_check(&self, unit: Owner, loc: u8, item: Option<Guid>, skip: bool) -> u8;
    /// Stack test (§4.5).
    fn stack_test(&self, a: Guid, b: Guid) -> bool;
    /// Equip from the cursor (§4.6): (result 1, out).
    fn equip_from_cursor(&mut self, player: Owner, item: Guid, loc: u8, skip: bool)
        -> (bool, bool);
    /// Auto-equip on pickup (§4.7): the location, or none.
    fn auto_equip(&self, unit: Owner, item: Guid, skip: bool) -> Option<u8>;

    // ---- §5 shared checks -----------------------------------------------

    /// Cursor item check (`0x005490E0`).
    fn check_cursor_item(&self, player: Owner, item: Guid) -> u32;
    /// Stored item check (`0x00549150`).
    fn check_stored(&self, player: Owner, item: Guid) -> u32;
    /// Stored-or-equipped check (`0x005491B0`).
    fn check_stored_or_equipped(&self, player: Owner, item: Guid) -> u32;
    /// Owned item check (`0x00549220`).
    fn check_owned(&self, player: Owner, item: Guid) -> u32;
    /// Belt item check (`0x005492F0`).
    fn check_belt(&self, player: Owner, item: Guid) -> u32;
    /// Ground-or-owned check (`0x00549350`).
    fn check_ground_or_owned(&self, player: Owner, item: Guid) -> u32;
    /// Busy (§5.2, `0x00535060`).
    fn busy(&self, player: Owner) -> bool;
    /// Trading (§5.2, `0x005678A0`).
    fn trading(&self, player: Owner) -> bool;
    /// Targeting reset (§5.3).
    fn targeting_reset(&mut self, player: Owner);
    /// Item-move gate (§5.4); true = allowed.
    fn item_move_gate(&mut self, player: Owner, item: Option<Guid>) -> bool;
}

/// Unit record fields read or written here.
pub trait MoveUnits {
    /// Unit lookup by type and GUID.
    fn unit_exists(&self, u: Owner) -> bool;
    /// Unit class (player class, monster class id).
    fn unit_class(&self, u: Owner) -> u32;
    /// Unit mode (unit +0x10; 17 = dead for a player).
    fn unit_mode(&self, u: Owner) -> u32;
    /// Position (subtiles).
    fn pos(&self, u: Owner) -> (i32, i32);
    fn set_pos(&mut self, u: Owner, x: i32, y: i32);
    /// Unit flags (unit +0xC4).
    fn unit_flags(&self, u: Owner) -> u32;
    fn set_unit_flags(&mut self, u: Owner, v: u32);
    /// Update bits (unit +0xC8; §6.1).
    fn update_bits(&self, u: Owner) -> u32;
    fn set_update_bits(&mut self, u: Owner, v: u32);
    /// Unit total of a stat (layer 0).
    fn stat(&self, u: Owner, id: u16) -> i32;
    /// Set a base stat (layer 0).
    fn set_stat(&mut self, u: Owner, id: u16, v: i32);
    /// Game +0x70.
    fn expansion(&self) -> bool;
    /// Game frame (game +0xA8).
    fn frame(&self) -> i32;

    // ---- item data (`inventory.md` §1.1, `generation.md` §1.3–§1.4) ----

    fn mode(&self, item: Guid) -> u8;
    fn set_mode(&mut self, item: Guid, m: u8);
    fn page(&self, item: Guid) -> u8;
    fn set_page(&mut self, item: Guid, p: u8);
    fn stored_page(&self, item: Guid) -> u8;
    fn set_stored_page(&mut self, item: Guid, p: u8);
    fn body_loc(&self, item: Guid) -> u8;
    fn set_body_loc(&mut self, item: Guid, loc: u8);
    fn cmd_flags(&self, item: Guid) -> u32;
    fn set_cmd_flags(&mut self, item: Guid, v: u32);
    fn item_flags(&self, item: Guid) -> u32;
    fn set_item_flags(&mut self, item: Guid, v: u32);
    /// Ground expiry (item data +0x24, §9.2).
    fn set_expiry(&mut self, item: Guid, frame: i32);
    /// The unit owning the item's inventory (item data +0x5C).
    fn item_owner(&self, item: Guid) -> Option<Owner>;
    /// Itemtypes test with equivalence (`0x00629BB0`).
    fn is_type(&self, item: Guid, ty: u16) -> bool;
    /// Primary type (`0x0062B400`: items `type`, no equivalence).
    fn primary_type(&self, item: Guid) -> u16;
    /// Stackable (`0x006289F0`).
    fn stackable(&self, item: Guid) -> bool;
    /// Itemtypes `autostack` of the primary type (`0x0062E790`).
    fn autostack(&self, item: Guid) -> bool;
    /// Itemtypes `quiver` of the primary type ≠ 0 (`0x0062E740`).
    fn quiver(&self, item: Guid) -> bool;
    /// Items code.
    fn code(&self, item: Guid) -> [u8; 4];
    /// Quality (item data +0).
    fn quality(&self, item: Guid) -> u8;
    /// Unique / set file index (−1 none).
    fn file_index(&self, item: Guid) -> i32;
    /// Items `quest` (+0x12A).
    fn quest(&self, item: Guid) -> u8;
    /// Items `useable` (+0x11D).
    fn useable(&self, item: Guid) -> bool;
    /// Items `component` (+0x115).
    fn component(&self, item: Guid) -> u8;
    /// Max stack (`0x006295B0`, `generation.md` §1.3).
    fn max_stack(&self, item: Guid) -> i32;
    /// Socketed with fillers (`0x0055F590`).
    fn socket_filled(&self, item: Guid) -> bool;
    /// Socket-filler test (`0x0062BEB0`).
    fn socket_filler(&self, item: Guid) -> bool;
    /// Sockets (`0x006299B0`).
    fn sockets(&self, item: Guid) -> i32;
    /// Fillers in the item's inventory, in link order (`0x0063CD60` counts them).
    fn fillers(&self, item: Guid) -> Vec<Guid>;
    /// Book / scroll spell (`0x00627F80`).
    fn spell(&self, item: Guid) -> i32;
}

/// Calls without a written or wired owner (see the module doc).
#[allow(unused_variables)]
pub trait MovePending {
    // ---- path and placement (`sim/path-placement.md`, being written) ----

    /// Unit-to-unit distance (`0x00641530`). Default: out of range.
    fn distance(&self, a: Owner, b: Owner) -> i32 {
        i32::MAX
    }
    /// Collision between two units on `mask` (`0x00622B50`).
    fn collides(&self, a: Owner, b: Owner, mask: u32) -> bool {
        false
    }
    /// Walk to an item (`0x00548A50`; arrival is the movement spec's).
    fn walk_to_item(&mut self, player: Owner, item: Guid, cursor: bool) {}
    /// Walk to a player or tile unit (`0x00548A50`, §7.1 types 0 and 5).
    fn walk_to_unit(&mut self, player: Owner, target: Owner, cursor: bool) {}
    /// Warp through a tile (`0x005550B0`, `sim/path-placement.md` §12.2;
    /// §7.1 type 5).
    fn tile_warp(&mut self, player: Owner, tile: Owner) {}
    /// Whether a room exists at (x, y) (`0x00463740`).
    fn room_at(&self, x: i32, y: i32) -> bool {
        false
    }
    /// Free-spot search `0x0064E810(room, start, origin, size, mask, mask2, last)`.
    fn free_spot(
        &self,
        start: (i32, i32),
        origin: (i32, i32),
        size: u32,
        mask: u32,
        mask2: u32,
        last: u32,
    ) -> Option<Spot> {
        None
    }
    /// The player's room is in a town level (`0x0061AB00`).
    fn in_town(&self, player: Owner) -> bool {
        false
    }

    // ---- rooms (`sim/units.md`, `unit-order.md`) ------------------------

    /// Room delete notice (`0x0061A270`).
    fn room_delete_notice(&mut self, item: Guid) {}
    /// Collision freed (`0x00623830`).
    fn free_collision(&mut self, item: Guid) {}
    /// Room list removal (`0x0064C370`); a no-op for an item in no room.
    fn remove_from_room(&mut self, item: Guid) {}
    /// Room added at a spot (ground placement `0x00558AA0`).
    fn add_to_room(&mut self, item: Guid, spot: Spot) {}
    /// Whether the item is in a room.
    fn in_room(&self, item: Guid) -> bool {
        false
    }
    /// Room-change notice with the item's old cell (`0x0063BCF0`).
    fn room_change_notice(&mut self, item: Guid, x: i32, y: i32) {}
    /// Queue the unit for update (`unit-order.md` §6).
    fn queue_update(&mut self, u: Owner) {}

    // ---- stat lists, item-skill link, inventory pass (§5.5–§5.7) ---------

    /// Stat refresh `0x0055C2C0(owner, 0)`.
    fn stat_refresh(&mut self, u: Owner) {}
    /// Stat refresh `0x0055C730(owner, 0, b)`.
    fn stat_refresh_unlink(&mut self, u: Owner, b: u32) {}
    /// Stat link `0x0063D1D0`.
    fn stat_link(&mut self, owner: Owner, item: Guid) {}
    /// Item-skill link `0x0055C270` (§5.5).
    fn charm_relink(&mut self, owner: Owner, item: Guid) {}
    /// Item-skill unlink `0x0055C6E0` (§5.5).
    fn charm_unlink(&mut self, owner: Owner, item: Guid) {}
    /// Active inventory item for its owner (`0x0062FF70`, §5.6).
    fn is_active(&self, owner: Owner, item: Guid) -> bool {
        false
    }
    /// Inventory pass `0x0055DBC0(0)` (§5.7).
    fn inventory_pass(&mut self, owner: Owner) {}
    /// Weapon-in-use update `0x006233A0`.
    fn weapon_in_use_update(&mut self, owner: Owner) {}
    /// Weapon bookkeeping `0x0055C5C0`.
    fn weapon_bookkeeping(&mut self, owner: Owner) {}
    /// Body leave effects `0x0062A360` and `0x0063D2B0` (§7.6).
    fn body_leave_effects(&mut self, owner: Owner, item: Guid) {}
    /// Hireling owner pass (§6.1 rule 3: `0x0063EE90` → inventory pass,
    /// `0x0055F4F0(1)`).
    fn hireling_owner_pass(&mut self, owner: Owner) {}

    // ---- belt removal gate (§3 rule 10) -----------------------------------

    /// `0x00567840` (§3 rule 10): may the belt be removed. Default: no.
    fn belt_remove_allowed(&self, player: Owner) -> bool {
        false
    }

    // ---- sounds ---------------------------------------------------------

    /// Sound event on a unit (`0x00553380` family).
    fn sound(&mut self, u: Owner, id: u32) {}
    /// Pickup sound on the player (`0x00553380`).
    fn pickup_sound(&mut self, player: Owner, item: Guid) {}
    /// Sound of a refused requirement (§7.6, §7.8; id not written).
    fn requirement_sound(&mut self, player: Owner) {}
    /// Sound of a hireling give (§7.23; id not written).
    fn merc_sound(&mut self, player: Owner) {}

    // ---- quests (`world/quests.md`) -------------------------------------

    /// Quest flag of the player's record for the current difficulty
    /// (`0x00543520` / `0x0065C310`). Default: clear.
    fn quest_flag(&self, player: Owner, quest: u8, flag: u8) -> bool {
        false
    }
    /// Hook ITEMPICKEDUP (`0x00543D80`).
    fn quest_item_picked(&mut self, player: Owner, item: Guid) {}
    /// Hook ITEMDROPPED (`0x00543DB0`).
    fn quest_item_dropped(&mut self, item: Guid) {}
    /// Carry-one unique: the record flag +0x2C has the bit of `0x006CE270`.
    fn carry_one(&self, item: Guid) -> bool {
        false
    }
    /// The player's corpses (the inventory's +0x34 list, `0x0063D570`,
    /// each node's GUID looked up as a player unit; §8.4 rule 6), whose
    /// item lists the held test also walks. Default: none.
    fn held_test_units(&self, player: Owner) -> Vec<Owner> {
        Vec::new()
    }

    // ---- item creation, freeing, ownership (`items/generation.md`) -------

    /// Create a `gld` item at a spot (`0x00559CE0`, draws). Default: none.
    fn create_gold(&mut self, unit: Owner, spot: Spot) -> Option<Guid> {
        None
    }
    /// Free an item (`0x00557FD0`).
    fn free_item(&mut self, item: Guid) {}
    /// Duplicate (`0x0055A2A0`, draws). Default: none.
    fn copy_item(&mut self, item: Guid) -> Option<Guid> {
        None
    }
    /// Make a new item the player's cursor item (`0x0055FB10`).
    fn give_cursor_item(&mut self, player: Owner, item: Guid) {}
    /// Decrement a stack instead of consuming (`0x0055EEA0`): true = it was
    /// a stack and was decremented.
    fn consume_one(&mut self, item: Guid) -> bool {
        false
    }
    /// Item owner := unit (`0x00621CE0`).
    fn set_owner(&mut self, item: Guid, owner: Owner) {}
    /// Pile owner (`0x00552FD0`).
    fn pile_owner(&self, item: Guid) -> Option<Owner> {
        None
    }
    /// `0x0044BE50` (§5.3, §7.22). Default: 0.
    fn query_0044be50(&self) -> bool {
        false
    }
    /// Party share id (`0x00554630`); −1 none.
    fn party_share_id(&self, player: Owner) -> i32 {
        -1
    }
    /// Party share (`0x00540900`, multiplayer).
    fn party_share(&mut self, player: Owner, take: i32) {}
    /// Gold pickup of a pile owned by a player (`0x0053FF00`).
    fn owned_gold_pickup(&mut self, player: Owner, pile: Guid, take: i32) {}
    /// New pile of the rest at the player (`0x0055B030`).
    fn rest_pile(&mut self, player: Owner, rest: i32) {}
    /// `0x00629930(src)`, the stack merge's second branch (§7.12). Default:
    /// no merge.
    fn merge_allowed(&self, src: Guid) -> bool {
        false
    }
    /// Book count change `0x0055C070(n)` of the tome `book` (§8.1 step 4:
    /// `inventory.md` §5.5 skill quantity += n, S→C 0x22).
    fn book_count_changed(&mut self, player: Owner, book: Guid, n: i32) {}

    // ---- item use (`0x005BF240`: item-use spec, unwritten) ----------------

    /// Use `0x005BF240(I, I, x, y)` of an item at a position (§7.11 step
    /// 3); true = used.
    fn use_item_at(&mut self, player: Owner, item: Guid, x: i32, y: i32) -> bool {
        false
    }
    /// Opening the Horadric Cube (item-use table entry 7, `0x005BF0C0`,
    /// `world/cube.md` §1): the interaction (type 4, the cube) and the
    /// S→C 0x77 messages; true = opened (the cube is not consumed).
    /// Default: not opened.
    fn open_cube(&mut self, player: Owner, cube: Guid) -> bool {
        false
    }
    /// Use `0x005BF240` on a target; true = used.
    fn use_item(&mut self, player: Owner, target: Owner, item: Guid) -> bool {
        false
    }
    /// Tome / skill charge update (`0x0055E050`, `0x006439B0`, S→C 0x22).
    fn charge_update(&mut self, player: Owner, item: Guid) {}
    /// Removal of a used item from the belt `0x00561E70`.
    fn remove_used(&mut self, player: Owner, item: Guid) {}
    /// Consume an item (`0x0055E000`: S→C via `0x0053D010` with flag
    /// 0x20, then `0x0055DF10(item, 0)`; §7.11, §7.18 step 9).
    fn consume_item(&mut self, player: Owner, item: Guid) {}
    /// Item skill of a scroll or tome (`0x0055E050`, §7.18 step 6: book →
    /// books `bookskill`, scroll → `scrollskill`, else −1). Default: −1.
    fn item_skill(&self, item: Guid) -> i32 {
        -1
    }
    /// The player has skill `skill` (`0x006439B0`). Default: no.
    fn has_skill(&self, player: Owner, skill: i32) -> bool {
        false
    }
    /// Skill decrement `0x0055E0D0(S)` (§7.18 step 6): quantity − 1, < 1 →
    /// 0 and skill 0 on the right when S is the right skill, S→C 0x22.
    fn skill_decrement(&mut self, player: Owner, skill: i32) {}
    /// Set or clear a quest flag of the player's record for the current
    /// difficulty (`0x0065C360` / `0x0065C3A0`; §7.11 step 4).
    fn set_quest_flag(&mut self, player: Owner, quest: u8, flag: u8, on: bool) {}
    /// `0x005458E0(player, chain)` after a quest item use (§7.11 step 4:
    /// S→C 0x5D `5D chain 02 00 0000` to the player's client).
    fn quest_item_used(&mut self, player: Owner, chain: u8) {}
    /// `0x0058A0A0` (§7.11 step 4, `tr2`).
    fn quest_tr2_used(&mut self, player: Owner) {}
    /// Skills and stats reset `0x00570360`, `0x00570C80` (§7.11 step 4,
    /// `toa`).
    fn reset_skills_stats(&mut self, player: Owner) {}
    /// Equip a picked-up item `0x00562E00(item, 0)`; true = success.
    fn equip_picked(&mut self, player: Owner, item: Guid) -> bool {
        false
    }

    // ---- sockets (`items/properties.md` §9–§10) ---------------------------

    /// Filler properties (`0x0055C2C0`) and owner link (`0x006276C0`).
    fn filler_linked(&mut self, filler: Guid, target: Guid) {}
    /// Runeword check and apply (§7.19 step 3); true = applied.
    fn runeword(&mut self, player: Owner, target: Guid) -> bool {
        false
    }

    // ---- hirelings (mercenary spec, unwritten) -----------------------------

    /// The player's hireling (`0x00574EC0(7, 0)`).
    fn hireling(&self, player: Owner) -> Option<Owner> {
        None
    }
    /// The player has a used skill (`0x00620250`(player): skill list
    /// +0x10, `skills/levels.md`; §7.23 step 2). Default: yes (0x61 does
    /// nothing).
    fn has_used_skill(&self, player: Owner) -> bool {
        true
    }
    /// Alive (`0x005541B0` = 0 for players; hireling alive).
    fn alive(&self, u: Owner) -> bool {
        false
    }
    /// The hireling belongs to the player (`0x0065A590`).
    fn owns_hireling(&self, player: Owner, merc: Owner) -> bool {
        false
    }
    /// Equip on the hireling `0x0054CED0`.
    fn equip_on_merc(&mut self, merc: Owner, item: Guid) {}
    /// After a take: hireling inventory pass, `0x0055F500`, `0x0055F4F0(0)`.
    fn merc_after_take(&mut self, merc: Owner) {}

    // ---- NPC / object / other unit picks (`world/npc.md`, waypoints) -----

    /// 0x16 type 1 (`world/npc.md` §2).
    fn pick_npc(&mut self, player: Owner, guid: Guid, cursor: u32) -> u32 {
        0
    }
    /// 0x16 type 2 (`world/waypoints.md`, objects).
    fn pick_object(&mut self, player: Owner, guid: Guid, cursor: u32) -> u32 {
        0
    }
    /// Corpse pickup `0x0057FB70(game, player, P)` steps 1–2 (§7.1 type 0,
    /// §12.1: state 7, the take permission, the experience return); true
    /// when the take-back (§12.2) follows. Default: no.
    fn corpse_pickup(&mut self, player: Owner, corpse: Owner) -> bool {
        false
    }
    /// Corpse slot fit `0x0055F2D0` (§12.3) of X for `unit` with D (the
    /// unit's item at L) and A (its item at the paired location, or D):
    /// (fit, L after the rule). Default: no fit.
    fn corpse_slot_fit(
        &self,
        unit: Owner,
        x: Guid,
        d: Option<Guid>,
        a: Option<Guid>,
        l: u8,
    ) -> (bool, u8) {
        (false, l)
    }
    /// §12.1 step 4 after a full take-back: C off the player's corpse
    /// list (`0x0063D4E0`), out of its room (`0x0061A270`), S→C 0x8E
    /// `CorpseAssign` to every player (`0x0053DF80`), `0x00623830`, C
    /// freed (`0x00555600`). Default: nothing.
    fn corpse_taken(&mut self, player: Owner, corpse: Owner) {}
    /// Replenish timers of an item (`0x00558530`, `0x00558580`,
    /// `items/generation.md` §9 step 6). Default: nothing.
    fn replenish_timers(&mut self, item: Guid) {}
    /// Player-to-player interaction `0x00566E60` (§7.1 type 0;
    /// multiplayer).
    fn player_interact(&mut self, player: Owner, other: Owner) {}

    // ---- transport -------------------------------------------------------

    /// Queue a message to the player's client now.
    fn send(&mut self, player: Owner, bytes: Vec<u8>) {}
    /// S→C 0x3E for an item stat (`0x0053D130`): the wired desk builds it
    /// with `units::messages::update_item_stat` and sends it through
    /// [`MovePending::send`]. Default: nothing.
    fn send_item_stat(&mut self, player: Owner, item: Guid, stat: u16) {}
    /// Item bit stream (`0x006313E0`, OQ1) with item flags OR-ed with
    /// `flags` and the page shown as `page`. Default: empty.
    fn item_bits(&self, item: Guid, flags: u32, page: u8) -> Vec<u8> {
        Vec::new()
    }
    /// The bit stream of a store item shown to its trading client
    /// (`0x0053EF30` with 0x38): the alt-code record (`bitstream.md`
    /// §4.1 r4) exactly when the item's quality is 4–9 and it lacks item
    /// flag 0x10. Default: the plain stream.
    fn store_item_bits(&self, item: Guid, page: u8) -> Vec<u8> {
        self.item_bits(item, 0, page)
    }
    /// Store messages 0x38 / 0x39 of the dispatcher's first step
    /// (`world/vendors.md`). Default: none.
    fn store_messages(&mut self, client: Owner, item: Guid) -> Vec<Vec<u8>> {
        Vec::new()
    }
}

/// Everything the handlers need.
pub trait MoveWorld: InventoryOps + MoveUnits + MovePending {}
impl<T: InventoryOps + MoveUnits + MovePending> MoveWorld for T {}
