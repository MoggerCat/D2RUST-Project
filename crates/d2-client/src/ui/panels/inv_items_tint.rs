// Spec: specs/ui/inventory.md (§2 r1, §3 r2–r3, §6 r4)
//! The tints under the inventory's items: the grid items (`inventory.md`
//! §3 r2–r3) and the equipped items (§6 r4). Drawn before the item
//! graphics, so each graphic lands over its tint.
//!
//! Each tint is the UI rectangle primitive `0x0046EFD0(x, y, w, h,
//! colour, 0)` (§2 r2): draw mode 0, blend kind 2, `A2[256·d + c]` over
//! the destination (§2 r3), the colour the act palette's nearest entry of
//! §2 r1 ([`ItemsUi::tint_colors`]). Read for the requirement check:
//! strength, dexterity and level from the tables (REC-242, requirement stat
//! modifiers not applied). Not read: the shooter / quiver term of §6 r4,
//! `0x004C2240`, `0x0062A4E0`, the quest-item term of §3 r3, the
//! transmogrify cursor (state 8), and the placement tint for a cursor
//! item (§4); with an item on the cursor nothing is hovered.

use super::super::draw::{RectRequest, UiDraw, UiDrawSink};
use super::super::geom::{Point, Rect};
use super::super::inv_grid::{
    equip_item_tint, grid_item_tint, hovered_tint, EquipItemFacts, GridItemFacts, Tint,
};
use super::inv_items::{InvLayout, ItemsUi};
use crate::bridge::items::{self, mode, ItemView};
use crate::bridge::world::ClientWorld;

/// The tint's draw mode (§2 r2): blend kind 2, the 25 % alpha table.
pub const TINT_MODE: u8 = 0;

fn contains(r: &Rect, p: Point) -> bool {
    p.x >= r.x && p.x < r.x + i32::from(r.w) && p.y >= r.y && p.y < r.y + i32::from(r.h)
}

impl ItemsUi {
    /// The requirement check of `it` against the local player.
    fn requirements_fail(&self, world: &ClientWorld, it: &ItemView) -> bool {
        let (Some(me), Some(tips), Some(code)) = (world.local(), self.tips.as_ref(), it.code)
        else {
            return false;
        };
        !tips.can_use(code, me.stat(0), me.stat(2), me.stat(12))
    }

    fn grid_facts(&self, world: &ClientWorld, it: &ItemView) -> GridItemFacts {
        GridItemFacts {
            requirements_fail: self.requirements_fail(world, it),
            flag_4: it.flags & 0x4 != 0,
            identified: it.flags & 0x10 != 0,
            ..GridItemFacts::default()
        }
    }

    /// Paints the tints of the page-0 grid items and the equipped items
    /// of the local player (module doc). `mouse` is the cursor.
    pub fn draw_tints(
        &self,
        world: &ClientWorld,
        layout: &InvLayout,
        mouse: Point,
        out: &mut dyn UiDrawSink,
    ) {
        let Some(colors) = self.tint_colors else {
            return;
        };
        let g = &layout.grid;
        let hover_ok = items::cursor_item(world).is_none();
        let mut paint = |t: Tint, r: Rect| {
            out.push(UiDraw::Rect(RectRequest::sized(
                r.x,
                r.y,
                i32::from(r.w),
                i32::from(r.h),
                colors[t as usize],
                TINT_MODE,
            )));
        };
        for it in items::local_items(world) {
            match it.mode {
                mode::STORED if it.page == 0 => {
                    let (w, h) = self.art.get(it.code.unwrap_or([0; 4])).map_or((1, 1), |a| {
                        (i32::from(a.inv_w.max(1)), i32::from(a.inv_h.max(1)))
                    });
                    let (x, y, cw, ch) = g.cell(i32::from(it.x), i32::from(it.y));
                    let r = Rect::new(x, y, (cw * w) as u16, (ch * h) as u16);
                    let t = if hover_ok && contains(&r, mouse) {
                        hovered_tint(0, false)
                    } else {
                        grid_item_tint(&self.grid_facts(world, &it))
                    };
                    paint(t, r);
                }
                mode::BODY if (1..=10).contains(&it.body) => {
                    let b = layout.equip[usize::from(it.body)];
                    if b.w == 0 || b.h == 0 {
                        continue;
                    }
                    let r = Rect::new(b.left, b.top, b.w as u16, b.h as u16);
                    let t = equip_item_tint(&EquipItemFacts {
                        hovered: hover_ok && contains(&r, mouse),
                        grid: self.grid_facts(world, &it),
                        ..EquipItemFacts::default()
                    });
                    if let Some(t) = t {
                        paint(t, r);
                    }
                }
                _ => {}
            }
        }
    }
}
