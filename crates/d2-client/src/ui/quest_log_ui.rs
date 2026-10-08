// Spec: specs/world/quests-status.md (§1 inputs, §3 tab build, §4 row derivation, §5 icon states), specs/ui/panels.md (§2 step 4, ui 0x0F), specs/world/quests.md (§6.2); preview fills: docs/handoff/q-quests.md
//! The quest log (ui 0x0F, left slot) installed in the original UI: the
//! rows [`QuestLog::build_tab`] derives from the status list S→C 0x52
//! sent, the player's and the game's flag records (S→C 0x28 / 0x29) and
//! the Den of Evil count (S→C 0x50 type 1), drawn as the slot frames,
//! the icons and, for the selected row, the title and the text.
//!
//! Opening the log (`Q`, `0x00468990` → `0x004A3FE0(1)`) asks the server
//! for the quest data with C→S 0x40 (`quests.md` §6.2); the answer
//! (0x28 type 6, 0x50, 0x52) fills the state.
//!
//! d2rs-own, unverified (the open function `0x004A3FE0`, the slot
//! positions `0x00723EA8` / `0x00723EAC` and the cel layout are not in a
//! spec; REC-53 in `docs/HANDOFF.md` §7): C→S 0x40 on every open; the
//! act tab is the loaded act; slots on a 3 × 2 grid; the text is one line
//! (no wrap at 270 px, no measure); the just-completed animation and its
//! C→S 0x58 acknowledge (§5 rule 1) are not run (the row draws as
//! completed); the speech replay button is not drawn.

use std::cell::RefCell;

use super::{OriginalUi, SharedRef};
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{ClientIntent, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::{cel, text, UiFiles};
use crate::ui::quest_log::state::{QuestLog, TabGate, TabRow};
use crate::ui::quest_log::tables::ICON_NAMES;
use crate::ui::quest_log::{IconState, QuestFlags, RowText};

/// The state UI id of the quest screen (`ui-states.tsv`).
pub const UI_QUEST_SCREEN: u8 = crate::ui::states::id::QUEST_SCREEN;

/// C→S 0x40 RequestQuestData (`quests.md` §6.2; the handler wants size 1).
pub fn request_quest_data() -> ClientIntent {
    ClientIntent(vec![0x40])
}

/// Panel cels under `data\global\ui\` (`quests-status.md` §1 rule 1).
const BACKGROUND: &str = "menu\\questbackground";
const SOCKETS: &str = "menu\\questsockets";
const DONE: &str = "menu\\questdone";

/// The files the log draws, for [`UiFiles::extend`].
pub fn quest_files() -> Vec<String> {
    let mut v = vec![
        BACKGROUND.to_string(),
        SOCKETS.to_string(),
        DONE.to_string(),
    ];
    v.extend(ICON_NAMES.iter().map(|n| format!("menu\\{n}")));
    v
}

/// What the log reads from the messages (copied by the controller after
/// each quest output).
#[derive(Clone, Debug)]
pub struct QuestInputs {
    /// S→C 0x52: `S[0..40]`.
    pub status: [u8; 41],
    /// P: S→C 0x28 (`[0x007C0D43]`).
    pub player: QuestFlags,
    /// G: S→C 0x29 (`[0x007C0D47]`).
    pub game: Option<QuestFlags>,
    /// D, Y, B of S→C 0x50 type 1.
    pub counters: [i32; 3],
}

impl Default for QuestInputs {
    fn default() -> Self {
        QuestInputs {
            status: [0; 41],
            player: QuestFlags::new(),
            game: None,
            counters: [0; 3],
        }
    }
}

/// A record of 96 bytes as 48 little-endian slots.
pub fn flags_of(record: &[u8; 96]) -> QuestFlags {
    let mut f = QuestFlags::new();
    for q in 0..48u8 {
        let i = usize::from(q) * 2;
        f.set_word(q, u16::from_le_bytes([record[i], record[i + 1]]));
    }
    f
}

/// The rows of the shown tab for `inputs`, deriving them on `log`.
pub fn rows(log: &mut QuestLog, inputs: &QuestInputs, tab: u8, multiplayer: bool) -> Vec<TabRow> {
    log.status = inputs.status;
    log.den = inputs.counters[0];
    log.tomb = inputs.counters[1];
    log.barbarians = inputs.counters[2];
    let gate = TabGate {
        expansion_installed: false,
        expansion_game: false,
        cel_loaded: &|_| true,
    };
    log.build_tab(
        tab,
        &inputs.player,
        inputs.game.as_ref(),
        multiplayer,
        &gate,
    )
}

/// The log's slot position (cel draw point), d2rs-own, unverified.
pub fn slot_at(slot: u8, screen_h: i32) -> Point {
    let (col, row) = (i32::from(slot % 3), i32::from(slot / 3));
    Point::new(40 + col * 90, screen_h - 48 - 270 + row * 100)
}

/// The quest log adapter (ui 0x0F).
pub(super) struct QuestLogUi {
    pub(super) sh: SharedRef,
    pub(super) log: RefCell<QuestLog>,
}

/// Frame of an icon cel for a row state (`quests-status.md` §5): 24
/// completed, 26 not available, 0 in progress.
fn icon_frame(s: IconState) -> u32 {
    match s {
        IconState::JustCompleted | IconState::Completed => 24,
        IconState::NotAvailable => 26,
        IconState::InProgress => 0,
    }
}

fn string_of(ctx: &UiCtx, id: u16) -> Option<Vec<u16>> {
    ctx.strings.get_id(id).map(<[u16]>::to_vec)
}

fn row_text(ctx: &UiCtx, t: RowText) -> Option<Vec<u16>> {
    match t {
        RowText::Empty => None,
        RowText::Id(id) => string_of(ctx, id),
        RowText::Append(id, n) => {
            let mut s = string_of(ctx, id)?;
            s.extend(n.to_string().encode_utf16());
            Some(s)
        }
        RowText::Format(id, n) => {
            let s = string_of(ctx, id)?;
            let f: String = String::from_utf16_lossy(&s).replacen("%d", &n.to_string(), 1);
            Some(f.encode_utf16().collect())
        }
    }
}

impl Panel for QuestLogUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_QUEST_SCREEN))
    }

    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, (s.w / 2 + 1) as u16, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let files: &UiFiles = &sh.tables.files;
        let (Some(back), Some(sockets)) = (files.id(BACKGROUND), files.id(SOCKETS)) else {
            return;
        };
        let h = sh.config.screen.h;
        out.push(cel(back, 0, 0, h - 48));
        let tab = ctx.world.act.map_or(0, |a| a.act).min(4);
        let rows = rows(
            &mut self.log.borrow_mut(),
            &sh.quest,
            tab,
            ctx.world.expansion == 0,
        );
        let selected = self.log.borrow().selected;
        for r in &rows {
            let at = slot_at(r.slot, h);
            out.push(cel(sockets, u32::from(r.slot == selected), at.x, at.y));
            let name = format!("menu\\{}", ICON_NAMES[usize::from(r.icon)]);
            if let Some(icon) = files.id(&name) {
                out.push(cel(icon, icon_frame(r.row.icon), at.x, at.y));
            }
        }
        // The selected row: title, then the text (`quests-status.md` §3
        // rule 4; no wrap).
        let pick = rows.iter().find(|r| r.slot == selected).or(rows.first());
        let Some(r) = pick else {
            return;
        };
        let base = h - 48 - 70;
        if crate::ui::quest_log::state::title_drawn(r.row.title) {
            if let Some(t) = string_of(ctx, r.row.title) {
                out.push(text(t, 40, base, 1, 0));
            }
        }
        if let Some(t) = row_text(ctx, r.row.text) {
            out.push(text(t, 40, base + 24, 6, 0));
        }
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    /// d2rs-own, unverified: a press inside the slot's frame selects it;
    /// the panel takes every press inside its rectangle.
    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        let UiEvent::Press {
            button: crate::ui::PointerButton::Left,
            at,
        } = e
        else {
            return UiResponse::Ignored;
        };
        let sh = self.sh.borrow();
        let h = sh.config.screen.h;
        let tab = ctx.world.act.map_or(0, |a| a.act).min(4);
        let rows = rows(
            &mut self.log.borrow_mut(),
            &sh.quest,
            tab,
            ctx.world.expansion == 0,
        );
        for r in rows {
            let p = slot_at(r.slot, h);
            // The icon cels are about 70 × 70 (unmeasured).
            if (p.x..p.x + 70).contains(&at.x) && (p.y - 70..p.y).contains(&at.y) {
                self.log.borrow_mut().selected = r.slot;
                return UiResponse::Consumed;
            }
        }
        UiResponse::Ignored
    }
}

impl OriginalUi {
    /// Copies what a quest output set into the message state to the
    /// log's inputs ([`QuestInputs`]); other outputs change nothing.
    pub(super) fn sync_quest_inputs(&mut self, o: &crate::bridge::output::Output) {
        use crate::bridge::output::Output;
        let mut sh = self.shared.borrow_mut();
        match o {
            Output::QuestLog { status } => sh.quest.status = *status,
            Output::QuestFlags { .. } | Output::NpcDialog(_) => {
                sh.quest.player = flags_of(&self.more.client_quest);
            }
            Output::GameQuestFlags { record } => sh.quest.game = Some(flags_of(record)),
            Output::QuestSpecial { code: 1, .. } => {
                let w = self.more.quest_words;
                sh.quest.counters = [w[0], w[1], w[2]];
            }
            _ => {}
        }
    }

    /// The rows of the quest log's shown tab (`tab`), derived on a
    /// scratch log; empty before any quest message.
    pub fn quest_rows(&self, tab: u8, multiplayer: bool) -> Vec<TabRow> {
        rows(
            &mut QuestLog::new(),
            &self.shared.borrow().quest,
            tab,
            multiplayer,
        )
    }
}
