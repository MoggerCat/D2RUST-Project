// Spec: specs/ui/frontend-menus.md (§F1.1–§F1.3, §F1.6), specs/ui/frontend-credits.md (C0–C2, C5)
//! The front end: a state machine from start-up through the trademark and
//! main menu to the game load. Plain Rust (no Bevy types except the video
//! stub's log line); the host feeds [`FrontInput`]s, calls [`FrontEnd::tick`]
//! every 40 ms (C0) and draws [`FrontEnd::draw`].
//!
//! Adding a screen: implement [`Screen`] in `screens/<name>.rs` and register
//! it in that module's `register`. Controls fire [`Action::Trigger`]s;
//! [`flow::next`] holds the transitions, so a screen names no other screen.
//!
//! Preview fills are `// d2rs-own, unverified`; open points are PROVISIONAL
//! under REC-168.

pub mod control;
pub mod flow;
pub mod glyphs;
pub mod screen;
pub mod screens;
pub mod startup;

use crate::ui::geom::Point;

pub use control::{Action, Control, ControlKind, TextRow};
pub use flow::{FlowCtx, GameLoad, Next, Trigger};
pub use screen::{FrontCtx, Placeholder, Registry, Screen};
pub use screens::ids::*;
pub use startup::{ProgressStore, VideoHook};

/// A screen's name. An open set: a new screen adds a const in its module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScreenId(pub &'static str);

/// One front-end tick is 40 ms (C0 r1).
pub const TICK_MS: u64 = 40;

/// The sky palette every `0x0043C4F0` screen loads (§F1.6 r1), under
/// `data\global\palette\sky\`.
pub const SKY_PALETTE: [&str; 2] = [
    r"data\global\palette\sky\pal.dat",
    r"data\global\palette\sky\pal.pl2",
];

/// Logo frame (§F1.5 r3): `((now − created)/40) mod 29`; frame 29 is never shown.
pub fn logo_frame(now_ms: u64, created_ms: u64) -> u32 {
    (now_ms.saturating_sub(created_ms) / 40 % 29) as u32
}

/// The palette of the character-create screen (`fechar`, "front end
/// character"). PROVISIONAL (§F1.6 r1 names `fechar` only as compared
/// against by `0x0042F2E0`; the builder `0x00435580` loads no palette in
/// the trace): chosen because the 1.14d screenshot of the create screen
/// matches the `charactercreationscreenEXP` art drawn with `fechar` (mean
/// RGB error 13 against 25 and more for every other palette of d2data).
pub const FECHAR_PALETTE: [&str; 2] = [
    r"data\global\palette\fechar\pal.dat",
    r"data\global\palette\fechar\pal.pl2",
];

/// The create screen's fire cel (§F3.2): itself the additive layer.
pub const FIRE: &str = r"FrontEnd\fire";
/// Draw mode of every fire overlay (§F1.5 r2): additive.
pub const FIRE_MODE: u8 = 3;

/// The fire cel drawn additively over a logo half's black base (§F1.5 r1;
/// file names §F1.5 r1, `0x006D4014`–`0x006D4074`).
pub fn fire_overlay(base: &str) -> Option<&'static str> {
    match base {
        r"FrontEnd\D2logoBlackLeft" => Some(r"FrontEnd\D2logoFireLeft"),
        r"FrontEnd\D2logoBlackRight" => Some(r"FrontEnd\D2logoFireRight"),
        _ => None,
    }
}

/// How the front end ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Program exit.
    Exit,
    /// Start the game (`client/model.md` §7 r9).
    GameLoad(GameLoad),
}

/// Input, in 800×600 frame coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontInput {
    Move(Point),
    /// Left button down / up.
    Down(Point),
    Up(Point),
    /// Key-down, a virtual-key code.
    Key(u16),
    /// A typed UTF-16 unit.
    Char(u16),
    /// Key-up, a virtual-key code.
    KeyUp(u16),
    /// Mouse wheel (120 per notch, positive away from the user).
    Wheel(i32),
    /// Middle button down.
    Middle,
}

/// Whether the Save folder holds a character (`0x00430BC0`, §F2.2).
pub trait SaveFolder {
    fn has_saves(&self) -> bool;
}

/// A fixed answer (tests, `--new`).
impl SaveFolder for bool {
    fn has_saves(&self) -> bool {
        *self
    }
}

/// One drawn thing, in creation order. The host maps it to its draw list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawItem {
    /// Frame `frame` of `file` (under `data\global\ui\`) at the cel position
    /// (x, bottom y).
    Art {
        file: &'static str,
        frame: u32,
        at: Point,
    },
    /// A label or text: a string id (0: `text` is literal).
    Text {
        string_id: u32,
        text: String,
        font: u16,
        at: Point,
        /// A button label (§F1.1 r5): centered in the button, baseline
        /// from its height; `None` for a plain text control.
        label: Option<Label>,
        /// Text colour `k` (`ui/text.md` §5; 0 none).
        color: i32,
        /// A text control's row (§F1.1 r8): laid out in the control box
        /// (`at` = the control's x, bottom y); `None`: a plain `DrawText`
        /// pen at `at`.
        boxed: Option<TextBox>,
    },
    /// A blended layer: frame `frame` of `file` drawn with draw mode
    /// `mode` (3 = additive) at (x, bottom y) plus the DC6 frame offsets
    /// (§F1.5 r2): the logo and title fire.
    Blend {
        file: &'static str,
        frame: u32,
        at: Point,
        mode: u8,
        /// Drawn with the frame's bottom-left at `at`, its offsets not
        /// applied (the create fire, [`FIRE_AT_BOX`]).
        boxed: bool,
    },
    /// A dark filled box (menu panels).
    Rect { at: Point, w: i32, h: i32 },
    /// A box outline.
    Border { at: Point, w: i32, h: i32 },
}

/// Where a button label sits (§F1.1 r5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    pub w: u16,
    pub h: u16,
    pub pressed: bool,
    /// The second label string (flag 0x40); 0 none.
    pub second: u32,
}

/// A text control's box and the row this item is (§F1.1 r8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextBox {
    pub w: u16,
    pub h: u16,
    /// Margins mx, my (descriptor [5], [6]).
    pub mx: i32,
    pub my: i32,
    /// Descriptor [11]: 2 centred, 0x10 right aligned, 0x20 / 1 no wrap.
    pub flags: u16,
    /// Rows added before this one (its first row's index).
    pub row: u16,
}

/// The create fire (descriptor 178 / 179, a type-3 control without an
/// anim table) is drawn with each frame's bottom-left at the control's
/// (x, bottom y), the frame offsets not applied. PROVISIONAL (§F3.1
/// table; §F3.3 r3 covers the anim-table case only): the 1.14d create
/// screenshot shows the fire at columns 345–454, bottom row 470; with the
/// offsets (−34, 132) of `fire.dc6` it would sit at 311–420, rows 476–602.
pub const FIRE_AT_BOX: bool = true;

pub struct FrontEnd {
    registry: Registry,
    saves: Box<dyn SaveFolder>,
    expansion: bool,
    current: ScreenId,
    controls: Vec<Control>,
    /// Build time (ms) of each control, for timers and animation.
    built_ms: u64,
    pressed: Option<usize>,
    pointer: Option<Point>,
    pending: Vec<FrontInput>,
    flow: FlowCtx,
    tick: u64,
    outcome: Option<Outcome>,
    palette: Option<[&'static str; 2]>,
    /// §F1.3 `[0x007795EC]`: 0 single player.
    pub game_kind: u8,
    /// Screens entered, in order (the log of the flow).
    pub entered: Vec<ScreenId>,
    /// Input flushes performed (C5 r4).
    pub flushes: u32,
}

impl FrontEnd {
    pub fn new(expansion: bool, saves: Box<dyn SaveFolder>, registry: Registry) -> Self {
        Self {
            registry,
            saves,
            expansion,
            current: TRADEMARK,
            controls: Vec::new(),
            built_ms: 0,
            pressed: None,
            pointer: None,
            pending: Vec::new(),
            flow: FlowCtx::default(),
            tick: 0,
            outcome: None,
            palette: None,
            game_kind: 0,
            entered: Vec::new(),
            flushes: 0,
        }
    }

    /// A front end with every screen module registered.
    pub fn with_screens(expansion: bool, saves: Box<dyn SaveFolder>) -> Self {
        let mut reg = Registry::default();
        screens::register_all(&mut reg);
        Self::new(expansion, saves, reg)
    }

    /// Enter the front end. `first_entry`: the first in this process (C1 r1):
    /// run the start-up chain, then the trademark screen; later entries go
    /// straight to the main menu.
    pub fn start(
        &mut self,
        first_entry: bool,
        store: &mut dyn ProgressStore,
        video: &mut dyn VideoHook,
    ) {
        if !first_entry {
            self.enter(MAIN_MENU);
            return;
        }
        let first_run = store.get().is_none();
        self.flush_input();
        // The d2rs video stub always has a backend (C5 r6).
        startup::run_startup(self.expansion, true, store, video);
        if first_run {
            // C1 r2.6: built at the end of the classic intro, then again;
            // the second build replaces the first and restarts its timer.
            self.enter(TRADEMARK);
        }
        self.enter(TRADEMARK);
    }

    /// Remove every pending input unprocessed (C5 r4).
    pub fn flush_input(&mut self) {
        self.pending.clear();
        self.flushes += 1;
    }

    pub fn input(&mut self, ev: FrontInput) {
        self.pending.push(ev);
    }

    pub fn current(&self) -> ScreenId {
        self.current
    }

    pub fn controls(&self) -> &[Control] {
        &self.controls
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    /// The palette files in use (§F1.6 r1); `None` until a screen loaded one.
    pub fn palette(&self) -> Option<[&'static str; 2]> {
        self.palette
    }

    pub fn now_ms(&self) -> u64 {
        self.tick * TICK_MS
    }

    /// Facts the flow table reads; a test or host sets `difficulties_open`.
    pub fn flow_mut(&mut self) -> &mut FlowCtx {
        &mut self.flow
    }

    /// One 40 ms tick (C0): process the pending input, then timers and the
    /// screen's own update.
    pub fn tick(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.tick += 1;
        for ev in std::mem::take(&mut self.pending) {
            if self.outcome.is_some() {
                break;
            }
            self.handle(ev);
            self.registry.get_mut(self.current).sync(&mut self.controls);
        }
        self.run_timers();
        if self.outcome.is_none() {
            let mut ctx = FrontCtx {
                expansion: self.expansion,
                flow: &mut self.flow,
                now_ms: self.tick * TICK_MS,
            };
            if let Some(t) = self.registry.get_mut(self.current).tick(&mut ctx) {
                self.trigger(t);
            }
            self.registry.get_mut(self.current).sync(&mut self.controls);
        }
    }

    /// Fire a flow trigger as the current screen would (§F1.3).
    pub fn trigger(&mut self, t: Trigger) {
        if self.outcome.is_some() {
            return;
        }
        if t == Trigger::SinglePlayer {
            // `0x00435CD0`: game kind := single player; `0x00430BC0` scan.
            self.game_kind = 0;
            self.flow.saves_found = self.saves.has_saves();
        }
        match flow::next(self.current, t, self.flow) {
            Next::Screen(s) => self.enter(s),
            Next::Exit => self.outcome = Some(Outcome::Exit),
            Next::GameLoad(g) => self.outcome = Some(Outcome::GameLoad(g)),
            Next::Stay => {}
        }
    }

    /// Jump to a screen without the flow (hosts and tests; screens the flow
    /// table does not reach yet, such as Configure Controls).
    pub fn goto(&mut self, id: ScreenId) {
        self.enter(id);
    }

    fn enter(&mut self, id: ScreenId) {
        self.current = id;
        self.pressed = None;
        self.built_ms = self.tick * TICK_MS;
        let screen = self.registry.get_mut(id);
        let mut ctx = FrontCtx {
            expansion: self.expansion,
            flow: &mut self.flow,
            now_ms: self.built_ms,
        };
        self.controls = screen.build(&mut ctx);
        screen.sync(&mut self.controls);
        if let Some(p) = screen.palette() {
            self.palette = Some(p);
        }
        self.entered.push(id);
    }

    fn fire(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Trigger(t) => self.trigger(t),
            Action::Custom(id) => {
                let mut ctx = FrontCtx {
                    expansion: self.expansion,
                    flow: &mut self.flow,
                    now_ms: self.tick * TICK_MS,
                };
                if let Some(t) = self.registry.get_mut(self.current).action(&mut ctx, id) {
                    self.trigger(t);
                }
            }
        }
    }

    /// The pointer's last position (800×600), if it was seen.
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
    }

    /// The topmost clickable control under `p`.
    fn hit(&self, p: Point) -> Option<usize> {
        self.controls.iter().rposition(|c| {
            c.enabled
                && c.visible
                && matches!(
                    c.kind,
                    ControlKind::Image | ControlKind::Button | ControlKind::AnimImage
                )
                && c.action != Action::None
                && c.contains(p)
        })
    }

    fn handle(&mut self, ev: FrontInput) {
        match ev {
            FrontInput::Move(p) => {
                self.pointer = Some(p);
                let mut ctx = FrontCtx {
                    expansion: self.expansion,
                    flow: &mut self.flow,
                    now_ms: self.tick * TICK_MS,
                };
                self.registry.get_mut(self.current).pointer(&mut ctx, p);
            }
            FrontInput::Wheel(d) => {
                let mut ctx = FrontCtx {
                    expansion: self.expansion,
                    flow: &mut self.flow,
                    now_ms: self.tick * TICK_MS,
                };
                self.registry.get_mut(self.current).wheel(&mut ctx, d);
            }
            FrontInput::KeyUp(k) => {
                let mut ctx = FrontCtx {
                    expansion: self.expansion,
                    flow: &mut self.flow,
                    now_ms: self.tick * TICK_MS,
                };
                self.registry.get_mut(self.current).key_up(&mut ctx, k);
            }
            FrontInput::Middle => {
                let mut ctx = FrontCtx {
                    expansion: self.expansion,
                    flow: &mut self.flow,
                    now_ms: self.tick * TICK_MS,
                };
                self.registry.get_mut(self.current).middle_down(&mut ctx);
            }
            FrontInput::Down(p) => self.pressed = self.hit(p),
            // PROVISIONAL (REC-168): a click fires on button-up over the
            // control that took the button-down.
            FrontInput::Up(p) => {
                let down = self.pressed.take();
                if let (Some(i), Some(j)) = (down, self.hit(p)) {
                    if i == j {
                        let a = self.controls[i].action;
                        self.fire(a);
                    }
                }
            }
            FrontInput::Key(k) => {
                let hit = self
                    .controls
                    .iter()
                    .find(|c| c.enabled && c.action != Action::None && c.takes_key(k))
                    .map(|c| c.action);
                if let Some(a) = hit {
                    self.fire(a);
                }
            }
            FrontInput::Char(u) => {
                let mut ctx = FrontCtx {
                    expansion: self.expansion,
                    flow: &mut self.flow,
                    now_ms: self.tick * TICK_MS,
                };
                self.registry.get_mut(self.current).char(&mut ctx, u);
            }
        }
    }

    /// §F1.1 r7: once `⌊now/1000⌋ − ⌊start/1000⌋ > seconds`, the timer
    /// fires on every update until the screen is torn down.
    fn run_timers(&mut self) {
        let now = self.tick * TICK_MS / 1000;
        let start = self.built_ms / 1000;
        let due = self
            .controls
            .iter()
            .find(|c| c.kind == ControlKind::Timer && now - start > u64::from(c.seconds))
            .map(|c| c.action);
        if let Some(a) = due {
            self.fire(a);
        }
    }

    /// The current screen's per-tick items (credits rows, hero frames, ...),
    /// drawn after [`FrontEnd::draw`]. `adv`: text width in a font.
    pub fn overlay(&mut self, adv: &dyn Fn(u16, &[u16]) -> i32) -> Vec<DrawItem> {
        let now = self.now_ms();
        self.registry.get_mut(self.current).overlay(now, adv)
    }

    /// The frame's draw items, in creation order.
    pub fn draw(&self) -> Vec<DrawItem> {
        let mut out = Vec::new();
        let now = self.now_ms();
        for (i, c) in self.controls.iter().enumerate().filter(|(_, c)| c.visible) {
            let pressed = self.pressed == Some(i) && c.enabled;
            let at = Point::new(c.x, c.y);
            match c.kind {
                ControlKind::Image => {
                    if let Some(file) = c.art {
                        out.extend(image_tiles(file, c.x, c.y, c.w, c.h));
                    }
                }
                ControlKind::AnimImage => {
                    if let Some(file) = c.art {
                        let frame = logo_frame(now, self.built_ms);
                        if file == FIRE {
                            out.push(DrawItem::Blend {
                                file,
                                frame,
                                at,
                                mode: FIRE_MODE,
                                boxed: FIRE_AT_BOX,
                            });
                        } else {
                            out.push(DrawItem::Art { file, frame, at });
                            if let Some(fire) = fire_overlay(file) {
                                out.push(DrawItem::Blend {
                                    file: fire,
                                    frame,
                                    at,
                                    mode: FIRE_MODE,
                                    boxed: false,
                                });
                            }
                        }
                    }
                }
                ControlKind::Button => {
                    if let Some(file) = c.art {
                        let tiles = control::button_tiles(c.w, c.h);
                        // §F1.1 r4: tiles draw left to right at +256 px.
                        for t in 0..tiles {
                            let frame = control::button_frame(t, tiles, pressed, c.enabled, false);
                            let at = Point::new(c.x + 256 * t as i32, c.y);
                            if c.enabled {
                                out.push(DrawItem::Art { file, frame, at });
                            } else {
                                // r4: a disabled button without flag 0x20
                                // draws its up frames with draw mode 1.
                                out.push(DrawItem::Blend {
                                    file,
                                    frame,
                                    at,
                                    mode: 1,
                                    boxed: false,
                                });
                            }
                        }
                    }
                    if c.string_id != 0 {
                        out.push(DrawItem::Text {
                            string_id: c.string_id,
                            text: String::new(),
                            font: control::label_font(c.h),
                            at,
                            label: Some(Label {
                                w: c.w,
                                h: c.h,
                                pressed,
                                second: c.second_label,
                            }),
                            color: 0,
                            boxed: None,
                        });
                    }
                }
                ControlKind::Text => out.extend(text_rows(c)),
                ControlKind::EditBox | ControlKind::Timer => {}
            }
        }
        out
    }
}

/// The rows of a text control, one item each (§F1.1 r8): a control nothing
/// was added to draws nothing. An empty first row followed by others still
/// takes its row (it is added, and draws nothing).
fn text_rows(c: &Control) -> Vec<DrawItem> {
    let first = (c.has_text() || !c.more_rows.is_empty()).then(|| TextRow {
        string_id: c.string_id,
        text: c.text.clone().unwrap_or_default(),
        color: c.color,
    });
    first
        .into_iter()
        .chain(c.more_rows.iter().cloned())
        .enumerate()
        .map(|(i, r)| DrawItem::Text {
            string_id: r.string_id,
            text: r.text,
            font: c.font,
            at: Point::new(c.x, c.y),
            label: None,
            color: r.color,
            boxed: Some(TextBox {
                w: c.w,
                h: c.h,
                mx: c.margin.0,
                my: c.margin.1,
                flags: c.flags,
                // d2rs-own: rows a wrap adds before this one are not
                // counted (every multi-row control here has short rows).
                row: i as u16,
            }),
        })
        .collect()
}

/// An image control's cel cut in 256 × 256 tiles (§F1.1 r4, §F1.4 table:
/// e.g. an 800 × 600 background is 4 × 3 frames, left to right, top to
/// bottom): one draw per tile. PROVISIONAL (§F1.1 r3 gives the control's
/// bottom edge only): tile row r is drawn at cel Y = (y − h + 1) + the
/// heights of rows 0..=r, so the art covers rows y − h + 2 … y + 1, one row
/// below the control box; every 1.14d screenshot (main menu, character
/// select, create) has screen row 0 black and the background one row lower
/// than a y − h + 1 top, while the buttons (drawn at their y) match.
fn image_tiles(file: &'static str, x: i32, y: i32, w: u16, h: u16) -> Vec<DrawItem> {
    let (w, h) = (i32::from(w.max(1)), i32::from(h.max(1)));
    let cols = (w + 255) / 256;
    let rows = (h + 255) / 256;
    let top = y - h + 1;
    let mut out = Vec::with_capacity((cols * rows) as usize);
    for r in 0..rows {
        let row_h = (h - 256 * r).min(256);
        let bottom = top + 256 * r + row_h;
        for c in 0..cols {
            out.push(DrawItem::Art {
                file,
                frame: (r * cols + c) as u32,
                at: Point::new(x + 256 * c, bottom),
            });
        }
    }
    out
}
