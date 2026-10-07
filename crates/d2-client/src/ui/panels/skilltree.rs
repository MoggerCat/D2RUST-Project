// Spec: specs/ui/panels.md
//! Skill tree (ui 4, right slot; §10, `0x004AC690`): the class art and
//! its tab frames, the skill icons of the current tab with their level
//! numbers, the tab and icon mouse handlers and the close button.
//!
//! Game facts (class, skills, free points) come from a [`SkillTreeView`]
//! the caller fills from the client world; the panel decides nothing: a
//! spent point leaves as C→S 0x3B (§10.5), a close as `SetUIState`.
//!
//! Open (spec gaps, see also §Open questions 4):
//! - The class icon file prefix (`CC` of `spells\CCskillicon`, §16.2) is
//!   not specified; the icon file id comes from the view
//!   ([`SkillTreeView::icon_file`]). No icon is drawn without it.
//! - The remap `k` of an icon (§10.3) is implemented literally in the
//!   stated order; spec OQ4 says "exact remap `k` per state: verify by
//!   capture". Unverified. [`UiDraw::Image`] carries no remap, so the
//!   remap is exposed through [`SkillTreePanel::icons`] for the sink.
//! - The close-button offset table `0x00724CE4` (§10.6) is only given for
//!   amazon; other classes have no close button here
//!   ([`close_offset`] returns `None`).
//! - The no-points mouse-down message (§10.5, 9 bytes through
//!   `0x004786A0`), the free-points box and the tab tool tips (§10.7):
//!   not implemented (OQ4).
//! - Whether the close button is pressed by a mouse down (§7.3 general
//!   rule; §10.6 names only the hover / release rectangle), whether a
//!   release elsewhere clears it, and the order of the close test against
//!   the tab / icon tests (the close rectangle overlaps column 3 row 6):
//!   not stated; implemented as press-sets / release-clears, tabs and
//!   icons first.
//! - §10.8: what the release handler does besides not consuming a click
//!   at x ≤ W / 2 is not stated; here it only clears the pressed icon.
//! - Draw order icons → level numbers → close button follows the section
//!   order of §10, not a read of `0x004AC690`.

use d2_proto::client::AddSkillPoint;

use super::{cel, emit_static_draws, text, utf16, PanelEnv, PanelOutput, PanelTables};
use crate::ui::draw::{UiDraw, UiDrawSink};
use crate::ui::geom::Point;
use crate::ui::layout::{Cond, PanelKey, Screen};
use crate::ui::panel::ClientIntent;

/// UI state id of the skill tree (§4.1).
pub const UI_SKILLTREE: u8 = 4;
/// Font16 (`text-fonts.tsv` id 1): level numbers below 10 (§10.4).
pub const FONT16: u16 = 1;
/// FontFormal10 (`text-fonts.tsv` id 4): level numbers ≥ 10 (§10.4).
pub const FONT_FORMAL10: u16 = 4;
/// Close button file (§7.2), path under `data\global\ui\`.
pub const CLOSE_FILE: &str = "panel\\buysellbtn";
/// Size of the close button's hover / release rectangle (§10.6,
/// `0x004AB5F0(0x22, 0x22)`).
pub const CLOSE_SIZE: i32 = 0x22;

/// Icon x offsets for columns 1–3: `X = W − sx − c` (§10.3).
const COLUMN_X: [i32; 3] = [305, 236, 167];
/// Icon y offsets for rows 1–6: `Y = H + sy − r` (§10.3).
const ROW_Y: [i32; 6] = [418, 350, 282, 214, 145, 77];

/// One class skill as the skill tree reads it (§10.3–§10.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillEntry {
    /// skills.txt row (the 0x3B payload).
    pub skill: u16,
    /// skilldesc `SkillPage` (+2): the tab 1–3 (0: not on the tree).
    pub page: u8,
    /// skilldesc `SkillRow` (+3), 1–6.
    pub row: u8,
    /// skilldesc `SkillColumn` (+4), 1–3.
    pub column: u8,
    /// skilldesc `IconCel` (+7).
    pub icon_cel: u32,
    /// The skill level shown (`%d`, §10.4).
    pub level: i32,
    /// Hard points (skill entry +0x28).
    pub hard_points: i32,
    /// The bonus part (`0x00644300`, §10.4).
    pub bonus: i32,
    /// The skill's flag byte (+5) tested against [`SkillTreeView::flag_mask`].
    pub flags: u8,
    /// `0x004AC4D0`: a point would be accepted (start remap 0 rather than
    /// 5 for a level-0 skill, §10.3).
    pub learnable: bool,
    /// The required-level check `0x00646CA0` passes (§10.5).
    pub req_level_ok: bool,
    /// Below the skill's max level (`0x004AA8B0`, `0x004AA900`, §10.5).
    pub below_max: bool,
}

/// The game facts the skill tree draws and tests (§10).
pub trait SkillTreeView {
    /// Player class 0–6 (amazon … assassin, [`super::CLASS_LETTERS`]).
    fn class(&self) -> Option<u8>;
    /// The class icon file id (`spells\CCskillicon`, prefix unspecified:
    /// Open, module doc).
    fn icon_file(&self) -> Option<u32>;
    /// Free skill points (stat 5).
    fn free_points(&self) -> i32;
    /// `[0x006CE270]`: an icon whose flag byte has no bit of it uses
    /// remap 1 (§10.3).
    fn flag_mask(&self) -> u8;
    /// The class skills in `0x00646140` order.
    fn skills(&self) -> &[SkillEntry];
}

/// One icon draw with what [`UiDraw::Image`] cannot carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconDraw {
    pub skill: u16,
    /// The cel draw (frame = `IconCel` + 1 while pressed).
    pub image: UiDraw,
    /// Remap `k` of the colored cel draw `0x004F64B0` (§10.3, unverified).
    pub remap: u8,
    /// The level number (§10.4), when drawn.
    pub level: Option<UiDraw>,
}

/// A release's result: whether the skill tree consumed it (§10.8) and
/// what it asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Release {
    pub consumed: bool,
    pub out: Vec<PanelOutput>,
}

/// Icon draw position (X, Y) of a skilldesc column 1–3 / row 1–6 (§10.3).
pub fn icon_pos(s: &Screen, column: u8, row: u8) -> Option<Point> {
    let cx = COLUMN_X.get(usize::from(column).checked_sub(1)?)?;
    let ry = ROW_Y.get(usize::from(row).checked_sub(1)?)?;
    Some(Point::new(s.w - s.sx() - cx, s.h + s.sy() - ry))
}

/// Icon hit test (§10.5, strict): `X < mx < X + 48`, `Y − 48 < my < Y`.
pub fn icon_hit(at: Point, p: Point) -> bool {
    at.x < p.x && p.x < at.x + 48 && at.y - 48 < p.y && p.y < at.y
}

/// The tab a mouse down at `p` selects (§10.2), if any: only when
/// my ≤ H − 48; x in [`W − sx − 88`, `W − sx`]; y strictly inside the
/// tab's band, the tab-1 bottom bound `H − 49` without `sy`.
pub fn tab_at(s: &Screen, p: Point) -> Option<u8> {
    if p.y > s.h - 48 {
        return None;
    }
    let right = s.w - s.sx();
    if p.x < right - 88 || p.x > right {
        return None;
    }
    let b = s.h + s.sy();
    let inside = |lo: i32, hi: i32| lo < p.y && p.y < hi;
    if inside(b - 372, b - 265) {
        Some(3)
    } else if inside(b - 264, b - 157) {
        Some(2)
    } else if inside(b - 156, s.h - 49) {
        Some(1)
    } else {
        None
    }
}

/// Close-button offset `o` (table `0x00724CE4`, index `3 × class + t`,
/// §10.6). Only the amazon row is specified (−149, −220, −305); `None`
/// for the other classes (Open).
pub fn close_offset(class: u8, tab: u8) -> Option<i32> {
    match (class, tab) {
        (0, 1) => Some(-149),
        (0, 2) => Some(-220),
        (0, 3) => Some(-305),
        _ => None,
    }
}

/// Close-button draw position (`W − sx + o`, `H + sy − 63`) (§10.6).
pub fn close_pos(s: &Screen, class: u8, tab: u8) -> Option<Point> {
    let o = close_offset(class, tab)?;
    Some(Point::new(s.w - s.sx() + o, s.h + s.sy() - 63))
}

/// Close hover / release rectangle (§10.6): x in [`X`, `X + 0x22`], y in
/// [`Y − 0x22`, `Y`], bounds inclusive as written.
pub fn close_hit(s: &Screen, class: u8, tab: u8, p: Point) -> bool {
    let Some(at) = close_pos(s, class, tab) else {
        return false;
    };
    at.x <= p.x && p.x <= at.x + CLOSE_SIZE && at.y - CLOSE_SIZE <= p.y && p.y <= at.y
}

/// Skill tree panel state (§10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillTreePanel {
    /// Current tab `[0x00724BEC]`, 1–3.
    pub tab: u8,
    /// The pressed icon's skill (§10.5).
    pub pressed: Option<u16>,
    /// The close button's pressed flag (frame 11, §7.3).
    pub close_pressed: bool,
}

impl Default for SkillTreePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillTreePanel {
    /// Tab 1, nothing pressed (§10.1).
    pub fn new() -> Self {
        Self {
            tab: 1,
            pressed: None,
            close_pressed: false,
        }
    }

    /// The skills of the current tab with their icon positions.
    fn tab_skills<'a>(
        &'a self,
        s: &'a Screen,
        view: &'a dyn SkillTreeView,
    ) -> impl Iterator<Item = (&'a SkillEntry, Point)> + 'a {
        view.skills()
            .iter()
            .filter(move |e| e.page == self.tab)
            .filter_map(move |e| Some((e, icon_pos(s, e.column, e.row)?)))
    }

    /// Remap `k` of an icon (§10.3), the stated order applied literally
    /// (unverified, spec OQ4): start 5 (level 0 and not learnable) or 0;
    /// 3 if the mouse is strictly inside; 1 if the flag byte has no bit
    /// of the mask; 3 if no free points, a level and not pressed.
    pub fn icon_remap(
        &self,
        view: &dyn SkillTreeView,
        e: &SkillEntry,
        at: Point,
        mouse: Point,
    ) -> u8 {
        let mut k = if e.level == 0 && !e.learnable { 5 } else { 0 };
        if icon_hit(at, mouse) {
            k = 3;
        }
        if e.flags & view.flag_mask() == 0 {
            k = 1;
        }
        if view.free_points() <= 0 && e.level > 0 && self.pressed != Some(e.skill) {
            k = 3;
        }
        k
    }

    /// The icons of the current tab (§10.3, §10.4); empty without the
    /// icon file (Open).
    pub fn icons(&self, s: &Screen, view: &dyn SkillTreeView, mouse: Point) -> Vec<IconDraw> {
        let Some(file) = view.icon_file() else {
            return Vec::new();
        };
        self.tab_skills(s, view)
            .map(|(e, at)| {
                let pressed = self.pressed == Some(e.skill);
                IconDraw {
                    skill: e.skill,
                    image: cel(file, e.icon_cel + u32::from(pressed), at.x, at.y),
                    remap: self.icon_remap(view, e, at, mouse),
                    level: level_text(e, at),
                }
            })
            .collect()
    }

    /// Draws the panel: art frames 0–3 and `4t … 4t + 3` (§10.1), the
    /// icons and level numbers (§10.3, §10.4), the close button (§10.6).
    pub fn draw(
        &self,
        t: &PanelTables,
        env: &PanelEnv,
        view: &dyn SkillTreeView,
        mouse: Point,
        out: &mut dyn UiDrawSink,
    ) {
        let tab = self.tab;
        let extra = move |c: Cond| matches!(c, Cond::Tab(n) if n == tab);
        let cond = env.cond(false, &extra);
        let class = view.class();
        emit_static_draws(t, PanelKey::Ui(UI_SKILLTREE), &cond, class, &|_| true, out);
        for icon in self.icons(&env.screen, view, mouse) {
            out.push(icon.image);
            if let Some(l) = icon.level {
                out.push(l);
            }
        }
        if let (Some(c), Some(file)) = (class, t.files.id(CLOSE_FILE)) {
            if let Some(at) = close_pos(&env.screen, c, self.tab) {
                out.push(cel(file, 10 + u32::from(self.close_pressed), at.x, at.y));
            }
        }
    }

    /// Mouse down (`0x004AB7E0`): tabs (§10.2), icons with free points
    /// (§10.5), the close button (§7.3).
    pub fn mouse_down(
        &mut self,
        env: &PanelEnv,
        view: &dyn SkillTreeView,
        p: Point,
    ) -> Vec<PanelOutput> {
        let s = env.screen;
        let mut out = Vec::new();
        if let Some(t) = tab_at(&s, p) {
            if t != self.tab {
                self.tab = t;
                out.push(PanelOutput::ClickSound);
            }
            return out;
        }
        if view.free_points() > 0 {
            let hit = self
                .tab_skills(&s, view)
                .find(|&(_, at)| icon_hit(at, p))
                .map(|(e, _)| e.skill);
            if let Some(skill) = hit {
                self.pressed = Some(skill);
                return out;
            }
        }
        if view.class().is_some_and(|c| close_hit(&s, c, self.tab, p)) {
            self.close_pressed = true;
        }
        out
    }

    /// Mouse up: a release on the pressed icon with free points, the
    /// required level and below the max level sends 0x3B (§10.5); a
    /// release on the close button toggles ui 4 (§10.6); pressed flags
    /// clear; x ≤ W / 2 is not consumed (§10.8).
    pub fn mouse_up(&mut self, env: &PanelEnv, view: &dyn SkillTreeView, p: Point) -> Release {
        let s = env.screen;
        let pressed = self.pressed.take();
        self.close_pressed = false;
        if p.x <= s.w / 2 {
            return Release::default();
        }
        let mut r = Release {
            consumed: true,
            out: Vec::new(),
        };
        if let Some(skill) = pressed {
            let on = self
                .tab_skills(&s, view)
                .find(|&(e, at)| e.skill == skill && icon_hit(at, p))
                .map(|(e, _)| e);
            if let Some(e) = on {
                if view.free_points() > 0 && e.req_level_ok && e.below_max {
                    r.out.push(PanelOutput::Intent(ClientIntent::from_message(
                        &AddSkillPoint { skill },
                    )));
                }
                return r;
            }
        }
        if view.class().is_some_and(|c| close_hit(&s, c, self.tab, p)) {
            r.out.push(PanelOutput::SetUi {
                ui: UI_SKILLTREE,
                mode: 2,
                jump: false,
            });
        }
        r
    }
}

/// Level number (§10.4): drawn when level > 0 or hard points ≠ 0; `%d` in
/// Font16 at (`X + 48`, `Y + 12`), FontFormal10 at x − 4 for ≥ 10; color 3
/// when the bonus part > 0, 1 when < 0, else 0.
fn level_text(e: &SkillEntry, at: Point) -> Option<UiDraw> {
    if e.level <= 0 && e.hard_points == 0 {
        return None;
    }
    let (font, dx) = if e.level >= 10 {
        (FONT_FORMAL10, -4)
    } else {
        (FONT16, 0)
    };
    let color = match e.bonus.signum() {
        1 => 3,
        -1 => 1,
        _ => 0,
    };
    Some(text(
        utf16(&e.level.to_string()),
        at.x + 48 + dx,
        at.y + 12,
        font,
        color,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::draw::{ImageRef, ImageRequest, TextRequest};

    const ICON_FILE: u32 = 9000;

    struct View {
        class: Option<u8>,
        free: i32,
        mask: u8,
        skills: Vec<SkillEntry>,
    }

    impl SkillTreeView for View {
        fn class(&self) -> Option<u8> {
            self.class
        }
        fn icon_file(&self) -> Option<u32> {
            Some(ICON_FILE)
        }
        fn free_points(&self) -> i32 {
            self.free
        }
        fn flag_mask(&self) -> u8 {
            self.mask
        }
        fn skills(&self) -> &[SkillEntry] {
            &self.skills
        }
    }

    fn entry(skill: u16, page: u8, column: u8, row: u8) -> SkillEntry {
        SkillEntry {
            skill,
            page,
            row,
            column,
            icon_cel: 20,
            level: 0,
            hard_points: 0,
            bonus: 0,
            flags: 1,
            learnable: true,
            req_level_ok: true,
            below_max: true,
        }
    }

    fn view(skills: Vec<SkillEntry>) -> View {
        View {
            class: Some(0),
            free: 1,
            mask: 1,
            skills,
        }
    }

    fn env(screen: Screen) -> PanelEnv {
        PanelEnv {
            screen,
            open_mode: 1,
            exp: true,
        }
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

    fn tables() -> PanelTables {
        PanelTables::load().expect("panel tables")
    }

    // Test vector "amazon skill tree tab 1, 800 × 600": frames 0–3 then
    // 4t … 4t + 3 as right quads (400, 316) …, class letter `a`.
    // Covers: specs/ui/panels.md §10 r1
    #[test]
    fn art_frames_per_tab_amazon_800() {
        let t = tables();
        let e = env(Screen::R800);
        let file = t.files.id("spells\\skltree_a_back").expect("amazon art");
        let quads = [(400, 316), (656, 316), (400, 492), (656, 492)];
        for tab in 1..=3u32 {
            let mut p = SkillTreePanel::new();
            p.tab = tab as u8;
            let v = view(Vec::new());
            let mut out: Vec<UiDraw> = Vec::new();
            p.draw(&t, &e, &v, Point::new(0, 0), &mut out);
            let im = images(&out);
            let mut want = Vec::new();
            for (i, q) in quads.iter().enumerate() {
                want.push((file, i as u32, q.0, q.1));
            }
            for (i, q) in quads.iter().enumerate() {
                want.push((file, 4 * tab + i as u32, q.0, q.1));
            }
            assert_eq!(im[..8], want[..], "tab {tab}");
        }
        // Another class substitutes its letter (sorceress `s`).
        let s_file = t.files.id("spells\\skltree_s_back").unwrap();
        let mut v = view(Vec::new());
        v.class = Some(1);
        let mut out: Vec<UiDraw> = Vec::new();
        SkillTreePanel::new().draw(&t, &env(Screen::R640), &v, Point::new(0, 0), &mut out);
        let im = images(&out);
        assert_eq!(im[0], (s_file, 0, 320, 256));
        assert_eq!(im[7], (s_file, 7, 576, 432));
    }

    // Test vector "close button at (571, 477)"; o per tab; hover/release
    // rectangle; release toggles ui 4 with jump 0.
    // Covers: specs/ui/panels.md §10 r6
    #[test]
    fn close_button_amazon() {
        let t = tables();
        let e = env(Screen::R800);
        let close = t.files.id(CLOSE_FILE).unwrap();
        let v = view(Vec::new());
        let mut p = SkillTreePanel::new();
        let mut out: Vec<UiDraw> = Vec::new();
        p.draw(&t, &e, &v, Point::new(0, 0), &mut out);
        assert_eq!(*images(&out).last().unwrap(), (close, 10, 571, 477));
        assert_eq!(close_pos(&Screen::R800, 0, 2), Some(Point::new(500, 477)));
        assert_eq!(close_pos(&Screen::R800, 0, 3), Some(Point::new(415, 477)));
        assert_eq!(close_pos(&Screen::R640, 0, 1), Some(Point::new(491, 417)));
        // Rectangle [571, 605] × [443, 477].
        for (x, y, inside) in [
            (571, 443, true),
            (605, 477, true),
            (570, 460, false),
            (606, 460, false),
            (580, 442, false),
            (580, 478, false),
        ] {
            assert_eq!(
                close_hit(&Screen::R800, 0, 1, Point::new(x, y)),
                inside,
                "({x},{y})"
            );
        }
        // Press draws frame 11; release inside toggles.
        let mut v0 = view(Vec::new());
        v0.free = 0;
        assert!(p.mouse_down(&e, &v0, Point::new(580, 460)).is_empty());
        assert!(p.close_pressed);
        let mut out: Vec<UiDraw> = Vec::new();
        p.draw(&t, &e, &v0, Point::new(0, 0), &mut out);
        assert_eq!(*images(&out).last().unwrap(), (close, 11, 571, 477));
        let r = p.mouse_up(&e, &v0, Point::new(580, 460));
        assert!(r.consumed);
        assert_eq!(
            r.out,
            vec![PanelOutput::SetUi {
                ui: 4,
                mode: 2,
                jump: false
            }]
        );
        assert!(!p.close_pressed);
        // Release outside: nothing.
        assert!(p.mouse_up(&e, &v0, Point::new(620, 460)).out.is_empty());
    }

    #[test]
    fn close_button_unspecified_classes_pending() {
        for class in 1..7 {
            for tab in 1..=3 {
                assert_eq!(close_offset(class, tab), None);
            }
        }
        let t = tables();
        let mut v = view(Vec::new());
        v.class = Some(3);
        let mut out: Vec<UiDraw> = Vec::new();
        SkillTreePanel::new().draw(&t, &env(Screen::R800), &v, Point::new(0, 0), &mut out);
        let close = t.files.id(CLOSE_FILE).unwrap();
        assert!(images(&out).iter().all(|i| i.0 != close));
    }

    // Test vector "skill in column 2, row 3 at 800 × 600: icon at
    // (484, 258); hit 485–531 × 211–257" (§10.3 position, §10.5 strict hit).
    // Covers: specs/ui/panels.md §10 r3, §10 r5
    #[test]
    fn icon_position_and_strict_hit() {
        let s = Screen::R800;
        assert_eq!(icon_pos(&s, 2, 3), Some(Point::new(484, 258)));
        let at = Point::new(484, 258);
        for x in 480..540 {
            for y in 205..265 {
                let want = (485..=531).contains(&x) && (211..=257).contains(&y);
                assert_eq!(icon_hit(at, Point::new(x, y)), want, "({x},{y})");
            }
        }
        // Every column / row against the layout table's draw and hit rows.
        let t = tables();
        for c in 1..=3u8 {
            for r in 1..=6u8 {
                let name = format!("icon_c{c}_r{r}");
                let draw = t
                    .item(PanelKey::Ui(4), &name, crate::ui::layout::RowKind::Draw)
                    .next()
                    .unwrap();
                let hit = t
                    .item(PanelKey::Ui(4), &name, crate::ui::layout::RowKind::Hit)
                    .next()
                    .unwrap();
                for scr in [Screen::R640, Screen::R800] {
                    let at = icon_pos(&scr, c, r).unwrap();
                    assert_eq!((at.x, at.y), (draw.x.eval(&scr), draw.y.eval(&scr)));
                    let rect = hit.hit_rect(&scr).unwrap();
                    assert!(icon_hit(at, rect.origin()));
                    assert!(!icon_hit(at, Point::new(rect.x - 1, rect.y)));
                    assert!(!icon_hit(at, Point::new(rect.x, rect.y - 1)));
                    let last = Point::new(rect.right() as i32 - 1, rect.bottom() as i32 - 1);
                    assert!(icon_hit(at, last));
                    assert!(!icon_hit(at, Point::new(last.x + 1, last.y)));
                    assert!(!icon_hit(at, Point::new(last.x, last.y + 1)));
                }
            }
        }
        assert_eq!(icon_pos(&s, 0, 1), None);
        assert_eq!(icon_pos(&s, 4, 1), None);
        assert_eq!(icon_pos(&s, 1, 7), None);
    }

    // Only skills of the current page are drawn; frame IconCel (+1 pressed).
    // Covers: specs/ui/panels.md §10 r3
    #[test]
    fn icons_of_current_tab() {
        let s = Screen::R800;
        let v = view(vec![
            entry(6, 1, 2, 3),
            entry(7, 2, 1, 1),
            entry(8, 0, 1, 1),
        ]);
        let mut p = SkillTreePanel::new();
        let ic = p.icons(&s, &v, Point::new(0, 0));
        assert_eq!(ic.len(), 1);
        assert_eq!(ic[0].skill, 6);
        assert_eq!(
            images(std::slice::from_ref(&ic[0].image)),
            vec![(ICON_FILE, 20, 484, 258)]
        );
        p.pressed = Some(6);
        let ic = p.icons(&s, &v, Point::new(0, 0));
        assert_eq!(
            images(std::slice::from_ref(&ic[0].image)),
            vec![(ICON_FILE, 21, 484, 258)]
        );
        p.tab = 2;
        let ic = p.icons(&s, &v, Point::new(0, 0));
        assert_eq!(ic.len(), 1);
        assert_eq!(ic[0].skill, 7);
    }

    // Covers: specs/ui/panels.md §10 r4
    #[test]
    fn level_number_font_position_color() {
        let at = Point::new(484, 258);
        let mut e = entry(6, 1, 2, 3);
        assert_eq!(level_text(&e, at), None);
        e.hard_points = 1;
        e.level = 0;
        let d = level_text(&e, at).unwrap();
        let t = &texts(std::slice::from_ref(&d))[0].clone();
        assert_eq!(
            (t.at.x, t.at.y, t.style.font, t.style.color),
            (532, 270, FONT16, 0)
        );
        assert_eq!(t.text, utf16("0"));
        e.hard_points = 0;
        e.level = 9;
        e.bonus = 2;
        let d = level_text(&e, at).unwrap();
        let t = &texts(std::slice::from_ref(&d))[0].clone();
        assert_eq!((t.at.x, t.style.font, t.style.color), (532, FONT16, 3));
        assert_eq!(t.text, utf16("9"));
        e.level = 10;
        e.bonus = -1;
        let d = level_text(&e, at).unwrap();
        let t = &texts(std::slice::from_ref(&d))[0].clone();
        assert_eq!(
            (t.at.x, t.at.y, t.style.font, t.style.color),
            (528, 270, FONT_FORMAL10, 1)
        );
        assert_eq!(t.text, utf16("10"));
    }

    // Covers: specs/ui/panels.md §10 r2
    #[test]
    fn tab_mouse_down_bounds() {
        for s in [Screen::R640, Screen::R800] {
            let r = s.w - s.sx();
            let b = s.h + s.sy();
            let tab = |x: i32, y: i32| tab_at(&s, Point::new(x, y));
            assert_eq!(tab(r - 88, b - 371), Some(3));
            assert_eq!(tab(r, b - 266), Some(3));
            assert_eq!(tab(r - 89, b - 300), None);
            assert_eq!(tab(r + 1, b - 300), None);
            assert_eq!(tab(r - 40, b - 372), None);
            assert_eq!(tab(r - 40, b - 265), None);
            assert_eq!(tab(r - 40, b - 264), None);
            assert_eq!(tab(r - 40, b - 263), Some(2));
            assert_eq!(tab(r - 40, b - 158), Some(2));
            assert_eq!(tab(r - 40, b - 157), None);
            assert_eq!(tab(r - 40, b - 156), None);
            assert_eq!(tab(r - 40, b - 155), Some(1));
            // Tab-1 bottom ignores sy: last row H − 50.
            assert_eq!(tab(r - 40, s.h - 50), Some(1));
            assert_eq!(tab(r - 40, s.h - 49), None);
        }
        // my ≤ H − 48 gate (at 640 the bands end above it anyway).
        let s = Screen::R800;
        assert_eq!(tab_at(&s, Point::new(700, 553)), None);
        // A change plays the click sound and sends nothing; same tab: nothing.
        let e = env(s);
        let v = view(Vec::new());
        let mut p = SkillTreePanel::new();
        assert_eq!(
            p.mouse_down(&e, &v, Point::new(700, 200)),
            vec![PanelOutput::ClickSound]
        );
        assert_eq!(p.tab, 3);
        assert!(p.mouse_down(&e, &v, Point::new(700, 200)).is_empty());
        assert_eq!(
            p.mouse_down(&e, &v, Point::new(700, 500)),
            vec![PanelOutput::ClickSound]
        );
        assert_eq!(p.tab, 1);
    }

    // §10.5 with free points: press marks, release on it sends 0x3B.
    // Covers: specs/ui/panels.md §10 r5
    #[test]
    fn icon_press_release_sends_add_skill_point() {
        let e = env(Screen::R800);
        let v = view(vec![entry(0x1A, 1, 2, 3)]);
        let mut p = SkillTreePanel::new();
        assert!(p.mouse_down(&e, &v, Point::new(500, 230)).is_empty());
        assert_eq!(p.pressed, Some(0x1A));
        let r = p.mouse_up(&e, &v, Point::new(510, 240));
        assert!(r.consumed);
        assert_eq!(
            r.out,
            vec![PanelOutput::Intent(ClientIntent(vec![0x3B, 0x1A, 0x00]))]
        );
        assert_eq!(p.pressed, None);
        // Release off the icon clears pressed and sends nothing.
        p.mouse_down(&e, &v, Point::new(500, 230));
        let r = p.mouse_up(&e, &v, Point::new(600, 230));
        assert!(r.out.is_empty());
        assert_eq!(p.pressed, None);
        // Release without a press sends nothing.
        assert!(p.mouse_up(&e, &v, Point::new(500, 230)).out.is_empty());
        // Required level / max level checks.
        for (req, below) in [(false, true), (true, false)] {
            let mut v2 = view(vec![entry(0x1A, 1, 2, 3)]);
            v2.skills[0].req_level_ok = req;
            v2.skills[0].below_max = below;
            p.mouse_down(&e, &v2, Point::new(500, 230));
            assert_eq!(p.pressed, Some(0x1A));
            assert!(p.mouse_up(&e, &v2, Point::new(500, 230)).out.is_empty());
        }
        // No free points: no press (the no-points path is OQ4).
        let mut v0 = view(vec![entry(0x1A, 1, 2, 3)]);
        v0.free = 0;
        assert!(p.mouse_down(&e, &v0, Point::new(500, 230)).is_empty());
        assert_eq!(p.pressed, None);
        // Icons of another tab are not hit.
        let v3 = view(vec![entry(0x1A, 2, 2, 3)]);
        p.mouse_down(&e, &v3, Point::new(500, 230));
        assert_eq!(p.pressed, None);
    }

    // Covers: specs/ui/panels.md §10 r8
    #[test]
    fn release_left_half_not_consumed() {
        let e = env(Screen::R800);
        let v = view(Vec::new());
        let mut p = SkillTreePanel::new();
        assert!(!p.mouse_up(&e, &v, Point::new(400, 300)).consumed);
        assert!(!p.mouse_up(&e, &v, Point::new(0, 300)).consumed);
        assert!(p.mouse_up(&e, &v, Point::new(401, 300)).consumed);
        let e = env(Screen::R640);
        assert!(!p.mouse_up(&e, &v, Point::new(320, 300)).consumed);
        assert!(p.mouse_up(&e, &v, Point::new(321, 300)).consumed);
    }

    // §10.3 remap order, literal (unverified: spec OQ4).
    // Covers: specs/ui/panels.md §10 r3
    #[test]
    fn icon_remap_literal_order() {
        let at = Point::new(484, 258);
        let out = Point::new(0, 0);
        let inside = Point::new(500, 230);
        let mut v = view(vec![]);
        let p = SkillTreePanel::new();
        let mut e = entry(6, 1, 2, 3);
        assert_eq!(p.icon_remap(&v, &e, at, out), 0);
        e.learnable = false;
        assert_eq!(p.icon_remap(&v, &e, at, out), 5);
        assert_eq!(p.icon_remap(&v, &e, at, inside), 3);
        e.flags = 2;
        assert_eq!(p.icon_remap(&v, &e, at, inside), 1);
        e.flags = 1;
        e.level = 1;
        assert_eq!(p.icon_remap(&v, &e, at, out), 0);
        v.free = 0;
        assert_eq!(p.icon_remap(&v, &e, at, out), 3);
        e.flags = 2;
        assert_eq!(p.icon_remap(&v, &e, at, out), 3);
        let mut pp = SkillTreePanel::new();
        pp.pressed = Some(6);
        assert_eq!(pp.icon_remap(&v, &e, at, out), 1);
    }
}
