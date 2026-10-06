// Spec: specs/client/ui.md
//! The Bevy input edge of the UI (spec §A4): the only part of `ui` that
//! touches Bevy. It reads the cursor from a `Window` in physical pixels
//! and maps it into the frame with [`Presentation`]; everything after is
//! plain Rust. Key and button bindings to actions are the controls'
//! (task C9), which implements [`super::UiInput`] on top of this.

use bevy::math::Vec2;
use bevy::window::Window;

use super::frame::{FrameError, FramePos, Presentation};

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
