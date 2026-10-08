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
//! - the key labels are the default bindings' names (`1`–`4`, REC-240):
//!   the bound key's name needs the controls table, which the HUD does
//!   not hold; a rebound key still shows its default label;
//! - the hover tip is the item tool tip ([`crate::ui::item_tip`]) at the
//!   text position, not the `0x0048C060` / `0x004E6410` strings;
//! - the highlight rectangles are in the draw list ([`BeltDraw::Box`])
//!   but not painted: the play sink has no rectangle primitive.

use std::collections::BTreeMap;

use crate::bridge::items::{self, mode};
use crate::bridge::world::ClientWorld;
use crate::ui::draw::UiDrawSink;
use crate::ui::item_tip::{ItemTips, TipLine};
use crate::ui::panel::ClientIntent;
use crate::ui::panels::control::belt::{
    hover_text, record_index, BeltDraw, BeltEffect, BeltItem, BeltRecord, BeltSlot8, BeltState,
    CursorInfo, CursorItem, MoveGates, SlotInfo, FONT_AFTER_BELT,
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

    /// The key label of belt slot `i` (§5 r4): the default binding's
    /// name (d2rs-own, unverified, REC-240).
    fn key_name(i: usize) -> Option<Vec<u16>> {
        (i < 4).then(|| vec![u16::from(b'1') + i as u16])
    }

    /// The belt's draw list (§5 r1–r5, r9): the type, the hover
    /// tracking, the pop-up rows, then the slots (rectangles, items,
    /// key labels) and the cursor-item highlight.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_list(
        &mut self,
        world: &ClientWorld,
        (w, h): (i32, i32),
        res2: bool,
        mouse: (i32, i32),
        alive: bool,
    ) -> Vec<BeltDraw> {
        self.update_type(world);
        let Some(rec) = self.record(res2).cloned() else {
            return Vec::new();
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
        let slots: Vec<SlotInfo> = (0..rec.boxes.len())
            .map(|i| SlotInfo {
                item: slot_item(i),
                key_name: Self::key_name(i),
            })
            .collect();
        draws.extend(self.state.slot_draws(&rec, &slots));
        draws
    }

    /// Draws the belt (`draw_list`). `mouse` is the pointer in the frame.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        world: &ClientWorld,
        items_ui: &ItemsUi,
        files: &UiFiles,
        size: (i32, i32),
        res2: bool,
        mouse: (i32, i32),
        alive: bool,
        out: &mut dyn UiDrawSink,
    ) {
        let draws = self.draw_list(world, size, res2, mouse, alive);
        let belt = belt_view(world);
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
                // §5 r4: font 1, color 4.
                BeltDraw::Label(l) => {
                    out.push(crate::ui::panels::text(
                        l.text,
                        l.x,
                        l.y,
                        FONT_AFTER_BELT,
                        l.color as u16,
                    ));
                }
                // No rectangle primitive in the play sink (module doc).
                BeltDraw::Box { .. } => {}
            }
        }
    }

    /// The hover tip of the hovered belt item (§5 r8) and its anchor:
    /// the lines of its last item stream. Empty unless the hover gate of
    /// [`hover_text`] holds (belt hovered, an item, no cursor item, box
    /// ≤ 3 or popped).
    pub fn hover_tip(&self, world: &ClientWorld, tips: &ItemTips) -> (Vec<TipLine>, (i32, i32)) {
        let belt = belt_view(world);
        let Some(v) = self
            .state
            .hover_item
            .and_then(|g| belt.values().find(|v| v.key.guid == g))
        else {
            return (Vec::new(), (0, 0));
        };
        let item = BeltItem {
            guid: v.key.guid,
            usable: true,
            has_use: true,
            blocked: false,
            pos_x: 0,
            quality3: false,
        };
        let cursor = items::cursor_item(world).is_some();
        if hover_text(&self.state, cursor, Some(&item), &[], &[], None).is_none() {
            return (Vec::new(), (0, 0));
        }
        let lines = items::stream(world, v.key).map_or_else(Vec::new, |s| tips.lines(s));
        (lines, self.state.text_pos)
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
