// Spec: specs/ui/panels-2.md (§14 r4, r11–r13), specs/ui/menus.md (§4), specs/world/vendors.md (§4 step 3, §7, §8); preview glue: docs/handoff/q-vendor-items.md
//! The NPC shop (ui 0x0C) installed in the original UI: the store the
//! server showed (S→C 0x9C action 0x0B, one per item, `vendors.md` §4
//! step 3) drawn on the shop's art with the tabs and the action buttons,
//! and the clicks that leave as C→S 0x32 buy, 0x33 sell and 0x35 repair
//! all (`menus.md` §4: [`ShopTx`]). The client decides nothing: the
//! server checks the trade and answers with S→C 0x2A and the item and
//! gold messages (`vendors.md` §7–§9).
//!
//! The shop opens by itself when a trade's store items arrive
//! ([`OriginalUi::shop_poll`], the same way as the hire list opens on
//! S→C 0x4F) for the trader nearest to the local player, or when the NPC
//! menu calls [`OriginalUi::open_shop`]. It closes with the UI state; the
//! close sends C→S 0x30 (`panels-2.md` §14 r5).
//!
//! d2rs-own, unverified (preview fills):
//! - the store grid (10 × 10 cells of 29 px) at (`sx` + 15, `H` + `sy` −
//!   400); the NPC's own `inventory.bin` record is not read;
//! - a left click on a store item buys it at once (the confirm dialog of
//!   `menus.md` §4.4 is not drawn); the right click is the spec's quick
//!   buy (§4.5), a left click with a cursor item on the grid sells it;
//! - the buy price shown is published by the server host
//!   ([`ShopPrices`]); the original client computes it from its tables;
//! - the repair button (frame 6) toggles a mode; the next click on one of
//!   the player's items sends C→S 0x35 for it (no confirm dialog, REC-231);
//! - the action button frames are 45 × 44 hit areas (`panels-2.md` §14
//!   r13); the Buy / Sell buttons only press and release;
//! - the time of the transaction rules is the bridge frame × 40 ms (+ 60 s).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use d2_proto::client::TerminateEntityChat;

use super::{OriginalUi, SharedRef};
use crate::bridge::items::{self, ItemView};
use crate::bridge::world::{ClientWorld, MONSTER};
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::inv_grid::GridRecord;
use crate::ui::panel::{ClientIntent, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::shop::{
    shop_button_records, shop_start_page, shop_tabs, ClickArgs, ClickEnv, ClickOutcome, ItemFacts,
    SendFacts, ShopButton, ShopEffect, ShopPanel, ShopTx, TxKind, DEFAULT_TABS, IDENTIFY_CLASSES,
    REPAIR_CLASSES, SELL_CLASSES, UI_SHOP,
};
use crate::ui::panels::{cel, text, utf16, PanelOutput, TextMeasure};
use crate::ui::root::UiRoot;
use crate::ui::states::id;
use crate::ui::PointerButton;

/// Store item prices by item GUID, written by the server host when it
/// shows the store and read by the shop panel (d2rs-own, unverified: the
/// original client computes the price from its own tables). Cloning
/// shares the board.
#[derive(Clone, Debug, Default)]
pub struct ShopPrices(Arc<Mutex<BTreeMap<u32, u32>>>);

impl ShopPrices {
    pub fn set(&self, guid: u32, price: u32) {
        if let Ok(mut m) = self.0.lock() {
            m.insert(guid, price);
        }
    }

    pub fn get(&self, guid: u32) -> Option<u32> {
        self.0.lock().ok()?.get(&guid).copied()
    }
}

/// The key the repair-all total is published under: item GUID 0, the
/// repair-all message's item (d2rs-own, unverified).
pub const REPAIR_ALL_KEY: u32 = 0;

impl PartialEq for ShopPrices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ShopPrices {}

/// The trade the shop panel is up for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopOpen {
    pub npc_guid: u32,
    pub npc_class: u32,
}

/// The shop panel's state.
#[derive(Debug, Default)]
pub struct ShopState {
    pub open: Option<ShopOpen>,
    tx: ShopTx,
    /// The current store page (`[0x007BCC04]`).
    page: u8,
    /// `ClientWorld::store_serial` seen by the last poll.
    serial: u32,
    /// Store records with a smaller `store_seq` belong to a closed trade.
    floor: u32,
    /// The pressed action button (state 1), by index.
    pressed: Option<usize>,
    prices: ShopPrices,
    /// The window is a gamble window (the menu chose Gamble).
    gamble: bool,
    /// The repair button is down: the next click on one of the player's
    /// items repairs it (d2rs-own, unverified; REC-231).
    repair_mode: bool,
}

impl ShopState {
    /// The single-item repair button is down.
    pub fn repair_mode(&self) -> bool {
        self.repair_mode
    }

    /// The window is a gamble window.
    pub fn gamble(&self) -> bool {
        self.gamble
    }

    /// The price the host published for an item.
    pub fn price(&self, guid: u32) -> Option<u32> {
        self.prices.get(guid)
    }
}

pub type SharedShop = Rc<RefCell<ShopState>>;

/// Cells of the store grid, columns × rows.
const GRID: (u8, u8) = (10, 10);
const CELL: u8 = 29;
/// The action button hit width and height (`panels-2.md` §14 r13).
const BUTTON_W: i32 = 45;

/// The classes that trade (shop button table, repair, identify and sell
/// classes).
fn trades(class: u32) -> bool {
    !shop_button_records(class).is_empty()
        || REPAIR_CLASSES.contains(&class)
        || SELL_CLASSES.contains(&class)
        || IDENTIFY_CLASSES.contains(&class)
}

fn grid_record(sh: &super::Shared) -> GridRecord {
    let s = sh.config.screen;
    let left = s.sx() + 15;
    let top = s.h + s.sy() - 400;
    GridRecord {
        grid_x: GRID.0,
        grid_y: GRID.1,
        left,
        right: left + i32::from(GRID.0) * i32::from(CELL),
        top,
        bottom: top + i32::from(GRID.1) * i32::from(CELL),
        cell_w: CELL,
        cell_h: CELL,
    }
}

/// The footprint of a store item in cells (the art row's, else 1 × 1).
fn footprint(sh: &super::Shared, it: &ItemView) -> (i32, i32) {
    it.code
        .and_then(|c| sh.items.art.get(c))
        .map_or((1, 1), |r| {
            (i32::from(r.inv_w.max(1)), i32::from(r.inv_h.max(1)))
        })
}

/// The store items of the open trade on `page`, packed row by row into
/// the grid in arrival order (d2rs-own, unverified, REC-162: the preview
/// has no NPC grid model, so the server's positions are not used).
fn page_items(sh: &super::Shared, world: &ClientWorld, st: &ShopState, page: u8) -> Vec<ItemView> {
    let mut list: Vec<ItemView> = items::store_items(world)
        .into_iter()
        .filter(|i| i.store_seq > st.floor && i.page == page)
        .collect();
    list.sort_by_key(|i| i.store_seq);
    let (cols, rows) = (i32::from(GRID.0), i32::from(GRID.1));
    let mut used = [[false; GRID.0 as usize]; GRID.1 as usize];
    for it in &mut list {
        let (w, h) = footprint(sh, it);
        let spot = (0..rows)
            .flat_map(|y| (0..cols).map(move |x| (x, y)))
            .find(|&(x, y)| {
                x + w <= cols
                    && y + h <= rows
                    && (y..y + h).all(|r| (x..x + w).all(|c| !used[r as usize][c as usize]))
            });
        let Some((x, y)) = spot else {
            // No room: parked off the grid, not drawn or hit.
            it.x = u16::MAX;
            it.y = u16::MAX;
            continue;
        };
        for r in y..y + h {
            for c in x..x + w {
                used[r as usize][c as usize] = true;
            }
        }
        it.x = x as u16;
        it.y = y as u16;
    }
    list.retain(|i| i.x < u16::from(GRID.0));
    list
}

/// The store item whose footprint holds `at`.
fn item_at(sh: &super::Shared, g: &GridRecord, list: &[ItemView], at: Point) -> Option<ItemView> {
    if !g.contains_mouse(at) {
        return None;
    }
    let (c, r) = g.mouse_cell(at);
    let (c, r) = (c as i32, r as i32);
    list.iter()
        .find(|i| {
            let (w, h) = footprint(sh, i);
            let (x, y) = (i32::from(i.x), i32::from(i.y));
            (x..x + w).contains(&c) && (y..y + h).contains(&r)
        })
        .cloned()
}

/// The tab records for the page counts.
fn tabs_of(world: &ClientWorld, st: &ShopState) -> ([(bool, bool); 4], u8) {
    let mut counts = [0u32; 5];
    for i in items::store_items(world)
        .iter()
        .filter(|i| i.store_seq > st.floor)
    {
        if let Some(c) = counts.get_mut(usize::from(i.page)) {
            *c += 1;
        }
    }
    shop_tabs(st.page, counts, 3)
}

struct NoMeasure;

impl TextMeasure for NoMeasure {
    fn width(&self, _: u16, _: &[u16]) -> Option<i32> {
        None
    }
}

/// The shop adapter (ui 0x0C, left slot).
pub struct ShopUi {
    pub(super) sh: SharedRef,
    pub(super) st: SharedShop,
}

impl ShopUi {
    fn panel(&self, world: &ClientWorld, st: &ShopState, open: ShopOpen) -> (ShopPanel, u8) {
        let (tabs, page) = tabs_of(world, st);
        let mut t = DEFAULT_TABS;
        for (rec, (active, visible)) in t.iter_mut().zip(tabs) {
            rec.active = active;
            rec.visible = visible;
        }
        let buttons = shop_button_records(open.npc_class)
            .iter()
            .enumerate()
            .map(|(i, b)| ShopButton {
                state: u16::from(st.pressed == Some(i)),
                base: b.base_frame,
            })
            .collect();
        (
            ShopPanel {
                tabs: t,
                buttons,
                captions_hidden: false,
                npc_guid: open.npc_guid,
            },
            page,
        )
    }

    /// The effects of a transaction click: the sends leave through the
    /// shared outputs.
    fn apply(sh: &mut super::Shared, effects: Vec<ShopEffect>) {
        for e in effects {
            match e {
                ShopEffect::Send(i) => sh.outputs.push(PanelOutput::Intent(i)),
                ShopEffect::Sound(_) => sh.outputs.push(PanelOutput::ClickSound),
                // d2rs-own: no waiting note, no confirm dialog, no cancel.
                ShopEffect::WaitingNote | ShopEffect::Confirm(_) | ShopEffect::Cancel => {}
            }
        }
    }

    fn click(
        sh: &mut super::Shared,
        st: &mut ShopState,
        open: ShopOpen,
        world: &ClientWorld,
        item: Option<(&ItemView, bool)>,
        repair_all: bool,
        repair_one: bool,
    ) {
        let (guid, class, mode, a1) = match item {
            Some((it, player)) => (
                it.key.guid,
                it.code.map_or(0, u32::from_le_bytes),
                u16::from(it.mode),
                u8::from(player),
            ),
            None => (0, 0, 0, 0),
        };
        let args = ClickArgs {
            npc_class: Some(open.npc_class),
            npc_guid: open.npc_guid,
            item: ItemFacts {
                guid,
                class,
                at_location_4: false,
                // d2rs-own: the server checks what may be sold.
                sellable: true,
                repairable: repair_one,
                type_ok_16: false,
            },
            a1,
            a4: 0,
            quick: true,
            repair_all,
            // The first transaction is never within 500 ms of the unset
            // last send (the original's tick count is large).
            now: (world.frames as u32).wrapping_mul(40).wrapping_add(60_000),
            send_facts: SendFacts {
                item_found: true,
                item_mode: mode,
                durability: 0,
                item_flag_1a5: false,
                item_is_cursor: a1 == 1,
            },
        };
        let prices = st.prices.clone();
        let price = move |_t: u8| prices.get(guid).unwrap_or(0);
        let env = ClickEnv {
            cursor_item: items::cursor_item(world).is_some(),
            gamble_shop: st.gamble,
            repair_all_button_on: true,
            repair_button_on: repair_one,
            repair_all_price: st.prices.get(REPAIR_ALL_KEY).unwrap_or(0),
            price: &price,
        };
        if let ClickOutcome::Effects(e) = st.tx.click(&args, &env) {
            // d2rs-own: no confirm dialog; a repair is confirmed at once.
            let confirm = e.contains(&ShopEffect::Confirm(TxKind::Repair));
            Self::apply(sh, e);
            if confirm {
                let e = st.tx.send_with(0, args.now, &args.send_facts);
                Self::apply(sh, e);
            }
        }
    }

    /// The repair button is down and the player clicked one of their
    /// items: C→S 0x35 for it. Returns whether the click was taken.
    pub(super) fn repair_click(
        sh: &mut super::Shared,
        st: &mut ShopState,
        world: &ClientWorld,
        item: Option<&ItemView>,
    ) -> bool {
        let Some(open) = st.open.filter(|_| st.repair_mode) else {
            return false;
        };
        if let Some(it) = item {
            Self::click(sh, st, open, world, Some((it, true)), false, true);
        }
        true
    }
}

impl Panel for ShopUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_SHOP))
    }

    /// The left half above the control panel (`panels.md` §4.4).
    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, (s.w / 2 + 1) as u16, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let st = self.st.borrow();
        let Some(open) = st.open else {
            return;
        };
        let (panel, page) = self.panel(ctx.world, &st, open);
        let no = NoMeasure;
        let measure: &dyn TextMeasure = match &sh.fonts {
            Some(f) => f,
            None => &no,
        };
        panel.draw(&sh.tables, &sh.env(), ctx.strings, measure, out);
        let s = sh.config.screen;
        if let Some(f) = sh.tables.files.id("panel\\buysellbtn") {
            for (frame, x, y) in panel.button_cels(&s) {
                out.push(cel(f, frame, x, y));
            }
        }
        let g = grid_record(&sh);
        let list = page_items(&sh, ctx.world, &st, page);
        for it in &list {
            let (x, y, _, _) = g.cell(i32::from(it.x), i32::from(it.y));
            sh.items.draw_at(&sh.tables.files, it, (x, y), out);
        }
        // The repair-all total above the button bar (d2rs-own).
        if let (true, Some(_)) = (REPAIR_CLASSES.contains(&open.npc_class), &sh.fonts) {
            if let Some(total) = st.prices.get(REPAIR_ALL_KEY).filter(|&t| t > 0) {
                out.push(text(
                    utf16(&total.to_string()),
                    s.sx() + 20,
                    s.h + s.sy() - 120,
                    crate::ui::panels::shop::FONT16,
                    4,
                ));
            }
        }
        // The hovered item's price.
        if let (Some(it), Some(_)) = (item_at(&sh, &g, &list, sh.mouse), &sh.fonts) {
            if let Some(price) = st.prices.get(it.key.guid) {
                out.push(text(
                    utf16(&price.to_string()),
                    sh.mouse.x + 14,
                    sh.mouse.y + 14,
                    crate::ui::panels::shop::FONT16,
                    4,
                ));
            }
        }
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        let (press, button, at) = match e {
            UiEvent::Press { button, at } => (true, button, at),
            UiEvent::Release { button, at } => (false, button, at),
            _ => return UiResponse::Ignored,
        };
        let mut sh = self.sh.borrow_mut();
        let mut st = self.st.borrow_mut();
        let Some(open) = st.open else {
            return UiResponse::Ignored;
        };
        if !press {
            // The buttons release where they were pressed.
            st.pressed = None;
            return UiResponse::Consumed;
        }
        let s = sh.config.screen;
        let (panel, page) = self.panel(ctx.world, &st, open);
        // Tab row (`panels-2.md` §14 r13).
        if at.y >= s.h + s.sy() - 479
            && at.y <= s.h + s.sy() - 449
            && at.x >= s.sx()
            && at.x < s.w / 2
        {
            let t = [80, 160, 240]
                .iter()
                .position(|&r| at.x <= s.sx() + r)
                .unwrap_or(3);
            let (visible, active) = (panel.tabs[t].visible, panel.tabs[t].active);
            if visible && !active {
                st.page = t as u8;
                sh.outputs.push(PanelOutput::ClickSound);
            }
            return UiResponse::Consumed;
        }
        // Button bar (strict bounds, `X[3][i]` without the draw's − 1).
        let (lo, hi) = (s.h + s.sy() - 109, s.h + s.sy() - 65);
        if at.y > lo && at.y < hi {
            let n = panel.buttons.len();
            if n > 0 && n <= 4 {
                let xs = crate::ui::panels::shop::BUTTON_X[n - 1];
                if let Some(i) =
                    (0..n).find(|&i| at.x > s.sx() + xs[i] && at.x < s.sx() + xs[i] + BUTTON_W)
                {
                    let rec = shop_button_records(open.npc_class)[i];
                    if rec.enabled {
                        sh.outputs.push(PanelOutput::ClickSound);
                        st.pressed = Some(i);
                        match rec.base_frame {
                            // Close (record 3 of the other traders).
                            10 => sh.outputs.push(PanelOutput::SetUi {
                                ui: id::NPC_SHOP,
                                mode: 1,
                                jump: false,
                            }),
                            // Repair one item: the button toggles; the
                            // next click on the player's item sends it.
                            6 => st.repair_mode = !st.repair_mode,
                            // Repair all.
                            18 => Self::click(&mut sh, &mut st, open, ctx.world, None, true, false),
                            _ => {}
                        }
                    }
                    return UiResponse::Consumed;
                }
            }
        }
        let g = grid_record(&sh);
        if !g.contains_mouse(at) {
            return UiResponse::Consumed;
        }
        let list = page_items(&sh, ctx.world, &st, page);
        let under = item_at(&sh, &g, &list, at);
        let cursor = items::cursor_item(ctx.world);
        match (button, cursor.as_ref(), under.as_ref()) {
            // A cursor item dropped on the store grid: sell it.
            (PointerButton::Left, Some(c), _) => Self::click(
                &mut sh,
                &mut st,
                open,
                ctx.world,
                Some((c, true)),
                false,
                false,
            ),
            // Right click (quick buy, §4.5) and, d2rs-own, left click.
            (PointerButton::Left | PointerButton::Right, None, Some(it)) => Self::click(
                &mut sh,
                &mut st,
                open,
                ctx.world,
                Some((it, false)),
                false,
                false,
            ),
            _ => {}
        }
        UiResponse::Consumed
    }
}

impl OriginalUi {
    /// Opens the shop for the NPC with this GUID and class (the NPC menu's
    /// Trade option, `menus.md` §3.1): the shop and inventory panels come
    /// up; the store items arrive as S→C 0x9C action 0x0B.
    pub fn open_shop(&mut self, npc_guid: u32, npc_class: u32) {
        let gamble = self.npcm.borrow().gamble;
        {
            let mut st = self.shop.borrow_mut();
            st.gamble = gamble;
            st.open = Some(ShopOpen {
                npc_guid,
                npc_class,
            });
            st.tx = ShopTx::default();
            st.pressed = None;
            st.repair_mode = false;
            // The start page of the class (`panels-2.md` §14 r12).
            st.page = shop_start_page(npc_class).0;
        }
        // d2rs-own: a refused open (the gate) leaves the shop state up
        // and `shop_poll` closes it again.
        let _ = self.set_ui(u32::from(id::NPC_SHOP), 0, false);
        let _ = self.set_ui(u32::from(id::INVENTORY), 0, false);
    }

    /// The prices the server host publishes ([`ShopPrices`]).
    pub fn set_shop_prices(&mut self, prices: ShopPrices) {
        self.shop.borrow_mut().prices = prices;
    }

    /// The shop state (tests).
    pub fn shop_state(&self) -> std::cell::Ref<'_, ShopState> {
        self.shop.borrow()
    }

    /// Per frame: opens the shop when a trade's store items arrived (for
    /// the trader nearest to the local player, d2rs-own) and ends the
    /// interaction (C→S 0x30) when the shop's UI state went off.
    pub fn shop_poll(&mut self, world: &ClientWorld, root: &mut UiRoot) {
        let (open, serial) = {
            let st = self.shop.borrow();
            (st.open, st.serial)
        };
        if world.store_serial != serial {
            self.shop.borrow_mut().serial = world.store_serial;
            if open.is_none() {
                if let Some((guid, class)) = nearest_trader(world) {
                    self.open_shop(guid, class);
                }
            }
        }
        let open = self.shop.borrow().open;
        if let Some(o) = open.filter(|_| !self.is_open(id::NPC_SHOP)) {
            let mut st = self.shop.borrow_mut();
            st.open = None;
            st.repair_mode = false;
            st.floor = world.store_serial;
            root.queue_intent(ClientIntent::from_message(&TerminateEntityChat {
                id: o.npc_guid,
            }));
        }
        self.sync_root(root);
    }
}

/// The trader nearest to the local player: a monster unit of a trading
/// class.
fn nearest_trader(world: &ClientWorld) -> Option<(u32, u32)> {
    let (px, py) = world.local().and_then(|u| u.position).unwrap_or((0, 0));
    let d = |u: &crate::bridge::world::ClientUnit| {
        let (x, y) = u.position.unwrap_or((u16::MAX, u16::MAX));
        (i64::from(x) - i64::from(px)).pow(2) + (i64::from(y) - i64::from(py)).pow(2)
    };
    world
        .units
        .values()
        .filter(|u| u.key.unit_type == MONSTER && trades(u.class))
        .min_by_key(|u| d(u))
        .map(|u| (u.key.guid, u.class))
}
