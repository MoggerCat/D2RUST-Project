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
use super::imbue_ui::{Imbue, NPC_CHARSI};
use super::original::OriginalUi;
use super::panel::{ClientIntent, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use super::panels::npc::{msg_chat_end, option_intent, NpcMenus};
use super::panels::PanelOutput;
use super::text::TextOpts;
use super::PointerButton;
use crate::bridge::items;
use crate::bridge::world::ClientWorld;
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
    /// The `%d` the caption appends (`menus.md` §2.3: Cain's `100 × n`).
    pub cost: Option<u32>,
}

/// Cain's identify count `0x0062A530` (`world/npc.md` §6 step 3): the
/// local player's items without flag 0x10 in the backpack (page 0), the
/// cube (page 3) or worn; the stash and the belt are skipped.
pub fn unidentified_count(world: &ClientWorld) -> u32 {
    items::local_items(world)
        .iter()
        .filter(|i| {
            i.flags & 0x10 == 0
                && match i.mode {
                    items::mode::STORED => i.page == 0 || i.page == 3,
                    items::mode::BODY => true,
                    _ => false,
                }
        })
        .count() as u32
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
    /// Charsi's imbue dialog (`imbue_ui.rs`).
    pub imbue: Option<Imbue>,
    menus: Option<NpcMenus>,
    /// The last Trade / Gamble choice was Gamble: the shop that opens next
    /// is a gamble window (`panels-2.md` §14: the gamble shop flag).
    pub gamble: bool,
    /// The NPC (GUID, class) the last Trade / Gamble was chosen for and
    /// the model's store serial at the choice: the shop its store items
    /// open, showing the records newer than that serial
    /// (`OriginalUi::shop_poll`; d2rs-own, the menu's choice names the
    /// trader, `menus.md` §3.1).
    pub shop_for: Option<(u32, u32, u32)>,
    /// The Resurrect edit of the next open (`panels-2.md` §14.2): the cost
    /// `[0x007C0DD0]` while the mercenary is dead in an expansion game,
    /// else `None`.
    pub resurrect: Option<u32>,
    /// The Trade / Gamble choice's NPC (GUID, class): the shop opens for
    /// it on the next shop poll (`OriginalUi::shop_poll`). d2rs-own,
    /// unverified (REC-277).
    pub shop_request: Option<(u32, u32)>,
    pub screen: (i32, i32),
}

pub type SharedNpcMenu = Rc<RefCell<NpcMenuState>>;

/// A row's caption (`menus.md` §2.3): Resurrect is `hireresurrect2`
/// (22696, "%s … %d") with the merc name and the cost; the merc name id
/// `[0x00725494]` is not in the client model, so the name is empty
/// (d2rs-own gap). Identify is `NPCIdentify2` (4021) then `100 × n`.
fn row_caption(
    r: &Row,
    label: &dyn Fn(u16, Option<OptionKind>) -> Vec<u16>,
    strings: &dyn Fn(u16) -> Vec<u16>,
) -> Vec<u16> {
    use super::panels::npc_menu::{
        slot_caption, Caption, CaptionCtx, STR_IDENTIFY_COST, STR_RESURRECT_SLOT,
    };
    match (r.kind, r.cost) {
        (Some(OptionKind::Resurrect), Some(c)) => {
            let ctx = CaptionCtx {
                resurrect_cost: c as i32,
                ..CaptionCtx::default()
            };
            match slot_caption(STR_RESURRECT_SLOT, &ctx, strings).0 {
                Caption::Text(t) => t,
                Caption::Skip => Vec::new(),
            }
        }
        (_, Some(c)) => {
            let mut t = label(STR_IDENTIFY_COST, r.kind);
            t.extend(utf16s(&c.to_string()));
            t
        }
        (_, None) => label(r.string, r.kind),
    }
}

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
        Some(OptionKind::Identify) => "Identify Items: ",
        Some(OptionKind::Resurrect) => "Resurrect",
        Some(OptionKind::Imbue) => "Imbue",
        None => "Cancel",
    }
}

impl NpcMenuState {
    /// Opens the menu for the NPC `guid` of class `class` (`menus.md`
    /// §2.2): the record of the class with the per-interaction edits and
    /// the builder additions (`panels.md` §14.2). A class without a
    /// record, or with only a talk option and no speech to show, still
    /// gets the box (record 0 rule, `panels.md` §14.7).
    pub fn open(
        &mut self,
        guid: u32,
        class: u32,
        char_level: i32,
        speech: Vec<u16>,
        identify_n: u32,
    ) {
        let menus = self.menus.get_or_insert_with(|| {
            NpcMenus::load().unwrap_or_else(|_| NpcMenus::from_records(Vec::new()))
        });
        self.imbue = None;
        menus.reset_for_interaction();
        menus.apply_builder(char_level);
        menus.apply_resurrect(self.resurrect.is_some(), true);
        let Some(rec) = menus.menu(class) else {
            return;
        };
        let mut rows: Vec<Row> = super::panels::npc::shown_options(rec)
            .into_iter()
            .flatten()
            // `menus.md` §2.3: no items to identify, no row; else the
            // caption carries `100 × n` (the quest-4 bits that waive it
            // are not in the client model: d2rs-own, unverified).
            .filter(|o| o.kind != OptionKind::Identify || identify_n > 0)
            .map(|o| Row {
                string: o.string,
                kind: Some(o.kind),
                cost: match o.kind {
                    OptionKind::Identify => Some(100 * identify_n),
                    // `menus.md` §2.3: `hireresurrect2` with the cost.
                    OptionKind::Resurrect => self.resurrect,
                    _ => None,
                },
            })
            .collect();
        if class == NPC_CHARSI {
            // d2rs-own, unverified (REC-145): the imbue insert.
            rows.push(Row {
                string: 4017,
                kind: Some(OptionKind::Imbue),
                cost: None,
            });
        }
        rows.push(Row {
            string: STR_CANCEL,
            kind: None,
            cost: None,
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
        if st.imbue.is_some() {
            return Imbue::rect();
        }
        if st.up.is_none() {
            return Rect::new(0, 0, 0, 0);
        }
        // The whole frame: a press outside the box ends the chat.
        FRAME
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let st = self.st.borrow();
        if let Some(im) = &st.imbue {
            im.draw(ctx, out);
            return;
        }
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
            let t = row_caption(r, &label, &|id| {
                ctx.strings
                    .get_id(id)
                    .map(<[u16]>::to_vec)
                    .unwrap_or_default()
            });
            out.push(text(t, Point::new(x + 20, y + ROW_H * (i as i32 + 2)), 0));
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
        if self.st.borrow().imbue.is_some() {
            return Imbue::rect().contains(p).then_some(WidgetId(0));
        }
        self.st
            .borrow()
            .box_rect()
            .contains(p)
            .then_some(WidgetId(0))
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        if self.st.borrow().imbue.is_some() {
            return self.imbue_event(e, ctx);
        }
        let Some((guid, class)) = self.st.borrow().up.as_ref().map(|o| (o.guid, o.class)) else {
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
            Some(OptionKind::Imbue) => {
                let mut st = self.st.borrow_mut();
                st.up = None;
                st.imbue = Some(Imbue::new(guid));
                UiResponse::Consumed
            }
            Some(OptionKind::Hire) => {
                self.hire.borrow_mut().up = Some(guid);
                self.st.borrow_mut().up = None;
                UiResponse::Consumed
            }
            Some(kind) => {
                {
                    let mut st = self.st.borrow_mut();
                    st.up = None;
                    if matches!(kind, OptionKind::Trade | OptionKind::Gamble) {
                        st.gamble = kind == OptionKind::Gamble;
                        st.shop_for = Some((guid, class, ctx.world.store_serial));
                        st.shop_request = Some((guid, class));
                    }
                }
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

impl NpcMenuUi {
    fn imbue_event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        let Some((press, at)) = Imbue::left(e) else {
            return UiResponse::Ignored;
        };
        if !Imbue::press_inside(at) {
            // Outside the dialog the inventory and the world keep working.
            return UiResponse::Ignored;
        }
        if press {
            return UiResponse::Consumed;
        }
        let mut st = self.st.borrow_mut();
        let Some(im) = st.imbue.as_mut() else {
            return UiResponse::Ignored;
        };
        let (r, close) = im.release(at, ctx);
        if close {
            st.imbue = None;
        }
        r
    }
}

impl OriginalUi {
    /// A delivered S->C 0x58 reaches Charsi's imbue dialog.
    pub(super) fn imbue_output(&mut self, o: &crate::bridge::output::Output) {
        let crate::bridge::output::Output::OpenUi { code, .. } = *o else {
            return;
        };
        let mut st = self.npcm.borrow_mut();
        if st.imbue.as_mut().is_some_and(|im| im.code(code)) {
            st.imbue = None;
        }
    }

    /// Opens the NPC menu for a delivered 0x28 (`msg_ui`).
    pub fn open_npc_menu(&mut self, guid: u32, class: u32, level: i32) {
        self.open_npc_menu_with(guid, class, level, 0);
    }

    /// [`Self::open_npc_menu`] with Cain's count of items to identify.
    pub fn open_npc_menu_with(&mut self, guid: u32, class: u32, level: i32, identify_n: u32) {
        let speech: Vec<u16> = self.npc_text().map_or(Vec::new(), |t| {
            t.nodes()
                .into_iter()
                .filter(|&(k, _)| k == 0)
                .map(|(_, id)| id)
                .collect()
        });
        // A seller's hire list (S→C 0x4F) opens from the Hire option.
        self.hire.borrow_mut().up = None;
        self.npcm
            .borrow_mut()
            .open(guid, class, level, speech, identify_n);
    }

    /// The NPC menu is up (the preview's automatic chat close waits for
    /// its cancel).
    pub fn npc_menu_up(&self) -> bool {
        self.npcm.borrow().up.is_some() || self.npcm.borrow().imbue.is_some()
    }

    /// Places `guid` in the open imbue dialog (headless-test seam).
    pub fn imbue_place(&mut self, guid: u32) {
        if let Some(im) = self.npcm.borrow_mut().imbue.as_mut() {
            im.place(guid);
        }
    }

    /// The open menu, for tests.
    pub fn npc_menu(&self) -> Option<Open> {
        self.npcm.borrow().up.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(id: u16) -> Vec<u16> {
        match id {
            22696 => utf16s("Resurrect %s: %d"),
            4021 => utf16s("Identify Items: "),
            _ => Vec::new(),
        }
    }

    // The Resurrect row is `hireresurrect2` with the cost, not the
    // identify caption; Identify keeps 4021 + the cost.
    // Covers: specs/ui/menus.md §2 r3
    #[test]
    fn resurrect_row_is_captioned_by_hireresurrect2() {
        let label = |id: u16, _: Option<OptionKind>| strings(id);
        let row = |kind, cost| Row {
            string: 0x1507,
            kind: Some(kind),
            cost: Some(cost),
        };
        let t = row_caption(&row(OptionKind::Resurrect, 500), &label, &strings);
        assert_eq!(String::from_utf16_lossy(&t), "Resurrect : 500");
        let t = row_caption(&row(OptionKind::Identify, 300), &label, &strings);
        assert_eq!(String::from_utf16_lossy(&t), "Identify Items: 300");
    }
}
