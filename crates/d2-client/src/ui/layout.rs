// Spec: specs/ui/panels.md
//! The panel machine tables (§16): `ui-states.tsv` (§16.1),
//! `panel-layout.tsv` (§16.2) and `npc-menus.tsv` (§16.3), their position
//! grammar and conditions, and the screen layout model (§1).
//!
//! The tables are our own spec data (not game files), embedded from
//! `specs/ui/`. Every parser is strict: an unknown word, a short row or a
//! bad number is an error, never a default.

use super::geom::Rect;

/// `specs/ui/ui-states.tsv`.
pub const UI_STATES_TSV: &str = include_str!("../../../../specs/ui/ui-states.tsv");
/// `specs/ui/panel-layout.tsv`.
pub const PANEL_LAYOUT_TSV: &str = include_str!("../../../../specs/ui/panel-layout.tsv");
/// `specs/ui/npc-menus.tsv`.
pub const NPC_MENUS_TSV: &str = include_str!("../../../../specs/ui/npc-menus.tsv");

/// Number of UI states (§2.1, `[0x0070F9CC]` = 0x26).
pub const UI_STATE_COUNT: usize = 38;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("{table}: bad header")]
    Header { table: &'static str },
    #[error("{table} line {line}: {what}")]
    Row {
        table: &'static str,
        line: usize,
        what: String,
    },
}

fn row_err(table: &'static str, line: usize, what: impl Into<String>) -> LayoutError {
    LayoutError::Row {
        table,
        line,
        what: what.into(),
    }
}

/// Splits a TSV into its data rows (1-based line number, cells); checks the
/// header and the cell count.
fn rows<'a>(
    table: &'static str,
    text: &'a str,
    header: &[&str],
) -> Result<Vec<(usize, Vec<&'a str>)>, LayoutError> {
    let mut lines = text.lines().enumerate();
    let head: Vec<&str> = lines
        .next()
        .map(|(_, l)| l.split('\t').collect())
        .unwrap_or_default();
    if head != header {
        return Err(LayoutError::Header { table });
    }
    let mut out = Vec::new();
    for (i, l) in lines {
        if l.is_empty() {
            continue;
        }
        let cells: Vec<&str> = l.split('\t').collect();
        if cells.len() != header.len() {
            return Err(row_err(table, i + 1, format!("{} cells", cells.len())));
        }
        out.push((i + 1, cells));
    }
    Ok(out)
}

fn parse_int(s: &str) -> Option<i64> {
    if let Some(h) = s.strip_prefix("0x") {
        i64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

// ---------------------------------------------------------------------
// §1 Screen layout model
// ---------------------------------------------------------------------

/// The frame size and resolution mode the panel positions depend on (§1,
/// §Inputs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub w: i32,
    pub h: i32,
    /// D2GFX `ResolutionMode`: 0 (640 × 480) or 2 (800 × 600).
    pub res_mode: u8,
}

impl Screen {
    pub const R640: Screen = Screen {
        w: 640,
        h: 480,
        res_mode: 0,
    };
    pub const R800: Screen = Screen {
        w: 800,
        h: 600,
        res_mode: 2,
    };

    /// The screen of the play frame
    /// ([`crate::rules::camera::FrameSize::play`]): [`Screen::R640`] for
    /// 640 × 480, else [`Screen::R800`].
    pub fn play() -> Screen {
        let f = crate::rules::camera::FrameSize::play();
        if (f.width, f.height) == (Screen::R640.w, Screen::R640.h) {
            Screen::R640
        } else {
            Screen::R800
        }
    }

    /// The whole screen as a rectangle (the clip of a full-screen draw).
    pub fn rect(&self) -> Rect {
        Rect::new(0, 0, self.w as u16, self.h as u16)
    }

    pub fn res2(&self) -> bool {
        self.res_mode == 2
    }

    /// `ScreenShiftX` (§1.1): 80 at resolution mode 2, else 0.
    pub fn sx(&self) -> i32 {
        if self.res2() {
            80
        } else {
            0
        }
    }

    /// `ScreenShiftY` (§1.1): −60 at resolution mode 2, else 0.
    pub fn sy(&self) -> i32 {
        if self.res2() {
            -60
        } else {
            0
        }
    }

    /// Left edge `X0` of the left panel slot (§1.5).
    pub fn left_x0(&self) -> i32 {
        self.sx()
    }

    /// Left edge `X0` of the right panel slot (§1.5).
    pub fn right_x0(&self) -> i32 {
        self.w - self.sx() - 320
    }

    /// The four quad draw positions of a 320 × 432 panel with left edge
    /// `x0` (§1.4), frames `f … f + 3` in this order.
    pub fn quads(&self, x0: i32) -> [(i32, i32); 4] {
        let top = self.h + self.sy() - 224;
        let bottom = self.h + self.sy() - 48;
        [(x0, top), (x0 + 256, top), (x0, bottom), (x0 + 256, bottom)]
    }
}

/// One term of a position expression (§16.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term {
    Int(i32),
    W,
    H,
    /// `W / 2`.
    W2,
    Sx,
    Sy,
}

/// A position expression: signed terms (§16.2 `x`, `y`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr(pub Vec<(i32, Term)>);

impl Expr {
    pub fn parse(s: &str) -> Option<Expr> {
        if s.is_empty() || s.contains(' ') {
            return None;
        }
        let mut terms = Vec::new();
        let mut sign = 1;
        let mut cur = String::new();
        let mut push = |sign: i32, word: &str| -> Option<()> {
            let t = match word {
                "W" => Term::W,
                "H" => Term::H,
                "W2" => Term::W2,
                "sx" => Term::Sx,
                "sy" => Term::Sy,
                _ if !word.is_empty() && word.bytes().all(|b| b.is_ascii_digit()) => {
                    Term::Int(word.parse().ok()?)
                }
                _ => return None,
            };
            terms.push((sign, t));
            Some(())
        };
        for c in s.chars() {
            if c == '+' || c == '-' {
                if !cur.is_empty() {
                    push(sign, &cur)?;
                    cur.clear();
                } else {
                    // No leading sign and no `+-`: terms are joined by one sign.
                    return None;
                }
                sign = if c == '-' { -1 } else { 1 };
            } else {
                cur.push(c);
            }
        }
        push(sign, &cur)?;
        Some(Expr(terms))
    }

    pub fn eval(&self, s: &Screen) -> i32 {
        self.0
            .iter()
            .map(|&(sign, t)| {
                sign * match t {
                    Term::Int(v) => v,
                    Term::W => s.w,
                    Term::H => s.h,
                    Term::W2 => s.w / 2,
                    Term::Sx => s.sx(),
                    Term::Sy => s.sy(),
                }
            })
            .sum()
    }
}

// ---------------------------------------------------------------------
// §16.1 ui-states.tsv
// ---------------------------------------------------------------------

/// Slot kind of a UI state (§4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Right,
    Left,
    Full,
    Anvil,
    None,
}

/// The gate action `C[i][ui]` (§3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conflict {
    /// 0: nothing.
    Ignore,
    /// 1: close `i`, continue.
    Close,
    /// 2: refuse.
    Refuse,
    /// 3: refuse; a fatal error when the request is ui 0.
    RefuseFatal0,
    /// 4: end an active NPC interaction, continue.
    EndNpc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiStateRow {
    pub id: u8,
    pub name: String,
    pub flag: u32,
    pub slot: SlotKind,
    pub exp_only: bool,
    /// `C[id][j]` for j = 0 … 37.
    pub conflicts: [Conflict; UI_STATE_COUNT],
}

/// The 38 rows of `ui-states.tsv`, index = id.
pub fn ui_states() -> Result<Vec<UiStateRow>, LayoutError> {
    parse_ui_states(UI_STATES_TSV)
}

pub fn parse_ui_states(text: &str) -> Result<Vec<UiStateRow>, LayoutError> {
    const T: &str = "ui-states.tsv";
    let mut out = Vec::new();
    for (line, c) in rows(
        T,
        text,
        &["id", "name", "flag", "slot", "exp_only", "conflicts"],
    )? {
        let id = c[0].parse::<u8>().map_err(|_| row_err(T, line, "id"))?;
        if usize::from(id) != out.len() {
            return Err(row_err(T, line, "ids not 0, 1, 2, …"));
        }
        let flag = parse_int(c[2])
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| row_err(T, line, "flag"))?;
        let slot = match c[3] {
            "right" => SlotKind::Right,
            "left" => SlotKind::Left,
            "full" => SlotKind::Full,
            "anvil" => SlotKind::Anvil,
            "none" => SlotKind::None,
            _ => return Err(row_err(T, line, "slot")),
        };
        let exp_only = match c[4] {
            "0" => false,
            "1" => true,
            _ => return Err(row_err(T, line, "exp_only")),
        };
        if c[5].len() != UI_STATE_COUNT {
            return Err(row_err(T, line, "conflicts length"));
        }
        let mut conflicts = [Conflict::Ignore; UI_STATE_COUNT];
        for (j, b) in c[5].bytes().enumerate() {
            conflicts[j] = match b {
                b'0' => Conflict::Ignore,
                b'1' => Conflict::Close,
                b'2' => Conflict::Refuse,
                b'3' => Conflict::RefuseFatal0,
                b'4' => Conflict::EndNpc,
                _ => return Err(row_err(T, line, "conflict digit")),
            };
        }
        out.push(UiStateRow {
            id,
            name: c[1].to_string(),
            flag,
            slot,
            exp_only,
            conflicts,
        });
    }
    if out.len() != UI_STATE_COUNT {
        return Err(row_err(T, out.len() + 1, "not 38 states"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// §16.2 panel-layout.tsv
// ---------------------------------------------------------------------

/// The `panel` column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelKey {
    Ui(u8),
    Border,
    CtrlPnl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Draw,
    Text,
    Hit,
}

/// The `frame` column (§16.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameSpec {
    None,
    /// A frame index (`draw`) or a string id (`text`).
    Index(u32),
    /// `a/b`: string alternatives (`text`).
    Alt(u32, u32),
    /// A word the panel resolves (`iconcel`, `tabstate`, `rowstate`,
    /// `value`, `level`, `tabstr`).
    Word(String),
}

/// The `color` column (§16.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorSpec {
    None,
    Index(u16),
    /// `cmp` (§8.7) or a state word (`tabstate`, `rowstate`).
    Word(String),
}

/// One `cond` word (§16.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cond {
    Always,
    Res2,
    ResNot2,
    ModeL,
    ModeR,
    Exp,
    Classic,
    Released,
    Pressed,
    StatPts,
    Tab(u8),
    SkillHere,
    Tab1Bottom,
    TabShown,
    TabVisible,
    RowUsed,
    RowKnown,
    Centered,
    SplitLf,
    HalfCentered,
}

impl Cond {
    fn parse(s: &str) -> Option<Cond> {
        Some(match s {
            "always" => Cond::Always,
            "res2" => Cond::Res2,
            "res_not2" => Cond::ResNot2,
            "mode_l" => Cond::ModeL,
            "mode_r" => Cond::ModeR,
            "exp" => Cond::Exp,
            "classic" => Cond::Classic,
            "released" => Cond::Released,
            "pressed" => Cond::Pressed,
            "statpts" => Cond::StatPts,
            "tab1" => Cond::Tab(1),
            "tab2" => Cond::Tab(2),
            "tab3" => Cond::Tab(3),
            "skill_here" => Cond::SkillHere,
            "tab1_bottom" => Cond::Tab1Bottom,
            "tab_shown" => Cond::TabShown,
            "tab_visible" => Cond::TabVisible,
            "row_used" => Cond::RowUsed,
            "row_known" => Cond::RowKnown,
            "centered" => Cond::Centered,
            "split_lf" => Cond::SplitLf,
            "half_centered" => Cond::HalfCentered,
            _ => return None,
        })
    }

    /// Words that say how text is placed, not whether a row applies.
    pub fn is_format(&self) -> bool {
        matches!(
            self,
            Cond::Always | Cond::Centered | Cond::SplitLf | Cond::HalfCentered | Cond::Tab1Bottom
        )
    }
}

/// The panel state a row's conditions are tested against. Panel-specific
/// words (`statpts`, `skill_here`, `row_used`, …) are answered by the
/// panel through `extra`.
pub struct CondEnv<'a> {
    pub screen: Screen,
    /// Screen open mode 0–3 (§4.2).
    pub open_mode: u8,
    /// Expansion game with the expansion installed (§8.1).
    pub exp: bool,
    pub pressed: bool,
    pub extra: &'a dyn Fn(Cond) -> bool,
}

impl CondEnv<'_> {
    pub fn holds(&self, c: Cond) -> bool {
        match c {
            c if c.is_format() => true,
            Cond::Res2 => self.screen.res2(),
            Cond::ResNot2 => !self.screen.res2(),
            Cond::ModeL => matches!(self.open_mode, 2 | 3),
            Cond::ModeR => matches!(self.open_mode, 1 | 3),
            Cond::Exp => self.exp,
            Cond::Classic => !self.exp,
            Cond::Released => !self.pressed,
            Cond::Pressed => self.pressed,
            other => (self.extra)(other),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutRow {
    pub line: usize,
    pub panel: PanelKey,
    pub item: String,
    pub kind: RowKind,
    /// Path under `data\global\ui\` without extension (`draw`).
    pub file: Option<String>,
    pub frame: FrameSpec,
    pub x: Expr,
    pub y: Expr,
    pub w: Option<i32>,
    pub h: Option<i32>,
    pub color: ColorSpec,
    pub font: Option<u16>,
    pub cond: Vec<Cond>,
    pub addr: u32,
}

impl LayoutRow {
    pub fn has(&self, c: Cond) -> bool {
        self.cond.contains(&c)
    }

    pub fn applies(&self, env: &CondEnv) -> bool {
        self.cond.iter().all(|&c| env.holds(c))
    }

    /// The `hit` rectangle (§16.2): `x … x + w − 1`, `y … y + h − 1`;
    /// with `tab1_bottom` the bottom row is `H − 50`.
    pub fn hit_rect(&self, s: &Screen) -> Option<Rect> {
        if self.kind != RowKind::Hit {
            return None;
        }
        let x = self.x.eval(s);
        let y = self.y.eval(s);
        let w = self.w?;
        let h = if self.has(Cond::Tab1Bottom) {
            s.h - 50 - y + 1
        } else {
            self.h?
        };
        Some(Rect::new(
            x,
            y,
            u16::try_from(w).ok()?,
            u16::try_from(h).ok()?,
        ))
    }
}

/// All rows of `panel-layout.tsv`, in file order.
pub fn panel_layout() -> Result<Vec<LayoutRow>, LayoutError> {
    parse_panel_layout(PANEL_LAYOUT_TSV)
}

pub fn parse_panel_layout(text: &str) -> Result<Vec<LayoutRow>, LayoutError> {
    const T: &str = "panel-layout.tsv";
    let header = [
        "panel", "item", "kind", "file", "frame", "x", "y", "w", "h", "color", "font", "cond",
        "addr",
    ];
    let mut out = Vec::new();
    for (line, c) in rows(T, text, &header)? {
        let e = |what: &str| row_err(T, line, what);
        let panel = match c[0] {
            "border" => PanelKey::Border,
            "ctrlpnl" => PanelKey::CtrlPnl,
            v => PanelKey::Ui(
                v.parse::<u8>()
                    .ok()
                    .filter(|&u| usize::from(u) < UI_STATE_COUNT)
                    .ok_or_else(|| e("panel"))?,
            ),
        };
        let kind = match c[2] {
            "draw" => RowKind::Draw,
            "text" => RowKind::Text,
            "hit" => RowKind::Hit,
            _ => return Err(e("kind")),
        };
        let file = match (kind, c[3]) {
            (RowKind::Draw, "-") => return Err(e("draw without file")),
            (RowKind::Draw, f) => Some(f.to_string()),
            (_, "-") => None,
            _ => return Err(e("file on a non-draw row")),
        };
        let frame = match c[4] {
            "-" => FrameSpec::None,
            v if v.bytes().all(|b| b.is_ascii_digit()) => {
                FrameSpec::Index(v.parse().map_err(|_| e("frame"))?)
            }
            v if v.contains('/') => {
                let (a, b) = v.split_once('/').ok_or_else(|| e("frame"))?;
                FrameSpec::Alt(
                    a.parse().map_err(|_| e("frame"))?,
                    b.parse().map_err(|_| e("frame"))?,
                )
            }
            v @ ("iconcel" | "tabstate" | "rowstate" | "value" | "level" | "tabstr") => {
                FrameSpec::Word(v.to_string())
            }
            _ => return Err(e("frame")),
        };
        let x = Expr::parse(c[5]).ok_or_else(|| e("x"))?;
        let y = Expr::parse(c[6]).ok_or_else(|| e("y"))?;
        let opt_int = |v: &str, what: &str| -> Result<Option<i32>, LayoutError> {
            match v {
                "-" => Ok(None),
                v => v.parse().map(Some).map_err(|_| e(what)),
            }
        };
        let w = opt_int(c[7], "w")?;
        let h = opt_int(c[8], "h")?;
        let color = match c[9] {
            "-" => ColorSpec::None,
            v if v.bytes().all(|b| b.is_ascii_digit()) => {
                ColorSpec::Index(v.parse().map_err(|_| e("color"))?)
            }
            v @ ("cmp" | "tabstate" | "rowstate") => ColorSpec::Word(v.to_string()),
            _ => return Err(e("color")),
        };
        let font = match c[10] {
            "-" => None,
            v => Some(v.parse().map_err(|_| e("font"))?),
        };
        let cond = c[11]
            .split(',')
            .map(|w| Cond::parse(w).ok_or_else(|| e("cond")))
            .collect::<Result<Vec<_>, _>>()?;
        let addr = parse_int(c[12])
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| e("addr"))?;
        if kind == RowKind::Hit
            && (w.is_none() || (h.is_none() && !cond.contains(&Cond::Tab1Bottom)))
        {
            return Err(e("hit without size"));
        }
        out.push(LayoutRow {
            line,
            panel,
            item: c[1].to_string(),
            kind,
            file,
            frame,
            x,
            y,
            w,
            h,
            color,
            font,
            cond,
            addr,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------
// §16.3 npc-menus.tsv
// ---------------------------------------------------------------------

/// The option kinds of §14.1 (handler per kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionKind {
    Talk,
    Trade,
    Gamble,
    Hire,
    TravelWest,
    SailWest,
    Identify,
    Resurrect,
    /// d2rs-own, unverified (REC-145): Charsi's runtime insert, never parsed.
    Imbue,
}

impl OptionKind {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "talk" => OptionKind::Talk,
            "trade" => OptionKind::Trade,
            "gamble" => OptionKind::Gamble,
            "hire" => OptionKind::Hire,
            "travel_west" => OptionKind::TravelWest,
            "sail_west" => OptionKind::SailWest,
            "identify" => OptionKind::Identify,
            "resurrect" => OptionKind::Resurrect,
            _ => return None,
        })
    }
}

/// One option slot: string id and handler kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuOption {
    pub string: u16,
    pub kind: OptionKind,
}

/// One static record of the option table `0x00726C48` (§14.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcMenuRecord {
    pub record: u8,
    pub npc: u32,
    /// Options + 1 for the trailing cancel.
    pub count: u32,
    /// Up to 5 slots; `None` = cleared.
    pub options: [Option<MenuOption>; 5],
    pub flag: u8,
}

/// The 48 static records of `npc-menus.tsv`.
pub fn npc_menus() -> Result<Vec<NpcMenuRecord>, LayoutError> {
    parse_npc_menus(NPC_MENUS_TSV)
}

pub fn parse_npc_menus(text: &str) -> Result<Vec<NpcMenuRecord>, LayoutError> {
    const T: &str = "npc-menus.tsv";
    let mut out = Vec::new();
    for (line, c) in rows(T, text, &["record", "npc", "count", "options", "flag"])? {
        let e = |what: &str| row_err(T, line, what);
        let record: u8 = c[0].parse().map_err(|_| e("record"))?;
        if usize::from(record) != out.len() {
            return Err(e("records not 0, 1, 2, …"));
        }
        let mut options = [None; 5];
        for (i, o) in c[3].split(',').enumerate() {
            let (s, k) = o.split_once(':').ok_or_else(|| e("option"))?;
            let slot = options.get_mut(i).ok_or_else(|| e("more than 5 options"))?;
            *slot = Some(MenuOption {
                string: s.parse().map_err(|_| e("option string"))?,
                kind: OptionKind::parse(k).ok_or_else(|| e("option kind"))?,
            });
        }
        out.push(NpcMenuRecord {
            record,
            npc: c[1].parse().map_err(|_| e("npc"))?,
            count: c[2].parse().map_err(|_| e("count"))?,
            options,
            flag: c[4].parse().map_err(|_| e("flag"))?,
        });
    }
    if out.len() != 48 {
        return Err(row_err(T, out.len() + 1, "not 48 records"));
    }
    Ok(out)
}
