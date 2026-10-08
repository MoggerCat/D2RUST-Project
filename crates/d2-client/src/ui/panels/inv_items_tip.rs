// Spec: specs/ui/inventory.md (§5 r1 hover, §10 r1 cursor state 6), specs/items/inventory-moves.md (§7.18)
//! The inventory panel's item tool tip and the identify cursor.
//!
//! - Hover (`inventory.md` §5 r1): the item under the mouse in the
//!   page-0 grid, in an equipment box or in the belt gives the tip lines
//!   of [`ItemTips`].
//! - Identify (`inventory.md` §10 r1): a right press on a scroll or tome
//!   of identify in the grid takes it as the used item (cursor state 6);
//!   the next left press on a grid item sends C→S 0x27 (target, used)
//!   and ends the state. A right press while the state is set cancels it.
//!
//! d2rs-own, unverified (REC-114): the right press is not specified
//! (`0x00468830` is set by the use handler `0x005BF240`, unwritten), the
//! state is client-local, and the belt is not hit-tested for the tip.

use super::{InvLayout, ItemsUi};
use crate::bridge::items::{self, mode, ItemView};
use crate::bridge::world::ClientWorld;
use crate::ui::geom::Point;
use crate::ui::item_tip::TipLine;
use crate::ui::layout::Screen;
use crate::ui::panel::ClientIntent;
use crate::ui::panels::{PanelOutput, UiFiles};
use d2_proto::client::UseItemAction;

/// The Horadric Cube's code (`items.txt` `box`).
pub fn is_cube(code: Option<[u8; 4]>) -> bool {
    matches!(code, Some(c) if &c == b"box ")
}

/// The identify scroll and tome codes (as the server's use effect,
/// `d2_sim::wiring::inventory::identify::IDENTIFY_CODES`).
pub fn is_identify(code: Option<[u8; 4]>) -> bool {
    matches!(code, Some(c) if &c == b"isc " || &c == b"ibk ")
}

impl ItemsUi {
    /// The grid or equipped item under `at`, if any.
    pub fn item_at(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        layout: &InvLayout,
        at: Point,
    ) -> Option<ItemView> {
        let g = &layout.grid;
        let cell = (i32::from(g.cell_w), i32::from(g.cell_h));
        let all = items::local_items(world);
        if g.cell_w > 0 && g.cell_h > 0 && g.contains_mouse(at) {
            let (mc, mr) = g.mouse_cell(at);
            let (mc, mr) = (mc as i32, mr as i32);
            return all
                .into_iter()
                .filter(|i| i.mode == mode::STORED && i.page == 0)
                .find(|i| {
                    let (w, h) = self.art(files, i, cell).map_or((1, 1), |a| (a.w, a.h));
                    let (x, y) = (i32::from(i.x), i32::from(i.y));
                    (x..x + w).contains(&mc) && (y..y + h).contains(&mr)
                });
        }
        let loc = (1u8..=10).find(|&l| {
            let b = layout.equip[usize::from(l)];
            b.w > 0
                && b.h > 0
                && (b.left..b.left + b.w).contains(&at.x)
                && (b.top..b.top + b.h).contains(&at.y)
        })?;
        all.into_iter()
            .find(|i| i.mode == mode::BODY && i.body == loc)
    }

    /// The tool tip lines of the item under `at`; none with a cursor item
    /// (§5 r2 clears the hover) or without tips.
    pub fn hover_lines(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        screen: &Screen,
        class: Option<u32>,
        at: Point,
    ) -> Vec<TipLine> {
        let (Some(tips), Some(layout)) = (&self.tips, self.layout(class, screen)) else {
            return Vec::new();
        };
        if items::cursor_item(world).is_some() {
            return Vec::new();
        }
        let Some(it) = self.item_at(world, files, &layout, at) else {
            return Vec::new();
        };
        items::stream(world, it.key).map_or_else(Vec::new, |s| tips.lines(s))
    }

    /// A right press in the panel: an identify item in the grid becomes
    /// the used item (cursor state 6); while the state is set it cancels.
    /// A stored Horadric Cube (`box `) is used (C→S 0x20), which opens it
    /// (`world/cube.md` §1; d2rs-own, unverified, REC-118).
    pub fn right_press(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        layout: &InvLayout,
        at: Point,
    ) -> Vec<PanelOutput> {
        if self.identify.get().is_some() {
            self.identify.set(None);
            return Vec::new();
        }
        if items::cursor_item(world).is_some() {
            return Vec::new();
        }
        let Some(it) = self.item_at(world, files, layout, at) else {
            return Vec::new();
        };
        if it.mode != mode::STORED {
            return Vec::new();
        }
        if is_identify(it.code) {
            self.identify.set(Some(it.key.guid));
        } else if is_cube(it.code) {
            let (x, y) = world.local().and_then(|u| u.position).unwrap_or((0, 0));
            return vec![PanelOutput::Intent(ClientIntent::from_message(
                &items::use_grid(it.key.guid, u32::from(x), u32::from(y)),
            ))];
        }
        Vec::new()
    }

    /// The 0x27 intent of a [`GridMsg::TargetUsed`](super::super::inv_grid::GridMsg);
    /// ends the identify state.
    pub(super) fn target_used(&self, target: u32, used: u32) -> UseItemAction {
        self.identify.set(None);
        UseItemAction { target, used }
    }

    /// The identify state is on (the host may draw a cursor hint).
    pub fn identify_pending(&self) -> bool {
        self.identify.get().is_some()
    }
}
