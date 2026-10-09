// Spec: specs/ui/inventory.md (§1 r1–r4, §3 r1, §5 r3, §6 r1–r2, §8 r2–r4, §10 r3–r4), specs/ui/panels.md (§9.2, §9.6, §9.7), specs/ui/panels-3.md (§23 r9, §29)
//! The local player's items in the inventory panel (ui 1): the page-0
//! grid items (`inventory.md` §3 r1) and the equipped items (§6 r2)
//! drawn with their inventory graphic (§8), the cursor item at the mouse
//! (`panels-3.md` §23 r9), and a left press on the grid (§10) or on an
//! equipment box (`panels-3.md` §29, [`equip`]) sent as a C→S item intent
//! through the root's outbox.
//!
//! Plain decisions over the client model ([`crate::bridge::items`]);
//! the server checks every move (`items/inventory-moves.md` §7).
//!
//! d2rs-own, unverified (preview fills, each named where it is used):
//! - the graphic frame size: the cel draw position needs the frame
//!   height (§8 r4: drawn at (x, top + h)); without a measured size
//!   ([`ItemsUi::frame_sizes`]) it is estimated as the footprint in
//!   cells × the cell size;
//! - the layout without `inventory.bin` rows: the grid of the spec's
//!   measured record 0 / 16 (`panels.md` §Test vectors), no equipment
//!   boxes;
//! - the drop cell `0x00486BD0` (not specified) is the cursor cell;
//! - tints (§3 r2–r3, §6 r4), sockets, ethereal draw mode and the
//!   item's colour remap are not drawn ([`super::super::ImageRequest`]
//!   has no draw mode or remap field).

use std::collections::BTreeMap;

use d2_proto::client::SwapCursorBufferItem;

use super::super::draw::{CelLook, Remap, UiDraw, UiDrawSink, DRAW_MODE_OPAQUE};
use super::super::geom::Point;
use super::super::inv_grid::HoverState;
use super::super::inv_grid::{
    equip_draw_point, grid_click, ClickCtx, EquipBox, GridMsg, GridRecord, ItemRef,
};
use super::super::layout::Screen;
use super::super::panel::{ClientIntent, WidgetId};
use super::super::widget::CellGrid;
use super::{cel, PanelOutput, UiFiles};
use crate::bridge::items::{self, mode, ItemArtRows, ItemView};
use crate::bridge::world::ClientWorld;

/// The [`UiFiles`] name prefix of an item graphic: a name
/// `*items\<invfile>` is `data\global\items\<invfile>.dc6` (§8 r2), not a
/// file under `data\global\ui\`.
pub const ITEMS_PREFIX: &str = "*items\\";

/// The [`UiFiles`] name of item graphic `inv_file`.
pub fn item_file_name(inv_file: &str) -> String {
    format!("{ITEMS_PREFIX}{}", inv_file.to_ascii_lowercase())
}

/// One `inventory.bin` record as the item draws read it: the page-0 grid
/// (§1 r1) and the equipment boxes by body location 0–10 (§6 r1; index 0
/// unused).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvLayout {
    pub grid: GridRecord,
    pub equip: [EquipBox; 11],
}

const NO_BOX: EquipBox = EquipBox {
    left: 0,
    top: 0,
    w: 0,
    h: 0,
};

/// The layout of an `inventory.bin` row (`panels.md` §9.2: grid, then the
/// ten rectangles into `0x007BCC58 + 0x14·L`: head 1, neck 2, torso 3,
/// rArm 4, lArm 5, rHand 6, lHand 7, belt 8, feet 9, gloves 10; each box
/// left, top and the width / height bytes).
pub fn inv_layout(r: &d2_data::tables::Inventory) -> InvLayout {
    let b = |left: u32, top: u32, w: u8, h: u8| EquipBox {
        left: left as i32,
        top: top as i32,
        w: i32::from(w),
        h: i32::from(h),
    };
    InvLayout {
        grid: GridRecord {
            grid_x: r.gridx,
            grid_y: r.gridy,
            left: r.gridleft as i32,
            right: r.gridright as i32,
            top: r.gridtop as i32,
            bottom: r.gridbottom as i32,
            cell_w: r.gridboxwidth,
            cell_h: r.gridboxheight,
        },
        equip: [
            NO_BOX,
            b(r.headleft, r.headtop, r.headwidth, r.headheight),
            b(r.neckleft, r.necktop, r.neckwidth, r.neckheight),
            b(r.torsoleft, r.torsotop, r.torsowidth, r.torsoheight),
            b(r.rarmleft, r.rarmtop, r.rarmwidth, r.rarmheight),
            b(r.larmleft, r.larmtop, r.larmwidth, r.larmheight),
            b(r.rhandleft, r.rhandtop, r.rhandwidth, r.rhandheight),
            b(r.lhandleft, r.lhandtop, r.lhandwidth, r.lhandheight),
            b(r.beltleft, r.belttop, r.beltwidth, r.beltheight),
            b(r.feetleft, r.feettop, r.feetwidth, r.feetheight),
            b(r.glovesleft, r.glovestop, r.gloveswidth, r.glovesheight),
        ],
    }
}

/// The ten equipment rectangles of an `inventory.bin` row (`panels.md`
/// §9.2), the empty-slot pictures' anchors (§9.4: left, bottom).
pub fn equip_rects(r: &d2_data::tables::Inventory) -> super::inventory::EquipRects {
    use super::inventory::BinRect;
    let b =
        |l: u32, rt: u32, t: u32, bt: u32| BinRect::new(l as i32, rt as i32, t as i32, bt as i32);
    super::inventory::EquipRects {
        r_arm: b(r.rarmleft, r.rarmright, r.rarmtop, r.rarmbottom),
        torso: b(r.torsoleft, r.torsoright, r.torsotop, r.torsobottom),
        l_arm: b(r.larmleft, r.larmright, r.larmtop, r.larmbottom),
        head: b(r.headleft, r.headright, r.headtop, r.headbottom),
        neck: b(r.neckleft, r.neckright, r.necktop, r.neckbottom),
        r_hand: b(r.rhandleft, r.rhandright, r.rhandtop, r.rhandbottom),
        l_hand: b(r.lhandleft, r.lhandright, r.lhandtop, r.lhandbottom),
        belt: b(r.beltleft, r.beltright, r.belttop, r.beltbottom),
        feet: b(r.feetleft, r.feetright, r.feettop, r.feetbottom),
        gloves: b(r.glovesleft, r.glovesright, r.glovestop, r.glovesbottom),
    }
}

/// d2rs-own, unverified: the grid of the spec's measured record 0 (640)
/// / 16 (800) (`panels.md` §Test vectors) when no `inventory.bin` rows
/// are set; no equipment boxes.
pub fn fallback_layout(screen: &Screen) -> InvLayout {
    let (dx, dy) = if screen.res2() { (80, 60) } else { (0, 0) };
    InvLayout {
        grid: GridRecord {
            grid_x: 10,
            grid_y: 4,
            left: 339 + dx,
            right: 626 + dx,
            top: 255 + dy,
            bottom: 368 + dy,
            cell_w: 29,
            cell_h: 29,
        },
        equip: [NO_BOX; 11],
    }
}

/// The item facts the inventory panel draws with, set by the app.
#[derive(Clone, Debug, Default)]
pub struct ItemsUi {
    /// Item-table art rows by code; empty: nothing is drawn.
    pub art: ItemArtRows,
    /// `inventory.bin` layouts by record (§9.2); `None`: [`fallback_layout`].
    pub layouts: Option<Vec<InvLayout>>,
    /// Measured frame sizes of item graphics by `invfile` (lower case);
    /// a missing one is estimated (module doc).
    pub frame_sizes: BTreeMap<String, (u32, u32)>,
    /// Shift is held (set by the host each frame): a shift-click on a
    /// belt-able grid item sends 0x63 (`inventory.md` §10 r3.4).
    pub shift: bool,
    /// Ctrl is held (set by the host each frame): a Ctrl-click on a grid
    /// item never lifts it (`inventory.md` §10 r3.3; the sell itself is
    /// the shop panel's).
    pub ctrl: bool,
    /// The GUID of the open store's NPC (`vendors.md` §7.2 rule 2; set by
    /// the shop when it opens, cleared when it closes): a Ctrl-click on a
    /// grid item sells it to this NPC (0x33, `inventory.md` §10 r3.3).
    pub store_npc: Option<u32>,
    /// The item tool tips' data (`inv_items_tip`); none: no tips.
    pub tips: Option<super::super::item_tip::ItemTips>,
    /// The inventory tables of the equip check (`items/inventory.md`
    /// §4.3, [`equip`]); none: an equipment box press does nothing.
    pub inv_tables: Option<std::sync::Arc<d2_sim::items::inventory::InvTables>>,
    /// The used item of the identify cursor (cursor state 6, `inv_items_tip`).
    pub identify: std::cell::Cell<Option<u32>>,
    /// The five tint palette indices of `inventory.md` §2 r1 (the act
    /// palette's nearest entries, [`super::super::inv_grid::tint_indices`]);
    /// `None` (no palette given): no tint is drawn.
    pub tint_colors: Option<[u8; 5]>,
    /// The hover state of §5 (`0x00487000`): one for every grid, as in
    /// the original; updated on each mouse event over an open grid
    /// ([`ItemsUi::track_hover`]) and read by the grid click (§10 r4).
    pub hover: std::cell::Cell<HoverState>,
    /// The equipment rectangles by `inventory.bin` record (§9.4 empty-slot
    /// pictures); `None`: none drawn.
    pub equip_rects: Option<Vec<super::inventory::EquipRects>>,
}

/// Item flag 0x400000: ethereal (`ui/inventory.md` §8 r4).
pub const ETHEREAL: u32 = 0x0040_0000;
/// The draw mode of an ethereal item graphic (§8 r4): 50 % alpha.
pub const ETHEREAL_MODE: u8 = 1;

/// Whether an item code fits a belt box: the type's itemtypes `beltable`
/// and a 1 x 1 size, as the server reads it (`seams/item-grids.md` §2.8,
/// `inventory-moves.md` §3.3). The server still checks the move.
pub fn fits_belt(art: &ItemArtRows, code: Option<[u8; 4]>) -> bool {
    code.and_then(|c| art.get(c)).is_some_and(|r| r.beltable)
}

/// The cube's grid, `inventory.bin` record 9 (`inventory.md` §1.3).
const CUBE_GRID: (i32, i32) = (3, 4);

/// An item's graphic: file id, footprint in cells, frame size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Art {
    pub(super) file: u32,
    pub(super) w: i32,
    pub(super) h: i32,
    pub(super) gw: i32,
    pub(super) gh: i32,
}

impl ItemsUi {
    /// Adds every art row's graphic to `files` (call before the files are
    /// handed to the art loader).
    pub fn register_files(&self, files: &mut UiFiles) {
        for row in self.art.0.values() {
            if !row.inv_file.is_empty() {
                files.add(&item_file_name(&row.inv_file));
            }
        }
    }

    /// The layout of the local player's record (`original::inventory_record`).
    pub fn layout(&self, class: Option<u32>, screen: &Screen) -> Option<InvLayout> {
        let r = super::super::original::inventory_record(class?, screen)?;
        match &self.layouts {
            Some(l) => l.get(r).copied(),
            None => Some(fallback_layout(screen)),
        }
    }

    pub(super) fn art(&self, files: &UiFiles, item: &ItemView, cell: (i32, i32)) -> Option<Art> {
        let row = self.art.get(item.code?)?;
        if row.inv_file.is_empty() {
            return None;
        }
        let file = files.id(&item_file_name(&row.inv_file))?;
        let (w, h) = (i32::from(row.inv_w.max(1)), i32::from(row.inv_h.max(1)));
        let (gw, gh) = match self.frame_sizes.get(&row.inv_file.to_ascii_lowercase()) {
            Some(&(gw, gh)) => (gw as i32, gh as i32),
            // d2rs-own, unverified: the footprint size.
            None => (w * cell.0, h * cell.1),
        };
        Some(Art { file, w, h, gw, gh })
    }

    /// The empty equipment-slot pictures (`panels.md` §9.4) of the local
    /// player's record: a body location with no item, a hand also not
    /// covered by a two-handed weapon in the other hand.
    pub fn draw_equip_backgrounds(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        class: Option<u32>,
        screen: &Screen,
        out: &mut dyn UiDrawSink,
    ) {
        use super::inventory::{body_loc, equip_backgrounds, EquipState};
        let Some(r) = class.and_then(|c| super::super::original::inventory_record(c, screen))
        else {
            return;
        };
        let Some(rects) = self.equip_rects.as_ref().and_then(|v| v.get(r)) else {
            return;
        };
        let mut eq = EquipState::default();
        for i in items::local_items(world) {
            if i.mode != mode::BODY || !(1..=10).contains(&i.body) {
                continue;
            }
            eq.occupied[usize::from(i.body)] = true;
            // d2rs-own, unverified: `0x0063D340` = 2 read as the items
            // `2handed` column (the equip check's two-handed test).
            let two = i.code.is_some_and(|c| {
                self.inv_tables
                    .as_deref()
                    .and_then(|t| t.items.iter().find(|r| r.code == c))
                    .is_some_and(|r| r.twohanded != 0)
            });
            match i.body {
                body_loc::RIGHT_HAND => eq.right_two_handed = two,
                body_loc::LEFT_HAND => eq.left_two_handed = two,
                _ => {}
            }
        }
        equip_backgrounds(rects, &eq, &|f| files.id(f), out);
    }

    /// The grid items of page 0 (§3 r1) and the equipped items (§6 r2) of
    /// the local player, item graphics drawn with the frame's top-left at
    /// the item's point (§8 r4).
    pub fn draw_panel(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        layout: &InvLayout,
        out: &mut dyn UiDrawSink,
    ) {
        for it in items::local_items(world) {
            if let Some(d) = self.item_draw(world, files, layout, &it) {
                out.push(d);
            }
        }
    }

    /// `inventory.md` §3 r2–r3, §6: per item, its tints then its graphic
    /// (`a1-panel-cube` rows 25–40: four cell boxes, the cube; four, the
    /// cap; ...), grid items and equipped items in the list order.
    pub fn draw_items(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        layout: &InvLayout,
        mouse: Point,
        out: &mut dyn UiDrawSink,
    ) {
        for it in items::local_items(world) {
            for d in self.item_tints(world, layout, mouse, &it) {
                out.push(d);
            }
            if let Some(d) = self.item_draw(world, files, layout, &it) {
                out.push(d);
            }
        }
    }

    /// One item's graphic in the panel (§3 r1, §6 r2, §8 r4).
    fn item_draw(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        layout: &InvLayout,
        it: &ItemView,
    ) -> Option<UiDraw> {
        let g = &layout.grid;
        let cell = (i32::from(g.cell_w), i32::from(g.cell_h));
        let a = self.art(files, it, cell)?;
        let top_left = match it.mode {
            mode::STORED if it.page == 0 => {
                let (x, y, _, _) = g.cell(i32::from(it.x), i32::from(it.y));
                Point::new(x, y)
            }
            mode::BODY if (1..=10).contains(&it.body) => {
                let b = layout.equip[usize::from(it.body)];
                if b.w == 0 || b.h == 0 {
                    return None;
                }
                equip_draw_point(it.body, b, cell, (a.w, a.h), false)
            }
            _ => return None,
        };
        Some(self.item_cel(world, it, a.file, top_left.x, top_left.y + a.gh))
    }

    /// An item's graphic with its frame's top-left at (`left`, `top`)
    /// (the belt box, `control-panel.md` §5 r4); nothing without art.
    pub fn draw_at(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        item: &ItemView,
        (left, top): (i32, i32),
        out: &mut dyn UiDrawSink,
    ) {
        if let Some(a) = self.art(files, item, (29, 29)) {
            out.push(self.item_cel(world, item, a.file, left, top + a.gh));
        }
    }

    /// How an item graphic is written (`ui/inventory.md` §8 r4): draw mode
    /// 1 for an ethereal item (flag 0x400000), else 5; the remap is the
    /// item's inventory colour ([`super::super::item_tip::ItemTips::inv_color`]),
    /// none without the tips or a map.
    pub fn item_look(&self, world: &ClientWorld, it: &ItemView) -> CelLook {
        let remap = self
            .tips
            .as_ref()
            .zip(items::stream(world, it.key))
            .and_then(|(t, s)| t.inv_color(s))
            .map_or(Remap::None, |(t, c)| Remap::ItemColor { t, c });
        CelLook {
            mode: if it.flags & ETHEREAL != 0 {
                ETHEREAL_MODE
            } else {
                DRAW_MODE_OPAQUE
            },
            remap,
        }
    }

    /// The cel draw `0x004F6480` of an item graphic at (x, y) with its
    /// [`Self::item_look`].
    pub(crate) fn item_cel(
        &self,
        world: &ClientWorld,
        it: &ItemView,
        file: u32,
        x: i32,
        y: i32,
    ) -> UiDraw {
        let mut d = cel(file, 0, x, y);
        if let UiDraw::Image(i) = &mut d {
            i.look = self.item_look(world, it);
        }
        d
    }

    /// The cursor item (`panels-3.md` §23 r9): its graphic with the
    /// top-left at (mx − gw / 2, my − gh / 2) (adj 0, halves rounded down).
    /// The cursor item's graphic frame size `gw` × `gh` (`panels-3.md`
    /// §23 r9), when its art is known.
    pub fn cursor_graphic_size(&self, files: &UiFiles, item: &ItemView) -> Option<(u32, u32)> {
        let a = self.art(files, item, (29, 29))?;
        Some((a.gw as u32, a.gh as u32))
    }

    pub fn draw_cursor(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        cell: (i32, i32),
        mouse: Point,
        out: &mut dyn UiDrawSink,
    ) {
        let Some(it) = items::cursor_item(world) else {
            return;
        };
        let Some(a) = self.art(files, &it, cell) else {
            return;
        };
        let x = mouse.x - (a.gw as u32 / 2) as i32;
        let y = mouse.y - (a.gh as u32 / 2) as i32;
        out.push(self.item_cel(world, &it, a.file, x, y + a.gh));
    }

    /// Left mouse down in the inventory panel: the grid click (§10) or an
    /// equipment-box click (`panels-3.md` §29).
    pub fn press(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        layout: &InvLayout,
        at: Point,
    ) -> Vec<PanelOutput> {
        let g = &layout.grid;
        if g.cell_w == 0 || g.cell_h == 0 {
            return Vec::new();
        }
        let cursor = items::cursor_item(world);
        if g.contains_mouse(at) {
            return self
                .grid_press(world, files, g, cursor.as_ref(), at, 0)
                .map(PanelOutput::Intent)
                .into_iter()
                .collect();
        }
        self.body_press(world, layout, cursor.as_ref(), at)
    }

    /// Right mouse down in the inventory panel (d2rs-own, REC-117): on a
    /// page-0 grid item with no item on the cursor, C→S 0x20 UseGridItem
    /// (`inventory-moves.md` §7.11) with the local player's point; the
    /// server decides whether the item can be used.
    pub fn use_press(
        &self,
        world: &ClientWorld,
        layout: &InvLayout,
        at: Point,
    ) -> Vec<PanelOutput> {
        let g = &layout.grid;
        if g.cell_w == 0 || g.cell_h == 0 || !g.contains_mouse(at) {
            return Vec::new();
        }
        if items::cursor_item(world).is_some() {
            return Vec::new();
        }
        let (c, r) = g.mouse_cell(at);
        let (c, r) = (c as i32, r as i32);
        let hit = items::local_items(world).into_iter().find(|i| {
            let (w, h) = self.art.get(i.code.unwrap_or([0; 4])).map_or((1, 1), |a| {
                (i32::from(a.inv_w.max(1)), i32::from(a.inv_h.max(1)))
            });
            let (x, y) = (i32::from(i.x), i32::from(i.y));
            i.mode == mode::STORED
                && i.page == 0
                && (x..x + w).contains(&c)
                && (y..y + h).contains(&r)
        });
        let (Some(it), Some((px, py))) = (hit, world.local().and_then(|p| p.position)) else {
            return Vec::new();
        };
        let m = d2_proto::client::UseGridItem {
            item: it.key.guid,
            x: u32::from(px),
            y: u32::from(py),
        };
        vec![PanelOutput::Intent(ClientIntent::from_message(&m))]
    }

    /// The hover handler `0x00487000` (§5) for a mouse event at `at`
    /// over grid `g` (page `page`): with a cursor item the cursor cell
    /// (§5 r2–r3, kept when the footprint overhangs), else the item at
    /// the mouse cell (§5 r1). Nothing outside the grid rectangle.
    pub fn track_hover(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        g: &GridRecord,
        page: u8,
        at: Point,
    ) {
        if g.cell_w == 0 || g.cell_h == 0 || !g.contains_mouse(at) {
            return;
        }
        let cell = (i32::from(g.cell_w), i32::from(g.cell_h));
        let mut h = self.hover.get();
        if let Some(cur) = items::cursor_item(world) {
            let a = self.art(files, &cur, cell);
            let (w, hh, gw, gh) = a.map_or((1, 1, cell.0, cell.1), |a| (a.w, a.h, a.gw, a.gh));
            h.with_cursor_item(grid_cursor_cell(g, at, (w, hh), (gw, gh)));
        } else {
            let (c, r) = g.mouse_cell(at);
            let (c, r) = (c as i32, r as i32);
            let under = items::local_items(world).into_iter().find(|i| {
                let (w, hh) = self.art.get(i.code.unwrap_or([0; 4])).map_or((1, 1), |a| {
                    (i32::from(a.inv_w.max(1)), i32::from(a.inv_h.max(1)))
                });
                let (x, y) = (i32::from(i.x), i32::from(i.y));
                i.mode == mode::STORED
                    && i.page == page
                    && (x..x + w).contains(&c)
                    && (y..y + hh).contains(&r)
            });
            h.without_cursor_item(under.map(|i| i.key.guid));
        }
        self.hover.set(h);
    }

    pub(super) fn grid_press(
        &self,
        world: &ClientWorld,
        files: &UiFiles,
        g: &GridRecord,
        cursor: Option<&ItemView>,
        at: Point,
        page: u8,
    ) -> Option<ClientIntent> {
        let cell = (i32::from(g.cell_w), i32::from(g.cell_h));
        let all = items::local_items(world);
        // Page-0 grid items with their footprints.
        let grid: Vec<(&ItemView, i32, i32, i32, i32)> = all
            .iter()
            .filter(|i| i.mode == mode::STORED && i.page == page)
            .map(|i| {
                let (w, h) = self.art.get(i.code.unwrap_or([0; 4])).map_or((1, 1), |r| {
                    (i32::from(r.inv_w.max(1)), i32::from(r.inv_h.max(1)))
                });
                (i, i32::from(i.x), i32::from(i.y), w, h)
            })
            .collect();
        let at_cell = |c: i32, r: i32| {
            grid.iter()
                .find(|(_, x, y, w, h)| (*x..x + w).contains(&c) && (*y..y + h).contains(&r))
                .map(|(i, ..)| *i)
        };
        // The facts of §10 r4.3 from the tables and the items' streams
        // (`seams/item-grids.md` §2.5; `inv_items_facts`).
        let tips = self.tips.as_ref().map(|t| t as &dyn facts::GridInfo);
        let stream_of = |i: &ItemView| items::stream(world, i.key);
        let cursor_stream = cursor.and_then(stream_of);
        let iref = |i: &ItemView| ItemRef {
            id: i.key.guid,
            is_cube: i.code == Some(*b"box "),
            stackable_onto: tips
                .zip(cursor_stream)
                .zip(stream_of(i))
                .is_some_and(|((t, a), b)| facts::stack_test(t, a, b)),
            book_kind: tips
                .zip(stream_of(i))
                .and_then(|(t, s)| facts::book_kind(t, s)),
            sellable: tips
                .zip(i.code)
                .is_some_and(|(t, code)| facts::GridInfo::sellable(t, code)),
            fits_belt: fits_belt(&self.art, i.code),
        };
        // The cube's grid (`inventory.bin` record 9: 3 x 4) and what lies
        // on its page 3, for the room test `0x0063B850` (§10 r4.4).
        let cube_grid = if page == 3 {
            (i32::from(g.grid_x), i32::from(g.grid_y))
        } else {
            CUBE_GRID
        };
        let cube_taken: Vec<(i32, i32, i32, i32)> = all
            .iter()
            .filter(|i| i.mode == mode::STORED && i.page == 3)
            .map(|i| {
                let (w, h) = self.art.get(i.code.unwrap_or([0; 4])).map_or((1, 1), |r| {
                    (i32::from(r.inv_w.max(1)), i32::from(r.inv_h.max(1)))
                });
                (i32::from(i.x), i32::from(i.y), w, h)
            })
            .collect();
        let cube_has_room = cursor.is_some_and(|c| {
            let size = self
                .art
                .get(c.code.unwrap_or([0; 4]))
                .map_or((1, 1), |r| (i32::from(r.inv_w), i32::from(r.inv_h)));
            facts::has_room(cube_grid, &cube_taken, size)
        });
        let (mc, mr) = g.mouse_cell(at);
        let under_view = at_cell(mc as i32, mr as i32);
        let under_mouse = under_view.map(iref);
        // §6 r5 / `inventory-moves.md` §7.19: a filler over a socketed item.
        let socket = cursor.zip(under_view).and_then(|(c, u)| {
            let tips = self.tips.as_ref()?;
            socket::socket_intent(tips, world, c, u)
        });
        // The kept cursor cell (§5 r3, `[0x00721E4C]` / `[0x00721E50]`)
        // and the overlap under the footprint. The press point is a mouse
        // event too: a footprint overhanging the right column or bottom
        // row there keeps the last valid cell.
        let mut cursor_cell = (mc, mr);
        let mut overlap: Vec<&ItemView> = Vec::new();
        // The drop cell `0x00486BD0` (§10 r4.2) and its placement test.
        let mut drop_cell = None;
        if let Some(cur) = cursor {
            let a = self.art(files, cur, cell);
            let (w, h) = a.map_or((1, 1), |a| (a.w, a.h));
            let (gw, gh) = a.map_or((cell.0, cell.1), |a| (a.gw, a.gh));
            if let Some((dc, dr)) = grid_drop_cell(g, at, (w, h), (gw, gh)) {
                let inside = dc + w <= i32::from(g.grid_x) && dr + h <= i32::from(g.grid_y);
                let free = !grid.iter().any(|(_, x, y, iw, ih)| {
                    *x < dc + w && dc < x + iw && *y < dr + h && dr < y + ih
                });
                if inside && free {
                    drop_cell = Some((dc as u32, dr as u32));
                }
            }
            self.track_hover(world, files, g, page, at);
            let (c, r) = self.hover.get().cursor_cell;
            // d2rs-own: no cell was ever set (§4 r2 tests ≥ 0).
            let fits = c >= 0 && r >= 0;
            if fits {
                cursor_cell = (c as u32, r as u32);
                let (c0, r0) = (cursor_cell.0 as i32, cursor_cell.1 as i32);
                for (i, x, y, iw, ih) in &grid {
                    if *x < c0 + w && c0 < x + iw && *y < r0 + h && r0 < y + ih {
                        overlap.push(i);
                    }
                }
            }
        }
        let ctx = ClickCtx {
            cursor_state: if cursor.is_some() {
                4
            } else if self.identify.get().is_some() {
                6
            } else {
                1
            },
            used_item: self.identify.get(),
            cursor_item: cursor.map(iref),
            cursor_scroll_kind: tips
                .zip(cursor_stream)
                .and_then(|(t, s)| facts::scroll_kind(t, s)),
            under_mouse,
            ready: true,
            own_player: true,
            own_inventory_context: true,
            // `ui/inventory.md` §4 r1: the stash grid in mode 0x0C, the
            // cube grid (page 3) in mode 0x0E.
            inventory_mode: match page {
                4 => 0x0C,
                3 => 0x0E,
                _ => 0,
            },
            page,
            shift: self.shift,
            ctrl: self.ctrl,
            store_open: self.store_npc.is_some(),
            overlap_item: overlap.first().map(|i| iref(i)),
            overlap_count: overlap.len() as u32,
            cube_under_footprint: overlap
                .iter()
                .find(|i| i.code == Some(*b"box "))
                .map(|i| iref(i)),
            drop_cell,
            swap_ok: overlap.len() == 1,
            cursor_cell,
            cube_has_room,
            cursor_can_socket: socket.is_some(),
        };
        match grid_click(&ctx).msg? {
            GridMsg::Lift { item } => Some(ClientIntent::from_message(&items::remove(item))),
            GridMsg::Place { item, x, y, page } => Some(ClientIntent::from_message(
                &items::insert(item, x, y, u32::from(page)),
            )),
            GridMsg::Swap {
                cursor,
                under,
                x,
                y,
            } => Some(ClientIntent::from_message(&SwapCursorBufferItem {
                cursor,
                target: under,
                x,
                y,
            })),
            GridMsg::ToBelt { item } => Some(ClientIntent::from_message(
                &d2_proto::client::ItemToBeltShift { item },
            )),
            GridMsg::TargetUsed { target, used } => {
                Some(ClientIntent::from_message(&self.target_used(target, used)))
            }
            GridMsg::Socket { .. } => socket.map(|m| ClientIntent::from_message(&m)),
            GridMsg::Stack { cursor, under } => {
                Some(ClientIntent::from_message(&d2_proto::client::StackItems {
                    src: cursor,
                    dst: under,
                }))
            }
            GridMsg::ScrollBook { cursor, under } => Some(ClientIntent::from_message(
                &d2_proto::client::ScrollToBook {
                    scroll: cursor,
                    book: under,
                },
            )),
            GridMsg::ToCube { item, cube } => {
                Some(ClientIntent::from_message(&d2_proto::client::ItemToCube {
                    item,
                    cube,
                }))
            }
            // `inventory.md` §10 r3.3: 0x33 with the open store's NPC, the
            // item's mode and the price the client shows (not read by the
            // server, `vendors.md` §7.2; d2rs-own: 0).
            GridMsg::Sell { item } => {
                let npc = self.store_npc?;
                let item_mode = all
                    .iter()
                    .find(|i| i.key.guid == item)
                    .map_or(0, |i| u16::from(i.mode));
                Some(ClientIntent::from_message(&d2_proto::client::SellItem {
                    npc,
                    item,
                    item_mode,
                    client_price: 0,
                }))
            }
            _ => None,
        }
    }
}

/// The cursor cell of a w × h cursor item with a gw × gh graphic over
/// grid `g` (§5 r3, [`CellGrid::cursor_cell`]); `None`: the handler
/// returns without change (the footprint would pass the grid's right or
/// bottom edge), the caller keeps its previous cell.
pub fn grid_cursor_cell(
    g: &GridRecord,
    at: Point,
    (w, h): (i32, i32),
    (gw, gh): (i32, i32),
) -> Option<(i32, i32)> {
    let cg = CellGrid::new(
        WidgetId(0),
        Point::new(g.left, g.top),
        u16::from(g.grid_x),
        u16::from(g.grid_y),
        u16::from(g.cell_w),
        u16::from(g.cell_h),
    )
    .ok()?;
    cg.cursor_cell(at, w as u16, h as u16, gw as u32, gh as u32)
}

/// The drop cell (`0x00486BD0`, `ui/inventory.md` §10 r4.2): the cursor
/// cell without the overflow returns.
pub fn grid_drop_cell(
    g: &GridRecord,
    at: Point,
    (w, h): (i32, i32),
    (gw, gh): (i32, i32),
) -> Option<(i32, i32)> {
    let cg = CellGrid::new(
        WidgetId(0),
        Point::new(g.left, g.top),
        u16::from(g.grid_x),
        u16::from(g.grid_y),
        u16::from(g.cell_w),
        u16::from(g.cell_h),
    )
    .ok()?;
    Some(cg.drop_cell(at, w as u16, h as u16, gw as u32, gh as u32))
}

/// The equipment box under `at` (`panels-3.md` §29 r1: boxes 1–10, first
/// hit in location order).
fn equip_loc(layout: &InvLayout, at: Point) -> Option<u8> {
    (1u8..=10).find(|&l| {
        let b = layout.equip[usize::from(l)];
        b.w > 0
            && b.h > 0
            && (b.left..b.left + b.w).contains(&at.x)
            && (b.top..b.top + b.h).contains(&at.y)
    })
}

impl ItemsUi {
    /// `panels-3.md` §29 r1 on a box hit: the socket test (r1.2, not under
    /// the use cursor), then the location's handler ([`equip::body_press`]).
    fn body_press(
        &self,
        world: &ClientWorld,
        layout: &InvLayout,
        cursor: Option<&ItemView>,
        at: Point,
    ) -> Vec<PanelOutput> {
        let Some(loc) = equip_loc(layout, at) else {
            return Vec::new();
        };
        let used = self.identify.get();
        if used.is_none() {
            if let (Some(tips), Some(c)) = (self.tips.as_ref(), cursor) {
                let all = items::local_items(world);
                let worn = all.iter().find(|i| i.mode == mode::BODY && i.body == loc);
                if let Some(m) = worn.and_then(|t| socket::socket_intent(tips, world, c, t)) {
                    return vec![PanelOutput::Intent(ClientIntent::from_message(&m))];
                }
            }
        }
        let (Some(t), Some(tips)) = (self.inv_tables.as_deref(), self.tips.as_ref()) else {
            return Vec::new();
        };
        let lookup = tips.tables();
        let decode = |s: &[u8]| tips.bits(s);
        let Some((view, inv, _)) = equip::ClientInv::build(world, t, &lookup, &decode) else {
            return Vec::new();
        };
        let state = match used {
            Some(u) => equip::Cursor::Use(u),
            None if cursor.is_some() => equip::Cursor::Item,
            None => equip::Cursor::Plain,
        };
        let p = equip::body_press(&view, &inv, loc, state);
        if p.end_use {
            self.identify.set(None);
        }
        p.out
    }
}

#[path = "inv_items_equip.rs"]
pub mod equip;
#[path = "inv_items_facts.rs"]
mod facts;
#[path = "inv_items_repair.rs"]
mod repair;
#[path = "inv_items_socket.rs"]
pub mod socket;
#[path = "inv_items_tip.rs"]
mod tip;
pub use tip::is_identify;

#[cfg(test)]
#[path = "inv_items_tests.rs"]
pub(crate) mod tests;
