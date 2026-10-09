// Spec: specs/world/quests-status.md (§1 client state, §3 tab build)
//! The client's quest-log state (§1) and the tab build (§3): the received
//! status list `S`, the last shown status `last`, the Den of Evil / staff
//! tomb / barbarian counters, the remembered tab slots, and the row list of
//! a shown act tab with the selected slot.

use super::tables::{chain_entry, entry, ENTRY_COUNT};
use super::{derive_row, IconState, QuestFlags, Row, RowCtx};

/// Highest icon index with a cel (§3 rule 2: "its icon ≤ 26").
pub const MAX_ICON: u8 = 26;
/// Rows built per tab (§3 rule 2).
pub const MAX_ROWS: usize = 6;
/// Act tabs.
pub const TAB_COUNT: usize = 5;
/// The Act V tab (§3 rule 3).
pub const ACT_V_TAB: u8 = 4;

/// What the 0x52 handler asks the panel layer to do afterwards (§1 rule 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogEffect {
    /// `0x004A23D0`: load the panel cels.
    LoadPanelCels,
    /// `0x004A3220(tab, reset)`.
    OpenTab { tab: u8, reset: bool },
}

/// A built row with its entry's slot and icon (§3 rule 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabRow {
    pub slot: u8,
    pub icon: u8,
    pub row: Row,
}

/// Gates of the tab build the caller knows (installs, loaded cels).
#[derive(Clone, Copy)]
pub struct TabGate<'a> {
    /// `0x00408F20`: expansion installed.
    pub expansion_installed: bool,
    /// `0x0044DCC0`: expansion game.
    pub expansion_game: bool,
    /// Whether icon cel k is loaded.
    pub cel_loaded: &'a dyn Fn(u8) -> bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestLog {
    /// `S[0..40]` at `[0x007BF356]`.
    pub status: [u8; ENTRY_COUNT],
    /// `last[0..40]` at `[0x007BF380]`.
    pub last: [u8; ENTRY_COUNT],
    /// D, Y, B: `[0x007BF2A4]`, `[0x007BF2A8]`, `[0x007BF2AC]`.
    pub den: i32,
    pub tomb: i32,
    pub barbarians: i32,
    /// Quest-log latch `[0x007BF298]`.
    pub latch: u32,
    /// Remembered slot per tab (`[0x007BF280 + 4·tab]`, `None` = −1).
    pub remembered: [Option<u8>; TAB_COUNT],
    /// The tab's last clicked selection (`[0x007BF2BD + 4·tab]`).
    pub clicked: [Option<u8>; TAB_COUNT],
    /// Selected slot `[0x007BF2B9]` (`None` = −1, set at every tab open).
    pub selected: Option<u8>,
    /// `[0x007BF2B0]`, zeroed by every 0x52.
    pub flag_2b0: u8,
}

impl Default for QuestLog {
    fn default() -> Self {
        QuestLog {
            status: [0; ENTRY_COUNT],
            last: [0; ENTRY_COUNT],
            den: 0,
            tomb: 0,
            barbarians: 0,
            latch: 0,
            remembered: [None; TAB_COUNT],
            clicked: [None; TAB_COUNT],
            selected: Some(0),
            flag_2b0: 0,
        }
    }
}

impl QuestLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// S→C 0x52 (`0x004A40D0`): the 42 message bytes; byte 1 + i is
    /// `S[i]`. `panel_ready` is `0x00483350` = 0; `shown_tab` is
    /// `[0x007C0255]`. Effects in call order.
    pub fn receive_0x52(
        &mut self,
        bytes: &[u8; 42],
        panel_ready: bool,
        shown_tab: u8,
    ) -> Vec<LogEffect> {
        self.status.copy_from_slice(&bytes[1..42]);
        self.flag_2b0 = 0;
        let mut out = Vec::new();
        if panel_ready && self.latch == 2 {
            out.push(LogEffect::LoadPanelCels);
            out.push(LogEffect::OpenTab {
                tab: shown_tab,
                reset: true,
            });
        }
        out
    }

    /// S→C 0x50 type 1 (`0x004A28A0`): D := u16@3, Y := u16@5, B := u16@7.
    /// Other types are not quest-log state.
    pub fn receive_0x50(&mut self, kind: u16, d: u16, y: u16, b: u16) {
        if kind == 1 {
            self.den = i32::from(d);
            self.tomb = i32::from(y);
            self.barbarians = i32::from(b);
        }
    }

    /// S→C 0x5D status write: `S[e]` of the entry whose chain is `chain`,
    /// only while the quest screen (UI state 15) is open. A chain with no
    /// entry writes nothing. Returns the entry written.
    pub fn receive_0x5d(&mut self, chain: u8, status: u8, quest_screen_open: bool) -> Option<u8> {
        if !quest_screen_open {
            return None;
        }
        let e = chain_entry(chain)?;
        self.status[e as usize] = status;
        Some(e)
    }

    /// `0x004A3020`: zero `S` and `last`.
    pub fn reset_status(&mut self) {
        self.status = [0; ENTRY_COUNT];
        self.last = [0; ENTRY_COUNT];
    }

    /// `0x004A3410` (game start): also the counters, the latch and the
    /// selection.
    pub fn reset_game(&mut self) {
        self.reset_status();
        self.den = 0;
        self.tomb = 0;
        self.barbarians = 0;
        self.latch = 0;
        self.remembered = [None; TAB_COUNT];
        self.clicked = [None; TAB_COUNT];
        self.selected = Some(0);
    }

    fn ctx<'a>(&self, p: &'a QuestFlags, g: Option<&'a QuestFlags>, mp: bool) -> RowCtx<'a> {
        RowCtx {
            p,
            g,
            multiplayer: mp,
            den: self.den,
            barbarians: self.barbarians,
        }
    }

    fn tab_open(tab: u8, gate: &TabGate<'_>) -> bool {
        tab != ACT_V_TAB || (gate.expansion_installed && gate.expansion_game)
    }

    /// Tab draw (§3 rule 2): entries in table order; shown when the tab
    /// matches, enabled, icon ≤ 26, its cel loaded and fewer than 6 rows
    /// were built. Each row is derived by §4 (rewriting `last`).
    pub fn build_tab(
        &mut self,
        tab: u8,
        p: &QuestFlags,
        g: Option<&QuestFlags>,
        multiplayer: bool,
        gate: &TabGate<'_>,
    ) -> Vec<TabRow> {
        let mut rows = Vec::new();
        if !Self::tab_open(tab, gate) {
            return rows;
        }
        let ctx = self.ctx(p, g, multiplayer);
        for i in 0..ENTRY_COUNT as u8 {
            let e = entry(i);
            if e.tab != tab
                || !e.enabled
                || e.icon > MAX_ICON
                || !(gate.cel_loaded)(e.icon)
                || rows.len() >= MAX_ROWS
            {
                continue;
            }
            let row = derive_row(i, self.status[i as usize], &ctx, &mut self.last[i as usize]);
            rows.push(TabRow {
                slot: e.slot,
                icon: e.icon,
                row,
            });
        }
        rows
    }

    /// Tab open `0x004A3220(tab, reset)` (§3 rule 3): returns the selected
    /// slot (also stored in `selected`), `None` when the tab is not built
    /// (Act V without the expansion) or nothing selects.
    pub fn open_tab(
        &mut self,
        tab: u8,
        reset: bool,
        p: &QuestFlags,
        g: Option<&QuestFlags>,
        multiplayer: bool,
        gate: &TabGate<'_>,
    ) -> Option<u8> {
        // §3 r3 order (0x004A3220): selected := none first, always; a
        // refused tab ends there; the selection is chosen (a remembered
        // slot without a scan); only then does `reset` clear the slots.
        self.selected = None;
        if !Self::tab_open(tab, gate) {
            return None;
        }
        let pick = self.choose(tab, p, g, multiplayer, gate);
        self.selected = pick;
        if reset {
            self.remembered = [None; TAB_COUNT];
        }
        pick
    }

    fn choose(
        &mut self,
        tab: u8,
        p: &QuestFlags,
        g: Option<&QuestFlags>,
        multiplayer: bool,
        gate: &TabGate<'_>,
    ) -> Option<u8> {
        if let Some(slot) = self.remembered[tab as usize] {
            return Some(slot);
        }
        let ctx = self.ctx(p, g, multiplayer);
        // The scan visits all 41 entries (no 6-row limit).
        let mut any_state_1_to_3 = false;
        let mut first_state3 = None;
        for i in 0..ENTRY_COUNT as u8 {
            let e = entry(i);
            if e.tab != tab || !e.enabled || e.icon > MAX_ICON || !(gate.cel_loaded)(e.icon) {
                continue;
            }
            let row = derive_row(i, self.status[i as usize], &ctx, &mut self.last[i as usize]);
            if row.changed || row.icon == IconState::JustCompleted {
                return Some(e.slot);
            }
            if matches!(
                row.icon,
                IconState::Completed | IconState::NotAvailable | IconState::InProgress
            ) {
                any_state_1_to_3 = true;
            }
            if row.icon == IconState::InProgress && first_state3.is_none() {
                first_state3 = Some(e.slot);
            }
        }
        if !any_state_1_to_3 {
            return None;
        }
        // §3 r3: the clicked slot when one was recorded, otherwise the first
        // state-3 entry (the reading is confirmed).
        self.clicked[tab as usize].or(first_state3)
    }
}

/// §3 rule 4: the selected row's title is drawn unless 3724.
pub fn title_drawn(title: u16) -> bool {
    title != super::tables::PLACEHOLDER_STRING
}

/// §3 rule 4: the text is drawn (wrapped at 270 px) unless a speech replay
/// runs.
pub const TEXT_WRAP_PX: i32 = 0x10E;

pub fn text_drawn(replay_running: bool) -> bool {
    !replay_running
}

/// §3 rule 4: the replay button plays the row's speech unless it is 3724 or
/// 3725.
pub fn replay_plays(speech: u16) -> bool {
    speech != super::tables::PLACEHOLDER_STRING && speech != super::tables::NULL_STRING
}

/// Entry range of a tab's icon loads (§2, §3 rule 3).
pub fn tab_icon_range(tab: u8) -> Option<(u8, u8)> {
    super::tables::TAB_ICON_RANGE.get(tab as usize).copied()
}
