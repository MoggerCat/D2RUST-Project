// Spec: specs/items/treasure.md §9, §9.1
//! The quest drop helper `0x00559A30` (§9): item level, item class (the
//! drop code, else the random class pick `0x00556240` with the magic
//! retry loop) and the class sub-pickers `0x00555E70` (armor),
//! `0x00555FB0` (weapons), `0x005560F0` (misc) of §9.1. Placement and
//! creation go through the walk's [`DropSink`], as §7.

use super::walk::{item_level, DropRequest, DropSink, Dropper, INIT_FLAGS_DROP, SPAWN_TYPE_DROP};
use super::TreasureError;
use crate::drlg::act_of_level;
use crate::items::tables::{ItemRec, ItemTables};
use crate::rng::Seed;

/// Quality 4 (magic): the §9 rule 3 retry loop.
pub const QUALITY_MAGIC: u8 = 4;
/// Highest item level of the class pick (`0x00556240`, fatal 0x180 above).
pub const MAX_PICK_LEVEL: i32 = 65;
/// Calls of `0x00556240` in the magic loop before the weapons picker
/// takes over (§9 rule 3).
pub const MAGIC_RETRIES: u32 = 11;
/// Candidates held by a sub-picker (§9.1 rule 3).
pub const MAX_CANDIDATES: usize = 1023;
/// itemtypes 40 `body part` (§9.1 rule 1).
pub const TYPE_BODY_PART: i16 = 40;
/// The `gld ` code (§9 rule 3, cached index `0x008846EC`).
pub const GOLD_CODE: [u8; 4] = *b"gld ";
/// d2rs guard for the unbounded magic loop of §9 rule 3: 1.14d never
/// leaves it when no weapon can be magic (the server hangs); d2rs stops
/// after this many retries with [`TreasureError::QuestDropHang`].
pub const MAGIC_LOOP_GUARD: u32 = 1 << 20;

/// A part of the combined items array (§9.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// `0x00555FB0`.
    Weapons = 0,
    /// `0x00555E70`.
    Armor = 1,
    /// `0x005560F0`, from the misc start to the array end.
    Misc = 2,
}

/// The tables and game facts the class pick reads.
#[derive(Clone, Copy)]
pub struct PickData<'a> {
    pub items: &'a [ItemRec],
    /// (start, count) per part ([`ItemTables::parts`]).
    pub parts: [Option<(usize, usize)>; 3],
    /// game +0x70.
    pub expansion: bool,
}

impl<'a> PickData<'a> {
    pub fn of(t: &'a ItemTables, expansion: bool) -> Self {
        Self {
            items: &t.items,
            parts: t.parts,
            expansion,
        }
    }
}

/// The filter `0x00555E00` (§9.1 rule 2) for one record; may draw one
/// `roll(d)` on `seed`.
fn passes(r: &ItemRec, l: i32, seed: &mut Seed, p6: i32, p7: i32) -> bool {
    let l = l.max(1);
    if r.spawnable == 0 || r.quest != 0 || i32::from(r.level) > l {
        return false;
    }
    if p7 == 0 {
        // The item level where a level id is expected (§9.1 rule 2).
        let a = i32::from(act_of_level(l as u32));
        let d = i32::from(r.rarity) - a;
        if d > 0 && seed.roll(d) != 0 {
            return false;
        }
    }
    p6 == -1 || i32::from(r.type_) == p6
}

/// A class sub-picker (§9.1): the combined index, or −1 (part absent,
/// or no candidate: d2rs's choice for the uninitialised slot, §9.1).
#[allow(clippy::too_many_arguments)]
pub fn sub_pick(
    d: &PickData<'_>,
    part: Part,
    seed: &mut Seed,
    l: i32,
    p6: i32,
    p7: i32,
    monster: bool,
) -> i32 {
    let Some((start, count)) = d.parts[part as usize] else {
        return -1;
    };
    let end = match part {
        Part::Misc => d.items.len(),
        _ => (start + count).min(d.items.len()),
    };
    let mut cands: Vec<usize> = Vec::new();
    for i in start..end {
        let r = &d.items[i];
        // Rule 1: body parts only from monsters.
        if part == Part::Misc && r.type_ == TYPE_BODY_PART && !monster {
            continue;
        }
        if !passes(r, l, seed, p6, p7) {
            continue;
        }
        // Rule 3.
        if (d.expansion || r.version < 100) && cands.len() < MAX_CANDIDATES {
            cands.push(i);
        }
    }
    let n = cands.len() as u32;
    if n == 0 {
        // d2rs Ruleset choice for the uninitialised slot (§9.1, OQ12; IT-11).
        return -1;
    }
    let lo = seed.step();
    let k = if n & (n - 1) == 0 {
        lo & (n - 1)
    } else {
        lo % n
    };
    cands[k as usize] as i32
}

/// The random class pick `0x00556240` (§9 rule 3).
pub fn class_pick(
    d: &PickData<'_>,
    seed: &mut Seed,
    l: i32,
    p6: i32,
    p7: i32,
    monster: bool,
) -> Result<i32, TreasureError> {
    if l > MAX_PICK_LEVEL {
        return Err(TreasureError::PickLevel(l));
    }
    let r = seed.roll(100) as i32;
    let gold = 65 - l;
    let armor = gold + l / 2 + 5;
    let weapons = armor + l / 2 + 10 + (l & 1);
    Ok(if r < gold {
        d.items
            .iter()
            .position(|x| x.code == GOLD_CODE)
            .map_or(-1, |i| i as i32)
    } else if r < armor {
        sub_pick(d, Part::Armor, seed, l, p6, p7, monster)
    } else if r < weapons {
        sub_pick(d, Part::Weapons, seed, l, p6, p7, monster)
    } else if r < 100 {
        sub_pick(d, Part::Misc, seed, l, p6, p7, monster)
    } else {
        return Err(TreasureError::PickRoll(r));
    })
}

/// The record of `c` may be magic (`bitfield1` bit 0).
fn may_be_magic(d: &PickData<'_>, c: i32) -> bool {
    usize::try_from(c)
        .ok()
        .and_then(|c| d.items.get(c))
        .is_some_and(|r| r.bitfield1 & 1 != 0)
}

/// The item class `c` of §9 rule 3. `drop_code`: the unit's drop item
/// code (unit +0xB8), `None` when 0.
#[allow(clippy::too_many_arguments)]
pub fn quest_class(
    d: &PickData<'_>,
    drop_code: Option<[u8; 4]>,
    q: u8,
    seed: &mut Seed,
    l: i32,
    p6: i32,
    p7: i32,
    monster: bool,
) -> Result<i32, TreasureError> {
    if let Some(code) = drop_code {
        return d
            .items
            .iter()
            .position(|x| x.code == code)
            .map(|i| i as i32)
            .ok_or(TreasureError::DropCode(code));
    }
    let mut c = class_pick(d, seed, l, p6, p7, monster)?;
    if q == QUALITY_MAGIC {
        let mut calls = 1u32;
        while !may_be_magic(d, c) {
            if calls >= MAGIC_LOOP_GUARD {
                return Err(TreasureError::QuestDropHang);
            }
            c = if calls < MAGIC_RETRIES + 1 {
                class_pick(d, seed, l, p6, p7, monster)?
            } else {
                sub_pick(d, Part::Weapons, seed, l, p6, p7, monster)
            };
            calls += 1;
        }
    }
    Ok(c)
}

/// The arguments of `0x00559A30` besides the game and the source unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestDropArgs {
    /// Quality `q`.
    pub quality: u8,
    /// The unit's drop item code (+0xB8), `None` when 0.
    pub drop_code: Option<[u8; 4]>,
    /// Item type filter (−1 = any).
    pub p6: i32,
    /// Skip the rarity roll when ≠ 0.
    pub p7: i32,
    /// The game's item format (game +0x78).
    pub item_format: u32,
}

/// What the helper writes out: `*&level` (always, rule 2) and the request
/// (only when a spot was found, rule 4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestDropOut<S, I> {
    pub level: i32,
    pub request: Option<DropRequest<S>>,
    pub item: Option<I>,
}

/// `0x00559A30` (§9 rules 2–5) for the dropper `u` (its kind, level and
/// position), drawing on `seed` (the unit seed of `U`; `None` for no
/// unit). The pipeline's ilvl write-back is the sink's.
pub fn quest_drop<S: DropSink>(
    d: &PickData<'_>,
    u: &Dropper,
    monster: bool,
    seed: &mut Seed,
    args: &QuestDropArgs,
    sink: &mut S,
) -> Result<QuestDropOut<S::Spot, S::Item>, TreasureError>
where
    S::Spot: Clone,
{
    // Rule 2.
    let level = item_level(u);
    // Rule 3.
    let c = quest_class(
        d,
        args.drop_code,
        args.quality,
        seed,
        level,
        args.p6,
        args.p7,
        monster,
    )?;
    // Rule 4.
    let Some(spot) = sink.place(u.x, u.y) else {
        return Ok(QuestDropOut {
            level,
            request: None,
            item: None,
        });
    };
    // Rule 5. A class of −1 fails creation (`generation.md` §3 step 2).
    let req = DropRequest {
        id: c as u16,
        quality: args.quality,
        index: 0,
        item_level: level,
        spot,
        spawn_type: SPAWN_TYPE_DROP,
        init_flags: INIT_FLAGS_DROP,
        item_format: args.item_format,
        drop_flags: 0,
    };
    let item = if c < 0 {
        None
    } else {
        sink.create(req.clone())
    };
    Ok(QuestDropOut {
        level,
        request: Some(req),
        item,
    })
}

#[cfg(test)]
mod tests;
