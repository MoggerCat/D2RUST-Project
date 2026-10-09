// Spec: specs/ui/frontend-menus.md (§F1.4 main menu, §F1.5 title animation)
//! The main menu (`0x004336C0`): background, the two logo halves, seven
//! buttons and the version text, in creation (= draw) order. Multiplayer
//! buttons are built disabled (Phase 7+). Sounds are named only (deferred).

use crate::ui::front_end::control::{vk, Action, Control, ControlKind};
use crate::ui::front_end::flow::Trigger;
use crate::ui::front_end::screen::{FrontCtx, Screen};
use crate::ui::front_end::{Registry, MAIN_MENU};

pub const BACKGROUND: &str = r"FrontEnd\gameselectscreenEXP";
pub const BACKGROUND_CLASSIC: &str = r"FrontEnd\gameselectscreen";
/// Button cels (§F1.4 r3 art column; all exist in the install).
pub const WIDE: &str = r"FrontEnd\WideButtonBlank";
pub const NARROW: &str = r"FrontEnd\NarrowButtonBlank";
pub const SHORT: &str = r"CharSelect\ShortButtonBlank";
/// The Battle.net button's art (§F1.4 r3 row 17, descriptor 17, global
/// `0x00779754`, `d2exp.mpq`).
pub const WIDE2: &str = r"FrontEnd\WideButtonBlank02";
/// Logo halves (§F1.5 r1): the black base cel (the fire overlay is drawn
/// additively over it by the host). File names: §F1.5 r1.
pub const LOGO_LEFT: &str = r"FrontEnd\D2logoBlackLeft";
pub const LOGO_RIGHT: &str = r"FrontEnd\D2logoBlackRight";
/// "v %d.%d%c" with 1, 14, 'd' (r4).
pub const VERSION_TEXT: &str = "v 1.14d";
/// Font16 (descriptor 0x115, record `0x007089C4`, r4).
const VERSION_FONT: u16 = 1;

/// Button string ids (r3).
pub const STR_SINGLE_PLAYER: u32 = 5106;
pub const STR_BATTLE_NET: u32 = 5107;
pub const STR_OTHER_MULTIPLAYER: u32 = 5108;
pub const STR_EXIT: u32 = 5109;
pub const STR_CREDITS: u32 = 5110;
pub const STR_CINEMATICS: u32 = 5111;

struct MainMenu;

fn button(
    art: &'static str,
    x: i32,
    y: i32,
    w: u16,
    h: u16,
    string: u32,
    action: Action,
) -> Control {
    // The multiplayer buttons (Phase 7+) have no action here but stay
    // enabled: r2 disables them only when `0x004FAC90` ≠ 0, and the 1.14d
    // screenshot draws them as enabled buttons (a disabled one would draw
    // with mode 1, §F1.1 r4). A click on them does nothing.
    Control::new(ControlKind::Button, x, y, w, h)
        .with_art(art)
        .with_string(string)
        .with_action(action)
}

/// Frame to draw for a resting, pressed or disabled button (r3 art, §F1.1
/// r4): the same rule the host applies. // d2rs-own, unverified: disabled
/// buttons draw the up frames (the cels have no third state).
pub fn button_frame(c: &Control, tile: u32, pressed: bool) -> u32 {
    let tiles = crate::ui::front_end::control::button_tiles(c.w, c.h);
    crate::ui::front_end::control::button_frame(tile, tiles, pressed, c.enabled, false)
}

impl Screen for MainMenu {
    fn build(&mut self, ctx: &mut FrontCtx) -> Vec<Control> {
        // Classic: the first four buttons sit 100 px higher.
        let up = if ctx.expansion { 0 } else { 100 };
        let bg = if ctx.expansion {
            BACKGROUND
        } else {
            BACKGROUND_CLASSIC
        };
        vec![
            Control::new(ControlKind::Image, 0, 599, 800, 600).with_art(bg),
            Control::new(ControlKind::AnimImage, 400, 120, 181, 170).with_art(LOGO_LEFT),
            Control::new(ControlKind::AnimImage, 400, 120, 188, 177).with_art(LOGO_RIGHT),
            button(
                WIDE,
                264,
                324 - up,
                272,
                35,
                STR_SINGLE_PLAYER,
                Action::Trigger(Trigger::SinglePlayer),
            ),
            button(WIDE2, 264, 366 - up, 272, 35, STR_BATTLE_NET, Action::None),
            // The gateway label is set at run time (0x00431AF0): none here.
            button(NARROW, 264, 391 - up, 272, 25, 0, Action::None),
            button(
                WIDE,
                264,
                433 - up,
                272,
                35,
                STR_OTHER_MULTIPLAYER,
                Action::None,
            ),
            button(
                SHORT,
                264,
                528,
                135,
                25,
                STR_CREDITS,
                Action::Trigger(Trigger::Credits),
            ),
            button(
                SHORT,
                402,
                528,
                135,
                25,
                STR_CINEMATICS,
                Action::Trigger(Trigger::Cinematics),
            ),
            button(
                WIDE,
                264,
                568,
                272,
                35,
                STR_EXIT,
                Action::Trigger(Trigger::Exit),
            )
            .with_hotkey(vk::ESC),
            {
                // r4: (0, 599) 200 × 40, flags 2 (centred), colour 0.
                Control::new(ControlKind::Text, 0, 599, 200, 40)
                    .with_font(VERSION_FONT, 2)
                    .with_text(VERSION_TEXT, 0)
            },
        ]
    }
}

pub fn register(reg: &mut Registry) {
    reg.register(MAIN_MENU, Box::new(MainMenu));
}
