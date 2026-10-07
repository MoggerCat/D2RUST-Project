// Spec: specs/formats/d2s-load.md; specs/world/hirelings-2.md §19; specs/formats/d2s.md §2.2 r7, §2.2 r8, §2.2 r9, §2.4 r4, §2.4 r5, §2.4 r6, §8.2 r7, §8.5 r2, §9; specs/world/quests.md §1.6
//! Character storage: what loading a parsed `.d2s` does to the game
//! (`formats/d2s-load.md`). `d2_formats::d2s` parses and checks the bytes
//! (`formats/d2s.md` §1–§8, §10); [`load`] then runs either the
//! new-character start (load §1, `0x00569F80`) or the load effects of a
//! full save (load §2, `0x0056B180`), step by step in the master's order,
//! each step on its owner's seam ([`CharacterWorld`]).
//!
//! What is decided here (the rules `formats/d2s.md` and
//! `world/quests.md` §1.6 own): the header's client fields (§2.2 rules
//! 7–9: act from the town byte, the map seed condition), the hotkey and
//! mouse-skill decode (§2.4 rule 4), the quest record normalisation, the
//! stat entries, the item flags of a loaded record (§8.2 rule 7,
//! [`d2_formats::d2s::item_flags_on_load`]), the golem's skill 90 check
//! (§8.5 rule 2) and the post-load values (§9 rule 4: gold limits,
//! stamina, hitpoints and mana, stats 67–69 and 30).
//!
//! The player unit exists before the load here (the session joins it,
//! `session::enter_game`): the 1.14d header step allocates it. The game
//! entry after a successful load (the quest entry with mode 0 and the
//! messages, `0x005344B0` / `0x00534520` → `0x00546270`) is the caller's
//! (`session::enter_game_from_save`).

use d2_formats::d2s::{self, Corpse, D2s, Header, Hireling, ItemEntry, Slot, StatEntry};
use d2_sim::combat::vitals::{init_player_stats, VitalsUnits};
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{HirelingCall, Pending, View};
use d2_sim::world::hirelings::life::{Loader, SavedHireling};

/// Stat ids the load reads or writes (`formats/d2s.md` §9).
pub mod stat {
    pub const HITPOINTS: u16 = 6;
    pub const MANA: u16 = 8;
    pub const STAMINA: u16 = 10;
    pub const MAXSTAMINA: u16 = 11;
    pub const LEVEL: u16 = 12;
    pub const GOLD: u16 = 14;
    pub const GOLDBANK: u16 = 15;
    pub const NEXTEXP: u16 = 30;
    pub const VELOCITYPERCENT: u16 = 67;
    pub const ATTACKRATE: u16 = 68;
    pub const OTHER_ANIMRATE: u16 = 69;
}

/// Iron Golem skill (§8.5 rule 2).
pub const SKILL_IRON_GOLEM: u16 = 90;
/// Gold carry limit per character level (§9 rule 4, `0x00622E70`).
pub const GOLD_PER_LEVEL: i32 = 10_000;
/// Stash gold limit (§9 rule 4, `0x00623460`).
pub const GOLDBANK_MAX: i32 = 2_500_000;
/// Quest slots the normalisation covers (`world/quests.md` §1.6: 42).
pub const QUEST_SLOTS: usize = 42;

/// What the game knows at load time (§2.2 rule 8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadContext {
    /// Game +0x6D (0 Normal, 1 Nightmare, 2 Hell).
    pub difficulty: u8,
    /// Game +0x6A = 3 and game +0x84 = 0: a town byte with 0x80 then
    /// sets game +0x7C to the save's map seed (§2.2 rule 8, Open
    /// question 9).
    pub map_seed_applies: bool,
}

/// One decoded hotkey or mouse skill (§2.4 rule 4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillSlot {
    /// −1: no skill.
    pub skill: i32,
    pub left: bool,
    /// 1-based inventory position; −1: none.
    pub item: i32,
}

impl From<Slot> for SkillSlot {
    fn from(s: Slot) -> Self {
        let (skill, left, item) = s.decode();
        SkillSlot { skill, left, item }
    }
}

/// The client and player fields the header step sets (§2.2 rules 8–9,
/// §2.4 rules 4–5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderLoad {
    /// Client weapon switch := +0x10 & 1.
    pub weapon_switch: bool,
    /// Client status := +0x24.
    pub status: u16,
    /// Client create time := +0x2C (rule 8; its only setter, rule 10).
    pub create_time: u32,
    /// Client act := (byte +0xA8 + difficulty) & 0x7F, 0 when ≥ 5.
    pub act: u8,
    /// Game +0x7C := +0xAB when the town byte has 0x80 and
    /// [`LoadContext::map_seed_applies`].
    pub map_seed: Option<u32>,
    /// Hotkeys to the client (`0x005390A0`).
    pub hotkeys: [SkillSlot; 16],
    /// Left, right, swap left, swap right (player data +0x70..+0x8C).
    pub mouse: [SkillSlot; 4],
    /// Client +0x480 := +0xCF (+0x481, +0x482 := 0).
    pub client_480: u8,
}

/// §2.2 rule 8. "t = byte +0xA8 + game difficulty" reads the town byte
/// at +0xA8 + difficulty (one byte per difficulty, §2.1).
pub fn header_load(h: &Header, ctx: &LoadContext) -> HeaderLoad {
    let t = h
        .towns
        .get(usize::from(ctx.difficulty))
        .copied()
        .unwrap_or(0);
    let act = t & 0x7F;
    HeaderLoad {
        weapon_switch: h.weapon_switch & 1 != 0,
        status: h.status,
        create_time: h.create_time,
        act: if act >= 5 { 0 } else { act },
        map_seed: (t & 0x80 != 0 && ctx.map_seed_applies).then_some(h.map_seed),
        hotkeys: h.hotkeys.map(SkillSlot::from),
        mouse: h.mouse.map(SkillSlot::from),
        client_480: h.client_cf,
    }
}

/// `0x0065C4D0` with normalize = 1 (`world/quests.md` §1.6): for every
/// slot q = 0..41 (u16 at 2q), clear bits 13 and 14, then bit 1 set →
/// set bit 15. Bytes past the 42 slots are kept.
pub fn normalise_quests(rec: &[u8; 96]) -> [u8; 96] {
    let mut out = *rec;
    for q in 0..QUEST_SLOTS {
        let at = 2 * q;
        let mut v = u16::from_le_bytes([out[at], out[at + 1]]);
        v &= !((1 << 13) | (1 << 14));
        if v & (1 << 1) != 0 {
            v |= 1 << 15;
        }
        out[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    out
}

/// §9 rule 4 gold limits: gold < 0 or above level × 10,000 → 0; goldbank
/// < 0 or above 2,500,000 → 0.
pub fn gold_limits(gold: i32, goldbank: i32, level: i32) -> (i32, i32) {
    let carry = level.wrapping_mul(GOLD_PER_LEVEL);
    let g = if gold < 0 || gold > carry { 0 } else { gold };
    let b = if !(0..=GOLDBANK_MAX).contains(&goldbank) {
        0
    } else {
        goldbank
    };
    (g, b)
}

/// The item lists of a save (§8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemList {
    Player,
    Corpse,
    Hireling,
    Golem,
}

/// A load step of `formats/d2s-load.md` §2 (and the new-character start
/// of §1) that the provider could not apply, with its owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unapplied {
    pub step: &'static str,
    pub reason: &'static str,
}

/// The seams of the load steps, each with the owner spec of
/// `formats/d2s-load.md`. Each step returns `Err(Unapplied)` when its
/// provider does not exist yet (named, never guessed); [`load`] records
/// that and goes on, in the master's order.
pub trait CharacterWorld {
    // ---- load §1: new-character start (`0x00569F80`)

    /// Client +0x3D4 |= 1, the unit's set-up (`0x00569F20`: mode 1,
    /// player data +0x50..+0x5C := 0) and its add messages
    /// (`sim/intents-events.md` §7.2).
    fn new_character_setup(&mut self) -> Result<(), Unapplied>;
    /// Start stats for the client act (`0x005706D0`, `combat/vitals.md`
    /// `init_player_stats`).
    fn start_stats(&mut self, act: u8) -> Result<(), Unapplied>;
    /// Start items (`0x00534F10`, `items/generation.md` §10.3).
    fn start_items(&mut self) -> Result<(), Unapplied>;
    /// The class's `charstats` `StartSkill` (record +0xAC); 0: none.
    fn start_skill(&self) -> Result<u16, Unapplied>;
    /// The player has the skill (`0x006439F0`).
    fn has_skill(&self, skill: u16) -> Result<bool, Unapplied>;
    /// Player data +0x70 / +0x74 (right, left) := 0, then the right skill
    /// selected with no item (`0x005701B0`, item −1) when given.
    fn set_mouse_skills(&mut self, right: Option<u16>) -> Result<(), Unapplied>;
    /// The quest entry `0x00546270` with `mode` (`world/quests.md` §3).
    fn quest_entry(&mut self, mode: u8) -> Result<(), Unapplied>;

    // ---- load §2: a full save (`0x0056B180`)

    /// `0x0056A090` (§2.2 rules 8–9, §2.4 rules 4–5).
    fn apply_header(&mut self, h: &HeaderLoad) -> Result<(), Unapplied>;
    /// `0x0056A370` → `0x0065C4D0`: the three records, normalised.
    fn set_quests(&mut self, records: &[[u8; 96]; 3]) -> Result<(), Unapplied>;
    /// `0x0056A3E0` (`world/waypoints.md` §3): the 16-byte records.
    fn set_waypoints(&mut self, records: &[[u8; 16]; 3]) -> Result<(), Unapplied>;
    /// `0x0056A470` (§6): fields A and B per difficulty.
    fn set_npc_fields(&mut self, a: &[[u8; 8]; 3], b: &[[u8; 8]; 3]) -> Result<(), Unapplied>;
    /// `0x0056A4F0` / `0x0056A620` (§7.1): one base stat.
    fn set_base_stat(&mut self, id: u16, layer: u16, value: i32) -> Result<(), Unapplied>;
    /// The total of a stat (`0x00625480`, layer 0).
    fn stat(&self, id: u16) -> Result<i32, Unapplied>;
    /// `0x0056A710` → `0x0056DEB0` (§7.2): the base level of the class
    /// skill at list index `index` added.
    fn add_skill_level(&mut self, index: usize, level: u8) -> Result<(), Unapplied>;
    /// `0x0056A7E0` / `0x0056A830` / `0x0056AC10` / `0x0056AE50`: the
    /// list's entries created (§8.2: flags of [`d2s::item_flags_on_load`],
    /// replenish timers), placed and their children inserted (the golem
    /// item gets mode 3 and goes to the client).
    fn create_items(&mut self, list: ItemList, entries: &[ItemEntry]) -> Result<(), Unapplied>;
    /// `0x0056A830` (§8.3): the corpse unit, linked to the player (its
    /// items go through [`CharacterWorld::create_items`] after it).
    fn create_corpse(&mut self, corpse: &Corpse) -> Result<(), Unapplied>;
    /// `0x0056AA50` (`world/hirelings.md` §10 rules 1–7).
    fn restore_hireling(&mut self, block: &Hireling) -> Result<(), Unapplied>;
    /// `world/hirelings.md` §10 rule 8, after the hireling's items.
    fn hireling_items_loaded(&mut self) -> Result<(), Unapplied>;
    /// Post-load (`0x0056AF20`, §2.4 rule 6): item indices → GUIDs and
    /// the left and right skills selected with their item.
    fn resolve_item_indices(&mut self) -> Result<(), Unapplied>;
    /// `0x00611800(class, level)`: the experience of the next level.
    fn next_exp(&self, level: i32) -> Result<i32, Unapplied>;
}

/// Why a load failed (internal codes of `formats/d2s.md` §10).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    /// §8.5 rule 2: a golem item without a skill 90 entry (code 23).
    #[error("golem item without the Iron Golem skill (internal code 23)")]
    GolemSkill,
}

impl LoadError {
    /// The load result of the error (§10 rule 1's table, `0x006E1208`):
    /// the code §8.2 rule 2's join reports.
    pub fn result(&self) -> u32 {
        match self {
            // Internal 23 → 10.
            LoadError::GolemSkill => 10,
        }
    }
}

/// What the load did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Load §1 ran (the 335-byte stub).
    pub new_character: bool,
    /// The client act (game entry's act).
    pub act: u8,
    /// Steps the provider could not apply, in order.
    pub unapplied: Vec<Unapplied>,
    /// Load §1: the right skill selected (`StartSkill`), if any.
    pub right_skill: Option<u16>,
}

fn note(r: &mut LoadReport, res: Result<(), Unapplied>) {
    if let Err(u) = res {
        r.unapplied.push(u);
    }
}

/// Runs the load of `save` (parsed and checked by `d2_formats::d2s`) on
/// `w`: load §1 for a stub, load §2 otherwise.
pub fn load(
    save: &D2s,
    ctx: &LoadContext,
    w: &mut dyn CharacterWorld,
) -> Result<LoadReport, LoadError> {
    let mut r = LoadReport::default();
    let Some(body) = &save.body else {
        // Load §1, §2.2 rule 7: the stub; the client act stays 0 (no
        // header step runs), so `StartSkill` applies (edge case 1).
        return Ok(load_new_character(w));
    };
    let h = header_load(&save.header, ctx);
    r.act = h.act;
    note(&mut r, w.apply_header(&h));
    let quests = body.quests.records.map(|q| normalise_quests(&q));
    note(&mut r, w.set_quests(&quests));
    note(&mut r, w.set_waypoints(&body.waypoints.records));
    note(&mut r, w.set_npc_fields(&body.npcs.a, &body.npcs.b));
    // Stats (§7.1), then hitpoints and mana remembered (§9 rule 2).
    for StatEntry { id, layer, value } in body.stats.entries() {
        note(&mut r, w.set_base_stat(id, layer, value));
    }
    let remembered = (w.stat(stat::HITPOINTS), w.stat(stat::MANA));
    // Skills (§7.2): +0x2A bytes, one base level each.
    for (i, &lvl) in body.skills.iter().enumerate() {
        if lvl != 0 {
            note(&mut r, w.add_skill_level(i, lvl));
        }
    }
    note(&mut r, w.create_items(ItemList::Player, &body.items));
    for c in &body.corpses {
        note(&mut r, w.create_corpse(c));
        note(&mut r, w.create_items(ItemList::Corpse, &c.items));
    }
    // Hireling (§2.5 rule 2, classic and expansion alike; the restore
    // itself answers "no hireling", `world/hirelings.md` §10 rule 1),
    // then its items (§8.4, read only for a restored hireling).
    note(&mut r, w.restore_hireling(&save.header.hireling));
    if let Some(Some(items)) = &body.hireling_items {
        note(&mut r, w.create_items(ItemList::Hireling, items));
        note(&mut r, w.hireling_items_loaded());
    }
    // Golem (§8.5 rule 2).
    if let Some(g) = &body.golem {
        if let Some(item) = &g.item {
            match w.has_skill(SKILL_IRON_GOLEM) {
                Ok(true) => note(
                    &mut r,
                    w.create_items(ItemList::Golem, std::slice::from_ref(item)),
                ),
                Ok(false) => return Err(LoadError::GolemSkill),
                Err(u) => r.unapplied.push(u),
            }
        }
    }
    post_load(w, remembered, &mut r);
    Ok(r)
}

/// Load §1 on `w` (the stub's load, and a new character,
/// `sim/intents-events.md` §8.2 rule 7): the client act stays 0 (no
/// header step runs), so `StartSkill` applies (edge case 1).
pub fn load_new_character(w: &mut dyn CharacterWorld) -> LoadReport {
    let mut r = LoadReport {
        new_character: true,
        act: 0,
        ..LoadReport::default()
    };
    r.right_skill = new_character(w, r.act, &mut r);
    r
}

/// Load §1 rule 1 after the unit exists; the right skill selected.
fn new_character(w: &mut dyn CharacterWorld, act: u8, r: &mut LoadReport) -> Option<u16> {
    note(r, w.new_character_setup());
    note(r, w.start_stats(act));
    note(r, w.start_items());
    let right = if act == 0 {
        match w.start_skill() {
            Ok(0) => None,
            Ok(s) => match w.has_skill(s) {
                Ok(true) => Some(s),
                Ok(false) => None,
                Err(u) => {
                    // PROVISIONAL (formats/d2s-load.md §1 rule 1, §8 rule 3;
                    // REC-02): with no skill-list provider the start skill
                    // is taken as present, as in the fresh 1.14d saves
                    // (Sorceress 36, Necromancer 70 = `StartSkill`).
                    r.unapplied.push(u);
                    Some(s)
                }
            },
            Err(u) => {
                r.unapplied.push(u);
                None
            }
        }
    } else {
        None
    };
    note(r, w.set_mouse_skills(right));
    note(r, w.quest_entry(1));
    right
}

/// §9 rule 4 (`0x0056AF80`): gold limits, stamina, item indices,
/// hitpoints and mana, stats 67–69, then stat 30.
fn post_load(
    w: &mut dyn CharacterWorld,
    remembered: (Result<i32, Unapplied>, Result<i32, Unapplied>),
    r: &mut LoadReport,
) {
    let read = |w: &dyn CharacterWorld, id, r: &mut LoadReport| match w.stat(id) {
        Ok(v) => Some(v),
        Err(u) => {
            r.unapplied.push(u);
            None
        }
    };
    let gold = read(w, stat::GOLD, r);
    let bank = read(w, stat::GOLDBANK, r);
    let level = read(w, stat::LEVEL, r);
    if let (Some(g), Some(b), Some(l)) = (gold, bank, level) {
        let (g2, b2) = gold_limits(g, b, l);
        if g2 != g {
            note(r, w.set_base_stat(stat::GOLD, 0, g2));
        }
        if b2 != b {
            note(r, w.set_base_stat(stat::GOLDBANK, 0, b2));
        }
    }
    if let Some(max) = read(w, stat::MAXSTAMINA, r) {
        note(r, w.set_base_stat(stat::STAMINA, 0, max));
    }
    note(r, w.resolve_item_indices());
    for (id, v) in [(stat::HITPOINTS, remembered.0), (stat::MANA, remembered.1)] {
        match v {
            Ok(v) => note(r, w.set_base_stat(id, 0, v)),
            Err(u) => r.unapplied.push(u),
        }
    }
    for id in [
        stat::VELOCITYPERCENT,
        stat::ATTACKRATE,
        stat::OTHER_ANIMRATE,
    ] {
        note(r, w.set_base_stat(id, 0, 100));
    }
    if let Some(l) = level {
        match w.next_exp(l) {
            Ok(n) => note(r, w.set_base_stat(stat::NEXTEXP, 0, n)),
            Err(u) => r.unapplied.push(u),
        }
    }
}

/// The load steps on the action wiring's unit side (`d2_sim::wiring::action::View`):
/// the player's stat list (set, totals, `maxstamina`), the start stats
/// (`combat/vitals.md` `init_player_stats`), `StartSkill` and the
/// experience table (`VitalsTables`). Every other step has no provider in
/// `d2-sim` yet and is reported as [`Unapplied`] with its owner.
pub struct ActionCharacter<'v, 'a, X> {
    pub v: &'v mut View<'a, X>,
    pub player: UnitId,
}

fn unapplied<T>(step: &'static str, reason: &'static str) -> Result<T, Unapplied> {
    Err(Unapplied { step, reason })
}

impl<X: Pending> ActionCharacter<'_, '_, X> {
    fn class(&self) -> i32 {
        self.v.units.get(self.player).map_or(0, |r| r.class as i32)
    }
}

impl<X: Pending> CharacterWorld for ActionCharacter<'_, '_, X> {
    fn new_character_setup(&mut self) -> Result<(), Unapplied> {
        unapplied(
            "new character set-up",
            "client +0x3D4, `0x00569F20` mode / player data and the add messages: \
             player data and part B of the add messages are not in d2-sim (intents-events.md §7.2)",
        )
    }
    fn start_stats(&mut self, act: u8) -> Result<(), Unapplied> {
        let Some(t) = self.v.h.vitals.clone() else {
            return unapplied("start stats", "no vitals tables on the action wiring");
        };
        init_player_stats(&mut *self.v, &t, self.player, u32::from(act));
        Ok(())
    }
    fn start_items(&mut self) -> Result<(), Unapplied> {
        unapplied(
            "start items",
            "`0x00534F10` (items/generation.md §10.3) has no d2-sim caller on a player unit",
        )
    }
    fn start_skill(&self) -> Result<u16, Unapplied> {
        let Some(t) = self.v.h.vitals.as_ref() else {
            return unapplied("start skill", "no vitals tables on the action wiring");
        };
        Ok(t.charstats(self.class()).map_or(0, |c| c.startskill))
    }
    fn has_skill(&self, _: u16) -> Result<bool, Unapplied> {
        unapplied(
            "has skill",
            "the player's skill list (`0x006439F0`) has no provider here",
        )
    }
    fn set_mouse_skills(&mut self, _: Option<u16>) -> Result<(), Unapplied> {
        unapplied(
            "mouse skills",
            "player data +0x70..+0x8C and `0x005701B0` are not in d2-sim",
        )
    }
    fn quest_entry(&mut self, _: u8) -> Result<(), Unapplied> {
        unapplied(
            "quest entry",
            "`0x00546270` runs on the host's quest control (WiredWorld), not the action wiring",
        )
    }
    fn apply_header(&mut self, _: &HeaderLoad) -> Result<(), Unapplied> {
        unapplied(
            "header",
            "client record fields (+0x1AC act, status, hotkeys, +0x480) have no home in d2-sim",
        )
    }
    fn set_quests(&mut self, _: &[[u8; 96]; 3]) -> Result<(), Unapplied> {
        unapplied(
            "quests",
            "the player's quest records live in the host rest (QuestRest::quests)",
        )
    }
    fn set_waypoints(&mut self, _: &[[u8; 16]; 3]) -> Result<(), Unapplied> {
        unapplied("waypoints", "player data +0x1C is not in d2-sim")
    }
    fn set_npc_fields(&mut self, _: &[[u8; 8]; 3], _: &[[u8; 8]; 3]) -> Result<(), Unapplied> {
        unapplied("npc fields", "player data +0x60 is not in d2-sim")
    }
    fn set_base_stat(&mut self, id: u16, layer: u16, value: i32) -> Result<(), Unapplied> {
        let v = &mut *self.v;
        v.stats.unit_set(&mut *v.h, self.player, id, value, layer);
        Ok(())
    }
    fn stat(&self, id: u16) -> Result<i32, Unapplied> {
        if id == stat::MAXSTAMINA {
            return Ok(VitalsUnits::max_stamina(&*self.v, self.player));
        }
        Ok(self.v.stats.unit_total(self.player, id, 0))
    }
    fn add_skill_level(&mut self, _: usize, _: u8) -> Result<(), Unapplied> {
        unapplied(
            "skills",
            "`0x0056DEB0` on a player unit (skills/levels.md) has no provider here",
        )
    }
    fn create_items(&mut self, _: ItemList, entries: &[ItemEntry]) -> Result<(), Unapplied> {
        if entries.is_empty() {
            return Ok(());
        }
        unapplied(
            "items",
            "item units from save records (`0x00558CB0`, `0x00531210`) have no d2-sim body",
        )
    }
    fn create_corpse(&mut self, _: &Corpse) -> Result<(), Unapplied> {
        unapplied(
            "corpse",
            "the corpse unit (`0x0056A830`) has no d2-sim body",
        )
    }
    /// `0x0056AA50`: queued for the host that holds the hireling lists
    /// (`ActionHooks::hireling_calls`, `HirelingCall::Restore`; the
    /// wired host runs `hirelings.md` §10 rules 1–7 on it, with the
    /// roomless allocation of `hirelings-2.md` §16 rule 3). The parser
    /// takes versions ≥ 0x5C only (`formats/d2s.md` §1 rule 6), so the
    /// loader is always the ≥ 0x5C one (`Loader::Current`).
    fn restore_hireling(&mut self, block: &Hireling) -> Result<(), Unapplied> {
        if !block.is_present() {
            // `world/hirelings.md` §10 rule 1: no hireling.
            return Ok(());
        }
        let Some(q) = self.v.h.hireling_calls.as_mut() else {
            return unapplied(
                "hireling",
                "no host holds the hireling lists (`ActionHooks::hireling_calls` is off)",
            );
        };
        q.push(HirelingCall::Restore {
            player: self.player,
            saved: SavedHireling {
                dead: block.flags & Hireling::DEAD != 0,
                seed: block.seed,
                name_index: block.name_index,
                id: block.id,
                experience: block.experience,
            },
            loader: Loader::Current,
        });
        Ok(())
    }
    fn hireling_items_loaded(&mut self) -> Result<(), Unapplied> {
        unapplied(
            "hireling rule 8",
            "the hireling's items are not created (see the items step), so rule 8 does not run",
        )
    }
    fn resolve_item_indices(&mut self) -> Result<(), Unapplied> {
        unapplied(
            "item indices",
            "`0x0056AF20` needs the hotkeys and the player's inventory list",
        )
    }
    fn next_exp(&self, level: i32) -> Result<i32, Unapplied> {
        let Some(t) = self.v.h.vitals.as_ref() else {
            return unapplied("nextexp", "no vitals tables on the action wiring");
        };
        Ok(t.threshold(self.class(), level as u32) as i32)
    }
}

/// The flags each record of a loaded item entry gets (§8.2 rule 7), for
/// a provider that creates items from records.
pub fn loaded_flags(stored: u32) -> u32 {
    d2s::item_flags_on_load(stored)
}

/// Load result → (m, string id) of the refusal message (load §5 rule 2,
/// client handler `0x0045C6D0` / `0x0044E380`). `alt_bit` is config byte
/// +0x1EF bit 0x20: results 17 and 18 then show 0x5522 / 0x5521. A
/// result outside 1..=26 gives m = 9 and string 5372 (rule 1's "any
/// other value").
pub fn refusal_message(result: u32, alt_bit: bool) -> (u8, u16) {
    // (m, string id) for results 1..=26, in order.
    const T: [(u8, u16); 26] = [
        (0, 5365),
        (1, 5366),
        (2, 5367),
        (3, 5368),
        (4, 5369),
        (5, 5371),
        (10, 5373),
        (11, 5374),
        (12, 5375),
        (13, 5376),
        (14, 5377),
        (15, 5378),
        (16, 5379),
        (17, 5380),
        (18, 5381),
        (19, 5360),
        (20, 5364),
        (21, 5363),
        (22, 5362),
        (23, 5361),
        (24, 5359),
        (9, 5372),
        (25, 10101),
        (26, 10102),
        (27, 5370),
        (28, 5371),
    ];
    match result {
        1..=26 => {
            let (m, id) = T[result as usize - 1];
            match (alt_bit, m) {
                (true, 20) => (m, 0x5522),
                (true, 21) => (m, 0x5521),
                _ => (m, id),
            }
        }
        _ => (9, 5372),
    }
}
pub mod save;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_fitems;
