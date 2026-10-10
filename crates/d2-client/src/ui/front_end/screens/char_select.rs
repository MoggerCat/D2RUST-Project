// Spec: specs/ui/frontend-menus.md (§F2.1–§F2.7, §F2.9)
//! Character select. `Model` is the list, selection and button logic (plain
//! Rust, tested on synthetic save folders); `CharSelect` is the [`Screen`]
//! that draws it and fires the flow triggers.
//!
//! A screen's controls are built only on entry, so every change the player
//! sees (selection, scrolling, a pop-up) re-enters the screen through
//! `Trigger::GameExit` (`flow::next` sends it to character select from any
//! screen) with `refresh` set; `build` then keeps the model instead of
//! rescanning. PROVISIONAL, REC-180 (1): a dedicated refresh trigger would
//! be cleaner, but it needs a shared `flow.rs` edit.
//!
//! Preview fills are `// d2rs-own, unverified`; open points are PROVISIONAL
//! under REC-180.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use d2_formats::d2s::{checksum, status, HEADER_SIZE, MAGIC, MAX_FILE, VERSION, VERSION_MIN};

use crate::ui::front_end::control::{vk, Action, Control, ControlKind, TextRow};
use crate::ui::front_end::flow::Trigger;
use crate::ui::front_end::screen::{FrontCtx, Screen};
use crate::ui::front_end::{Registry, SaveFolder, CHAR_SELECT};

/// Visible slots: 2 columns × 4 rows (§F2.4 r1).
pub const SLOTS: usize = 8;
/// Double-click window (§F2.5 r2).
pub const DOUBLE_CLICK_MS: u64 = 500;
/// String ids used by the screen.
pub mod strings {
    pub const EXIT: u32 = 5101;
    pub const OK: u32 = 5102;
    pub const DELETE: u32 = 5272;
    pub const CREATE_NEW: u32 = 10832;
    pub const CHARACTER: u32 = 21796;
    pub const CONVERT_TO: u32 = 22732;
    pub const EXPANSION_LABEL: u32 = 22730;
    pub const EXPANSION_CHARACTER: u32 = 22731;
    pub const DEAD_HARDCORE: u32 = 5304;
    pub const DELETE_QUESTION: u32 = 5163;
    pub const NO: u32 = 5167;
    pub const YES: u32 = 5166;
    pub const CONVERT_WARNING: u32 = 22734;
    pub const CONVERT_FAILED: u32 = 21872;
}
/// `Action::Custom` ids (the descriptor ids of §F2.9 where they exist).
pub mod ids {
    pub const SLOT_TEXT: u32 = 0x84; // 0x84..=0x8B
    pub const SLOT_FIGURE: u32 = 0x8C; // 0x8C..=0x93
    pub const OK: u32 = 0xA2;
    pub const CONVERT: u32 = 0xA5;
    pub const DELETE: u32 = 0xA6;
    pub const POPUP_NO: u32 = 0xD7;
    pub const POPUP_YES: u32 = 0xD8;
    pub const KEY_HOME: u32 = 0x100;
    pub const KEY_END: u32 = 0x101;
    pub const KEY_LEFT: u32 = 0x102;
    pub const KEY_RIGHT: u32 = 0x103;
    pub const KEY_UP: u32 = 0x104;
    pub const KEY_DOWN: u32 = 0x105;
}
/// Virtual keys of the selection box's handler (§F2.5 r3).
mod key {
    pub const END: u16 = 35;
    pub const HOME: u16 = 36;
    pub const LEFT: u16 = 37;
    pub const UP: u16 = 38;
    pub const RIGHT: u16 = 39;
    pub const DOWN: u16 = 40;
}

/// One save in the list (§F2.2 r3–r4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The file name up to its first `.` (not the header's name field).
    pub name: String,
    pub class: u8,
    pub level: u8,
    pub status: u16,
    pub components: [u8; 11],
    pub colours: [u8; 11],
    /// File last-write time, nanoseconds since the epoch.
    pub mtime: u128,
}

impl Entry {
    pub fn hardcore(&self) -> bool {
        self.status & status::HARDCORE != 0
    }
    pub fn dead(&self) -> bool {
        self.status & (status::HARDCORE | status::DEAD) == (status::HARDCORE | status::DEAD)
    }
    pub fn expansion(&self) -> bool {
        self.status & status::EXPANSION != 0
    }
    /// Progression p, status bits 8–12.
    pub fn progression(&self) -> u8 {
        status::progression(self.status)
    }
    /// Difficulties open: 1 Normal, 2 + Nightmare, 3 + Hell (§F2.6 r3,
    /// §F2.8 r1; the thresholds of `formats/d2s.md` §2.2 r5.4).
    pub fn difficulties_open(&self) -> u8 {
        let p = self.progression();
        let nightmare = if self.expansion() { p >= 5 } else { p >= 4 };
        let hell = (!self.expansion() && p >= 8) || p >= 10;
        1 + u8::from(nightmare) + u8::from(nightmare && hell)
    }
    /// Paper doll `(class', mode)` (§F2.2 r6): softcore mode 5; hardcore
    /// alive mode 1 (expansion front end); hardcore dead class' 8 / 9, mode 5.
    pub fn paper_doll(&self, expansion_front_end: bool) -> (u8, u8) {
        if self.dead() {
            (
                if matches!(self.class, 0 | 1 | 6) {
                    8
                } else {
                    9
                },
                5,
            )
        } else if self.hardcore() && expansion_front_end {
            (self.class, 1)
        } else {
            (self.class, 5)
        }
    }
}

/// The class name (d2rs-own English; PROVISIONAL REC-180 (4)).
pub fn class_name(class: u8) -> &'static str {
    const NAMES: [&str; 7] = [
        "Amazon",
        "Sorceress",
        "Necromancer",
        "Paladin",
        "Barbarian",
        "Druid",
        "Assassin",
    ];
    NAMES.get(usize::from(class)).copied().unwrap_or("")
}

/// The title prefix (`world/quests-act1-rest.md` "Character title").
pub fn title_prefix(class: u8, p: u8, hardcore: bool, expansion: bool) -> &'static str {
    let mut t = if expansion {
        match p {
            0..=4 => 0,
            5..=9 => 1,
            10..=14 => 2,
            _ => 3,
        }
    } else {
        match p {
            0..=3 => 0,
            4..=7 => 1,
            8..=11 => 2,
            _ => 3,
        }
    };
    if hardcore && t != 0 {
        t += 3;
    }
    let female = matches!(class, 0 | 1 | 6 | 9);
    match (expansion, female, t) {
        (false, true, 1) => "Dame ",
        (false, false, 1) => "Sir ",
        (false, true, 2) => "Lady ",
        (false, false, 2) => "Lord ",
        (false, true, 3) => "Baroness ",
        (false, false, 3) => "Baron ",
        (false, true, 4) => "Countess ",
        (false, false, 4) => "Count ",
        (false, true, 5) => "Duchess ",
        (false, false, 5) => "Duke ",
        (false, true, 6) => "Queen ",
        (false, false, 6) => "King ",
        (true, _, 1) => "Slayer ",
        (true, _, 2) => "Champion ",
        (true, _, 3) => "Matriarch/Patriarch ",
        (true, _, 4) => "Destroyer ",
        (true, _, 5) => "Conqueror ",
        (true, _, 6) => "Guardian ",
        _ => "",
    }
}

/// The expansion tier 3 title differs by sex.
fn title(class: u8, p: u8, hardcore: bool, expansion: bool) -> String {
    let s = title_prefix(class, p, hardcore, expansion);
    if s.starts_with("Matriarch") {
        let f = if matches!(class, 0 | 1 | 6 | 9) {
            "Matriarch "
        } else {
            "Patriarch "
        };
        return f.to_string();
    }
    s.to_string()
}

/// The header read of `0x0043C8A0` (§F2.2 r3); `None` = rejected.
pub fn read_header(b: &[u8], name: &str, mtime: u128) -> Option<Entry> {
    if b.len() < 8 || u32::from_le_bytes(b[0..4].try_into().ok()?) != MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(b[4..8].try_into().ok()?);
    if version >= VERSION_MIN {
        if b.len() < HEADER_SIZE || version > VERSION {
            return None;
        }
        let mut components = [0u8; 11];
        let mut colours = [0u8; 11];
        components.copy_from_slice(&b[0x88..0x88 + 11]);
        colours.copy_from_slice(&b[0x98..0x98 + 11]);
        if components[0] == 0 {
            components = DEFAULT_COMPONENTS;
        }
        Some(Entry {
            name: name.to_string(),
            class: b[0x28],
            level: b[0x2B],
            status: u16::from_le_bytes([b[0x24], b[0x25]]),
            components,
            colours,
            mtime,
        })
    } else {
        if b.len() < 0x82 || version < 0x47 {
            return None;
        }
        // d2rs-own, unverified: the legacy level offset is not in
        // `formats/d2s-legacy.md`; level 1 is shown (REC-180 (3)).
        Some(Entry {
            name: name.to_string(),
            class: b[0x22],
            level: 1,
            status: u16::from_le_bytes([b[0x18], b[0x19]]),
            components: DEFAULT_COMPONENTS,
            colours: [0xFF; 11],
            mtime,
        })
    }
}

/// The default set at `0x0070CCC8` (not in the specs): d2rs-own, unverified
/// (REC-180 (3)); the standard unequipped set.
const DEFAULT_COMPONENTS: [u8; 11] = [0xFF; 11];

fn mtime_of(p: &Path) -> u128 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

/// Scan `dir` (§F2.2, §F2.3): `*.d2s`, name up to the first `.`, filtered,
/// header-checked, sorted newest first (expansion) or in scan order
/// (classic). Scan order is the sorted file-name order (d2rs: the file
/// system's order is not stable).
pub fn scan(dir: &Path, expansion_front_end: bool) -> Vec<Entry> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(String, PathBuf)> = rd
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let f = e.file_name().to_string_lossy().into_owned();
            f.to_ascii_lowercase()
                .ends_with(".d2s")
                .then(|| (f, e.path()))
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for (file, path) in files {
        let stem = file.split('.').next().unwrap_or("").to_string();
        let n = stem.chars().count();
        if !(2..=15).contains(&n) || stem.ends_with('-') || stem.starts_with('_') {
            continue;
        }
        // The read goes to `<folder><stem>.d2s`, not the listed file.
        let read_path = dir.join(format!("{stem}.d2s"));
        let Ok(mut bytes) = fs::read(&read_path) else {
            continue;
        };
        bytes.truncate(MAX_FILE);
        if let Some(e) = read_header(&bytes, &stem, mtime_of(&path)) {
            out.push(e);
        }
    }
    if expansion_front_end {
        out.sort_by_key(|e| std::cmp::Reverse(e.mtime)); // stable: ties keep scan order
    }
    out
}

/// `<folder>*.d2s` holds a usable character (`0x00430BC0`).
#[derive(Clone, Debug)]
pub struct DirSaves {
    pub dir: PathBuf,
    pub expansion: bool,
}

impl SaveFolder for DirSaves {
    fn has_saves(&self) -> bool {
        !scan(&self.dir, self.expansion).is_empty()
    }
}

/// A pop-up over the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    None,
    /// §F2.7 r1.
    Delete,
    /// §F2.7 r2.
    Convert,
    /// A message box with OK (string id).
    Message(u32),
}

/// What OK did (§F2.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OkResult {
    Nothing,
    /// Message box 5304 up.
    Message(u32),
    /// Difficulty box: `difficulties_open` > 1.
    Difficulty(Entry),
    /// The game starts at once at Normal.
    Start(Entry),
}

/// Which buttons are on (§F2.5 r6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub ok: bool,
    pub delete: bool,
    pub convert: bool,
    pub create: bool,
}

/// The screen state: list, selection, scrolling, pop-up.
#[derive(Clone, Debug)]
pub struct Model {
    pub dir: PathBuf,
    pub expansion: bool,
    pub entries: Vec<Entry>,
    /// `[0x0070CC00]`; −1 none.
    pub sel: i32,
    /// `[0x00779DC8]`.
    pub first: usize,
    /// Scroll bar thumb position (rows of 2).
    pub scroll: i32,
    pub popup: Popup,
    last_click: Option<(u64, i32)>,
}

impl Model {
    pub fn new(dir: PathBuf, expansion: bool) -> Self {
        let mut m = Self {
            dir,
            expansion,
            entries: Vec::new(),
            sel: -1,
            first: 0,
            scroll: 0,
            popup: Popup::None,
            last_click: None,
        };
        m.rescan();
        m
    }

    /// Scan again; `sel` = `first` = 0 (§F2.5 r1, §F2.7 r1).
    pub fn rescan(&mut self) {
        self.entries = scan(&self.dir, self.expansion);
        self.sel = if self.entries.is_empty() { -1 } else { 0 };
        self.first = 0;
        self.scroll = 0;
        self.popup = Popup::None;
        self.last_click = None;
    }

    fn n(&self) -> i32 {
        self.entries.len() as i32
    }

    pub fn selected(&self) -> Option<&Entry> {
        usize::try_from(self.sel)
            .ok()
            .and_then(|i| self.entries.get(i))
    }

    pub fn buttons(&self) -> Buttons {
        let e = self.selected();
        Buttons {
            ok: e.is_some(),
            delete: e.is_some(),
            convert: self.expansion && e.is_some_and(|e| !e.expansion()),
            create: true,
        }
    }

    /// Scroll bar range ⌈(n − 8) / 2⌉ when n > 8 (§F2.5 r4).
    pub fn scroll_range(&self) -> Option<i32> {
        (self.n() > SLOTS as i32).then(|| (self.n() - SLOTS as i32 + 1) / 2)
    }

    /// The entry in visible slot `i` (§F2.4 r2).
    pub fn slot(&self, i: usize) -> Option<&Entry> {
        if i >= SLOTS {
            return None;
        }
        self.entries.get(self.first + i)
    }

    fn keep_visible(&mut self) {
        let sel = self.sel.max(0) as usize;
        if sel < self.first {
            self.first = sel;
        } else if sel >= self.first + SLOTS {
            self.first = sel + 1 - SLOTS;
        }
    }

    /// A key of the selection box (§F2.5 r3).
    pub fn key(&mut self, vk: u16) {
        let n = self.n();
        if n == 0 || self.popup != Popup::None {
            return;
        }
        let eight = SLOTS as i32;
        match vk {
            key::HOME => {
                self.sel = 0;
                self.first = 0;
            }
            key::END => {
                self.sel = n - 1;
                if n > eight {
                    self.first = (n - eight) as usize;
                }
            }
            key::LEFT => {
                if self.sel & 1 == 1 {
                    self.sel -= 1;
                    self.keep_visible();
                }
            }
            key::RIGHT => {
                if self.sel & 1 == 0 && self.sel < n - 1 {
                    self.sel += 1;
                    self.keep_visible();
                }
            }
            key::UP => {
                if self.sel >= 2 {
                    self.sel -= 2;
                    if (self.sel as usize) < self.first {
                        self.first = (self.sel & !1) as usize;
                        self.scroll -= 1;
                    }
                }
            }
            key::DOWN if self.sel + 2 < n => {
                self.sel += 2;
                if self.sel as usize >= self.first + SLOTS {
                    let f = if self.sel & 1 == 1 {
                        self.sel - 7
                    } else {
                        self.sel - 6
                    };
                    self.first = f.max(0) as usize;
                    self.scroll += 1;
                }
            }
            _ => {}
        }
    }

    /// The scroll bar moved by `d` (§F2.5 r4): `first += 2d`, clamped to
    /// `[0, n − 1]`; `sel` unchanged.
    pub fn scroll_by(&mut self, d: i32) {
        let f = (self.first as i32 + 2 * d).clamp(0, (self.n() - 1).max(0));
        self.first = f as usize;
        self.scroll += d;
    }

    /// A click on visible slot `i` at `now_ms` (§F2.5 r2). Returns `true`
    /// for a double click (the caller runs OK).
    pub fn click(&mut self, i: usize, now_ms: u64) -> bool {
        if self.popup != Popup::None {
            return false;
        }
        let k = (self.first + i) as i32;
        if k > self.n() {
            return false;
        }
        self.sel = if self.n() < 1 { -1 } else { k };
        let double = self
            .last_click
            .is_some_and(|(t, s)| s == self.sel && now_ms.saturating_sub(t) <= DOUBLE_CLICK_MS);
        self.last_click = Some((now_ms, self.sel));
        double
    }

    /// OK / Enter (§F2.6).
    pub fn ok(&mut self) -> OkResult {
        if self.popup != Popup::None {
            return OkResult::Nothing;
        }
        let Some(e) = self.selected().cloned() else {
            return OkResult::Nothing;
        };
        if e.expansion() && !self.expansion {
            return OkResult::Nothing;
        }
        if e.dead() {
            self.popup = Popup::Message(strings::DEAD_HARDCORE);
            return OkResult::Message(strings::DEAD_HARDCORE);
        }
        if e.difficulties_open() > 1 {
            OkResult::Difficulty(e)
        } else {
            OkResult::Start(e)
        }
    }

    /// Delete YES (§F2.7 r1): every non-directory `<folder><name>.*`,
    /// stopping at the first failed delete; then rescan.
    pub fn delete_selected(&mut self) {
        if let Some(name) = self.selected().map(|e| e.name.clone()) {
            let prefix = format!("{}.", name.to_ascii_lowercase());
            loop {
                let Ok(rd) = fs::read_dir(&self.dir) else {
                    break;
                };
                let hit = rd.filter_map(Result::ok).find(|e| {
                    e.file_type().is_ok_and(|t| t.is_file())
                        && e.file_name()
                            .to_string_lossy()
                            .to_ascii_lowercase()
                            .starts_with(&prefix)
                });
                match hit {
                    Some(f) if fs::remove_file(f.path()).is_ok() => {}
                    _ => break,
                }
            }
        }
        self.rescan();
    }

    /// Convert YES (§F2.7 r2, `formats/d2s.md` §2.7 r2). A failure shows
    /// message 21872 after a rescan.
    pub fn convert_selected(&mut self) {
        let Some(e) = self.selected().cloned() else {
            self.popup = Popup::None;
            return;
        };
        if e.expansion() {
            self.popup = Popup::None;
            return;
        }
        let path = self.dir.join(format!("{}.d2s", e.name));
        let done = (|| -> Option<()> {
            let mut b = fs::read(&path).ok()?;
            if b.len() > MAX_FILE || b.len() < HEADER_SIZE {
                return None;
            }
            if u32::from_le_bytes(b[4..8].try_into().ok()?) < VERSION_MIN {
                return None;
            }
            let s = u16::from_le_bytes([b[0x24], b[0x25]]) | status::EXPANSION;
            b[0x24..0x26].copy_from_slice(&s.to_le_bytes());
            b[0x0C..0x10].fill(0);
            let c = checksum(&b);
            b[0x0C..0x10].copy_from_slice(&c.to_le_bytes());
            fs::write(&path, b).ok()
        })();
        if done.is_some() {
            let i = self.sel as usize;
            self.entries[i].status |= status::EXPANSION;
            self.popup = Popup::None;
        } else {
            self.rescan();
            self.popup = Popup::Message(strings::CONVERT_FAILED);
        }
    }

    /// The slot's text lines (§F2.4 r4): title + name, level line, and the
    /// expansion string id when status & 0x20. Colour: 1 (red) hardcore,
    /// else 4 (gold) for the name line.
    pub fn slot_lines(&self, i: usize) -> Option<SlotLines> {
        let e = self.slot(i)?;
        Some(SlotLines {
            name: format!(
                "{}{}",
                title(e.class, e.progression(), e.hardcore(), e.expansion()),
                e.name
            ),
            name_colour: if e.hardcore() { 1 } else { 4 },
            level: format!("Level {} {}", e.level, class_name(e.class)),
            expansion: e.expansion().then_some(strings::EXPANSION_CHARACTER),
        })
    }
}

/// What a slot shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotLines {
    pub name: String,
    pub name_colour: u8,
    pub level: String,
    /// String id 22731 in colour 2 (green).
    pub expansion: Option<u32>,
}

/// The character the player chose, for the host and the difficulty box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub entry: Entry,
    /// Difficulties open (1 Normal, 2 + Nightmare, 3 + Hell).
    pub difficulties_open: u8,
}

/// Where the screen leaves the chosen character.
pub type SelectionHandle = Arc<Mutex<Option<Selection>>>;

/// The screen.
pub struct CharSelect {
    dir: Option<PathBuf>,
    model: Option<Model>,
    refresh: bool,
    handle: SelectionHandle,
}

impl CharSelect {
    pub fn new(dir: Option<PathBuf>, handle: SelectionHandle) -> Self {
        Self {
            dir,
            model: None,
            refresh: false,
            handle,
        }
    }

    pub fn model(&self) -> Option<&Model> {
        self.model.as_ref()
    }

    fn again(&mut self) -> Option<Trigger> {
        self.refresh = true;
        Some(Trigger::GameExit)
    }

    fn run_ok(&mut self, ctx: &mut FrontCtx) -> Option<Trigger> {
        let m = self.model.as_mut()?;
        match m.ok() {
            OkResult::Nothing => None,
            OkResult::Message(_) => self.again(),
            OkResult::Difficulty(e) | OkResult::Start(e) => {
                ctx.flow.difficulties_open = e.difficulties_open();
                *self.handle.lock().ok()? = Some(Selection {
                    difficulties_open: e.difficulties_open(),
                    entry: e,
                });
                Some(Trigger::Ok)
            }
        }
    }
}

const BG: &str = r"CharSelect\characterselectscreenEXP";
const MEDIUM: &str = r"FrontEnd\MediumButtonBlank";
const TALL: &str = r"CharSelect\TallButtonBlank";
const SEL_BOX: &str = r"CharSelect\charselectbox";
const POPUP: &str = r"FrontEnd\PopUpOKCancel2";
const CANCEL: &str = r"FrontEnd\CancelButtonBlank";
const SCROLL: &str = r"FrontEnd\joingamescrollbars";

fn slot_x(i: usize) -> i32 {
    if i & 1 == 0 {
        37
    } else {
        309
    }
}

fn slot_y(i: usize) -> i32 {
    178 + 93 * (i / 2) as i32
}

/// Fonts of the text descriptors (§F1.1 r6): Font42 (`0x007089AC`),
/// Font16 (`0x007089C4`).
const FONT42: u16 = 3;
const FONT16: u16 = 1;

/// A pop-up text (0xD9 / 0xDD, 268, 320, 264 × 120). PROVISIONAL (§F2.7
/// gives no font for these descriptors): as the create pop-up 208 (§F3.6
/// r2.4): Font16, margins 10 / 8, flag 2 (centred, wrapped).
fn popup_text(string_id: u32) -> Control {
    let mut c = Control::new(ControlKind::Text, 268, 320, 264, 120)
        .with_string(string_id)
        .with_font(FONT16, 2);
    c.margin = (10, 8);
    c
}

fn button(x: i32, y: i32, w: u16, h: u16, art: &'static str, label: u32) -> Control {
    Control::new(ControlKind::Button, x, y, w, h)
        .with_art(art)
        .with_string(label)
}

impl Screen for CharSelect {
    fn build(&mut self, ctx: &mut FrontCtx) -> Vec<Control> {
        if !self.refresh || self.model.is_none() {
            // d2rs-own, unverified: no folder = an empty list.
            let dir = self.dir.clone().unwrap_or_default();
            self.model = Some(Model::new(dir, ctx.expansion));
        }
        self.refresh = false;
        let m = self.model.as_ref().expect("model set above");
        let b = m.buttons();
        let mut v = Vec::new();

        // Background (0xA1).
        v.push(Control::new(ControlKind::Image, 0, 599, 800, 600).with_art(BG));
        // Slot labels (0x84+i) and click controls (0x84+i / 0x8C+i).
        for i in 0..SLOTS {
            if let Some(l) = m.slot_lines(i) {
                // 0x84 + i: 200 × 92, a = 76, b = 3, Font16 (§F2.9); rows
                // name (gold / red), level (colour 0), expansion (green).
                // PROVISIONAL (§F2.9 gives no flags): no wrap (flag 0x20),
                // as the 1.14d screenshot shows EXPANSION CHARACTER on one
                // row although it is wider than w − 2·mx = 48.
                // PROVISIONAL (§F2.4 r4 lists three rows): an empty first
                // row (the realm, +0x100, empty in single player) precedes
                // the name, as the 1.14d screenshot puts the name one row
                // pitch (14 px) below the r8 first baseline.
                let mut c = Control::new(ControlKind::Text, slot_x(i), slot_y(i), 200, 92)
                    .with_font(FONT16, 0x20);
                c.margin = (76, 3);
                c.more_rows.push(TextRow {
                    string_id: 0,
                    text: l.name.clone(),
                    color: i32::from(l.name_colour),
                });
                c.more_rows.push(TextRow {
                    string_id: 0,
                    text: l.level.clone(),
                    color: 0,
                });
                if let Some(id) = l.expansion {
                    c.more_rows.push(TextRow {
                        string_id: id,
                        text: String::new(),
                        color: 2,
                    });
                }
                v.push(c);
            }
        }
        for i in 0..SLOTS {
            let (x, y) = (slot_x(i), slot_y(i));
            v.push(
                Control::new(ControlKind::Button, x, y, 200, 92)
                    .with_action(Action::Custom(ids::SLOT_TEXT + i as u32)),
            );
            v.push(
                Control::new(ControlKind::Button, x + 200, y, 72, 93)
                    .with_action(Action::Custom(ids::SLOT_FIGURE + i as u32)),
            );
        }
        // Selection box (0x96) when the selection is visible.
        if let Ok(s) = usize::try_from(m.sel) {
            if s >= m.first && s < m.first + SLOTS && s < m.entries.len() + 1 {
                let i = s - m.first;
                // `charselectbox` is two frames, 256 + 16 px wide: both are
                // drawn side by side. PROVISIONAL (§F2.4 r5 gives 256 × 93):
                // the 1.14d screenshot shows the box 272 px wide (37–308).
                v.push(
                    Control::new(ControlKind::Image, slot_x(i), slot_y(i), 272, 93)
                        .with_art(SEL_BOX),
                );
            }
        }
        // Top label (0x9C): the selected name.
        // PROVISIONAL (§F2.4 r6, §F2.9 give no flags): centred (flags 2),
        // as the 1.14d screenshot shows the name centred in 85–550.
        if let Some(e) = m.selected() {
            v.push(
                Control::new(ControlKind::Text, 85, 78, 466, 42)
                    .with_font(FONT42, 2)
                    .with_text(e.name.clone(), 0),
            );
        }
        // Scroll bar (0xA7), n > 8.
        if m.scroll_range().is_some() {
            v.push(Control::new(ControlKind::Image, 564, 457, 34, 371).with_art(SCROLL));
        }
        // Buttons (0xA2–0xA6).
        let mut ok = button(627, 572, 128, 35, MEDIUM, strings::OK)
            .with_hotkey(vk::ENTER)
            .with_action(Action::Custom(ids::OK));
        ok.enabled = b.ok;
        v.push(ok);
        v.push(
            button(33, 572, 128, 35, MEDIUM, strings::EXIT)
                .with_hotkey(vk::ESC)
                .with_action(Action::Trigger(Trigger::Exit)),
        );
        // Second label lines (§F2.7, `0x00500BF0`): CHARACTER, EXPANSION.
        let mut create = button(33, 528, 168, 60, TALL, strings::CREATE_NEW)
            .with_action(Action::Trigger(Trigger::CreateNew));
        create.second_label = strings::CHARACTER;
        v.push(create);
        let mut conv = button(233, 528, 168, 60, TALL, strings::CONVERT_TO)
            .with_action(Action::Custom(ids::CONVERT));
        conv.second_label = strings::EXPANSION_LABEL;
        conv.enabled = b.convert;
        conv.visible = ctx.expansion;
        v.push(conv);
        let mut del = button(433, 528, 168, 60, TALL, strings::DELETE)
            .with_action(Action::Custom(ids::DELETE));
        del.second_label = strings::CHARACTER;
        del.enabled = b.delete;
        v.push(del);
        // Keys (the selection box's handler).
        for (k, id) in [
            (key::HOME, ids::KEY_HOME),
            (key::END, ids::KEY_END),
            (key::LEFT, ids::KEY_LEFT),
            (key::RIGHT, ids::KEY_RIGHT),
            (key::UP, ids::KEY_UP),
            (key::DOWN, ids::KEY_DOWN),
        ] {
            v.push(Control::key_only(k, Action::Custom(id)));
        }
        // Pop-up, on top of everything.
        match m.popup {
            Popup::None => {}
            Popup::Delete | Popup::Convert => {
                let q = if m.popup == Popup::Delete {
                    strings::DELETE_QUESTION
                } else {
                    strings::CONVERT_WARNING
                };
                v.push(Control::new(ControlKind::Image, 268, 350, 264, 176).with_art(POPUP));
                v.push(popup_text(q));
                v.push(
                    button(281, 337, 96, 32, CANCEL, strings::NO)
                        .with_hotkey(vk::ESC)
                        .with_action(Action::Custom(ids::POPUP_NO)),
                );
                v.push(
                    button(421, 337, 96, 32, CANCEL, strings::YES)
                        .with_action(Action::Custom(ids::POPUP_YES)),
                );
            }
            Popup::Message(id) => {
                v.push(Control::new(ControlKind::Image, 268, 350, 264, 176).with_art(POPUP));
                v.push(popup_text(id));
                v.push(
                    button(351, 337, 96, 32, CANCEL, strings::OK)
                        .with_hotkey(vk::ENTER)
                        .with_keys(&[vk::ESC])
                        .with_action(Action::Custom(ids::POPUP_NO)),
                );
            }
        }
        if m.popup != Popup::None {
            // The pop-up layer owns the input: the list's controls go dead.
            // d2rs-own, unverified: Esc/Enter belong to the pop-up only.
            let n = v.len();
            let popup_start = v.iter().position(|c| c.art == Some(POPUP)).unwrap_or(n);
            for c in &mut v[..popup_start] {
                c.enabled = false;
            }
        }
        v
    }

    fn action(&mut self, ctx: &mut FrontCtx, id: u32) -> Option<Trigger> {
        let now = ctx.now_ms;
        match id {
            ids::OK => return self.run_ok(ctx),
            ids::CONVERT => {
                let m = self.model.as_mut()?;
                if m.buttons().convert && m.popup == Popup::None {
                    m.popup = Popup::Convert;
                    return self.again();
                }
            }
            ids::DELETE => {
                let m = self.model.as_mut()?;
                if m.buttons().delete && m.popup == Popup::None {
                    m.popup = Popup::Delete;
                    return self.again();
                }
            }
            ids::POPUP_NO => {
                self.model.as_mut()?.popup = Popup::None;
                return self.again();
            }
            ids::POPUP_YES => {
                let m = self.model.as_mut()?;
                match m.popup {
                    Popup::Delete => m.delete_selected(),
                    Popup::Convert => m.convert_selected(),
                    _ => {}
                }
                return self.again();
            }
            ids::KEY_HOME..=ids::KEY_DOWN => {
                let k = [
                    key::HOME,
                    key::END,
                    key::LEFT,
                    key::RIGHT,
                    key::UP,
                    key::DOWN,
                ][(id - ids::KEY_HOME) as usize];
                self.model.as_mut()?.key(k);
                return self.again();
            }
            _ => {
                let i = if (ids::SLOT_TEXT..ids::SLOT_TEXT + 8).contains(&id) {
                    id - ids::SLOT_TEXT
                } else if (ids::SLOT_FIGURE..ids::SLOT_FIGURE + 8).contains(&id) {
                    id - ids::SLOT_FIGURE
                } else {
                    return None;
                };
                let double = self.model.as_mut()?.click(i as usize, now);
                if double {
                    if let Some(t) = self.run_ok(ctx) {
                        return Some(t);
                    }
                }
                return self.again();
            }
        }
        None
    }

    // `0x0043AE30` ends with the palette loader `0x0042F2E0` on the sky
    // paths `[0x006D3A08]` / `[0x006D3A0C]` (REC-1906): re-entering after
    // the create screen (fechar) restores them.
}

/// Register the screen with no save folder (an empty list). The host
/// replaces it with [`register_with`].
pub fn register(reg: &mut Registry) {
    register_with(reg, None, SelectionHandle::default());
}

/// Register the screen on `dir`; the chosen character lands in `handle`.
pub fn register_with(reg: &mut Registry, dir: Option<PathBuf>, handle: SelectionHandle) {
    reg.register(CHAR_SELECT, Box::new(CharSelect::new(dir, handle)));
}
