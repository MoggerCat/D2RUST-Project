// Spec: specs/ui/controls.md (§3 row 56 Esc), specs/ui/panels.md (§3.1 ui 9)
//! The in-game menu (ui state 9, the Esc menu) in play: a modal box with
//! "Options", "Save and Exit Game" and "Return to Game".
//!
//! What is specified: Esc (command 56) closes the menu when it is open,
//! else closes the open panels, else opens it (`controls.md` §3 row 56:
//! `0x004690B0`); the gate of state 9 (`panels.md` §3.1: no player, no
//! menu; a dead player respawns instead); the world takes no click while
//! it is open (`controls.md` §7 r2). The menu's art, layout and strings
//! have no spec yet (REC-237 in `docs/HANDOFF.md` §7, M22).
//!
//! Preview fills, each `// d2rs-own, unverified`:
//! - the box is tiles of the synthetic fill file ([`FILL_FILE`] frame
//!   [`DARK`]), the three entries are Font16 text in English;
//! - "Options" opens a second page (Resolution, Window Mode, Controls,
//!   Previous) over the d2rs config (`app::config`): the two rows cycle
//!   their value and set `settings_changed` for the host to write
//!   `settings.toml` (REC-172); "Controls" only names `controls.toml`
//!   (no Configure Controls screen yet); "Save and Exit Game" asks the host to save
//!   and close (`app::save::request_save_and_exit`, read through
//!   [`super::OriginalUi::take_exit_request`]); "Return to Game" closes.

use crate::app::config::Settings;

use super::hud::{FILL_FILE, FILL_H, FILL_W};
use super::{left, SharedRef};
use crate::ui::draw::{ImageRef, ImageRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::{utf16, PanelOutput};
use crate::ui::text::TextOpts;
use crate::ui::FRAME;

/// The panel's id: the UI state number (§3.1).
pub const ESC_PANEL: PanelId = PanelId(9);

/// The fill file's dark frame (made by `hud::fill_frames`).
pub const DARK: u32 = 4;

/// The frame is 800 × 600 (resolution mode 2).
const FRAME_W: i32 = 800;
const BOX_W: i32 = 260;
const BOX_TOP: i32 = 190;
const BOX_H: i32 = 215;
const ITEM_TOP: i32 = 210;
const ITEM_H: i32 = 45;
/// Font16, and the colors of an entry at rest and under the mouse.
const FONT: u16 = 1;
const COLOR_REST: u16 = 0;
const COLOR_HOVER: u16 = 3;

/// The entries, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Options,
    SaveAndExit,
    Return,
    Resolution,
    WindowMode,
    Controls,
    Previous,
}

/// Which page is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    Main,
    Options,
}

const MAIN: [Entry; 3] = [Entry::Options, Entry::SaveAndExit, Entry::Return];
const OPTIONS: [Entry; 4] = [
    Entry::Resolution,
    Entry::WindowMode,
    Entry::Controls,
    Entry::Previous,
];

/// The menu's own state.
#[derive(Clone, Debug, Default)]
pub struct EscState {
    /// "Save and Exit Game" was chosen and the host has not read it yet.
    pub exit_requested: bool,
    pub view: View,
    /// The settings the Options page shows and edits.
    pub settings: Settings,
    /// A setting changed and the host has not written it yet.
    pub settings_changed: bool,
}

impl EscState {
    fn entries(&self) -> &'static [Entry] {
        match self.view {
            View::Main => &MAIN,
            View::Options => &OPTIONS,
        }
    }

    fn label(&self, e: Entry) -> String {
        match e {
            Entry::Options => "Options".into(),
            Entry::SaveAndExit => "Save and Exit Game".into(),
            Entry::Return => "Return to Game".into(),
            Entry::Resolution => {
                let (w, h) = self.settings.window_size();
                format!("Resolution: {w}x{h}")
            }
            Entry::WindowMode => format!("Window Mode: {}", self.settings.window_mode.name()),
            Entry::Controls => "Controls: controls.toml".into(),
            Entry::Previous => "Previous".into(),
        }
    }
}

fn box_left() -> i32 {
    (FRAME_W - BOX_W) / 2
}

fn entry_rect(i: usize) -> Rect {
    Rect::new(
        box_left(),
        ITEM_TOP + ITEM_H * i as i32,
        BOX_W as u16,
        ITEM_H as u16,
    )
}

/// The entry under `p`.
pub fn entry_at(state: &EscState, p: Point) -> Option<Entry> {
    state
        .entries()
        .iter()
        .enumerate()
        .find(|(i, _)| entry_rect(*i).contains(p))
        .map(|(_, e)| *e)
}

/// The adapter (installed last: top-most, so it takes every click).
pub struct EscMenuUi {
    pub(super) sh: SharedRef,
}

impl Panel for EscMenuUi {
    fn id(&self) -> PanelId {
        ESC_PANEL
    }

    // Modal: the whole frame.
    fn rect(&self) -> Rect {
        FRAME
    }

    fn draw(&self, _ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        if let Some(file) = sh.tables.files.id(FILL_FILE) {
            let (w, h) = (FILL_W as i32, FILL_H as i32);
            let mut y = BOX_TOP;
            while y < BOX_TOP + BOX_H {
                let mut x = box_left();
                while x < box_left() + BOX_W {
                    let cw = (box_left() + BOX_W - x).min(w);
                    let ch = (BOX_TOP + BOX_H - y).min(h);
                    out.push(UiDraw::Image(ImageRequest {
                        image: ImageRef { file, frame: DARK },
                        at: Point::new(x, y),
                        clip: Rect::new(x, y, cw as u16, ch as u16),
                    }));
                    x += w;
                }
                y += h;
            }
        }
        for (i, entry) in sh.esc.entries().iter().enumerate() {
            let r = entry_rect(i);
            let label = sh.esc.label(*entry);
            let color = if entry_at(&sh.esc, sh.mouse) == Some(*entry) {
                COLOR_HOVER
            } else {
                COLOR_REST
            };
            out.push(UiDraw::Text(TextRequest {
                text: utf16(&label),
                at: Point::new(r.x, r.y + 30),
                style: TextStyle { font: FONT, color },
                opts: TextOpts::Draw {
                    centered: true,
                    block_w: Some(BOX_W),
                    mode: 5,
                },
                clip: FRAME,
            }));
        }
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        let mut sh = self.sh.borrow_mut();
        // Release on an entry chooses it; everything else is swallowed.
        if let Some((false, at)) = left(e) {
            match entry_at(&sh.esc, at) {
                Some(Entry::Options) => {
                    sh.outputs.push(PanelOutput::ClickSound);
                    sh.esc.view = View::Options;
                }
                Some(Entry::Previous) => {
                    sh.outputs.push(PanelOutput::ClickSound);
                    sh.esc.view = View::Main;
                }
                Some(Entry::Resolution) => {
                    sh.outputs.push(PanelOutput::ClickSound);
                    sh.esc.settings.resolution ^= 1;
                    sh.esc.settings_changed = true;
                }
                Some(Entry::WindowMode) => {
                    sh.outputs.push(PanelOutput::ClickSound);
                    sh.esc.settings.window_mode = sh.esc.settings.window_mode.next();
                    sh.esc.settings_changed = true;
                }
                Some(Entry::Controls) => {}
                Some(Entry::SaveAndExit) => {
                    sh.outputs.push(PanelOutput::ClickSound);
                    sh.esc.exit_requested = true;
                }
                Some(Entry::Return) => {
                    sh.outputs.push(PanelOutput::ClickSound);
                    sh.outputs.push(PanelOutput::SetUi {
                        ui: 9,
                        mode: 1,
                        jump: false,
                    });
                }
                None => {}
            }
        }
        match e {
            UiEvent::Press { .. } | UiEvent::Release { .. } => UiResponse::Consumed,
            _ => UiResponse::Ignored,
        }
    }
}
