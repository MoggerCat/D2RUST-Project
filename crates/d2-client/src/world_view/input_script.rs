// Spec: specs/tools/facts-render.md (§5 r11)
//! `play --input SCRIPT`: scripted pointer input for a scene, the d2rs
//! counterpart of `tools/trace-recorder/autostart.py --input`. The steps
//! become the same [`UiEvent`]s the window would send, timed by server
//! ticks (the window's own pointer input is ignored while a script runs),
//! so a scene reaches the same place on every run. Client input only:
//! nothing here decides an outcome.

use crate::controls::keymap::vk_to_key;
use crate::controls::Key;
use crate::ui::{FramePos, Point, PointerButton, UiEvent};

/// One step of a script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `wait N`: N server ticks.
    Wait(u64),
    /// `move X Y`: the cursor to frame pixel (X, Y).
    Move(Point),
    /// `click X Y` / `rclick X Y`: cursor there, press, and the release
    /// on the next tick.
    Click(PointerButton, Point),
    /// `key K`: the key pressed on this tick, as the window's key press
    /// (its bound world action and its typed character). K is a letter
    /// or digit, `ESC`, `TAB`, `ENTER`, `SPACE`, or a Windows virtual key
    /// in hex (`0xC0`), as `autostart.py`'s `key`.
    Key(Key),
}

/// The virtual key of a `key` step's name (`autostart.py`'s names).
fn key_vk(name: &str) -> Option<u16> {
    match name.to_ascii_uppercase().as_str() {
        "ESC" => Some(0x1B),
        "TAB" => Some(0x09),
        "ENTER" => Some(0x0D),
        "SPACE" => Some(0x20),
        n if n.len() == 1 && n.as_bytes()[0].is_ascii_alphanumeric() => {
            Some(u16::from(n.as_bytes()[0]))
        }
        n => n
            .strip_prefix("0X")
            .and_then(|h| u16::from_str_radix(h, 16).ok()),
    }
}

/// Parses `wait N; move X Y; click X Y; rclick X Y; key K` (steps separated by
/// `;`, blank steps ignored). Coordinates are 800 × 600 frame pixels.
pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
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
            "key" => match args {
                [k] => Step::Key(
                    key_vk(k)
                        .and_then(vk_to_key)
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
    /// nothing else that tick), then moves up to and including the next
    /// press or wait.
    pub fn events(&mut self, tick: u64) -> Vec<UiEvent> {
        let mut out = Vec::new();
        if let Some((button, at)) = self.release.take() {
            out.push(UiEvent::Release { button, at });
            return out;
        }
        while self.next < self.steps.len() && tick >= self.resume_at {
            let step = self.steps[self.next];
            self.next += 1;
            match step {
                Step::Wait(n) => {
                    self.resume_at = tick + n;
                    break;
                }
                Step::Move(p) => {
                    out.push(UiEvent::CursorMoved(p));
                    self.cursor = Some(p);
                }
                Step::Key(k) => self.keys.push(k),
                Step::Click(button, at) => {
                    out.push(UiEvent::CursorMoved(at));
                    out.push(UiEvent::Press { button, at });
                    self.cursor = Some(at);
                    self.release = Some((button, at));
                    break;
                }
            }
        }
        out
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
                Step::Key(Key::I),
                Step::Key(Key::Escape),
                Step::Key(Key::Tab),
                Step::Key(Key::Grave),
                Step::Key(Key::Digit7),
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
}
