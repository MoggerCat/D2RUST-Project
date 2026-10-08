// Spec: specs/ui/messages.md §11 (item-socket dialog, NPC mode 1: Charsi's imbue); specs/world/npc.md §8.1; preview fills: docs/handoff/q-imbue-ui.md
//! Charsi's imbue dialog (UI state 0x0E, NPC mode) in the play preview,
//! drawn and driven by the NPC menu panel (`npc_menu_ui.rs`): the menu's
//! Imbue row opens it, an item on the cursor is placed with a click in
//! the item area, the imbue button sends C→S 0x38 `[0][NPC][item]` and
//! the close button (or Esc-less press outside) ends the chat with 0x30.
//! S→C 0x58 codes 1, 5, 6, 7 close it, code 4 (refused, item kept) takes
//! it out of the waiting step (`messages/socket.rs` holds the full
//! original state machine; it is not wired here).
//!
//! Preview fills (`// d2rs-own, unverified`, REC-145 in `docs/HANDOFF.md`
//! §7): the dialog is plain text at the spec's geometry (no background
//! art or button cels), the NPC's accept check is the server's (any
//! cursor item can be placed), a placed item stays on the cursor until
//! the server consumes it, and the Imbue row is always offered in
//! Charsi's menu.

use super::draw::{TextRequest, TextStyle, UiDraw, UiDrawSink};
use super::geom::{Point, Rect, FRAME};
use super::messages::socket::{BUTTONS, MOUSE_WINDOW, STR_OK};
use super::panel::{ClientIntent, UiCtx, UiEvent, UiResponse};
use super::panels::npc::msg_chat_end;
use super::text::TextOpts;
use super::PointerButton;
use crate::bridge::world::{ClientWorld, KindData};
use d2_proto::client::EntityAction;

/// Charsi's NPC class.
pub use super::messages::socket::NPC_CHARSI;
/// The item area (`messages.md` §11 r5): x 123–211, y 106–220.
const ITEM_AREA: (i32, i32, i32, i32) = (123, 106, 211, 220);
/// Strings: the instruction (Charsi), "imbue" and "close" captions.
const STR_INSTRUCTION: u16 = 10076;
const STR_IMBUE: u16 = 4017;
const STR_CLOSE: u16 = 4143;
const STR_WAITING: u16 = 3353;

/// The open dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Imbue {
    pub npc: u32,
    /// The GUID placed in the item area (step 2).
    pub placed: Option<u32>,
    /// The 0x38 went out, the 0x58 answer is awaited (step 3).
    pub waiting: bool,
}

/// The local player's cursor item GUID (`cursor_item`, set by the
/// item messages).
fn cursor_guid(w: &ClientWorld) -> Option<u32> {
    match &w.local()?.kind {
        KindData::Player(p) => p.cursor_item,
        _ => None,
    }
}

impl Imbue {
    pub fn new(npc: u32) -> Self {
        Self {
            npc,
            placed: None,
            waiting: false,
        }
    }

    /// The dialog's rectangle (the original's mouse window).
    pub fn rect() -> Rect {
        let (l, t, r, b) = MOUSE_WINDOW;
        Rect::new(l, t, (r - l) as u16, (b - t) as u16)
    }

    fn in_item_area(p: Point) -> bool {
        let (l, t, r, b) = ITEM_AREA;
        (l..=r).contains(&p.x) && (t..=b).contains(&p.y)
    }

    /// A 0x58 code reached the dialog (`messages.md` §11 r3). `true`:
    /// close it.
    pub fn code(&mut self, code: u8) -> bool {
        match code {
            1 | 5 | 6 | 7 => true,
            4 => {
                self.waiting = false;
                false
            }
            _ => false,
        }
    }

    pub fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let label = |id: u16, fallback: &str| {
            ctx.strings
                .get_id(id)
                .map(<[u16]>::to_vec)
                .unwrap_or_else(|| fallback.encode_utf16().collect())
        };
        let mut text = |t: Vec<u16>, at: Point| {
            out.push(UiDraw::Text(TextRequest {
                text: t,
                at,
                style: TextStyle { font: 1, color: 0 },
                opts: TextOpts::default(),
                clip: FRAME,
            }));
        };
        text(
            label(STR_INSTRUCTION, "Place an item to imbue"),
            Point::new(115, 120),
        );
        if self.waiting {
            text(label(STR_WAITING, "Waiting..."), Point::new(150, 130));
        } else if self.placed.is_some() {
            text(label(STR_OK, "Item placed"), Point::new(150, 150));
        }
        let imbue = &BUTTONS[0];
        let close = &BUTTONS[1];
        text(label(STR_IMBUE, "Imbue"), Point::new(imbue.x, imbue.y));
        text(label(STR_CLOSE, "Close"), Point::new(close.x, close.y));
    }

    /// A left release at `at`. `Some(intent)` is to be sent; the second
    /// value says the dialog closes.
    pub fn release(&mut self, at: Point, ctx: &UiCtx) -> (UiResponse, bool) {
        if BUTTONS[1].hit(at.x, at.y) {
            let close = !self.waiting;
            let r = if close {
                UiResponse::Intent(ClientIntent(msg_chat_end(self.npc).to_vec()))
            } else {
                UiResponse::Consumed
            };
            return (r, close);
        }
        if BUTTONS[0].hit(at.x, at.y) {
            return match self.placed {
                // The server checks that the item is the cursor item.
                Some(g) if !self.waiting => {
                    self.waiting = true;
                    let i = ClientIntent::from_message(&EntityAction {
                        action: 0,
                        npc: self.npc,
                        item: g,
                    });
                    (UiResponse::Intent(i), false)
                }
                _ => (UiResponse::Consumed, false),
            };
        }
        if Self::in_item_area(at) && !self.waiting {
            self.placed = cursor_guid(ctx.world);
        }
        (UiResponse::Consumed, false)
    }

    /// Places `guid` as a click on the item area with it on the cursor
    /// does (a seam for the headless end-to-end test, whose client model
    /// has no item stream).
    pub fn place(&mut self, guid: u32) {
        if !self.waiting {
            self.placed = Some(guid);
        }
    }

    /// A press: inside the dialog it is the release's, outside it falls
    /// through to the inventory and the world.
    pub fn press_inside(at: Point) -> bool {
        Self::rect().contains(at)
    }

    pub fn left(e: UiEvent) -> Option<(bool, Point)> {
        match e {
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            } => Some((true, at)),
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            } => Some((false, at)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientUnit, PlayerData, UnitKey};
    use crate::ui::NoStrings;

    fn world(cursor: Option<u32>) -> ClientWorld {
        let mut w = ClientWorld::default();
        let p = UnitKey::new(0, 1);
        let mut u = ClientUnit::new(p);
        u.kind = KindData::Player(PlayerData {
            cursor_item: cursor,
            ..PlayerData::default()
        });
        w.units.insert(p, u);
        w.local_player = Some(p);
        w
    }

    // Covers: specs/ui/messages.md §11 r5, §11 r6
    #[test]
    fn place_then_imbue_sends_0x38_action_0() {
        let w = world(Some(77));
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        let mut im = Imbue::new(9);
        // Imbue before placing does nothing.
        assert_eq!(
            im.release(Point::new(130, 230), &ctx).0,
            UiResponse::Consumed
        );
        im.release(Point::new(160, 160), &ctx);
        assert_eq!(im.placed, Some(77));
        let (r, close) = im.release(Point::new(130, 230), &ctx);
        let want = ClientIntent::from_message(&EntityAction {
            action: 0,
            npc: 9,
            item: 77,
        });
        assert_eq!((r, close), (UiResponse::Intent(want), false));
        assert!(im.waiting);
        // Code 4 (refused, item kept) leaves the waiting step; 6 closes.
        assert!(!im.code(4) && !im.waiting);
        assert!(im.code(6));
    }

    // Covers: specs/ui/messages.md §11 r6
    #[test]
    fn close_button_ends_the_chat() {
        let w = world(None);
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        let mut im = Imbue::new(9);
        let (r, close) = im.release(Point::new(190, 230), &ctx);
        assert!(close);
        assert_eq!(
            r,
            UiResponse::Intent(ClientIntent(msg_chat_end(9).to_vec()))
        );
    }
}
