// Spec: specs/monsters/init.md (Inputs; every call into another spec)
//! [`InitHost`]: the state init works on and every call into a system
//! another spec owns. Defaults are the narrowest reading: nothing
//! happens, or the value that makes init do nothing. The provider of each
//! method is named in its doc. Spawning seams re-enter init (a spawned
//! monster runs [`super::create`] / [`super::type_init`] through the
//! host), so the host owns all state and init reaches it through `&mut
//! self`.

use crate::game::Game;
use crate::units::record::Units;
use crate::units::UnitId;

use super::{CreateRequest, GameInfo, MonsterStore};

#[allow(unused_variables)]
pub trait InitHost {
    // ---- state ----

    fn game(&mut self) -> &mut Game;
    /// Unit records (seed +0x20, mode +0x10, flags +0xC4, class).
    fn units(&mut self) -> &mut Units;
    /// Monster data blocks.
    fn monsters(&mut self) -> &mut MonsterStore;
    /// Game settings at the time of the call.
    fn info(&self) -> GameInfo;
    /// `0x00573930` writes difficulty 2 into game +0x6D when it finds
    /// ≥ 3 (§9 step 3, edge case 12).
    fn set_difficulty(&mut self, d: u8) {}

    // ---- stats (provider: units/stats session, `StatLists::unit_base`
    // / `unit_set`, layer 0) ----

    /// The unit's base stat (layer 0).
    fn stat(&self, unit: UnitId, stat: u16) -> i32;
    /// Sets the unit's base stat (layer 0) through the value callback.
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32);

    // ---- type init (§5, §6) ----

    /// §5 step 2: allocate AI control, AI params and the interaction
    /// block (`0x0058EBC0`, `0x005A6490`, `0x00572BA0`). Provider: AI
    /// session (`AiStore::entry(unit).control = Some(..)`).
    fn alloc_ai(&mut self, unit: UnitId) {}
    /// `0x005B0E00(game, unit, AI control, state)`
    /// (`monsters::ai::install`).
    fn ai_install(&mut self, unit: UnitId, state: u32) {}
    /// The level id of the unit's room (`0x0061DCA0` argument, §5 step 6).
    /// Provider: DRLG.
    fn level_id(&mut self, unit: UnitId) -> i32 {
        0
    }
    /// §5 step 4 / §10: the variant count (+0x03) of the class's region
    /// data, 0 when it has none (`0x00547BC0`, `population.md`).
    fn region_variant_count(&mut self, unit: UnitId, class: u32) -> u8 {
        0
    }
    /// The 16 component bytes of variant `i` (region data +0x04 + 16·i).
    fn region_variant(&mut self, unit: UnitId, class: u32, i: u32) -> [u8; 16] {
        [0; 16]
    }
    /// §5 step 7: `0x00545CD0(game, unit, room, 1)` (quests spec).
    fn attach_quest_chain(&mut self, unit: UnitId) {}
    /// §5 step 7: combat mode `0x00553570` (units spec).
    fn set_combat_mode(&mut self, unit: UnitId) {}
    /// §5 step 7: `0x005533D0` (`sim/units.md`).
    fn after_type_init(&mut self, unit: UnitId) {}
    /// §6 step 12: post an extra stat list (`0x006251F0` + `0x00626E10`,
    /// `sim/stat-lists.md`).
    fn post_extra_list(&mut self, unit: UnitId) {}
    /// §6 step 13: `npc_store` → NPC store inventory `0x00536B20`, else a
    /// new inventory `0x0063ABD0`. Provider: items / vendors.
    fn new_inventory(&mut self, unit: UnitId, npc_store: bool) {}
    /// Unit +0x60 is set.
    fn has_inventory(&mut self, unit: UnitId) -> bool {
        false
    }
    /// §6 step 14: give `skill` at `level`, then its mode when given
    /// (skills spec).
    fn give_skill(&mut self, unit: UnitId, skill: u16, level: i32, mode: Option<u8>) {}
    /// §11: item property assignment on the unit `0x0065FD70` (property
    /// spec).
    fn apply_property(&mut self, unit: UnitId, prop: i32, par: i32, min: i32, max: i32) {}
    /// §12: the inventory holds an item at body location `loc`.
    fn has_item_at(&mut self, unit: UnitId, loc: u8) -> bool {
        false
    }
    /// §12: create item `code` at `loc` with modifier `modifier`, item
    /// level `level` (`0x00573B20`, treasure spec; its draws use the unit
    /// seed).
    fn create_equip_item(
        &mut self,
        unit: UnitId,
        code: [u8; 4],
        loc: u8,
        modifier: u8,
        level: i32,
    ) {
    }

    // ---- creation sequence (§4; `population.md` §9) ----

    /// Placement `population.md` §9 (active room seed draws): the
    /// accepted point, `None` when there is none.
    fn place(&mut self, req: &CreateRequest) -> Option<(i32, i32)> {
        None
    }
    /// §4 step 1: `0x00555230` (`units::lifecycle::allocate`), whose
    /// `init_kind` must run [`super::type_init`]. `None` when refused.
    fn allocate(&mut self, req: &CreateRequest, x: i32, y: i32) -> Option<UnitId> {
        None
    }
    /// §4 step 2: region count `0x00547D90` (nc = `never_count`) and the
    /// coord list (`population.md` §9.6 steps 2–3).
    fn register_spawn(&mut self, unit: UnitId, req: &CreateRequest, never_count: u32) {}
    /// `0x005543B0` alignment value (0, 1 or 2; units spec).
    fn set_alignment(&mut self, unit: UnitId, alignment: u8) {}
    /// §4 step 5: party minions `0x005B2830` (`population.md` §10).
    fn party_minions(&mut self, unit: UnitId, req: &CreateRequest) {}

    // ---- bosses (§14, §16–§21) ----

    /// §14.2: quest chain record `0x005436B0` (quests spec).
    fn quest_chain(&mut self, unit: UnitId, chain: u32) {}
    /// §14.2 bloodraven: state corpse_noselect (states spec).
    fn set_corpse_noselect(&mut self, unit: UnitId) {}
    /// `0x005A09E0` steps 1–4 (`population.md` §6.3): placement and
    /// creation of a boss. Step 5 is [`super::mark_boss`].
    fn boss_spawn(
        &mut self,
        req: &CreateRequest,
        guid: Option<u32>,
        warp_check: bool,
    ) -> Option<UnitId> {
        None
    }
    /// `0x005A0320`: bosses spawned (+0x2C8) of the boss's region += 1
    /// (`population.md` §6.3 step 5).
    fn count_region_boss(&mut self, unit: UnitId) {}
    /// Quest hook `0x00544E80` (quests spec).
    fn boss_quest_hook(&mut self, unit: UnitId) {}
    /// Owner data `0x0058F030(game, boss, boss GUID, 1, 1, 0)`
    /// (`monsters/ai.md`).
    fn boss_owner_data(&mut self, unit: UnitId) {}
    /// §18 step 1: one boss minion spawn (`population.md` §6.5 step 4:
    /// `0x005B2F70` with a coord list, else `0x005B23C0`; spread 3,
    /// flags 0x40).
    fn spawn_boss_minion(
        &mut self,
        boss: UnitId,
        class: u32,
        coord_list: Option<u32>,
    ) -> Option<UnitId> {
        None
    }
    /// §18 step 1: owner data, minion list, owner GUID (`0x0058F030`,
    /// `0x0058F100`, `0x005DD330`; `monsters/ai.md`).
    fn link_minion(&mut self, boss: UnitId, minion: UnitId) {}
    /// The unit's minion list (`0x0058F380`).
    fn minions(&mut self, boss: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    /// §17.3: monster type `montype` is `ty` or nested in it
    /// (`0x005A0070`, montype.txt).
    fn montype_is(&mut self, montype: u16, ty: u16) -> bool {
        montype == ty
    }
    /// §19.5: give and assign the aura skill (`0x0056DEB0`, `0x005701B0`).
    fn give_aura(&mut self, unit: UnitId, skill: u16, level: i32) {}
    /// §19.6 umod 26: set AI control flag 0x20.
    fn set_ai_flag(&mut self, unit: UnitId, flag: u16) {}
    /// §20 step 5: quest records by `hcIdx` (quests spec).
    fn superunique_quest(&mut self, unit: UnitId, hc_idx: u32) {}
    /// §20 step 5: state 118 on the Countess (states spec).
    fn set_state(&mut self, unit: UnitId, state: u16) {}
    /// §21 minion restore: `0x005B30E0` with the GUID, flags 0x62
    /// (`population.md` §6.3 step 4 retries).
    fn spawn_with_guid(&mut self, req: &CreateRequest) -> Option<UnitId> {
        None
    }

    // ---- callbacks (§22) ----

    /// Umod 41's handler: `0x00573780` (`monsters/ai.md` §1).
    fn run_ai_tick(&mut self, unit: UnitId) {}
    /// Umod 34's gate `0x006259B0` (true = do nothing).
    fn umod34_gate(&mut self, unit: UnitId) -> bool {
        false
    }
}
