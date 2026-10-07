// Spec: specs/world/npc.md §7
//! Mercenaries: the hire list (§7.1, §7.2), hiring (§7.3), resurrection
//! (§7.4) and the quest-granted mercenary (§7.5).

use super::{
    code, resurrect_message, send_code, stat, u32_at, NpcControl, NpcError, NpcVendors, NpcWorld,
    RESURRECTORS, SELLERS, UNIT_MONSTER,
};
use crate::rng::Seed;
use crate::units::UnitId;
use crate::world::npc::class::{KASHYA, QUAL_KEHK};

/// §7.1 step 1: slots of a hire list (0x450 bytes / 16).
pub const HIRE_SLOTS: usize = 69;
/// §7.1 step 4: offers drawn per list.
pub const OFFERS: usize = 10;
/// §7.3 step 1 (`0x00576890`): Normal level cap by the NPC's act.
pub const LEVEL_CAP: [u32; 5] = [12, 20, 28, 36, 45];
/// §7.4 step 3: the resurrection cost cap.
pub const RESURRECT_CAP: u32 = 50_000;
/// Pet kind of a hireling (`0x00574EC0(7, …)`).
pub const PET_HIRELING: u8 = 7;

/// The `hireling` fields this spec reads (§Constants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HireRow {
    /// +0x00.
    pub version: u16,
    /// +0x08 (monster class of the mercenary).
    pub class: u32,
    /// +0x0C (1-based).
    pub act: u32,
    /// +0x10 (1-based).
    pub difficulty: u32,
    /// +0x14 (monstats class of the seller).
    pub seller: u32,
    /// +0x18.
    pub gold: u32,
    /// +0x1C.
    pub level: u32,
    /// +0x114 / +0x116 (string ids, `fixups.md` §7).
    pub name_first: u16,
    pub name_last: u16,
}

/// One hire slot (16 bytes: u16 name id, u32 seed @4, u32 hired @8, u32
/// offered @0xC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HireSlot {
    pub name: u16,
    pub seed: u32,
    pub hired: bool,
    pub offered: bool,
}

/// A hire list (record +0x10): the slots 0 … n−1 in use.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HireList {
    pub slots: Vec<HireSlot>,
}

impl HireList {
    fn has_offer(&self) -> bool {
        self.slots.iter().any(|s| s.offered && !s.hired)
    }
}

/// Output of the hire init `0x006637F0` (§7.3 step 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HireOffer {
    /// Index of the picked `hireling` row.
    pub row: usize,
    /// Mercenary level L.
    pub level: i32,
    pub price: u32,
}

/// What the mercenary init (`0x00573270`) receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MercInit {
    /// Index of the `hireling` row.
    pub row: usize,
    pub name: u16,
    /// The slot seed.
    pub seed: u32,
    /// §7.3's offer; `None` for the quest mercenary (§7.5).
    pub offer: Option<HireOffer>,
}

/// §7.3 step 1 / `vendors.md` §2: the Normal level cap.
pub fn capped_level(level: u32, difficulty: u8, act: u8) -> u32 {
    match LEVEL_CAP.get(usize::from(act)) {
        Some(&cap) if difficulty == 0 => level.min(cap),
        _ => level,
    }
}

/// §7.4 step 3 (`0x006637B0`): min((L·L / 2)·15, 50000), signed
/// division, unsigned cap.
pub fn resurrect_cost(level: u32) -> u32 {
    let l = level as i32;
    let cost = (l.wrapping_mul(l) / 2).wrapping_mul(15) as u32;
    cost.min(RESURRECT_CAP)
}

/// `0x006637F0(seed, act, difficulty)` (§7.3 step 5): the offer of a
/// slot seed. `version`: 100 in expansion games, else 0. `None`: no rows.
pub fn hire_init(
    rows: &[HireRow],
    version: u16,
    seed: u32,
    act: u32,
    difficulty: u8,
    player_level: u32,
) -> Option<HireOffer> {
    let mut local = Seed::init_low(seed);
    let (act1, diff1) = (act.wrapping_add(1), u32::from(difficulty) + 1);
    let same = |r: &HireRow| r.act == act1 && r.difficulty == diff1 && r.version == version;
    let first = rows.iter().position(same)?;
    let level = rows[first].level;
    let candidates: Vec<usize> = std::iter::once(first)
        .chain((first + 1..rows.len()).filter(|&i| same(&rows[i]) && rows[i].level == level))
        .collect();
    let row = candidates[local.roll(candidates.len() as i32) as usize];
    let lo = local.step();
    let l = ((lo % 5) as i32)
        .wrapping_add(player_level as i32)
        .wrapping_sub(5)
        .max(2);
    Some(HireOffer {
        row,
        level: l,
        price: price(&rows[row], l),
    })
}

/// §7.3 step 5: gold · (100 + 15·(L − row level)) / 100 (signed), at
/// least the row's gold.
pub fn price(row: &HireRow, l: i32) -> u32 {
    let gold = row.gold as i32;
    let p = gold
        .wrapping_mul(100i32.wrapping_add(15i32.wrapping_mul(l.wrapping_sub(row.level as i32))))
        / 100;
    p.max(gold) as u32
}

impl NpcControl {
    /// Game version of the `hireling` rows.
    fn version(&self) -> u16 {
        if self.expansion {
            100
        } else {
            0
        }
    }

    /// `0x00575FF0` → `0x006564D0`: the first row of a seller for a
    /// difficulty column (1-based) and the game's version.
    pub fn seller_row(&self, seller: u16, difficulty: u32) -> Option<usize> {
        let v = self.version();
        self.hirelings.iter().position(|r| {
            r.seller == u32::from(seller) && r.difficulty == difficulty && r.version == v
        })
    }

    /// `0x00576070(record)` (§7.1): make the hire list of a class when
    /// its record has none yet. A class without a record: nothing.
    pub fn make_hire_list(&mut self, class: u16) -> Result<(), NpcError> {
        let Some(i) = self.records.iter().position(|r| r.class == class) else {
            return Ok(());
        };
        if self.records[i].hire_made {
            return Ok(());
        }
        self.records[i].hire_made = true;
        if self.records[i].hire.is_some() {
            return Ok(());
        }
        let row = self.seller_row(class, 1).ok_or(NpcError::NoHirelingRow {
            seller: class,
            difficulty: 1,
        })?;
        let (first, last) = (
            self.hirelings[row].name_first,
            self.hirelings[row].name_last,
        );
        let n = i32::from(last) - i32::from(first) + 1;
        if n < 1 || n as usize > HIRE_SLOTS {
            return Err(NpcError::HireListSize { first, last });
        }
        let mut slots: Vec<HireSlot> = (0..n as u16)
            .map(|k| HireSlot {
                name: first + k,
                seed: self.seed.step(),
                hired: false,
                offered: false,
            })
            .collect();
        let n = slots.len();
        for _ in 0..OFFERS {
            let s = self.seed.roll(n as i32) as usize;
            let mut p = s;
            let free = loop {
                if !slots[p].offered && !slots[p].hired {
                    break true;
                }
                p = (p + 1) % n;
                if p == s {
                    break false;
                }
            };
            if !free {
                // The probe returned to s: stop at once.
                break;
            }
            slots[p].offered = true;
        }
        self.records[i].hire = Some(HireList { slots });
        Ok(())
    }

    /// `0x00576770(npc, first)` (§7.2): S→C 0x4F, then one 0x4E per
    /// offered, not hired slot. Only the four sellers send. `first` is
    /// passed by every caller; the spec gives it no effect.
    pub fn send_hire_list<W: NpcWorld>(
        &mut self,
        w: &mut W,
        player: UnitId,
        npc: UnitId,
        _first: bool,
    ) -> Result<(), NpcError> {
        let class = w.monster_class(npc).unwrap_or(0);
        if !SELLERS.contains(&class) {
            return Ok(());
        }
        w.send(player, &[0x4F]);
        self.make_hire_list(class)?;
        let slots = self
            .record(class)
            .and_then(|r| r.hire.as_ref())
            .map(|h| h.slots.clone())
            .unwrap_or_default();
        for s in slots.iter().filter(|s| s.offered && !s.hired) {
            let mut m = [0u8; 7];
            m[0] = 0x4E;
            m[1..3].copy_from_slice(&s.name.to_le_bytes());
            m[3..7].copy_from_slice(&s.seed.to_le_bytes());
            w.send(player, &m);
        }
        Ok(())
    }

    /// `0x00577010` (§7.3 step 8): when no slot is offered and not hired,
    /// free the list, clear +0x21 and make a new list.
    fn refill(&mut self, class: u16) -> Result<(), NpcError> {
        let Some(r) = self.record_mut(class) else {
            return Ok(());
        };
        if r.hire.as_ref().is_some_and(HireList::has_offer) {
            return Ok(());
        }
        r.hire = None;
        r.hire_made = false;
        self.make_hire_list(class)
    }

    /// `0x0054BBD0`: C→S 0x36 hire (§7.3) → `0x00577FE0` → `0x005770E0`.
    pub fn hire<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        msg: &[u8],
    ) -> Result<u32, NpcError> {
        if msg.len() != 9 {
            return Ok(3);
        }
        let guid = u32_at(msg, 1);
        let name = u16::from_le_bytes([msg[5], msg[6]]);
        let refuse = |w: &mut W, c: u8| {
            send_code(w, player, c, u32::MAX);
            Ok(0)
        };
        let Some(npc) = w.monster_by_guid(guid) else {
            return refuse(w, code::REFUSED);
        };
        if w.interact_unit(player) != Some((UNIT_MONSTER, guid)) {
            return refuse(w, code::REFUSED);
        }
        let class = w.monster_class(npc).unwrap_or(0);
        // TODO(npc §7.3 steps 1, 3): step 1 needs the record's act before
        // step 3 tests the record; a missing record answers code 9 here.
        let Some(act) = self.record(class).map(|r| r.act) else {
            return refuse(w, code::REFUSED);
        };
        let lvl = capped_level(w.stat(player, stat::LEVEL), self.difficulty, act);
        let f = w.quest_flags(player);
        if (class == QUAL_KEHK && !f.get(36, 0)) || (class == KASHYA && lvl < 8 && !f.get(2, 0)) {
            return refuse(w, code::GATE);
        }
        // TODO(npc §7.3 step 3): a seller without a Normal row or without
        // a hire list is not described (§7.1 asserts); code 9.
        let Some(row) = self.seller_row(class, 1) else {
            return refuse(w, code::REFUSED);
        };
        let (first, last) = (
            self.hirelings[row].name_first,
            self.hirelings[row].name_last,
        );
        if name < first || name > last {
            return refuse(w, code::REFUSED);
        }
        let k = usize::from(name - first);
        let slot = self
            .record(class)
            .and_then(|r| r.hire.as_ref())
            .and_then(|h| h.slots.get(k).copied());
        let Some(slot) = slot.filter(|s| s.name == name && !s.hired) else {
            return refuse(w, code::REFUSED);
        };
        // `0x00663750(expansion, 0, name)` (`hirelings.md` §1.2 rule 3):
        // the first row of the game's version with `Class` 0, else the
        // first whose name range holds the name; its act − 1, none → 0.
        let v = self.version();
        let act = self
            .hirelings
            .iter()
            .filter(|r| r.version == v)
            .find(|r| r.class == 0)
            .or_else(|| {
                self.hirelings
                    .iter()
                    .filter(|r| r.version == v)
                    .find(|r| r.name_first <= name && name <= r.name_last)
            })
            .map_or(0, |r| r.act.wrapping_sub(1));
        // TODO(npc §7.3 step 5): "player level" read as stat 12 (not
        // step 1's capped lvl).
        let level = w.stat(player, stat::LEVEL);
        let Some(offer) = hire_init(
            &self.hirelings,
            self.version(),
            slot.seed,
            act,
            self.difficulty,
            level,
        ) else {
            return Ok(0);
        };
        if !w.pay(player, offer.price) {
            return refuse(w, code::NO_GOLD);
        }
        let merc_class = self.hirelings[offer.row].class;
        let merc = match w.spawn_mercenary(npc, merc_class, 4) {
            Some(m) => Some(m),
            None => w.spawn_mercenary(player, merc_class, 4),
        };
        let Some(merc) = merc else {
            return refuse(w, code::NOT_PLACED);
        };
        if let Some(s) = self
            .record_mut(class)
            .and_then(|r| r.hire.as_mut())
            .and_then(|h| h.slots.get_mut(k))
        {
            s.hired = true;
        }
        let init = MercInit {
            row: offer.row,
            name,
            seed: slot.seed,
            offer: Some(offer),
        };
        w.init_mercenary(player, merc, &init);
        let count = w.interaction(npc).map_or(0, |l| l.nodes.len());
        self.send_hire_list(w, player, npc, count < 2)?;
        let mg = w.guid(merc);
        send_code(w, player, code::MERC, mg);
        self.refill(class)?;
        Ok(0)
    }

    /// `0x0054BC00`: C→S 0x62 resurrect (§7.4) → `0x00579C00`.
    pub fn resurrect<W: NpcWorld + NpcVendors>(
        &mut self,
        w: &mut W,
        player: UnitId,
        msg: &[u8],
    ) -> u32 {
        if !self.expansion || msg.len() != 5 {
            return 3;
        }
        let guid = u32_at(msg, 1);
        let ok = w.monster_by_guid(guid).is_some_and(|npc| {
            w.interact_unit(player) == Some((UNIT_MONSTER, guid))
                && w.monster_class(npc)
                    .is_some_and(|c| RESURRECTORS.contains(&c))
        });
        if !ok {
            send_code(w, player, code::REFUSED, u32::MAX);
            return 0;
        }
        let Some(merc) = w.pet(player, PET_HIRELING, 1) else {
            send_code(w, player, code::REFUSED, u32::MAX);
            return 0;
        };
        // Step 2, d2rs policy (edge case 11; `hirelings.md` §9 rule 3,
        // edge case 5): 1.14d does not test the dead bit and revives (and
        // frees) a living hireling; a living node is answered like a
        // missing one, before the cost. The `(7, 1)` node is living
        // exactly when `(7, 0)` returns the same unit.
        if w.pet(player, PET_HIRELING, 0) == Some(merc) {
            send_code(w, player, code::REFUSED, u32::MAX);
            return 0;
        }
        let cost = resurrect_cost(w.stat(merc, stat::LEVEL));
        if !w.pay(player, cost) {
            send_code(w, player, code::NO_GOLD, u32::MAX);
            return 0;
        }
        w.clear_unit_flag(merc, 0x10000);
        w.set_mode(merc, 1);
        let max = w.max_life(merc);
        w.set_stat(merc, stat::LIFE, max);
        w.revive_mercenary(player, merc);
        w.send(player, &resurrect_message());
        let mg = w.guid(merc);
        send_code(w, player, code::MERC, mg);
        0
    }

    /// `0x00579180` (§7.5): the quest-granted mercenary of `npc_class`.
    pub fn quest_mercenary<W: NpcWorld>(
        &mut self,
        w: &mut W,
        player: UnitId,
        npc_class: u16,
    ) -> Result<(), NpcError> {
        if self
            .record(npc_class)
            .and_then(|r| r.hire.as_ref())
            .is_none()
        {
            return Ok(());
        }
        if self.expansion {
            if w.pet(player, PET_HIRELING, 1).is_some() {
                return self.refill(npc_class);
            }
        } else if w.pet(player, PET_HIRELING, 0).is_some() {
            return Ok(());
        }
        // TODO(npc §7.5): no row for the game's difficulty, or no slot
        // offered and not hired, is not described; nothing happens.
        let Some(row) = self.seller_row(npc_class, u32::from(self.difficulty) + 1) else {
            return Ok(());
        };
        let Some(slot) = self
            .record_mut(npc_class)
            .and_then(|r| r.hire.as_mut())
            .and_then(|h| h.slots.iter_mut().find(|s| s.offered && !s.hired))
        else {
            return Ok(());
        };
        slot.hired = true;
        let slot = *slot;
        let mut m = [0u8; 15];
        m[0] = 0x50;
        m[1..3].copy_from_slice(&2u16.to_le_bytes());
        m[3..5].copy_from_slice(&slot.name.to_le_bytes());
        w.send(player, &m);
        let class = self.hirelings[row].class;
        let Some(merc) = [4, 6, 12]
            .into_iter()
            .find_map(|mode| w.spawn_mercenary(player, class, mode))
        else {
            return Ok(());
        };
        let init = MercInit {
            row,
            name: slot.name,
            seed: slot.seed,
            offer: None,
        };
        w.init_mercenary(player, merc, &init);
        self.refill(npc_class)
    }
}
