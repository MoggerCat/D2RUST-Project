// Spec: specs/client/msg-ui.md (§1 rules 2, 5–7, §2 rule 2, §3 rules 2–3, §5 rule 2, §16 rules 4–5, open question 10), specs/client/bridge.md (§10 rules 5–6)
//! The UI consumer of the bridge outputs (`client/bridge.md` §10):
//! [`OriginalUi::apply_output`] runs the 1.14d UI dispatch of S→C 0x5D
//! (`0x004A2CB0`), 0x63 (`0x0049CF90`) and 0x77 (`0x004B8CF0`) at
//! delivery, on the UI's own state ([`MsgUiState`]) and flags; it keeps
//! 0x27's NPC text list (`0x004A1600`) and chooses the case of 0x28's
//! dialog branch (`0x004B6DD0`), which it hands back to the bridge
//! ([`OriginalUi::take_dialog_answer`] → `Bridge::npc_dialog_branch`:
//! the model writes and C→S 0x31 are the bridge's, open question 10
//! decided as A). Sounds go
//! through the UI's request path ([`UiOutcome::sounds`]); a part whose
//! input or callee no spec gives yet is not guessed: it is skipped and
//! named in [`UiOutcome::skipped`]. Nothing here writes the model (§10
//! rule 6).

use d2_sim::world::waypoints::WaypointRecord;

use super::{OriginalUi, OriginalUiError};
use crate::audio::driver::SoundRequest;
use crate::bridge::msg::ui::{quest_row, QuestRow};
use crate::bridge::msg::ui_npc::DialogCase;
use crate::bridge::output::{Consumer, NpcDialog, Output};
use crate::bridge::world::{ClientWorld, UnitKey, PLAYER};
use crate::rules::lighting::environment::act_index;

/// `SetUIState` modes (`ui/panels.md` §2).
const ON: u32 = 0;
const OFF: u32 = 1;

/// UI states the dispatch uses (`ui/ui-states.tsv`).
pub const UI_CHAT: u32 = 5;
pub const UI_QUESTSCREEN: u8 = 15;
pub const UI_QUESTLOG: u32 = 17;
pub const UI_WAYPOINT: u32 = 20;
pub const UI_MPTRADE: u32 = 23;
pub const UI_STASH: u32 = 25;
pub const UI_CUBE: u32 = 26;

/// Inventory modes (`ui/panels.md` §11, §12).
pub const MODE_STASH: u8 = 0x0C;
pub const MODE_STASH_2: u8 = 0x0D;
pub const MODE_CUBE: u8 = 0x0E;

/// The trade state `[0x007C0E7C]` value "refused" (§3 rule 2).
pub const TRADE_REFUSED: u8 = 7;

/// Named parts the UI skipped (their input or callee is not specified).
pub mod skip {
    pub const MONSTER_EFFECT: &str = "0x5D: 0x0046F870(211, 1) (msg-ui open question 1)";
    pub const ACT_END_VIDEO: &str =
        "0x5D: 0x0044EC80, the video-5 flag and the character record word +0x1EF (msg-ui §1 r2)";
    pub const VIDEO_7: &str = "0x5D: the video-7 flag [0x007A0628] (render/composition.md §4)";
    pub const DEN_COUNTER: &str =
        "0x5D: the client quest flags [0x007C0D43] of the Den counter path (msg-ui OQ 4)";
    pub const QUEST_LOG_TABLE: &str =
        "0x5D: the quest-log table 0x00723F30 (41 entries) is not in the specs (msg-ui §1 r6)";
    pub const WAYPOINT_TAB_GATE: &str =
        "0x63: the waypoint tab gate reads the client quest flags (msg-ui OQ 4)";
    pub const WAYPOINT_ROWS: &str = "0x63: the row rebuild 0x0049C7F0 (ui/panels.md §13 r5)";
    pub const TRADE: &str = "0x77: the trade helpers of codes 0x00–0x06 (msg-ui OQ 5)";
    pub const NO_LOCAL_PLAYER: &str = "0x77 code 9: no local player";
    pub const TRADE_CLOSE_HELPER: &str = "0x77: the trade close helper 0x00487B30 (msg-ui OQ 5)";
    pub const NOT_APPLIED: &str =
        "a UI output whose dispatch (msg-ui §4–§22, msg-units §8; ui/*) is not written yet";
    pub const NPC_TEXT_SHOW: &str =
        "0x27: the overhead text, list start 0x006616E0, box 0x004A1510 and panel 0x004A1320 (msg-ui §5 r2; ui/*)";
    pub const NPC_DIALOG_UI: &str =
        "0x28: overlay 72 off, 0x004B2250, 0x0044DA40 and the chosen case's UI calls (msg-ui §16 r4.1–r4.3; ui/*)";
    pub const NPC_DIALOG_M: &str =
        "0x28: m = 0x00661400(txt, 0) of this NPC text list is not specified (msg-ui §16 r4.3, OQ 10): no case handed back, no C→S 0x31";
}

/// The NPC text list `[0x007BF250]` as 0x27 rebuilt it (§5 r2.1): bytes
/// 6–39 of the message (count u8@0, entry k < 8: kind u8@2+4k, string
/// id u16@4+4k).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcTextList {
    pub bytes: [u8; 34],
}

impl NpcTextList {
    /// The list rebuilt from a 0x27 record (its 40 bytes).
    pub fn from_record(record: &[u8; 40]) -> Self {
        let mut bytes = [0; 34];
        bytes.copy_from_slice(&record[6..40]);
        Self { bytes }
    }

    pub fn count(&self) -> u8 {
        self.bytes[0]
    }

    /// Entry k's kind (k < 8).
    pub fn kind(&self, k: usize) -> u8 {
        self.bytes[2 + 4 * k]
    }

    /// Entry k's string id (k < 8).
    pub fn string(&self, k: usize) -> u16 {
        u16::from_le_bytes([self.bytes[4 + 4 * k], self.bytes[5 + 4 * k]])
    }

    /// The list as the build `0x00661510` and the start `0x006616E0` leave
    /// it (§16 r9): nodes (kind, string id) prepended in message order,
    /// so reversed; a list of two or more nodes is stable-insertion
    /// sorted by string id, ascending (unsigned u16). A count past 8 is
    /// not a list (the build asserts at count ≥ 8, [`Self::checked`]).
    pub fn nodes(&self) -> Vec<(u8, u16)> {
        let n = usize::from(self.count()).min(8);
        let mut v: Vec<(u8, u16)> = (0..n)
            .rev()
            .map(|k| (self.kind(k), self.string(k)))
            .collect();
        // Stable insertion sort (`0x006615D0`): `sort_by_key` is stable.
        v.sort_by_key(|&(_, id)| id);
        v
    }

    /// `0x00661510`'s assertion (§16 r9.1, `0x00661557`): a count of 8 or
    /// more is fatal.
    pub fn checked(self) -> Result<Self, OriginalUiError> {
        if self.count() >= 8 {
            Err(OriginalUiError::NpcTextCount(self.count()))
        } else {
            Ok(self)
        }
    }

    /// `0x00661400(list, 0)`, the m of 0x28's dialog branch: the first
    /// node of kind 0 in list order (so the smallest kind-0 string id),
    /// 0xFFFF when none (§16 r9.3).
    pub fn m(&self) -> u16 {
        self.nodes()
            .into_iter()
            .find(|&(k, _)| k == 0)
            .map_or(0xFFFF, |(_, id)| id)
    }

    /// `0x00661440(list, 0)`, m2: the same for kind 1.
    pub fn m2(&self) -> u16 {
        self.nodes()
            .into_iter()
            .find(|&(k, _)| k == 1)
            .map_or(0xFFFF, |(_, id)| id)
    }
}

/// 0x28's dialog branch (§16 r4.3), the case 1.14d takes (the first that
/// holds), from the UI state `[0x007C0C68]`, the NPC text list and the
/// captured inputs. B3–B6 are one case for the
/// bridge ([`DialogCase::Rest`]): they write the model alike.
///
/// Reading taken (as the bridge's): B0 ends the branch, the "(always
/// next)" rows run only when B0 does not hold.
pub fn dialog_case(
    ui_7c0c68: u8,
    txt: Option<&NpcTextList>,
    d: &NpcDialog,
) -> Result<Option<DialogCase>, OriginalUiError> {
    if ui_7c0c68 != 0 {
        return Ok(Some(DialogCase::B0));
    }
    // txt := `0x0049F900`: none → fatal 0x1060.
    let txt = txt.ok_or(OriginalUiError::NoNpcText)?;
    let m = txt.m();
    if d.cursor_item {
        return Ok(Some(DialogCase::B1));
    }
    Ok(Some(if m != 0xFFFF {
        DialogCase::B2 { m: u32::from(m) }
    } else {
        DialogCase::Rest
    }))
}

/// The waypoint menu's stored state (§2 rule 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaypointMenuState {
    /// `[0x007BF07D]`.
    pub guid: u32,
    /// The record buffer `[0x007BF081]` after the load copy.
    pub record: WaypointRecord,
    /// The tab `[0x007BF086]`; `None` when its gate needs the client
    /// quest flags (skipped).
    pub tab: Option<u8>,
    /// The close latch `[0x007BF085]`.
    pub close_latch: bool,
}

/// The UI globals the three dispatches write (§1–§3), by 1.14d address.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MsgUiState {
    /// The quest-log latch `[0x007BF298]` (§1 rule 6).
    pub quest_log_latch: bool,
    /// `[0x007BF2AC]` (§1 rule 2, f bit 5, c 32).
    pub quest_7bf2ac: Option<i16>,
    /// `[0x007BC9D8]` (f bit 1, c 23, expansion game).
    pub act_end_7bc9d8: bool,
    /// `[0x007BC9D4]` (f bit 1, c 23, classic game).
    pub act_end_7bc9d4: bool,
    /// The waypoint menu (§2), once a 0x63 opened it.
    pub waypoint: Option<WaypointMenuState>,
    /// The trade state `[0x007C0E7C]` (§3 rule 2).
    pub trade_state: u8,
    /// `[0x007BCE28]` (§3, codes 0x0E / 0x0F, close trade).
    pub trade_7bce28: bool,
    /// `[0x007C0E80]` (close trade).
    pub trade_7c0e80: u32,
    /// The inventory mode (`ui/panels.md` §11, §12).
    pub inventory_mode: u8,
    /// `[0x007C0C68]`, read by 0x28's dialog branch (§16 r4.3, case B0).
    /// PROVISIONAL (ui/messages.md §14 / client/msg-ui.md §16): no spec
    /// gives a writer of this UI global, so it stays 0 (the value the
    /// recorded 0x28 of A seq 37353 implies: it took B2); settled by a
    /// Ghidra xref scan of `[0x007C0C68]` writers.
    pub ui_7c0c68: u8,
}

impl OriginalUi {
    /// Applies one bridge output at delivery (`client/bridge.md` §10
    /// rule 4): the UI dispatch of its message, with the gate facts of
    /// `world` now. A `ServerSound` is the audio layer's (rule 5): no
    /// effect here.
    pub fn apply_output(&mut self, o: &Output, world: &ClientWorld) -> Result<(), OriginalUiError> {
        let r = self.apply_output_inner(o, world);
        self.sync_quest_inputs(o);
        r
    }

    fn apply_output_inner(
        &mut self,
        o: &Output,
        world: &ClientWorld,
    ) -> Result<(), OriginalUiError> {
        self.refresh_facts(world);
        self.imbue_output(o);
        if matches!(o, Output::ChatLine { .. }) {
            // `messages.md` §3: the screen message; the overhead record
            // of type 5 is `chat_line`'s.
            self.game_message(o, world.frames);
            return Ok(());
        }
        if self.apply_more(o)? {
            return Ok(());
        }
        match *o {
            Output::QuestUi {
                chain,
                flags,
                status,
                extra,
            } => self.quest_ui(chain, flags, status, extra, world),
            Output::WaypointMenu { guid, record } => self.waypoint_menu(guid, &record, world),
            Output::TradeAction {
                code,
                dead_or_absent,
            } => self.trade_action(code, dead_or_absent, world),
            Output::NpcText {
                ref bytes, present, ..
            } => self.npc_text_record(bytes, present),
            Output::NpcDialog(ref d) => self.npc_dialog(d, world),
            Output::NpcTransaction { ref bytes, .. } => {
                self.npc_transaction(bytes);
                Ok(())
            }
            // Not UI outputs (`client/bridge.md` §10 rule 5).
            Output::ServerSound { .. } | Output::ShrineSound { .. } => Ok(()),
            _ if o.consumer() != Consumer::Ui => Ok(()),
            // The UI dispatches of `client/msg-ui.md` §4–§22 and
            // `client/msg-units.md` §8 (PC 2's `ui/*`) are not written yet.
            _ => {
                self.skip(skip::NOT_APPLIED);
                Ok(())
            }
        }
    }

    /// The UI globals the outputs wrote.
    pub fn msg_state(&self) -> &MsgUiState {
        &self.msg
    }

    /// Sets `[0x007C0C68]` (§16 r4.3, case B0): the seam for the UI
    /// writer no spec gives yet ([`MsgUiState::ui_7c0c68`]).
    pub fn set_ui_7c0c68(&mut self, v: u8) {
        self.msg.ui_7c0c68 = v;
    }

    /// `ui/messages.md` §6 r2: the NPC text list is freed when the
    /// interaction ends (`0x004A1730`, from `0x004B3C20`) and on game exit
    /// (`0x004A0680`).
    pub fn free_npc_text(&mut self) {
        self.npc_text = None;
        self.talk_list();
    }

    /// The NPC text list `[0x007BF250]` (§5 r2), `None` when freed.
    pub fn npc_text(&self) -> Option<&NpcTextList> {
        self.npc_text.as_ref()
    }

    /// The case of 0x28's dialog branch chosen since the last call, with
    /// its output, for `Bridge::npc_dialog_branch` (§16 r4.3, open
    /// question 10 decided as A). Taken by the output dispatcher right
    /// after the `NpcDialog` it answers, before the next output.
    pub fn take_dialog_answer(&mut self) -> Option<(Box<NpcDialog>, DialogCase)> {
        self.dialog_answer.take()
    }

    /// §5 r2 at delivery: the list part of `0x004A1600`; what is shown is
    /// skipped (`ui/*`).
    fn npc_text_record(&mut self, bytes: &[u8; 40], present: bool) -> Result<(), OriginalUiError> {
        let list = NpcTextList::from_record(bytes);
        match bytes[1] {
            // r2.1: an overhead number, the list stays.
            1 if present && list.count() == 1 && list.kind(0) == 3 => {
                let guid = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
                let text = list.string(0).to_string();
                self.set_overhead(UnitKey::new(1, guid), text.as_bytes(), self.more.own_lang);
            }
            1 => self.npc_text = Some(list.checked()?),
            // r2.2: the box or the panel; the list stays.
            2 => {}
            // r2.3.
            _ => self.npc_text = None,
        }
        self.talk_list();
        self.skip(skip::NPC_TEXT_SHOW);
        Ok(())
    }

    /// §16 r4 at delivery: the UI-only calls are skipped; the branch case
    /// is chosen and kept for the bridge.
    fn npc_dialog(&mut self, d: &NpcDialog, world: &ClientWorld) -> Result<(), OriginalUiError> {
        // r4.2: `[0x007C0D43]` := Q (§16 r7).
        self.more.client_quest = d.quest_flags;
        self.skip(skip::NPC_DIALOG_UI);
        // The menu box (`npc_box`, `menus.md` §2.2) opens here.
        let level = world.local().map_or(1, |u| world.base(u.key, 12, 0));
        let n = super::npc_box::unidentified_count(world);
        // `panels-2.md` §14.2: the Resurrect edit while the mercenary is
        // dead (`[0x00725494]` ≠ −1, S→C 0x9B) in an expansion game.
        let expansion = world.expansion != 0 && self.shared.borrow().config.expansion_installed;
        self.npcm.borrow_mut().resurrect = (expansion && self.more.merc_state != 0xFFFF)
            .then_some(u32::from(self.more.merc_7c0dd0));
        self.npcm.borrow_mut().merc_name = self.more.merc_state;
        self.hire.borrow_mut().merc_state = self.more.merc_state;
        self.open_npc_menu_with(d.guid, d.class, level, n, world);
        match dialog_case(self.msg.ui_7c0c68, self.npc_text.as_ref(), d)? {
            Some(case) => {
                self.npc_speech(d, case);
                self.dialog_answer = Some((Box::new(d.clone()), case));
            }
            None => self.skip(skip::NPC_DIALOG_M),
        }
        Ok(())
    }

    /// The speech of 0x28's dialog branch (`client/msg-ui.md` §16 r4.3, in
    /// the order of the table): B2 plays the dialog line of `m`
    /// (`0x004A10E0(U, m, 1)`), B3 and B6 the NPC's greeting
    /// (`0x004B4FD0`, `0x004B66B0`; `triggers.md` §10 r1); B0, B1, B4 and B5
    /// make none. B3 holds when m2 (`0x00661440`) is not 0xFFFF.
    fn npc_speech(&mut self, d: &NpcDialog, case: DialogCase) {
        let request = match case {
            DialogCase::B2 { m } => SoundRequest::NpcDialogLine {
                npc: d.unit,
                class: d.class,
                key: m as i32,
            },
            DialogCase::Rest => {
                let m2 = self.npc_text.as_ref().map_or(0xFFFF, NpcTextList::m2);
                let b3 = m2 != 0xFFFF;
                let b5 = d.f4b1a10.is_some_and(|v| v != 0);
                if !b3 && !(d.interact && !b5) {
                    return;
                }
                SoundRequest::NpcGreeting {
                    npc: d.unit,
                    class: d.class,
                }
            }
            DialogCase::B0 | DialogCase::B1 => return,
        };
        self.outcome.sounds.push(request);
    }

    fn skip(&mut self, what: &'static str) {
        self.outcome.skipped.push(what);
    }

    fn sound(&mut self, id: i32) {
        self.outcome.sounds.push(SoundRequest::Ui(id));
    }

    /// §1 rule 2's output rows, at delivery (rule 5).
    fn quest_ui(
        &mut self,
        c: u8,
        f: u8,
        // The status byte is read only by the quest-log table parts
        // (rules 6.2, 7), which are skipped.
        _s: u8,
        v: i16,
        world: &ClientWorld,
    ) -> Result<(), OriginalUiError> {
        match quest_row(c, f) {
            QuestRow::ScreenMessage(id) => self.game_message_id(id),
            QuestRow::MonsterEffect211 => self.skip(skip::MONSTER_EFFECT),
            QuestRow::Sounds(ids) => {
                for &id in ids {
                    self.sound(id);
                }
            }
            QuestRow::SoundExtra => self.sound(i32::from(v)),
            QuestRow::ActEnd => {
                // `0x0044DCC0`: `[0x007A04F4]` ≠ 0.
                if world.expansion != 0 {
                    self.msg.act_end_7bc9d8 = true;
                } else {
                    self.skip(skip::ACT_END_VIDEO);
                    self.msg.act_end_7bc9d4 = true;
                }
            }
            QuestRow::Video7 => self.skip(skip::VIDEO_7),
            QuestRow::SetThenTail => {
                self.msg.quest_7bf2ac = Some(v);
                self.quest_log_tail()?;
            }
            QuestRow::DenCounter => self.skip(skip::DEN_COUNTER),
            QuestRow::Tail => self.quest_log_tail()?,
            // Model rows ran at receive (§1 rule 3); "nothing" rows.
            QuestRow::Untargetable(_)
            | QuestRow::Eclipse
            | QuestRow::ExitRequested
            | QuestRow::Nothing => {}
        }
        Ok(())
    }

    /// The quest-log tail T (§1 rule 6).
    fn quest_log_tail(&mut self) -> Result<(), OriginalUiError> {
        if !self.msg.quest_log_latch {
            if self.set_ui(UI_QUESTLOG, ON, false)? {
                // r6.1: the entry of c sets its act's selected slot.
                self.skip(skip::QUEST_LOG_TABLE);
                self.msg.quest_log_latch = true;
            }
        } else if self.is_open(UI_QUESTSCREEN) {
            // r6.2: the entry of c's status byte and slot.
            self.skip(skip::QUEST_LOG_TABLE);
        }
        Ok(())
    }

    /// The waypoint menu `0x0049CF90` (§2 rule 2).
    fn waypoint_menu(
        &mut self,
        guid: u32,
        record: &[u8; 16],
        world: &ClientWorld,
    ) -> Result<(), OriginalUiError> {
        // r2.1: jump 1; refused → nothing stored.
        if !self.set_ui(UI_WAYPOINT, ON, true)? {
            return Ok(());
        }
        // r2.2: the input reset `0x0044DA40`, applied by the host to the
        // world clicks ([`OriginalUi::take_input_reset`]).
        self.shared.borrow_mut().input_reset = true;
        // r2.3: the act index of the local player's room's level
        // (`0x006427F0`); none → tab 0. `0x0049C760(a)`: a ≥ 5 → 0;
        // tab 0 has no gate; the others read the client quest flags.
        let quest = self.more.client_quest;
        let gate = |record: u32| crate::bridge::objects::quest_bit(&quest, record as u8, 0);
        let tab = match world.player_level() {
            None => 0,
            Some(level) => match act_index(u32::from(level)) {
                a if a >= WAYPOINT_TABS => 0,
                a => crate::ui::panels::waypoint::set_tab(a as u8, &gate),
            },
        };
        // r2.4–r2.6: the row rebuild before and after the store.
        self.skip(skip::WAYPOINT_ROWS);
        let record = WaypointRecord::load_copy(record).map_err(OriginalUiError::Waypoint)?;
        self.msg.waypoint = Some(WaypointMenuState {
            guid,
            record,
            tab: Some(tab),
            close_latch: false,
        });
        // The installed menu's rows (`waypoint_ui`).
        let mut sh = self.shared.borrow_mut();
        let seq = sh.waypoint_open.map_or(1, |o| o.seq.wrapping_add(1));
        sh.waypoint_open = Some(super::WaypointOpen {
            guid,
            record,
            current: world.player_level().map_or(0, u32::from),
            seq,
            tab,
        });
        Ok(())
    }

    /// The UI action `0x004B8CF0` (§3 rule 2).
    fn trade_action(
        &mut self,
        code: u8,
        dead_or_absent: bool,
        world: &ClientWorld,
    ) -> Result<(), OriginalUiError> {
        match code {
            0x00 | 0x01 | 0x02 | 0x05 | 0x06 => self.skip(skip::TRADE),
            0x09 => match world.local_player {
                // Player event sound 23 on the local player (`0x004CB9C0`).
                Some(p) => self
                    .outcome
                    .sounds
                    .push(SoundRequest::PlayerEvent { unit: p, event: 23 }),
                None => self.skip(skip::NO_LOCAL_PLAYER),
            },
            // The partner `[0x007C0E60]` (0x78, §11 r2): player (0, GUID)
            // in S → player event sound 23 on it.
            0x0A => {
                let p = UnitKey::new(PLAYER, self.more.partner_guid);
                if world.units.contains_key(&p) {
                    self.outcome
                        .sounds
                        .push(SoundRequest::PlayerEvent { unit: p, event: 23 });
                }
            }
            0x0C | 0x0D => {
                self.msg.trade_state = 0;
                self.close_trade(code == 0x0D, dead_or_absent)?;
            }
            0x0E => self.msg.trade_7bce28 = true,
            0x0F => self.msg.trade_7bce28 = false,
            // `0x00489E00` (`ui/panels.md` §11 r1); the button states it
            // clears belong to the stash panel, not wired.
            0x10 => {
                self.set_ui(UI_STASH, ON, false)?;
                self.msg.inventory_mode = MODE_STASH;
            }
            // `0x00489F50`; the item lists it frees are not held.
            0x11 => {
                if matches!(self.msg.inventory_mode, MODE_STASH | MODE_STASH_2) {
                    self.msg.inventory_mode = 0;
                    self.set_ui(UI_STASH, OFF, false)?;
                }
            }
            // `0x0048A460` (`ui/panels.md` §12 r1).
            0x15 => {
                self.set_ui(UI_CUBE, ON, false)?;
                self.msg.inventory_mode = MODE_CUBE;
                // The open clears the close latch and the animation flag.
                let mut sh = self.shared.borrow_mut();
                sh.cube_opened = true;
                sh.cube_anim.set(Default::default());
            }
            // 0x03, 0x04, 0x07, 0x08, 0x0B, 0x12–0x14 and past 0x15.
            _ => {}
        }
        Ok(())
    }

    /// Close trade `0x004B8940(x)` (§3 rule 3). The lists it frees are
    /// not held.
    fn close_trade(&mut self, x: bool, dead_or_absent: bool) -> Result<(), OriginalUiError> {
        self.msg.trade_7c0e80 = 0;
        if self.msg.trade_state != TRADE_REFUSED {
            if self.is_open(UI_MPTRADE as u8) {
                self.set_ui(UI_MPTRADE, OFF, false)?;
                self.msg.trade_7bce28 = false;
                self.skip(skip::TRADE_CLOSE_HELPER);
            }
            if x && !dead_or_absent {
                // `0x00463DF0() = 0` (captured, r4) → `SetUIState(1
                // inventory, toggle, 0)`.
                self.set_ui(1, 2, false)?;
            }
            if self.is_open(UI_CHAT as u8) {
                self.set_ui(UI_CHAT, OFF, false)?;
            }
        }
        // The state is 0 here for both callers: no decline (rule 3).
        Ok(())
    }
}

/// The waypoint tab count `[0x007224E4]` (§2 r2.3).
pub const WAYPOINT_TABS: u32 = 5;

#[path = "msg_ui_more.rs"]
mod more;
pub use more::{ChatAction, IntroEntry, MsgUiMore, OverheadText};

#[cfg(test)]
#[path = "msg_ui_tests.rs"]
mod tests;
