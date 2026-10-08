// Spec: specs/ui/control-panel.md (§4 r1, §6 r1/r4, §8 r1)
//! The control panel tool tips (`Tip` of `ui::panels::control`) bound to
//! the string tables: the run, menu, new-stats / new-skills and
//! experience hovers resolve their string ids through [`StringLookup`]
//! and leave as centred text draws.
// d2rs-own, unverified: the tip font (1, the font of the globe numbers)
// is not named by the spec; the globe numbers (§3 r6) need text widths
// and are not drawn.

use crate::ui::draw::{TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::Point;
use crate::ui::panel::StringLookup;
use crate::ui::panels::control::buttons::{menu_tip, run_tip, tip_800, BtnEnv, NewBtn};
use crate::ui::panels::control::globes::{exp_tip, ExpIn, Tip};
use crate::ui::text::TextOpts;
use crate::ui::FRAME;

/// Tip font (d2rs-own, unverified).
const TIP_FONT: u16 = 1;

/// What the tips read.
pub struct TipIn<'a> {
    pub w: i32,
    pub h: i32,
    pub mouse: (i32, i32),
    pub mini_open: bool,
    /// State 9 (the new-stats / skills tip is hidden while it is open).
    pub state9_open: bool,
    pub exp: ExpIn,
    pub strings: &'a dyn StringLookup,
}

/// The tips under the mouse, in the order run, menu, new stats, new
/// skills, experience.
pub fn hud_tips(i: &TipIn<'_>) -> Vec<Tip> {
    let s = |id: u16| {
        i.strings
            .get_id(id)
            .map(<[u16]>::to_vec)
            .unwrap_or_default()
    };
    let env = BtnEnv {
        w: i.w,
        h: i.h,
        res2: true,
        open_mode: 0,
    };
    [
        run_tip(i.w, i.h, i.mouse, [None, None], &s),
        menu_tip(i.w, i.h, i.mini_open, i.mouse, &s),
        tip_800(&env, NewBtn::Stats, i.mouse, i.state9_open, &s),
        tip_800(&env, NewBtn::Skills, i.mouse, i.state9_open, &s),
        exp_tip(&i.exp, i.w, i.h, i.mouse, &s),
    ]
    .into_iter()
    .flatten()
    .filter(|t| !t.text.is_empty())
    .collect()
}

/// Pushes the tips as text draws.
pub fn draw_tips(i: &TipIn<'_>, out: &mut dyn UiDrawSink) {
    for t in hud_tips(i) {
        out.push(UiDraw::Text(TextRequest {
            text: t.text,
            at: Point::new(t.x, t.y),
            style: TextStyle {
                font: TIP_FONT,
                color: u16::from(t.color),
            },
            opts: if t.centered {
                TextOpts::centered()
            } else {
                TextOpts::default()
            },
            clip: FRAME,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Strs(HashMap<u16, Vec<u16>>);
    impl StringLookup for Strs {
        fn get(&self, _: &str) -> Option<&[u16]> {
            None
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            self.0.get(&id).map(Vec::as_slice)
        }
    }

    fn strs() -> Strs {
        Strs(
            [
                (4179, "Run"),
                (4167, "Open Mini Panel"),
                (3986, "New Stats"),
            ]
            .into_iter()
            .map(|(k, v)| (k, v.encode_utf16().collect()))
            .collect(),
        )
    }

    fn tips(mouse: (i32, i32), strings: &Strs) -> Vec<String> {
        hud_tips(&TipIn {
            w: 800,
            h: 600,
            mouse,
            mini_open: false,
            state9_open: false,
            exp: ExpIn::default(),
            strings,
        })
        .iter()
        .map(|t| String::from_utf16_lossy(&t.text))
        .collect()
    }

    #[test]
    fn tips_resolve_their_string_ids() {
        let s = strs();
        // Run rectangle, menu rectangle, new-stats button.
        assert_eq!(tips((255, 580), &s), ["Run"]);
        assert_eq!(tips((400, 570), &s), ["Open Mini Panel"]);
        assert_eq!(tips((220, 570), &s), ["New Stats"]);
        assert!(tips((10, 10), &s).is_empty());
        let mut out: Vec<UiDraw> = Vec::new();
        draw_tips(
            &TipIn {
                w: 800,
                h: 600,
                mouse: (400, 570),
                mini_open: false,
                state9_open: false,
                exp: ExpIn::default(),
                strings: &s,
            },
            &mut out,
        );
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn no_strings_draws_nothing() {
        assert!(tips((400, 570), &Strs(HashMap::new())).is_empty());
    }
}
