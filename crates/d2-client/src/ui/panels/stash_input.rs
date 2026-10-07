// Spec: specs/ui/panels-2.md (§20), specs/ui/panels.md (§11 r7–r8, §12 r7–r8)
//! Stash and cube mouse input (`panels-2.md` §20): the press and release
//! rectangles, the pressed flags, the click sound and the C→S messages
//! with the number of 0x4F 0x12 / 0x17 each path sends.

use super::stash_cube::{cube_close, cube_transmute, stash_close, stash_close_pos, UI_CUBE};
use super::PanelOutput;
use crate::ui::geom::Point;
use crate::ui::layout::Screen;

/// The GoldMax text is drawn in font 1 (`Font16`, `panels-2.md` §20 r6):
/// the belt draw sets it and nothing in between changes it.
pub const GOLD_MAX_FONT: u16 = 1;

fn open(v: i32, lo: i32, hi: i32) -> bool {
    lo < v && v < hi
}

/// The stash gold button `0x00489920` (§20 r1, inclusive): x in [`sx +
/// 73`, `sx + 150`], y in [`H + sy − 455`, `H + sy − 438`] expansion,
/// [`H + sy − 259`, `H + sy − 242`] classic.
pub fn stash_gold_rect_hit(s: &Screen, exp: bool, p: Point) -> bool {
    let (y0, y1) = if exp {
        (s.h + s.sy() - 455, s.h + s.sy() - 438)
    } else {
        (s.h + s.sy() - 259, s.h + s.sy() - 242)
    };
    (s.sx() + 73..=s.sx() + 150).contains(&p.x) && (y0..=y1).contains(&p.y)
}

/// The stash close button `0x00489980` (strict, §20 r1): `X` < x < `X +
/// 40`, `Y − 35` < y < `Y + 5`.
pub fn stash_close_hit(s: &Screen, exp: bool, p: Point) -> bool {
    let (x, y) = stash_close_pos(s, exp);
    open(p.x, x, x + 40) && open(p.y, y - 35, y + 5)
}

/// The cube close button `0x00489FB0` (strict, §20 r2): `sx + 275` < x <
/// `sx + 315`, `H + sy − 100` < y < `H + sy − 60`.
pub fn cube_close_hit(s: &Screen, p: Point) -> bool {
    open(p.x, s.sx() + 275, s.sx() + 315) && open(p.y, s.h + s.sy() - 100, s.h + s.sy() - 60)
}

/// The transmute button `0x0048A000` (strict, §20 r2): `sx + 144` < x <
/// `sx + 184`, `H + sy − 223` < y < `H + sy − 183`.
pub fn transmute_hit(s: &Screen, p: Point) -> bool {
    open(p.x, s.sx() + 144, s.sx() + 184) && open(p.y, s.h + s.sy() - 223, s.h + s.sy() - 183)
}

/// The tool tip positions of the cube hovers (§20 r3): `strClose` at
/// (`sx + 289 − w / 2`, `H + sy − 100`), `strUiMenu2` at (`sx + 158 − w /
/// 2`, `H + sy − 223`), `w` the text width.
pub fn cube_tooltips(s: &Screen, close_w: i32, transmute_w: i32) -> (Point, Point) {
    (
        Point::new(s.sx() + 289 - close_w / 2, s.h + s.sy() - 100),
        Point::new(s.sx() + 158 - transmute_w / 2, s.h + s.sy() - 223),
    )
}

/// The `strClose` tool tip position of the stash hover (§20 r1): (`X + 12 −
/// w / 2`, `Y − 35`).
pub fn stash_close_tooltip(s: &Screen, exp: bool, w: i32) -> Point {
    let (x, y) = stash_close_pos(s, exp);
    Point::new(x + 12 - w / 2, y - 35)
}

/// What an input did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StashCubeEffect {
    pub consumed: bool,
    /// UI sound 4 (`0x004B9A00(4, 0, 0, 0)`).
    pub sound4: bool,
    pub outputs: Vec<PanelOutput>,
}

impl StashCubeEffect {
    fn click() -> Self {
        Self {
            consumed: true,
            sound4: true,
            outputs: Vec::new(),
        }
    }
}

/// The button flags of the stash / cube panels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StashCubeInput {
    /// `[0x007BCE90]`: inventory close pressed.
    pub inv_close: bool,
    /// `[0x007BCE34]`: stash gold button.
    pub stash_gold: bool,
    /// `[0x007BCE3C]` and pressed `[0x007BCE38]`: stash close.
    pub stash_close_flag: bool,
    pub stash_close_pressed: bool,
    /// `[0x007BCE44]` and pressed `[0x007BCE40]`: cube close.
    pub cube_close_flag: bool,
    pub cube_close_pressed: bool,
    /// `[0x007BCE4C]` and pressed `[0x007BCE48]`: transmute.
    pub transmute_flag: bool,
    pub transmute_pressed: bool,
    /// `[0x007BCE9C]`.
    pub latch_ce9c: bool,
    /// The cube close latch `[0x007BCC54]` (§12 r7); the open clears it.
    pub cube_close_sent: bool,
}

/// The pointer facts of one event.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub at: Point,
    /// In the inventory close rectangle (`0x00486E10`), the belt popup
    /// not covering it.
    pub in_inv_close: bool,
    pub cursor_item: bool,
}

impl StashCubeInput {
    /// Stash mouse down `0x00492510` (§20 r1), inventory modes 0x0C /
    /// 0x0D: the inventory close rectangle sets `[0x007BCE90]`; else the
    /// stash gold button sets `[0x007BCE34]`; else the stash close button
    /// sets `[0x007BCE3C]`, pressed and `[0x007BCE9C]`. Each: sound 4,
    /// consumed.
    pub fn stash_down(&mut self, s: &Screen, exp: bool, p: &Pointer) -> StashCubeEffect {
        if p.in_inv_close {
            self.inv_close = true;
        } else if stash_gold_rect_hit(s, exp, p.at) {
            self.stash_gold = true;
        } else if stash_close_hit(s, exp, p.at) {
            self.stash_close_flag = true;
            self.stash_close_pressed = true;
            self.latch_ce9c = true;
        } else {
            return StashCubeEffect::default();
        }
        StashCubeEffect::click()
    }

    /// Stash mouse up `0x00489AC0` (`panels.md` §11 r7): in the inventory
    /// close rectangle with no cursor item → `SetUIState(0x19, off, 0)`
    /// only (the hook sends one 0x4F 0x12); in the stash close rectangle
    /// with the button pressed → pressed := 0, `SetUIState` (one message)
    /// and 0x4F 0x12 again: two messages. Not pressed → nothing.
    pub fn stash_up(&mut self, s: &Screen, exp: bool, p: &Pointer) -> StashCubeEffect {
        let mut e = StashCubeEffect::default();
        if p.in_inv_close && !p.cursor_item {
            e.consumed = true;
            e.outputs = stash_close().outputs;
        } else if stash_close_hit(s, exp, p.at) && self.stash_close_pressed {
            self.stash_close_pressed = false;
            e.consumed = true;
            e.outputs = stash_close().outputs;
            e.outputs.extend(stash_close().outputs.into_iter().skip(1));
        }
        e
    }

    /// Cube mouse down `0x004927C0` (§20 r2), mode 0x0E.
    pub fn cube_down(&mut self, s: &Screen, p: &Pointer) -> StashCubeEffect {
        if p.in_inv_close {
            self.inv_close = true;
        } else if cube_close_hit(s, p.at) {
            self.cube_close_flag = true;
            self.cube_close_pressed = true;
            self.transmute_flag = false;
            self.transmute_pressed = false;
            self.latch_ce9c = true;
        } else if transmute_hit(s, p.at) {
            self.cube_close_flag = false;
            self.cube_close_pressed = false;
            if !p.cursor_item {
                self.transmute_flag = true;
                self.transmute_pressed = true;
                self.latch_ce9c = true;
                return StashCubeEffect::click();
            }
            // consumed either way, but no sound without the press
            return StashCubeEffect {
                consumed: true,
                ..Default::default()
            };
        } else {
            return StashCubeEffect::default();
        }
        StashCubeEffect::click()
    }

    /// Cube mouse up `0x0048A190` (§20 r3), mode 0x0E: `[0x007BCE90]` :=
    /// 0; the inventory close rectangle (belt popup closed, no cursor
    /// item) → `SetUIState(0x1A, off, 0)` (one 0x4F 0x17 through the
    /// close hook); the cube close rectangle with pressed → pressed := 0
    /// and the cube close; the transmute rectangle with pressed → pressed
    /// := 0, C→S 0x4F 0x18.
    pub fn cube_up(&mut self, s: &Screen, p: &Pointer) -> StashCubeEffect {
        self.inv_close = false;
        let mut e = StashCubeEffect::default();
        if p.in_inv_close && !p.cursor_item {
            e.consumed = true;
            e.outputs = self.cube_close_path();
        } else if cube_close_hit(s, p.at) && self.cube_close_pressed {
            self.cube_close_pressed = false;
            e.consumed = true;
            e.outputs = self.cube_close_path();
        } else if transmute_hit(s, p.at) && self.transmute_pressed {
            self.transmute_pressed = false;
            e.consumed = true;
            e.outputs = cube_transmute();
        }
        e
    }

    /// Any other mode on a cube mouse up: only `[0x007BCE9C]` := 0.
    pub fn cube_up_other_mode(&mut self) {
        self.latch_ce9c = false;
    }

    /// The cube open `0x0048A460` clears the close latch.
    pub fn cube_opened(&mut self) {
        self.cube_close_sent = false;
    }

    /// `0x0048A050` (`panels.md` §12 r7): `SetUIState(0x1A, off, 0)`, then
    /// C→S 0x4F 0x17 only if the latch is 0, setting it: one message per
    /// open, whichever path closes.
    pub fn cube_close_path(&mut self) -> Vec<PanelOutput> {
        let mut out = vec![PanelOutput::SetUi {
            ui: UI_CUBE,
            mode: 1,
            jump: false,
        }];
        if !self.cube_close_sent {
            self.cube_close_sent = true;
            out.extend(cube_close());
        }
        out
    }

    /// The cube-gone close (§20 r4, `0x0048EEB0`–`0x0048F183`): the draw
    /// calls `SetUIState(0x1A, off, 0)` (when ui 0x1A was open its close
    /// hook runs `0x0048A050`, which sends the latched 0x17), then
    /// `0x0048F183` sends 0x17 again unconditionally: two messages (one
    /// if ui 0x1A was already closed).
    pub fn cube_gone(&mut self, ui_was_open: bool) -> Vec<PanelOutput> {
        let mut out = if ui_was_open {
            self.cube_close_path()
        } else {
            Vec::new()
        };
        out.extend(cube_close());
        out
    }
}

#[cfg(test)]
mod tests;
