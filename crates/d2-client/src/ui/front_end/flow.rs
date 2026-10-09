// Spec: specs/ui/frontend-menus.md (§F1.3 screen flow), specs/ui/frontend-credits.md (C2, C3)
//! The screen-flow table. Controls never name their target screen: they emit
//! a [`Trigger`] and [`next`] resolves it, so a screen module needs no edit
//! here. The conditions (saves found, difficulties open) come from [`FlowCtx`].

use super::ScreenId;

/// What happened on a screen (the "Trigger" column of §F1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// Trademark: click on the image, a key of its list, or the 9 s timer.
    Continue,
    /// Main menu: Single Player.
    SinglePlayer,
    /// Main menu: Credits.
    Credits,
    /// Main menu: Cinematics.
    Cinematics,
    /// Exit button or Esc (descriptors 5101 EXIT, hotkey 27).
    Exit,
    /// Character select: Create New.
    CreateNew,
    /// OK button or Enter (5102).
    Ok,
    /// Difficulty popup button: 0 Normal, 1 Nightmare, 2 Hell.
    Difficulty(u8),
    /// The in-game exit (`0x50` code 23 path): back to the main menu.
    GameExit,
    /// Options menu: Configure Controls (`0x0047F400`, §O9 r1).
    ConfigureControls,
}

/// How the game is started when a flow ends in a game load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameLoad {
    /// Chosen in the difficulty popup; `None`: the character's own (no box).
    pub difficulty: Option<u8>,
    /// Came from character create (a new character).
    pub new_character: bool,
}

/// Where a trigger leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Screen(ScreenId),
    /// Leave the front end: program exit.
    Exit,
    /// Leave the front end into the game (`0x00434A00`).
    GameLoad(GameLoad),
    /// The trigger does nothing on this screen.
    Stay,
}

/// The facts the conditional rows read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlowCtx {
    /// `<save dir>*.d2s` found something (`0x00430BC0`).
    pub saves_found: bool,
    /// Difficulties open for the chosen character (`0x00439840`).
    pub difficulties_open: u8,
}

/// §F1.3, single player. `from` is the screen the trigger fired on.
pub fn next(from: ScreenId, t: Trigger, ctx: FlowCtx) -> Next {
    use super::screens::ids::*;
    use Trigger::*;
    match (from, t) {
        (f, Continue) if f == TRADEMARK => Next::Screen(MAIN_MENU),
        (f, SinglePlayer) if f == MAIN_MENU => {
            if ctx.saves_found {
                Next::Screen(CHAR_SELECT)
            } else {
                Next::Screen(CHAR_CREATE)
            }
        }
        (f, Exit) if f == MAIN_MENU => Next::Exit,
        (f, Credits) if f == MAIN_MENU => Next::Screen(CREDITS),
        (f, Cinematics) if f == MAIN_MENU => Next::Screen(CINEMATICS),
        // C3 r5, C8: both leave to the main menu.
        (f, Exit) if f == CREDITS || f == CINEMATICS => Next::Screen(MAIN_MENU),
        (f, Exit) if f == CHAR_SELECT => Next::Screen(MAIN_MENU),
        (f, CreateNew) if f == CHAR_SELECT => Next::Screen(CHAR_CREATE),
        (f, Ok) if f == CHAR_SELECT => {
            if ctx.difficulties_open > 1 {
                Next::Screen(DIFFICULTY)
            } else {
                Next::GameLoad(GameLoad {
                    difficulty: None,
                    new_character: false,
                })
            }
        }
        // Also when no character exists.
        (f, Exit) if f == CHAR_CREATE => Next::Screen(CHAR_SELECT),
        (f, Ok) if f == CHAR_CREATE => Next::GameLoad(GameLoad {
            difficulty: None,
            new_character: true,
        }),
        (f, Difficulty(d)) if f == DIFFICULTY => Next::GameLoad(GameLoad {
            difficulty: Some(d),
            new_character: false,
        }),
        (f, Exit) if f == DIFFICULTY => Next::Screen(CHAR_SELECT),
        // Configure Controls: Cancel / Accept return to the Options menu
        // (q-menu-controls, REC-184).
        (f, ConfigureControls) if f == OPTIONS => Next::Screen(CONTROLS),
        (f, Exit | Ok) if f == CONTROLS => Next::Screen(OPTIONS),
        // In-game exit: the main menu. Measured (REC-200): Save and Exit
        // from a single-player game leaves to the main menu (launcher mode
        // 4 at once, the Single Player button screen 0.5 s later), not to
        // character select; `traces/frontend/rec200-save-exit.json`.
        (_, GameExit) => Next::Screen(MAIN_MENU),
        _ => Next::Stay,
    }
}
