// Spec: specs/ui/menus.md (§2 r1–r7, §3 r1–r2), specs/ui/panels-2.md (§14 r7–r10), specs/client/msg-ui.md (§18)
//! The NPC menu (ui 8) in play: the spec box of
//! [`build_npc_menu`] / [`MenuBox`] anchored above the interaction's NPC
//! (`menus.md` §2.6), opened by S→C 0x28 (`msg_ui`) as `SetUIState(8,
//! on)` so the ui-states gates apply, the hire-list request C→S 0x38
//! action 3 for 252 / 198 / 515 / 150 (§2.2), the record lookup of
//! `panels-2.md` §14.7 (no match → record 0), the waiting state of the
//! Identify / Resurrect sends and S→C 0x2A's rebuild (§14.10,
//! `msg-ui.md` §18), and the hire list's Back rebuilding the menu
//! (`menus.md` §3.2).
//!
//! The adapter owns the interaction's state; the box, its captions, its
//! layout and its draw are the spec modules'. Draws go through the
//! present sink: the framed box (`0x0046EFD0(x, y, w, h, 0, 1)`) is the
//! sink's rectangle ([`RectRequest::sized`]).
// d2rs-own, unverified (M22, each provisional until the spec of the menu
// box window handlers 0x0E / 1 lands, `q-ui-audit.md` "Spec gaps"):
// - an item's hit band is (pen y − height, pen y] across the box width;
// - the pointer over a selectable item selects it (style 1: color 3);
// - a left release on the item pressed runs its handler; a left press
//   outside the box ends the interaction (C→S 0x30, `SetUIState(8,
//   off)`) and is consumed;
// - Talk and the services are `npc_talk`'s (topic box, dialog panel,
//   item-socket dialog);
// - Charsi's Imbue row is inserted before the cancel (REC-145);
// - the NPC's name line is empty (monstats name keys are not in the
//   client model) and the heal cost is 0 (`0x00622DE0` not in the model);
// - an Identify / Resurrect send closes the box until S→C 0x2A.

use std::cell::RefCell;
use std::rc::Rc;

use super::game_messages::Measure;
use super::SharedRef;
use crate::bridge::hover::feet;
use crate::bridge::items;
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
use crate::ui::draw::{RectRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect, FRAME};
use crate::ui::hire_list::hire_intent;
use crate::ui::layout::{MenuOption, NpcMenuRecord, OptionKind};
use crate::ui::messages::socket::NPC_CHARSI;
use crate::ui::messages::Ltrb;
use crate::ui::panel::WidgetId;
use crate::ui::panel::{ClientIntent, Panel, PanelId, StringLookup, UiCtx, UiEvent, UiResponse};
use crate::ui::panels::menu_box::{anchor, MenuBox, MenuDraw, WaitingNote, STR_WAITING};
use crate::ui::panels::npc::{
    cain_count_reset, msg_chat_end, option_intent, record_index, NpcMenus,
};
use crate::ui::panels::npc_menu::{
    build_npc_menu, hire_open, CaptionCtx, HireOpen, NpcMenuBuild, NpcMenuHandler, NpcMenuInput,
};
use crate::ui::panels::shop::{
    confirm_answer, confirm_box, ConfirmHandler, ConfirmResult, ShopEffect, TxKind,
};
use crate::ui::panels::PanelOutput;
use crate::ui::root::UiRoot;
use crate::ui::states::id;
use crate::ui::text::TextOpts;
use crate::ui::PointerButton;

use super::OriginalUi;

/// A box item's text and color (tests).
pub type BoxItem = (Vec<u16>, u8);

/// The panel id: the UI state (ui 8), so the root mirrors its flag.
pub const NPC_MENU_PANEL: PanelId = PanelId(id::NPC_MENU as u16);
/// The menu state `[0x007C0C6B]` of a send that waits for S→C 0x2A
/// (`panels-2.md` §14.10).
pub const MENU_WAITING: u8 = 10;
/// `[0x007C0C6B]` after a failed transaction (§14.10).
pub const MENU_FAILED: u8 = 12;
/// UI sound of a 0x2A result 1 outside the waiting states (`msg-ui.md`
/// §18 r2).
pub const SOUND_TX_DONE: i32 = 221;
/// `NPCImbue` string of the d2rs-own insert (REC-145).
const STR_IMBUE: u16 = 4017;

/// One selectable row of the box: its string id, the option it runs
/// (`None` = the cancel item) and the `%d` its caption carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub string: u16,
    pub kind: Option<OptionKind>,
    pub cost: Option<u32>,
}

/// The open menu, as tests and the host see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Open {
    pub guid: u32,
    pub class: u32,
    /// The selectable items of the box in order (the options, then the
    /// cancel item).
    pub rows: Vec<Row>,
    /// Talk was chosen: the box is closed, the topic box or the dialog
    /// panel is up (`npc_talk`).
    pub talking: bool,
}

/// Cain's identify count `0x0062A530` (`world/npc.md` §6 step 3): the
/// local player's items without flag 0x10 in the backpack (page 0), the
/// cube (page 3) or worn; the stash and the belt are skipped.
pub fn unidentified_count(world: &ClientWorld) -> u32 {
    items::local_items(world)
        .iter()
        .filter(|i| {
            i.flags & 0x10 == 0
                && match i.mode {
                    items::mode::STORED => i.page == 0 || i.page == 3,
                    items::mode::BODY => true,
                    _ => false,
                }
        })
        .count() as u32
}

/// The interaction the menu belongs to.
#[derive(Clone, Debug)]
struct Interaction {
    guid: u32,
    class: u32,
    identify_n: u32,
    /// The box was asked for and not built yet (needs the strings).
    build: bool,
}

#[derive(Default)]
pub struct NpcMenuState {
    up: Option<Interaction>,
    bx: Option<MenuBox<NpcMenuHandler>>,
    /// The record the box was built from (its options by slot).
    rec: Option<NpcMenuRecord>,
    rows: Vec<Row>,
    /// The NPC's anchor (§2.6), taken at the open / rebuild.
    anchor: Option<(i32, i32)>,
    /// The item index the left press went down on.
    pressed: Option<usize>,
    /// `[0x007C0C6B]`.
    pub menu_state: u8,
    /// The waiting note (§2.7).
    note: Option<WaitingNote>,
    /// Messages of the build (C→S 0x38 action 3) waiting for the root.
    pending: Vec<PanelOutput>,
    menus: Option<NpcMenus>,
    /// The talk, its dialog panel and the item-socket dialog
    /// (`npc_talk`).
    pub(super) talk: super::npc_talk::TalkState,
    /// The last Trade / Gamble choice was Gamble: the shop that opens next
    /// is a gamble window (`panels-2.md` §14: the gamble shop flag).
    pub gamble: bool,
    /// The NPC (GUID, class) the last Trade / Gamble was chosen for and
    /// the model's store serial at the choice (`OriginalUi::shop_poll`).
    pub shop_for: Option<(u32, u32, u32)>,
    /// The Resurrect edit of the next open (`panels-2.md` §14.2): the cost
    /// `[0x007C0DD0]` while the mercenary is dead in an expansion game.
    pub resurrect: Option<u32>,
    /// The mercenary's name string id `[0x00725494]` (S→C 0x9B).
    pub merc_name: u16,
    /// The Trade / Gamble choice's NPC (GUID, class): the shop opens for
    /// it on the next shop poll. d2rs-own, unverified (REC-277).
    pub shop_request: Option<(u32, u32)>,
    pub screen: (i32, i32),
}

pub type SharedNpcMenu = Rc<RefCell<NpcMenuState>>;

/// The camera of the local player's cell: the walk prediction's while it
/// holds, else the model's (d2rs-own: the view's sub-tile offset is not
/// in the model, so the anchor can be up to a sub-tile off the drawn
/// view).
fn camera(w: &ClientWorld, open_mode: u8) -> Option<Camera> {
    let me = w.local()?;
    let (x, y) = w.predicted(me).map_or(me.cell(), |p| p.cell);
    let at = moving_to_client((u32::from(x) << 16) | 0x8000, (u32::from(y) << 16) | 0x8000);
    let mode = OpenMode::new(open_mode).unwrap_or(OpenMode::NONE);
    Some(Camera::new(FrameSize::D2RS, mode, at, (0, 0)))
}

/// §2.6: the NPC's screen point raised by 150 (at least 20); none
/// without the NPC in the model. The camera's unit point is the client
/// pixel point less the tile origin already.
fn npc_anchor(world: &ClientWorld, guid: u32, open_mode: u8) -> Option<(i32, i32)> {
    let cell = world.units.get(&UnitKey::new(1, guid))?.position?;
    let cam = camera(world, open_mode)?;
    anchor(Some(feet(&cam, cell)), (0, 0))
}

fn string_fn(s: &dyn StringLookup) -> impl Fn(u16) -> Vec<u16> + '_ {
    move |id| s.get_id(id).map(<[u16]>::to_vec).unwrap_or_default()
}

/// An item's hit band (module doc: d2rs-own).
pub(super) fn item_rect<H>(bx: &MenuBox<H>, i: usize) -> Ltrb {
    let top = bx.pos.1 + bx.items[..i].iter().map(|it| it.height).sum::<i32>();
    Ltrb::new(
        bx.pos.0,
        top + 1,
        bx.pos.0 + bx.size.0,
        top + bx.items[i].height + 1,
    )
}

fn ltrb_contains(r: &Ltrb, p: Point) -> bool {
    p.x >= r.l && p.x < r.r && p.y >= r.t && p.y < r.b
}

/// The item of `bx` at `p` that is selectable.
pub(super) fn item_at<H: Clone + PartialEq>(bx: &MenuBox<H>, p: Point) -> Option<usize> {
    (0..bx.items.len()).find(|&i| bx.items[i].selectable && ltrb_contains(&item_rect(bx, i), p))
}

/// The draws of a menu box through the present sink.
pub(crate) fn push_menu_draws(draws: Vec<MenuDraw>, out: &mut dyn UiDrawSink) {
    for d in draws {
        match d {
            MenuDraw::Frame(r) => out.push(UiDraw::Rect(RectRequest::sized(
                r.x, r.y, r.w, r.h, r.color, r.mode,
            ))),
            MenuDraw::Text {
                text,
                x,
                y,
                color,
                font,
            } => out.push(UiDraw::Text(TextRequest {
                text,
                at: Point::new(x, y),
                style: TextStyle {
                    font,
                    color: u16::from(color),
                },
                opts: TextOpts::default(),
                clip: FRAME,
            })),
            // Cels and the pentagram need the cel draw of the sink
            // (q-fix-ui-draw-sink); the NPC menu (style 1) has neither.
            MenuDraw::Cel { .. } | MenuDraw::Pentspin { .. } => {}
        }
    }
}

impl NpcMenuState {
    /// The menus table, loaded once and edited per interaction.
    fn menus(&mut self) -> &mut NpcMenus {
        self.menus.get_or_insert_with(|| {
            NpcMenus::load().unwrap_or_else(|_| NpcMenus::from_records(Vec::new()))
        })
    }

    /// The interaction start (`panels-2.md` §14.2): the table edits; the
    /// box is built by [`Self::build`] once the strings are at hand.
    fn start(&mut self, guid: u32, class: u32, level: i32, identify_n: u32) {
        let resurrect = self.resurrect.is_some();
        let menus = self.menus();
        menus.reset_for_interaction();
        menus.apply_builder(level);
        menus.apply_resurrect(resurrect, true);
        self.talk.topic = None;
        self.talk.active = false;
        self.note = None;
        self.menu_state = 1;
        self.up = Some(Interaction {
            guid,
            class,
            identify_n,
            build: true,
        });
    }

    /// `0x004B4830` (`menus.md` §2.2): the record of the NPC's class
    /// (§14.7: no match → record 0) with the captions of §2.3.
    fn build(&mut self, world: &ClientWorld, strings: &dyn StringLookup, m: &Measure<'_>) {
        let Some(it) = self.up.clone() else {
            return;
        };
        self.up.as_mut().unwrap().build = false;
        let (records, idx) = {
            let menus = self.menus();
            let idx = record_index(menus.records(), Some(it.class));
            (menus.records().to_vec(), idx)
        };
        let Some(mut rec) = idx.and_then(|i| records.get(i).cloned()) else {
            return;
        };
        let mut imbue = false;
        if it.class == NPC_CHARSI {
            // d2rs-own, unverified (REC-145): the imbue insert.
            let slot = (rec.count as usize).saturating_sub(1);
            if slot < rec.options.len() {
                rec.options[slot] = Some(MenuOption {
                    string: STR_IMBUE,
                    kind: OptionKind::Imbue,
                });
                rec.count += 1;
                imbue = true;
            }
        }
        let count = rec.count;
        let player = world.local();
        let captions = CaptionCtx {
            merc_name_id: self.merc_name,
            resurrect_cost: self.resurrect.map_or(0, |c| c as i32),
            life_below_max: player
                .is_some_and(|p| world.total(p.key, 6, 0) < world.total(p.key, 7, 0)),
            heal_cost: 0,
            identify_n: it.identify_n as i32,
            // The quest-4 bits that waive the cost are not in the client
            // model (d2rs-own, unverified).
            identify_quest_bits_clear: true,
            quest41_bit0: false,
            quest41_bit1: false,
            difficulty: world.difficulty,
        };
        let interaction_ok = world.units.contains_key(&UnitKey::new(1, it.guid));
        let anchor = self.anchor.or(Some((self.screen.0 / 2, self.screen.1 / 3)));
        let input = NpcMenuInput {
            npc_class: it.class,
            npc_guid: it.guid,
            interaction_ok,
            npc_name: Vec::new(),
            player_guid: player.map(|p| p.key.guid),
            captions: &captions,
        };
        let strings = string_fn(strings);
        match build_npc_menu(
            &mut rec,
            &input,
            anchor.unwrap_or((0, 0)),
            &strings,
            self.screen,
            m,
        ) {
            Ok(NpcMenuBuild::Ended(out)) => {
                self.pending.push(PanelOutput::Intent(ClientIntent(
                    msg_chat_end(it.guid).to_vec(),
                )));
                self.pending.extend(out);
                self.close();
            }
            Ok(NpcMenuBuild::Built { send, bx }) => {
                self.pending.extend(send);
                self.rows = bx
                    .items
                    .iter()
                    .filter(|i| i.selectable)
                    .map(|i| match i.handler {
                        Some(NpcMenuHandler::Slot(s)) => {
                            let o = rec.options[s].expect("a built slot has an option");
                            Row {
                                string: o.string,
                                kind: Some(o.kind),
                                cost: match o.kind {
                                    OptionKind::Identify => Some(100 * it.identify_n),
                                    OptionKind::Resurrect => self.resurrect,
                                    _ => None,
                                },
                            }
                        }
                        _ => Row {
                            string: crate::ui::panels::npc_menu::STR_LOWER_CANCEL,
                            kind: None,
                            cost: None,
                        },
                    })
                    .collect();
                // §2.3: the identify caption sets the table record's count.
                if rec.count != count {
                    let c = rec.count - u32::from(imbue);
                    if let Some(r) = idx.and_then(|i| self.menus().record_mut(i)) {
                        r.count = c;
                    }
                }
                self.rec = Some(rec);
                self.bx = Some(bx);
                self.pressed = None;
            }
            // A layout the original treats as fatal: no box (d2rs-own).
            Err(_) => self.bx = None,
        }
    }

    /// The box and the interaction go (the interaction's end).
    fn close(&mut self) {
        self.talk.topic = None;
        self.talk.active = false;
        if self.talk.dialog.panel.is_some() {
            let _ = self.talk.dialog.close();
        }
        self.up = None;
        self.bx = None;
        self.rec = None;
        self.rows.clear();
        self.pressed = None;
        self.menu_state = 0;
    }

    /// The interaction's end `0x004B3C20` (`panels-2.md` §14.9): C→S 0x30
    /// [1][GUID], `SetUIState(8, off)`.
    fn end(&mut self, guid: u32) -> Vec<PanelOutput> {
        self.close();
        vec![
            PanelOutput::Intent(ClientIntent(msg_chat_end(guid).to_vec())),
            PanelOutput::SetUi {
                ui: id::NPC_MENU,
                mode: 1,
                jump: false,
            },
        ]
    }

    /// [`Self::end`] for the talk and the services.
    pub(super) fn end_interaction(&mut self, guid: u32) -> Vec<PanelOutput> {
        self.end(guid)
    }

    /// The interaction NPC's class.
    pub(super) fn npc_class(&self) -> Option<u32> {
        self.up.as_ref().map(|it| it.class)
    }

    /// The built record's flag byte (§14.8).
    pub(super) fn record_flag(&self) -> u8 {
        self.rec.as_ref().map_or(0, |r| r.flag)
    }

    /// The menu is built again at the next poll.
    pub(super) fn ask_build(&mut self) {
        if let Some(it) = self.up.as_mut() {
            it.build = true;
        }
    }

    /// The NPC's anchor (§2.6), else the frame's centre (d2rs-own).
    pub(super) fn anchor_or_centre(&self) -> (i32, i32) {
        self.anchor
            .unwrap_or((self.screen.0 / 2, self.screen.1 / 3))
    }

    /// Outputs that wait for the next poll (outside an event).
    pub(super) fn push_pending(&mut self, o: Vec<PanelOutput>) {
        self.pending.extend(o);
    }

    fn open_view(&self) -> Option<Open> {
        let it = self.up.as_ref()?;
        let talking = self.talk.active;
        (self.bx.is_some() || talking).then(|| Open {
            guid: it.guid,
            class: it.class,
            rows: if self.bx.is_some() {
                self.rows.clone()
            } else {
                Vec::new()
            },
            talking,
        })
    }

    /// The selectable item `k` of the box, as an item index.
    fn row_item(&self, k: usize) -> Option<usize> {
        let bx = self.bx.as_ref()?;
        (0..bx.items.len())
            .filter(|&i| bx.items[i].selectable)
            .nth(k)
    }
}

/// The panel.
pub struct NpcMenuUi {
    pub(super) sh: SharedRef,
    /// The shop, whose confirm dialog this panel draws and answers (ui 8
    /// stays open under the shop, `panels-2.md` §14.9).
    pub(super) shop: super::shop_ui::SharedShop,
    pub st: SharedNpcMenu,
    pub hire: crate::ui::hire_list::SharedHire,
}

impl NpcMenuUi {
    fn out(&self, o: Vec<PanelOutput>) {
        self.sh.borrow_mut().outputs.extend(o);
    }

    pub(super) fn out_pub(&self, o: Vec<PanelOutput>) {
        self.out(o);
    }

    fn run(&mut self, item: usize, ctx: &UiCtx) -> UiResponse {
        let (handler, guid, class) = {
            let st = self.st.borrow();
            let (Some(bx), Some(it)) = (st.bx.as_ref(), st.up.as_ref()) else {
                return UiResponse::Consumed;
            };
            (bx.items[item].handler, it.guid, it.class)
        };
        let kind = match handler {
            Some(NpcMenuHandler::Slot(s)) => self
                .st
                .borrow()
                .rec
                .as_ref()
                .and_then(|r| r.options[s])
                .map(|o| o.kind),
            Some(NpcMenuHandler::Cancel) => {
                let o = self.st.borrow_mut().end(guid);
                self.out(o);
                return UiResponse::Consumed;
            }
            _ => None,
        };
        let Some(kind) = kind else {
            return UiResponse::Consumed;
        };
        let mut st = self.st.borrow_mut();
        // Every option handler closes the box first (`0x004B8020`).
        st.bx = None;
        st.pressed = None;
        match kind {
            OptionKind::Talk => {
                drop(st);
                self.talk_open(ctx);
                UiResponse::Consumed
            }
            OptionKind::Imbue => {
                let o = super::npc_talk::open_imbue(&mut st, guid, class, ctx.world);
                drop(st);
                self.out(o);
                UiResponse::Consumed
            }
            OptionKind::Hire => {
                let up = self.hire.borrow().up.is_some();
                match hire_open(up) {
                    HireOpen::CloseAndRebuild => {
                        self.hire.borrow_mut().up = None;
                        if let Some(it) = st.up.as_mut() {
                            it.build = true;
                        }
                    }
                    HireOpen::Build => self.hire.borrow_mut().up = Some(guid),
                }
                UiResponse::Consumed
            }
            kind => {
                if matches!(kind, OptionKind::Trade | OptionKind::Gamble) {
                    st.gamble = kind == OptionKind::Gamble;
                    st.shop_for = Some((guid, class, ctx.world.store_serial));
                    st.shop_request = Some((guid, class));
                }
                if matches!(kind, OptionKind::Identify | OptionKind::Resurrect) {
                    st.menu_state = MENU_WAITING;
                }
                drop(st);
                if let Some(o) = option_intent(kind, guid) {
                    self.out(o);
                }
                UiResponse::Consumed
            }
        }
    }

    /// The confirm dialog `0x004B2F50` (`menus.md` §4.4): Yes / No by
    /// `confirm_answer`. The pointer and press handling are the menu
    /// box's (d2rs-own, module doc).
    fn confirm_event(&mut self, e: UiEvent) -> UiResponse {
        let Some(at) = e.at() else {
            return UiResponse::Ignored;
        };
        let item = {
            let shop = self.shop.borrow();
            let Some(c) = shop.confirm.as_ref() else {
                return UiResponse::Ignored;
            };
            item_at(&c.bx, at)
        };
        let mut shop = self.shop.borrow_mut();
        let Some(c) = shop.confirm.as_mut() else {
            return UiResponse::Ignored;
        };
        match e {
            UiEvent::CursorMoved(_) => {
                if let Some(i) = item {
                    c.bx.selected = i as i32;
                }
                return UiResponse::Ignored;
            }
            UiEvent::Press {
                button: PointerButton::Left,
                ..
            } => {
                c.pressed = item;
                return UiResponse::Consumed;
            }
            UiEvent::Release {
                button: PointerButton::Left,
                ..
            } => {}
            _ => return UiResponse::Consumed,
        }
        let pressed = c.pressed.take();
        let Some(i) = item.filter(|&i| pressed == Some(i)) else {
            return UiResponse::Consumed;
        };
        let yes = c.bx.items[i].handler == Some(ConfirmHandler::Yes);
        let c = shop.confirm.take().expect("checked above");
        match confirm_answer(c.kind, shop.tx.menu_state, yes) {
            ConfirmResult::Send => {
                let effects = shop.tx.send_with(0, c.now, &c.facts);
                let mut out = Vec::new();
                for e in effects {
                    match e {
                        ShopEffect::Send(i) => out.push(PanelOutput::Intent(i)),
                        ShopEffect::Sound(id) => out.push(PanelOutput::Sound(id as i32)),
                        ShopEffect::WaitingNote => shop.note = true,
                        ShopEffect::Confirm(_) | ShopEffect::Cancel => {}
                    }
                }
                drop(shop);
                self.out(out);
            }
            ConfirmResult::Cancel => {
                shop.tx.cancel();
                shop.tx.menu_state = 3;
            }
            ConfirmResult::Hire => {
                drop(shop);
                if let Some((npc, name)) = c.hire {
                    let mut h = self.hire.borrow_mut();
                    h.hired = true;
                    h.sent = true;
                    drop(h);
                    self.out(vec![PanelOutput::Intent(hire_intent(npc, name))]);
                }
            }
            ConfirmResult::HireList => {
                if let Some((npc, _)) = c.hire {
                    self.hire.borrow_mut().up = Some(npc);
                }
            }
            ConfirmResult::Nothing => {}
        }
        UiResponse::Consumed
    }
}

impl Panel for NpcMenuUi {
    fn id(&self) -> PanelId {
        NPC_MENU_PANEL
    }

    fn rect(&self) -> Rect {
        if self.shop.borrow().confirm.is_some() {
            return FRAME;
        }
        let st = self.st.borrow();
        let talking = st.talk.active;
        if st.bx.is_some() || talking {
            // The whole frame: a press outside the box ends the chat; a
            // press while talking ends the talk.
            FRAME
        } else {
            Rect::new(0, 0, 0, 0)
        }
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let st = self.st.borrow();
        let sh = self.sh.borrow();
        let m = Measure(sh.fonts.as_ref());
        let mut spin = 0;
        if let Some(bx) = &st.bx {
            push_menu_draws(bx.draw(&mut spin, &m), out);
        }
        if let Some(n) = &st.note {
            push_menu_draws(n.bx.draw(&mut spin, &m), out);
        }
        if let Some(c) = &self.shop.borrow().confirm {
            push_menu_draws(c.bx.draw(&mut spin, &m), out);
        }
        self.draw_talk(&st, &m, out);
        let _ = ctx;
    }

    fn hit(&self, p: Point) -> Option<WidgetId> {
        let st = self.st.borrow();
        let bx = st.bx.as_ref()?;
        ltrb_contains(&bx.rect(), p).then(|| WidgetId(item_at(bx, p).map_or(0, |i| i as u16 + 1)))
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        if self.shop.borrow().confirm.is_some() {
            return self.confirm_event(e);
        }
        let Some(guid) = self.st.borrow().up.as_ref().map(|it| it.guid) else {
            return UiResponse::Ignored;
        };
        if self.st.borrow().bx.is_none() {
            return self.talk_event(e, guid, ctx);
        }
        let (inside, item) = {
            let st = self.st.borrow();
            let Some(bx) = st.bx.as_ref() else {
                return UiResponse::Ignored;
            };
            match e.at() {
                Some(p) => (ltrb_contains(&bx.rect(), p), item_at(bx, p)),
                None => return UiResponse::Ignored,
            }
        };
        match e {
            UiEvent::CursorMoved(_) => {
                if let Some(i) = item {
                    if let Some(bx) = self.st.borrow_mut().bx.as_mut() {
                        bx.selected = i as i32;
                    }
                }
                UiResponse::Ignored
            }
            UiEvent::Press {
                button: PointerButton::Left,
                ..
            } => {
                if inside {
                    self.st.borrow_mut().pressed = item;
                    return UiResponse::Consumed;
                }
                let o = self.st.borrow_mut().end(guid);
                self.out(o);
                UiResponse::Consumed
            }
            UiEvent::Release {
                button: PointerButton::Left,
                ..
            } => {
                let pressed = self.st.borrow_mut().pressed.take();
                match item.filter(|&i| pressed == Some(i)) {
                    Some(i) => self.run(i, ctx),
                    None if inside => UiResponse::Consumed,
                    None => UiResponse::Ignored,
                }
            }
            _ if inside => UiResponse::Consumed,
            _ => UiResponse::Ignored,
        }
    }
}

impl OriginalUi {
    /// Opens the NPC menu for a delivered 0x28 (`msg_ui`; `menus.md`
    /// §2.2): `SetUIState(8, on)`, the table edits, the anchor; the box
    /// is built at the next [`Self::npc_menu_poll`] (it needs the
    /// strings).
    pub fn open_npc_menu(&mut self, guid: u32, class: u32, level: i32, world: &ClientWorld) {
        let n = unidentified_count(world);
        self.open_npc_menu_with(guid, class, level, n, world);
    }

    /// [`Self::open_npc_menu`] with Cain's count of items to identify.
    pub fn open_npc_menu_with(
        &mut self,
        guid: u32,
        class: u32,
        level: i32,
        identify_n: u32,
        world: &ClientWorld,
    ) {
        self.talk_list();
        // A seller's hire list (S→C 0x4F) opens from the Hire option.
        self.hire.borrow_mut().up = None;
        let _ = self.set_ui(u32::from(id::NPC_MENU), 0, false);
        let open_mode = self.open_mode().get();
        let mut st = self.npcm.borrow_mut();
        st.anchor = npc_anchor(world, guid, open_mode);
        st.start(guid, class, level, identify_n);
    }

    /// Per UI frame, before the events: builds an asked-for box with the
    /// frame's strings (§2.2), queues its messages on the root, rebuilds
    /// the box the hire list's Back asked for (§3.2) and drops the menu
    /// when ui 8 went off.
    pub fn npc_menu_poll(
        &mut self,
        world: &ClientWorld,
        root: &mut UiRoot,
        strings: &dyn StringLookup,
    ) {
        let (back, sent) = {
            let mut h = self.hire.borrow_mut();
            (std::mem::take(&mut h.back), std::mem::take(&mut h.sent))
        };
        if back {
            if let Some(it) = self.npcm.borrow_mut().up.as_mut() {
                it.build = true;
            }
        }
        let now = (world.frames as u32).wrapping_mul(40);
        if sent {
            // §3.4: `[0x007C0C6B]` := 10 and the waiting note (§2.7). The
            // clock is the client frame count at 40 ms (d2rs-own, as
            // `game_messages`).
            self.npcm.borrow_mut().menu_state = MENU_WAITING;
            self.open_waiting_note(strings, now);
        }
        // A shop send's waiting note (§4.3).
        if std::mem::take(&mut self.shop.borrow_mut().note) {
            self.open_waiting_note(strings, now);
        }
        // §3.4 `0x004B3610`: the list closed, kind 5, the confirm dialog
        // at the mouse.
        let hire_confirm = self.hire.borrow_mut().confirm.take();
        if let Some((npc, name)) = hire_confirm {
            let sh = self.shared.borrow();
            let m = Measure(sh.fonts.as_ref());
            let s = string_fn(strings);
            let frame = (sh.config.screen.w, sh.config.screen.h);
            let mouse = (sh.mouse.x, sh.mouse.y);
            if let Ok(bx) = confirm_box(TxKind::Hire, None, mouse, &s, frame, &m) {
                self.shop.borrow_mut().confirm = Some(super::shop_ui::ConfirmUp {
                    bx,
                    kind: TxKind::Hire,
                    facts: Default::default(),
                    now,
                    hire: Some((npc, name)),
                    pressed: None,
                });
            }
        }
        if !self.is_open(id::NPC_MENU) {
            let mut st = self.npcm.borrow_mut();
            if st.up.is_some() {
                st.close();
            }
        }
        {
            let sh = self.shared.borrow();
            let m = Measure(sh.fonts.as_ref());
            let mut st = self.npcm.borrow_mut();
            if st.up.as_ref().is_some_and(|it| it.build) {
                st.build(world, strings, &m);
            }
        }
        self.talk_poll(world, strings);
        let pending = std::mem::take(&mut self.npcm.borrow_mut().pending);
        for o in pending {
            match o {
                PanelOutput::Intent(i) => root.queue_intent(i),
                PanelOutput::SetUi { ui, mode, jump } => {
                    let _ = self.set_ui(u32::from(ui), u32::from(mode), jump);
                }
                PanelOutput::Sound(id) => self
                    .outcome
                    .sounds
                    .push(crate::audio::driver::SoundRequest::Ui(id)),
            }
        }
        self.sync_root(root);
    }

    /// S→C 0x2A at the UI (`msg-ui.md` §18 r2, `panels-2.md` §14.10): in
    /// the waiting state the note closes and, on result 3 or 6, Cain's
    /// counts reset and the menu is rebuilt; result 5 changes nothing
    /// here; any other result sets state 12 (its note caption is open,
    /// §14.10). Outside it, result 1 plays UI sound 221.
    pub(super) fn npc_transaction(&mut self, bytes: &[u8; 15]) {
        let result = bytes[2];
        let mut st = self.npcm.borrow_mut();
        if st.menu_state != MENU_WAITING {
            drop(st);
            if bytes[2] == 1 {
                self.outcome
                    .sounds
                    .push(crate::audio::driver::SoundRequest::Ui(SOUND_TX_DONE));
            }
            return;
        }
        st.note = None;
        match result {
            5 => {}
            3 | 6 => {
                let class = st.up.as_ref().map_or(0, |it| it.class);
                let menus = st.menus();
                let mut recs = menus.records().to_vec();
                cain_count_reset(&mut recs, class, result);
                *menus = NpcMenus::from_records(recs);
                st.menu_state = 1;
                if let Some(it) = st.up.as_mut() {
                    // The rebuild's identify caption counts again.
                    it.build = true;
                }
            }
            _ => st.menu_state = MENU_FAILED,
        }
    }

    /// The NPC menu (built, asked for, or talking) or the imbue dialog is
    /// up (the preview's automatic chat close waits for them).
    pub fn npc_menu_up(&self) -> bool {
        let st = self.npcm.borrow();
        let menu = st
            .up
            .as_ref()
            .is_some_and(|it| it.build || st.talk.active || st.bx.is_some());
        menu || st.talk.socket.is_some()
    }

    /// The open menu (built box or talk), for tests and the host.
    pub fn npc_menu(&self) -> Option<Open> {
        self.npcm.borrow().open_view()
    }

    /// The centre of the selectable row `k` of the built box (tests: the
    /// box sits above the NPC, `menus.md` §2.4–§2.6).
    pub fn npc_menu_row_point(&self, k: usize) -> Option<Point> {
        let st = self.npcm.borrow();
        let i = st.row_item(k)?;
        let r = item_rect(st.bx.as_ref()?, i);
        Some(Point::new((r.l + r.r) / 2, (r.t + r.b) / 2))
    }

    /// The built box's rectangle and its items' (text, color), for tests.
    pub fn npc_menu_box(&self) -> Option<(Ltrb, Vec<BoxItem>)> {
        let st = self.npcm.borrow();
        let bx = st.bx.as_ref()?;
        Some((
            bx.rect(),
            bx.items.iter().map(|i| (i.text.clone(), i.color)).collect(),
        ))
    }

    /// The waiting state `[0x007C0C6B]` (tests).
    pub fn npc_menu_state(&self) -> u8 {
        self.npcm.borrow().menu_state
    }

    /// The text of the waiting note's item, when one is up.
    pub fn npc_waiting_note(&self) -> bool {
        self.npcm.borrow().note.is_some()
    }

    /// §2.7: the waiting note at the NPC's anchor (also after 0x36,
    /// `0x004B1F01`); a second one is refused (fatal in 1.14d).
    pub(super) fn open_waiting_note(&mut self, strings: &dyn StringLookup, now: u32) {
        let sh = self.shared.borrow();
        let m = Measure(sh.fonts.as_ref());
        let mut st = self.npcm.borrow_mut();
        let at = st.anchor.unwrap_or((st.screen.0 / 2, st.screen.1 / 3));
        let text = string_fn(strings)(STR_WAITING);
        if let Ok(n) = WaitingNote::open(st.note.as_ref(), at, &text, now, &m) {
            st.note = Some(n);
        }
    }
}

#[cfg(test)]
#[path = "npc_box_tests.rs"]
mod tests;
