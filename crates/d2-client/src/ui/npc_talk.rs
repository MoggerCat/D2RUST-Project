// Spec: specs/ui/messages.md (§6 r1–r3, §7 r1–r8, §11 r1–r6), specs/ui/panels-2.md (§14 r8, §14 r9)
//! Talk and the NPC services in play, on the spec modules: the menu's Talk
//! closes the box and opens the talk topic box
//! ([`npc_text::topic_box`], `messages.md` §6 r3) at the NPC's anchor; a
//! topic plays its text in the dialog panel ([`DialogUi`], §7: top centre
//! 325 wide, scrolled by `timeGetTime`, skipped by a button press); the
//! topic box's cancel is the talk sequence's end (`panels-2.md` §14.8:
//! the menu is built again, or the interaction ends). Charsi's imbue is
//! the item-socket dialog ([`SocketDialog`], §11, ui 0x0E, NPC mode) with
//! its own panel.
// d2rs-own, unverified (M22; each named where it is used):
// - the topic captions `0x00722678` and the intro table `0x00726850` with
//   its text records come from `facts/ui/npc-talk-*.tsv`
//   (`messages::npc_facts`); "gossip" plays the record the gossip index
//   picks (§6 r4–r5) with a seed copied from the local player's unit seed
//   at the first use and advanced here only (not written back to the
//   unit), and game quest 12 bit 13 read as clear (no stock effect);
//   "introduction" plays text record 0 of the entry (REC-727, PROVISIONAL:
//   the handler `0x004B41E0` is not read; settled by a Ghidra read, see
//   `docs/handoff/pc1-data.md` Step 4); the greeting of the menu open
//   (§13 r3) is taken to give a sound in mode 2 (REC-728, PROVISIONAL:
//   the sound existence `0x004E0590 != 0` is the audio layer's), so the
//   C→S 0x4D goes out with it;
// - the end callback `0x004B18C0` of an NPC topic opens the topic box
//   again; the topic box's cancel `0x004B5810` is the talk end of §14.8;
//   "about the merchants" and "Horadric Cube" do nothing (handlers not
//   specified);
// - the clock of the scroll and the skip is the client frame count at
//   40 ms; the speech is not waited for (no speech playback yet, §7 r5);
//   the panel border (`menu\boxpieces`) is not drawn;
// - the imbue dialog opens from the menu's Imbue row (REC-145; the 1.14d
//   opener is S→C 0x58 code 0): ui 8 goes off first (C[8][0x0E] refuses
//   0x0E), the cursor item stays on the model's cursor while placed, the
//   NPC accept check is the server's, and the placed item is not drawn.

use d2_sim::rng::Seed;

use super::game_messages::Measure;
use super::npc_box::{push_menu_draws, NpcMenuState, NpcMenuUi, SharedNpcMenu};
use super::{OriginalUi, SharedRef};
use crate::bridge::items;
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::ui::draw::{CelLook, UiDrawSink};
use crate::ui::draw::{ImageRef, ImageRequest, RectRequest, TextRequest, TextStyle, UiDraw};
use crate::ui::geom::{Point, Rect};
use crate::ui::messages::dialog::{
    panel_draw, DialogDraw, DialogEffect, DialogOpen, DialogUi, PassInput, SkipEvent, PANEL_FONT,
};
use crate::ui::messages::intro::{GossipCtx, GossipOutcome, IntroTable};
use crate::ui::messages::npc_facts::{caption_pairs, intro_table};
use crate::ui::messages::npc_text::{topic_box, TextList, TopicHandler, TopicInput};
use crate::ui::messages::socket::{
    background_file, CursorItem, DrawEnv, Mode, SocketDialog, SocketDraw, SocketEffect,
    SocketFrame, Step, BUTTON_CEL, UI_SOCKET,
};
use crate::ui::messages::{Metrics, FONT_16};
use crate::ui::panel::WidgetId;
use crate::ui::panel::{ClientIntent, Panel, PanelId, StringLookup, UiCtx, UiEvent, UiResponse};
use crate::ui::panels::menu_box::MenuBox;
use crate::ui::panels::npc::msg_chat_end;
use crate::ui::panels::PanelOutput;
use crate::ui::states::id;
use crate::ui::text::TextOpts;
use crate::ui::PointerButton;

/// The item-socket dialog's panel id: its UI state (0x0E).
pub const SOCKET_PANEL: PanelId = PanelId(UI_SOCKET as u16);
/// The intro table's no-introduction classes (`messages.md` §13 r1).
const NO_INTRO: [u32; 5] = crate::ui::messages::intro::NO_INTRO;

/// The talk of the open interaction.
#[derive(Default)]
pub struct TalkState {
    /// The talk topic box (`[0x007C0D6F]`).
    pub topic: Option<MenuBox<TopicHandler>>,
    topic_pressed: Option<usize>,
    /// The dialog panel's globals (§7).
    pub dialog: DialogUi,
    /// The NPC text list `[0x007BF250]` as §6 r1 builds it.
    pub list: Option<TextList>,
    /// The talk is running: topic box or dialog (the menu box is closed).
    pub active: bool,
    /// The topic box is asked for (built at the next poll, it needs the
    /// strings).
    pub reopen: bool,
    /// The item-socket dialog (§11).
    pub socket: Option<SocketDialog>,
    /// The intro table `0x00726850` (§13), loaded from the facts at the
    /// first use.
    intro: Option<IntroTable>,
    /// The local unit seed copied at the first gossip (d2rs-own).
    seed: Option<Seed>,
}

impl TalkState {
    /// The intro table, loaded from the facts on first use.
    pub fn intro(&mut self) -> &mut IntroTable {
        self.intro.get_or_insert_with(intro_table)
    }
}

/// The client frame clock (d2rs-own, module doc).
fn now(world: &ClientWorld) -> u32 {
    (world.frames as u32).wrapping_mul(40)
}

fn strings_of(s: &dyn StringLookup) -> impl Fn(u16) -> Vec<u16> + '_ {
    move |id| s.get_id(id).map(<[u16]>::to_vec).unwrap_or_default()
}

/// The topic box of §6 r3 for the interaction's NPC.
fn build_topic(
    st: &NpcMenuState,
    world: &ClientWorld,
    strings: &dyn StringLookup,
    m: &dyn Metrics,
) -> Option<MenuBox<TopicHandler>> {
    let class = st.npc_class()?;
    let has_cube = items::local_items(world)
        .iter()
        .any(|i| i.code == Some(*b"box ") && !i.store);
    let input = TopicInput {
        list: st.talk.list.as_ref(),
        no_intro: NO_INTRO.contains(&class),
        npc_class: class,
        has_cube,
        caption_table: caption_pairs(),
        box_up: false,
    };
    let s = strings_of(strings);
    topic_box(&input, st.anchor_or_centre(), &s, st.screen, m).ok()
}

/// The text id a topic plays (§6 r3–r5): a replayed quest topic its list
/// entry; "introduction" text record 0 of the NPC's intro entry (REC-727);
/// "gossip" the record the gossip index picks (the first click re-rolls
/// every entry's index).
fn topic_text(
    st: &mut NpcMenuState,
    h: TopicHandler,
    class: u32,
    player: (u8, Option<(u32, u32)>),
    quest: &[u8; 96],
) -> Option<u16> {
    let (player_class, seed) = player;
    match h {
        TopicHandler::Replay(k) => st.talk.list.as_ref()?.nth_of_kind(2, k).map(|e| e.string),
        TopicHandler::Introduction => {
            let i = st.talk.intro().index_of(class)?;
            st.talk.intro().entries[i].records.first().map(|r| r.text())
        }
        TopicHandler::Gossip => {
            let talk = &mut st.talk;
            let seed = talk.seed.get_or_insert_with(|| {
                let (lo, hi) = seed.unwrap_or((1, 666));
                Seed::new(lo, hi)
            });
            let gate = |record: u32, bit: u32| {
                u32::from(crate::bridge::objects::quest_bit(
                    quest,
                    record as u8,
                    bit as u8,
                ))
            };
            let ctx = GossipCtx {
                class: player_class,
                quest_bit: &gate,
                game_quest12_bit13: false,
            };
            let table = talk.intro.get_or_insert_with(intro_table);
            let i = table.index_of(class)?;
            match table.gossip_click(i, true, seed, &ctx) {
                GossipOutcome::Play { text } => text,
                GossipOutcome::Ended => None,
            }
        }
        _ => None,
    }
}

/// A topic box item's hit band (the menu box's, `npc_box`).
fn topic_item_at(bx: &MenuBox<TopicHandler>, p: Point) -> Option<usize> {
    super::npc_box::item_at(bx, p)
}

impl NpcMenuUi {
    /// Talk (`0x004B6C70`, §14.9): the menu box is closed already; the
    /// topic box opens.
    pub(super) fn talk_open(&mut self, ctx: &UiCtx) {
        let sh = self.sh.borrow();
        let m = Measure(sh.fonts.as_ref());
        let mut st = self.st.borrow_mut();
        st.menu_state = 1;
        st.talk.active = true;
        st.talk.topic = build_topic(&st, ctx.world, ctx.strings, &m);
        st.talk.topic_pressed = None;
    }

    /// Events while the talk runs: the dialog's skip input (§7 r7: a
    /// button down skips, a right button up is only consumed), else the
    /// topic box.
    pub(super) fn talk_event(&mut self, e: UiEvent, guid: u32, ctx: &UiCtx) -> UiResponse {
        if !self.st.borrow().talk.active {
            return UiResponse::Ignored;
        }
        let t = now(ctx.world);
        if self.st.borrow().talk.dialog.panel.is_some() {
            let ev = match e {
                UiEvent::Press { .. } => SkipEvent::ButtonDown,
                UiEvent::Release {
                    button: PointerButton::Right,
                    ..
                } => SkipEvent::RightUp,
                UiEvent::Release { .. } => return UiResponse::Consumed,
                _ => return UiResponse::Ignored,
            };
            let out = self.st.borrow_mut().talk.dialog.skip_event(ev, t);
            self.dialog_effects(out.effects);
            return if out.consumed {
                UiResponse::Consumed
            } else {
                UiResponse::Ignored
            };
        }
        let Some(at) = e.at() else {
            return UiResponse::Ignored;
        };
        let item = {
            let st = self.st.borrow();
            let Some(bx) = st.talk.topic.as_ref() else {
                return UiResponse::Ignored;
            };
            topic_item_at(bx, at)
        };
        match e {
            UiEvent::CursorMoved(_) => {
                if let (Some(i), Some(bx)) = (item, self.st.borrow_mut().talk.topic.as_mut()) {
                    bx.selected = i as i32;
                }
                UiResponse::Ignored
            }
            UiEvent::Press {
                button: PointerButton::Left,
                ..
            } => {
                self.st.borrow_mut().talk.topic_pressed = item;
                UiResponse::Consumed
            }
            UiEvent::Release {
                button: PointerButton::Left,
                ..
            } => {
                let pressed = self.st.borrow_mut().talk.topic_pressed.take();
                let Some(i) = item.filter(|&i| pressed == Some(i)) else {
                    return UiResponse::Consumed;
                };
                let h = self
                    .st
                    .borrow()
                    .talk
                    .topic
                    .as_ref()
                    .and_then(|b| b.items[i].handler);
                if let Some(h) = h {
                    self.topic_chosen(h, guid, ctx);
                }
                UiResponse::Consumed
            }
            _ => UiResponse::Consumed,
        }
    }

    fn topic_chosen(&mut self, h: TopicHandler, guid: u32, ctx: &UiCtx) {
        match h {
            TopicHandler::Cancel => self.talk_end(guid, ctx),
            TopicHandler::Introduction | TopicHandler::Gossip | TopicHandler::Replay(_) => {
                let class = self.st.borrow().npc_class().unwrap_or(0);
                let player = ctx
                    .world
                    .local()
                    .map_or((0, None), |u| (u.class as u8, u.seed));
                let quest = self.sh.borrow().client_quest;
                let id = topic_text(&mut self.st.borrow_mut(), h, class, player, &quest);
                let Some(id) = id else {
                    return;
                };
                let text = strings_of(ctx.strings)(id);
                let (w, h) = self.st.borrow().screen;
                let s = self.sh.borrow().config.screen;
                let effects = {
                    let mut st = self.st.borrow_mut();
                    // The topic box is freed (§6 r4) and the text plays
                    // (`0x004A10E0`, §7 r1) with the NPC talk's end
                    // callback `0x004B18C0`.
                    st.talk.topic = None;
                    let fx = st.talk.dialog.open(
                        DialogOpen::Npc {
                            unit: Some(guid),
                            id: u32::from(id),
                        },
                        &text,
                        w,
                        h,
                        s.sx(),
                        s.sy(),
                        now(ctx.world),
                    );
                    st.talk.dialog.end_callback = true;
                    fx
                };
                self.dialog_effects(effects);
            }
            _ => {}
        }
    }

    /// §14.8: the talk sequence ended with no next message.
    fn talk_end(&mut self, guid: u32, ctx: &UiCtx) {
        let o = self.st.borrow_mut().talk_end(guid, ctx.world);
        self.out_pub(o);
    }

    /// The dialog's effects that reach the rest of the UI.
    pub(super) fn dialog_effects(&mut self, effects: Vec<DialogEffect>) {
        let o = self.st.borrow_mut().dialog_effects(effects);
        self.out_pub(o);
    }

    /// The talk's draws: the topic box, then the dialog panel (§7 r4).
    pub(super) fn draw_talk(&self, st: &NpcMenuState, m: &dyn Metrics, out: &mut dyn UiDrawSink) {
        let mut spin = 0;
        if let Some(bx) = &st.talk.topic {
            push_menu_draws(bx.draw(&mut spin, m), st.screen_rect(), out);
        }
        let d = &st.talk.dialog;
        let Some(p) = d.panel.as_ref() else {
            return;
        };
        let font_h = m.font_height(PANEL_FONT);
        let clip = st.screen_rect();
        for dr in panel_draw(d.x, d.y, !d.place1, p.scroll.p, &p.lines, font_h) {
            match dr {
                DialogDraw::Backing(r) => {
                    out.push(UiDraw::Rect(RectRequest::sized(
                        r.x, r.y, r.w, r.h, r.color, r.mode,
                    )));
                }
                DialogDraw::Border { .. } => {}
                DialogDraw::Line(l, _) => out.push(UiDraw::Text(TextRequest {
                    text: l.text,
                    at: Point::new(l.x, l.y),
                    style: TextStyle {
                        font: PANEL_FONT,
                        color: 0,
                    },
                    opts: TextOpts::default(),
                    clip,
                })),
                DialogDraw::Window {
                    line,
                    x,
                    y,
                    skip,
                    lines,
                } => out.push(UiDraw::Text(TextRequest {
                    text: p.lines[line].clone(),
                    at: Point::new(x, y),
                    style: TextStyle {
                        font: PANEL_FONT,
                        color: 0,
                    },
                    opts: TextOpts::Vertical { skip, lines },
                    clip,
                })),
            }
        }
    }
}

impl NpcMenuState {
    /// §14.8 (`0x004B6A30`): rebuild the menu or end the interaction.
    pub(super) fn talk_end(&mut self, guid: u32, world: &ClientWorld) -> Vec<PanelOutput> {
        use crate::ui::panels::npc::{talk_end, TalkEnd};
        let class = self.npc_class().unwrap_or(0);
        let flag = self.record_flag();
        let present = world.units.contains_key(&UnitKey::new(1, guid));
        self.talk.topic = None;
        self.talk.active = false;
        match talk_end(present, class, flag) {
            TalkEnd::RebuildMenu => {
                self.menu_state = 1;
                self.ask_build();
                Vec::new()
            }
            TalkEnd::EndInteraction => self.end_interaction(guid),
        }
    }

    /// Applies the dialog's effects: its sends; the NPC talk's end
    /// callback opens the topic box again (module doc).
    pub(super) fn dialog_effects(&mut self, effects: Vec<DialogEffect>) -> Vec<PanelOutput> {
        let mut out = Vec::new();
        for e in effects {
            match e {
                DialogEffect::Send(i) => out.push(PanelOutput::Intent(i)),
                DialogEffect::EndCallback { .. } => {
                    self.talk.dialog.end_callback = false;
                    self.talk.dialog.panel = None;
                    if self.talk.active {
                        self.talk.reopen = true;
                    }
                }
                _ => {}
            }
        }
        out
    }
}

impl OriginalUi {
    /// The talk's frame step (from [`Self::npc_menu_poll`]): the dialog
    /// pass (§7 r6) and a topic box asked for again.
    pub(super) fn talk_poll(&mut self, world: &ClientWorld, strings: &dyn StringLookup) {
        let pending = {
            let mut st = self.npcm.borrow_mut();
            let mut out = Vec::new();
            if st.talk.dialog.up {
                let r = st.talk.dialog.pass(&PassInput {
                    now: now(world),
                    speech_waiting: false,
                    end_callback_result: 0,
                    timed_box_drawn: false,
                    local_dead_or_absent: world.local().is_none_or(|u| u.is_dead()),
                    unit_in_mode12: false,
                });
                out = st.dialog_effects(r.effects);
            }
            out
        };
        self.npcm.borrow_mut().push_pending(pending);
        let reopen = std::mem::take(&mut self.npcm.borrow_mut().talk.reopen);
        if reopen {
            let sh = self.shared.borrow();
            let m = Measure(sh.fonts.as_ref());
            let mut st = self.npcm.borrow_mut();
            st.talk.topic = build_topic(&st, world, strings, &m);
        }
    }

    /// 0x27 type 1 built a new text list (§6 r1), or the list was freed.
    pub(super) fn talk_list(&mut self) {
        let list = self.npc_text().and_then(|l| TextList::build(l).ok());
        self.npcm.borrow_mut().talk.list = list;
    }

    /// A delivered S→C 0x58 (§11 r2–r3): codes other than 0 reach the open
    /// dialog; its effects leave at the next poll.
    pub(super) fn imbue_output(&mut self, o: &crate::bridge::output::Output) {
        let crate::bridge::output::Output::OpenUi { code, arg, .. } = *o else {
            return;
        };
        let mut st = self.npcm.borrow_mut();
        let Some(d) = st.talk.socket.as_mut() else {
            return;
        };
        let guid = d.npc_guid;
        let fx = d.code(code, u32::from(arg));
        let out = socket_effects(fx, guid);
        if st
            .talk
            .socket
            .as_ref()
            .is_some_and(|d| d.step == Step::Closed)
        {
            st.talk.socket = None;
        }
        st.push_pending(out);
    }

    /// Places `guid` in the open item-socket dialog (headless-test seam:
    /// the fixture's model has no item stream for the cursor item).
    pub fn imbue_place(&mut self, guid: u32) {
        if let Some(d) = self.npcm.borrow_mut().talk.socket.as_mut() {
            d.step = Step::Placed;
            d.placed = guid;
        }
    }

    /// The talk's topic box items' text, when one is up (tests).
    pub fn npc_topics(&self) -> Option<Vec<Vec<u16>>> {
        let st = self.npcm.borrow();
        let bx = st.talk.topic.as_ref()?;
        Some(bx.items.iter().map(|i| i.text.clone()).collect())
    }

    /// The centre of the topic box's selectable item `k` (tests).
    pub fn npc_topic_point(&self, k: usize) -> Option<Point> {
        let st = self.npcm.borrow();
        let bx = st.talk.topic.as_ref()?;
        let i = (0..bx.items.len())
            .filter(|&i| bx.items[i].selectable)
            .nth(k)?;
        let r = super::npc_box::item_rect(bx, i);
        Some(Point::new((r.l + r.r) / 2, (r.t + r.b) / 2))
    }

    /// The centre of the topic box's cancel (its last selectable item),
    /// when one is up (tests).
    pub fn npc_topic_cancel_point(&self) -> Option<Point> {
        let n = {
            let st = self.npcm.borrow();
            let bx = st.talk.topic.as_ref()?;
            bx.items.iter().filter(|i| i.selectable).count()
        };
        self.npc_topic_point(n.checked_sub(1)?)
    }

    /// The dialog panel's lines, when it is up (tests).
    pub fn npc_dialog_lines(&self) -> Option<Vec<Vec<u16>>> {
        let st = self.npcm.borrow();
        Some(st.talk.dialog.panel.as_ref()?.lines.clone())
    }

    /// The item-socket dialog's step, when it is up (tests).
    pub fn socket_step(&self) -> Option<Step> {
        Some(self.npcm.borrow().talk.socket.as_ref()?.step)
    }
}

/// The socket dialog's effects for the UI (`msg_u32s`, sounds, states).
fn socket_effects(fx: Vec<SocketEffect>, npc: u32) -> Vec<PanelOutput> {
    let mut out = Vec::new();
    for e in fx {
        match e {
            SocketEffect::Send(i) => out.push(PanelOutput::Intent(i)),
            SocketEffect::Sound(id) => out.push(PanelOutput::Sound(id as i32)),
            SocketEffect::Ui(o) => out.push(o),
            // `0x004B3FE0` / `0x004B3FB0`: the interaction ends.
            SocketEffect::EndInteraction | SocketEffect::EndInteractionFull => {
                out.push(PanelOutput::Intent(ClientIntent(
                    msg_chat_end(npc).to_vec(),
                )));
                out.push(PanelOutput::SetUi {
                    ui: id::NPC_MENU,
                    mode: 1,
                    jump: false,
                });
            }
            _ => {}
        }
    }
    out
}

/// Opens the item-socket dialog for the NPC (module doc: the Imbue row):
/// ui 8 off, then §11 r2 with the open accepted.
pub(super) fn open_imbue(
    st: &mut NpcMenuState,
    guid: u32,
    class: u32,
    world: &ClientWorld,
) -> Vec<PanelOutput> {
    let mut d = SocketDialog::new();
    let player = world.local().map(|u| u.key.guid);
    let fx = d.open(Mode::Npc, 0, (guid, class), player, true, now(world));
    st.talk.socket = Some(d);
    let mut out = vec![PanelOutput::SetUi {
        ui: id::NPC_MENU,
        mode: 1,
        jump: false,
    }];
    out.extend(socket_effects(fx, guid));
    out
}

/// The files the socket dialog draws (§11 r1).
pub(super) fn socket_files() -> [String; 3] {
    [
        background_file(Mode::Npc).to_string(),
        background_file(Mode::Object).to_string(),
        BUTTON_CEL.to_string(),
    ]
}

/// The item-socket dialog's panel (ui 0x0E): the left half above the
/// control panel; the inventory keeps the right half (§11 r5: the
/// inventory handler sees a click first).
pub struct SocketUi {
    pub(super) sh: SharedRef,
    pub(super) st: SharedNpcMenu,
}

impl SocketUi {
    fn apply(&self, fx: Vec<SocketEffect>) {
        let mut st = self.st.borrow_mut();
        let guid = st.talk.socket.as_ref().map_or(0, |d| d.npc_guid);
        let out = socket_effects(fx, guid);
        if st
            .talk
            .socket
            .as_ref()
            .is_some_and(|d| d.step == Step::Closed)
        {
            st.talk.socket = None;
        }
        drop(st);
        self.sh.borrow_mut().outputs.extend(out);
    }
}

impl Panel for SocketUi {
    fn id(&self) -> PanelId {
        SOCKET_PANEL
    }

    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, 320, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let m = Measure(sh.fonts.as_ref());
        let strings = strings_of(ctx.strings);
        let frame = {
            let mut st = self.st.borrow_mut();
            let Some(d) = st.talk.socket.as_mut() else {
                return;
            };
            let env = DrawEnv {
                w: sh.config.screen.w,
                h: sh.config.screen.h,
                mouse: (sh.mouse.x, sh.mouse.y),
                player_dead_or_absent: ctx.world.local().is_none_or(|u| u.is_dead()),
                item_size: None,
                strings: &strings,
            };
            d.draw(&env, &m)
        };
        let draws = match frame {
            Ok(SocketFrame::Draw(d)) => d,
            Ok(SocketFrame::Close(fx)) => {
                drop(sh);
                self.apply(fx);
                return;
            }
            Err(_) => return,
        };
        let file = |n: &str| sh.tables.files.id(n);
        let clip = sh.config.screen.rect();
        for d in draws {
            match d {
                SocketDraw::Background { mode, x, y } => {
                    if let Some(f) = file(background_file(mode)) {
                        out.push(cel(f, 0, x, y, clip));
                    }
                }
                SocketDraw::Button { frame, x, y } => {
                    if let Some(f) = file(BUTTON_CEL) {
                        out.push(cel(f, frame, x, y, clip));
                    }
                }
                SocketDraw::HoverBox(r) => out.push(UiDraw::Rect(RectRequest::sized(
                    r.x, r.y, r.w, r.h, r.color, r.mode,
                ))),
                SocketDraw::Text(l) => out.push(UiDraw::Text(TextRequest {
                    text: l.text,
                    at: Point::new(l.x, l.y),
                    style: TextStyle {
                        font: FONT_16,
                        color: u16::try_from(l.color).unwrap_or(0),
                    },
                    opts: TextOpts::default(),
                    clip,
                })),
                // The placed item's cel (module doc: not drawn).
                SocketDraw::Item { .. } => {}
            }
        }
    }

    fn hit(&self, p: Point) -> Option<WidgetId> {
        self.rect().contains(p).then_some(WidgetId(0))
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        if self.st.borrow().talk.socket.is_none() {
            return UiResponse::Ignored;
        }
        let player = ctx.world.local().map(|u| u.key.guid);
        let fx = match e {
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            } => {
                let cursor = items::cursor_item(ctx.world).map(|c| CursorItem {
                    guid: c.key.guid,
                    // The server checks the type and the NPC's accept
                    // (module doc).
                    type_ok: true,
                    npc_accepts: true,
                    is_staff: c.code == Some(*b"hst "),
                });
                let mut st = self.st.borrow_mut();
                let d = st.talk.socket.as_mut().expect("checked above");
                d.left_down(now(ctx.world), (at.x, at.y), false, cursor).0
            }
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            } => {
                let sh = self.sh.borrow();
                let m = Measure(sh.fonts.as_ref());
                let strings = strings_of(ctx.strings);
                let mut st = self.st.borrow_mut();
                let d = st.talk.socket.as_mut().expect("checked above");
                d.left_up((at.x, at.y), player, &strings, &m)
            }
            UiEvent::Press { .. } | UiEvent::Release { .. } => Vec::new(),
            _ => return UiResponse::Ignored,
        };
        self.apply(fx);
        UiResponse::Consumed
    }
}

fn cel(file: u32, frame: u32, x: i32, y: i32, clip: Rect) -> UiDraw {
    UiDraw::Image(ImageRequest {
        image: ImageRef { file, frame },
        at: Point::new(x, y),
        clip,
        look: CelLook::PLAIN,
        call: crate::ui::draw::CelCall::Draw,
    })
}

#[cfg(test)]
#[path = "npc_talk_tests.rs"]
mod tests;
