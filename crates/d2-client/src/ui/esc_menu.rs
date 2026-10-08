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
//! The tree, rows, input and settings are `options_menu` (spec
//! `ui/frontend-options.md` §O2–§O8); this panel draws it and routes
//! events. The DC6 art is `esc_art` (REC-257); a row without art (Window
//! Mode) is Font16 English text with a thin bar and gold knob stand-in
//! (REC-187). Esc closes the whole menu (`OriginalUi::game_menu_key`,
//! §O1 r4).

use super::hud::{FILL_FILE, FILL_H, FILL_W};
use super::options_menu::{Kind, MenuEvent, OptionsMenu, HALF};
use super::{left, SharedRef};
use crate::ui::draw::{ImageRef, ImageRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::{utf16, PanelOutput};
use crate::ui::text::TextOpts;
use crate::ui::FRAME;

/// The panel's id: the UI state number (§3.1).
pub const ESC_PANEL: PanelId = PanelId(9);

/// The fill file's frames (made by `hud::fill_frames`): gold, white, dark.
const GOLD: u32 = 1;
const WHITE: u32 = 3;
pub const DARK: u32 = 4;

/// Menu keys as UI `Char` events (the original's handler table, §O5 r1,
/// keyed by VK code): private-use code units, d2rs-own.
pub const CHAR_ENTER: u16 = 0x0D;
pub const CHAR_LEFT: u16 = 0xF025;
pub const CHAR_UP: u16 = 0xF026;
pub const CHAR_RIGHT: u16 = 0xF027;
pub const CHAR_DOWN: u16 = 0xF028;

/// Font16, and the colors of a row at rest, selected and disabled.
const FONT: u16 = 1;
const COLOR_REST: u16 = 0;
const COLOR_SELECTED: u16 = 3;
const COLOR_DISABLED: u16 = 5;

/// The menu's own state.
#[derive(Clone, Debug, Default)]
pub struct EscState {
    /// "Save and Exit Game" was chosen and the host has not read it yet.
    pub exit_requested: bool,
    /// "Configure Controls" was chosen and the host has not read it yet.
    pub controls_requested: bool,
    /// The Configure Controls screen, while it is open over the menu
    /// (`controls_host`).
    pub controls: Option<super::controls_host::ControlsHost>,
    /// Bindings accepted on that screen, until the host reads them.
    pub accepted: Option<crate::controls::Bindings>,
    /// The menu tree and the settings it edits (`options_menu`).
    pub menu: OptionsMenu,
}

impl EscState {
    /// Leave the Controls screen for the Options menu (Previous selected).
    pub fn close_controls(&mut self, f: super::controls_host::Finished) {
        use super::controls_host::Finished;
        self.controls = None;
        self.menu.return_from_controls();
        if let Finished::Accept(b) = f {
            self.accepted = Some(b);
        }
    }
}

/// A row's label x and the value / slider geometry, §O4 r1.
const LABEL_X: i32 = HALF - 230;
const VALUE_BLOCK: i32 = 130;

/// The menu draws over the world with no backdrop (§O4, PROVISIONAL REC-212),
/// so a dark strip behind the rows keeps the English stand-in text readable.
pub(super) fn push_fill(out: &mut dyn UiDrawSink, file: u32, frame: u32, r: Rect) {
    let (w, h) = (FILL_W as i32, FILL_H as i32);
    let mut y = r.y;
    while y < r.y + i32::from(r.h) {
        let mut x = r.x;
        while x < r.x + i32::from(r.w) {
            let cw = (r.x + i32::from(r.w) - x).min(w);
            let ch = (r.y + i32::from(r.h) - y).min(h);
            out.push(UiDraw::Image(ImageRequest {
                image: ImageRef { file, frame },
                at: Point::new(x, y),
                clip: Rect::new(x, y, cw as u16, ch as u16),
            }));
            x += w;
        }
        y += h;
    }
}

fn text(out: &mut dyn UiDrawSink, s: &str, at: Point, color: u16, centered_in: Option<i32>) {
    out.push(UiDraw::Text(TextRequest {
        text: utf16(s),
        at,
        style: TextStyle { font: FONT, color },
        opts: TextOpts::Draw {
            centered: centered_in.is_some(),
            block_w: centered_in,
            mode: 5,
        },
        clip: FRAME,
    }));
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

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        {
            let mut sh = self.sh.borrow_mut();
            let file = sh.tables.files.id(FILL_FILE);
            if let Some(c) = sh.esc.controls.as_mut() {
                c.draw(ctx, file, out);
                return;
            }
        }
        let sh = self.sh.borrow();
        let m = &sh.esc.menu;
        let file = sh.tables.files.id(FILL_FILE);
        for (i, def) in m.rows().iter().enumerate() {
            if super::esc_art::draw_row(&sh.tables.files, m, i, out) {
                continue;
            }
            let yb = m.baseline(i);
            let color = if !m.enabled(i) && def.kind != Kind::Title {
                COLOR_DISABLED
            } else if i == m.selected {
                COLOR_SELECTED
            } else {
                COLOR_REST
            };
            match def.kind {
                Kind::Title | Kind::Action => {
                    text(
                        out,
                        def.label,
                        Point::new(0, yb - 12),
                        color,
                        Some(super::options_menu::W),
                    );
                }
                Kind::Choice(_) => {
                    text(out, def.label, Point::new(LABEL_X, yb - 12), color, None);
                    let v = def.values[m.value(i) as usize];
                    let x = HALF + 230 - VALUE_BLOCK;
                    text(out, v, Point::new(x, yb - 12), color, Some(VALUE_BLOCK));
                }
                Kind::Slider { .. } => {
                    text(out, def.label, Point::new(LABEL_X, yb - 12), color, None);
                    if let Some(file) = file {
                        let y = m.slider_y(i);
                        // Track from h − 60 to h + 230, skull at h − 60 + t.
                        push_fill(out, file, WHITE, Rect::new(HALF - 60, y - 14, 290, 2));
                        push_fill(
                            out,
                            file,
                            GOLD,
                            Rect::new(HALF - 60 + m.slider_t(i), y - 20, 12, 14),
                        );
                    }
                }
            }
        }
        super::esc_art::draw_pents(&sh.tables.files, m, ctx.tick, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        let mut sh = self.sh.borrow_mut();
        if let Some(c) = sh.esc.controls.as_mut() {
            let done = match e {
                UiEvent::CursorMoved(p) => {
                    c.moved(p);
                    None
                }
                UiEvent::Wheel { steps, .. } => {
                    c.wheel(steps);
                    None
                }
                _ => match left(e) {
                    Some((true, at)) => c.press(at),
                    _ => None,
                },
            };
            if let Some(f) = done {
                sh.esc.close_controls(f);
            }
            return UiResponse::Consumed;
        }
        let m = &mut sh.esc.menu;
        let mut consumed = false;
        match e {
            UiEvent::CursorMoved(p) => m.moved(p),
            UiEvent::Char(c) => {
                consumed = true;
                match c {
                    CHAR_ENTER => m.key_enter(),
                    CHAR_LEFT => m.key_left(),
                    CHAR_UP => m.key_up(),
                    CHAR_RIGHT => m.key_right(),
                    CHAR_DOWN => m.key_down(),
                    _ => consumed = false,
                }
            }
            _ => {
                if let Some((down, at)) = left(e) {
                    if down {
                        m.press(at);
                    } else {
                        m.release(at);
                    }
                }
            }
        }
        for ev in m.take_events() {
            match ev {
                MenuEvent::CursorPass => {}
                MenuEvent::CursorSelect => sh.outputs.push(PanelOutput::ClickSound),
                MenuEvent::SaveAndExit => sh.esc.exit_requested = true,
                MenuEvent::Close => sh.outputs.push(PanelOutput::SetUi {
                    ui: 9,
                    mode: 1,
                    jump: false,
                }),
                MenuEvent::ConfigureControls => sh.esc.controls_requested = true,
            }
        }
        match e {
            UiEvent::Press { .. } | UiEvent::Release { .. } => UiResponse::Consumed,
            _ if consumed => UiResponse::Consumed,
            _ => UiResponse::Ignored,
        }
    }
}
