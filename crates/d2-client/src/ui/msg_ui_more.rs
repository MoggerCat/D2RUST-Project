// Spec: specs/client/msg-ui.md (§4 r4, §7 r2 code 2, §7 r4–r5, §9 r3–r4, §10 r2, §11 r2, §12 r2, §13 r2, §14 r2, §15 r2, §16 r2, r7)
//! The UI consumer of the bridge outputs that only store UI globals or
//! make a small, fully specified UI call: 0x50 (hire popup), 0x8A (NPC
//! interact), 0x91, 0x78, 0x29, 0x52, 0x5E, 0x9B, 0x28 (client quest
//! record) and the overhead text store of 0x26 (`client/msg-ui.md` §4
//! r4). It is a child of [`super`] so it reads the same private state.
//! Calls whose callee no spec gives are named in the outcome's `skipped`
//! list, as in [`super::skip`].

use super::{OriginalUi, OriginalUiError, ON};
use crate::bridge::output::Output;
use crate::bridge::world::{UnitKey, MONSTER};

/// UI state 0x23 (`ui/ui-states.tsv`, UI_HIREICONS).
pub const UI_HIREICONS: u32 = 0x23;

/// Class `act5pow` (§9 r3).
pub const CLASS_ACT5POW: u32 = 534;
/// Class `act2guard2` (§9 r3).
pub const CLASS_ACT2GUARD2: u32 = 331;
/// Overlay shown on an NPC that wants to talk (§9 r3).
pub const OVERLAY_NPC_WANTS: u16 = 72;

/// Parts the dispatches of this module skip, named.
pub mod skip {
    pub const QUEST_LOG_TAIL: &str =
        "0x52: 0x00483350 and the tab rebuild 0x004A23D0 / 0x004A3220 (msg-ui §13 r2; ui/*)";
    pub const MERC_MENU: &str =
        "0x9B: the NPC-menu edit 0x004B6440 (ui/panels.md §14; msg-ui §15 r2)";
    pub const CHAT_FILTER: &str =
        "0x26: the text filter object 0x00611560 (msg-ui §4 r3.2) is not specified: nothing is filtered";
}

/// One entry of the NPC intro table `0x00726850` (§10 r2): the NPC
/// class (+0) and the flag byte (+0x12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntroEntry {
    pub class: u32,
    pub flag: u8,
}

/// The overhead text record of a unit (§4 r4, `0x00661110`): duration,
/// end, language and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverheadText {
    /// +0: d = 8 · min(n, 254) + 125 for a text of n characters.
    pub duration: u32,
    /// +4: the overhead counter at creation plus d.
    pub end: u32,
    /// +8.
    pub lang: u8,
    /// +0x10: at most 254 characters.
    pub text: Vec<u8>,
}

/// The UI globals these dispatches write, by 1.14d address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MsgUiMore {
    /// The intro table `0x00726850` (input: its rows come from the
    /// game data).
    pub intro_table: Vec<IntroEntry>,
    /// The monstats row count (data +0xA80), the bound of §10 r2.
    pub monstats_rows: u32,
    /// The registry value `PopupHireling` (missing = 0, §7 r4).
    pub popup_hireling: u32,
    /// `[0x007BEECC]`.
    pub hire_7beecc: u32,
    /// `[0x007BEEE4]`: the one-time hireling-panel open (§7 r4).
    pub hire_7beee4: u32,
    /// `[0x007BF2A4]`, `[0x007BF2A8]`, `[0x007BF2AC]` (§7 r2 code 1).
    pub quest_words: [i32; 3],
    /// `[0x007BF098..0x007BF0A3]` := bytes 3–14 (§7 r2 code 4).
    pub quest_special_bytes: [u8; 12],
    /// `[0x007BF254]`.
    pub quest_7bf254: u32,
    /// `[0x0070EE8C]` and `[0x007A0674]` (§7 r2 code 23).
    pub flag_70ee8c: u32,
    pub flag_7a0674: u32,
    /// `[0x007C025F]` and `[0x007C0265]` (§7 r2 code 36).
    pub zoo_latch: bool,
    pub zoo_word: u16,
    /// The trade partner name (§11 r2: byte 15 forced to 0); the wide
    /// conversion `0x00526F20` keeps the bytes as characters.
    pub partner_name: [u8; 16],
    /// `[0x007C0E60]`.
    pub partner_guid: u32,
    /// The game quest record `[0x007C0D47]` (§12 r2).
    pub game_quest_record: [u8; 96],
    /// The client quest record `[0x007C0D43]` (§16 r7): zeroed at game
    /// UI start and on a local-player change, overwritten by 0x28 only.
    pub client_quest: [u8; 96],
    /// The quest-log status bytes `[0x007BF356 + i]` (§13 r2).
    pub quest_log_status: [u8; 41],
    /// `[0x007BF2B0]`.
    pub quest_7bf2b0: u32,
    /// `[0x007C0EA4..0x007C0EC8]` and `[0x007C0ECC]` (§14 r2).
    pub quest_avail: [u8; 37],
    pub quest_avail_set: bool,
    /// `[0x00725494]` (−1: the hireling is alive) and `[0x007C0DD0]`
    /// (§15 r2).
    pub merc_state: u16,
    pub merc_7c0dd0: u16,
    /// The `0x004B6440` EAX values run so far, in order (§15 r2).
    pub merc_menu_calls: Vec<u8>,
    /// `[0x007C0D25]` / `[0x007C0D29]`: the interact NPC and whether an
    /// interaction is active (UI state, `ui/messages.md` §14).
    pub interact_npc: u32,
    pub interact_active: bool,
    /// `0x004B1620()` (§9 r3), a UI state the dispatch reads.
    pub f4b1620: bool,
    /// UI sounds played on a unit (`0x004B9A00(id, unit, 0, 0, 0)`).
    pub unit_sounds: Vec<(i32, UnitKey)>,
    /// Overlays created on a unit (`0x00470390(unit, overlay, 3, …)`).
    pub overlays: Vec<(UnitKey, u16)>,
    /// The overhead text records by unit key (§4 r4).
    pub overhead: std::collections::BTreeMap<UnitKey, OverheadText>,
    /// `[0x007BF20E]`: steps once per overhead draw.
    pub overhead_counter: u32,
    /// The client's own language id (`0x00525150`, 0–13).
    pub own_lang: u8,
}

impl Default for MsgUiMore {
    fn default() -> Self {
        Self {
            intro_table: Vec::new(),
            monstats_rows: 0,
            popup_hireling: 0,
            hire_7beecc: 0,
            hire_7beee4: 0,
            quest_words: [0; 3],
            quest_special_bytes: [0; 12],
            quest_7bf254: 0,
            flag_70ee8c: 0,
            flag_7a0674: 0,
            zoo_latch: false,
            zoo_word: 0,
            partner_name: [0; 16],
            partner_guid: 0,
            game_quest_record: [0; 96],
            client_quest: [0; 96],
            quest_log_status: [0; 41],
            quest_7bf2b0: 0,
            quest_avail: [0; 37],
            quest_avail_set: false,
            merc_state: 0,
            merc_7c0dd0: 0,
            merc_menu_calls: Vec::new(),
            interact_npc: 0,
            interact_active: false,
            f4b1620: false,
            unit_sounds: Vec::new(),
            overlays: Vec::new(),
            overhead: std::collections::BTreeMap::new(),
            overhead_counter: 0,
            own_lang: 0,
        }
    }
}

/// The `0x0049E280(text, lang)` refusal of §4 r3.3: languages 6, 7, 9,
/// 12 convert with `MultiByteToWideChar`; when `lang` differs from the
/// client's own language id and the pair is not {7, 12}, a text with a
/// character ≥ 0x80 is refused. `true`: the text is converted.
pub fn text_converts(text: &[u8], lang: u8, own_lang: u8) -> bool {
    let pair_7_12 = matches!((lang, own_lang), (7, 12) | (12, 7));
    lang == own_lang || pair_7_12 || text.iter().all(|&c| c < 0x80)
}

/// What a delivered `ChatLine` asks the screen to show (§4 r3.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatAction {
    /// Type 4, or type 1 with u8@3 ∈ {0, 1}: a screen message.
    ScreenMessage(Vec<u8>),
    /// Type 6: a formatted line, then the screen message.
    Formatted(Vec<u8>),
    /// Types 2 and 1 otherwise: a line with the name.
    Named { name: Vec<u8>, text: Vec<u8> },
    /// Type 5 with the unit present: overhead text of the unit.
    Overhead(UnitKey, Vec<u8>),
    /// Type 7: `0x0048BBE0(u8@8)`.
    Type7(u8),
}

impl OriginalUi {
    /// The UI globals of [`MsgUiMore`].
    pub fn more(&self) -> &MsgUiMore {
        &self.more
    }

    /// Writable inputs of [`MsgUiMore`] (tables, registry values).
    pub fn more_mut(&mut self) -> &mut MsgUiMore {
        &mut self.more
    }

    /// The UI dispatch of the outputs this module owns; `Ok(false)`: not
    /// one of them.
    pub(super) fn apply_more(&mut self, o: &Output) -> Result<bool, OriginalUiError> {
        match *o {
            Output::QuestSpecial { code, words } => self.quest_special(code, &words)?,
            Output::NpcInteract {
                unit,
                present,
                class,
                mdata_3c,
                blocker_open,
            } => self.npc_interact(unit, present, class, mdata_3c, blocker_open),
            Output::NpcIntro { slots } => self.npc_intro(&slots),
            Output::TradePartner { name, guid } => {
                self.more.partner_name = name;
                self.more.partner_name[15] = 0;
                self.more.partner_guid = guid;
            }
            Output::GameQuestFlags { record } => self.more.game_quest_record = record,
            Output::QuestLog { status } => {
                self.more.quest_log_status = status;
                self.more.quest_7bf2b0 = 0;
                self.skip(skip::QUEST_LOG_TAIL);
            }
            Output::QuestAvailability { bytes } => {
                self.more.quest_avail = bytes;
                self.more.quest_avail_set = true;
            }
            Output::HireListReset => self.hire.borrow_mut().reset(),
            Output::HireOffer { name, seed } => self.hire.borrow_mut().offer(name, seed),
            Output::MercRevive { state, value } => self.merc_revive(state, value),
            Output::QuestFlags { record } => self.more.client_quest = record,
            Output::OverheadClear { unit } => {
                self.more.overhead.remove(&unit);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// 0x50 (§7 r2): the UI part of the codes the handler emits.
    fn quest_special(&mut self, code: u16, w: &[u16; 6]) -> Result<(), OriginalUiError> {
        match code {
            1 => {
                self.more.quest_words = [i32::from(w[0]), i32::from(w[1]), i32::from(w[2])];
            }
            2 => {
                // r4: the entry scan never matters; the hire popup only.
                self.more.hire_7beecc = 0;
                self.set_ui(UI_HIREICONS, ON, false)?;
                if self.more.popup_hireling == 0 {
                    self.more.hire_7beee4 = 1;
                }
            }
            // r5: `0x004B25C0` writes two fields nothing reads; d2rs
            // keeps neither (`client/bridge.md` §10 r9).
            3 => {}
            4 => {
                let mut b = [0u8; 12];
                for (i, v) in b.chunks_mut(2).enumerate() {
                    v.copy_from_slice(&w[i].to_le_bytes());
                }
                self.more.quest_special_bytes = b;
                self.more.quest_7bf254 = 0;
            }
            23 => {
                self.more.flag_70ee8c = 0;
                self.more.flag_7a0674 = 1;
            }
            36 => {
                self.more.zoo_latch = true;
                self.more.zoo_word = w[0];
            }
            _ => {}
        }
        Ok(())
    }

    /// 0x8A at delivery (§9 r3, r4).
    fn npc_interact(
        &mut self,
        unit: UnitKey,
        present: bool,
        class: u32,
        mdata_3c: Option<i32>,
        blocker_open: bool,
    ) {
        if !present {
            return;
        }
        if class == CLASS_ACT5POW {
            let id = if mdata_3c.is_some_and(|v| v != -1) {
                4603
            } else {
                4607
            };
            self.more.unit_sounds.push((id, unit));
            return;
        }
        // r4: `[0x007C0D29]` set and the GUID equals `[0x007C0D25]`
        // (the message's unit is type 1).
        let is_interact_npc = self.more.interact_active
            && unit.unit_type == MONSTER
            && unit.guid == self.more.interact_npc;
        if !is_interact_npc {
            self.more.overlays.push((unit, OVERLAY_NPC_WANTS));
        }
        let q = &self.more.client_quest;
        let quest_clear = !crate::bridge::objects::quest_bit(q, 12, 8)
            && !crate::bridge::objects::quest_bit(q, 12, 1);
        if class == CLASS_ACT2GUARD2 && !self.more.f4b1620 && quest_clear && !blocker_open {
            self.more.unit_sounds.push((3983, unit));
        }
    }

    /// 0x91 (§10 r2): every intro entry whose class equals a slot value
    /// below the monstats row count gets its flag byte := 1.
    fn npc_intro(&mut self, slots: &[u16; 12]) {
        let rows = self.more.monstats_rows;
        for &v in slots {
            if u32::from(v) >= rows {
                continue;
            }
            for e in &mut self.more.intro_table {
                if e.class == u32::from(v) {
                    e.flag = 1;
                }
            }
        }
    }

    /// 0x9B (§15 r2).
    fn merc_revive(&mut self, state: u16, value: u16) {
        self.more.merc_state = state;
        self.more.merc_7c0dd0 = value;
        if state == 0xFFFF {
            self.more.merc_menu_calls.extend([11, 8, 24, 21, 43]);
            self.skip(skip::MERC_MENU);
        }
    }

    /// The overhead text `0x0049F410(unit, text, lang)` (§4 r4): an empty
    /// text frees the unit's record; otherwise a new record replaces the
    /// old one.
    pub fn set_overhead(&mut self, unit: UnitKey, text: &[u8], lang: u8) {
        if text.is_empty() {
            self.more.overhead.remove(&unit);
            return;
        }
        let n = text.len().min(254) as u32;
        let duration = 8 * n + 125;
        let rec = OverheadText {
            duration,
            end: self.more.overhead_counter.wrapping_add(duration),
            lang,
            text: text[..n as usize].to_vec(),
        };
        self.more.overhead.insert(unit, rec);
    }

    /// One overhead draw (`0x004A0E70`): the counter steps once, and a
    /// record whose end has passed is freed (`0x004A0A00`).
    pub fn overhead_draw(&mut self) {
        self.more.overhead_counter = self.more.overhead_counter.wrapping_add(1);
        let now = self.more.overhead_counter;
        self.more.overhead.retain(|_, r| r.end >= now);
    }

    /// 0x26 at delivery (`0x0049F490`, §4 r3). `roster_squelched`: the
    /// roster test `0x0047A170(GUID)` is 0 (an entry with relation 4
    /// set, only from out-of-scope messages): a player line shows
    /// nothing.
    pub fn chat_line(&mut self, o: &Output, roster_squelched: bool) -> Option<ChatAction> {
        let Output::ChatLine {
            kind,
            lang,
            unit,
            b8,
            name,
            text,
            present,
            ..
        } = o
        else {
            return None;
        };
        // r3.1.
        if unit.unit_type == 0 && roster_squelched {
            return None;
        }
        // r3.2: the filter object is not specified.
        // PROVISIONAL (client/msg-ui.md §4 r3.2): nothing is filtered;
        // settled by a capture of a filtered line.
        self.skip(skip::CHAT_FILTER);
        // r3.3.
        if !text_converts(text, *lang, self.more.own_lang) {
            return None;
        }
        // r3.4.
        match *kind {
            4 => Some(ChatAction::ScreenMessage(text.clone())),
            6 => Some(ChatAction::Formatted(text.clone())),
            1 if matches!(unit.unit_type, 0 | 1) => Some(ChatAction::ScreenMessage(text.clone())),
            1 | 2 => Some(ChatAction::Named {
                name: name.clone(),
                text: text.clone(),
            }),
            5 if *present => {
                self.set_overhead(*unit, text, *lang);
                Some(ChatAction::Overhead(*unit, text.clone()))
            }
            7 => Some(ChatAction::Type7(*b8)),
            _ => None,
        }
    }
}
