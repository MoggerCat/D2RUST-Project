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

pub use control::{Action, Control, ControlKind};
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

/// The create screen's fire cel (§F3.2): itself the additive layer.
pub const FIRE: &str = r"FrontEnd\fire";
/// Draw mode of every fire overlay (§F1.5 r2): additive.
pub const FIRE_MODE: u8 = 3;

/// The fire cel drawn additively over a logo half's black base (§F1.5 r1).
/// // d2rs-own, unverified: file names (the base names are the main menu's).
pub fn fire_overlay(base: &str) -> Option<&'static str> {
    match base {
        r"FrontEnd\BlackLeft" => Some(r"FrontEnd\FireLeft"),
        r"FrontEnd\BlackRight" => Some(r"FrontEnd\FireRight"),
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
    },
    /// A blended layer: frame `frame` of `file` drawn with draw mode
    /// `mode` (3 = additive) at (x, bottom y) plus the DC6 frame offsets
    /// (§F1.5 r2): the logo and title fire.
    Blend {
        file: &'static str,
        frame: u32,
        at: Point,
        mode: u8,
    },
}

/// Where a button label sits (§F1.1 r5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label {
    pub w: u16,
    pub h: u16,
    pub pressed: bool,
}

pub struct FrontEnd {
    registry: Registry,
    saves: Box<dyn SaveFolder>,
    expansion: bool,
    current: ScreenId,
    controls: Vec<Control>,
    /// Build time (ms) of each control, for timers and animation.
    built_ms: u64,
    pressed: Option<usize>,
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
        if screen.loads_sky_palette() {
            self.palette = Some(SKY_PALETTE);
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
            FrontInput::Move(_) => {}
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
                        out.push(DrawItem::Art { file, frame: 0, at });
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
                            });
                        } else {
                            out.push(DrawItem::Art { file, frame, at });
                            if let Some(fire) = fire_overlay(file) {
                                out.push(DrawItem::Blend {
                                    file: fire,
                                    frame,
                                    at,
                                    mode: FIRE_MODE,
                                });
                            }
                        }
                    }
                }
                ControlKind::Button => {
                    if let Some(file) = c.art {
                        let tiles = control::button_tiles(c.w, c.h);
                        for t in 0..tiles {
                            let frame = control::button_frame(t, tiles, pressed, c.enabled, false);
                            out.push(DrawItem::Art { file, frame, at });
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
                            }),
                        });
                    }
                }
                ControlKind::Text => out.push(DrawItem::Text {
                    string_id: c.string_id,
                    text: c.text.clone().unwrap_or_default(),
                    font: c.font,
                    at,
                    label: None,
                }),
                ControlKind::EditBox | ControlKind::Timer => {}
            }
        }
        out
    }
}
