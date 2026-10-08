// Spec: specs/ui/control-panel.md (§5)
//! The belt in play: the rules of `ui::panels::control::belt` bound to
//! the client model. The belt items are drawn in their boxes (§5 r4),
//! the pop-up rows show while the mouse is over the belt strip (§5 r9),
//! and a click on a box sends 0x23 / 0x24 / 0x25 (§5 r11). The belt key
//! uses are `bridge::belt`.
//!
//! d2rs-own, unverified (preview fills):
//! - the belt type is read from the worn belt's `armor` row (`belt`
//!   column), default type 2 without one;
//! - the resolution index of `belts.bin` is 0 below mode 2, else 1;
//! - every belt item counts as usable, nothing is blocked; the cursor
//!   item fits a belt by code ([`super::panels::inv_items::fits_belt`]);
//! - no key labels or hover text (they need the string table by id).

use std::collections::BTreeMap;

use crate::bridge::items::{self, mode};
use crate::bridge::world::ClientWorld;
use crate::ui::draw::UiDrawSink;
use crate::ui::panel::ClientIntent;
use crate::ui::panels::control::belt::{
    record_index, BeltDraw, BeltEffect, BeltItem, BeltRecord, BeltSlot8, BeltState, CursorInfo,
    CursorItem, MoveGates, SlotInfo,
};
use crate::ui::panels::inv_items::{fits_belt, ItemsUi};
use crate::ui::panels::UiFiles;

/// The popped belt rows' art.
pub const POPBELT: &str = "panel\\ctrlpnl_popbelt";

/// The `belts.bin` records and the worn belts' types.
#[derive(Clone, Debug, Default)]
pub struct BeltParts {
    /// Records by `resolution · 7 + type` (§5 r1).
    pub records: Vec<BeltRecord>,
    /// Item code → `belts` row (`armor.txt` `belt`).
    pub types: BTreeMap<[u8; 4], u8>,
}

/// The belt of the HUD: the rule state and the tables.
#[derive(Clone, Debug, Default)]
pub struct HudBelt {
    pub state: BeltState,
    pub parts: BeltParts,
}

fn belt_view(world: &ClientWorld) -> BTreeMap<u16, crate::bridge::items::ItemView> {
    items::belt(world)
}

impl HudBelt {
    /// The record of the current belt type at resolution `res2`.
    fn record(&self, res2: bool) -> Option<&BeltRecord> {
        self.parts
            .records
            .get(record_index(u32::from(res2), self.state.belt_type))
    }

    /// §5 r1: the type from the worn belt (body location 8).
    fn update_type(&mut self, world: &ClientWorld) {
        let worn = items::local_items(world)
            .into_iter()
            .find(|i| i.mode == mode::BODY && i.body == 8);
        let slot8 = match worn {
            None => BeltSlot8::Empty,
            Some(i) => match i.code.and_then(|c| self.parts.types.get(&c)) {
                Some(&t) => BeltSlot8::Belt(i32::from(t)),
                None => BeltSlot8::Other,
            },
        };
        let _ = self.state.update_type(slot8);
    }

    fn slot_item(world: &ClientWorld) -> impl Fn(usize) -> Option<BeltItem> {
        let belt = belt_view(world);
        move |i| {
            belt.get(&(i as u16)).map(|v| BeltItem {
                guid: v.key.guid,
                usable: true,
                has_use: true,
                blocked: false,
                pos_x: i,
                quality3: false,
            })
        }
    }

    fn cursor(world: &ClientWorld) -> CursorInfo {
        CursorInfo {
            mode: 0,
            item: items::cursor_item(world).map(|c| CursorItem {
                guid: c.key.guid,
                fits_belt: fits_belt(c.code),
                swap_ok: true,
                blocked: false,
                put_sound: 0,
            }),
        }
    }

    /// The top of the area the belt takes while popped (for the panel's
    /// rectangle), when popped.
    pub fn popped_top(&self, res2: bool) -> Option<i32> {
        if !self.state.popped {
            return None;
        }
        self.record(res2)?.boxes.iter().map(|b| b.top).min()
    }

    /// Draws the belt: the type, the hover tracking, the pop-up rows and
    /// the items (§5 r1–r4, r9). `mouse` is the pointer in the frame.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        world: &ClientWorld,
        items_ui: &ItemsUi,
        files: &UiFiles,
        (w, h): (i32, i32),
        res2: bool,
        mouse: (i32, i32),
        alive: bool,
        out: &mut dyn UiDrawSink,
    ) {
        self.update_type(world);
        let Some(rec) = self.record(res2).cloned() else {
            return;
        };
        let slot_item = Self::slot_item(world);
        let cursor = Self::cursor(world);
        let gates = MoveGates {
            in_game: true,
            input_blocked: false,
            alive,
            x44bfe0: false,
            state_1f_open: false,
            state_9_open: false,
        };
        self.state
            .mouse_move(&rec, w, h, mouse.0, mouse.1, &gates, &cursor, &slot_item);
        let mut draws = self.state.popup_draws(w, h);
        let belt = belt_view(world);
        let slots: Vec<SlotInfo> = (0..rec.boxes.len())
            .map(|i| SlotInfo {
                item: slot_item(i),
                key_name: None,
            })
            .collect();
        draws.extend(self.state.slot_draws(&rec, &slots));
        for d in draws {
            match d {
                BeltDraw::PopRow { x, y } => {
                    if let Some(f) = files.id(POPBELT) {
                        out.push(crate::ui::panels::cel(f, 0, x, y));
                    }
                }
                BeltDraw::Item { guid, x, y } => {
                    if let Some(v) = belt.values().find(|v| v.key.guid == guid) {
                        items_ui.draw_at(files, v, (x, y), out);
                    }
                }
                // Highlight rectangles and labels need the line / text
                // draws the play HUD has none of (preview).
                BeltDraw::Box { .. } | BeltDraw::Label(_) => {}
            }
        }
    }

    /// Whether the point is on the belt (hit area §5 r6).
    pub fn over(
        &self,
        world: &ClientWorld,
        (w, h): (i32, i32),
        res2: bool,
        at: (i32, i32),
    ) -> bool {
        let _ = world;
        self.record(res2)
            .is_some_and(|r| self.state.hit_area(r, w, h, at.0, at.1))
    }

    /// The belt click on a release (§5 r11): the intents it sends.
    pub fn click(&self, world: &ClientWorld, res2: bool, at: (i32, i32)) -> Vec<ClientIntent> {
        let Some(rec) = self.record(res2) else {
            return Vec::new();
        };
        self.state
            .click(
                rec,
                at.0,
                at.1,
                &Self::cursor(world),
                &Self::slot_item(world),
            )
            .into_iter()
            .filter_map(|e| match e {
                BeltEffect::Send(i) => Some(i),
                _ => None,
            })
            .collect()
    }
}
