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
//!   ([`world_clicks`], the same call the window path makes after the UI),
//!   at the poke point: after the snapshot of frame F − 1, before frame
//!   F's drain (`scenario-diff.md` §2 r4).

use crate::bridge::click::ClickView;
use crate::bridge::link::ServerLink;
use crate::bridge::predict::{Predict, Speeds, WalkTap};
use crate::bridge::world::ClientWorld;
use crate::bridge::{Bridge, BridgeError};
use crate::controls::click::ClickState;
use crate::controls::keymap::vk_to_key;
use crate::controls::Key;
use crate::rules::camera::FrameSize;
use crate::ui::{FramePos, Point, PointerButton, UiEvent};

use super::ui_bind::world_clicks;

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
    /// world action and its typed character); headless refuses it.
    Key(u32),
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

/// Parses `wait N; move X Y; click X Y; rclick X Y; frame F; hold X Y N;
/// key K` (steps separated by `;`, blank steps ignored). Coordinates are
/// 800 × 600 frame pixels. `frame` numbers are ≥ 1 and never go back;
/// `hold` needs a `frame` step before it (its N counts frames); a `key`
/// is one the controls key table holds ([`play_key`]).
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
        }
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
    /// hands them to the window's key path.
    pub fn events(&mut self, tick: u64) -> Vec<UiEvent> {
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
/// §3 r8): the shared form only (it starts with `frame`; no `wait`, no
/// `key`), applied through the bridge at the poke point.
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
        if let Some(s) = steps
            .iter()
            .find(|s| matches!(s, Step::Wait(_) | Step::Key(_)))
        {
            return Err(format!(
                "state-dump --input: {s:?} is not applied headless (`wait` is \
                 side-specific; `key` needs the controls layer)"
            ));
        }
        Ok(Self {
            steps,
            next: 0,
            anchor: 0,
            held: None,
            mouse: Point { x: 0, y: 0 },
            click: ClickState::default(),
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

    /// Steps not run yet (the footer's "not reached").
    pub fn pending(&self) -> usize {
        self.steps.len() - self.next + usize::from(self.held.is_some())
    }

    /// Whether a button is held (the held repeat runs every pass).
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// The pointer events due after server frame `last` ran, and a log
    /// line per step (`frame F: click X Y at frame L`).
    pub fn events(&mut self, last: i32) -> (Vec<UiEvent>, Vec<String>) {
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
                Step::Frame(_) | Step::Wait(_) | Step::Key(_) => {}
            }
        }
        (out, log)
    }

    /// The headless click view: the 800 × 600 play frame, no panel open,
    /// no hover model (strict: a click on a unit is a point click), no
    /// shake.
    fn view(&self) -> ClickView {
        let size = FrameSize::play();
        ClickView {
            size,
            open_mode: 0,
            right_panel_bottom: size.play_height(),
            skill_y_limit: size.play_height(),
            mouse: (self.mouse.x, self.mouse.y),
            game_menu_open: false,
            pick: false,
            shake: (0, 0),
        }
    }

    /// One client pass after server frame `last`: the due steps' events
    /// through the world-click dispatcher (C→S messages sent at once),
    /// then the held repeat. Returns the log lines.
    pub fn apply<L: ServerLink>(
        &mut self,
        bridge: &mut Bridge<L>,
        last: i32,
    ) -> Result<Vec<String>, BridgeError> {
        let (events, log) = self.events(last);
        if events.is_empty() && !self.holding() {
            return Ok(log);
        }
        let view = self.view();
        let local_at = self.walk.as_ref().and_then(|(p, _, _)| p.position());
        world_clicks(bridge, &mut self.click, view, &events, 0, local_at)?;
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
        assert!(Headless::new(parse("frame 2; key r").unwrap()).is_err());
        let mut h = Headless::new(
            parse("frame 10; click 600 300; hold 1 2 3; frame 20; move 5 6; frame 21; rclick 7 8")
                .unwrap(),
        )
        .unwrap();
        let left = PointerButton::Left;
        for f in 0..9 {
            assert_eq!(h.events(f).0, [], "frame {f}");
        }
        let (at, hold) = (p(600, 300), p(1, 2));
        let (ev, log) = h.events(9);
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
        assert_eq!(h.events(10).0, []);
        assert_eq!(h.events(11).0, []);
        // hold 3 from frame 10: released before frame 13's drain
        assert_eq!(
            h.events(12).0,
            [UiEvent::Release {
                button: left,
                at: hold
            }]
        );
        assert!(!h.holding());
        assert_eq!(h.events(18).0, []);
        assert_eq!(h.events(19).0, [UiEvent::CursorMoved(p(5, 6))]);
        // a late stop: the step still runs, noted
        let (ev, log) = h.events(25);
        assert_eq!(ev.len(), 3);
        assert_eq!(log[0], "frame 21 late: after frame 25");
        assert_eq!(h.pending(), 0);
    }
}
