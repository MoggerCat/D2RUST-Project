// Spec: specs/ui/messages.md
//! §5 overhead text (the per-unit record, the bubble, its placement and
//! draw) and §8 the timed text box (`0x004A1510`).

use super::{atol, LineDraw, Ltrb, Metrics, RectDraw, FONT_CHAT};

/// The steps of the text pass `0x004A0E70` (UI pass step 10), in order
/// (§5 r1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextPassStep {
    /// Counter += 1, the view origin read, the placed-bubble list reset.
    Begin,
    PlayerBubbles,
    MonsterBubbles,
    ObjectBubbles,
    /// The dialog panel step (§7 r6).
    DialogPanel,
    /// The timed box (§8 r2).
    TimedBox,
    /// The screen messages (§2 r4) while state 0x18 is closed.
    ScreenMessages,
    /// The menu box `[0x007BF1B0]` (`ui/menus.md` §2) when it exists.
    MenuBox,
}

/// §5 r1: the order of the text pass.
pub const TEXT_PASS: [TextPassStep; 8] = [
    TextPassStep::Begin,
    TextPassStep::PlayerBubbles,
    TextPassStep::MonsterBubbles,
    TextPassStep::ObjectBubbles,
    TextPassStep::DialogPanel,
    TextPassStep::TimedBox,
    TextPassStep::ScreenMessages,
    TextPassStep::MenuBox,
];

/// The dialog panel's size in the placed-bubble list (§5 r1).
pub const DIALOG_W: i32 = 325;
pub const DIALOG_H: i32 = 112;
/// Bubbles placed before the next is drawn unplaced (§5 r4).
pub const MAX_PLACED: usize = 16;
/// Placement candidate size (§5 r5: not the bubble's).
pub const CAND_W: i32 = 400;
pub const CAND_H: i32 = 280;
/// `0x00722654`: used as `m` for both axes (§5 r5).
pub const MOVE_TABLE: [i32; 8] = [1, 1, -1, 1, 1, -1, -1, -1];

/// An overhead record, unit +0xA4 (§5 r2): text, language and the end
/// counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverheadRecord {
    pub text: Vec<u8>,
    pub lang: u8,
    /// Counter at creation + d.
    pub end: u32,
}

impl OverheadRecord {
    /// The record of 0x26 type 5 (the player's text).
    pub fn new(counter: u32, d: u32, text: &[u8], lang: u8) -> Self {
        Self {
            text: text.to_vec(),
            lang,
            end: counter.wrapping_add(d),
        }
    }

    /// The record of 0x27 (type 1, kind 3, count 1): the decimal string
    /// of a string id.
    pub fn from_string_id(counter: u32, d: u32, id: u32) -> Self {
        Self::new(counter, d, id.to_string().as_bytes(), 0)
    }

    /// §5 r4: the counter is above the end: the record is freed (and the
    /// frame still draws it).
    pub fn expired(&self, counter: u32) -> bool {
        counter > self.end
    }
}

/// The unit kinds of §5 r3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Player,
    /// Monsters and objects.
    Other,
}

/// §5 r3: the bubble point of a unit, or `None` when nothing is drawn.
/// `(ux, uy)` is the unit's client pixel point (`0x00620900`), `view` the
/// view rectangle's (left, top).
pub fn bubble_point(
    ux: i32,
    uy: i32,
    view: (i32, i32),
    kind: UnitKind,
    open_mode: u8,
    w: i32,
    h: i32,
) -> Option<(i32, i32)> {
    let mut px = ux - view.0;
    let mut py = uy - view.1;
    py -= if kind == UnitKind::Player { 30 } else { 10 };
    match open_mode {
        1 => px -= w / 4,
        2 => px += w / 4,
        3 => return None,
        _ => {}
    }
    (0 < py && py < h && -100 < px && px < w + 100).then_some((px, py))
}

/// The text a bubble shows (§5 r3). `string(id)` is the string table
/// (`0x00524A30`); `convert(text, lang)` the record text converted with
/// its language (`0x0049E280`, `None` on failure).
/// The string table (`0x00524A30`).
pub type StringFn<'a> = &'a dyn Fn(u32) -> Option<Vec<u16>>;
/// The record text converted with its language (`0x0049E280`).
pub type ConvertFn<'a> = &'a dyn Fn(&[u8], u8) -> Option<Vec<u16>>;

pub fn bubble_text(
    kind: UnitKind,
    rec: &OverheadRecord,
    string: StringFn<'_>,
    convert: ConvertFn<'_>,
) -> Option<Vec<u16>> {
    let cut = |mut v: Vec<u16>| {
        v.truncate(199);
        v
    };
    match kind {
        UnitKind::Other => {
            let v = atol(&rec.text);
            if (1..=0xFFFE).contains(&v) {
                string(v as u32).map(cut)
            } else {
                None
            }
        }
        UnitKind::Player => {
            if rec.text.starts_with(&[0xFF, 0xFF]) {
                let v = atol(&rec.text[2..]);
                if (1069..=1295).contains(&v) || (21894..=22037).contains(&v) || v == 10916 {
                    if let Some(s) = string(v as u32).filter(|s| !s.is_empty()) {
                        return Some(cut(s));
                    }
                }
            }
            convert(&rec.text, rec.lang)
        }
    }
}

/// The box of a bubble (`0x0049D3C0`, §5 r4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BubbleBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub lines: Vec<Vec<u16>>,
    /// Field +0x16: 3 not drawn, 0 → 1 at the first draw (§5 r6, open
    /// question 2: d2rs uses 0).
    pub flag: u8,
}

impl BubbleBox {
    /// Font 13, wrapped to 180 pixels: w = widest line (width A) + 20, h
    /// = 15 · lines + 6. Position (x, y) is set by [`Self::at`].
    pub fn new(text: &[u16], m: &dyn Metrics) -> Self {
        let lines = m.wrap(FONT_CHAT, text, 180);
        let widest = lines
            .iter()
            .map(|l| m.width_a(FONT_CHAT, l))
            .max()
            .unwrap_or(0);
        Self {
            x: 0,
            y: 0,
            w: widest + 20,
            h: 15 * lines.len() as i32 + 6,
            lines,
            flag: 0,
        }
    }

    /// §5 r4 position: x = px − w / 2 (`w >> 1`), y = py − 50 − 15 ·
    /// lines − 3.
    pub fn at(mut self, px: i32, py: i32) -> Self {
        self.x = px - (self.w >> 1);
        self.y = py - 50 - 15 * self.lines.len() as i32 - 3;
        self
    }

    pub fn rect(&self) -> Ltrb {
        Ltrb::xywh(self.x, self.y, self.w, self.h)
    }
}

/// The result of one bubble (§5 r4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BubbleResult {
    /// Placed (possibly moved) or unplaced, and the draw (§5 r6) ran.
    Drawn {
        draw: Option<BubbleDraw>,
        placed: bool,
    },
    /// Placement refused: no draw, and font 13 stays current.
    Refused { font_restored: bool },
}

/// The draw of a bubble (§5 r6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BubbleDraw {
    pub backing: RectDraw,
    pub lines: Vec<LineDraw>,
}

/// §5 r6, `0x0049D9A0` → `0x0049D8E0(box, x, y)`.
pub fn bubble_draw(b: &mut BubbleBox, w: i32, h: i32, open_mode: u8) -> Option<BubbleDraw> {
    let (x, y) = (b.x, b.y);
    let skip = match open_mode {
        1 => x > w / 2 - 10,
        2 => x < w / 2 + 10,
        3 => true,
        _ => false,
    };
    if skip || b.flag == 3 {
        return None;
    }
    if b.flag == 0 {
        b.flag = 1;
    }
    if !(y >= 0 && y + b.h < h) {
        return None;
    }
    let n = b.lines.len() as i32;
    let shown = n.min(10);
    let lines = b
        .lines
        .iter()
        .take(shown as usize)
        .enumerate()
        .map(|(i, l)| LineDraw {
            text: l.clone(),
            x: x + 10,
            y: y + 18 + 15 * (i as i32 + n - shown),
            color: 0,
        })
        .collect();
    Some(BubbleDraw {
        backing: RectDraw {
            x,
            y: y + 4,
            w: b.w,
            h: b.h - 5,
            color: 0,
            mode: 1,
        },
        lines,
    })
}

/// The per-frame state of the text pass: counter `[0x007BF20E]`, the
/// view origin `[0x007BF214]` and the placed bubbles (slots
/// `0x007BF0A8`, count `[0x007BF224]`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverheadPass {
    pub counter: u32,
    pub view: (i32, i32),
    slots: Vec<Ltrb>,
    count: usize,
}

impl OverheadPass {
    /// §5 r1, the start of the pass: the counter += 1, the view read, the
    /// placed count := 1 with slot 0 = the dialog panel rectangle while
    /// the panel is up (`dialog` = its x and y), else 0.
    pub fn begin(&mut self, view: (i32, i32), dialog: Option<(i32, i32)>) {
        self.counter = self.counter.wrapping_add(1);
        self.view = view;
        self.slots.clear();
        self.count = 0;
        if let Some((x, y)) = dialog {
            self.slots.push(Ltrb::new(x, y, x + DIALOG_W, y + DIALOG_H));
            self.count = 1;
        }
    }

    /// `[0x007BF224]`.
    pub fn placed(&self) -> usize {
        self.count
    }

    pub fn slots(&self) -> &[Ltrb] {
        &self.slots
    }

    /// `0x0049E070(rect, n)` (§5 r5): the (possibly moved) rectangle, or
    /// `None` (refused). The accepted rectangle is stored in slot n.
    /// PROVISIONAL (specs/ui/messages.md §5 r5; REC-ui-bubble-move): the
    /// slot holds the 400 × 280 candidate and the bubble keeps its own
    /// size at the candidate's origin.
    pub fn place(&mut self, rect: Ltrb, w: i32, h: i32) -> Option<Ltrb> {
        let n = self.slots.len();
        if n == 0 || !self.slots.iter().any(|s| rect.intersects(s)) {
            self.slots.push(rect);
            return Some(rect);
        }
        for dx in (50..=350).step_by(50) {
            for dy in (35..=245).step_by(35) {
                for m in MOVE_TABLE {
                    let (x, y) = (rect.l + m * dx, rect.t + m * dy);
                    let cand = Ltrb::xywh(x, y, CAND_W, CAND_H);
                    if x >= 0
                        && x + CAND_W <= w
                        && y >= 0
                        && y + CAND_H <= h
                        && !self.slots.iter().any(|s| cand.intersects(s))
                    {
                        self.slots.push(cand);
                        let (bw, bh) = (rect.r - rect.l, rect.b - rect.t);
                        return Some(Ltrb::xywh(x, y, bw, bh));
                    }
                }
            }
        }
        None
    }

    /// One bubble (`0x004A0A00`, §5 r4 and r6): placement runs while
    /// fewer than 16 bubbles are placed; a refusal draws nothing and does
    /// not restore the font; with 16 placed the bubble is drawn unplaced.
    /// A drawn bubble counts.
    pub fn bubble(&mut self, mut b: BubbleBox, w: i32, h: i32, open_mode: u8) -> BubbleResult {
        let placed = self.count < MAX_PLACED;
        if placed {
            match self.place(b.rect(), w, h) {
                Some(r) => {
                    b.x = r.l;
                    b.y = r.t;
                }
                None => {
                    return BubbleResult::Refused {
                        font_restored: false,
                    }
                }
            }
        }
        self.count += 1;
        BubbleResult::Drawn {
            draw: bubble_draw(&mut b, w, h, open_mode),
            placed,
        }
    }
}

/// The timed text box `[0x007BF1A8]` (§8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedBox {
    pub bx: BubbleBox,
    /// `[0x007BF1AC]`.
    pub expiry: u32,
}

/// What the text pass does with the timed box (§8 r2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimedBoxPass {
    Draw(Option<BubbleDraw>),
    /// After the expiry: freed (`0x0049E300`), `[0x007BF1C4]` and the
    /// unit-dialog flag `[0x007BF20A]` cleared.
    Freed,
}

impl TimedBox {
    /// `0x004A1510(id)` (§8 r1): the string wrapped to 180 pixels in font
    /// 13; expiry = now + (L + 25) · 200 ms, L = the text length in
    /// units; x = 320 − w / 2, y = 100 − h / 4 (fixed, not scaled by W or
    /// H).
    pub fn open(text: &[u16], now: u32, m: &dyn Metrics) -> Self {
        let mut bx = BubbleBox::new(text, m);
        bx.flag = 1;
        bx.x = 320 - bx.w / 2;
        bx.y = 100 - bx.h / 4;
        Self {
            bx,
            expiry: now.wrapping_add((text.len() as u32 + 25) * 200),
        }
    }

    /// §8 r2, per text pass: before the expiry the box is drawn as a
    /// bubble (§5 r6) at (x, y); after it the box is freed.
    pub fn pass(&mut self, now: u32, w: i32, h: i32, open_mode: u8) -> TimedBoxPass {
        if now < self.expiry {
            TimedBoxPass::Draw(bubble_draw(&mut self.bx, w, h, open_mode))
        } else {
            TimedBoxPass::Freed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::{w, Fixed};
    use super::*;

    // Covers: specs/ui/messages.md §5 r1
    #[test]
    fn text_pass_order_and_begin() {
        assert_eq!(TEXT_PASS[0], TextPassStep::Begin);
        assert_eq!(TEXT_PASS[1], TextPassStep::PlayerBubbles);
        assert_eq!(TEXT_PASS[2], TextPassStep::MonsterBubbles);
        assert_eq!(TEXT_PASS[3], TextPassStep::ObjectBubbles);
        assert_eq!(TEXT_PASS[4], TextPassStep::DialogPanel);
        assert_eq!(TEXT_PASS[5], TextPassStep::TimedBox);
        assert_eq!(TEXT_PASS[6], TextPassStep::ScreenMessages);
        assert_eq!(TEXT_PASS[7], TextPassStep::MenuBox);
        let mut p = OverheadPass::default();
        p.begin((10, 20), None);
        assert_eq!((p.counter, p.view, p.placed()), (1, (10, 20), 0));
        // The dialog panel is slot 0 and counts as one placed bubble.
        p.begin((0, 0), Some((237, 12)));
        assert_eq!((p.counter, p.placed()), (2, 1));
        assert_eq!(p.slots(), &[Ltrb::new(237, 12, 562, 124)]);
    }

    // Test vector "overhead record from 0x27 str 3983, counter 1000".
    // Covers: specs/ui/messages.md §5 r2
    #[test]
    fn record_end_and_text() {
        let r = OverheadRecord::from_string_id(1000, 8 * 4 + 125, 3983);
        assert_eq!(r.end, 1157);
        assert_eq!(r.text, b"3983");
        let p = OverheadRecord::new(5, 10, b"hello", 2);
        assert_eq!((p.end, p.lang, p.text.as_slice()), (15, 2, &b"hello"[..]));
        // §5 r4: above the end the record is freed.
        assert!(!r.expired(1157));
        assert!(r.expired(1158));
    }

    // Covers: specs/ui/messages.md §5 r3
    #[test]
    fn per_unit_point_and_text() {
        // Monster: py −= 10; mode 1 px −= W/4; mode 2 px += W/4; mode 3 none.
        assert_eq!(
            bubble_point(400, 310, (0, 0), UnitKind::Other, 0, 800, 600),
            Some((400, 300))
        );
        assert_eq!(
            bubble_point(400, 330, (100, 20), UnitKind::Player, 0, 800, 600),
            Some((300, 280))
        );
        assert_eq!(
            bubble_point(400, 310, (0, 0), UnitKind::Other, 1, 800, 600),
            Some((200, 300))
        );
        assert_eq!(
            bubble_point(400, 310, (0, 0), UnitKind::Other, 2, 800, 600),
            Some((600, 300))
        );
        assert_eq!(
            bubble_point(400, 310, (0, 0), UnitKind::Other, 3, 800, 600),
            None
        );
        // W/4 rounds toward 0 (W = 802 → 200).
        assert_eq!(
            bubble_point(0, 100, (0, 0), UnitKind::Other, 2, 802, 600),
            Some((200, 90))
        );
        // The visible window: 0 < py < H and −100 < px < W + 100.
        assert!(bubble_point(0, 10, (0, 0), UnitKind::Other, 0, 800, 600).is_none());
        assert!(bubble_point(0, 11, (0, 0), UnitKind::Other, 0, 800, 600).is_some());
        assert!(bubble_point(-100, 100, (0, 0), UnitKind::Other, 0, 800, 600).is_none());
        assert!(bubble_point(-99, 100, (0, 0), UnitKind::Other, 0, 800, 600).is_some());
        assert!(bubble_point(900, 100, (0, 0), UnitKind::Other, 0, 800, 600).is_none());
        assert!(bubble_point(899, 100, (0, 0), UnitKind::Other, 0, 800, 600).is_some());
        assert!(bubble_point(0, 610, (0, 0), UnitKind::Other, 0, 800, 600).is_none());
        // Text: monsters take the string of atol(text) in 1..=0xFFFE.
        let strings = |id: u32| Some(w(&format!("S{id}")));
        let conv = |t: &[u8], _l: u8| Some(t.iter().map(|&b| u16::from(b)).collect::<Vec<u16>>());
        let rec = |t: &[u8]| OverheadRecord::new(0, 0, t, 0);
        assert_eq!(
            bubble_text(UnitKind::Other, &rec(b"3983"), &strings, &conv),
            Some(w("S3983"))
        );
        assert_eq!(
            bubble_text(UnitKind::Other, &rec(b"0"), &strings, &conv),
            None
        );
        assert_eq!(
            bubble_text(UnitKind::Other, &rec(b"65535"), &strings, &conv),
            None
        );
        assert_eq!(
            bubble_text(UnitKind::Other, &rec(b"abc"), &strings, &conv),
            None
        );
        // The string is cut to 199 units.
        let long = |_: u32| Some(vec![65u16; 300]);
        assert_eq!(
            bubble_text(UnitKind::Other, &rec(b"7"), &long, &conv)
                .unwrap()
                .len(),
            199
        );
        // Players: 0xFF 0xFF + a number in the voice-line ranges.
        for ok in [
            &b"\xff\xff1069"[..],
            &b"\xff\xff1295"[..],
            &b"\xff\xff21894"[..],
            &b"\xff\xff22037"[..],
            &b"\xff\xff10916"[..],
        ] {
            assert!(
                bubble_text(UnitKind::Player, &rec(ok), &strings, &conv)
                    .unwrap()
                    .starts_with(&w("S")),
                "{ok:?}"
            );
        }
        // Outside the ranges, or without the prefix: the converted text.
        assert_eq!(
            bubble_text(UnitKind::Player, &rec(b"hi"), &strings, &conv),
            Some(w("hi"))
        );
        assert_eq!(
            bubble_text(UnitKind::Player, &rec(b"\xff\xff1068"), &strings, &conv),
            Some(vec![255, 255, 49, 48, 54, 56])
        );
        // An empty string falls back to the conversion; a conversion
        // failure shows nothing.
        let empty = |_: u32| Some(Vec::new());
        assert!(bubble_text(UnitKind::Player, &rec(b"\xff\xff1069"), &empty, &conv).is_some());
        let fail = |_: &[u8], _: u8| -> Option<Vec<u16>> { None };
        assert_eq!(
            bubble_text(UnitKind::Player, &rec(b"hi"), &strings, &fail),
            None
        );
    }

    // Test vector "monster bubble, one 80-px line, (px, py) = (400, 310)".
    // Covers: specs/ui/messages.md §5 r4, §5 r6
    #[test]
    fn bubble_box_and_draw() {
        // An 80-px line is 80 / 7 → use 8 units of 7 → 56; build a text
        // whose width A is 80 with the 7-px metric: 80 is not a multiple,
        // so check w = widest + 20 with 11 units (77).
        let m = Fixed;
        let b = BubbleBox::new(&[65u16; 11], &m).at(400, 300);
        assert_eq!((b.w, b.h), (97, 21));
        assert_eq!((b.x, b.y), (400 - 48, 300 - 50 - 15 - 3));
        // The spec's vector with an 80-px line: w 100, h 21 at (350, 232).
        let mut b = BubbleBox {
            x: 0,
            y: 0,
            w: 100,
            h: 21,
            lines: vec![w("0123456789")],
            flag: 0,
        }
        .at(400, 300);
        assert_eq!((b.x, b.y), (350, 232));
        let d = bubble_draw(&mut b, 800, 600, 0).unwrap();
        assert_eq!(
            d.backing,
            RectDraw {
                x: 350,
                y: 236,
                w: 100,
                h: 16,
                color: 0,
                mode: 1
            }
        );
        assert_eq!((d.lines[0].x, d.lines[0].y), (360, 250));
        // Field +0x16: 0 → 1 at the first draw; 3 is never drawn.
        assert_eq!(b.flag, 1);
        b.flag = 3;
        assert!(bubble_draw(&mut b, 800, 600, 0).is_none());
        b.flag = 1;
        // y ≥ 0 and y + h < H.
        b.y = -1;
        assert!(bubble_draw(&mut b, 800, 600, 0).is_none());
        b.y = 600 - 21;
        assert!(bubble_draw(&mut b, 800, 600, 0).is_none());
        b.y = 600 - 22;
        assert!(bubble_draw(&mut b, 800, 600, 0).is_some());
        // Open modes: mode 1 not drawn at x > W/2 − 10, mode 2 at x < W/2 + 10,
        // mode 3 never.
        b.y = 100;
        b.x = 391;
        assert!(bubble_draw(&mut b, 800, 600, 1).is_none());
        b.x = 390;
        assert!(bubble_draw(&mut b, 800, 600, 1).is_some());
        b.x = 409;
        assert!(bubble_draw(&mut b, 800, 600, 2).is_none());
        b.x = 410;
        assert!(bubble_draw(&mut b, 800, 600, 2).is_some());
        assert!(bubble_draw(&mut b, 800, 600, 3).is_none());
        // A bubble of more than 10 lines draws its first 10 lines at the
        // positions of the last 10.
        let mut b = BubbleBox {
            x: 10,
            y: 100,
            w: 50,
            h: 15 * 12 + 6,
            lines: (0..12).map(|i| w(&format!("l{i}"))).collect(),
            flag: 0,
        };
        let d = bubble_draw(&mut b, 800, 600, 0).unwrap();
        assert_eq!(d.lines.len(), 10);
        assert_eq!(d.lines[0].y, 100 + 18 + 15 * 2);
        assert_eq!(d.lines[9].y, 100 + 18 + 15 * 11);
    }

    // Test vector "two bubbles with equal rectangles (40, 40, 140, 61)".
    // Covers: specs/ui/messages.md §5 r5
    #[test]
    fn placement_moves_the_second_bubble() {
        let mut p = OverheadPass::default();
        p.begin((0, 0), None);
        let r = Ltrb::new(40, 40, 140, 61);
        assert_eq!(p.place(r, 800, 600), Some(r));
        // The first goes in unchanged (n = 0); the second intersects it.
        let moved = p.place(r, 800, 600).unwrap();
        assert_eq!((moved.l, moved.t), (90, 75));
        assert_eq!(p.slots().len(), 2);
        assert_eq!(p.slots()[1], Ltrb::xywh(90, 75, 400, 280));
        // No intersection → slot n := rect and accept.
        let mut p = OverheadPass::default();
        p.begin((0, 0), None);
        p.place(r, 800, 600).unwrap();
        let far = Ltrb::new(300, 300, 400, 321);
        assert_eq!(p.place(far, 800, 600), Some(far));
        // Nothing fits at W = 100: refused.
        let mut p = OverheadPass::default();
        p.begin((0, 0), None);
        p.place(r, 100, 100).unwrap();
        assert_eq!(p.place(r, 100, 100), None);
    }

    // Covers: specs/ui/messages.md §5 r5
    #[test]
    fn placement_tries_only_diagonal_moves_in_table_order() {
        // The dialog panel (650, 500)–(975, 612) is slot 0; the bubble
        // (600, 520)–(700, 541) overlaps it. Every (+, +) candidate fails
        // the bounds; the first (−, −) candidate that clears the slot is
        // dx 350, dy 210 (x' = 250 ends at 650, touching the slot).
        let mut p = OverheadPass::default();
        p.begin((0, 0), Some((650, 500)));
        let r = Ltrb::new(600, 520, 700, 541);
        let moved = p.place(r, 800, 600).unwrap();
        assert_eq!(moved, Ltrb::xywh(250, 310, 100, 21));
        assert_eq!(p.slots()[1], Ltrb::xywh(250, 310, 400, 280));
        assert_eq!(MOVE_TABLE, [1, 1, -1, 1, 1, -1, -1, -1]);
    }

    // Covers: specs/ui/messages.md §5 r4
    #[test]
    fn sixteen_placed_then_unplaced_and_refusal_keeps_the_font() {
        let m = Fixed;
        let mut p = OverheadPass::default();
        p.begin((0, 0), None);
        let mk = |x| BubbleBox::new(&w("hello"), &m).at(x, 300);
        // Sixteen placed bubbles at distinct places.
        for i in 0..16 {
            let r = p.bubble(mk(40 + 60 * (i % 16)), 800, 600, 0);
            assert!(matches!(r, BubbleResult::Drawn { placed: true, .. }), "{i}");
        }
        assert_eq!(p.placed(), 16);
        // The 17th is drawn unplaced, on top of anything; it counts.
        let r = p.bubble(mk(400), 800, 600, 0);
        assert!(matches!(r, BubbleResult::Drawn { placed: false, .. }));
        assert_eq!(p.placed(), 17);
        assert_eq!(p.slots().len(), 16);
        // A refusal draws nothing and leaves font 13 current; the count
        // is not bumped.
        let mut p = OverheadPass::default();
        p.begin((0, 0), None);
        p.bubble(mk(400), 100, 100, 0);
        let before = p.placed();
        assert_eq!(
            p.bubble(mk(400), 100, 100, 0),
            BubbleResult::Refused {
                font_restored: false
            }
        );
        assert_eq!(p.placed(), before);
    }

    // Covers: specs/ui/messages.md §8 r1, §8 r2
    #[test]
    fn timed_box_geometry_and_lifetime() {
        let m = Fixed;
        // Test vector: a text of 20 units is shown 9000 ms.
        let text = vec![65u16; 20];
        let mut t = TimedBox::open(&text, 5000, &m);
        assert_eq!(t.expiry, 5000 + 45 * 200);
        // w = 140 + 20 = 160, h = 21: x = 320 − 80, y = 100 − 5.
        assert_eq!((t.bx.w, t.bx.h), (160, 21));
        assert_eq!((t.bx.x, t.bx.y), (240, 95));
        // Before the expiry: drawn as a bubble; after: freed.
        assert!(matches!(
            t.pass(5000 + 8999, 640, 480, 0),
            TimedBoxPass::Draw(Some(_))
        ));
        assert_eq!(t.pass(5000 + 9000, 640, 480, 0), TimedBoxPass::Freed);
        // Fixed position at every resolution.
        let a = TimedBox::open(&text, 0, &m);
        assert_eq!((a.bx.x, a.bx.y), (240, 95));
    }
}
