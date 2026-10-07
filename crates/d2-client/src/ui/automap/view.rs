// Spec: specs/ui/automap.md (§8 r3–r4, §9)
//! View geometry (§9): the marker rectangle and mini origin, the panel
//! side, the drawing origin, plus the size setter and the re-centre of §8.

use super::options::Options;
use crate::rules::camera::{ClientPos, PANEL_HEIGHT};

/// Scale divisor `[0x00711254]` (§8 r3).
pub const DIV_FULL: i32 = 10;
pub const DIV_MINI: i32 = 20;

/// The per-frame facts the view reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameFacts {
    /// Display width W `[0x007A5220]` and height H.
    pub width: i32,
    pub height: i32,
    /// The screen open mode 0–3 (`ui/panels.md` §4).
    pub open_mode: u8,
    /// `0x00492C10` ≠ 0: the mini map moves down 96 px ([`mini_down`],
    /// §9 r1).
    pub mini_down: bool,
    /// The unit origin (cx, cy) (`render/camera.md` §3).
    pub unit_origin: ClientPos,
}

impl FrameFacts {
    /// Hp = H − 40 `[0x007A521C]`.
    pub fn play_height(&self) -> i32 {
        self.height - PANEL_HEIGHT
    }
}

/// An inclusive rectangle `[left, right] × [top, bottom]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Bounds {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.left <= x && x <= self.right && self.top <= y && y <= self.bottom
    }
}

/// The view state (§8 r3–r4, §9 r1–r2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct View {
    /// `[0x007A5150]`: mini (1) or full screen (0).
    pub mini: bool,
    /// `[0x00711254]`.
    pub div: i32,
    /// (ox, oy) `[0x007A5188]`, `[0x007A518C]`.
    pub offset: (i32, i32),
    /// (mx, my).
    pub mini_origin: (i32, i32),
    /// `[0x007A51C8]`–`[0x007A51D4]`.
    pub marker: Bounds,
    /// The values §9 r1 compares with: last `0x00492C10` result and last
    /// `Left` (`[0x007A51E8]`, a copy made at init).
    last_down: bool,
    last_left: bool,
    /// The `Left` value the panel side remembers while it forces one
    /// (§9 r2).
    saved_left: Option<bool>,
}

impl View {
    /// The view after UI init: full size, divisor 10, offset (0, 0).
    pub fn new(opts: &Options) -> Self {
        View {
            mini: false,
            div: DIV_FULL,
            offset: (0, 0),
            mini_origin: (0, 0),
            marker: Bounds::default(),
            last_down: false,
            last_left: opts.left,
            saved_left: None,
        }
    }

    /// §9 r1 geometry: mini origin and marker rectangle.
    pub fn compute(&mut self, opts: &Options, f: &FrameFacts) {
        let (w, h) = (f.width, f.height);
        self.mini_origin = if !opts.left {
            (2 * w / 3, 78)
        } else {
            (0, if f.mini_down { 96 } else { 0 })
        };
        let (mx, my) = self.mini_origin;
        self.marker = if self.mini {
            Bounds {
                left: mx - 8,
                top: my,
                right: w / 3 + mx,
                bottom: h / 3 + my,
            }
        } else {
            Bounds {
                left: -16,
                top: -16,
                right: w,
                bottom: h,
            }
        };
    }

    /// §8 r4 (`0x00457640`): runs when `force` or `AutoMap Centers`.
    pub fn recentre(&mut self, force: bool, opts: &Options, f: &FrameFacts) {
        if !force && !opts.centers {
            return;
        }
        self.compute(opts, f);
        let (mx, my) = self.mini_origin;
        self.offset = if self.mini {
            (f.width / 3 - mx - 16, f.height / 3 - my - 16)
        } else {
            (0, 0)
        };
    }

    /// §9 r1 (`0x00457520`, each automap frame): a change of the mini
    /// map's down shift (mini only) or of `Left` stores them and
    /// re-centres with force 0; then the geometry is recomputed.
    pub fn frame(&mut self, opts: &Options, f: &FrameFacts) {
        if (self.mini && f.mini_down != self.last_down) || opts.left != self.last_left {
            self.last_down = f.mini_down;
            self.last_left = opts.left;
            self.recentre(false, opts, f);
        }
        self.compute(opts, f);
    }

    /// §8 r3 (`0x0045A720`): on change, divisor 20 mini / 10 full and a
    /// re-centre with force 0. Returns whether the size changed (the cel
    /// files are then reloaded, §8 r5).
    pub fn set_size(&mut self, mini: bool, opts: &Options, f: &FrameFacts) -> bool {
        if self.mini == mini {
            return false;
        }
        self.mini = mini;
        self.div = if mini { DIV_MINI } else { DIV_FULL };
        self.recentre(false, opts, f);
        true
    }

    /// §9 r2 (`0x00459700`): open mode 1 forces `Left` := 1, mode 2
    /// forces 0, modes 0 and 3 restore it. Returns s: ±W/4 in full mode
    /// (+W/4 in open mode 1, −W/4 in open mode 2), else 0.
    ///
    /// Reading: the old value is remembered when a force starts and
    /// restored once; the setter of §8 is not involved (no registry
    /// write).
    pub fn panel_side(&mut self, opts: &mut Options, f: &FrameFacts) -> i32 {
        match f.open_mode {
            1 | 2 => {
                if self.saved_left.is_none() {
                    self.saved_left = Some(opts.left);
                }
                opts.left = f.open_mode == 1;
            }
            _ => {
                if let Some(v) = self.saved_left.take() {
                    opts.left = v;
                }
            }
        }
        if self.mini {
            return 0;
        }
        match f.open_mode {
            1 => f.width / 4,
            2 => -(f.width / 4),
            _ => 0,
        }
    }

    /// §9 r3: (Ax, Ay) for panel side `s`.
    pub fn origin(&self, s: i32, f: &FrameFacts) -> (i32, i32) {
        let (cx, cy) = (f.unit_origin.x, f.unit_origin.y);
        (
            self.offset.0 + 40 + (cx / self.div - f.width / 2 + s),
            self.offset.1 + 15 + (cy / self.div - f.play_height() / 2),
        )
    }

    /// §9 r3: the screen (X, Y) of cell (x, y) for origin `a`.
    pub fn cell_screen(&self, x: i16, y: i16, a: (i32, i32)) -> (i32, i32) {
        (
            i32::from(x) * 10 / self.div - a.0,
            i32::from(y) * 10 / self.div - a.1,
        )
    }
}

/// `0x00492C10` (§9 r1): the party-portrait state `[0x007BEECC]` ≠ 2
/// (`ui/messages.md`: the portrait pass is skipped at 2), so the left mini
/// map moves down 96 px whenever the portraits are not hidden. Gives
/// [`FrameFacts::mini_down`].
pub fn mini_down(portrait_state: u32) -> bool {
    portrait_state != 2
}
