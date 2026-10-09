// Spec: specs/formats/d2s.md §2.2 rule 8, §2.4 (mouse skills, hotkeys r1, r2, r4, r6, r8), §8.4 (hireling items), §8.5 (golem item); specs/formats/d2s-load.md §4
//! The save gaps of the played character (q-save-gaps): the mouse skills,
//! the hireling's items and the Iron Golem's
//! item, read from the running game at save time ([`read_gaps`], laid over
//! the save by [`apply_gaps`]) and made again at the join ([`join_gaps`]).
//! The runeword refresh of a load is `InvDesk::load_entry`'s.
//!
//! d2rs-own, unverified; PROVISIONAL points are REC-241 in
//! `docs/HANDOFF.md`: the weapon-swap pair of mouse skills has no sim
//! state (C→S 0x60 is a stub) and passes through as loaded; a skill
//! granted by an item is not in the sim's skill list, so a mouse skill
//! saved with that item's index selects nothing on load and the hand
//! keeps its selection (`d2s.md` §2.4 rule 6.3, q-fix-save-gaps (1));
//! a loaded golem item is kept as loaded until a golem is summoned (the
//! re-summon, `d2s.md` Open question 15, is not wired); the progression
//! bits of the status word have no live source and pass through.

use d2_formats::d2s::{Body, D2s, Golem, Hireling, ItemEntry, Slot};
use d2_server::adapters::handlers::player::HotKey;
use d2_server::adapters::handlers::world::HirelingBlock;
use d2_server::adapters::session::HotKey as SessionHotKey;
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
    /// The mouse pair of the weapon set not in hand (header +0x80, +0x84)
    /// and the switch bit (+0x10 bit 0). `None`: no skill list.
    pub swap: Option<([Slot; 2], bool)>,
    /// The live hireling's header block (`None`: no hireling node; the
    /// loaded block passes through).
    pub hireling: Option<HirelingBlock>,
    /// The client's save flags (status word) with the progression
    /// (bits 8–12) the quests raised. `None`: no flags for the player.
    pub status: Option<u16>,
    /// The 16 hot-key slots of the player's client as C→S 0x51 and the
    /// load left them (§2.4 rule 8), encoded (rules 1–2). `None`: the
    /// player has no client.
    pub hotkeys: Option<[Slot; 16]>,
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
        // A golem re-summoned at the join has no item unit (REC-265): the
        // loaded item stays with it.
        .map(|items| {
            items
                .into_iter()
                .next()
                .or_else(|| sim.world.rest.golem_items.get(&player).cloned())
        });
    let swap = sim
        .events
        .action
        .hooks()
        .skill_lists
        .get(&player)
        .map(|list| (swap_slots(list, &guids), list.weapon_switch));
    let hireling = sim
        .world
        .hireling_block(&mut sim.game, &mut sim.events, player);
    let status = sim.world.rest.save_flags.get(&player).copied();
    let hotkeys = sim
        .client_list()
        .into_iter()
        .find(|&c| sim.player_of(c) == Some(player))
        .map(|c| hotkey_slots(&sim.hotkeys(c), &guids));
    Gaps {
        hotkeys,
        mouse,
        town,
        hireling_items,
        golem,
        swap,
        hireling,
        status,
    }
}

/// `save` with the [`Gaps`] laid over it.
pub fn apply_gaps(save: &mut D2s, gaps: &Gaps) {
    if let Some(keys) = gaps.hotkeys {
        save.header.hotkeys = keys;
    }
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
    if let Some(([left, right], switch)) = gaps.swap {
        save.header.mouse[2] = left;
        save.header.mouse[3] = right;
        save.header.weapon_switch = u32::from(switch);
    }
    // §2.5 rule 1: the block of the living hireling node.
    if let Some(b) = gaps.hireling {
        let h = &mut save.header.hireling;
        h.flags = if b.dead { Hireling::DEAD } else { 0 };
        h.seed = b.seed;
        h.name_index = b.name_index;
        h.id = b.id;
        h.experience = b.experience;
    }
    // `quests-act1-rest.md` §5: the progression is never lowered.
    if let Some(flags) = gaps.status {
        let p = |s: u16| s & 0x1F00;
        save.header.status = (save.header.status & !0x1F00) | p(flags).max(p(save.header.status));
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
    // The loader reads a `jf` list exactly when it restores the header's
    // hireling (§8.4 rule 2): a block without a list, or a list without a
    // block, would make it take the next marker for an item list (22).
    // `jf` cannot be left out while `kf` follows (the loader would read
    // `kf` as the `jf` marker), so a block whose items could not be read
    // saves the empty list (§8.4 rule 5).
    match (&body.hireling_items, hireling) {
        (Some(None), true) => body.hireling_items = Some(Some(Vec::new())),
        (Some(Some(_)), false) => body.hireling_items = Some(None),
        _ => {}
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
    select_swap_skills(
        s,
        player,
        &save.header.mouse[2..4],
        save.header.weapon_switch,
    );
    s.world.rest.save_flags.insert(player, save.header.status);
    join_golem(s, player, body);
}

/// A new character's client save flags: the expansion bit, and the
/// least progression that unlocks `difficulty` (the reader's §2.2 rule 5.4
/// test: Nightmare 5 / 4, Hell 10 / 8, expansion / classic), so the save
/// of a `play --new --difficulty` character loads on the difficulty it
/// was played on. d2rs-own, unverified (REC-282): the original has no new
/// character above Normal.
pub fn seed_new_flags(s: &mut Sim, player: UnitId, expansion: bool, difficulty: u8) {
    s.world
        .rest
        .save_flags
        .insert(player, new_flags(expansion, difficulty));
}

/// The flags of [`seed_new_flags`].
pub fn new_flags(expansion: bool, difficulty: u8) -> u16 {
    let progression: u16 = match (difficulty, expansion) {
        (0, _) => 0,
        (1, true) => 5,
        (1, false) => 4,
        (_, true) => 10,
        (_, false) => 8,
    };
    let exp = if expansion {
        d2_formats::d2s::status::EXPANSION
    } else {
        0
    };
    exp | progression << 8
}

/// `d2s-load.md` §3: a saved golem item with the skill 90 entry present
/// casts the Iron Golem at the join. The item is not made as a unit
/// (REC-265), so the golem comes without it and the saved item bytes stay
/// in the save.
fn join_golem(s: &mut Sim, player: UnitId, body: &Body) {
    let Some(Golem {
        item: Some(item),
        flag: 1,
    }) = &body.golem
    else {
        return;
    };
    if s.events.action.golem_resummon(&mut s.game, player) {
        s.world.rest.golem_items.insert(player, item.clone());
    } else {
        let log = &mut s.events.action.hooks().x.log;
        log.push("join: save load: golem: not re-summoned (no skill 90 or no summon)".into());
    }
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
/// §2.4 rules 1–2: each hot key as the writer stores it: the skill
/// (−1: none → 0xFFFF) with the left flag, and its item GUID as the
/// 1-based position in `guids` (the inventory list in link order; not
/// found → 0). A skill the writer would assert on (> 0x7FFF) is saved
/// as none.
pub fn hotkey_slots(keys: &[HotKey; 16], guids: &[u32]) -> [Slot; 16] {
    keys.map(|k| {
        let item = guids
            .iter()
            .position(|&g| g == k.item)
            .map_or(0, |p| p as u16 + 1);
        Slot::encode(i32::from(k.skill), k.left, item).unwrap_or(Slot::NONE)
    })
}

/// §2.4 rules 4 and 6.1: the saved hot keys as the load leaves the
/// client slots: code 0xFFFF → no skill; else skill = code & 0x0FFF, the
/// left flag, and a non-zero item index turned into the GUID at that
/// 1-based position of `guids` (past the end, or no index → −1).
pub fn loaded_hotkeys(slots: &[Slot; 16], guids: &[u32]) -> [SessionHotKey; 16] {
    slots.map(|s| {
        let (skill, left, item) = s.decode();
        let item = usize::try_from(item)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| guids.get(i).copied())
            .unwrap_or(u32::MAX);
        SessionHotKey {
            skill: skill as i16,
            flag: left,
            item,
        }
    })
}

/// The item index is the 1-based position of the owner item's GUID in
/// `guids` (the inventory list in link order); a native skill and an
/// unknown GUID give 0. No left skill is the all-zero pair.
pub fn mouse_slots(list: &SkillList, guids: &[u32]) -> [Slot; 2] {
    let view = list.view();
    // The four mouse words are the plain skill id: unlike the hotkeys, no
    // 0x8000 left flag is or-ed in (§2.4 rule 3, `0x00569155`–`0x0056920D`).
    let slot = |idx: Option<usize>| {
        let Some(e) = idx.and_then(|i| view.get(i)) else {
            return Slot::default();
        };
        let item = guids
            .iter()
            .position(|&g| g as i32 == e.owner_guid)
            .map_or(0, |p| p as u16 + 1);
        Slot::encode(e.skill, false, item).unwrap_or_default()
    };
    [slot(list.left), slot(list.right)]
}

/// §2.4 rules 4–6: the left and right skill with the item of their index
/// (the GUID at that 1-based position of `guids`; past the end → −1),
/// selected on `list`.
pub fn select_mouse(list: &mut SkillList, mouse: &[Slot], guids: &[u32]) {
    for (slot, left) in mouse.iter().zip([true, false]) {
        let (skill, _, item) = slot.decode();
        // Rule 6.2: only a non-zero skill is selected (Attack never is);
        // rule 6.3: skill 5 selects nothing.
        if skill == 0 || skill == 5 {
            continue;
        }
        let owner = match usize::try_from(item) {
            Ok(i) if i > 0 => guids.get(i - 1).map_or(-1, |&g| g as i32),
            _ => -1,
        };
        // Rule 6.3: the entry is found by skill id and owner GUID, both
        // exact; none: that hand keeps its selection (no class fallback).
        let Some(idx) = list.find(skill, owner) else {
            continue;
        };
        if left {
            list.left = Some(idx);
        } else {
            list.right = Some(idx);
        }
    }
}

/// §2.4 rules 1–3 for the swap set's pair.
pub fn swap_slots(list: &SkillList, guids: &[u32]) -> [Slot; 2] {
    let mut pair = list.clone();
    pair.left = list.swap_left;
    pair.right = list.swap_right;
    mouse_slots(&pair, guids)
}

fn select_swap_skills(s: &mut Sim, player: UnitId, mouse: &[Slot], switch: u32) {
    let guids = s.world.item_guids(player);
    if let Some(list) = s.events.action.hooks().skill_lists.get_mut(&player) {
        let mut pair = list.clone();
        pair.left = None;
        pair.right = None;
        select_mouse(&mut pair, mouse, &guids);
        list.swap_left = pair.left;
        list.swap_right = pair.right;
        list.weapon_switch = switch & 1 != 0;
    }
}

fn select_mouse_skills(s: &mut Sim, player: UnitId, mouse: &[Slot]) {
    let guids = s.world.item_guids(player);
    if let Some(list) = s.events.action.hooks().skill_lists.get_mut(&player) {
        select_mouse(list, mouse, &guids);
    }
}
