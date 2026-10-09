// Spec: specs/ui/panels-2.md (§14 r4, r11–r13), specs/ui/menus.md (§4), specs/world/vendors.md (§4 step 3, §7, §8); preview glue: docs/handoff/q-vendor-items.md
//! The NPC shop (ui 0x0C) installed in the original UI: the store the
//! server showed (S→C 0x9C action 0x0B, one per item, `vendors.md` §4
//! step 3) drawn on the shop's art with the tabs and the action buttons,
//! and the clicks that leave as C→S 0x32 buy, 0x33 sell and 0x35 repair
//! all (`menus.md` §4: [`ShopTx`]). The client decides nothing: the
//! server checks the trade and answers with S→C 0x2A and the item and
//! gold messages (`vendors.md` §7–§9).
//!
//! The shop opens when the NPC menu's Trade / Gamble choice is made
//! (REC-277: the next poll calls [`OriginalUi::open_shop`]) or, d2rs-own,
//! when a trade's store items arrive for the trader nearest to the local
//! player. It is the full-slot ui 0x0C (`panels.md` §4.1): ui 1 stays off
//! and the root draws the inventory family under it (§5 step 5); ui 8
//! stays on. Its close is the interaction's end (`panels-2.md` §14.9:
//! C→S 0x30, `SetUIState(8, off)`).
//!
//! Clicks go through the spec transaction rules with the callers of
//! `menus.md` §4.5 ([`caller_args`]): a right click on a store item is the
//! quick buy (`0x00491AD0`, Shift = `MK_SHIFT`), a left click goes through
//! the confirm dialog (`0x0048FFE0`, [`confirm_box`], drawn and answered
//! by the NPC menu panel), a cursor item dropped on the store grid is the
//! quick sell (`0x00491D20`); with the repair button down a click on one
//! of the player's items is the confirmed repair, whose 0x35 carries the
//! item's stat 72. Action buttons stay down until clicked again (§14
//! r13); repair all acts on the release over its pressed button.
//!
//! d2rs-own, unverified (preview fills):
//! - the store grid (10 × 10 cells of 29 px) at (`sx` + 15, `H` + `sy` −
//!   400) with the store items packed row by row (REC-162; the server's
//!   cells and the NPC's `inventory.bin` record are q-fix-seam-store-grid);
//! - the buy price shown is published by the server host
//!   ([`ShopPrices`]); the sell and repair prices are not computed
//!   (`vendors.md` §9 cost in the client: open);
//! - a click on one of the player's items is a shop click only with the
//!   repair button down (when the player grid hands clicks on is not
//!   specified); `0x0062A130`, `0x004B1F80` and the `items.txt` byte
//!   +0x1A5 are not read (the server checks);
//! - Close acts on the press (the release is a spec gap);
//! - the time of the transaction rules is the bridge frame × 40 ms (+ 60 s).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::{OriginalUi, SharedRef};
use crate::bridge::items::{self, ItemView};
use crate::bridge::world::{ClientWorld, MONSTER};
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::inv_grid::GridRecord;
use crate::ui::panel::StringLookup;
use crate::ui::panel::{ClientIntent, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::menu_box::MenuBox;
use crate::ui::panels::npc::msg_chat_end;
use crate::ui::panels::shop::{
    caller_args, confirm_box, shop_button_records, shop_start_page, shop_tabs, Caller, ClickArgs,
    ClickEnv, ClickOutcome, ConfirmHandler, ItemFacts, SendFacts, ShopButton, ShopEffect,
    ShopPanel, ShopTx, TxKind, DEFAULT_TABS, IDENTIFY_CLASSES, MK_SHIFT, REPAIR_CLASSES,
    SELL_CLASSES, UI_SHOP,
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
    pub(super) tx: ShopTx,
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
    /// items repairs it (d2rs-own, unverified; REC-177).
    repair_mode: bool,
    /// A shop closed since the last Trade / Gamble choice: a late store
    /// record (a sold item's copy) opens nothing until the next choice.
    closed: bool,
    /// The confirm dialog `0x004B2F50` (`menus.md` §4.4), drawn and
    /// answered by the NPC menu panel (ui 8 is open under the shop).
    pub(super) confirm: Option<ConfirmUp>,
    /// A send asked for the waiting note (`menus.md` §2.7): opened by the
    /// next [`OriginalUi::npc_menu_poll`].
    pub(super) note: bool,
}

/// The open confirm dialog: the box, its kind and the send facts of the
/// pending item (looked up at the click).
#[derive(Clone, Debug)]
pub struct ConfirmUp {
    pub bx: MenuBox<ConfirmHandler>,
    pub kind: TxKind,
    pub facts: SendFacts,
    pub now: u32,
    /// The hire of a kind-5 dialog: (NPC GUID, record name).
    pub hire: Option<(u32, u16)>,
    /// The item index the left press went down on.
    pub pressed: Option<usize>,
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

    /// The confirm dialog's kind and its items' text, when one is up.
    pub fn confirm(&self) -> Option<(TxKind, Vec<Vec<u16>>)> {
        let c = self.confirm.as_ref()?;
        Some((c.kind, c.bx.items.iter().map(|i| i.text.clone()).collect()))
    }

    /// The pressed action button, by index (buttons stay down, §14 r13).
    pub fn pressed(&self) -> Option<usize> {
        self.pressed
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

/// The store items of the open trade on `page`, at the cells the stream
/// gave them (`seams/item-grids.md` §2.9): the server placed them in the
/// NPC's grid (`vendors.md` §3.1 r4) and the client draws and hit-tests
/// them there, without repacking. An item whose footprint leaves the
/// grid is not drawn or hit.
fn page_items(sh: &super::Shared, world: &ClientWorld, st: &ShopState, page: u8) -> Vec<ItemView> {
    let mut list: Vec<ItemView> = items::store_items(world)
        .into_iter()
        .filter(|i| i.store_seq > st.floor && i.page == page)
        .collect();
    list.sort_by_key(|i| i.store_seq);
    in_grid(list, |i| footprint(sh, i))
}

/// The items whose footprint (`size`) lies inside the store grid, left at
/// their stream cells.
fn in_grid(mut list: Vec<ItemView>, size: impl Fn(&ItemView) -> (i32, i32)) -> Vec<ItemView> {
    list.retain(|i| {
        let (w, h) = size(i);
        i32::from(i.x) + w <= i32::from(GRID.0) && i32::from(i.y) + h <= i32::from(GRID.1)
    });
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

/// The action button under `at` (`panels-2.md` §14 r13: `sx + X[n −
/// 1][i]` < x < that + 45, `H + sy − 109` < y < `H + sy − 65`, strict).
fn button_at(s: &crate::ui::layout::Screen, n: usize, at: Point) -> Option<usize> {
    let (lo, hi) = (s.h + s.sy() - 109, s.h + s.sy() - 65);
    if n == 0 || n > 4 || at.y <= lo || at.y >= hi {
        return None;
    }
    let xs = crate::ui::panels::shop::BUTTON_X[n - 1];
    (0..n).find(|&i| at.x > s.sx() + xs[i] && at.x < s.sx() + xs[i] + BUTTON_W)
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

    /// The effects of a transaction step: the sends leave through the
    /// shared outputs, the confirm dialog opens at the mouse (§4.4), the
    /// waiting note is asked of the NPC menu (§2.7).
    fn apply(
        sh: &mut super::Shared,
        st: &mut ShopState,
        world: &ClientWorld,
        strings: &dyn StringLookup,
        facts: (SendFacts, u32, Option<&ItemView>),
        effects: Vec<ShopEffect>,
    ) {
        for e in effects {
            match e {
                ShopEffect::Send(i) => sh.outputs.push(PanelOutput::Intent(i)),
                ShopEffect::Sound(id) => sh.outputs.push(PanelOutput::Sound(id as i32)),
                ShopEffect::WaitingNote => st.note = true,
                ShopEffect::Confirm(kind) => {
                    let (send, now, item) = facts;
                    let name = item.and_then(|it| item_name(sh, world, it));
                    let price = st.tx.pending.map_or(0, |p| p.price);
                    let s = |id: u16| strings.get_id(id).map(<[u16]>::to_vec).unwrap_or_default();
                    let m = super::game_messages::Measure(sh.fonts.as_ref());
                    let frame = (sh.config.screen.w, sh.config.screen.h);
                    let mouse = (sh.mouse.x, sh.mouse.y);
                    let named = name.as_deref().map(|n| (n, price));
                    if let Ok(bx) = confirm_box(kind, named, mouse, &s, frame, &m) {
                        st.confirm = Some(ConfirmUp {
                            bx,
                            kind,
                            facts: send,
                            now,
                            hire: None,
                            pressed: None,
                        });
                    }
                }
                ShopEffect::Cancel => {}
            }
        }
    }

    /// The click `0x004B3870` of `caller` (`menus.md` §4.1, §4.5) on
    /// `item` (`player`: one of the player's items). `shift` is the
    /// window message's `MK_SHIFT`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn click(
        sh: &mut super::Shared,
        st: &mut ShopState,
        open: ShopOpen,
        world: &ClientWorld,
        strings: &dyn StringLookup,
        caller: Caller,
        item: Option<&ItemView>,
        shift: bool,
    ) {
        let (a1, a4, quick, repair_all) = caller_args(caller, if shift { MK_SHIFT } else { 0 });
        let (guid, class, mode) = item.map_or((0, 0, 0), |it| {
            (
                it.key.guid,
                it.code.map_or(0, u32::from_le_bytes),
                u16::from(it.mode),
            )
        });
        // §4.3: the item's stat 72 (durability) for a one-item repair.
        let durability = item.map_or(0, |it| world.base(it.key, 72, 0).max(0) as u32);
        let now = (world.frames as u32).wrapping_mul(40).wrapping_add(60_000);
        let send = SendFacts {
            item_found: item.is_some() || repair_all,
            item_mode: mode,
            durability,
            // d2rs-own: the `items.txt` byte +0x1A5 is not in the client
            // tables yet; the server reads the bit.
            item_flag_1a5: false,
            item_is_cursor: item.is_some_and(|it| it.mode == items::mode::CURSOR),
        };
        let args = ClickArgs {
            npc_class: Some(open.npc_class),
            npc_guid: open.npc_guid,
            item: ItemFacts {
                guid,
                class,
                at_location_4: false,
                // d2rs-own: `0x0062A130` and `0x004B1F80` are not
                // specified; the server checks the sell and the repair.
                sellable: true,
                repairable: true,
                type_ok_16: false,
            },
            a1,
            a4,
            quick,
            repair_all,
            now,
            send_facts: send,
        };
        let prices = st.prices.clone();
        let price = move |_t: u8| prices.get(guid).unwrap_or(0);
        let env = ClickEnv {
            cursor_item: items::cursor_item(world).is_some(),
            // PROVISIONAL (REC-729): menus.md §4.2 r2 says [0x007C0DB0] is
            // only ever written with 0, yet the server (vendors.md §7.1
            // rule 2) accepts a gamble buy only with transaction 2. Until
            // a recorded Gamble buy shows the C->S 0x32 u32@9 (pc1-data
            // Step 4), the window keeps sending the OR 2.
            gamble_shop: st.gamble,
            repair_all_button_on: repair_all,
            repair_button_on: st.repair_mode,
            repair_all_price: st.prices.get(REPAIR_ALL_KEY).unwrap_or(0),
            price: &price,
        };
        if let ClickOutcome::Effects(e) = st.tx.click(&args, &env) {
            Self::apply(sh, st, world, strings, (send, now, item), e);
        }
    }

    /// A left press on one of the player's items while the shop is up: the
    /// grid / body-location click `0x004B3870(a1 = 1, quick 0)` (§4.5):
    /// repair through the confirm dialog. Returns whether the press was
    /// taken (repair button down, no cursor item, an item under it).
    pub(super) fn player_item_click(
        sh: &mut super::Shared,
        st: &mut ShopState,
        world: &ClientWorld,
        strings: &dyn StringLookup,
        item: Option<&ItemView>,
    ) -> bool {
        // d2rs-own until the player-grid shop handler is specified
        // (`menus.md` §4.5 names its callers, not when the grid hands a
        // click on): only the down repair button turns a click on one of
        // the player's items into the click; else the item is lifted.
        let Some(open) = st.open.filter(|_| st.repair_mode) else {
            return false;
        };
        let Some(it) = item.filter(|_| items::cursor_item(world).is_none()) else {
            return false;
        };
        let caller = if it.mode == items::mode::BODY {
            Caller::BodyLocation
        } else {
            Caller::GridClick(1)
        };
        Self::click(sh, st, open, world, strings, caller, Some(it), false);
        true
    }
}

/// The tip context of a store item hovered in the shop (`item-tips.md`
/// Inputs, §1 r3, §11 r2–r3): the inventory mode, a store item
/// (`[0x00721E38]` = 0), the gamble store and the price text `Cost: ` +
/// the price (gold fails, §11 r3).
///
/// d2rs-own, unverified: the inventory mode is 4 while the repair button
/// is on (§11 r1's repair mode), else 1 (`panels-2.md` §16 r13, the idle
/// trade mode); the other buttons' modes are not in the spec. A store
/// item without a price yet fails.
fn store_tip_ctx<'a>(
    base: crate::ui::item_tip_build::TipCtx<'a>,
    st: &ShopState,
    guid: u32,
    code: Option<[u8; 4]>,
) -> crate::ui::item_tip_build::TipCtx<'a> {
    use crate::ui::item_tip_build::PriceText;
    let gold = code.is_some_and(|c| &c == b"gld ");
    let price = match st.prices.get(guid) {
        Some(p) if !gold => PriceText::Price {
            label: crate::ui::item_tip_build::tid::COST,
            price: p as i32,
        },
        _ => PriceText::Fail,
    };
    crate::ui::item_tip_build::TipCtx {
        mode: if st.repair_mode { 4 } else { 1 },
        own_item: false,
        // The same global [0x007C0DB0] (ui/item-tips.md inputs): always 0.
        gamble: false,
        price,
        ..base
    }
}

/// The item's name: the first line of its tip (d2rs-own until the
/// `item-tips.md` builder names it, q-fix-ui-item-tips).
fn item_name(sh: &super::Shared, world: &ClientWorld, it: &ItemView) -> Option<Vec<u16>> {
    let tips = sh.items.tips.as_ref()?;
    let stream = items::stream(world, it.key)?;
    tips.lines(stream).into_iter().next().map(|l| l.text)
}

impl Panel for ShopUi {
    /// `ui/panels.md` §9 r1 revision: drawn before the inventory.
    fn draw_before(&self) -> Option<PanelId> {
        Some(PanelId(u16::from(
            crate::ui::panels::inventory::UI_INVENTORY,
        )))
    }

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
            sh.items
                .draw_at(ctx.world, &sh.tables.files, it, (x, y), out);
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
        // The hovered store item's tip with the store context
        // (`item-tips.md` §1 r3, §11), else the bare price.
        let hovered = item_at(&sh, &g, &list, sh.mouse);
        let tip = hovered.as_ref().and_then(|it| {
            let tips = sh.items.tips.as_ref()?;
            let stream = crate::bridge::items::stream(ctx.world, it.key)?;
            let me = crate::ui::item_tip_world::WorldUnit::local(ctx.world);
            let base = crate::ui::item_tip_world::hover_ctx(tips, ctx.world, me.as_ref(), it);
            let tc = store_tip_ctx(base, &st, it.key.guid, it.code);
            let lines = tips.tip_lines(stream, &tc);
            (!lines.is_empty()).then_some(lines)
        });
        if let Some(lines) = tip {
            crate::ui::item_tip::draw_tip(
                &lines,
                sh.mouse,
                (s.w, s.h),
                sh.fonts.as_ref(),
                &sh.tables.files,
                out,
            );
        } else if let (Some(it), Some(_)) = (hovered, &sh.fonts) {
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
        let s = sh.config.screen;
        let (panel, page) = self.panel(ctx.world, &st, open);
        if !press {
            // Buttons stay down until clicked again (§14 r13); the
            // repair-all button acts on its release over it while pressed
            // (§4.5, `0x00488B00`: WM_LBUTTONUP, base frame 18).
            let i = button_at(&s, panel.buttons.len(), at);
            let rec = i.map(|i| shop_button_records(open.npc_class)[i]);
            if let (Some(i), Some(rec)) = (i, rec) {
                if rec.base_frame == 18 && st.pressed == Some(i) {
                    st.pressed = None;
                    Self::click(
                        &mut sh,
                        &mut st,
                        open,
                        ctx.world,
                        ctx.strings,
                        Caller::RepairAll(1),
                        None,
                        false,
                    );
                }
            }
            return UiResponse::Consumed;
        }
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
                // Id 6 (`client/ui.md` §B8.1, `0x00491F7B` …).
                sh.outputs.push(PanelOutput::Sound(6));
            }
            return UiResponse::Consumed;
        }
        // Button bar (strict bounds, `X[3][i]` without the draw's − 1).
        if let Some(i) = button_at(&s, panel.buttons.len(), at) {
            {
                {
                    let rec = shop_button_records(open.npc_class)[i];
                    if rec.enabled {
                        // Not a constant-id site of `client/ui.md` §B8.1:
                        // the click sound of `panels-2.md` §16 (id 0).
                        sh.outputs
                            .push(PanelOutput::Sound(crate::ui::original::CLICK_SOUND_ID));
                        // §14 r13: another pressed button is released and
                        // this one pressed; this one pressed is released.
                        let was = st.pressed == Some(i);
                        st.pressed = (!was).then_some(i);
                        match rec.base_frame {
                            // Close (record 3 of the other traders).
                            10 => sh.outputs.push(PanelOutput::SetUi {
                                ui: id::NPC_SHOP,
                                mode: 1,
                                jump: false,
                            }),
                            // Repair one item: the button is the mode.
                            6 => st.repair_mode = !was,
                            // Repair all: pressed here, sent on the
                            // release (above).
                            18 => st.repair_mode = false,
                            _ => st.repair_mode = false,
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
        let shift = sh.items.shift;
        let caller = match (button, cursor.as_ref(), under.as_ref()) {
            // §4.5 `0x00491D20`: a cursor item dropped on the store grid
            // sells it (quick).
            (PointerButton::Left, Some(c), _) => Some((Caller::DropOnStoreGrid, c.clone())),
            // `0x00491AD0`: the right click buys at once (quick).
            (PointerButton::Right, None, Some(it)) => {
                Some((Caller::StoreGridRightClick, it.clone()))
            }
            // `0x0048FFE0`: the left click goes through the confirm
            // dialog (§4.4).
            (PointerButton::Left, None, Some(it)) => Some((Caller::GridClick(0), it.clone())),
            _ => None,
        };
        if let Some((c, it)) = caller {
            Self::click(
                &mut sh,
                &mut st,
                open,
                ctx.world,
                ctx.strings,
                c,
                Some(&it),
                shift,
            );
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
            st.confirm = None;
            st.note = false;
            st.pressed = None;
            st.repair_mode = false;
            st.closed = false;
            // The start page of the class (`panels-2.md` §14 r12).
            st.page = shop_start_page(npc_class).0;
        }
        // `panels.md` §4.1, §5 step 5: the shop is the full-slot ui 0x0C;
        // it draws the inventory family itself (ui 1 stays off, ui 8 on).
        // d2rs-own: a refused open (the gate) leaves the shop state up
        // and `shop_poll` closes it again.
        self.shared.borrow_mut().items.store_npc = Some(npc_guid);
        let _ = self.set_ui(u32::from(id::NPC_SHOP), 0, false);
    }

    /// The prices the server host publishes ([`ShopPrices`]).
    pub fn set_shop_prices(&mut self, prices: ShopPrices) {
        self.shop.borrow_mut().prices = prices;
    }

    /// The shop state (tests).
    pub fn shop_state(&self) -> std::cell::Ref<'_, ShopState> {
        self.shop.borrow()
    }

    /// The centre of the panel cell the shop shows store item `guid` at
    /// on the current page (tests).
    pub fn store_item_point(&self, world: &ClientWorld, guid: u32) -> Option<Point> {
        let sh = self.shared.borrow();
        let st = self.shop.borrow();
        st.open?;
        let g = grid_record(&sh);
        let (_, page) = tabs_of(world, &st);
        let it = page_items(&sh, world, &st, page)
            .into_iter()
            .find(|i| i.key.guid == guid)?;
        let (l, t, w, h) = g.cell(i32::from(it.x), i32::from(it.y));
        Some(Point::new(l + w / 2, t + h / 2))
    }

    /// The tip lines of store item `guid` as the shop draws them on
    /// hover, with the store context ([`store_tip_ctx`]; tests).
    pub fn store_tip_lines(&self, world: &ClientWorld, guid: u32) -> Vec<String> {
        let sh = self.shared.borrow();
        let st = self.shop.borrow();
        let Some(tips) = sh.items.tips.as_ref() else {
            return Vec::new();
        };
        let Some(it) = items::store_items(world)
            .into_iter()
            .find(|i| i.key.guid == guid)
        else {
            return Vec::new();
        };
        let Some(stream) = items::stream(world, it.key) else {
            return Vec::new();
        };
        let me = crate::ui::item_tip_world::WorldUnit::local(world);
        let base = crate::ui::item_tip_world::hover_ctx(tips, world, me.as_ref(), &it);
        let tc = store_tip_ctx(base, &st, guid, it.code);
        tips.tip_lines(stream, &tc)
            .iter()
            .map(|l| String::from_utf16_lossy(&l.text))
            .collect()
    }

    /// The shown page's store items as (GUID, x, y, w, h): the stream's
    /// cell and the footprint (tests).
    pub fn store_item_cells(&self, world: &ClientWorld) -> Vec<(u32, i32, i32, i32, i32)> {
        let sh = self.shared.borrow();
        let st = self.shop.borrow();
        if st.open.is_none() {
            return Vec::new();
        }
        let (_, page) = tabs_of(world, &st);
        page_items(&sh, world, &st, page)
            .iter()
            .map(|i| {
                let (w, h) = footprint(&sh, i);
                (i.key.guid, i32::from(i.x), i32::from(i.y), w, h)
            })
            .collect()
    }

    /// The store item the shop's hit test finds at grid cell (x, y)
    /// (`item_at` at the cell's centre; tests).
    pub fn store_item_at_cell(&self, world: &ClientWorld, x: i32, y: i32) -> Option<u32> {
        let sh = self.shared.borrow();
        let st = self.shop.borrow();
        st.open?;
        let g = grid_record(&sh);
        let (_, page) = tabs_of(world, &st);
        let list = page_items(&sh, world, &st, page);
        let (l, t, w, h) = g.cell(x, y);
        item_at(&sh, &g, &list, Point::new(l + w / 2, t + h / 2)).map(|i| i.key.guid)
    }

    /// The centre of store grid cell (x, y) on screen (tests).
    pub fn store_cell_point(&self, x: i32, y: i32) -> Point {
        let g = grid_record(&self.shared.borrow());
        let (l, t, w, h) = g.cell(x, y);
        Point::new(l + w / 2, t + h / 2)
    }

    /// The centre of the confirm dialog's Yes (`yes`) or No item (tests).
    pub fn shop_confirm_point(&self, yes: bool) -> Option<Point> {
        let st = self.shop.borrow();
        let c = st.confirm.as_ref()?;
        let want = if yes {
            ConfirmHandler::Yes
        } else {
            ConfirmHandler::No
        };
        let i = c.bx.items.iter().position(|i| i.handler == Some(want))?;
        let r = super::npc_box::item_rect(&c.bx, i);
        Some(Point::new((r.l + r.r) / 2, (r.t + r.b) / 2))
    }

    /// Per frame: opens the shop when a trade's store items arrived (for
    /// the trader nearest to the local player, d2rs-own) and ends the
    /// interaction (C→S 0x30) when the shop's UI state went off.
    pub fn shop_poll(&mut self, world: &ClientWorld, root: &mut UiRoot) {
        // d2rs-own, unverified (REC-277): the Trade / Gamble choice opens
        // the shop at once, so its close ends the interaction (C→S 0x30)
        // even when no store item arrives.
        let request = self.npcm.borrow_mut().shop_request.take();
        if let Some((guid, class)) = request {
            if self.shop.borrow().open.is_none() {
                self.open_shop(guid, class);
                // The store records newer than the choice are this trade's
                // (a closed shop's late copy is not; q-smoke-town).
                if let Some((_, _, floor)) = self.npcm.borrow_mut().shop_for.take() {
                    self.shop.borrow_mut().floor = floor;
                }
            }
        }
        let (open, serial) = {
            let st = self.shop.borrow();
            (st.open, st.serial)
        };
        if world.store_serial != serial {
            self.shop.borrow_mut().serial = world.store_serial;
            if open.is_none() {
                // The trader the menu chose, else the nearest one (the
                // model's local position is the last placement, not the
                // predicted walk: d2rs-own, unverified).
                let chosen = self.npcm.borrow_mut().shop_for.take();
                if let Some((guid, class, floor)) = chosen {
                    self.open_shop(guid, class);
                    // The records of an earlier trade (a closed shop's late
                    // copy) are not this store's.
                    self.shop.borrow_mut().floor = floor;
                } else if !self.shop.borrow().closed {
                    // A closed shop's late records (the copy of an item sold
                    // just before the close) are not a new trade.
                    if let Some((guid, class)) = nearest_trader(world) {
                        self.open_shop(guid, class);
                    }
                }
            }
        }
        let open = self.shop.borrow().open;
        if let Some(o) = open.filter(|_| !self.is_open(id::NPC_SHOP)) {
            {
                let mut st = self.shop.borrow_mut();
                st.open = None;
                self.shared.borrow_mut().items.store_npc = None;
                st.repair_mode = false;
                st.floor = world.store_serial;
                st.closed = true;
            }
            // panels-2.md §14.9: the interaction's end `0x004B3C20` sends
            // 30 [1 u32][GUID u32], then `SetUIState(8, off)`.
            root.queue_intent(ClientIntent(msg_chat_end(o.npc_guid).to_vec()));
            self.shop.borrow_mut().confirm = None;
            let _ = self.set_ui(u32::from(id::NPC_MENU), 1, false);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::UnitKey;

    fn at(guid: u32, x: u16, y: u16) -> ItemView {
        ItemView {
            key: UnitKey::new(4, guid),
            code: Some(*b"hp1 "),
            flags: 0,
            mode: 0,
            body: 0,
            page: 1,
            x,
            y,
            owner: None,
            store: true,
            store_seq: guid,
            gold: None,
        }
    }

    // Covers: specs/seams/item-grids.md §2.9
    #[test]
    fn store_items_stay_at_the_servers_cells() {
        // The sim placed these with gaps (first free spot, 2 x 3 items);
        // the shop must not pack them to the top-left.
        let list = vec![at(1, 4, 0), at(2, 0, 5), at(3, 9, 9), at(4, 9, 8)];
        let size = |i: &ItemView| if i.key.guid == 4 { (2, 3) } else { (1, 1) };
        let kept = in_grid(list, size);
        let cells: Vec<_> = kept.iter().map(|i| (i.key.guid, i.x, i.y)).collect();
        // Guid 4 (2 x 3 at (9, 8)) leaves the grid and is not drawn.
        assert_eq!(cells, vec![(1, 4, 0), (2, 0, 5), (3, 9, 9)]);
    }
}
