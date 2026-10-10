// Spec: specs/ui/panels-3.md (§23)
//! The mouse cursor machine (`panels-3.md` §23): the seven cursor types,
//! the state `s`, type `t` and frame `f` (8.8 fixed point) changes on
//! mouse move, button down / up and cursor-item changes, the timed step
//! after each draw, and the draw (item graphic or type cel). Plain state;
//! the Bevy layer feeds the events and the time and draws the result.

use thiserror::Error;

/// One record of the cursor type table `0x00712010` (§23 r1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorType {
    pub name: &'static str,
    pub animated: bool,
    pub loops: bool,
    pub frames: u32,
    /// 1/256 frame per step.
    pub step: u32,
    /// Animates on press.
    pub press: bool,
    /// Draw function: 0x004683C0 (types 0–5) or 0x00468460 (type 6).
    pub shop_draw: bool,
}

const fn ct(
    name: &'static str,
    animated: bool,
    loops: bool,
    frames: u32,
    step: u32,
    press: bool,
    shop_draw: bool,
) -> CursorType {
    CursorType {
        name,
        animated,
        loops,
        frames,
        step,
        press,
        shop_draw,
    }
}

/// The 7 types: Gaunt, grasp, ohand, orotate, ppress, protate, buysell.
pub const TYPES: [CursorType; 7] = [
    ct("Gaunt", false, false, 1, 0, true, false),
    ct("grasp", true, false, 8, 0, true, false),
    ct("ohand", true, false, 8, 0x40, true, false),
    ct("orotate", true, true, 8, 0x20, true, false),
    ct("ppress", true, true, 8, 0x40, true, false),
    ct("protate", true, true, 8, 0x40, true, false),
    ct("buysell", false, false, 1, 0, false, true),
];

/// The cel path of type `t` (§23 r1).
pub fn cel_path(t: usize) -> String {
    format!("data\\global\\ui\\cursor\\{}", TYPES[t].name)
}

/// A state the step does not know (§23 r8: fatal).
#[derive(Debug, Error, PartialEq, Eq)]
#[error("cursor step in state {0}: fatal in the original")]
pub struct CursorFatal(pub u8);

/// What the mouse-move handler did (§23 r4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveResult {
    /// The position was clamped and the OS cursor moved there.
    Clamped {
        x: i32,
        y: i32,
    },
    Done,
}

/// What a draw puts on the screen (§23 r9–r11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorDraw {
    /// The cursor item's inventory graphic, top-left at (x, y).
    Item { x: i32, y: i32 },
    /// A cel frame at (x, y), light 0xFF, draw mode 5, no remap.
    Cel { t: u8, frame: u32, x: i32, y: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// `[0x007A6B08]`.
    pub drawn: bool,
    pub mx: i32,
    pub my: i32,
    pub adj: i32,
    pub t: u8,
    /// 8.8 fixed point, except type 6.
    pub f: i32,
    pub s: u8,
    pub last: u32,
    pub idle: u32,
    /// An item unit (type 4) is on the cursor.
    pub item: bool,
}

/// One step of the D2 generator on a client seed (`sim/rng.md`): low word
/// × 0x6AC690C5 + high word, 64-bit.
pub fn seed_step(seed: u64) -> u64 {
    (seed & 0xFFFF_FFFF)
        .wrapping_mul(0x6AC6_90C5)
        .wrapping_add(seed >> 32)
}

impl Cursor {
    /// Init `0x004680B0` (§23 r3): drawn := 1, s := 1, t := 5, f := 0,
    /// idle := now, mouse := (W / 2, H / 2).
    pub fn init(adj: i32, w: i32, h: i32, now: u32) -> Self {
        Self {
            drawn: true,
            mx: w / 2,
            my: h / 2,
            adj,
            t: 5,
            f: 0,
            s: 1,
            last: 0,
            idle: now,
            item: false,
        }
    }

    /// Mouse move `0x00468840` (§23 r4). `clip` = `0x004F6270()` ≠ 0 and
    /// `0x00407FF0()` = 0.
    pub fn mouse_move(
        &mut self,
        x: i32,
        y: i32,
        now: u32,
        w: i32,
        h: i32,
        clip: bool,
    ) -> MoveResult {
        self.mx = x;
        self.my = y;
        self.drawn = true;
        if clip && !((0..w).contains(&x) && (0..h).contains(&y)) {
            self.mx = x.clamp(0, w - 1);
            self.my = y.clamp(0, h - 1);
            return MoveResult::Clamped {
                x: self.mx,
                y: self.my,
            };
        }
        self.idle = now;
        if self.s == 4 || self.s == 2 {
            self.s = 3;
            self.t = 2;
            self.f = 8 * 256 - 256;
        }
        MoveResult::Done
    }

    /// `WM_NCMOUSEMOVE` `0x00467EF0` (§23 r14): not drawn over the
    /// non-client area. Returns whether the OS cursor call `0x004F59F0(0)`
    /// is due (hit-test code 2, `HTCAPTION`).
    pub fn nc_mouse_move(&mut self, hit_test: u32) -> bool {
        self.drawn = false;
        hit_test == 2
    }

    /// Button down `0x00467F20` (§23 r5).
    pub fn button_down(&mut self, x: i32, y: i32, now: u32) {
        self.mx = x;
        self.my = y;
        self.idle = now;
        if self.s != 5 && TYPES[usize::from(self.t)].press {
            self.s = 5;
            self.t = 4;
            self.f = 0;
        }
    }

    /// Button up `0x00467FA0` (§23 r6).
    pub fn button_up(&mut self, x: i32, y: i32, now: u32) {
        self.mx = x;
        self.my = y;
        self.idle = now;
        if self.s == 5 {
            self.s = 1;
            self.t = 5;
            self.f = 0;
        }
    }

    /// Cursor item `0x00468070` (§23 r7): t := 5, f := 0, s := 4 with an
    /// item, else 1.
    pub fn set_item(&mut self, item: bool) {
        self.item = item;
        self.t = 5;
        self.f = 0;
        self.s = if item { 4 } else { 1 };
    }

    /// Shop cursor with an item `0x00468010(u, f0)`: t := 6, f := f0, s := 6.
    pub fn shop_item(&mut self, f0: i32) {
        self.item = true;
        self.t = 6;
        self.f = f0;
        self.s = 6;
    }

    /// Shop cursor without an item `0x00468040(f0, flag)`: t := 6, f :=
    /// f0, s := 7 + (flag ≠ 0).
    pub fn shop_empty(&mut self, f0: i32, flag: bool) {
        self.item = false;
        self.t = 6;
        self.f = f0;
        self.s = 7 + u8::from(flag);
    }

    /// Step `0x00468310` (§23 r8), run after every type-cel draw.
    /// `player`: a local player exists; `seed`: its client seed.
    pub fn step(&mut self, now: u32, player: bool, seed: &mut u64) -> Result<(), CursorFatal> {
        let ty = TYPES[usize::from(self.t)];
        let span = (ty.frames * 256) as i32;
        if ty.animated && now > self.last.wrapping_add(16) {
            self.last = now;
            if self.s == 3 {
                self.f -= 0x40;
                if self.f < 0 {
                    if ty.loops {
                        self.f += span;
                    } else {
                        self.s = 1;
                        self.t = 5;
                        self.f = 0;
                    }
                }
            } else if player {
                if self.s == 1 {
                    *seed = seed_step(*seed);
                    if (*seed as u32) & 0x3F < 16 {
                        self.f += 0x20;
                    }
                } else {
                    self.f += ty.step as i32;
                }
                if self.f >= span {
                    if ty.loops {
                        self.f -= span;
                    } else if self.s == 2 {
                        self.s = 4;
                        self.t = 3;
                        self.f = 0;
                    } else if self.s == 5 {
                        self.s = 1;
                        self.t = 5;
                        self.f = 0;
                    } else {
                        return Err(CursorFatal(self.s));
                    }
                }
            }
        }
        if self.s == 1 && now > self.idle.wrapping_add(5000) {
            self.s = 2;
            self.t = 2;
            self.f = 0;
        }
        Ok(())
    }

    /// Draw `0x004684C0` (§23 r9–r11), once per drawn frame after the UI
    /// pass. `item_gfx` = the cursor item's inventory graphic frame size
    /// when an item unit is on the cursor. Nothing while not drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        w: i32,
        h: i32,
        item_gfx: Option<(u32, u32)>,
        now: u32,
        player: bool,
        seed: &mut u64,
    ) -> Result<Option<CursorDraw>, CursorFatal> {
        let (d, step) = self.place(w, h, item_gfx);
        if step {
            self.step(now, player, seed)?;
        }
        Ok(d)
    }

    /// The draw of [`Self::draw`] without its step: what is drawn, and
    /// whether the step `0x00468310` follows it (types 0–5, no cursor
    /// item). The caller runs [`Self::step`] after the frame's other seed
    /// draws (`client/model.md` Randomness r4: the cursor step is the
    /// frame's last).
    pub fn place(
        &self,
        w: i32,
        h: i32,
        item_gfx: Option<(u32, u32)>,
    ) -> (Option<CursorDraw>, bool) {
        if !self.drawn {
            return (None, false);
        }
        if let (true, Some((gw, gh))) = (self.s < 6, item_gfx) {
            // no step runs, so the cursor frame stays where it was
            let d = CursorDraw::Item {
                x: self.adj + self.mx - (gw / 2) as i32,
                y: self.my - (gh / 2) as i32,
            };
            return (Some(d), false);
        }
        if TYPES[usize::from(self.t)].shop_draw {
            // type 6: frame f (not shifted) at (adj + mx, my + 33); no
            // clamp, no step
            let d = CursorDraw::Cel {
                t: self.t,
                frame: self.f as u32,
                x: self.adj + self.mx,
                y: self.my + 33,
            };
            return (Some(d), false);
        }
        // types 0–5: x = clamp(mx + adj, adj, W − adj − 1), y = clamp(my,
        // 0, H − 1), frame f >> 8; then the step
        let d = CursorDraw::Cel {
            t: self.t,
            frame: (self.f >> 8) as u32,
            x: (self.mx + self.adj).clamp(self.adj, w - self.adj - 1),
            y: self.my.clamp(0, h - 1),
        };
        (Some(d), true)
    }
}

#[cfg(test)]
mod tests;
