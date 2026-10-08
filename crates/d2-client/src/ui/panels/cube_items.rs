// Spec: specs/ui/panels.md (§12 r3), specs/ui/inventory.md (§1 r2, §10), specs/items/inventory.md (§1.3)
//! The Horadric Cube grid (ui 0x1A, page 3): the local player's page-3
//! items drawn with their inventory graphic and a left press on the grid
//! sent as a C→S item intent (the page-3 grid click is the inventory's,
//! `inventory.md` §10, with page 3). The server checks every move
//! (`items/inventory-moves.md` §7, `world/cube.md` §2).
//!
//! d2rs-own, unverified: without `inventory.bin` rows the grid is an
//! estimated 3 × 4 grid of 29-pixel cells centred in the left half, above
//! the transmute button. PROVISIONAL, REC-116 in `docs/HANDOFF.md` §7.

use super::super::draw::UiDrawSink;
use super::super::geom::Point;
use super::super::inv_grid::GridRecord;
use super::super::layout::Screen;
use super::inv_items::ItemsUi;
use super::{cel, PanelOutput, UiFiles};
use crate::bridge::items::{self, mode};
use crate::bridge::world::ClientWorld;

/// The cube page (`items/inventory.md` §1.2).
pub const CUBE_PAGE: u8 = 3;

/// The `inventory.bin` record of the cube grid, "Transmogrify Box" (9),
/// +16 at 800 × 600 (`items/inventory.md` §1.3).
pub fn cube_record(screen: &Screen) -> usize {
    if screen.res2() {
        9 + 16
    } else {
        9
    }
}

/// d2rs-own, unverified: the estimated corner (module doc).
pub fn fallback_cube_grid(screen: &Screen) -> GridRecord {
    let (dx, dy) = if screen.res2() { (80, 60) } else { (0, 0) };
    GridRecord {
        grid_x: 3,
        grid_y: 4,
        left: 116 + dx,
        right: 116 + dx + 3 * 29,
        top: 130 + dy,
        bottom: 130 + dy + 4 * 29,
        cell_w: 29,
        cell_h: 29,
    }
}

/// The cube item (code `box `, stored in the inventory) is in the model.
pub fn cube_present(world: &ClientWorld) -> bool {
    items::local_items(world)
        .iter()
        .any(|i| i.mode == mode::STORED && i.code == Some(*b"box "))
}

impl ItemsUi {
    /// The cube grid's record for this install.
    pub fn cube_grid(&self, screen: &Screen) -> GridRecord {
        match &self.layouts {
            Some(l) => l
                .get(cube_record(screen))
                .map_or_else(|| fallback_cube_grid(screen), |l| l.grid),
            None => fallback_cube_grid(screen),
        }
    }

    /// The local player's page-3 items (stored mode), graphics drawn with
    /// the frame's top-left at the cell (`inventory.md` §8 r4).
    pub fn draw_cube(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        g: &GridRecord,
        out: &mut dyn UiDrawSink,
    ) {
        let cell = (i32::from(g.cell_w), i32::from(g.cell_h));
        for it in items::local_items(world) {
            if it.mode != mode::STORED || it.page != CUBE_PAGE {
                continue;
            }
            let Some(a) = self.art(files, &it, cell) else {
                continue;
            };
            let (x, y, _, _) = g.cell(i32::from(it.x), i32::from(it.y));
            out.push(cel(a.file, 0, x, y + a.gh));
        }
    }

    /// Left mouse down on the cube grid: the page-3 grid click.
    pub fn press_cube(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        g: &GridRecord,
        at: Point,
    ) -> Vec<PanelOutput> {
        if g.cell_w == 0 || g.cell_h == 0 || !g.contains_mouse(at) {
            return Vec::new();
        }
        let all = items::local_items(world);
        let cursor = items::cursor_item(world);
        self.grid_press(files, g, &all, cursor.as_ref(), at, CUBE_PAGE)
            .map(PanelOutput::Intent)
            .into_iter()
            .collect()
    }
}
