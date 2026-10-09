// Spec: specs/ui/control-panel.md (§4 r1, §6 r1/r4, §8 r1)
//! The control panel tool tips (`Tip` of `ui::panels::control`) bound to
//! the string tables: the run, menu, new-stats / new-skills and
//! experience hovers resolve their string ids through [`StringLookup`]
//! and leave as centred text draws.
// d2rs-own, unverified: the tip font (1, the font of the globe numbers)
// is not named by the spec. Without the font measure the globe numbers
// are centred on width 0.

use crate::ui::draw::{RectRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use crate::ui::geom::Point;
use crate::ui::panel::StringLookup;
use crate::ui::panels::control::buttons::{menu_tip, run_tip, tip_800, BtnEnv, NewBtn};
use crate::ui::panels::control::globes::{
    exp_tip, globe_numbers, stamina_tip, ExpIn, NumbersIn, StaminaIn, Tip,
};
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
    /// The tip font's measure (the pop-up placement, `control-panel.md`
    /// §5 r14); `None`: the tips are drawn at their call point.
    pub fonts: Option<&'a super::FontMeasure>,
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

/// What the globe numbers and the stamina tip read (§3 r6, §4 r2).
pub struct GlobeTextIn<'a> {
    pub w: i32,
    pub h: i32,
    pub numbers: NumbersIn,
    pub stamina: StaminaIn,
    pub strings: &'a dyn StringLookup,
    /// Width A of a string in the tip font; none without the font.
    pub width_a: &'a dyn Fn(&[u16]) -> i32,
    /// As [`TipIn::fonts`].
    pub fonts: Option<&'a super::FontMeasure>,
}

/// A tip (`0x00502280(text, x, y, k, centre)`) as its pop-up draw
/// `0x00503000` ([`push_popup`]).
fn push_tip(t: Tip, w: i32, h: i32, fonts: Option<&super::FontMeasure>, out: &mut dyn UiDrawSink) {
    push_popup(
        t.text,
        Point::new(t.x, t.y),
        u16::from(t.color),
        t.centered,
        (w, h),
        fonts,
        out,
    );
}

/// The colour and mode of the pop-up's backing box (§5 r14 step 5:
/// `DrawRectangle(…, colour 0, mode 2)`, blend kind 2 of
/// `render/blend-modes.md` §8 r2).
pub const POPUP_BOX_COLOR: u8 = 0;
pub const POPUP_BOX_MODE: u8 = 2;

/// The pop-up draw `0x00503000` of a call `0x00502280(text, at, color,
/// centre)` in the tip font (`control-panel.md` §5 r14): the backing box
/// (x', b − Ht)–(x' + W, b) in colour 0, mode 2, then the text centred in
/// a block of max width + 8 whose centre is x (centre 1), bottom y + 2,
/// at the bottom − 3 for Font16. Without the font's measure: the text at
/// the call point, no box. The too-tall font swap (step 3) and the one
/// slot per frame are not applied.
pub(super) fn push_popup(
    text: Vec<u16>,
    at: Point,
    color: u16,
    centered: bool,
    (w, h): (i32, i32),
    fonts: Option<&super::FontMeasure>,
    out: &mut dyn UiDrawSink,
) {
    let style = TextStyle {
        font: TIP_FONT,
        color,
    };
    if let Some(fr) = fonts.and_then(|f| f.popup(TIP_FONT, &text, at, centered, (w, h))) {
        let (p0, p1) = fr.rect;
        out.push(UiDraw::Rect(RectRequest {
            x0: p0.x,
            y0: p0.y,
            x1: p1.x,
            y1: p1.y,
            color: POPUP_BOX_COLOR,
            mode: POPUP_BOX_MODE,
        }));
        out.push(UiDraw::Text(TextRequest {
            text,
            at: fr.pen,
            style,
            opts: fr.opts,
            clip: FRAME,
        }));
        return;
    }
    out.push(UiDraw::Text(TextRequest {
        text,
        at,
        style,
        opts: if centered {
            TextOpts::centered()
        } else {
            TextOpts::default()
        },
        clip: FRAME,
    }));
}

/// Pushes the life / mana numbers (§3 r6) and the stamina tip (§4 r2).
pub fn draw_globe_text(i: &GlobeTextIn<'_>, out: &mut dyn UiDrawSink) {
    let s = |id: u16| {
        i.strings
            .get_id(id)
            .map(<[u16]>::to_vec)
            .unwrap_or_default()
    };
    let style = |color: u16| TextStyle {
        font: TIP_FONT,
        color,
    };
    for l in globe_numbers(&i.numbers, i.w, i.h, &s, i.width_a) {
        if l.text.is_empty() {
            continue;
        }
        out.push(UiDraw::Text(TextRequest {
            text: l.text,
            at: Point::new(l.x, l.y),
            style: style(l.color as u16),
            opts: TextOpts::default(),
            clip: FRAME,
        }));
    }
    let st = &i.stamina;
    if let Some(t) = stamina_tip(st.shown, st.max, st.shrine, i.w, i.h, i.numbers.mouse, &s) {
        if !t.text.is_empty() {
            push_tip(t, i.w, i.h, i.fonts, out);
        }
    }
}

/// Pushes the tips as text draws.
pub fn draw_tips(i: &TipIn<'_>, out: &mut dyn UiDrawSink) {
    for t in hud_tips(i) {
        push_tip(t, i.w, i.h, i.fonts, out);
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
            fonts: None,
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
                fonts: None,
            },
            &mut out,
        );
        assert_eq!(out.len(), 1);
    }

    fn globe_strs() -> Strs {
        Strs(
            [
                (4165, "Life: %d / %d"),
                (4166, "Mana: %d / %d"),
                (4164, "Stamina: %d / %d"),
            ]
            .into_iter()
            .map(|(k, v)| (k, v.encode_utf16().collect()))
            .collect(),
        )
    }

    fn globe_text(mouse: (i32, i32), show_hp: bool) -> Vec<(String, i32, i32, u16)> {
        globe_text_in(mouse, show_hp, None)
    }

    fn globe_text_in(
        mouse: (i32, i32),
        show_hp: bool,
        fonts: Option<&crate::ui::original::FontMeasure>,
    ) -> Vec<(String, i32, i32, u16)> {
        let s = globe_strs();
        let mut out: Vec<UiDraw> = Vec::new();
        draw_globe_text(
            &GlobeTextIn {
                w: 800,
                h: 600,
                numbers: NumbersIn {
                    show_hp,
                    mouse,
                    life_shown: 50 << 8,
                    life_max: 100 << 8,
                    mana_shown: 30 << 8,
                    mana_max: 40 << 8,
                    living_player: true,
                    ..NumbersIn::default()
                },
                stamina: StaminaIn {
                    shown: 20 << 8,
                    max: 25 << 8,
                    shrine: false,
                },
                strings: &s,
                width_a: &|t| 6 * t.len() as i32,
                fonts,
            },
            &mut out,
        );
        out.into_iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some((
                    String::from_utf16_lossy(&t.text),
                    t.at.x,
                    t.at.y,
                    t.style.color,
                )),
                // The pop-up's backing box (checked by its own test).
                UiDraw::Rect(_) => None,
                _ => panic!("text and pop-up boxes only"),
            })
            .collect()
    }

    #[test]
    fn hovering_the_life_globe_draws_the_numbers() {
        // "Life: 50 / 100" is 14 units, width 84: x = 65 - 42.
        assert_eq!(
            globe_text((60, 550), false),
            [("Life: 50 / 100".to_string(), 23, 505, 0)]
        );
        // Mana: "Mana: 30 / 40" is 13 units, width 78: x = 720 - 39.
        assert_eq!(
            globe_text((700, 550), false),
            [("Mana: 30 / 40".to_string(), 681, 505, 0)]
        );
        // The toggle shows life without hover; elsewhere nothing.
        assert_eq!(globe_text((400, 300), true).len(), 1);
        assert!(globe_text((400, 300), false).is_empty());
    }

    #[test]
    fn hovering_stamina_draws_its_tip() {
        // Stamina rectangle x 273…375, y 573…591; centred at (324, 548).
        assert_eq!(
            globe_text((300, 580), false),
            [("Stamina: 20 / 25".to_string(), 324, 548, 0)]
        );
    }

    // Covers: specs/ui/control-panel.md §4 r2, §5 r14
    #[test]
    fn the_stamina_tip_is_a_popup_centred_on_its_point() {
        use d2_formats::font::{FontTable, Glyph};
        // Font16 stand-in: every advance 6, height byte 10.
        let glyphs = (0..256u16)
            .map(|i| Glyph {
                code: i,
                unknown1: 0,
                width: 6,
                height: 10,
                unknown2: 1,
                unknown3: 0,
                frame: i,
                unknown5: 0,
            })
            .collect();
        let mut m = crate::ui::original::FontMeasure::default();
        m.insert(
            TIP_FONT,
            FontTable {
                version: 1,
                unknown: 0,
                count: 256,
                height: 10,
                width: 0,
                glyphs,
            },
        );
        // 16 units, max width 96, W = 104: the block starts at 324 − 52;
        // bottom 548 + 2, text row 550 − 3.
        assert_eq!(
            globe_text_in((300, 580), false, Some(&m)),
            [("Stamina: 20 / 25".to_string(), 272, 547, 0)]
        );
    }

    // Covers: specs/ui/control-panel.md §5 r14
    #[test]
    fn a_popup_draws_its_backing_box_before_its_text() {
        use d2_formats::font::{FontTable, Glyph};
        // The spec vector: `Run` at (255, 577), centre 1, 800 × 600, max
        // width 26, Ht 16 → W = 34, box (238, 563)–(272, 579) in colour
        // 0, mode 2; the text block at x 238, y 576.
        let widths = |c: u16| match c {
            0x52 => 10,
            0x75 | 0x6E => 8,
            _ => 6,
        };
        let glyphs = (0..256u16)
            .map(|i| Glyph {
                code: i,
                unknown1: 0,
                width: widths(i),
                height: 16,
                unknown2: 1,
                unknown3: 0,
                frame: i,
                unknown5: 0,
            })
            .collect();
        let mut m = crate::ui::original::FontMeasure::default();
        m.insert(
            TIP_FONT,
            FontTable {
                version: 1,
                unknown: 0,
                count: 256,
                height: 10,
                width: 0,
                glyphs,
            },
        );
        let mut out: Vec<UiDraw> = Vec::new();
        push_popup(
            "Run".encode_utf16().collect(),
            Point::new(255, 577),
            0,
            true,
            (800, 600),
            Some(&m),
            &mut out,
        );
        let [UiDraw::Rect(r), UiDraw::Text(t)] = &out[..] else {
            panic!("box then text: {out:?}");
        };
        assert_eq!(
            (r.x0, r.y0, r.x1, r.y1, r.color, r.mode),
            (238, 563, 272, 579, 0, 2)
        );
        assert_eq!((t.at.x, t.at.y), (238, 576));
    }

    #[test]
    fn no_strings_draws_nothing() {
        assert!(tips((400, 570), &Strs(HashMap::new())).is_empty());
    }
}
