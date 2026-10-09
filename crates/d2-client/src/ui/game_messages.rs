// Spec: specs/ui/messages.md (§2 r1–r5, §3), specs/client/msg-ui.md (§1 r2, §4 r3)
//! Game messages in play: the screen message list (`0x0049E3A0`) fed by
//! S→C 0x26 chat lines and the quest screen messages of S→C 0x5D, drawn
//! at the top left as dark-backed lines for 10 s. The rules are
//! [`crate::ui::messages::chat`]; this adapter only owns the list in the
//! UI's shared state, the font metrics and the draw requests.
// d2rs-own, unverified: the client frame counts 40 ms (the 25 Hz client
// frame) as the `GetTickCount()` of the expiry; the text of a chat line is
// read as Latin-1 (REC-ui-chat-filter code page); the backing is the
// HUD's dark fill tiles instead of `DrawRectangle`.

use super::hud::{FILL_FILE, FILL_H, FILL_W};
use crate::bridge::output::Output;
use crate::ui::draw::{ImageRef, ImageRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect};
use crate::ui::messages::chat::{
    chat_action, ChatAction, ChatMessage, ChatStrings, ScreenMessages,
};
use crate::ui::messages::{Metrics, FONT_CHAT};
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::text::TextOpts;

use super::{FontMeasure, SharedRef};

/// The adapter's id: not a UI state, open for good.
pub const MESSAGES_PANEL: PanelId = PanelId(0x111);

/// Milliseconds one client frame adds to the expiry clock (module doc).
pub const MS_PER_FRAME: u32 = 40;
/// The fill file's dark frame (`hud::fill_frames`).
const DARK: u32 = 4;
/// UI states the draw gates read (`ui/panels.md` §2).
const UI_STATE_13: u8 = 0x13;
const UI_MESSAGE_LOG: u8 = 0x18;

/// The wide text of 8-bit message bytes (Latin-1, module doc).
pub(super) fn wide(b: &[u8]) -> Vec<u16> {
    b.iter().map(|&c| u16::from(c)).collect()
}

/// The screen message list, and the string ids (quest messages) waiting
/// for the string table of the next draw.
#[derive(Clone, Debug, Default)]
pub struct GameMessages {
    list: ScreenMessages,
    pending_ids: Vec<(u16, u32)>,
    strings: Option<ChatStrings>,
}

/// Font metrics from the loaded tables; without them a line is not
/// wrapped and a unit is 8 pixels wide.
pub(crate) struct Measure<'a>(pub(crate) Option<&'a FontMeasure>);

impl Metrics for Measure<'_> {
    fn wrap(&self, font: u16, text: &[u16], max: i32) -> Vec<Vec<u16>> {
        self.0
            .and_then(|f| f.wrap(font, text, max))
            .unwrap_or_else(|| vec![text.to_vec()])
    }
    fn width_a(&self, font: u16, text: &[u16]) -> i32 {
        self.0
            .and_then(|f| f.width_a(font, text))
            .unwrap_or(8 * text.len() as i32)
    }
    fn width_c(&self, font: u16, text: &[u16]) -> i32 {
        self.0
            .and_then(|f| f.width_c(font, text))
            .unwrap_or(8 * text.len() as i32)
    }
    fn font_height(&self, _font: u16) -> i32 {
        16
    }
}

impl GameMessages {
    /// The list (tests, the message log).
    pub fn list(&self) -> &ScreenMessages {
        &self.list
    }

    /// `0x0049E3A0(text, color)` at client frame `frames`.
    pub fn add(
        &mut self,
        text: &[u16],
        color: u32,
        frames: u64,
        w: i32,
        log_open: bool,
        fonts: Option<&FontMeasure>,
    ) {
        let now = (frames as u32).wrapping_mul(MS_PER_FRAME);
        self.list
            .add(text, color, now, w, log_open, &Measure(fonts));
    }

    /// A screen message of string table entry `id` (S→C 0x5D rows), shown
    /// at the next draw, which has the string table.
    pub fn add_string_id(&mut self, id: u16, color: u32) {
        self.pending_ids.push((id, color));
    }

    /// The line a delivered 0x26 adds (`messages.md` §3 table), `None`
    /// for the types that add none (overhead text, recipe scroll, the
    /// refused rows).
    pub fn chat_line_text(&mut self, o: &Output) -> Option<(Vec<u16>, u32)> {
        let Output::ChatLine {
            kind,
            unit,
            b8,
            name,
            text,
            present,
            ..
        } = o
        else {
            return None;
        };
        let (name, text) = (wide(name), wide(text));
        let msg = ChatMessage {
            kind: *kind,
            b3: unit.unit_type,
            c8: *b8,
            name: &name,
            name_bytes: name.len(),
            text: &text,
            text_bytes: text.len(),
            raw_text: &text,
            unit_present: *present,
        };
        let strings = self.strings.get_or_insert_with(ChatStrings::default);
        match chat_action(&msg, strings) {
            Ok(ChatAction::Line { text, color }) => Some((text, color)),
            _ => None,
        }
    }
}

impl super::OriginalUi {
    /// The game messages the delivered `o` shows (`messages.md` §2, §3;
    /// `msg-ui.md` §1 r2 rows 3 and 15, §4 r3). The 0x26 gates (squelch,
    /// conversion, the overhead record) stay in [`Self::chat_line`].
    pub(super) fn game_message(&mut self, o: &Output, frames: u64) {
        let (w, log_open) = {
            let sh = self.shared.borrow();
            (sh.config.screen.w, sh.states.is_open(UI_MESSAGE_LOG))
        };
        if matches!(o, Output::ChatLine { .. }) {
            if self.chat_line(o, false).is_none() {
                return;
            }
            let mut sh = self.shared.borrow_mut();
            let sh = &mut *sh;
            if let Some((text, color)) = sh.messages.chat_line_text(o) {
                sh.messages
                    .add(&text, color, frames, w, log_open, sh.fonts.as_ref());
            }
        }
    }

    /// A screen message of string `id`, colour 0 (`msg-ui.md` §1 r2).
    pub(super) fn game_message_id(&mut self, id: u16) {
        self.shared.borrow_mut().messages.add_string_id(id, 0);
    }
}

/// The panel that draws the list (module doc).
pub(super) struct MessagesUi {
    pub(super) sh: SharedRef,
}

impl Panel for MessagesUi {
    fn id(&self) -> PanelId {
        MESSAGES_PANEL
    }

    fn rect(&self) -> Rect {
        Rect::new(0, 0, 0, 0)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let mut sh = self.sh.borrow_mut();
        let sh = &mut *sh;
        let now = (ctx.tick as u32).wrapping_mul(MS_PER_FRAME);
        let (w, open_mode) = (sh.config.screen.w, sh.states.open_mode().get());
        let clip = sh.config.screen.rect();
        let (state_13, log_open) = (
            sh.states.is_open(UI_STATE_13),
            sh.states.is_open(UI_MESSAGE_LOG),
        );
        let m = Measure(sh.fonts.as_ref());
        for (id, color) in std::mem::take(&mut sh.messages.pending_ids) {
            if let Some(t) = ctx.strings.get_id(id) {
                sh.messages.list.add(t, color, now, w, log_open, &m);
            }
        }
        // §2 r4: the list is drawn only while the message log is closed.
        if !log_open {
            let fill = sh.tables.files.id(FILL_FILE);
            for (r, l) in sh.messages.list.draw(w, open_mode, state_13, &m) {
                if let Some(file) = fill {
                    backing(file, &r, out);
                }
                out.push(UiDraw::Text(TextRequest {
                    text: l.text,
                    at: Point::new(l.x, l.y),
                    style: TextStyle {
                        font: FONT_CHAT,
                        color: u16::try_from(l.color).unwrap_or(0),
                    },
                    opts: TextOpts::default(),
                    clip,
                }));
            }
        }
        // §2 r5.
        sh.messages.list.expire(now);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, _e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        UiResponse::Ignored
    }
}

/// The dark text backing of rectangle `r` as fill tiles.
pub(super) fn backing(file: u32, r: &crate::ui::messages::RectDraw, out: &mut dyn UiDrawSink) {
    let (tw, th) = (FILL_W as i32, FILL_H as i32);
    let mut y = r.y;
    while y < r.y + r.h {
        let mut x = r.x;
        while x < r.x + r.w {
            let (cw, ch) = ((r.x + r.w - x).min(tw), (r.y + r.h - y).min(th));
            out.push(UiDraw::Image(ImageRequest {
                image: ImageRef { file, frame: DARK },
                at: Point::new(x, y),
                clip: Rect::new(x, y, cw as u16, ch as u16),
                look: crate::ui::CelLook::PLAIN,
            }));
            x += tw;
        }
        y += th;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientWorld, UnitKey, PLAYER};
    use crate::ui::layout::Screen;
    use crate::ui::original::{OriginalUi, UiConfig};
    use crate::ui::{NoPanelRules, StringLookup, UiRoot};

    struct Strs;
    impl StringLookup for Strs {
        fn get(&self, _: &str) -> Option<&[u16]> {
            None
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            const T: [u16; 4] = [b'Q' as u16, b'u' as u16, b'e' as u16, b's' as u16];
            (id == 3708).then_some(&T[..])
        }
    }

    fn setup() -> (OriginalUi, UiRoot, ClientWorld) {
        setup_at(Screen::R800)
    }

    fn setup_at(screen: Screen) -> (OriginalUi, UiRoot, ClientWorld) {
        let config = UiConfig {
            screen,
            expansion_installed: true,
        };
        let ui = OriginalUi::new(config, None).unwrap();
        let mut root = UiRoot::new(Box::new(NoPanelRules));
        ui.install(&mut root).unwrap();
        (ui, root, ClientWorld::default())
    }

    fn chat(kind: u8, unit_type: u8, c8: u8, name: &str, text: &str) -> Output {
        Output::ChatLine {
            kind,
            lang: 0,
            unit: UnitKey::new(unit_type, 4),
            b8: c8,
            b9: 0,
            name: name.as_bytes().to_vec(),
            text: text.as_bytes().to_vec(),
            present: false,
            player_name: None,
        }
    }

    fn lines(root: &UiRoot, w: &ClientWorld, tick: u64) -> Vec<(String, i32, i32, u16)> {
        let ctx = UiCtx {
            tick,
            world: w,
            strings: &Strs,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) if t.style.font == FONT_CHAT => Some((
                    String::from_utf16_lossy(&t.text),
                    t.at.x,
                    t.at.y,
                    t.style.color,
                )),
                _ => None,
            })
            .collect()
    }

    // Covers: specs/ui/messages.md §2 r1
    #[test]
    fn the_list_is_clipped_to_the_screen_at_640_and_800() {
        // §2 r1: x 15, y 20 on both frames; only the clip is the screen.
        for screen in [Screen::R800, Screen::R640] {
            let (mut ui, root, mut w) = setup_at(screen);
            w.frames = 10;
            ui.apply_output(&chat(4, PLAYER, 1, "", "Hi"), &w).unwrap();
            let ctx = UiCtx {
                tick: 10,
                world: &w,
                strings: &Strs,
            };
            let mut out: Vec<UiDraw> = Vec::new();
            root.draw(&ctx, &mut out);
            let t: Vec<_> = out
                .iter()
                .filter_map(|d| match d {
                    UiDraw::Text(t) if t.style.font == FONT_CHAT => Some((t.at, t.clip)),
                    _ => None,
                })
                .collect();
            assert_eq!(t, [(Point::new(15, 20), screen.rect())], "{screen:?}");
        }
    }

    // Covers: specs/ui/messages.md §2 r1, §2 r4, §2 r5
    #[test]
    fn a_chat_line_is_drawn_at_the_top_left_for_ten_seconds() {
        let (mut ui, root, mut w) = setup();
        assert!(lines(&root, &w, 0).is_empty());
        w.frames = 10;
        ui.apply_output(&chat(4, PLAYER, 1, "", "You cannot use this"), &w)
            .unwrap();
        // Font 13, x 15, y 20, colour c8; the 8 px fallback metric.
        let v = lines(&root, &w, 10);
        assert_eq!(v, vec![("You cannot use this".to_string(), 15, 20, 1)]);
        // A second line steps 15 px.
        ui.apply_output(&chat(4, PLAYER, 0, "", "Second"), &w)
            .unwrap();
        assert_eq!(lines(&root, &w, 11).len(), 2);
        assert_eq!(lines(&root, &w, 11)[1].2, 35);
        // 10 s = 250 frames of 40 ms: gone after the expiry.
        // §2 r5: the expiry runs at the end of the draw that finds it.
        assert_eq!(lines(&root, &w, 10 + 251).len(), 2);
        assert!(lines(&root, &w, 10 + 252).is_empty());
    }

    // Covers: specs/ui/messages.md §3 r1, §3 r3
    #[test]
    fn a_named_line_and_a_whisper_take_the_formats_of_the_table() {
        let (mut ui, root, w) = setup();
        ui.apply_output(&chat(1, 2, 2, "Bob", "hi"), &w).unwrap();
        ui.apply_output(&chat(2, 0, 0, "Eve", "psst"), &w).unwrap();
        let v = lines(&root, &w, 0);
        assert_eq!(v[0].0, "\u{ff}c4Bob\u{ff}c0: hi");
        assert_eq!(v[1].0, "Eve whispers: psst");
        assert_eq!(v[1].3, 2);
    }

    // Covers: specs/client/msg-ui.md §1 r2
    #[test]
    fn a_quest_screen_message_shows_its_string() {
        let (mut ui, root, w) = setup();
        ui.apply_output(
            &Output::QuestUi {
                chain: 3,
                flags: 1,
                status: 0,
                extra: 0,
            },
            &w,
        )
        .unwrap();
        assert_eq!(lines(&root, &w, 0), vec![("Ques".to_string(), 15, 20, 0)]);
    }
}
