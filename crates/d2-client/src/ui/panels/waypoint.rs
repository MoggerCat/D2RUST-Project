// Spec: specs/ui/panels.md
//! Waypoint menu (ui 0x14, left slot; §13, `0x0049C9C0`): background,
//! act tabs, close button, the rows of the current tab, the title and the
//! C→S 0x49 messages.
//!
//! Row and tab data (level names, which waypoints are known, which acts
//! are reachable) come from a [`WaypointView`]; the menu decides nothing:
//! travel and close leave as C→S 0x49 (`world/waypoints.md` §6).
//!
//! Open (spec gaps):
//! - Tab and row click rectangles and the tab switch (§13.7, spec OQ7):
//!   not implemented. Callers drive [`WaypointPanel::choose_row`],
//!   [`WaypointPanel::close`], [`WaypointPanel::tab`] and
//!   [`WaypointPanel::selected`] directly. Whether a row must be known to
//!   be chosen is not stated; any present row sends.
//! - The close button's hover effect (§13.4) is a filled rectangle and a
//!   tool tip; [`UiDraw`] has neither, so it is exposed as the query
//!   [`close_hover`] for the root to draw. The rectangle's extent is not
//!   stated; the query returns the hover area.
//! - The self-close (§13.1) mode is elided in the spec (`SetUIState(0x14,
//!   …)`); `off` is used. Its jump is 0: §4.3's scan of all call sites
//!   lists no jump = 1 caller in the draw function. When the latch
//!   `[0x007BF085]` is cleared is not stated; [`WaypointPanel::open`]
//!   clears it.
//! - Whether a close through `0x0049CEC0` ([`WaypointPanel::close`]) also
//!   sends the close hook's 0x49 (`0x0049CF50`, [`WaypointPanel::close_hook`])
//!   a second time, and the order of the 0x49 and the `SetUIState` call,
//!   are not stated.
//! - Row text color: "5 unknown, 0 known, 3 if hovered-selected or current
//!   level" (§13.5) is applied in that order (3 overrides); an unknown
//!   selected row is untested by the spec. The order of the current
//!   level's extra frame-0 draw against its row icon is not stated (drawn
//!   after).
//! - Which tab and row are current when the menu opens is not stated.

use d2_proto::client::TakeOrCloseWp;

use super::{cel, emit_static_draws, text, PanelEnv, PanelOutput, PanelTables, TextMeasure};
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::layout::{Cond, FrameSpec, LayoutRow, PanelKey, RowKind, Screen};
use crate::ui::panel::{ClientIntent, StringLookup};

/// UI state id of the waypoint menu (§4.1).
pub const UI_WAYPOINT: u8 = 0x14;
/// Font16 (`text-fonts.tsv` id 1): rows and title (§13.5, §13.6).
pub const FONT16: u16 = 1;
/// `waypointsheader` (§13.6).
pub const STR_HEADER: u16 = 3990;
/// `nowaypoints` (§13.6).
pub const STR_NO_WAYPOINTS: u16 = 3991;
/// `strUiMenu1` "Cancel", the close tool tip (§13.4).
pub const STR_CANCEL: u16 = 4130;
/// Rows per tab (§13.5).
pub const MAX_ROWS: usize = 9;
/// Waypoint icons file (§13.2).
pub const ICONS_FILE: &str = "menu\\waygateicons";

/// One waypoint row of the current tab (§13.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WpRow {
    /// The level id sent by 0x49.
    pub level: u16,
    /// The player knows this waypoint.
    pub known: bool,
    /// This is the level the player stands in.
    pub current: bool,
    /// Level name (`0x00453E70`), UTF-16.
    pub name: Vec<u16>,
}

/// The game facts the waypoint menu draws (§13).
pub trait WaypointView {
    /// The rows of tab `tab` (at most [`MAX_ROWS`] are used).
    fn rows(&self, tab: u8) -> &[WpRow];
    /// The act of tab 1–4 is reachable (quest checks `0x004B32D0` /
    /// `0x0065C310`, records 7, 15, 23, 26, §13.3).
    fn tab_reachable(&self, tab: u8) -> bool;
    /// Any other waypoint is known (`[0x007BF08E]`, §13.6).
    fn any_other_known(&self) -> bool;
}

/// The close button's hover result (§13.4): the root draws the filled
/// rectangle (color 0, mode 2) and queues the tool tip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseHover {
    /// Hover area: x in [`sx + 273`, `sx + 308`], y in [`387 − sy`,
    /// `420 − sy`].
    pub area: Rect,
    /// Tool-tip string id ("Cancel").
    pub tooltip: u16,
    /// Tool tip centered on this x.
    pub tooltip_cx: i32,
    /// Tool tip y.
    pub tooltip_y: i32,
}

/// The close hover at `p`, if the mouse is in the area (§13.4).
pub fn close_hover(s: &Screen, p: Point) -> Option<CloseHover> {
    let (sx, sy) = (s.sx(), s.sy());
    let area = Rect::new(sx + 273, 387 - sy, 36, 34);
    area.contains(p).then_some(CloseHover {
        area,
        tooltip: STR_CANCEL,
        tooltip_cx: sx + 291,
        tooltip_y: 385 - sy,
    })
}

/// Tab frame (§13.3): the current tab draws `2t`; another draws `2t + 1`
/// if it is tab 0 or its act is reachable; otherwise nothing.
pub fn tab_frame(current: u8, tab: u8, reachable: bool) -> Option<u32> {
    let t = u32::from(tab);
    if tab == current {
        Some(2 * t)
    } else if tab == 0 || reachable {
        Some(2 * t + 1)
    } else {
        None
    }
}

/// Waypoint menu state (§13).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaypointPanel {
    /// Current tab `[0x007BF086]`.
    pub tab: u8,
    /// Selected row `[0x007BF06D]`.
    pub selected: Option<u8>,
    /// Close button pressed `[0x007BF06C]`.
    pub close_pressed: bool,
    /// The waypoint object's GUID `[0x007BF07D]`.
    pub guid: u32,
    /// Self-close message sent `[0x007BF085]`.
    close_sent: bool,
}

impl WaypointPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// S→C 0x63 (§13.1): store the GUID and open with jump = 1.
    pub fn open(&mut self, guid: u32) -> Vec<PanelOutput> {
        self.guid = guid;
        self.close_sent = false;
        vec![PanelOutput::SetUi {
            ui: UI_WAYPOINT,
            mode: 0,
            jump: true,
        }]
    }

    fn msg(&self, level: u16) -> PanelOutput {
        PanelOutput::Intent(ClientIntent::from_message(&TakeOrCloseWp {
            wp: self.guid,
            level,
        }))
    }

    /// Choosing row `row` of the current tab: C→S 0x49 `[GUID][level]`
    /// (§13.7, `0x0049D0F3`). Nothing for a row that is not there.
    pub fn choose_row(&self, view: &dyn WaypointView, row: usize) -> Vec<PanelOutput> {
        let rows = view.rows(self.tab);
        match rows.get(row).filter(|_| row < MAX_ROWS) {
            Some(r) => vec![self.msg(r.level)],
            None => Vec::new(),
        }
    }

    /// The close paths `0x0049CEC0` / `0x0049D160` (§13.7, §4.3): 0x49
    /// with level 0, then `SetUIState(0x14, off, jump = 1)`.
    pub fn close(&mut self) -> Vec<PanelOutput> {
        self.close_pressed = false;
        vec![
            self.msg(0),
            PanelOutput::SetUi {
                ui: UI_WAYPOINT,
                mode: 1,
                jump: true,
            },
        ]
    }

    /// The close hook `0x0049CF50` (§2.6, §13.7): 0x49 with level 0.
    pub fn close_hook(&mut self) -> Vec<PanelOutput> {
        self.close_pressed = false;
        vec![self.msg(0)]
    }

    /// The draw-time check (§13.1): with the player gone (or in a state
    /// `0x00463DF0` rejects) the menu closes itself and sends 0x49 level 0
    /// once (latch).
    pub fn self_close(&mut self, player_ok: bool) -> Vec<PanelOutput> {
        if player_ok {
            return Vec::new();
        }
        let mut out = vec![PanelOutput::SetUi {
            ui: UI_WAYPOINT,
            mode: 1,
            jump: false,
        }];
        if !self.close_sent {
            self.close_sent = true;
            out.push(self.msg(0));
        }
        out
    }

    /// Draws the menu (§13.2–§13.6): background, tabs, close button, rows,
    /// title. Text rows whose string or width is missing are left out.
    pub fn draw(
        &self,
        t: &PanelTables,
        env: &PanelEnv,
        view: &dyn WaypointView,
        strings: &dyn StringLookup,
        measure: &dyn TextMeasure,
        out: &mut dyn UiDrawSink,
    ) {
        let s = env.screen;
        let key = PanelKey::Ui(UI_WAYPOINT);
        let cond = env.cond(self.close_pressed, &|c: Cond| matches!(c, Cond::TabShown));
        emit_static_draws(t, key, &cond, None, &|r| r.item.starts_with("art"), out);
        // Tabs (§13.3).
        for r in t.rows(key) {
            if r.kind != RowKind::Draw || !r.item.starts_with("tab") || !r.applies(&cond) {
                continue;
            }
            let Some(n) = r.item[3..].parse::<u8>().ok() else {
                continue;
            };
            let reachable = n != 0 && view.tab_reachable(n);
            if let (Some(f), Some(file)) =
                (tab_frame(self.tab, n, reachable), t.files.row_file(r, None))
            {
                out.push(cel(file, f, r.x.eval(&s), r.y.eval(&s)));
            }
        }
        // Close button (§13.4), frame 10 + pressed.
        emit_static_draws(t, key, &cond, None, &|r| r.item == "close", out);
        // Rows (§13.5).
        let icons = t.files.id(ICONS_FILE);
        for (i, row) in view.rows(self.tab).iter().take(MAX_ROWS).enumerate() {
            let name = format!("row{i}");
            let sel = self.selected == Some(i as u8);
            if row.known {
                if let (Some(file), Some(d)) = (icons, find(t, key, &name, RowKind::Draw)) {
                    let (x, y) = (d.x.eval(&s), d.y.eval(&s));
                    let f = if row.current {
                        u32::from(sel)
                    } else {
                        3 + u32::from(sel)
                    };
                    out.push(cel(file, f, x, y));
                    if row.current {
                        out.push(cel(file, 0, x, y));
                    }
                }
            }
            if let Some(tr) = find(t, key, &name, RowKind::Text) {
                let color = if sel || row.current {
                    3
                } else if row.known {
                    0
                } else {
                    5
                };
                let font = tr.font.unwrap_or(FONT16);
                out.push(text(
                    row.name.clone(),
                    tr.x.eval(&s),
                    tr.y.eval(&s),
                    font,
                    color,
                ));
            }
        }
        // Title (§13.6).
        if let Some(tr) = find(t, key, "title", RowKind::Text) {
            let id = match tr.frame {
                FrameSpec::Alt(a, b) => {
                    if view.any_other_known() {
                        a
                    } else {
                        b
                    }
                }
                _ => return,
            };
            let font = tr.font.unwrap_or(FONT16);
            let Some(str16) = u16::try_from(id).ok().and_then(|id| strings.get_id(id)) else {
                return;
            };
            let Some(w) = measure.width(font, str16) else {
                return;
            };
            let color = match tr.color {
                crate::ui::layout::ColorSpec::Index(k) => k,
                _ => 0,
            };
            out.push(text(
                str16.to_vec(),
                tr.x.eval(&s) - w / 2,
                tr.y.eval(&s),
                font,
                color,
            ));
        }
    }
}

fn find<'a>(
    t: &'a PanelTables,
    key: PanelKey,
    item: &'a str,
    kind: RowKind,
) -> Option<&'a LayoutRow> {
    t.item(key, item, kind).next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::{ImageRef, ImageRequest, TextRequest, UiDraw};
    use crate::ui::panels::utf16;

    struct View {
        rows: Vec<WpRow>,
        reach: [bool; 5],
        other: bool,
    }

    impl WaypointView for View {
        fn rows(&self, _tab: u8) -> &[WpRow] {
            &self.rows
        }
        fn tab_reachable(&self, tab: u8) -> bool {
            self.reach[usize::from(tab)]
        }
        fn any_other_known(&self) -> bool {
            self.other
        }
    }

    struct Strings(Vec<(u16, Vec<u16>)>);

    impl StringLookup for Strings {
        fn get(&self, _key: &str) -> Option<&[u16]> {
            None
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            self.0
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, s)| s.as_slice())
        }
    }

    /// Width 7 per code unit.
    struct Measure;

    impl TextMeasure for Measure {
        fn width(&self, _font: u16, text: &[u16]) -> Option<i32> {
            Some(7 * text.len() as i32)
        }
    }

    fn row(level: u16, known: bool, current: bool) -> WpRow {
        WpRow {
            level,
            known,
            current,
            name: utf16(&format!("L{level}")),
        }
    }

    fn env(screen: Screen, exp: bool) -> PanelEnv {
        PanelEnv {
            screen,
            open_mode: 2,
            exp,
        }
    }

    fn strings() -> Strings {
        Strings(vec![
            (3990, utf16("Waypoints")),
            (3991, utf16("No Waypoints")),
        ])
    }

    fn draw(p: &WaypointPanel, e: &PanelEnv, v: &View) -> (PanelTables, Vec<UiDraw>) {
        let t = PanelTables::load().unwrap();
        let mut out: Vec<UiDraw> = Vec::new();
        p.draw(&t, e, v, &strings(), &Measure, &mut out);
        (t, out)
    }

    fn images(d: &[UiDraw]) -> Vec<(u32, u32, i32, i32)> {
        d.iter()
            .filter_map(|d| match d {
                UiDraw::Image(ImageRequest {
                    image: ImageRef { file, frame },
                    at,
                    ..
                }) => Some((*file, *frame, at.x, at.y)),
                _ => None,
            })
            .collect()
    }

    fn texts(d: &[UiDraw]) -> Vec<&TextRequest> {
        d.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    fn view() -> View {
        View {
            rows: Vec::new(),
            reach: [true, true, false, false, false],
            other: true,
        }
    }

    // Background quads (left slot) and the tab frames per game type.
    // Covers: specs/ui/panels.md §13 r2
    // Covers: specs/ui/panels.md §13 r3
    #[test]
    fn background_and_tabs() {
        let p = WaypointPanel {
            tab: 1,
            ..Default::default()
        };
        // Expansion, 800 × 600.
        let (t, out) = draw(&p, &env(Screen::R800, true), &view());
        let im = images(&out);
        let bg = t.files.id("menu\\waygatebackground").unwrap();
        let tabs = t.files.id("menu\\expwaygatetabs").unwrap();
        assert_eq!(
            im[..4],
            [
                (bg, 0, 80, 316),
                (bg, 1, 336, 316),
                (bg, 2, 80, 492),
                (bg, 3, 336, 492)
            ]
        );
        // Tab 0 always (2t + 1 = 1), tab 1 current (2), tabs 2–4 unreachable.
        assert_eq!(im[4..6], [(tabs, 1, 85, 94), (tabs, 2, 147, 94)]);
        // Classic, 640 × 480, tab 0 current, tab 2 reachable.
        let mut v = view();
        v.reach = [false, false, true, false, false];
        let p0 = WaypointPanel::default();
        let (t, out) = draw(&p0, &env(Screen::R640, false), &v);
        let im = images(&out);
        let ctabs = t.files.id("menu\\waygatetabs").unwrap();
        assert_eq!(im[4..6], [(ctabs, 0, 3, 33), (ctabs, 5, 159, 33)]);
        assert!(im.iter().all(|i| i.0 != tabs));
        // Expansion fifth tab at sx + 253.
        let mut v = view();
        v.reach = [true; 5];
        let (_, out) = draw(&p0, &env(Screen::R640, true), &v);
        let im = images(&out);
        assert_eq!(
            im[4..9],
            [
                (tabs, 0, 5, 34),
                (tabs, 3, 67, 34),
                (tabs, 5, 129, 34),
                (tabs, 7, 191, 34),
                (tabs, 9, 253, 34)
            ]
        );
        assert_eq!(tab_frame(2, 2, false), Some(4));
        assert_eq!(tab_frame(2, 3, false), None);
        assert_eq!(tab_frame(2, 0, false), Some(1));
    }

    // Close button frame 10 + pressed at (sx + 273, 417 − sy); hover area.
    #[test]
    fn close_button_and_hover() {
        let mut p = WaypointPanel::default();
        let (t, out) = draw(&p, &env(Screen::R800, true), &view());
        let close = t.files.id("panel\\buysellbtn").unwrap();
        assert!(images(&out).contains(&(close, 10, 353, 477)));
        p.close_pressed = true;
        let (_, out) = draw(&p, &env(Screen::R800, true), &view());
        assert!(images(&out).contains(&(close, 11, 353, 477)));
        assert!(!images(&out).contains(&(close, 10, 353, 477)));
        let s = Screen::R800;
        let h = close_hover(&s, Point::new(353, 447)).unwrap();
        assert_eq!((h.tooltip, h.tooltip_cx, h.tooltip_y), (4130, 371, 445));
        assert!(close_hover(&s, Point::new(388, 480)).is_some());
        assert!(close_hover(&s, Point::new(352, 460)).is_none());
        assert!(close_hover(&s, Point::new(389, 460)).is_none());
        assert!(close_hover(&s, Point::new(360, 446)).is_none());
        assert!(close_hover(&s, Point::new(360, 481)).is_none());
        let s = Screen::R640;
        assert!(close_hover(&s, Point::new(273, 387)).is_some());
        assert!(close_hover(&s, Point::new(308, 420)).is_some());
    }

    // Row icons (frame sel / 3 + sel, frame 0 again for the current
    // level), positions, text colors.
    // Covers: specs/ui/panels.md §13 r5
    #[test]
    fn rows_icons_and_text() {
        let mut v = view();
        v.rows = vec![
            row(1, true, false),
            row(3, true, true),
            row(4, false, false),
            row(5, true, false),
            row(6, true, false),
        ];
        let p = WaypointPanel {
            selected: Some(3),
            ..Default::default()
        };
        let (t, out) = draw(&p, &env(Screen::R640, true), &v);
        let icons = t.files.id(ICONS_FILE).unwrap();
        let im: Vec<_> = images(&out).into_iter().filter(|i| i.0 == icons).collect();
        assert_eq!(
            im,
            vec![
                (icons, 3, 17, 89),
                (icons, 0, 17, 125),
                (icons, 0, 17, 125),
                (icons, 4, 17, 197),
                (icons, 3, 17, 234),
            ]
        );
        let tx = texts(&out);
        let rows: Vec<_> = tx[..5]
            .iter()
            .map(|t| (t.at.x, t.at.y, t.style.font, t.style.color))
            .collect();
        assert_eq!(
            rows,
            vec![
                (80, 84, 1, 0),
                (80, 119, 1, 3),
                (80, 154, 1, 5),
                (80, 189, 1, 3),
                (80, 224, 1, 0)
            ]
        );
        assert_eq!(tx[0].text, utf16("L1"));
        // 800 × 600: + (80, 60).
        let (_, out) = draw(&p, &env(Screen::R800, true), &v);
        let im: Vec<_> = images(&out).into_iter().filter(|i| i.0 == icons).collect();
        assert_eq!(im[0], (icons, 3, 97, 149));
        assert_eq!((texts(&out)[4].at.x, texts(&out)[4].at.y), (160, 284));
        // At most nine rows.
        v.rows = (0..12).map(|l| row(l, true, false)).collect();
        let (_, out) = draw(&WaypointPanel::default(), &env(Screen::R640, true), &v);
        let im: Vec<_> = images(&out).into_iter().filter(|i| i.0 == icons).collect();
        assert_eq!(im.len(), 9);
        assert_eq!(im[8], (icons, 3, 17, 378));
        assert_eq!(texts(&out).len(), 10);
    }

    // Covers: specs/ui/panels.md §13 r6
    #[test]
    fn title_header_or_none() {
        let p = WaypointPanel::default();
        let (_, out) = draw(&p, &env(Screen::R800, true), &view());
        let tx = texts(&out);
        let title = tx.last().unwrap();
        assert_eq!(title.text, utf16("Waypoints"));
        // width 63: x = 80 + 160 − 31.
        assert_eq!(
            (title.at.x, title.at.y, title.style.font, title.style.color),
            (209, 108, 1, 0)
        );
        let mut v = view();
        v.other = false;
        let (_, out) = draw(&p, &env(Screen::R640, true), &v);
        let title = *texts(&out).last().unwrap();
        assert_eq!(title.text, utf16("No Waypoints"));
        // width 84: x = 160 − 42.
        assert_eq!((title.at.x, title.at.y), (118, 48));
    }

    // Covers: specs/ui/panels.md §13 r1, §13 r7
    #[test]
    fn messages_choose_and_close() {
        let mut p = WaypointPanel::new();
        assert_eq!(
            p.open(0x0B),
            vec![PanelOutput::SetUi {
                ui: 0x14,
                mode: 0,
                jump: true
            }]
        );
        let mut v = view();
        v.rows = vec![row(1, true, false), row(0x1D, true, false)];
        assert_eq!(
            p.choose_row(&v, 1),
            vec![PanelOutput::Intent(ClientIntent(vec![
                0x49, 0x0B, 0, 0, 0, 0x1D, 0, 0, 0
            ]))]
        );
        assert!(p.choose_row(&v, 2).is_empty());
        let close = PanelOutput::Intent(ClientIntent(vec![0x49, 0x0B, 0, 0, 0, 0, 0, 0, 0]));
        assert_eq!(
            p.close(),
            vec![
                close.clone(),
                PanelOutput::SetUi {
                    ui: 0x14,
                    mode: 1,
                    jump: true
                }
            ]
        );
        assert_eq!(p.close_hook(), vec![close]);
    }

    // §13.1: player gone → close, 0x49 level 0 once (latch).
    // Covers: specs/ui/panels.md §13 r1
    #[test]
    fn self_close_latch() {
        let mut p = WaypointPanel::new();
        p.open(7);
        assert!(p.self_close(true).is_empty());
        let off = PanelOutput::SetUi {
            ui: 0x14,
            mode: 1,
            jump: false,
        };
        let msg = PanelOutput::Intent(ClientIntent(vec![0x49, 7, 0, 0, 0, 0, 0, 0, 0]));
        assert_eq!(p.self_close(false), vec![off.clone(), msg.clone()]);
        assert_eq!(p.self_close(false), vec![off.clone()]);
        p.open(7);
        assert_eq!(p.self_close(false), vec![off, msg]);
    }
}
