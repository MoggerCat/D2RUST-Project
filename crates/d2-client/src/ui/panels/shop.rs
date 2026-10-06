// Spec: specs/ui/panels.md
//! §14.4–§14.5: the NPC shop panel (ui 0x0C, `0x00488400`).
//!
//! Draw order (as §14.4 lists the parts): the `buysell` art quads, [the
//! NPC store grid, `0x00483FF0`: `ui/inventory.md`, not here], the action
//! buttons, the tabs (each tab: cel, then caption, as in
//! `panel-layout.tsv`).
//!
//! Not implemented (spec incomplete; listed in the report):
//! - Open: §14.4 the action buttons' DC6 file is not named, so they are
//!   exposed as (frame, x, y) through [`ShopPanel::button_cels`] and not
//!   drawn. The 4 × 4 X table is indexed by `mode` = button count − 1 here
//!   (row `m` holds `m + 1` entries); the spec writes `X[mode][i]` without
//!   saying how `mode` derives from `[0x00722160]`.
//! - Open: §14.4 hit rectangles of the tabs and action buttons, and what a
//!   click on them does: not stated.
//! - Open: §14.5 buy 0x32 / sell 0x33 / repair 0x35 builders: the values
//!   the client writes in the fields the server does not read (client
//!   price u32 @13, 0x35 u16 @9) and which click picks which transaction
//!   `t` are not stated (`world/vendors.md` §7.1, §7.2, §8.1 give only the
//!   server side). Gamble identify 0x37 and hire 0x36: field layout and
//!   sender inputs not stated here. Not built.
//! - Open: §14.5 the close of the shop sends 0x30 through `0x004B3C20`;
//!   bytes 1–4 of that message (not read by the server, `world/npc.md` §3)
//!   are written 0 here.
//! - Open: the "`0x004B3500()` ≠ 0" flag that hides the captions is an
//!   input ([`ShopPanel::captions_hidden`]); its meaning is not stated.

use d2_proto::client::TerminateEntityChat;

use super::{emit_static_draws, no_extra, text, PanelEnv, PanelOutput, PanelTables, TextMeasure};
use crate::ui::draw::UiDrawSink;
use crate::ui::layout::{PanelKey, Screen};
use crate::ui::panel::{ClientIntent, StringLookup};

/// The shop's ui id (§4.1).
pub const UI_SHOP: u8 = 0x0C;
/// Font16 (`text-fonts.tsv` id 1), the tab caption font (§14.4).
pub const FONT16: u16 = 1;
/// Action button X table `0x00722168` (§14.4): row `m` has `m + 1`
/// entries.
pub const BUTTON_X: [[i32; 4]; 4] = [
    [273, 0, 0, 0],
    [169, 273, 0, 0],
    [169, 221, 273, 0],
    [116, 169, 221, 273],
];

/// One tab record of `0x00722110` (stride 18; §14.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopTab {
    /// i32 @0.
    pub x: i32,
    /// i32 @4.
    pub y: i32,
    /// String id u16 @8.
    pub string: u16,
    /// u32 @0x0A.
    pub active: bool,
    /// u32 @0x0E.
    pub visible: bool,
}

/// The default tab records (§14.4): Armor 4036, Weapons 4037, Weapons
/// 4037, Misc 4039 at x 42, 121, 201, 281, y 20. Active / visible are set
/// at run time; the defaults here are both false.
pub const DEFAULT_TABS: [ShopTab; 4] = [
    ShopTab {
        x: 42,
        y: 20,
        string: 4036,
        active: false,
        visible: false,
    },
    ShopTab {
        x: 121,
        y: 20,
        string: 4037,
        active: false,
        visible: false,
    },
    ShopTab {
        x: 201,
        y: 20,
        string: 4037,
        active: false,
        visible: false,
    },
    ShopTab {
        x: 281,
        y: 20,
        string: 4039,
        active: false,
        visible: false,
    },
];

/// One action button record of `0x007BC9E4` (stride 20): state u16 @0,
/// base frame u16 @0x0A.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopButton {
    pub state: u16,
    pub base: u16,
}

/// The shop panel's inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopPanel {
    pub tabs: [ShopTab; 4],
    /// The shown action buttons (`[0x00722160]`, 0–4).
    pub buttons: Vec<ShopButton>,
    /// `0x004B3500()` ≠ 0: tab captions are not drawn.
    pub captions_hidden: bool,
    /// GUID of the NPC traded with.
    pub npc_guid: u32,
}

impl ShopPanel {
    /// Draws the art quads, then the tabs (§14.4). The store grid and the
    /// action buttons are not drawn here (module Open).
    pub fn draw(
        &self,
        t: &PanelTables,
        env: &PanelEnv,
        strings: &dyn StringLookup,
        measure: &dyn TextMeasure,
        out: &mut dyn UiDrawSink,
    ) {
        let cond = env.cond(false, &no_extra);
        emit_static_draws(
            t,
            PanelKey::Ui(UI_SHOP),
            &cond,
            None,
            &|r| r.item.starts_with("art"),
            out,
        );
        let Some(tabs_file) = t.files.id("panel\\buyselltabs") else {
            return;
        };
        let s = &env.screen;
        for (i, tab) in (0i32..).zip(self.tabs.iter()) {
            if !tab.visible {
                continue;
            }
            let frame = if tab.active { i } else { i + 4 };
            out.push(super::cel(
                tabs_file,
                frame as u32,
                s.sx() + 80 * i,
                s.h + s.sy() - 449,
            ));
            if self.captions_hidden {
                continue;
            }
            let Some(str16) = strings.get_id(tab.string) else {
                continue;
            };
            let Some(width) = measure.width(FONT16, str16) else {
                continue;
            };
            let color = if tab.active { 4 } else { 0 };
            out.push(text(
                str16.to_vec(),
                s.sx() + tab.x - width / 2,
                s.h + s.sy() - 480 + tab.y,
                FONT16,
                color,
            ));
        }
    }

    /// The action buttons as (frame, x, y) draw positions: frame = base +
    /// state, x = `sx − 1 + X[n − 1][i]`, y = `H + sy − 63` (§14.4).
    /// Empty for 0 or more than 4 buttons.
    pub fn button_cels(&self, s: &Screen) -> Vec<(u32, i32, i32)> {
        let n = self.buttons.len();
        if n == 0 || n > 4 {
            return Vec::new();
        }
        self.buttons
            .iter()
            .zip(BUTTON_X[n - 1])
            .map(|(b, x)| {
                (
                    u32::from(b.base) + u32::from(b.state),
                    s.sx() - 1 + x,
                    s.h + s.sy() - 63,
                )
            })
            .collect()
    }

    /// Closing the shop ends the interaction: C→S 0x30 with the NPC GUID
    /// (§14.5, `0x004B3C20`).
    pub fn close_intent(&self) -> Vec<PanelOutput> {
        vec![PanelOutput::Intent(ClientIntent::from_message(
            &TerminateEntityChat { id: self.npc_guid },
        ))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::{ImageRef, UiDraw};

    struct Strs;
    impl StringLookup for Strs {
        fn get(&self, _: &str) -> Option<&[u16]> {
            None
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            const A: &[u16] = &[65, 114];
            const W: &[u16] = &[87, 101, 97];
            const M: &[u16] = &[77];
            match id {
                4036 => Some(A),
                4037 => Some(W),
                4039 => Some(M),
                _ => None,
            }
        }
    }
    /// Width = 10 × length (the sink measures for real).
    struct Meas;
    impl TextMeasure for Meas {
        fn width(&self, font: u16, t: &[u16]) -> Option<i32> {
            assert_eq!(font, FONT16);
            Some(10 * t.len() as i32 + 1)
        }
    }

    fn panel() -> ShopPanel {
        let mut tabs = DEFAULT_TABS;
        for t in &mut tabs {
            t.visible = true;
        }
        tabs[1].active = true;
        tabs[3].visible = false;
        ShopPanel {
            tabs,
            buttons: vec![],
            captions_hidden: false,
            npc_guid: 7,
        }
    }

    enum D {
        I(String, u32, i32, i32),
        T(Vec<u16>, i32, i32, u16, u16),
    }

    fn run(p: &ShopPanel, screen: Screen) -> Vec<D> {
        let t = PanelTables::load().unwrap();
        let env = PanelEnv {
            screen,
            open_mode: 3,
            exp: true,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        p.draw(&t, &env, &Strs, &Meas, &mut out);
        out.into_iter()
            .map(|d| match d {
                UiDraw::Image(i) => {
                    let ImageRef { file, frame } = i.image;
                    D::I(t.files.name(file).unwrap().into(), frame, i.at.x, i.at.y)
                }
                UiDraw::Text(x) => D::T(x.text, x.at.x, x.at.y, x.style.font, x.style.color),
            })
            .collect()
    }

    // Partial: §14 r4 (art quads and tabs; grid and buttons excluded).
    #[test]
    fn shop_art_and_tabs_800() {
        let d = run(&panel(), Screen::R800);
        let mut it = d.iter();
        for (f, x, y) in [(0, 80, 316), (1, 336, 316), (2, 80, 492), (3, 336, 492)] {
            match it.next().unwrap() {
                D::I(n, fr, xx, yy) => {
                    assert_eq!((n.as_str(), *fr, *xx, *yy), ("panel\\buysell", f, x, y))
                }
                D::T(..) => panic!(),
            }
        }
        let rest: Vec<_> = it.collect();
        assert_eq!(rest.len(), 6);
        // Tab 0 inactive: frame 4 at (80, 91); caption "Ar" width 21 at
        // x = 80 + 42 − 10, y = 600 − 60 − 480 + 20, color 0.
        match (rest[0], rest[1]) {
            (D::I(n, 4, 80, 91), D::T(s, 112, 80, 1, 0)) => {
                assert_eq!(n, "panel\\buyselltabs");
                assert_eq!(s, &vec![65, 114]);
            }
            _ => panic!(),
        }
        // Tab 1 active: frame 1 at (160, 91); "Wea" width 31, x 80 + 121 − 15.
        assert!(matches!(
            (rest[2], rest[3]),
            (D::I(_, 1, 160, 91), D::T(_, 186, 80, 1, 4))
        ));
        assert!(matches!(
            (rest[4], rest[5]),
            (D::I(_, 6, 240, 91), D::T(_, 266, 80, 1, 0))
        ));
    }

    // Partial: §14 r4 (caption suppression flag).
    #[test]
    fn shop_captions_hidden() {
        let mut p = panel();
        p.captions_hidden = true;
        let d = run(&p, Screen::R640);
        assert_eq!(d.len(), 4 + 3);
        assert!(d.iter().all(|x| matches!(x, D::I(..))));
        assert!(matches!(d[4], D::I(_, 4, 0, 31)));
    }

    // Partial: §14 r4 (button positions and frames; file unnamed).
    #[test]
    fn shop_button_positions() {
        let b = |base, state| ShopButton { state, base };
        let mut p = panel();
        p.buttons = vec![b(0, 1), b(2, 0), b(4, 1), b(6, 0)];
        assert_eq!(
            p.button_cels(&Screen::R640),
            vec![(1, 115, 417), (2, 168, 417), (5, 220, 417), (6, 272, 417)]
        );
        p.buttons = vec![b(8, 0)];
        assert_eq!(p.button_cels(&Screen::R800), vec![(8, 352, 477)]);
        p.buttons.clear();
        assert!(p.button_cels(&Screen::R800).is_empty());
    }

    // Partial: §14 r5 (close of the shop only).
    #[test]
    fn shop_close_sends_0x30() {
        let o = panel().close_intent();
        assert_eq!(
            o,
            vec![PanelOutput::Intent(ClientIntent(vec![
                0x30, 0, 0, 0, 0, 7, 0, 0, 0
            ]))]
        );
    }
}
