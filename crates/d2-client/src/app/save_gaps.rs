// Spec: specs/formats/d2s.md §2.2 rule 8, §2.4 (mouse skills), §8.4 (hireling items), §8.5 (golem item); specs/formats/d2s-load.md §4
//! The save gaps of the played character (q-save-gaps): the mouse skills,
//! the act of the town byte, the hireling's items and the Iron Golem's
//! item, read from the running game at save time ([`read_gaps`], laid over
//! the save by [`apply_gaps`]) and made again at the join ([`join_gaps`]).
//! The runeword refresh of a load is `InvDesk::load_entry`'s.
//!
//! d2rs-own, unverified; PROVISIONAL points are REC-241 in
//! `docs/HANDOFF.md`: the weapon-swap pair of mouse skills has no sim
//! state (C→S 0x60 is a stub) and passes through as loaded; a skill
//! granted by an item is not in the sim's skill list, so a mouse skill
//! saved with an item index loads on the native entry of the same skill;
//! a loaded golem item is kept as loaded until a golem is summoned (the
//! re-summon, `d2s.md` Open question 15, is not wired); the progression
//! bits of the status word have no live source and pass through.

use d2_formats::d2s::{Body, D2s, Golem, ItemEntry, Slot};
use d2_sim::skills::list::SkillList;
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::QuestRest;

use super::single_player::Sim;

/// What the running game says beyond [`super::save_full::Extra`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gaps {
    /// Left and right mouse skill of the current weapon set (header
    /// +0x78, +0x7C). `None`: no skill list.
    pub mouse: Option<[Slot; 2]>,
    /// (game difficulty, client act) for the town byte (+0xA8 + d).
    pub town: Option<(u8, u8)>,
    /// The hireling's item list (`None`: no hireling unit or no inventory
    /// model; the loaded list passes through).
    pub hireling_items: Option<Vec<ItemEntry>>,
    /// The Iron Golem's item: `Some(None)` a living golem without one.
    /// `None`: no golem; the loaded section passes through.
    pub golem: Option<Option<ItemEntry>>,
}

/// Reads [`Gaps`] from the game's `player`.
pub fn read_gaps(sim: &mut Sim, player: UnitId) -> Gaps {
    let guids = sim.world.item_guids(player);
    let mouse = sim
        .events
        .action
        .hooks()
        .skill_lists
        .get(&player)
        .map(|list| mouse_slots(list, &guids));
    let difficulty = sim.events.action.hooks().ai_info.difficulty;
    let town = sim.world.rest.unit_act(player).map(|act| (difficulty, act));
    let hireling_items = sim
        .world
        .hireling_unit(&sim.game, player)
        .and_then(|m| sim.world.save_items(&mut sim.game, &mut sim.events, m).ok());
    let golem = sim
        .world
        .golem_unit(&sim.game, &mut sim.events, player)
        .and_then(|g| sim.world.save_items(&mut sim.game, &mut sim.events, g).ok())
        .map(|items| items.into_iter().next());
    Gaps {
        mouse,
        town,
        hireling_items,
        golem,
    }
}

/// `save` with the [`Gaps`] laid over it.
pub fn apply_gaps(save: &mut D2s, gaps: &Gaps) {
    if let Some([left, right]) = gaps.mouse {
        save.header.mouse[0] = left;
        save.header.mouse[1] = right;
    }
    // §2.1: the writer sets the byte of the game's difficulty (act | 0x80)
    // and zeroes the other two.
    if let Some((difficulty, act)) = gaps.town {
        save.header.towns = [0; 3];
        save.header.towns[usize::from(difficulty).min(2)] = (if act < 5 { act } else { 0 }) | 0x80;
    }
    let expansion = save.header.status & d2_formats::d2s::status::EXPANSION != 0;
    let hireling = save.header.hireling.is_present();
    let Some(body) = save.body.as_mut() else {
        return;
    };
    // The list is read back only for a header with a hireling block
    // (§8.4 rule 2); the block itself is not live yet (a hireling hired
    // since the load has no block), so such a list is not written.
    if let Some(list) = gaps.hireling_items.as_ref().filter(|_| hireling) {
        body.set_hireling_items(expansion, Some(list.clone()));
    }
    if let Some(item) = &gaps.golem {
        set_golem(body, expansion, item.clone());
    }
}

fn set_golem(body: &mut Body, expansion: bool, item: Option<ItemEntry>) {
    if expansion {
        body.golem = Some(Golem {
            flag: u8::from(item.is_some()),
            item,
        });
    }
}

/// The join's part of a load (`d2s.md` §2.4 rule 6, §8.4): the hireling's
/// items on its unit and the mouse skills selected. Logs what failed as
/// `join: save load: …` lines.
pub fn join_gaps(s: &mut Sim, player: UnitId, save: &D2s) {
    let Some(body) = &save.body else {
        return;
    };
    if let Some(Some(items)) = &body.hireling_items {
        if !items.is_empty() {
            join_hireling_items(s, player, items);
        }
    }
    select_mouse_skills(s, player, &save.header.mouse[..2]);
}

/// §8.4 rule 2 on the hireling restored by the load: its unit is made by
/// the queued restore call, which runs here so the items have an owner.
fn join_hireling_items(s: &mut Sim, player: UnitId, items: &[ItemEntry]) {
    s.world.hireling_calls(&mut s.game, &mut s.events);
    let Some(merc) = s.world.hireling_unit(&s.game, player) else {
        let log = &mut s.events.action.hooks().x.log;
        log.push("join: save load: hireling items: no hireling unit".into());
        return;
    };
    let loaded = s.world.load_items(&mut s.game, &mut s.events, merc, items);
    // The messages for the player's client ride with the join's items.
    let own: Vec<Vec<u8>> = loaded
        .sent
        .iter()
        .filter(|(u, _)| *u == player)
        .map(|(_, b)| b.clone())
        .collect();
    if !own.is_empty() {
        s.events
            .action
            .sys
            .hooks
            .session
            .join_items
            .entry(player)
            .or_default()
            .extend(own);
    }
    let log = &mut s.events.action.hooks().x.log;
    log.extend(
        loaded
            .faults
            .iter()
            .map(|f| format!("join: save load: hireling items: {f}")),
    );
}

/// §2.4 rules 1–3: the left and right skill of `list` as header slots.
/// The item index is the 1-based position of the owner item's GUID in
/// `guids` (the inventory list in link order); a native skill and an
/// unknown GUID give 0. No left skill is the all-zero pair.
pub fn mouse_slots(list: &SkillList, guids: &[u32]) -> [Slot; 2] {
    let view = list.view();
    let slot = |idx: Option<usize>, left: bool| {
        let Some(e) = idx.and_then(|i| view.get(i)) else {
            return Slot::default();
        };
        let item = guids
            .iter()
            .position(|&g| g as i32 == e.owner_guid)
            .map_or(0, |p| p as u16 + 1);
        Slot::encode(e.skill, left, item).unwrap_or_default()
    };
    [slot(list.left, true), slot(list.right, false)]
}

/// §2.4 rules 4–6: the left and right skill with the item of their index
/// (the GUID at that 1-based position of `guids`; past the end → −1),
/// selected on `list`.
pub fn select_mouse(list: &mut SkillList, mouse: &[Slot], guids: &[u32]) {
    for (slot, left) in mouse.iter().zip([true, false]) {
        // "No left skill" is the all-zero pair (§2.4 rule 3).
        if slot.code == 0 && slot.item == 0 && left {
            continue;
        }
        let (skill, _, item) = slot.decode();
        if skill < 0 {
            continue;
        }
        let owner = match usize::try_from(item) {
            Ok(i) if i > 0 => guids.get(i - 1).map_or(-1, |&g| g as i32),
            _ => -1,
        };
        let idx = list.find(skill, owner).or_else(|| list.native(skill));
        if left {
            list.left = idx;
        } else {
            list.right = idx;
        }
    }
}

fn select_mouse_skills(s: &mut Sim, player: UnitId, mouse: &[Slot]) {
    let guids = s.world.item_guids(player);
    if let Some(list) = s.events.action.hooks().skill_lists.get_mut(&player) {
        select_mouse(list, mouse, &guids);
    }
}
