// Spec: specs/ui/frontend-options.md (§O9), specs/ui/controls.md (§3.3, §5)
//! CONFIGURE CONTROLS over the game: the Esc menu's Options → Configure
//! Controls row opens the [`ConfigureControls`] model (the one the front
//! end hosts, `q-menu-controls`) while ui 9 stays open. The Esc menu panel
//! draws it instead of the menu, keys reach it raw
//! ([`super::OriginalUi::controls_key`]) and pointer events by the §O9 r2
//! geometry. Cancel and Accept return to the Options menu with Previous
//! Menu selected; Accept writes `controls.toml` and hands the new play
//! bindings to the host ([`super::OriginalUi::take_accepted_bindings`]).
//!
//! d2rs-own, unverified (REC-256): English stand-ins for strings the
//! table lookup does not give, rectangles for the border / scroll art,
//! the message clock (40 ms per frame tick), clicks act on press.

use std::path::PathBuf;

use super::esc_menu::DARK;
use crate::controls::keymap::action_of_cmd;
use crate::controls::Bindings;
use crate::ui::draw::{TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::front_end::screens::controls::{
    load_table, save_table, table_to_bindings, CfgDraw, ConfigureControls, Done, BTN_HALF, C, FONT,
    M, T, VISIBLE, W,
};
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::UiCtx;
use crate::ui::panels::utf16;
use crate::ui::text::TextOpts;
use crate::ui::FRAME;

/// The open screen.
#[derive(Clone, Debug)]
pub struct ControlsHost {
    model: ConfigureControls,
    path: Option<PathBuf>,
    pointer: Option<Point>,
}

/// What a key or click finished.
#[derive(Debug)]
pub enum Finished {
    Cancel,
    Accept(Bindings),
}

fn contains(x: i32, y: i32, w: i32, h: i32, p: Point) -> bool {
    (x..x + w).contains(&p.x) && (y..y + h).contains(&p.y)
}

fn stand_in(id: u32) -> &'static str {
    match id {
        3921 => "Function",
        3922 => "Key / Button One",
        3923 => "Key / Button Two",
        3762 => "None",
        3972 => "Default",
        3973 => "Accept",
        3974 => "Cancel",
        3978 => "Can't assign the mouse wheel here",
        3979 => "Can't assign this key",
        _ => "",
    }
}

impl ControlsHost {
    pub fn open(expansion: bool, path: Option<PathBuf>) -> Self {
        let table = load_table(path.as_deref());
        Self {
            model: ConfigureControls::open(expansion, table),
            path,
            pointer: None,
        }
    }

    pub fn model(&self) -> &ConfigureControls {
        &self.model
    }

    fn finish(&mut self, d: Done) -> Option<Finished> {
        match d {
            Done::Stay => None,
            Done::Cancel => Some(Finished::Cancel),
            Done::Accept => {
                if let Some(p) = &self.path {
                    // d2rs-own: a failed write is logged, the screen closes.
                    if let Err(e) = save_table(self.model.table(), p) {
                        eprintln!("controls: {e}");
                    }
                }
                Some(Finished::Accept(table_to_bindings(self.model.table())))
            }
        }
    }

    /// A key went down (`vk`: Windows virtual key; Esc is 27).
    pub fn key(&mut self, vk: u16, now_ms: u64) -> Option<Finished> {
        let was = self.model.editing();
        let d = self.model.key_down(vk, false, now_ms);
        // No key-up event reaches here: the release of the Enter that
        // began editing is swallowed (r5), as the front-end screen does.
        if !was && self.model.editing() {
            self.model.key_up(vk, now_ms);
        }
        self.finish(d)
    }

    pub fn moved(&mut self, p: Point) {
        self.pointer = Some(p);
    }

    pub fn wheel(&mut self, steps: i32) {
        self.model.wheel(steps * 120);
    }

    /// A left press at `p`: key cell, button or scroll arrow.
    pub fn press(&mut self, p: Point) -> Option<Finished> {
        self.pointer = Some(p);
        for r in 0..VISIBLE as i32 {
            let (top, h) = (T + 41 + 18 * r, 18);
            for (col, x, w) in [(1, M + 18, 2 * C), (0, M + 18 + 2 * C, C)] {
                if contains(x, top, w, h, p) {
                    self.model.click_cell(r as usize, col);
                    return None;
                }
            }
        }
        for i in 0..3 {
            let c = M + 206 * i + 103;
            if contains(c - BTN_HALF, T + 329, 2 * BTN_HALF, 38, p) {
                let d = match i {
                    0 => self.model.cancel(),
                    1 => {
                        self.model.default_all();
                        Done::Stay
                    }
                    _ => self.model.accept(),
                };
                return self.finish(d);
            }
        }
        for (down, y) in [(false, T + 47), (true, T + 305)] {
            if contains(W - M - 31, y, 12, 13, p) {
                self.model.scroll(down);
            }
        }
        None
    }

    pub fn draw(&mut self, ctx: &UiCtx, fill: Option<u32>, out: &mut dyn UiDrawSink) {
        let now = ctx.tick * 40;
        let pointer = self.pointer.map(|p| (p.x, p.y));
        let rows = self.model.rows().to_vec();
        let draws = self.model.draw_list(now, pointer);
        let label = |id: u32, lit: &str| -> String {
            if id == 0 {
                return lit.to_string();
            }
            if let Some(t) = u16::try_from(id).ok().and_then(|i| ctx.strings.get_id(i)) {
                return String::from_utf16_lossy(t);
            }
            if let Some(r) = rows.iter().find(|r| r.string_id == id) {
                if let Some(a) = action_of_cmd(r.cmd) {
                    return a.name().replace('_', " ");
                }
            }
            stand_in(id).to_string()
        };
        let mut rect = |c: u32, x: i32, y: i32, w: i32, h: i32| {
            if let Some(file) = fill {
                super::esc_menu::push_fill(
                    out,
                    file,
                    c,
                    Rect::new(x, y, w.max(1) as u16, h.max(1) as u16),
                );
            }
        };
        let mut texts = Vec::new();
        for d in draws {
            match d {
                CfgDraw::Rect { x, y, w, h } => rect(DARK, x, y, w, h),
                CfgDraw::Border { x, y, w, h } => {
                    rect(1, x, y, w, 1);
                    rect(1, x, y + h - 1, w, 1);
                    rect(1, x, y, 1, h);
                    rect(1, x + w - 1, y, 1, h);
                }
                CfgDraw::Slider { frame, x, y } => match frame {
                    10 | 11 => rect(1, x, y, 12, 13),
                    14 => rect(1, x, y, 12, 12),
                    _ => {}
                },
                CfgDraw::Text {
                    string_id,
                    text,
                    x,
                    y,
                    color,
                } => texts.push((label(string_id, &text), x, y, color)),
            }
        }
        for (t, x, y, color) in texts {
            // The buttons and the message pass their centre as x.
            let centred = y == T + 351 || y == T + 399;
            let (at_x, block) = if centred {
                (x - BTN_HALF * 2, Some(BTN_HALF * 4))
            } else {
                (x, None)
            };
            out.push(UiDraw::Text(TextRequest {
                text: utf16(&t),
                at: Point::new(at_x, y - 11),
                style: TextStyle {
                    font: FONT,
                    color: u16::from(color),
                },
                opts: TextOpts::Draw {
                    centered: centred,
                    block_w: block,
                    mode: 5,
                },
                clip: FRAME,
            }));
        }
    }
}
