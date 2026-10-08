// Spec: specs/ui/frontend-credits.md (C5 video hook, C6 progress byte N, C8 cinematics menu)
//! The cinematics menu (`0x00431600`): 7 entries (expansion) or 5 (classic)
//! plus CANCEL. Entries 1…L(N) are enabled (C8 r3); a click plays the video
//! through the [`VideoHook`] and leaves the menu as it was (C8 r5, r7);
//! CANCEL / Esc go to the main menu (C8 r6).
//!
//! The screen needs the progress store and the video hook, which
//! [`FrontCtx`] does not carry, so it holds shared handles
//! ([`register_with`]). PROVISIONAL (REC-231): the C5 r4 input flush around
//! a video is the host's; the screen counts them in [`Handles::flushes`].

use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::front_end::control::{vk, Action, Control, ControlKind};
use crate::ui::front_end::flow::Trigger;
use crate::ui::front_end::screen::{FrontCtx, Screen};
use crate::ui::front_end::startup::{MemProgress, ProgressStore, StubVideo, VideoHook};
use crate::ui::front_end::{Registry, CINEMATICS};

/// Video file names of the seven entries (C5 r1), without size suffix.
pub const VIDEOS: [&str; 7] = [
    "d2intro",
    "Act02start",
    "Act03start",
    "Act04start",
    "Act04end",
    "D2x_Intro_",
    "D2x_Out_",
];

pub const BACKGROUND_EXP: &str = r"FrontEnd\gameselectscreenEXP";
pub const PANEL_EXP: &str = r"FrontEnd\CinematicsSelectionEXP";
pub const PANEL_CLASSIC: &str = r"FrontEnd\CinematicsSelection";
pub const WIDE_BUTTON: &str = r"FrontEnd\WideButtonBlank";
pub const MEDIUM_BUTTON: &str = r"FrontEnd\MediumButtonBlank";

/// Entry labels: expansion 21797–21803, classic 4137–4140 and 5113.
const LABELS_EXP: [u32; 7] = [21797, 21798, 21799, 21800, 21801, 21802, 21803];
const LABELS_CLASSIC: [u32; 5] = [4137, 4138, 4139, 4140, 5113];
/// Bottom edges of the entries (C8 table).
const Y_EXP: [i32; 7] = [181, 224, 268, 310, 353, 396, 439];
const Y_CLASSIC: [i32; 5] = [316, 359, 402, 445, 488];

/// Custom action ids: entry `i` is `ENTRY + i`.
const ENTRY: u32 = 100;

/// Level L(N) (C6 r2, `0x004313A0`).
pub fn level(n: u8) -> u8 {
    if n & 0x01 != 0 {
        7
    } else if n & 0x80 != 0 {
        6
    } else if n & 0x10 != 0 {
        5
    } else if n & 0x08 != 0 {
        4
    } else if n & 0x40 != 0 {
        3
    } else if n & 0x04 != 0 {
        2
    } else {
        1
    }
}

/// The in-game writer of N (C6 r3, `0x00482CA0`) for a video request.
/// `id`: the video id of `ui/frontend-loading.md` L10 (2 Act II start …
/// 5 `ACT04END`, 6 `D2X_INTRO_`, 7 `D2X_OUT_`). With N missing nothing is
/// written.
pub fn note_video_request(store: &mut dyn ProgressStore, id: u32) {
    let Some(n) = store.get() else { return };
    let want = match id {
        2 => 2,
        3 => 3,
        4 => 4,
        5 | 6 => 5,
        7 => 7,
        _ => 1,
    };
    if want > level(n) {
        store.set(match want {
            2 => 0x26,
            3 => 0x62,
            4 => 0x2A,
            5 => 0xB2,
            _ => 0x23,
        });
    }
}

/// Number of entries enabled for N (C8 r3).
pub fn enabled_entries(n: Option<u8>, expansion: bool) -> usize {
    // Missing N: level 1 (C6 r1).
    let l = usize::from(level(
        n.unwrap_or(crate::ui::front_end::startup::PROGRESS_DEFAULT),
    ));
    if expansion {
        l
    } else {
        l.min(5)
    }
}

/// The video path of an entry (C5 r1, stub form).
pub fn video_path(name: &str) -> String {
    format!(r"DATA\LOCAL\video\ENG\{name}640x292.bik")
}

/// What the host shares with the screen.
pub struct Handles {
    pub store: Rc<RefCell<dyn ProgressStore>>,
    pub video: Rc<RefCell<dyn VideoHook>>,
    /// Input flushes the host owes (two per video, C5 r4).
    pub flushes: u32,
}

struct Cinematics {
    h: Rc<RefCell<Handles>>,
}

impl Screen for Cinematics {
    fn build(&mut self, ctx: &mut FrontCtx) -> Vec<Control> {
        let n = self.h.borrow().store.borrow().get();
        // Missing N: the menu writes the default (C6 r1).
        if n.is_none() {
            self.h
                .borrow()
                .store
                .borrow_mut()
                .set(crate::ui::front_end::startup::PROGRESS_DEFAULT);
        }
        let on = enabled_entries(n, ctx.expansion);
        let mut v = Vec::new();
        let (labels, ys, title_y, panel, cancel_y): (&[u32], &[i32], i32, _, i32) = if ctx.expansion
        {
            v.push(Control::new(ControlKind::Image, 0, 599, 800, 600).with_art(BACKGROUND_EXP));
            v.push(Control::new(ControlKind::Image, 237, 505, 326, 427).with_art(PANEL_EXP));
            (&LABELS_EXP, &Y_EXP, 153, PANEL_EXP, 488)
        } else {
            // PROVISIONAL (REC-168): the classic background (descriptor 8,
            // with the logo) is the shell's; only the panel is drawn here.
            v.push(Control::new(ControlKind::Image, 237, 555, 326, 341).with_art(PANEL_CLASSIC));
            (&LABELS_CLASSIC, &Y_CLASSIC, 283, PANEL_CLASSIC, 538)
        };
        let _ = panel;
        let mut title = Control::new(ControlKind::Text, 262, title_y, 272, 35).with_string(5114);
        title.font = 7; // Font24 (0x007089BC); id per REC-231
        v.push(title);
        for (i, (&s, &y)) in labels.iter().zip(ys).enumerate() {
            let mut b = Control::new(ControlKind::Button, 262, y, 272, 35)
                .with_art(WIDE_BUTTON)
                .with_string(s)
                .with_action(Action::Custom(ENTRY + i as u32));
            if i >= on {
                b.enabled = false;
                if ctx.expansion {
                    b.string_id = 0; // empty label (C8 r3)
                }
            }
            v.push(b);
        }
        v.push(
            Control::new(ControlKind::Button, 334, cancel_y, 128, 35)
                .with_art(MEDIUM_BUTTON)
                .with_string(5103)
                .with_hotkey(vk::ESC)
                .with_action(Action::Trigger(Trigger::Exit)),
        );
        v
    }

    fn action(&mut self, ctx: &mut FrontCtx, id: u32) -> Option<Trigger> {
        let i = id.checked_sub(ENTRY)? as usize;
        let n = self.h.borrow().store.borrow().get();
        if i >= enabled_entries(n, ctx.expansion) {
            return None;
        }
        // C8 r5: flush, play, flush; the menu is rebuilt with N re-read,
        // and a menu video never changes N (r7).
        let mut h = self.h.borrow_mut();
        h.flushes += 1;
        h.video.borrow_mut().play(&video_path(VIDEOS[i]));
        h.flushes += 1;
        None
    }
}

/// Register with explicit handles (the host's settings file and video).
pub fn register_with(reg: &mut Registry, h: Rc<RefCell<Handles>>) {
    reg.register(CINEMATICS, Box::new(Cinematics { h }));
}

/// Default registration: in-memory N and the logging video stub.
pub fn register(reg: &mut Registry) {
    let h = Handles {
        store: Rc::new(RefCell::new(MemProgress::default())),
        video: Rc::new(RefCell::new(StubVideo)),
        flushes: 0,
    };
    register_with(reg, Rc::new(RefCell::new(h)));
}
