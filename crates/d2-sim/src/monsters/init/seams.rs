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

use crate::combat::DamageRecord;
use crate::skills::use_::bodies::MissileRequest;
use crate::stats::lists::ListId;

use super::callbacks::{AuraFields, SkillCalc, StateApply};
use super::find::FindQuery;
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
    /// `skills/bodies.md` §6.5 step 9: the base code of the owner's item
    /// at body location `loc` (`0x00628590`); `None` when it has none.
    fn owner_item_code(&mut self, owner: UnitId, loc: u8) -> Option<[u8; 4]> {
        None
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

    /// §14.3: quest chain record `0x005436B0` (quests spec).
    fn quest_chain(&mut self, unit: UnitId, chain: u32) {}
    /// §14.3 bloodraven: state 118 corpse_noselect on
    /// (`0x00639DB0(unit, 118, 1)`).
    fn set_corpse_noselect(&mut self, unit: UnitId) {}
    /// §14.3 ancient barbarian equipment: difficulty 1 replaces `code`
    /// by its items row's `ubercode` (+0x88), 2 by `ultracode` (+0x8C)
    /// (row found with `0x00633640`). Default: unchanged.
    fn item_tier_code(&mut self, code: [u8; 4], difficulty: u8) -> [u8; 4] {
        code
    }
    /// §14.3 `0x00573B20(game, unit, &entry, level, 4)`: create `code`
    /// as a magic item (spawn mode 4) at item level `level` and equip it
    /// at body location `loc` (`items/generation.md` §10.2; game- and
    /// item-seed draws). Default: nothing.
    fn create_boss_item(&mut self, unit: UnitId, code: [u8; 4], loc: u8, level: i32) {}
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
    /// §20.1: `0x005B23C0(game, unit, class, mode, spread, flags)`, a
    /// monster near the unit (`population.md`).
    fn spawn_near_unit(&mut self, unit: UnitId, class: u32, mode: u32, spread: i32, flags: u32) {}
    /// §20.1 "spawn(c, r, n, f)": `0x005B24E0(game, boss, c, mode 1, r,
    /// n, f)` (`population.md` Open question 4).
    fn spawn_group(&mut self, boss: UnitId, class: u32, r: i32, n: i32, flags: u32) {}
    /// §20.1 `0x00545B50(game, unit)`: the quest preset-boss hook
    /// (`world/quests-act3.md`, `world/quests-act5.md`).
    fn quest_preset_boss(&mut self, unit: UnitId) {}
    /// §20.1 hcIdx 60: owner data `0x0058F030(game, unit, own GUID, 1,
    /// 1, 0)`.
    fn owner_data_self(&mut self, unit: UnitId) {}
    /// `0x0063EC70(room, class)`: the class remapped for the unit's
    /// room level. Default: unchanged.
    fn class_for_level(&mut self, unit: UnitId, class: u32) -> u32 {
        class
    }
    /// §20.1: a state on (`0x00639DB0(unit, state, 1)`).
    fn set_state(&mut self, unit: UnitId, state: u16) {}
    /// §21 minion restore: `0x005B30E0` with the GUID, flags 0x62
    /// (`population.md` §6.3 step 4 retries).
    fn spawn_with_guid(&mut self, req: &CreateRequest) -> Option<UnitId> {
        None
    }

    // ---- callbacks (§22; `monsters/umod-callbacks.md`) ----

    /// A stat total (`0x00625480(unit, stat, 0)`, `stat-lists.md`):
    /// level(12), the damage stats, tohit. Default: the base value.
    fn stat_total(&self, unit: UnitId, stat: u16) -> i32 {
        self.stat(unit, stat)
    }
    /// "Base list set": list set `0x00627150` on the unit's flag-1 list
    /// (`umod-callbacks.md` §1 rule 3). Default: [`InitHost::set_stat`].
    fn set_base_list_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.set_stat(unit, stat, value);
    }
    /// The unit's position (static path for types 2, 4, 5, else the
    /// dynamic path; (0, 0) without a path; `skills/bodies.md` §1).
    fn position(&self, unit: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// "Has s" `0x00639DF0` (`stat-lists.md` §9).
    fn has_state(&self, unit: UnitId, state: u16) -> bool {
        false
    }
    /// A state of flag group `group` (`0x0063A770`,
    /// `data/runtime-maps.md` §4).
    fn has_state_in_group(&self, unit: UnitId, group: u8) -> bool {
        false
    }
    /// §27 step 4, the monster teardown `0x005736A0`: drop the unit's own
    /// combat-list entries (`0x0057C980`); with `free_inventory` remove
    /// the inventory's items (`0x00555AE0`) and free it (`0x0063AC40`);
    /// return the hover record (`0x006611A0`, pointer kept); free AI
    /// control, AI params and the interaction block (`0x0058F810`,
    /// `0x005A64D0`, `0x00572BC0`). The timers are cancelled by the
    /// caller. Default: nothing.
    fn monster_teardown(&mut self, unit: UnitId, free_inventory: bool) {}
    /// §27 step 7: the plain mode set `0x00624690` (`sim/units.md` §4.1).
    /// Default: the unit record's mode.
    fn set_mode_plain(&mut self, unit: UnitId, mode: u32) {
        if let Some(r) = self.units().get_mut(unit) {
            r.mode = mode;
        }
    }
    /// "Mode set m": `0x005A7E60` + `0x005A7C20(game, &rec, 1)`
    /// (`units.md` §4.6), which runs the umod dispatcher again.
    fn set_mode(&mut self, unit: UnitId, mode: u32) {}
    /// Owner `0x00552FD0` (unit flags +0xC8 bit 10, owner +0x98).
    fn owner(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// Minion owner `0x0058F0D0` (`ai.md` §3.1).
    fn minion_owner(&mut self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// Alignment `0x006259B0` (0, 1, 2).
    fn alignment(&self, unit: UnitId) -> u8 {
        0
    }
    /// Hostility `0x00554200(game, a, b)` (`combat/hit.md`).
    fn hostile(&self, a: UnitId, b: UnitId) -> bool {
        false
    }
    /// Target `skills/bodies.md` §2.1 (the unit's target unit).
    fn target(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// Target position `0x0056D2C0` (`skills/bodies.md` §2.4).
    fn target_position(&self, unit: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// The target point of a missile's path (`0x00648A00` /
    /// `0x00648A10`).
    fn path_target_point(&self, missile: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// Missile creation `0x0059FA30` with a parameter record
    /// (`missiles.md` §R2.1, §R2.3). Runs the owner's mode-5 umods.
    fn create_missile(&mut self, req: MissileRequest<UnitId>) -> Option<UnitId> {
        None
    }
    /// missiles row `class` flags dword (+0x04); `None` without a row
    /// (`0x0046ACE0`).
    fn missile_row_flags(&self, class: i32) -> Option<u32> {
        None
    }
    /// The velocity of missile `class` at `level` (`0x00663270`).
    fn missile_velocity(&self, class: i32, level: i32) -> i32 {
        0
    }
    /// A missile's (skill, level) (`0x0064A280`, `0x0064A210`).
    fn missile_skill_level(&self, missile: UnitId) -> (i32, i32) {
        (0, 0)
    }
    /// The unit find `0x0065A950` around (x, y) from the room
    /// containing the point, searched from `near`'s room
    /// (`umod-callbacks.md` §3.1, `super::find`): the found units in
    /// order.
    fn find_units(&mut self, near: UnitId, q: FindQuery) -> Vec<UnitId> {
        Vec::new()
    }
    /// The line test of area damage: `0x00645950(x, y, V, 0x805)` is 0
    /// (§3.2 step 3).
    fn line_clear(&mut self, from: (i32, i32), unit: UnitId) -> bool {
        false
    }
    /// `0x005AD730(game, src, V, rec)` (`missiles.md` §R6: hit flags,
    /// block, damage apply and reaction); nothing unless `src` is a
    /// missile with an owner.
    fn missile_hit(&mut self, src: UnitId, unit: UnitId, rec: &DamageRecord) {}
    /// A skill-calc field of a skill at a level (`skills/levels.md`
    /// `eval`) with `unit` as the caster.
    fn skill_calc(&mut self, unit: UnitId, skill: u16, calc: SkillCalc, level: i32) -> i32 {
        0
    }
    /// The aura columns of a skill row (`None` past the skills count).
    fn aura_fields(&self, skill: u16) -> Option<AuraFields> {
        None
    }
    /// itemstatcost and states row counts.
    fn stat_and_state_counts(&self) -> (i32, i32) {
        (0, 0)
    }
    /// `apply_state` (`skills/bodies.md` §2.7); the state list made.
    fn apply_state(&mut self, req: StateApply) -> Option<ListId> {
        None
    }
    /// List set `0x00627150` on a state list.
    fn set_list_stat(&mut self, list: ListId, stat: u16, value: i32) {}
    /// The unit's minion list freed (`0x0058F160`, `ai.md`).
    fn free_minions(&mut self, unit: UnitId) {}
    /// Owner data `0x0058F030(game, unit, −1, 1, 0, 0)` (`ai.md`).
    fn clear_owner_data(&mut self, unit: UnitId) {}
    /// Pet remove `0x005750E0(game, owner, GUID, kill 1)` (`sim/pets.md`
    /// §6).
    fn remove_pet(&mut self, owner: UnitId, pet: UnitId) {}
    /// `umod-callbacks.md` §15.1: `0x0063E990` undead.
    fn is_undead(&self, unit: UnitId) -> bool {
        false
    }
    /// missiles row `class` `Range`; `None` without a row (§15.2: the
    /// original then reads through a null record).
    fn missile_range(&self, class: i32) -> Option<i32> {
        None
    }
    /// §15.2 `0x005E0070`: set the uber death flag of `slot` (0 = game
    /// +0x1DF0 ubermephisto, 1 = +0x1DEC uberdiablo, 2 = +0x1DE8
    /// uberbaal); true when all three are set. Default: never all set.
    fn set_uber_death(&mut self, slot: usize) -> bool {
        false
    }
    /// §15.2: game +0x8C.
    fn game_8c(&self) -> i32 {
        0
    }
    /// §15.2: the unit's item code (+0xB8) := `code`, then the drop
    /// helper `0x00559A30(game, unit, arg, …)` (`world/quests-act1-rest.md`
    /// item 1); with `announce` also `0x0055FE80(game, 0, item)`.
    fn quest_drop(&mut self, unit: UnitId, code: [u8; 4], arg: i32, announce: bool) {}
    /// Umod 24's steal (§17 steps 3–4: belt slot, copy, drop; items
    /// spec seams). Unreachable in 1.14d.
    fn steal_belt_item(&mut self, unit: UnitId, target: UnitId) {}
    /// `0x005B2490(game, unit, class, mode, spread, flags)`
    /// (`init.md` §1).
    fn spawn_near(&mut self, unit: UnitId, class: u32, mode: u32, spread: i32, flags: u32) {}
    /// §23.2 step 3: `0x0064D870(room, x, y, 1, 0x3C01)` ≠ 0
    /// (`sim/path-placement.md`).
    fn footprint_occupied(&mut self, unit: UnitId) -> bool {
        false
    }
    /// AI param 0 (`0x0058EC50(unit, 1)`).
    fn ai_param0(&mut self, unit: UnitId) -> i32 {
        0
    }
    /// `0x0058EC00`: AI param 0 := v.
    fn set_ai_param0(&mut self, unit: UnitId, v: i32) {}
    /// The raise test `0x00645510(unit, 0)`.
    fn can_raise(&mut self, unit: UnitId) -> bool {
        false
    }
    /// `0x005DEAD0(game, unit, mode, skill, 0, 0, 0)` (`ai.md` skill
    /// use).
    fn ai_use_skill(&mut self, unit: UnitId, mode: u32, skill: u16) {}
    /// Unit flags +0xC4 &= !mask.
    fn clear_unit_flags(&mut self, unit: UnitId, mask: u32) {
        if let Some(r) = self.units().get_mut(unit) {
            r.flags &= !mask;
        }
    }
    /// The level of the unit's skill `skill` (`0x006439F0` +
    /// `0x006442A0(unit, entry, 1)`); `None` without the entry.
    fn skill_level(&mut self, unit: UnitId, skill: u16) -> Option<i32> {
        None
    }
    /// Life percent `0x00621F20`.
    fn life_percent(&self, unit: UnitId) -> i32 {
        0
    }
    /// The area level of the unit's room (`0x0061DCA0(level id, d,
    /// expansion)`, `init.md` §7 rule 2); `None` when the room has no
    /// level id.
    fn room_area_level(&mut self, unit: UnitId) -> Option<i32> {
        None
    }
}
