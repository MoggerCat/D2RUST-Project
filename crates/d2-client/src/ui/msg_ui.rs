// Spec: specs/client/msg-ui.md (§1 rules 2, 5–7, §2 rule 2, §3 rules 2–3), specs/client/bridge.md (§10 rules 5–6)
//! The UI consumer of the bridge outputs (`client/bridge.md` §10):
//! [`OriginalUi::apply_output`] runs the 1.14d UI dispatch of S→C 0x5D
//! (`0x004A2CB0`), 0x63 (`0x0049CF90`) and 0x77 (`0x004B8CF0`) at
//! delivery, on the UI's own state ([`MsgUiState`]) and flags. Sounds go
//! through the UI's request path ([`UiOutcome::sounds`]); a part whose
//! input or callee no spec gives yet is not guessed: it is skipped and
//! named in [`UiOutcome::skipped`]. Nothing here writes the model (§10
//! rule 6).

use d2_sim::world::waypoints::WaypointRecord;

use super::{OriginalUi, OriginalUiError};
use crate::audio::driver::SoundRequest;
use crate::bridge::msg::ui::{quest_row, QuestRow};
use crate::bridge::output::Output;
use crate::bridge::world::ClientWorld;
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
    pub const SCREEN_MESSAGE: &str = "0x5D: screen message 0x0049E3A0 (msg-ui §1 r2)";
    pub const MONSTER_EFFECT: &str = "0x5D: 0x0046F870(211, 1) (msg-ui open question 1)";
    pub const ACT_END_VIDEO: &str =
        "0x5D: 0x0044EC80, the video-5 flag and the character record word +0x1EF (msg-ui §1 r2)";
    pub const VIDEO_7: &str = "0x5D: the video-7 flag [0x007A0628] (render/composition.md §4)";
    pub const DEN_COUNTER: &str =
        "0x5D: the client quest flags [0x007C0D43] of the Den counter path (msg-ui OQ 4)";
    pub const QUEST_LOG_TABLE: &str =
        "0x5D: the quest-log table 0x00723F30 (41 entries) is not in the specs (msg-ui §1 r6)";
    pub const INPUT_RESET: &str = "0x63: the input reset 0x0044DA40";
    pub const WAYPOINT_TAB_GATE: &str =
        "0x63: the waypoint tab gate reads the client quest flags (msg-ui OQ 4)";
    pub const WAYPOINT_ROWS: &str = "0x63: the row rebuild 0x0049C7F0 (ui/panels.md §13 r5)";
    pub const TRADE: &str = "0x77: the trade helpers of codes 0x00–0x06 (msg-ui OQ 5)";
    pub const TRADE_PARTNER: &str =
        "0x77 code 0x0A: the trade partner [0x007C0E60] has no writer in the specs";
    pub const NO_LOCAL_PLAYER: &str = "0x77 code 9: no local player";
    pub const TRADE_CLOSE_HELPER: &str = "0x77: the trade close helper 0x00487B30 (msg-ui OQ 5)";
    pub const CUBE_CHECK: &str = "0x77: 0x00463DF0 before the inventory toggle (unspecified)";
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
}

impl OriginalUi {
    /// Applies one bridge output at delivery (`client/bridge.md` §10
    /// rule 4): the UI dispatch of its message, with the gate facts of
    /// `world` now. A `ServerSound` is the audio layer's (rule 5): no
    /// effect here.
    pub fn apply_output(&mut self, o: &Output, world: &ClientWorld) -> Result<(), OriginalUiError> {
        self.refresh_facts(world);
        match *o {
            Output::QuestUi {
                chain,
                flags,
                status,
                extra,
            } => self.quest_ui(chain, flags, status, extra, world),
            Output::WaypointMenu { guid, record } => self.waypoint_menu(guid, &record, world),
            Output::TradeAction { code } => self.trade_action(code, world),
            Output::ServerSound { .. } => Ok(()),
        }
    }

    /// The UI globals the outputs wrote.
    pub fn msg_state(&self) -> &MsgUiState {
        &self.msg
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
            QuestRow::ScreenMessage(_) => self.skip(skip::SCREEN_MESSAGE),
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
        self.skip(skip::INPUT_RESET);
        // r2.3: the act index of the local player's room's level
        // (`0x006427F0`); none → tab 0. `0x0049C760(a)`: a ≥ 5 → 0;
        // tab 0 has no gate; the others read the client quest flags.
        let tab = match world.player_level() {
            None => Some(0),
            Some(level) => match act_index(u32::from(level)) {
                a if a >= WAYPOINT_TABS => Some(0),
                0 => Some(0),
                _ => {
                    self.skip(skip::WAYPOINT_TAB_GATE);
                    None
                }
            },
        };
        // r2.4–r2.6: the row rebuild before and after the store.
        self.skip(skip::WAYPOINT_ROWS);
        let record = WaypointRecord::load_copy(record).map_err(OriginalUiError::Waypoint)?;
        self.msg.waypoint = Some(WaypointMenuState {
            guid,
            record,
            tab,
            close_latch: false,
        });
        Ok(())
    }

    /// The UI action `0x004B8CF0` (§3 rule 2).
    fn trade_action(&mut self, code: u8, world: &ClientWorld) -> Result<(), OriginalUiError> {
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
            0x0A => self.skip(skip::TRADE_PARTNER),
            0x0C | 0x0D => {
                self.msg.trade_state = 0;
                self.close_trade(code == 0x0D)?;
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
            }
            // 0x03, 0x04, 0x07, 0x08, 0x0B, 0x12–0x14 and past 0x15.
            _ => {}
        }
        Ok(())
    }

    /// Close trade `0x004B8940(x)` (§3 rule 3). The lists it frees are
    /// not held.
    fn close_trade(&mut self, x: bool) -> Result<(), OriginalUiError> {
        self.msg.trade_7c0e80 = 0;
        if self.msg.trade_state != TRADE_REFUSED {
            if self.is_open(UI_MPTRADE as u8) {
                self.set_ui(UI_MPTRADE, OFF, false)?;
                self.msg.trade_7bce28 = false;
                self.skip(skip::TRADE_CLOSE_HELPER);
            }
            if x {
                // `0x00463DF0() = 0` → `SetUIState(1 inventory, toggle, 0)`.
                self.skip(skip::CUBE_CHECK);
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

#[cfg(test)]
#[path = "msg_ui_tests.rs"]
mod tests;
