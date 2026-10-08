// Spec: specs/client/ui.md
//! The Bevy input edge of the UI (spec §A4): the only part of `ui` that
//! touches Bevy. It reads the cursor from a `Window` in physical pixels
//! and maps it into the frame with [`Presentation`]; everything after is
//! plain Rust. Key and button bindings to actions are the controls'
//! (task C9), which implements [`super::UiInput`] on top of this.

use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::window::Window;

use super::frame::{FrameError, FramePos, Presentation};
use super::panel::{ActionId, UiEvent};
use crate::controls::{Bindings, Context, Key};

/// The presentation for a window's current physical size.
pub fn presentation(window: &Window) -> Result<Presentation, FrameError> {
    Presentation::new(window.physical_width(), window.physical_height())
}

/// The window pixel holding a physical cursor position: Bevy reports it
/// as floats; the pixel is the floor of each coordinate.
pub fn window_pixel(physical: Vec2) -> (i64, i64) {
    (physical.x.floor() as i64, physical.y.floor() as i64)
}

/// Where the window's cursor is in the frame; `Outside` when the cursor
/// is not in the window.
pub fn cursor_frame_pos(window: &Window) -> Result<FramePos, FrameError> {
    let p = presentation(window)?;
    Ok(match window.physical_cursor_position() {
        Some(c) => {
            let (x, y) = window_pixel(c);
            p.to_frame(x, y)
        }
        None => FramePos::Outside,
    })
}

/// Bevy key codes of the controls' keyboard [`Key`]s (§A4, §A6): one row
/// per key; the mouse inputs are the pointer events. Keys are a d2rs
/// table (the original's key handling is `ui/controls.md`).
pub const KEY_CODES: &[(KeyCode, Key)] = {
    use KeyCode as C;
    &[
        (C::KeyA, Key::A),
        (C::KeyB, Key::B),
        (C::KeyC, Key::C),
        (C::KeyD, Key::D),
        (C::KeyE, Key::E),
        (C::KeyF, Key::F),
        (C::KeyG, Key::G),
        (C::KeyH, Key::H),
        (C::KeyI, Key::I),
        (C::KeyJ, Key::J),
        (C::KeyK, Key::K),
        (C::KeyL, Key::L),
        (C::KeyM, Key::M),
        (C::KeyN, Key::N),
        (C::KeyO, Key::O),
        (C::KeyP, Key::P),
        (C::KeyQ, Key::Q),
        (C::KeyR, Key::R),
        (C::KeyS, Key::S),
        (C::KeyT, Key::T),
        (C::KeyU, Key::U),
        (C::KeyV, Key::V),
        (C::KeyW, Key::W),
        (C::KeyX, Key::X),
        (C::KeyY, Key::Y),
        (C::KeyZ, Key::Z),
        (C::Digit0, Key::Digit0),
        (C::Digit1, Key::Digit1),
        (C::Digit2, Key::Digit2),
        (C::Digit3, Key::Digit3),
        (C::Digit4, Key::Digit4),
        (C::Digit5, Key::Digit5),
        (C::Digit6, Key::Digit6),
        (C::Digit7, Key::Digit7),
        (C::Digit8, Key::Digit8),
        (C::Digit9, Key::Digit9),
        (C::F1, Key::F1),
        (C::F2, Key::F2),
        (C::F3, Key::F3),
        (C::F4, Key::F4),
        (C::F5, Key::F5),
        (C::F6, Key::F6),
        (C::F7, Key::F7),
        (C::F8, Key::F8),
        (C::F9, Key::F9),
        (C::F10, Key::F10),
        (C::F11, Key::F11),
        (C::F12, Key::F12),
        (C::ShiftLeft, Key::LeftShift),
        (C::ShiftRight, Key::RightShift),
        (C::ControlLeft, Key::LeftCtrl),
        (C::ControlRight, Key::RightCtrl),
        (C::AltLeft, Key::LeftAlt),
        (C::AltRight, Key::RightAlt),
        (C::Space, Key::Space),
        (C::Enter, Key::Enter),
        (C::Escape, Key::Escape),
        (C::Tab, Key::Tab),
        (C::Backspace, Key::Backspace),
        (C::Insert, Key::Insert),
        (C::Delete, Key::Delete),
        (C::Home, Key::Home),
        (C::End, Key::End),
        (C::PageUp, Key::PageUp),
        (C::PageDown, Key::PageDown),
        (C::ArrowUp, Key::Up),
        (C::ArrowDown, Key::Down),
        (C::ArrowLeft, Key::Left),
        (C::ArrowRight, Key::Right),
        (C::Backquote, Key::Grave),
        (C::Minus, Key::Minus),
        (C::Equal, Key::Equals),
        (C::BracketLeft, Key::LeftBracket),
        (C::BracketRight, Key::RightBracket),
        (C::Backslash, Key::Backslash),
        (C::Semicolon, Key::Semicolon),
        (C::Quote, Key::Quote),
        (C::Comma, Key::Comma),
        (C::Period, Key::Period),
        (C::Slash, Key::Slash),
        (C::PrintScreen, Key::PrintScreen),
        (C::Pause, Key::Pause),
        (C::Numpad0, Key::Numpad0),
        (C::Numpad1, Key::Numpad1),
        (C::Numpad2, Key::Numpad2),
        (C::Numpad3, Key::Numpad3),
        (C::Numpad4, Key::Numpad4),
        (C::Numpad5, Key::Numpad5),
        (C::Numpad6, Key::Numpad6),
        (C::Numpad7, Key::Numpad7),
        (C::Numpad8, Key::Numpad8),
        (C::Numpad9, Key::Numpad9),
        (C::NumpadAdd, Key::NumpadAdd),
        (C::NumpadSubtract, Key::NumpadSubtract),
        (C::NumpadMultiply, Key::NumpadMultiply),
        (C::NumpadDivide, Key::NumpadDivide),
        (C::NumpadEnter, Key::NumpadEnter),
    ]
};

/// The controls key of a Bevy key code; `None` for a key the controls
/// do not name.
pub fn key_of(code: KeyCode) -> Option<Key> {
    KEY_CODES.iter().find(|(c, _)| *c == code).map(|(_, k)| *k)
}

/// The UI events of the keys pressed this frame: each key bound in the
/// world context to an action becomes [`UiEvent::Action`], in `pressed`
/// order (§A4: actions come from the bindings, never from raw keys).
pub fn key_actions(bindings: &Bindings, pressed: &[KeyCode]) -> Vec<UiEvent> {
    pressed
        .iter()
        .filter_map(|&c| key_of(c))
        .filter_map(|k| bindings.action_for(Context::World, k))
        .filter_map(|a| u16::try_from(a.index()).ok())
        .map(|i| UiEvent::Action(ActionId(i)))
        .collect()
}

/// The typed characters of the keys pressed this frame, for the text
/// the UI asks for (the gold dialog's digits): digit keys → the digit,
/// Backspace → 8, Enter → 0x0D, Escape → 0x1B, as [`UiEvent::Char`].
/// d2rs-own, unverified (D1): a raw key map (no layout, no shift); the
/// panels that take no text ignore these events.
pub fn key_chars(pressed: &[KeyCode]) -> Vec<UiEvent> {
    use KeyCode as C;
    const DIGITS: [(KeyCode, u16); 20] = [
        (C::Digit0, b'0' as u16),
        (C::Digit1, b'1' as u16),
        (C::Digit2, b'2' as u16),
        (C::Digit3, b'3' as u16),
        (C::Digit4, b'4' as u16),
        (C::Digit5, b'5' as u16),
        (C::Digit6, b'6' as u16),
        (C::Digit7, b'7' as u16),
        (C::Digit8, b'8' as u16),
        (C::Digit9, b'9' as u16),
        (C::Numpad0, b'0' as u16),
        (C::Numpad1, b'1' as u16),
        (C::Numpad2, b'2' as u16),
        (C::Numpad3, b'3' as u16),
        (C::Numpad4, b'4' as u16),
        (C::Numpad5, b'5' as u16),
        (C::Numpad6, b'6' as u16),
        (C::Numpad7, b'7' as u16),
        (C::Numpad8, b'8' as u16),
        (C::Numpad9, b'9' as u16),
    ];
    pressed
        .iter()
        .filter_map(|c| match c {
            C::Backspace => Some(8),
            C::Enter | C::NumpadEnter => Some(0x0D),
            C::Escape => Some(0x1B),
            c => DIGITS.iter().find(|(d, _)| d == c).map(|&(_, u)| u),
        })
        .map(UiEvent::Char)
        .collect()
}

#[cfg(test)]
mod key_tests {
    use super::*;
    use crate::controls::{Action, Preset};

    // Covers: specs/client/ui.md §a4-input-actions
    #[test]
    fn every_keyboard_key_has_one_code() {
        for &k in Key::ALL {
            let n = KEY_CODES.iter().filter(|(_, x)| *x == k).count();
            let mouse = k.name().starts_with("Mouse");
            assert_eq!(n, usize::from(!mouse), "{}", k.name());
        }
        for (i, (c, _)) in KEY_CODES.iter().enumerate() {
            assert!(KEY_CODES[..i].iter().all(|(d, _)| d != c));
        }
    }

    #[test]
    fn typed_keys_become_chars() {
        let e = key_chars(&[KeyCode::Digit4, KeyCode::KeyX, KeyCode::Backspace, KeyCode::Enter]);
        assert_eq!(
            e,
            vec![UiEvent::Char(b'4' as u16), UiEvent::Char(8), UiEvent::Char(0x0D)]
        );
    }

    // Covers: specs/client/ui.md §a4-input-actions, §a6-controls-file-d2controls-1-m20 r2
    #[test]
    fn bound_keys_become_world_actions_in_order() {
        let b = Preset::Dev.bindings().unwrap();
        let e = key_actions(&b, &[KeyCode::KeyI, KeyCode::KeyX, KeyCode::KeyT]);
        assert_eq!(
            e,
            vec![
                UiEvent::Action(ActionId(Action::ToggleInventory.index() as u16)),
                UiEvent::Action(ActionId(Action::ToggleSkillTree.index() as u16)),
            ]
        );
    }
}
