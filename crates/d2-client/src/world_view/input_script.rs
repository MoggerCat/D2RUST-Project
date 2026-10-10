// Spec: specs/tools/facts-render.md (§5 r11), specs/tools/scenario-diff.md (§2 r4, §3 r8)
//! `play --input SCRIPT` and `state-dump --input SCRIPT`: scripted pointer
//! input, the d2rs counterpart of `tools/trace-recorder/autostart.py
//! --input`. Client input only: nothing here decides an outcome.
//!
//! - `play` ([`InputScript`]): the steps become the same [`UiEvent`]s the
//!   window would send, timed by server ticks (the window's own pointer
//!   input is ignored while a script runs).
//! - `state-dump` ([`Headless`]): no window and no UI; the steps' pointer
//!   events go to the world-click dispatcher through the bridge
//!   ([`world_clicks`], the same call the window path makes after the UI)
//!   with the `play` preview's hover pick ([`crate::bridge::hover::pick`]),
//!   and the `key` steps' bound world actions to the handlers the window
//!   runs for the actions no panel took ([`HEADLESS_KEY_ACTIONS`]), at the
//!   poke point: after the snapshot of frame F − 1, before frame F's
//!   drain (`scenario-diff.md` §2 r4, §3 r8).
//! - `clickunit` / `rclickunit` (both): a click on the screen point of
//!   the nearest unit of a type and class ([`unit_point`], the rule of
//!   `autostart.py`'s `nearest`).

use crate::bridge::click::ClickView;
use crate::bridge::hover::unit_feet;
use crate::bridge::link::ServerLink;
use crate::bridge::predict::{Predict, Speeds, WalkTap};
use crate::bridge::world::{ClientWorld, UnitKey};
use crate::bridge::{Bridge, BridgeError};
use crate::controls::click::ClickState;
use crate::controls::keymap::vk_to_key;
use crate::controls::{Action, Bindings, Key, Preset};
use crate::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
use crate::ui::{ActionId, FramePos, Point, PointerButton, UiEvent};

use super::interact::PreviewInteract;
use super::ui_bind::world_clicks;

/// Classes a `clickunit` step can list.
pub const MAX_CLASSES: usize = 8;

/// The target of a `clickunit` / `rclickunit` step: the nearest unit of
/// `unit_type` whose class is one of `classes[..count]` (`count` 0: `*`,
/// any class, living units only), clicked at its screen point plus
/// `offset` (default `(0, −8)`, as `autostart.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitSel {
    pub button: PointerButton,
    pub unit_type: u8,
    pub classes: [u32; MAX_CLASSES],
    pub count: u8,
    pub offset: (i32, i32),
}

impl UnitSel {
    /// `*`: any class.
    pub fn any(&self) -> bool {
        self.count == 0
    }

    fn takes(&self, class: u32) -> bool {
        self.any() || self.classes[..usize::from(self.count)].contains(&class)
    }

    /// The step as written (`clickunit 1 19,20 0 -8`).
    pub fn text(&self) -> String {
        let op = if self.button == PointerButton::Left {
            "clickunit"
        } else {
            "rclickunit"
        };
        let cls = if self.any() {
            "*".to_owned()
        } else {
            self.classes[..usize::from(self.count)]
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "{op} {} {cls} {} {}",
            self.unit_type, self.offset.0, self.offset.1
        )
    }
}

/// The unit a `clickunit` step clicks and the frame point it clicks, or
/// why there is none: among the model's units of the step's type and
/// classes (`*`: any class, but not in mode 0 or 12, as `autostart.py`'s
/// `nearest`), the one whose screen point is nearest the frame centre
/// (squared distance; ties: the lower unit key, d2rs-own — 1.14d keeps
/// the first of its unit-table walk). The screen point is the unit draw
/// point of `render/camera.md` §4 with no extra offset
/// ([`unit_feet`]: static units by the static rule, moving units at their
/// cell centre — d2rs-own, unverified: the model holds cells, 1.14d reads
/// the 16.16 position); the click is that point plus the step's offset,
/// inside the 800 × 600 frame.
pub fn unit_point(
    world: &ClientWorld,
    cam: &Camera,
    sel: &UnitSel,
) -> Result<(UnitKey, Point), String> {
    let size = FrameSize::play();
    let (cx, cy) = (size.width / 2, size.height / 2);
    let mut best: Option<(i64, UnitKey, (i32, i32))> = None;
    for (key, u) in &world.units {
        if key.unit_type != sel.unit_type || !sel.takes(u.class) {
            continue;
        }
        if sel.any() && matches!(u.mode, 0 | 12) {
            continue;
        }
        let Some(cell) = u.position else { continue };
        let s = unit_feet(cam, key.unit_type, cell);
        let (dx, dy) = (i64::from(s.0 - cx), i64::from(s.1 - cy));
        let d = dx * dx + dy * dy;
        if best.is_none_or(|(b, _, _)| d < b) {
            best = Some((d, *key, s));
        }
    }
    let Some((_, key, s)) = best else {
        return Err("no such unit".into());
    };
    let (x, y) = (s.0 + sel.offset.0, s.1 + sel.offset.1);
    if !(0..size.width).contains(&x) || !(0..size.height).contains(&y) {
        return Err(format!(
            "unit {}:{} at ({x}, {y}), outside the frame",
            key.unit_type, key.guid
        ));
    }
    Ok((key, Point { x, y }))
}

/// What answers a `clickunit` step: the unit and the frame point clicked
/// ([`unit_point`]), or why none.
pub type ResolveUnit<'a> = dyn FnMut(&UnitSel) -> Result<(UnitKey, Point), String> + 'a;

/// The camera a `clickunit` step reads: the local player at `local_at`
/// (16.16; `None`: the model's position), open mode 0, no shake.
pub fn script_camera(world: &ClientWorld, local_at: Option<(u32, u32)>) -> Option<Camera> {
    world.local()?;
    let (x, y) = local_at.or_else(|| world.local_position())?;
    Some(Camera::new(
        FrameSize::play(),
        OpenMode::NONE,
        moving_to_client(x, y),
        (0, 0),
    ))
}

/// The world actions a headless `key` step may run: the ones whose
/// handler the window path runs on an action no panel took
/// (`world_view::present`): the belt columns (commands 23–26,
/// [`crate::bridge::belt::send_keys`]), the run lock (command 35,
/// [`crate::bridge::click::RunMods::toggle_run`]), weapon swap (44,
/// [`super::swap_key::send_swaps`]) and the speech commands (27–33, 55,
/// [`super::swap_key::send_says`]). Every other key (panels, automap,
/// chat, held modifiers, the skill hotkeys F1–F8, whose use sender
/// `ui/controls.md` §3.1 r2 does not name and the window does not wire)
/// is refused headless.
pub const HEADLESS_KEY_ACTIONS: [Action; 14] = [
    Action::BeltSlot1,
    Action::BeltSlot2,
    Action::BeltSlot3,
    Action::BeltSlot4,
    Action::ToggleRun,
    Action::SwapWeapons,
    Action::Say0,
    Action::Say1,
    Action::Say2,
    Action::Say3,
    Action::Say4,
    Action::Say5,
    Action::Say6,
    Action::Say7X,
];

/// The world action a headless `key` step runs (the original key
/// configuration, key mode 1: no panel is open), or why it is refused.
pub fn headless_key_action(bindings: &Bindings, vk: u32) -> Result<Action, String> {
    let key = play_key(vk).ok_or_else(|| format!("key 0x{vk:02X}: not a key"))?;
    let action = crate::ui::edge::input_actions(bindings, &[key])
        .into_iter()
        .find_map(|e| match e {
            UiEvent::Action(ActionId(a)) => Action::ALL.get(usize::from(a)).copied(),
            _ => None,
        });
    match action {
        Some(a) if HEADLESS_KEY_ACTIONS.contains(&a) => Ok(a),
        other => Err(format!(
            "key 0x{vk:02X} ({}): not applied headless ({}); headless keys: 1-4 (belt), \
             R (run lock), W (weapon swap), NumPad 0-7 (speech); panels, automap, chat, \
             held modifiers and the skill hotkeys F1-F8 are not (use `input d2rs` with \
             `play`, or a C->S `send` line)",
            key.name(),
            other.map_or("bound to nothing", |a| a.name())
        )),
    }
}

/// One step of a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `wait N`: N server ticks (`play` only; side-specific).
    Wait(u64),
    /// `move X Y`: the cursor to frame pixel (X, Y).
    Move(Point),
    /// `click X Y` / `rclick X Y`: cursor there, press, and the release
    /// (`play`: on the next tick; framed: in the same pass).
    Click(PointerButton, Point),
    /// `frame F`: the steps after it apply after server frame F − 1 ran,
    /// before frame F's drain.
    Frame(u32),
    /// `hold X Y N` (after a `frame` step): left press at (X, Y), its
    /// release N frames later (before frame F + N's drain).
    Hold(Point, u32),
    /// `key K`: a key down and up (Windows virtual-key code). `play`
    /// delivers it as the window's key press ([`play_key`]: its bound
    /// world action and its typed character); headless runs its world
    /// action ([`headless_key_action`]).
    Key(u32),
    /// `clickunit T C[,C..]|* [DX DY]` / `rclickunit ...`: a click on the
    /// screen point of a unit ([`unit_point`]).
    ClickUnit(UnitSel),
}

/// The virtual-key code of `key K`: a letter or digit, ESC, TAB, ENTER,
/// SPACE, SHIFT, CTRL, ALT, F1–F12 or a number (as `autostart.py`).
pub fn vk_code(k: &str) -> Option<u32> {
    let up = k.to_ascii_uppercase();
    let named = match up.as_str() {
        "ESC" => Some(0x1B),
        "TAB" => Some(0x09),
        "ENTER" => Some(0x0D),
        "SPACE" => Some(0x20),
        "SHIFT" => Some(0x10),
        "CTRL" => Some(0x11),
        "ALT" => Some(0x12),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    if let Some(n) = up.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=12).contains(&n) {
            return Some(0x6F + n);
        }
    }
    let b = up.as_bytes();
    if b.len() == 1 && b[0].is_ascii_alphanumeric() {
        return Some(u32::from(b[0]));
    }
    match up.strip_prefix("0X") {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => up.parse().ok(),
    }
}

/// The window key of a `key` step's virtual-key code (`play`), through
/// the controls key table ([`vk_to_key`]); `None` for a code the table
/// cannot hold (the parser rejects those).
pub fn play_key(vk: u32) -> Option<Key> {
    u16::try_from(vk).ok().and_then(vk_to_key)
}

/// A decimal or `0x` hexadecimal integer (as `autostart.py`'s `int(x, 0)`).
fn int(s: &str) -> Option<i64> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let v = match body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        Some(h) => i64::from_str_radix(h, 16).ok()?,
        None => body.parse().ok()?,
    };
    Some(if neg { -v } else { v })
}

/// Parses `wait N; move X Y; click X Y; rclick X Y; frame F; hold X Y N;
/// key K; clickunit T C[,C..]|* [DX DY]; rclickunit ...` (steps separated
/// by `;`, blank steps ignored). Coordinates are 800 × 600 frame pixels.
/// `frame` numbers are ≥ 1 and never go back; `hold` needs a `frame` step
/// before it (its N counts frames); a `key` is one the controls key table
/// holds ([`play_key`]); a `clickunit` names a unit type 0–5 and up to
/// [`MAX_CLASSES`] classes (or `*`).
pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut last_frame: Option<u32> = None;
    for raw in text.split(';') {
        let words: Vec<&str> = raw.split_whitespace().collect();
        let Some((&op, args)) = words.split_first() else {
            continue;
        };
        let num = |s: &str| -> Result<i64, String> {
            s.parse()
                .map_err(|_| format!("`{}`: `{s}` is not a number", raw.trim()))
        };
        let point = |args: &[&str]| -> Result<Point, String> {
            match args {
                [x, y] => {
                    let (x, y) = (num(x)?, num(y)?);
                    if !(0..800).contains(&x) || !(0..600).contains(&y) {
                        return Err(format!("`{}`: outside the 800 × 600 frame", raw.trim()));
                    }
                    Ok(Point {
                        x: x as i32,
                        y: y as i32,
                    })
                }
                _ => Err(format!("`{}`: needs X Y", raw.trim())),
            }
        };
        steps.push(match op {
            "wait" => match args {
                [n] => Step::Wait(
                    u64::try_from(num(n)?)
                        .map_err(|_| format!("`{}`: negative wait", raw.trim()))?,
                ),
                _ => return Err(format!("`{}`: needs N (server ticks)", raw.trim())),
            },
            "move" => Step::Move(point(args)?),
            "click" => Step::Click(PointerButton::Left, point(args)?),
            "rclick" => Step::Click(PointerButton::Right, point(args)?),
            "frame" => {
                let f = match args {
                    [f] => u32::try_from(num(f)?).ok().filter(|&f| f >= 1),
                    _ => None,
                }
                .ok_or_else(|| format!("`{}`: needs F ≥ 1 (server frame)", raw.trim()))?;
                if last_frame.is_some_and(|l| f < l) {
                    return Err(format!("`{}`: frame before the previous one", raw.trim()));
                }
                last_frame = Some(f);
                Step::Frame(f)
            }
            "hold" => {
                if last_frame.is_none() {
                    return Err(format!(
                        "`{}`: hold needs a `frame` step before it (N frames)",
                        raw.trim()
                    ));
                }
                let [x, y, n] = args else {
                    return Err(format!("`{}`: needs X Y N", raw.trim()));
                };
                let n = u32::try_from(num(n)?)
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or_else(|| format!("`{}`: N ≥ 1 frames", raw.trim()))?;
                Step::Hold(point(&[x, y])?, n)
            }
            "key" => match args {
                [k] => Step::Key(
                    vk_code(k)
                        .filter(|&vk| play_key(vk).is_some())
                        .ok_or_else(|| format!("`{}`: unknown key `{k}`", raw.trim()))?,
                ),
                _ => return Err(format!("`{}`: needs K", raw.trim())),
            },
            "clickunit" | "rclickunit" => {
                let bad = || format!("`{}`: needs T C[,C..]|* [DX DY]", raw.trim());
                let (t, cls, offset) = match args {
                    [t, c] => (t, c, (0, -8)),
                    [t, c, dx, dy] => {
                        let off = |s: &str| int(s).and_then(|v| i32::try_from(v).ok());
                        (t, c, (off(dx).ok_or_else(bad)?, off(dy).ok_or_else(bad)?))
                    }
                    _ => return Err(bad()),
                };
                let unit_type = int(t)
                    .and_then(|v| u8::try_from(v).ok())
                    .filter(|&v| v <= 5)
                    .ok_or_else(|| format!("`{}`: T is a unit type 0-5", raw.trim()))?;
                let mut classes = [0u32; MAX_CLASSES];
                let mut count = 0u8;
                if *cls != "*" {
                    for (i, c) in cls.split(',').enumerate() {
                        let v = int(c).and_then(|v| u32::try_from(v).ok()).ok_or_else(bad)?;
                        *classes.get_mut(i).ok_or_else(|| {
                            format!("`{}`: at most {MAX_CLASSES} classes", raw.trim())
                        })? = v;
                        count += 1;
                    }
                }
                Step::ClickUnit(UnitSel {
                    button: if op == "clickunit" {
                        PointerButton::Left
                    } else {
                        PointerButton::Right
                    },
                    unit_type,
                    classes,
                    count,
                    offset,
                })
            }
            _ => return Err(format!("`{}`: unknown step `{op}`", raw.trim())),
        });
    }
    Ok(steps)
}

/// A running script: [`InputScript::events`] once per new server tick.
#[derive(bevy::prelude::Resource, Debug, Clone, PartialEq, Eq)]
pub struct InputScript {
    steps: Vec<Step>,
    next: usize,
    resume_at: u64,
    release: Option<(PointerButton, Point)>,
    release_at: u64,
    cursor: Option<Point>,
    keys: Vec<Key>,
    notes: Vec<String>,
}

impl InputScript {
    pub fn new(steps: Vec<Step>) -> Self {
        InputScript {
            steps,
            next: 0,
            resume_at: 0,
            release: None,
            release_at: 0,
            cursor: None,
            keys: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Why `clickunit` steps run since the last call were skipped.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    /// Where the script put the cursor last.
    pub fn cursor(&self) -> Option<FramePos> {
        self.cursor.map(FramePos::Inside)
    }

    /// The keys of the `key` steps run since the last call, in order
    /// (the caller turns them into the window's key events).
    pub fn take_keys(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.keys)
    }

    /// Whether every step has run.
    pub fn done(&self) -> bool {
        self.next >= self.steps.len() && self.release.is_none()
    }

    /// The events of server tick `tick`: a pending release first (and
    /// nothing else that tick; nothing at all while a hold lasts), then
    /// moves up to and including the next press or wait. `frame F` waits
    /// for tick F − 1 (PROVISIONAL: `play` counts the bridge's server
    /// ticks, which equal the server frame from frame 1; the events reach
    /// the UI on the next loop pass, not exactly at the poke point). Key
    /// steps run without using the tick up; [`InputScript::take_keys`]
    /// hands them to the window's key path. A `clickunit` step resolves
    /// to nothing here ([`InputScript::events_with`] resolves it).
    pub fn events(&mut self, tick: u64) -> Vec<UiEvent> {
        self.events_with(tick, &mut |_| Err("no unit resolver".into()))
    }

    /// [`InputScript::events`] with `resolve` answering `clickunit`
    /// steps ([`unit_point`]); one that does not resolve is skipped
    /// (noted in [`InputScript::take_notes`]).
    pub fn events_with(&mut self, tick: u64, resolve: &mut ResolveUnit<'_>) -> Vec<UiEvent> {
        let mut out = Vec::new();
        if let Some((button, at)) = self.release {
            if tick >= self.release_at {
                self.release = None;
                out.push(UiEvent::Release { button, at });
            }
            return out;
        }
        while self.next < self.steps.len() && tick >= self.resume_at {
            let step = self.steps[self.next];
            if let Step::Frame(f) = step {
                if tick + 1 < u64::from(f) {
                    break;
                }
            }
            self.next += 1;
            match step {
                Step::Wait(n) => {
                    self.resume_at = tick + n;
                    break;
                }
                Step::Frame(_) => {}
                Step::Move(p) => {
                    out.push(UiEvent::CursorMoved(p));
                    self.cursor = Some(p);
                }
                Step::Key(vk) => self.keys.extend(play_key(vk)),
                Step::ClickUnit(sel) => match resolve(&sel) {
                    // the hover frame: the cursor now, the click a tick later
                    Ok((_, at)) => {
                        out.push(UiEvent::CursorMoved(at));
                        self.cursor = Some(at);
                        self.steps.splice(
                            self.next..self.next,
                            [Step::Wait(1), Step::Click(sel.button, at)],
                        );
                    }
                    Err(e) => self.notes.push(format!("{}: {e}", sel.text())),
                },
                Step::Click(button, at) => {
                    out.push(UiEvent::CursorMoved(at));
                    out.push(UiEvent::Press { button, at });
                    self.cursor = Some(at);
                    self.release = Some((button, at));
                    self.release_at = tick + 1;
                    break;
                }
                Step::Hold(at, n) => {
                    let button = PointerButton::Left;
                    out.push(UiEvent::CursorMoved(at));
                    out.push(UiEvent::Press { button, at });
                    self.cursor = Some(at);
                    self.release = Some((button, at));
                    self.release_at = tick + u64::from(n);
                    break;
                }
            }
        }
        out
    }
}

/// The headless run's script (`state-dump --input`, `scenario-diff.md`
/// §3 r8): the shared form only (it starts with `frame`; no `wait`; a
/// `key` only when its action is one of [`HEADLESS_KEY_ACTIONS`]),
/// applied through the bridge at the poke point.
#[derive(Debug, Clone)]
pub struct Headless {
    steps: Vec<Step>,
    next: usize,
    /// F of the last `frame` step run.
    anchor: u32,
    /// The held left button: its point and the frame before whose drain
    /// it is released.
    held: Option<(Point, u32)>,
    mouse: Point,
    click: ClickState,
    /// The key bindings: the original key configuration (`play`'s
    /// default, `ui/controls.md` §3).
    bindings: Bindings,
    /// The run lock and modifier word of the world click (command 35).
    run: crate::bridge::click::RunMods,
    /// The pending interaction of a click on a unit out of reach
    /// ([`PreviewInteract`], as `play`'s preview).
    interact: PreviewInteract,
    /// The hover pick of the previous pass (`ClickView::prev_hover`).
    prev_pick: Option<crate::bridge::world::UnitKey>,
    /// The local player's walk prediction (the walks its link records,
    /// the charstats speeds), as `play` keeps it (`world_view::walk`):
    /// the click reads the predicted position.
    walk: Option<(Predict, WalkTap, Option<Speeds>)>,
}

impl Headless {
    /// The script, or why the headless run cannot play it.
    pub fn new(steps: Vec<Step>) -> Result<Self, String> {
        if !matches!(steps.first(), Some(Step::Frame(_)) | None) {
            return Err("state-dump --input: the script starts with `frame F`".into());
        }
        let bindings = Preset::Original
            .bindings()
            .ok_or("state-dump --input: no original key configuration")?;
        for s in &steps {
            match *s {
                Step::Wait(_) => {
                    return Err(format!(
                        "state-dump --input: {s:?} is not applied headless (`wait` is \
                         side-specific)"
                    ))
                }
                Step::Key(vk) => {
                    headless_key_action(&bindings, vk)
                        .map_err(|e| format!("state-dump --input: {e}"))?;
                }
                _ => {}
            }
        }
        Ok(Self {
            steps,
            next: 0,
            anchor: 0,
            held: None,
            mouse: Point { x: 0, y: 0 },
            click: ClickState::default(),
            bindings,
            run: Default::default(),
            interact: PreviewInteract::default(),
            prev_pick: None,
            walk: None,
        })
    }

    /// Clicks read the local player at the walk prediction of `play`
    /// ([`crate::bridge::predict::PredictLink`]'s `tap`, the character's
    /// `speeds`): d2rs-own, unverified (client/model.md OQ2, REC-51: the
    /// server sends the walker nothing, so the model's cell stays at the
    /// walk's start). Without it a click reads the model's cell.
    pub fn with_prediction(mut self, tap: WalkTap, speeds: Option<Speeds>) -> Self {
        self.walk = Some((Predict::new(), tap, speeds));
        self
    }

    /// After each bridge frame: the walks sent, then a prediction step if
    /// the server ticked (`PreviewWalk::frame`).
    pub fn observe(&mut self, world: &ClientWorld, ticked: bool) {
        if let Some((predict, tap, speeds)) = self.walk.as_mut() {
            let walks = tap.take();
            match *speeds {
                Some(sp) => predict.frame(world, walks, ticked, sp),
                None => predict.observe(world),
            }
        }
    }

    /// The prediction as `play` hands it to the model after its walk frame
    /// (`world_view::walk_room::preview_walk_room`): the local player's
    /// room recached at the predicted sub-tile, and the prediction
    /// recorded for the position check's rule 8 (`Bridge::set_local_walk`,
    /// REC-277), so a server point the client part has not reached is
    /// taken, not answered with C→S 0x5F. No prediction: nothing.
    ///
    /// d2rs-own, unverified. PROVISIONAL (client/model.md OQ2; REC-51,
    /// REC-1385: the headless state-dump client follows the server's walk
    /// and point as the play preview does; the 1.14d client's own player
    /// stands at the server player's point at every check of
    /// milestone-anya and combat-cold-plains-wp).
    pub fn sync_local<L: ServerLink>(&self, bridge: &mut Bridge<L>) {
        let Some((predict, _, _)) = self.walk.as_ref() else {
            return;
        };
        if let Some((x, y)) = predict.cell() {
            bridge.recache_local_room(x, y);
        }
        bridge.set_local_walk(predict.position(), predict.mode());
    }

    /// After each bridge frame: [`Headless::observe`], then on a server
    /// tick the pending interaction's frame (`play` runs it once per
    /// drawn tick, `world_view::interact`): once the walk toward the
    /// clicked unit has ended, the interact sender (C→S 0x13, or the item
    /// pick-up). Returns a log line per interaction sent.
    pub fn after_frame<L: ServerLink>(
        &mut self,
        bridge: &mut Bridge<L>,
        ticked: bool,
    ) -> Result<Vec<String>, BridgeError> {
        self.observe(bridge.world(), ticked);
        if !ticked {
            return Ok(Vec::new());
        }
        let target = self.interact.pending.map(|p| p.target);
        let walking = self
            .walk
            .as_ref()
            .is_some_and(|(p, _, _)| p.walking().is_some());
        let before = self.interact.pending.is_some();
        self.interact.frame(bridge, walking)?;
        Ok(match target {
            Some(t) if before && self.interact.pending.is_none() => {
                vec![format!("interact {}:{}", t.unit_type, t.guid)]
            }
            _ => Vec::new(),
        })
    }

    /// Steps not run yet (the footer's "not reached").
    pub fn pending(&self) -> usize {
        self.steps.len() - self.next + usize::from(self.held.is_some())
    }

    /// Whether a button is held (the held repeat runs every pass).
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// The events due after server frame `last` ran, and a log line per
    /// step (`frame F: click X Y after frame L`). Pointer steps give
    /// pointer events, `key` steps their bound world action
    /// ([`UiEvent::Action`]); `resolve` answers `clickunit` steps
    /// ([`unit_point`]): one that does not resolve gives no event, only
    /// its log line.
    pub fn events(
        &mut self,
        last: i32,
        resolve: &mut ResolveUnit<'_>,
    ) -> (Vec<UiEvent>, Vec<String>) {
        let (mut out, mut log) = (Vec::new(), Vec::new());
        let last = i64::from(last);
        if let Some((at, until)) = self.held {
            if last < i64::from(until) - 1 {
                return (out, log);
            }
            self.held = None;
            out.push(UiEvent::Release {
                button: PointerButton::Left,
                at,
            });
            log.push(format!(
                "frame {until}: release {} {} after frame {last}",
                at.x, at.y
            ));
            self.anchor = until;
        }
        while let Some(&step) = self.steps.get(self.next) {
            if let Step::Frame(f) = step {
                if last < i64::from(f) - 1 {
                    break;
                }
                self.anchor = f;
                self.next += 1;
                if last > i64::from(f) - 1 {
                    log.push(format!("frame {f} late: after frame {last}"));
                }
                continue;
            }
            self.next += 1;
            let f = self.anchor;
            match step {
                Step::Move(p) => {
                    self.mouse = p;
                    out.push(UiEvent::CursorMoved(p));
                    log.push(format!(
                        "frame {f}: move {} {} after frame {last}",
                        p.x, p.y
                    ));
                }
                Step::Click(button, at) => {
                    self.mouse = at;
                    out.push(UiEvent::CursorMoved(at));
                    out.push(UiEvent::Press { button, at });
                    out.push(UiEvent::Release { button, at });
                    let name = if button == PointerButton::Left {
                        "click"
                    } else {
                        "rclick"
                    };
                    log.push(format!(
                        "frame {f}: {name} {} {} after frame {last}",
                        at.x, at.y
                    ));
                }
                Step::ClickUnit(sel) => match resolve(&sel) {
                    // The hover frame (scenario-diff.md §2 r4.5): the
                    // cursor before frame F's drain, the click before
                    // frame F + 1's; the next steps wait for it.
                    Ok((unit, at)) => {
                        self.mouse = at;
                        out.push(UiEvent::CursorMoved(at));
                        log.push(format!(
                            "frame {f}: {}: unit {}:{} at ({}, {}), clicked before frame {} \
                             after frame {last}",
                            sel.text(),
                            unit.unit_type,
                            unit.guid,
                            at.x,
                            at.y,
                            f + 1
                        ));
                        self.steps.splice(
                            self.next..self.next,
                            [Step::Frame(f + 1), Step::Click(sel.button, at)],
                        );
                        continue;
                    }
                    Err(e) => log.push(format!(
                        "frame {f}: {}: not clicked ({e}) after frame {last}",
                        sel.text()
                    )),
                },
                Step::Key(vk) => match headless_key_action(&self.bindings, vk) {
                    Ok(a) => {
                        out.push(UiEvent::Action(ActionId(a.index() as u16)));
                        log.push(format!(
                            "frame {f}: key 0x{vk:02X} ({}) after frame {last}",
                            a.name()
                        ));
                    }
                    Err(e) => log.push(format!("frame {f}: {e}")),
                },
                Step::Hold(at, n) => {
                    self.mouse = at;
                    out.push(UiEvent::CursorMoved(at));
                    out.push(UiEvent::Press {
                        button: PointerButton::Left,
                        at,
                    });
                    self.held = Some((at, f + n));
                    log.push(format!(
                        "frame {f}: hold {} {} {n} after frame {last}",
                        at.x, at.y
                    ));
                    break;
                }
                Step::Frame(_) | Step::Wait(_) => {}
            }
        }
        (out, log)
    }

    /// The headless click view: the 800 × 600 play frame, no panel open,
    /// the `play` preview's hover pick (d2rs-own, unverified, decision
    /// D1: [`crate::bridge::hover::pick`] over the model's unit cells, as
    /// `world_view::present` sets it), no shake.
    fn view(&self) -> ClickView {
        let size = FrameSize::play();
        ClickView {
            size,
            open_mode: 0,
            right_panel_bottom: size.play_height(),
            skill_y_limit: size.play_height(),
            mouse: (self.mouse.x, self.mouse.y),
            game_menu_open: false,
            pick: true,
            shake: (0, 0),
            prev_hover: Some(self.prev_pick),
        }
    }

    /// One client pass after server frame `last`: the due steps' events
    /// through the handlers the window path runs, in its order
    /// (`world_view::present`): the run lock toggles, the belt keys
    /// (C→S 0x26), the world-click dispatcher (C→S messages sent at once;
    /// the held repeat), the pending interaction noted, then weapon swap
    /// (0x60) and speech (0x3F). Returns the log lines.
    pub fn apply<L: ServerLink>(
        &mut self,
        bridge: &mut Bridge<L>,
        last: i32,
    ) -> Result<Vec<String>, BridgeError> {
        let local_at = self.walk.as_ref().and_then(|(p, _, _)| p.position());
        let cam = script_camera(bridge.world(), local_at);
        let world = bridge.world();
        let (events, log) = self.events(last, &mut |sel| match &cam {
            Some(c) => unit_point(world, c, sel),
            None => Err("no local player".into()),
        });
        // The pass's draw: the hover under the cursor, seen by the next press.
        let next_pick = cam
            .as_ref()
            .and_then(|c| crate::bridge::hover::pick(world, c, (self.mouse.x, self.mouse.y)));
        if events.is_empty() && !self.holding() {
            self.prev_pick = next_pick;
            return Ok(log);
        }
        let toggle = UiEvent::Action(ActionId(Action::ToggleRun.index() as u16));
        for _ in events.iter().filter(|e| **e == toggle) {
            self.run.toggle_run();
        }
        crate::bridge::belt::send_keys(bridge, &events, Default::default())?;
        let view = self.view();
        let (outs, _) = world_clicks(
            bridge,
            &mut self.click,
            view,
            &events,
            self.run.word(),
            local_at,
        )?;
        let pressed = events.iter().any(|e| matches!(e, UiEvent::Press { .. }));
        self.interact.note(&outs, pressed);
        self.prev_pick = next_pick;
        super::swap_key::send_swaps(&events, bridge)?;
        super::swap_key::send_says(&events, bridge)?;
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    #[test]
    fn parses_every_step_and_rejects_bad_ones() {
        assert_eq!(
            parse("move 320 240; wait 50;click 600 300 ; rclick 1 2;").unwrap(),
            [
                Step::Move(p(320, 240)),
                Step::Wait(50),
                Step::Click(PointerButton::Left, p(600, 300)),
                Step::Click(PointerButton::Right, p(1, 2)),
            ]
        );
        for bad in [
            "jump 1 2",
            "click 1",
            "click 800 0",
            "move -1 0",
            "wait -3",
            "wait",
            "click a b",
            "key",
            "key F1X",
            "key 0xZZ",
            "key I J",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert_eq!(parse(" ; ").unwrap(), []);
        assert_eq!(
            parse("key I; key esc; key TAB; key 0xC0; key 7").unwrap(),
            [
                Step::Key(0x49),
                Step::Key(0x1B),
                Step::Key(0x09),
                Step::Key(0xC0),
                Step::Key(0x37),
            ]
        );
        assert_eq!(
            [0x49, 0x1B, 0x09, 0xC0, 0x37].map(play_key),
            [
                Some(Key::I),
                Some(Key::Escape),
                Some(Key::Tab),
                Some(Key::Grave),
                Some(Key::Digit7),
            ]
        );
    }

    #[test]
    fn keys_run_on_their_tick_without_using_it_up() {
        let mut s = InputScript::new(parse("wait 2; key I; move 1 2; wait 1; key C").unwrap());
        assert_eq!(s.events(1), []);
        assert_eq!(s.take_keys(), []);
        assert_eq!(s.events(3), [UiEvent::CursorMoved(p(1, 2))]);
        assert_eq!(s.take_keys(), [Key::I]);
        assert_eq!(s.take_keys(), []);
        assert_eq!(s.events(4), []);
        assert_eq!(s.take_keys(), [Key::C]);
        assert!(s.done());
    }

    // Covers: specs/tools/scenario-diff.md §2 r4
    #[test]
    fn parses_the_shared_frame_form() {
        assert_eq!(
            parse("frame 10; click 600 300; hold 1 2 3; frame 20; key r; key F2; key 0x41")
                .unwrap(),
            [
                Step::Frame(10),
                Step::Click(PointerButton::Left, p(600, 300)),
                Step::Hold(p(1, 2), 3),
                Step::Frame(20),
                Step::Key(u32::from(b'R')),
                Step::Key(0x71),
                Step::Key(0x41),
            ]
        );
        for bad in [
            "frame 0",
            "frame",
            "frame 5; frame 4",
            "hold 1 2 3",
            "frame 3; hold 1 2 0",
            "frame 3; hold 1 2",
            "key",
            "key nope",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert_eq!(vk_code("esc"), Some(0x1B));
        assert_eq!(vk_code("F12"), Some(0x7B));
        assert_eq!(vk_code("7"), Some(u32::from(b'7')));
    }

    #[test]
    fn steps_run_on_their_ticks_and_a_click_releases_on_the_next() {
        let mut s =
            InputScript::new(parse("move 320 240; wait 10; click 600 300; move 5 6").unwrap());
        assert_eq!(s.events(1), [UiEvent::CursorMoved(p(320, 240))]);
        assert_eq!(s.cursor(), Some(FramePos::Inside(p(320, 240))));
        for t in 2..11 {
            assert_eq!(s.events(t), [], "tick {t}");
        }
        let at = p(600, 300);
        let button = PointerButton::Left;
        assert_eq!(
            s.events(11),
            [UiEvent::CursorMoved(at), UiEvent::Press { button, at }]
        );
        assert_eq!(s.events(12), [UiEvent::Release { button, at }]);
        assert!(!s.done());
        assert_eq!(s.events(13), [UiEvent::CursorMoved(p(5, 6))]);
        assert!(s.done());
        assert_eq!(s.events(14), []);
    }

    // Covers: specs/tools/scenario-diff.md §2 r4
    #[test]
    fn play_waits_for_frame_minus_one_and_holds_n_ticks() {
        let mut s = InputScript::new(parse("frame 5; hold 10 20 3; move 1 1").unwrap());
        for t in 1..4 {
            assert_eq!(s.events(t), [], "tick {t}");
        }
        let (at, button) = (p(10, 20), PointerButton::Left);
        assert_eq!(
            s.events(4),
            [UiEvent::CursorMoved(at), UiEvent::Press { button, at }]
        );
        assert_eq!(s.events(5), []);
        assert_eq!(s.events(6), []);
        assert_eq!(s.events(7), [UiEvent::Release { button, at }]);
        assert_eq!(s.events(8), [UiEvent::CursorMoved(p(1, 1))]);
        assert!(s.done());
    }

    // Covers: specs/tools/scenario-diff.md §3 r8
    #[test]
    fn headless_applies_each_step_after_frame_f_minus_one() {
        assert!(
            Headless::new(parse("click 1 1").unwrap()).is_err(),
            "no frame"
        );
        assert!(Headless::new(parse("frame 2; wait 3").unwrap()).is_err());
        assert!(Headless::new(parse("frame 2; key i").unwrap()).is_err());
        let mut none = |_: &UnitSel| -> Result<(UnitKey, Point), String> { Err("none".into()) };
        let mut h = Headless::new(
            parse("frame 10; click 600 300; hold 1 2 3; frame 20; move 5 6; frame 21; rclick 7 8")
                .unwrap(),
        )
        .unwrap();
        let left = PointerButton::Left;
        for f in 0..9 {
            assert_eq!(h.events(f, &mut none).0, [], "frame {f}");
        }
        let (at, hold) = (p(600, 300), p(1, 2));
        let (ev, log) = h.events(9, &mut none);
        assert_eq!(
            ev,
            [
                UiEvent::CursorMoved(at),
                UiEvent::Press { button: left, at },
                UiEvent::Release { button: left, at },
                UiEvent::CursorMoved(hold),
                UiEvent::Press {
                    button: left,
                    at: hold
                },
            ]
        );
        assert_eq!(
            log,
            [
                "frame 10: click 600 300 after frame 9",
                "frame 10: hold 1 2 3 after frame 9"
            ]
        );
        assert!(h.holding());
        assert_eq!(h.events(10, &mut none).0, []);
        assert_eq!(h.events(11, &mut none).0, []);
        // hold 3 from frame 10: released before frame 13's drain
        assert_eq!(
            h.events(12, &mut none).0,
            [UiEvent::Release {
                button: left,
                at: hold
            }]
        );
        assert!(!h.holding());
        assert_eq!(h.events(18, &mut none).0, []);
        assert_eq!(h.events(19, &mut none).0, [UiEvent::CursorMoved(p(5, 6))]);
        // a late stop: the step still runs, noted
        let (ev, log) = h.events(25, &mut none);
        assert_eq!(ev.len(), 3);
        assert_eq!(log[0], "frame 21 late: after frame 25");
        assert_eq!(h.pending(), 0);
    }

    mod headless_units {
        use std::sync::{Arc, Mutex};

        use d2_proto::PROTOCOL_VERSION;

        use super::super::*;
        use crate::bridge::dispatch::Dispatch;
        use crate::bridge::link::{LinkError, Pumped, SendQueue, Sent};
        use crate::bridge::skills::{SkillEntry, SkillList};
        use crate::bridge::world::{
            ClientUnit, ItemData, ItemRecord, KindData, PlayerData, SkillRow, MONSTER, PLAYER,
        };

        #[derive(Clone, Default)]
        struct RecordingLink {
            sent: Arc<Mutex<Vec<Vec<u8>>>>,
        }

        impl ServerLink for RecordingLink {
            fn protocol_version(&self) -> u32 {
                PROTOCOL_VERSION
            }
            fn send(&mut self, _: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
                self.sent.lock().unwrap().push(msg.to_vec());
                Ok(Sent::Queued)
            }
            fn pump(&mut self) -> Result<Pumped, LinkError> {
                Ok(Pumped { ticked: false })
            }
            fn receive(&mut self) -> Vec<Vec<u8>> {
                Vec::new()
            }
        }

        const ME: UnitKey = UnitKey {
            unit_type: PLAYER,
            guid: 1,
        };

        fn monster(guid: u32, class: u32, cell: (u16, u16), mode: u32) -> ClientUnit {
            let mut u = ClientUnit::new(UnitKey::new(MONSTER, guid));
            u.class = class;
            u.position = Some(cell);
            u.mode = mode;
            u.flag_4 = true;
            u
        }

        /// Synthetic fixture: in game (out of town), the local player at
        /// (100, 100) in mode 1 with stamina and Attack (anim 7, range 1)
        /// as its left skill; monsters 9 (class 19, at (103, 100)), 10
        /// (class 19, at (110, 100)), 11 (class 20, dead, at (101, 101)).
        fn scene() -> (Bridge<RecordingLink>, RecordingLink) {
            let link = RecordingLink::default();
            let mut b = Bridge::with_dispatch(link.clone(), Dispatch::empty()).unwrap();
            b.set_skill_rows(vec![SkillRow {
                anim: 7,
                range: 1,
                ingame: true,
                ..SkillRow::default()
            }]);
            let w = b.world_mut();
            let mut p = ClientUnit::new(ME);
            p.position = Some((100, 100));
            p.server_point = (100, 100);
            p.mode = 1;
            p.kind = KindData::Player(PlayerData::default());
            p.stats.insert(10, 100 << 8);
            p.skills = Some(SkillList {
                entries: vec![SkillEntry {
                    skill: crate::controls::click::ATTACK,
                    mode: 7,
                    base: 1,
                    owner: crate::bridge::skills::NATIVE,
                    ..SkillEntry::default()
                }],
                left: Some(0),
                ..SkillList::default()
            });
            w.units.insert(ME, p);
            w.local_player = Some(ME);
            w.in_game = true;
            for u in [
                monster(9, 19, (103, 100), 1),
                monster(10, 19, (110, 100), 1),
                monster(11, 20, (101, 101), 12),
            ] {
                w.units.insert(u.key, u);
            }
            (b, link)
        }

        fn sel(text: &str) -> UnitSel {
            match parse(text).unwrap()[..] {
                [Step::ClickUnit(s)] => s,
                ref other => panic!("{other:?}"),
            }
        }

        // Covers: specs/tools/scenario-diff.md §2 r4
        #[test]
        fn clickunit_parses_autostarts_form() {
            let s = sel("clickunit 1 19,0x14");
            assert_eq!(
                (s.button, s.unit_type, &s.classes[..2], s.count, s.offset),
                (PointerButton::Left, 1, &[19, 20][..], 2, (0, -8))
            );
            let r = sel("rclickunit 2 * 3 -12");
            assert_eq!(
                (r.button, r.any(), r.offset),
                (PointerButton::Right, true, (3, -12))
            );
            assert_eq!(r.text(), "rclickunit 2 * 3 -12");
            for bad in [
                "clickunit 1",
                "clickunit 6 19",
                "clickunit 1 x",
                "clickunit 1 19 4",
                "clickunit 1 1,2,3,4,5,6,7,8,9",
            ] {
                assert!(parse(bad).is_err(), "{bad}");
            }
        }

        // Covers: specs/tools/scenario-diff.md §2 r4; specs/render/camera.md §4
        #[test]
        fn unit_point_is_the_nearest_units_draw_point_plus_the_offset() {
            let (b, _) = scene();
            let w = b.world();
            let cam = script_camera(w, None).unwrap();
            let (k, at) = unit_point(w, &cam, &sel("clickunit 1 19")).unwrap();
            assert_eq!(k, UnitKey::new(MONSTER, 9), "the nearer of the two");
            let (fx, fy) = unit_feet(&cam, MONSTER, (103, 100));
            assert_eq!((at.x, at.y), (fx, fy - 8));
            // the local player stands at the frame centre's draw point
            assert_eq!(unit_feet(&cam, PLAYER, (100, 100)), (400, 292));
            // `*`: living units only (the dead 11 is nearer)
            let (k, _) = unit_point(w, &cam, &sel("clickunit 1 *")).unwrap();
            assert_eq!(k.guid, 9);
            // a listed class keeps the dead
            let (k, _) = unit_point(w, &cam, &sel("clickunit 1 20")).unwrap();
            assert_eq!(k.guid, 11);
            assert!(unit_point(w, &cam, &sel("clickunit 1 99")).is_err());
            assert!(unit_point(w, &cam, &sel("clickunit 1 19 0 -600")).is_err());
        }

        // Covers: specs/tools/scenario-diff.md §3 r8; specs/ui/controls.md §6 r8
        #[test]
        fn headless_clickunit_on_a_monster_sends_the_skill_on_the_unit() {
            let (mut b, link) = scene();
            let mut h = Headless::new(parse("frame 3; clickunit 1 19").unwrap()).unwrap();
            assert!(h.apply(&mut b, 1).unwrap().is_empty());
            // the hover frame: the cursor before frame 3, the click before frame 4
            let log = h.apply(&mut b, 2).unwrap();
            assert!(
                log[0].starts_with("frame 3: clickunit 1 19 0 -8: unit 1:9 at ("),
                "{log:?}"
            );
            assert!(link.sent.lock().unwrap().is_empty(), "no click yet");
            assert_eq!(h.pending(), 2);
            let log = h.apply(&mut b, 3).unwrap();
            assert!(log[0].starts_with("frame 4: click "), "{log:?}");
            let mut want = vec![0x06];
            want.extend_from_slice(&1u32.to_le_bytes());
            want.extend_from_slice(&9u32.to_le_bytes());
            assert_eq!(link.sent.lock().unwrap().as_slice(), [want]);
            assert_eq!(h.pending(), 0);
        }

        // Covers: specs/tools/scenario-diff.md §2 r4, §3 r8
        #[test]
        fn headless_click_on_a_monsters_body_picks_it() {
            let (mut b, link) = scene();
            let cam = script_camera(b.world(), None).unwrap();
            // 40 rows above the feet: the cell under the point is not the
            // monster's, the hover box (hover::pick) still holds it. The
            // cursor moves a frame before the press (1.14d hovers while it
            // draws).
            let (fx, fy) = unit_feet(&cam, MONSTER, (110, 100));
            let script = format!(
                "frame 1; move {fx} {}; frame 2; click {fx} {}",
                fy - 40,
                fy - 40
            );
            let mut h = Headless::new(parse(&script).unwrap()).unwrap();
            h.apply(&mut b, 0).unwrap();
            h.apply(&mut b, 1).unwrap();
            let sent = link.sent.lock().unwrap();
            assert_eq!(sent.len(), 1, "{sent:?}");
            assert_eq!(sent[0][5..9], 10u32.to_le_bytes(), "{sent:?}");
        }

        // Covers: specs/tools/scenario-diff.md §2 r4
        #[test]
        fn headless_press_in_the_pass_of_its_move_is_a_point_click() {
            let (mut b, link) = scene();
            let cam = script_camera(b.world(), None).unwrap();
            let (fx, fy) = unit_feet(&cam, MONSTER, (110, 100));
            let script = format!("frame 1; click {fx} {}", fy - 40);
            let mut h = Headless::new(parse(&script).unwrap()).unwrap();
            h.apply(&mut b, 0).unwrap();
            let sent = link.sent.lock().unwrap();
            assert!(
                sent.iter().all(|m| m[0] != 0x06),
                "no skill on a unit nothing hovered: {sent:?}"
            );
        }

        // Covers: specs/ui/controls.md §7 r2, §7 r3
        #[test]
        fn headless_keys_run_their_world_actions() {
            let b = Preset::Original.bindings().unwrap();
            let act = |k: &str| headless_key_action(&b, vk_code(k).unwrap());
            assert_eq!(act("1"), Ok(Action::BeltSlot1));
            assert_eq!(act("4"), Ok(Action::BeltSlot4));
            assert_eq!(act("R"), Ok(Action::ToggleRun));
            assert_eq!(act("W"), Ok(Action::SwapWeapons));
            for refused in ["I", "TAB", "F1", "F8", "ESC", "SHIFT", "5"] {
                assert!(act(refused).is_err(), "{refused}");
            }
            assert!(Headless::new(parse("frame 1; key F1").unwrap()).is_err());

            // `key w`: C→S 0x60; `key r` then a ground click: a run.
            let (mut br, link) = scene();
            let mut h =
                Headless::new(parse("frame 1; key w; key r; click 600 450").unwrap()).unwrap();
            let log = h.apply(&mut br, 0).unwrap();
            assert_eq!(log.len(), 3, "{log:?}");
            let sent = link.sent.lock().unwrap();
            let walks: Vec<_> = sent
                .iter()
                .filter_map(|m| crate::bridge::predict::walk_of(m))
                .collect();
            assert_eq!(walks.len(), 1, "{sent:?}");
            assert!(walks[0].run, "the run lock");
            assert_eq!(
                sent.last().unwrap(),
                &vec![0x60],
                "the swap after the click"
            );
        }

        // Covers: specs/tools/scenario-diff.md §3 r8
        #[test]
        fn play_clickunit_moves_then_clicks_a_tick_later() {
            let at = Point { x: 300, y: 200 };
            let mut found =
                |_: &UnitSel| -> Result<(UnitKey, Point), String> { Ok((UnitKey::new(1, 9), at)) };
            let mut s = InputScript::new(parse("clickunit 1 19; move 1 1").unwrap());
            assert_eq!(s.events_with(1, &mut found), [UiEvent::CursorMoved(at)]);
            let button = PointerButton::Left;
            assert_eq!(
                s.events_with(2, &mut found),
                [UiEvent::CursorMoved(at), UiEvent::Press { button, at }]
            );
            assert_eq!(
                s.events_with(3, &mut found),
                [UiEvent::Release { button, at }]
            );
            let mut none = |_: &UnitSel| -> Result<(UnitKey, Point), String> { Err("x".into()) };
            let mut t = InputScript::new(parse("clickunit 1 19").unwrap());
            assert_eq!(t.events_with(1, &mut none), []);
            assert_eq!(t.take_notes(), ["clickunit 1 19 0 -8: x"]);
            assert!(t.done());
        }

        // Covers: specs/ui/controls.md §7 r2, §7 r3
        #[test]
        fn headless_belt_key_sends_0x26() {
            let (mut b, link) = scene();
            let w = b.world_mut();
            // A potion in belt slot 0: flags 0x10, version 0x65, mode 2, x 0.
            let k = UnitKey::new(4, 7);
            let mut s = vec![0u8; 12];
            s[0] = 0x10;
            let mut bits: u64 = 0x65 | (2 << 10);
            for byte in s.iter_mut().skip(4).take(4) {
                *byte = bits as u8;
                bits >>= 8;
            }
            let mut u = ClientUnit::new(k);
            u.kind = KindData::Item(ItemData {
                last: Some(ItemRecord {
                    id: 0x9C,
                    action: 0x0E,
                    category: 0x10,
                    owner: None,
                    seq: 0,
                    stream: s,
                }),
                ..ItemData::default()
            });
            w.units.insert(k, u);
            w.belt_ready[0] = true;
            let mut h = Headless::new(parse("frame 1; key 1").unwrap()).unwrap();
            h.apply(&mut b, 0).unwrap();
            assert_eq!(
                link.sent.lock().unwrap().as_slice(),
                [vec![0x26, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]]
            );
        }

        // Covers: specs/tools/state-snapshot.md §3 r4
        /// With a prediction the model records it for the position check
        /// (rule 8 takes the server's point); without one nothing is set.
        #[test]
        fn sync_local_records_the_prediction_for_the_check() {
            let (mut b, _) = scene();
            let plain = Headless::new(Vec::new()).unwrap();
            plain.sync_local(&mut b);
            assert!(b.world().predicted(b.world().local().unwrap()).is_none());
            let mut h = Headless::new(Vec::new())
                .unwrap()
                .with_prediction(WalkTap::default(), None);
            h.observe(b.world(), false);
            h.sync_local(&mut b);
            assert!(b.world().predicted(b.world().local().unwrap()).is_some());
        }
    }
}
