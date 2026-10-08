// Spec: specs/ui/frontend-menus.md (§F1.3 trademark row), specs/ui/frontend-credits.md (C2)
//! The trademark screen (`0x0042FB20`): image (descriptor 2), 9 s timer
//! (3), logo halves (6, 7) and the legal text (4). Click, a key of the C2
//! list, or the timer → main menu.

use crate::ui::front_end::control::{Action, Control, ControlKind};
use crate::ui::front_end::flow::Trigger;
use crate::ui::front_end::screen::{FrontCtx, Screen};
use crate::ui::front_end::{Registry, TRADEMARK};

/// Timer seconds (descriptor 3).
pub const TIMER_SECONDS: u32 = 9;

/// Keys whose key-down goes to the main menu (C2 r1): Backspace, Tab,
/// Enter, Space, Page Up/Down, End, Home, arrows, Delete, H, N, R, F1, Esc.
pub const KEYS: [u16; 18] = [
    8,
    9,
    13,
    32,
    33,
    34,
    35,
    36,
    37,
    38,
    39,
    40,
    46,
    b'H' as u16,
    b'N' as u16,
    b'R' as u16,
    112,
    27,
];

pub const BACKGROUND: &str = "FrontEnd\\trademarkscreenEXP";
pub const BACKGROUND_CLASSIC: &str = "FrontEnd\\trademark";

/// Legal text: descriptor 4, FontFormal12, (100, 580) 600×80.
const TEXT_FONT: u16 = 5;

struct Trademark;

impl Screen for Trademark {
    fn build(&mut self, ctx: &mut FrontCtx) -> Vec<Control> {
        let go = Action::Trigger(Trigger::Continue);
        let art = if ctx.expansion {
            BACKGROUND
        } else {
            BACKGROUND_CLASSIC
        };
        let mut text = Control::new(ControlKind::Text, 100, 580, 600, 80);
        text.font = TEXT_FONT;
        vec![
            Control::new(ControlKind::Image, 0, 599, 800, 600)
                .with_art(art)
                .with_keys(&KEYS)
                .with_action(go),
            Control::timer(TIMER_SECONDS, go),
            Control::new(ControlKind::AnimImage, 400, 120, 181, 170),
            Control::new(ControlKind::AnimImage, 400, 120, 188, 177),
            text,
        ]
    }
}

pub fn register(reg: &mut Registry) {
    reg.register(TRADEMARK, Box::new(Trademark));
}
