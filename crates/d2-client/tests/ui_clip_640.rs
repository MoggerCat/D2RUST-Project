// Spec: specs/ui/panels.md (§1 r1 screen), specs/client/ui.md (§a5-logical-resolution)
//! The play frame 640 × 480: the widget, imbue and controls modules clip to
//! the 640 × 480 screen, not to 800 × 600. Own binary: the play frame is a
//! once-per-process setting.

use d2_client::rules::camera::FrameSize;
use d2_client::ui::layout::Screen;

mod ui_clip_support;

#[test]
fn clips_follow_the_640_screen() {
    FrameSize::set_play(FrameSize::LOW).unwrap();
    ui_clip_support::clips_are_the_screen(Screen::R640);
}
