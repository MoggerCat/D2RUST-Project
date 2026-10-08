// Spec: specs/ui/menus.md §2 (NPC menu box), specs/ui/panels.md §14.1–§14.2 (option table), specs/world/npc.md §3 (chat close); preview fills: docs/handoff/q-npc-menu.md
//! The NPC menu (ui 8) in the play preview: a town NPC's click opens the
//! option box (Talk / Trade / Hire / Gamble / Identify per
//! `npc-menus.tsv`, the edits of `panels.md` §14.2 applied), Talk shows the
//! NPC's text list, Trade sends C→S 0x38 action 1 (the vendor panel is
//! `OriginalUi::open_shop`, another branch), Hire opens the hire list,
//! Identify sends C→S 0x34 and the cancel row (or a press outside the box)
//! ends the chat with C→S 0x30.
//!
//! The box opens when S→C 0x28 is delivered (`msg_ui`): the dialog
//! branch's C→S 0x2F / 0x31 are the bridge's, as before. Preview fills
//! (`// d2rs-own, unverified`, REC-123 in `docs/HANDOFF.md` §7): plain
//! text rows at a fixed anchor (no box art, no anchor from the NPC's
//! screen point, no selection colour), English labels when the string
//! table has no id, Talk draws the text list's strings (kind 0) with no
//! scroll widget and sends nothing (the 0x2F went out with the 0x28),
//! and no NPC name line (monstats name keys are not in the client).

use std::cell::RefCell;
use std::rc::Rc;

use super::draw::{TextRequest, TextStyle, UiDraw, UiDrawSink};
use super::geom::{Point, Rect, FRAME};
use super::original::OriginalUi;
use super::panel::{ClientIntent, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use super::panels::npc::{msg_chat_end, option_intent, NpcMenus};
use super::panels::PanelOutput;
use super::text::TextOpts;
use super::PointerButton;
use crate::ui::layout::OptionKind;

/// The panel id (one past the hire list's, always open).
pub const NPC_MENU_PANEL: PanelId = PanelId(0x103);
const ROW_H: i32 = 20;
const BOX_W: i32 = 200;
/// `lowercasecancel` (`menus.md` §2.2).
const STR_CANCEL: u16 = 4142;
/// NPC classes whose hire list the Hire option opens (`menus.md` §3.1).
const NPC_TALK_BOX_W: i32 = 360;

/// One option row: its string id and what it does (`None` = cancel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub string: u16,
    pub kind: Option<OptionKind>,
}

/// The open menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub guid: u32,
    pub class: u32,
    pub rows: Vec<Row>,
    /// The text list's strings of kind 0 (the NPC's speech), by id.
    pub speech: Vec<u16>,
    /// Talk was chosen: the speech is drawn.
    pub talking: bool,
}

#[derive(Default)]
pub struct NpcMenuState {
    pub up: Option<Open>,
    menus: Option<NpcMenus>,
    pub screen: (i32, i32),
}

pub type SharedNpcMenu = Rc<RefCell<NpcMenuState>>;

fn utf16s(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// d2rs-own, unverified: the label when no string table is bound.
fn fallback(kind: Option<OptionKind>) -> &'static str {
    match kind {
        Some(OptionKind::Talk) => "Talk",
        Some(OptionKind::Trade) => "Trade",
        Some(OptionKind::Gamble) => "Gamble",
        Some(OptionKind::Hire) => "Hire",
        Some(OptionKind::TravelWest) => "Travel",
        Some(OptionKind::SailWest) => "Sail",
        Some(OptionKind::Identify) => "Identify Items",
        Some(OptionKind::Resurrect) => "Resurrect",
        None => "Cancel",
    }
}

impl NpcMenuState {
    /// Opens the menu for the NPC `guid` of class `class` (`menus.md`
    /// §2.2): the record of the class with the per-interaction edits and
    /// the builder additions (`panels.md` §14.2). A class without a
    /// record, or with only a talk option and no speech to show, still
    /// gets the box (record 0 rule, `panels.md` §14.7).
    pub fn open(&mut self, guid: u32, class: u32, char_level: i32, speech: Vec<u16>) {
        let menus = self.menus.get_or_insert_with(|| {
            NpcMenus::load().unwrap_or_else(|_| NpcMenus::from_records(Vec::new()))
        });
        menus.reset_for_interaction();
        menus.apply_builder(char_level);
        let Some(rec) = menus.menu(class) else {
            return;
        };
        let mut rows: Vec<Row> = super::panels::npc::shown_options(rec)
            .into_iter()
            .flatten()
            .map(|o| Row {
                string: o.string,
                kind: Some(o.kind),
            })
            .collect();
        rows.push(Row {
            string: STR_CANCEL,
            kind: None,
        });
        self.up = Some(Open {
            guid,
            class,
            rows,
            speech,
            talking: false,
        });
    }

    fn box_pos(&self) -> (i32, i32) {
        // d2rs-own, unverified: centred, a third down the frame.
        ((self.screen.0 - BOX_W) / 2, self.screen.1 / 4)
    }

    fn box_rect(&self) -> Rect {
        let Some(o) = &self.up else {
            return Rect::new(0, 0, 0, 0);
        };
        let (x, y) = self.box_pos();
        Rect::new(
            x,
            y,
            BOX_W as u16,
            (ROW_H * (o.rows.len() as i32 + 1)) as u16,
        )
    }

    /// The row index at `p`.
    fn row_at(&self, p: Point) -> Option<usize> {
        let o = self.up.as_ref()?;
        let (x, y) = self.box_pos();
        let top = y + ROW_H;
        if p.x < x || p.x >= x + BOX_W || p.y < top {
            return None;
        }
        let k = ((p.y - top) / ROW_H) as usize;
        (k < o.rows.len()).then_some(k)
    }
}

/// The panel.
pub struct NpcMenuUi {
    pub st: SharedNpcMenu,
    pub hire: super::hire_list::SharedHire,
}

impl Panel for NpcMenuUi {
    fn id(&self) -> PanelId {
        NPC_MENU_PANEL
    }

    fn rect(&self) -> Rect {
        let st = self.st.borrow();
        if st.up.is_none() {
            return Rect::new(0, 0, 0, 0);
        }
        // The whole frame: a press outside the box ends the chat.
        FRAME
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let st = self.st.borrow();
        let Some(o) = &st.up else {
            return;
        };
        let label = |id: u16, kind: Option<OptionKind>| {
            ctx.strings
                .get_id(id)
                .map(<[u16]>::to_vec)
                .unwrap_or_else(|| utf16s(fallback(kind)))
        };
        let text = |text: Vec<u16>, at: Point, color: u16| {
            UiDraw::Text(TextRequest {
                text,
                at,
                style: TextStyle { font: 1, color },
                opts: TextOpts::default(),
                clip: FRAME,
            })
        };
        let (x, y) = st.box_pos();
        for (i, r) in o.rows.iter().enumerate() {
            out.push(text(
                label(r.string, r.kind),
                Point::new(x + 20, y + ROW_H * (i as i32 + 2)),
                0,
            ));
        }
        if o.talking {
            let tx = (st.screen.0 - NPC_TALK_BOX_W) / 2;
            for (i, id) in o.speech.iter().enumerate() {
                if let Some(t) = ctx.strings.get_id(*id) {
                    out.push(text(
                        t.to_vec(),
                        Point::new(tx, y + ROW_H * (o.rows.len() as i32 + 3 + i as i32)),
                        0,
                    ));
                }
            }
        }
    }

    fn hit(&self, p: Point) -> Option<WidgetId> {
        self.st
            .borrow()
            .box_rect()
            .contains(p)
            .then_some(WidgetId(0))
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        let Some(guid) = self.st.borrow().up.as_ref().map(|o| o.guid) else {
            return UiResponse::Ignored;
        };
        let at = match e {
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            } => {
                // Presses inside the box are the release's; one outside
                // ends the chat and the click goes on to the world.
                if self.st.borrow().box_rect().contains(at) {
                    return UiResponse::Consumed;
                }
                self.st.borrow_mut().up = None;
                return UiResponse::Intent(ClientIntent(msg_chat_end(guid).to_vec()));
            }
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            } => at,
            _ => return UiResponse::Ignored,
        };
        let Some(k) = self.st.borrow().row_at(at) else {
            return UiResponse::Consumed;
        };
        let row = self.st.borrow().up.as_ref().map(|o| o.rows[k]).unwrap();
        match row.kind {
            // Talk: the speech is shown; the 0x2F went out with the 0x28.
            Some(OptionKind::Talk) => {
                if let Some(o) = self.st.borrow_mut().up.as_mut() {
                    o.talking = true;
                }
                UiResponse::Consumed
            }
            Some(OptionKind::Hire) => {
                self.hire.borrow_mut().up = Some(guid);
                self.st.borrow_mut().up = None;
                UiResponse::Consumed
            }
            Some(kind) => {
                self.st.borrow_mut().up = None;
                match option_intent(kind, guid).and_then(|v| v.into_iter().next()) {
                    Some(PanelOutput::Intent(i)) => UiResponse::Intent(i),
                    _ => UiResponse::Consumed,
                }
            }
            None => {
                self.st.borrow_mut().up = None;
                UiResponse::Intent(ClientIntent(msg_chat_end(guid).to_vec()))
            }
        }
    }
}

impl OriginalUi {
    /// Opens the NPC menu for a delivered 0x28 (`msg_ui`).
    pub(super) fn open_npc_menu(&mut self, guid: u32, class: u32, level: i32) {
        let speech: Vec<u16> = self.npc_text().map_or(Vec::new(), |t| {
            t.nodes()
                .into_iter()
                .filter(|&(k, _)| k == 0)
                .map(|(_, id)| id)
                .collect()
        });
        // A seller's hire list (S→C 0x4F) opens from the Hire option.
        self.hire.borrow_mut().up = None;
        self.npcm.borrow_mut().open(guid, class, level, speech);
    }

    /// The NPC menu is up (the preview's automatic chat close waits for
    /// its cancel).
    pub fn npc_menu_up(&self) -> bool {
        self.npcm.borrow().up.is_some()
    }

    /// The open menu, for tests.
    pub fn npc_menu(&self) -> Option<Open> {
        self.npcm.borrow().up.clone()
    }
}
