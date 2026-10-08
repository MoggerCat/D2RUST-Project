// Spec: specs/ui/frontend-credits.md (C3, C4, C7, C10)
//! The credits screen (`0x004312C0`): the credits text file parsed into the
//! three columns A, B, C (C4), scrolled 2 px per drawn tick from baseline 590
//! (C3). EXIT or Esc leaves to the main menu; nothing else does, and the
//! screen never returns by itself with the shipped files (C3 r6).
//!
//! The pure parts ([`decode`], [`parse`], [`Scroll`], [`layout`]) take the
//! font advance as a function so they test without game files.
//!
//! PROVISIONAL (REC-185): the host draws [`Credits::rows`] (FontFormal10,
//! font 3 in `ui/text.md`'s id table) from `CreditsScreen::visible_rows`;
//! the shared draw list has no per-tick text yet. Missing text file: spec C4
//! r2 is a fatal error; the preview shows an empty list instead.

use crate::ui::front_end::control::{vk, Action, Control, ControlKind};
use crate::ui::front_end::flow::Trigger;
use crate::ui::front_end::screen::{FrontCtx, Screen};
use crate::ui::front_end::{DrawItem, Registry, CREDITS};
use crate::ui::geom::Point;

/// Blank rows before the first line and before "The End" (C4 r3).
const LEAD_BLANKS: usize = 50;
/// Row pitch: font height 15 + line gap 4 (C3 r2).
pub const PITCH: i32 = 19;
/// Rows drawn per column (C3 r2: while 610 − 19k ≥ 19).
pub const VISIBLE_ROWS: usize = 32;
/// Top row at entry (C3 r1).
pub const TOP_START: usize = 20;
/// Column width (C3 table).
const COL_W: i32 = 250;
/// Heading colour k; every other row is 4 (C4 r4).
pub const K_HEADING: i32 = 1;
pub const K_TEXT: i32 = 4;

pub const BACKGROUND: &str = "CharSelect\\creditsbckg";
pub const BACKGROUND_EXPANSION: &str = "CharSelect\\creditsbckgexpand";
pub const BUTTON: &str = "FrontEnd\\MediumButtonBlank";
/// String 5101, `EXIT`.
const EXIT_STRING: u32 = 5101;
/// FontFormal10 (`0x007089D4`).
pub const FONT: u16 = 3;

/// One text row of a column.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub text: Vec<u16>,
    pub k: i32,
}

impl Row {
    fn blank() -> Self {
        Self::default()
    }
    fn new(text: &[u16], k: i32) -> Self {
        Self {
            text: text.to_vec(),
            k,
        }
    }
}

/// The three columns (A right-aligned, B left-aligned, C centred).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Credits {
    pub a: Vec<Row>,
    pub b: Vec<Row>,
    pub c: Vec<Row>,
}

/// C4 r2: the file bytes → text units, CR and LF → NUL. `None`: ≤ 2 bytes.
pub fn decode(bytes: &[u8]) -> Option<Vec<u16>> {
    if bytes.len() <= 2 {
        return None;
    }
    let pairs =
        |b: &[u8]| -> Vec<[u8; 2]> { (0..b.len() / 2).map(|i| [b[2 * i], b[2 * i + 1]]).collect() };
    let mut units: Vec<u16> = match u16::from_le_bytes([bytes[0], bytes[1]]) {
        0xFEFF => pairs(&bytes[2..])
            .into_iter()
            .map(u16::from_le_bytes)
            .collect(),
        0xFFFE => pairs(&bytes[2..])
            .into_iter()
            .map(u16::from_be_bytes)
            .collect(),
        _ => bytes.iter().map(|&b| u16::from(b)).collect(),
    };
    for u in &mut units {
        if *u == 13 || *u == 10 {
            *u = 0;
        }
    }
    Some(units)
}

fn lines(units: &[u16]) -> Vec<&[u16]> {
    // The walk steps over the line plus 2 units (it assumes CR LF).
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < units.len() {
        let end = units[pos..]
            .iter()
            .position(|&u| u == 0)
            .map_or(units.len(), |n| pos + n);
        out.push(&units[pos..end]);
        pos = end + 2;
    }
    out
}

fn units_of(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl Credits {
    fn balance(&mut self) {
        let n = self.a.len().max(self.b.len()).max(self.c.len());
        for col in [&mut self.a, &mut self.b, &mut self.c] {
            col.resize(n, Row::blank());
            col.push(Row::blank());
        }
    }
}

/// C4 r3: parse decoded text into the columns.
pub fn parse(units: &[u16]) -> Credits {
    let ls = lines(units);
    let mut cr = Credits::default();
    for col in [&mut cr.c, &mut cr.a, &mut cr.b] {
        col.resize(LEAD_BLANKS, Row::blank());
    }
    let star = |l: &[u16]| l.first() == Some(&u16::from(b'*'));
    let mut i = 0;
    'walk: while i < ls.len() {
        if star(ls[i]) {
            cr.a.push(Row::blank());
            cr.b.push(Row::blank());
            cr.c.push(Row::new(&ls[i][1..], K_HEADING));
            cr.balance();
            i += 1;
            continue;
        }
        while i < ls.len() && !star(ls[i]) {
            let l1 = ls[i];
            let alpha = l1
                .first()
                .is_some_and(|&u| u < 0x80 && (u as u8).is_ascii_alphabetic());
            if !alpha {
                cr.c.push(Row::blank());
                cr.c.push(Row::blank());
                break 'walk;
            }
            i += 1;
            let l2 = match ls.get(i) {
                Some(l) if !star(l) => {
                    i += 1;
                    Some(*l)
                }
                _ => None,
            };
            match l2 {
                Some(l2) if !l2.is_empty() => {
                    cr.a.push(Row::new(l1, K_TEXT));
                    cr.b.push(Row::new(l2, K_TEXT));
                    cr.c.push(Row::blank());
                }
                _ => {
                    cr.c.push(Row::new(l1, K_TEXT));
                    cr.c.push(Row::blank());
                }
            }
        }
        cr.balance();
    }
    for _ in 0..LEAD_BLANKS {
        cr.c.push(Row::blank());
    }
    cr.c.push(Row::new(&units_of("The End"), K_TEXT));
    cr
}

/// The scroll state (C3 r3, r4): top row and pixel offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scroll {
    pub top: usize,
    pub off: i32,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            top: TOP_START,
            off: 0,
        }
    }
}

impl Scroll {
    /// One drawn tick; `rows` = rows in the column. With no next row the
    /// column stops scrolling.
    pub fn step(&mut self, rows: usize) {
        if self.top + 1 >= rows {
            return;
        }
        self.off -= 2;
        if self.off < -19 {
            self.off = 0;
            self.top += 1;
        }
    }
}

/// Which column a drawn row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    A,
    B,
    C,
}

/// One drawn row: pen x, baseline y, colour k.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowDraw {
    pub col: Col,
    pub row: usize,
    pub text: Vec<u16>,
    pub k: i32,
    pub x: i32,
    pub y: i32,
}

/// C3 r2: the rows drawn for scroll state `s`; `adv` = text width (width A of
/// `ui/text.md` §6 in FontFormal10). Blank rows are not listed.
pub fn layout(
    cr: &Credits,
    s: Scroll,
    expansion: bool,
    adv: &dyn Fn(&[u16]) -> i32,
) -> Vec<RowDraw> {
    let (xa, xb, xc) = if expansion {
        (400, 410, 280)
    } else {
        (560, 570, 440)
    };
    let mut out = Vec::new();
    for (col, rows) in [(Col::C, &cr.c), (Col::A, &cr.a), (Col::B, &cr.b)] {
        for k in 0..VISIBLE_ROWS {
            let Some(r) = rows.get(s.top + k) else { break };
            if r.text.is_empty() {
                continue;
            }
            let w = adv(&r.text);
            let x = match col {
                Col::A => xa - w,
                Col::B => xb,
                Col::C => xc + ((COL_W - w) / 2).max(0),
            };
            out.push(RowDraw {
                col,
                row: s.top + k,
                text: r.text.clone(),
                k: r.k,
                x,
                y: (615 - 610) + 15 + PITCH * k as i32 + s.off,
            });
        }
    }
    out
}

/// Where the text comes from: the file bytes for (expansion?). d2rs-own: the
/// default reads loose files under `D2_GAME_DIR`; the host replaces it with the
/// archive reader (`Credits.txt` / `ExpansionCredits.txt`, C4 r1).
pub type TextSource = Box<dyn Fn(bool) -> Option<Vec<u8>>>;

pub fn loose_file(expansion: bool) -> Option<Vec<u8>> {
    let dir = std::env::var_os("D2_GAME_DIR")?;
    let name = if expansion {
        "ExpansionCredits.txt"
    } else {
        "Credits.txt"
    };
    std::fs::read(
        std::path::Path::new(&dir)
            .join("data/local/ui/eng")
            .join(name),
    )
    .ok()
}

/// The screen.
pub struct CreditsScreen {
    source: TextSource,
    credits: Credits,
    scroll: Scroll,
    expansion: bool,
}

impl CreditsScreen {
    pub fn new(source: TextSource) -> Self {
        Self {
            source,
            credits: Credits::default(),
            scroll: Scroll::default(),
            expansion: false,
        }
    }

    /// The rows to draw now (host draw hook).
    pub fn visible_rows(&self, adv: &dyn Fn(&[u16]) -> i32) -> Vec<RowDraw> {
        layout(&self.credits, self.scroll, self.expansion, adv)
    }

    pub fn scroll(&self) -> Scroll {
        self.scroll
    }
}

impl Screen for CreditsScreen {
    fn build(&mut self, ctx: &mut FrontCtx) -> Vec<Control> {
        self.expansion = ctx.expansion;
        self.scroll = Scroll::default();
        self.credits = (self.source)(ctx.expansion)
            .and_then(|b| decode(&b))
            .map(|u| parse(&u))
            .unwrap_or_default();
        let art = if ctx.expansion {
            BACKGROUND_EXPANSION
        } else {
            BACKGROUND
        };
        let exit = Action::Trigger(Trigger::Exit);
        let (xa, xb, xc) = if ctx.expansion {
            (400, 410, 280)
        } else {
            (560, 570, 440)
        };
        let col = |x| {
            let mut c = Control::new(ControlKind::Text, x, 615, 250, 610);
            c.font = FONT;
            c
        };
        vec![
            // Background: no click, no key handler (C3 table, desc 41).
            Control::new(ControlKind::Image, 0, 599, 800, 600).with_art(art),
            Control::new(ControlKind::Button, 33, 578, 128, 35)
                .with_art(BUTTON)
                .with_string(EXIT_STRING)
                .with_hotkey(vk::ESC)
                .with_action(exit),
            col(xa),
            col(xb),
            col(xc),
        ]
    }

    fn overlay(&mut self, _now_ms: u64, adv: &dyn Fn(u16, &[u16]) -> i32) -> Vec<DrawItem> {
        // d2rs-own, unverified: the colour `k` of a row is not carried by
        // `DrawItem::Text` yet (REC-231).
        self.visible_rows(&|t| adv(FONT, t))
            .into_iter()
            .map(|r| DrawItem::Text {
                string_id: 0,
                text: String::from_utf16_lossy(&r.text),
                font: FONT,
                at: Point::new(r.x, r.y),
            })
            .collect()
    }

    fn tick(&mut self, _ctx: &mut FrontCtx) -> Option<Trigger> {
        self.scroll.step(self.credits.c.len());
        // C3 r6: no automatic return with the shipped files (d2rs: never).
        None
    }
}

pub fn register(reg: &mut Registry) {
    register_with(reg, Box::new(loose_file));
}

pub fn register_with(reg: &mut Registry, source: TextSource) {
    reg.register(CREDITS, Box::new(CreditsScreen::new(source)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Vec<u16> {
        units_of(s)
    }
    fn text(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| String::from_utf16_lossy(&r.text))
            .collect()
    }
    fn file(s: &str) -> Vec<u16> {
        decode(s.replace('\n', "\r\n").as_bytes()).unwrap()
    }

    #[test]
    fn decode_variants() {
        assert_eq!(decode(b"ab"), None);
        assert_eq!(decode(b"a\r\nb"), Some(vec![97, 0, 0, 98]));
        let le = [0xFF, 0xFE, b'a', 0, 13, 0, 10, 0];
        assert_eq!(decode(&le), Some(vec![97, 0, 0]));
        let be = [0xFE, 0xFF, 0, b'a', 0, 13, 0, 10];
        assert_eq!(decode(&be), Some(vec![97, 0, 0]));
    }

    #[test]
    fn spec_vector_heading_pair_stop() {
        // Lines `*H`, `Al`, `Bo`, (blank), `Cy`.
        let cr = parse(&file("*H\nAl\nBo\n\nCy\n"));
        let t = text(&cr.c);
        assert_eq!(t[50], "H");
        assert_eq!(cr.c[50].k, K_HEADING);
        assert_eq!(t[51], "");
        // A/B: pair on the row after the heading's blank.
        assert_eq!(text(&cr.a)[52], "Al");
        assert_eq!(text(&cr.b)[52], "Bo");
        assert!(!t.iter().any(|s| s == "Cy"));
        assert_eq!(t.last().unwrap(), "The End");
        // 53 after the pair (the stop skips balance), + 2 stop blanks + 50 + The End.
        assert_eq!(cr.c.len(), 53 + 2 + 50 + 1);
        assert_eq!(cr.a.len(), 53);
    }

    #[test]
    fn single_line_and_blank_second_line_centre() {
        let cr = parse(&file("Solo\nNext\n\n"));
        // "Solo" with L2 "Next" is a pair; then blank L1 stops.
        assert_eq!(text(&cr.a)[50], "Solo");
        let cr = parse(&file("Solo\n\nZed\nYam\n"));
        // L2 blank → C row "Solo" (k 4) + blank; then pair Zed/Yam.
        assert_eq!(text(&cr.c)[50], "Solo");
        assert_eq!(cr.c[50].k, K_TEXT);
        assert_eq!(text(&cr.a)[50], "Zed");
    }

    #[test]
    fn scroll_vectors() {
        let mut s = Scroll::default();
        let adv = |t: &[u16]| t.len() as i32 * 5;
        // Row 50: pair "Ab"/"Cd" (expansion).
        let cr = parse(&file("Ab\nCd\n"));
        assert_eq!(text(&cr.a)[50], "Ab");
        let row50 = |s: Scroll| {
            layout(&cr, s, true, &adv)
                .into_iter()
                .find(|d| d.col == Col::A && d.row == 50)
                .unwrap()
        };
        let n = cr.c.len();
        s.step(n);
        assert_eq!((s.top, row50(s).y), (20, 588));
        for _ in 1..8 {
            s.step(n);
        }
        s.step(n); // draw 9
        assert_eq!((s.top, row50(s).y), (20, 572));
        s.step(n); // draw 10
        assert_eq!((s.top, row50(s).y), (21, 571));
        // Pair placement: "Ab" ends at x 400, "Cd" starts at 410.
        let d = row50(s);
        assert_eq!(d.x + adv(&d.text), 400);
        let b = layout(&cr, s, true, &adv)
            .into_iter()
            .find(|d| d.col == Col::B && d.row == 50)
            .unwrap();
        assert_eq!(b.x, 410);
        // 300 ticks: row 50 at the top baseline 20.
        let mut s = Scroll::default();
        for _ in 0..300 {
            s.step(10_000);
        }
        assert_eq!((s.top, s.off), (50, 0));
        assert_eq!(row50(s).y, 20);
    }

    #[test]
    fn centred_rows() {
        let mut cr = Credits::default();
        cr.c.push(Row::new(&u("x"), K_TEXT));
        let wide = |_: &[u16]| 300;
        let d = layout(&cr, Scroll { top: 0, off: 0 }, true, &wide);
        assert_eq!(d[0].x, 280); // wider than 250: the column's left edge
        let narrow = |_: &[u16]| 100;
        let d = layout(&cr, Scroll { top: 0, off: 0 }, false, &narrow);
        assert_eq!(d[0].x, 440 + 75);
    }

    #[test]
    fn stops_at_last_row_and_visible_count() {
        let mut s = Scroll { top: 4, off: 0 };
        s.step(5);
        assert_eq!(s, Scroll { top: 4, off: 0 });
        let cr = Credits {
            c: vec![Row::new(&u("r"), K_TEXT); 100],
            ..Default::default()
        };
        let adv = |_: &[u16]| 5;
        let d = layout(&cr, Scroll { top: 0, off: 0 }, true, &adv);
        assert_eq!(d.len(), VISIBLE_ROWS);
        assert_eq!((d[0].y, d[31].y), (20, 609));
    }
}
