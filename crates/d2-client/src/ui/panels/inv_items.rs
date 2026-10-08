// Spec: specs/ui/inventory.md (§1 r1–r4, §3 r1, §5 r3, §6 r1–r2, §8 r2–r4, §10 r3–r4), specs/ui/panels.md (§9.2, §9.6, §9.7), specs/ui/panels-3.md (§23 r9)
//! The local player's items in the inventory panel (ui 1): the page-0
//! grid items (`inventory.md` §3 r1) and the equipped items (§6 r2)
//! drawn with their inventory graphic (§8), the cursor item at the mouse
//! (`panels-3.md` §23 r9), and a left press on the grid (§10) or on an
//! equipment box sent as a C→S item intent through the root's outbox.
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
//! - the equipment click (`0x00490780` family is not specified,
//!   `panels.md` §15): inside a box, cursor item + empty → 0x1A, cursor
//!   item + occupied → 0x1D, no cursor item + occupied → 0x1C;
//! - the drop cell `0x00486BD0` (not specified) is the cursor cell;
//! - tints (§3 r2–r3, §6 r4), sockets, ethereal draw mode and the
//!   item's colour remap are not drawn ([`super::super::ImageRequest`]
//!   has no draw mode or remap field).

use std::collections::BTreeMap;

use d2_proto::client::{RemoveBodyItem, SwapCursorBufferItem, SwapCursorWithBody};

use super::super::draw::UiDrawSink;
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
    /// The item tool tips' data (`inv_items_tip`); none: no tips.
    pub tips: Option<super::super::item_tip::ItemTips>,
    /// The used item of the identify cursor (cursor state 6, `inv_items_tip`).
    pub identify: std::cell::Cell<Option<u32>>,
    /// The hover state of §5 (`0x00487000`): one for every grid, as in
    /// the original; updated on each mouse event over an open grid
    /// ([`ItemsUi::track_hover`]) and read by the grid click (§10 r4).
    pub hover: std::cell::Cell<HoverState>,
}

/// d2rs-own, unverified: whether an item code is a belt-able potion
/// (`hp1`–`hp5`, `mp1`–`mp5`, `rvs`, `rvl`, `vps`, `yps`, `wms`, the
/// throwing potions `gps`/`gpm`/`gpl`/`ops`/`opm`/`opl`); the server
/// still checks the move (`inventory-moves.md` §7.24).
pub fn fits_belt(code: Option<[u8; 4]>) -> bool {
    let Some(c) = code else {
        return false;
    };
    matches!(
        &c[..3],
        b"hp1"
            | b"hp2"
            | b"hp3"
            | b"hp4"
            | b"hp5"
            | b"mp1"
            | b"mp2"
            | b"mp3"
            | b"mp4"
            | b"mp5"
            | b"rvs"
            | b"rvl"
            | b"vps"
            | b"yps"
            | b"wms"
            | b"gps"
            | b"gpm"
            | b"gpl"
            | b"ops"
            | b"opm"
            | b"opl"
    )
}

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
        let g = &layout.grid;
        let cell = (i32::from(g.cell_w), i32::from(g.cell_h));
        for it in items::local_items(world) {
            let Some(a) = self.art(files, &it, cell) else {
                continue;
            };
            let top_left = match it.mode {
                mode::STORED if it.page == 0 => {
                    let (x, y, _, _) = g.cell(i32::from(it.x), i32::from(it.y));
                    Point::new(x, y)
                }
                mode::BODY if (1..=10).contains(&it.body) => {
                    let b = layout.equip[usize::from(it.body)];
                    if b.w == 0 || b.h == 0 {
                        continue;
                    }
                    equip_draw_point(it.body, b, cell, (a.w, a.h), false)
                }
                _ => continue,
            };
            out.push(cel(a.file, 0, top_left.x, top_left.y + a.gh));
        }
    }

    /// An item's graphic with its frame's top-left at (`left`, `top`)
    /// (the belt box, `control-panel.md` §5 r4); nothing without art.
    pub fn draw_at(
        &self,
        files: &UiFiles,
        item: &ItemView,
        (left, top): (i32, i32),
        out: &mut dyn UiDrawSink,
    ) {
        if let Some(a) = self.art(files, item, (29, 29)) {
            out.push(cel(a.file, 0, left, top + a.gh));
        }
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
        out.push(cel(a.file, 0, x, y + a.gh));
    }

    /// Left mouse down in the inventory panel: the grid click (§10) or an
    /// equipment-box click (d2rs-own, module doc). The intents only.
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
        let all = items::local_items(world);
        let cursor = items::cursor_item(world);
        let intent = if g.contains_mouse(at) {
            self.grid_press(world, files, g, cursor.as_ref(), at, 0)
        } else {
            self.equip_socket(world, layout, &all, cursor.as_ref(), at)
                .or_else(|| equip_press(layout, &all, cursor.as_ref(), at))
        };
        intent.map(PanelOutput::Intent).into_iter().collect()
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
        let iref = |i: &ItemView| ItemRef {
            id: i.key.guid,
            is_cube: i.code == Some(*b"box "),
            stackable_onto: false,
            book_kind: None,
            sellable: false,
            fits_belt: fits_belt(i.code),
        };
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
        let mut fits = true;
        if let Some(cur) = cursor {
            let a = self.art(files, cur, cell);
            let (w, h) = a.map_or((1, 1), |a| (a.w, a.h));
            self.track_hover(world, files, g, page, at);
            let (c, r) = self.hover.get().cursor_cell;
            // d2rs-own: no cell was ever set (§4 r2 tests ≥ 0).
            fits = c >= 0 && r >= 0;
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
            cursor_scroll_kind: None,
            under_mouse,
            ready: true,
            own_player: true,
            own_inventory_context: true,
            inventory_mode: if page == 4 { 0x0C } else { 0 },
            page,
            shift: self.shift,
            ctrl: false,
            store_open: false,
            overlap_item: overlap.first().map(|i| iref(i)),
            overlap_count: overlap.len() as u32,
            cube_under_footprint: overlap
                .iter()
                .find(|i| i.code == Some(*b"box "))
                .map(|i| iref(i)),
            // d2rs-own: the drop cell `0x00486BD0` is the cursor cell.
            drop_cell: fits.then_some(cursor_cell),
            swap_ok: overlap.len() == 1,
            cursor_cell,
            cube_has_room: false,
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
            // Not produced by the facts above (no stack / scroll
            // / cube / shop facts in the preview).
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

/// d2rs-own, unverified: an equipment-box press (module doc).
/// The equipment box under `at`.
fn equip_loc(layout: &InvLayout, at: Point) -> Option<u8> {
    (1u8..=10).find(|&l| {
        let b = layout.equip[usize::from(l)];
        b.w > 0
            && b.h > 0
            && (b.left..b.left + b.w).contains(&at.x)
            && (b.top..b.top + b.h).contains(&at.y)
    })
}

fn equip_press(
    layout: &InvLayout,
    all: &[ItemView],
    cursor: Option<&ItemView>,
    at: Point,
) -> Option<ClientIntent> {
    let loc = equip_loc(layout, at)?;
    let worn = all.iter().find(|i| i.mode == mode::BODY && i.body == loc);
    match (cursor, worn) {
        (Some(c), None) => Some(ClientIntent::from_message(&items::equip(c.key.guid, loc))),
        (Some(c), Some(_)) => Some(ClientIntent::from_message(&SwapCursorWithBody {
            item: c.key.guid,
            bodyloc: loc,
        })),
        (None, Some(_)) => Some(ClientIntent::from_message(&RemoveBodyItem {
            bodyloc: u16::from(loc),
        })),
        (None, None) => None,
    }
}

impl ItemsUi {
    /// A filler on the cursor over a worn socketed item: C→S 0x28
    /// (`inventory.md` §6 r5 counts the equipment boxes too).
    fn equip_socket(
        &self,
        world: &ClientWorld,
        layout: &InvLayout,
        all: &[ItemView],
        cursor: Option<&ItemView>,
        at: Point,
    ) -> Option<ClientIntent> {
        let (tips, c) = (self.tips.as_ref()?, cursor?);
        let loc = equip_loc(layout, at)?;
        let worn = all.iter().find(|i| i.mode == mode::BODY && i.body == loc)?;
        let m = socket::socket_intent(tips, world, c, worn)?;
        Some(ClientIntent::from_message(&m))
    }
}

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
