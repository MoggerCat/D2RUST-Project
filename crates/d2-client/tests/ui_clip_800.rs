// Spec: specs/ui/panels.md (§1 r1 screen), specs/client/ui.md (§a5-logical-resolution)
//! The default play frame 800 × 600: the same modules clip to 800 × 600.
//! Own binary (see `ui_clip_640.rs`).

use d2_client::ui::layout::Screen;

mod ui_clip_support;

#[test]
fn clips_follow_the_800_screen() {
    ui_clip_support::clips_are_the_screen(Screen::R800);
}
