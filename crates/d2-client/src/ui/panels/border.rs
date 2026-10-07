// Spec: specs/ui/panels.md
//! §6: the 800 × 600 stone border (`0x00498630` / `0x00498700`, chooser
//! `0x00499450`) and the control panel base art (`0x004983D0`).
//!
//! The rows are `panel-layout.tsv` `border` and `ctrlpnl`. The control
//! panel rows use y = H, the GDI reference (§6.2; under DirectDraw the
//! outer two pieces sit at H − 1, not reproduced: the reference driver is
//! GDI). The overlays drawn after the base (globes, belt, skill buttons,
//! run / menu buttons, level name timer; §6.3) are §Open questions 1 and
//! not drawn here.

use super::{emit_static_draws, no_extra, PanelEnv, PanelTables};
use crate::ui::draw::UiDrawSink;
use crate::ui::layout::PanelKey;

/// Draws the border (resolution mode 2 only: left frames 0–4 when the open
/// mode is 2 or 3, right frames 5–9 when it is 1 or 3) and then the
/// control panel base (`ctrlpnl7` frames 0–4 at resolution mode ≠ 2,
/// `800ctrlpnl7` frames 0–5 at mode 2), in that order (§6.1, §6.2).
pub fn draw_border_and_ctrlpnl(t: &PanelTables, env: &PanelEnv, out: &mut dyn UiDrawSink) {
    let cond = env.cond(false, &no_extra);
    emit_static_draws(t, PanelKey::Border, &cond, None, &|_| true, out);
    emit_static_draws(t, PanelKey::CtrlPnl, &cond, None, &|_| true, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::{ImageRef, UiDraw};
    use crate::ui::layout::Screen;

    fn draws(env: PanelEnv) -> (PanelTables, Vec<(String, u32, i32, i32)>) {
        let t = PanelTables::load().unwrap();
        let mut out: Vec<UiDraw> = Vec::new();
        draw_border_and_ctrlpnl(&t, &env, &mut out);
        let v = out
            .into_iter()
            .map(|d| match d {
                UiDraw::Image(i) => {
                    let ImageRef { file, frame } = i.image;
                    (
                        t.files.name(file).unwrap().to_string(),
                        frame,
                        i.at.x,
                        i.at.y,
                    )
                }
                UiDraw::Text(_) => panic!("text"),
            })
            .collect();
        (t, v)
    }

    // Covers: specs/ui/panels.md §6 r1
    #[test]
    fn border_800_mode3_all_frames_then_ctrlpnl() {
        let (_, d) = draws(PanelEnv {
            screen: Screen::R800,
            open_mode: 3,
            exp: true,
        });
        assert_eq!(d.len(), 16);
        let border: Vec<_> = d[..10]
            .iter()
            .map(|(f, fr, x, y)| (f.as_str(), *fr, *x, *y))
            .collect();
        assert_eq!(
            border,
            vec![
                ("panel\\800borderframe", 0, 0, 253),
                ("panel\\800borderframe", 1, 256, 63),
                ("panel\\800borderframe", 2, 0, 484),
                ("panel\\800borderframe", 3, 0, 553),
                ("panel\\800borderframe", 4, 256, 553),
                ("panel\\800borderframe", 5, 400, 63),
                ("panel\\800borderframe", 6, 544, 253),
                ("panel\\800borderframe", 7, 713, 484),
                ("panel\\800borderframe", 8, 544, 553),
                ("panel\\800borderframe", 9, 400, 553),
            ]
        );
        let ctrl: Vec<_> = d[10..]
            .iter()
            .map(|(f, fr, x, y)| (f.as_str(), *fr, *x, *y))
            .collect();
        assert_eq!(
            ctrl,
            vec![
                ("panel\\800ctrlpnl7", 0, 0, 600),
                ("panel\\800ctrlpnl7", 1, 165, 600),
                ("panel\\800ctrlpnl7", 2, 293, 600),
                ("panel\\800ctrlpnl7", 3, 421, 600),
                ("panel\\800ctrlpnl7", 4, 549, 600),
                ("panel\\800ctrlpnl7", 5, 683, 600),
            ]
        );
    }

    // Covers: specs/ui/panels.md §6 r1
    #[test]
    fn border_800_one_side_per_open_mode() {
        for (mode, frames) in [
            (0u8, vec![]),
            (1, vec![5, 6, 7, 8, 9]),
            (2, vec![0, 1, 2, 3, 4]),
        ] {
            let (_, d) = draws(PanelEnv {
                screen: Screen::R800,
                open_mode: mode,
                exp: true,
            });
            let got: Vec<u32> = d
                .iter()
                .filter(|(f, ..)| f == "panel\\800borderframe")
                .map(|(_, fr, ..)| *fr)
                .collect();
            assert_eq!(got, frames, "mode {mode}");
            assert_eq!(d.len(), frames.len() + 6);
        }
    }

    // Covers: specs/ui/panels.md §6 r2
    #[test]
    fn ctrlpnl_640_five_frames_no_border() {
        let (_, d) = draws(PanelEnv {
            screen: Screen::R640,
            open_mode: 3,
            exp: false,
        });
        let got: Vec<_> = d
            .iter()
            .map(|(f, fr, x, y)| (f.as_str(), *fr, *x, *y))
            .collect();
        // No border at 640 × 480 whatever the open mode.
        assert_eq!(
            got,
            vec![
                ("panel\\ctrlpnl7", 0, 0, 480),
                ("panel\\ctrlpnl7", 1, 165, 480),
                ("panel\\ctrlpnl7", 2, 293, 480),
                ("panel\\ctrlpnl7", 3, 421, 480),
                ("panel\\ctrlpnl7", 4, 523, 480),
            ]
        );
    }
}
