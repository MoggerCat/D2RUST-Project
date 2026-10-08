// Spec: specs/ui/frontend-options.md (§O2 tree, §O4 positions, §O5 input, §O6–§O8 rows)
//! The Esc menu engine: the menu-record tree (Game → Options → Sound /
//! Video / Automap), the selected row, the 7-entry input table (wrap past
//! disabled rows, slider clamp, click on release, drag) and the value
//! mapping onto [`Settings`]. No drawing here: [`super::esc_menu`] draws
//! and routes events; this module only decides.
//!
//! d2rs-own, unverified: the Window Mode row of the Video menu (the
//! 1.14d menu has none); an expansion install is assumed (exp-only rows
//! are present).

use crate::app::config::Settings;
use crate::ui::geom::Point;

/// The frame is 800 × 600 (one logical size, `client/ui.md` §A5).
pub const W: i32 = 800;
pub const H: i32 = 600;
/// h = W / 2.
pub const HALF: i32 = W / 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuId {
    #[default]
    Game,
    Options,
    Sound,
    Video,
    Automap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Title,
    Action,
    /// Number of values.
    Choice(u8),
    /// Positions (`n`) and style (0 plain, 1 centre-marked).
    Slider {
        n: u8,
        style: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Title,
    Options,
    ExitGame,
    ReturnToGame,
    SoundOptions,
    VideoOptions,
    AutomapOptions,
    ConfigureControls,
    Previous,
    Sound,
    Music,
    Sound3d,
    Eax,
    Bias3d,
    NpcSpeech,
    Resolution,
    WindowMode,
    LightQuality,
    BlendShadow,
    Perspective,
    Gamma,
    Contrast,
    MapMode,
    MapFade,
    MapCenter,
    MapParty,
    MapNames,
}

/// One row of a menu record: the label image name (§O2 r3; drawn as
/// English text here, the DC6 labels are not available to this module)
/// and the value names of a choice.
#[derive(Clone, Copy, Debug)]
pub struct RowDef {
    pub row: Row,
    pub kind: Kind,
    pub label: &'static str,
    pub values: &'static [&'static str],
}

const fn def(row: Row, kind: Kind, label: &'static str, values: &'static [&'static str]) -> RowDef {
    RowDef {
        row,
        kind,
        label,
        values,
    }
}

const OFF_ON: &[&str] = &["Off", "On"];
const NO_YES: &[&str] = &["No", "Yes"];
const S21: Kind = Kind::Slider { n: 21, style: 0 };
const S21C: Kind = Kind::Slider { n: 21, style: 1 };

const GAME: &[RowDef] = &[
    def(Row::Options, Kind::Action, "Options", &[]),
    def(Row::ExitGame, Kind::Action, "Save and Exit Game", &[]),
    def(Row::ReturnToGame, Kind::Action, "Return to Game", &[]),
];
const OPTIONS: &[RowDef] = &[
    def(Row::SoundOptions, Kind::Action, "Sound Options", &[]),
    def(Row::VideoOptions, Kind::Action, "Video Options", &[]),
    def(Row::AutomapOptions, Kind::Action, "Automap Options", &[]),
    def(
        Row::ConfigureControls,
        Kind::Action,
        "Configure Controls",
        &[],
    ),
    def(Row::Previous, Kind::Action, "Previous Menu", &[]),
];
const SOUND: &[RowDef] = &[
    def(Row::Title, Kind::Title, "Sound Options", &[]),
    def(Row::Sound, S21, "Sound", &[]),
    def(Row::Music, S21, "Music", &[]),
    def(Row::Sound3d, Kind::Choice(2), "3D Sound", OFF_ON),
    def(Row::Eax, Kind::Choice(2), "EAX", OFF_ON),
    def(Row::Bias3d, S21C, "3D Bias", &[]),
    def(
        Row::NpcSpeech,
        Kind::Choice(3),
        "NPC Speech",
        &["Audio Only", "Text Only", "Audio and Text"],
    ),
    def(Row::Previous, Kind::Action, "Previous Menu", &[]),
];
const VIDEO: &[RowDef] = &[
    def(Row::Title, Kind::Title, "Video Options", &[]),
    def(
        Row::Resolution,
        Kind::Choice(2),
        "Resolution",
        &["640x480", "800x600"],
    ),
    def(
        Row::WindowMode,
        Kind::Choice(3),
        "Window Mode",
        &["Windowed", "Borderless", "Fullscreen"],
    ),
    def(
        Row::LightQuality,
        Kind::Choice(3),
        "Light Quality",
        &["Low", "Medium", "High"],
    ),
    def(Row::BlendShadow, Kind::Choice(2), "Blended Shadows", OFF_ON),
    def(Row::Perspective, Kind::Choice(2), "Perspective", OFF_ON),
    def(Row::Gamma, S21C, "Gamma", &[]),
    def(
        Row::Contrast,
        Kind::Slider { n: 100, style: 1 },
        "Contrast",
        &[],
    ),
    def(Row::Previous, Kind::Action, "Previous Menu", &[]),
];
const AUTOMAP: &[RowDef] = &[
    def(Row::Title, Kind::Title, "Automap Options", &[]),
    def(
        Row::MapMode,
        Kind::Choice(2),
        "Automap Size",
        &["Full Screen", "Mini"],
    ),
    def(
        Row::MapFade,
        Kind::Choice(4),
        "Automap Fade",
        &["No", "Center", "Everything", "Auto"],
    ),
    def(
        Row::MapCenter,
        Kind::Choice(2),
        "Center when Cleared",
        NO_YES,
    ),
    def(Row::MapParty, Kind::Choice(2), "Show Party", NO_YES),
    def(Row::MapNames, Kind::Choice(2), "Show Party Names", NO_YES),
    def(Row::Previous, Kind::Action, "Previous Menu", &[]),
];

impl MenuId {
    pub fn rows(self) -> &'static [RowDef] {
        match self {
            MenuId::Game => GAME,
            MenuId::Options => OPTIONS,
            MenuId::Sound => SOUND,
            MenuId::Video => VIDEO,
            MenuId::Automap => AUTOMAP,
        }
    }

    /// Header (§O2 r1): row pitch, label baseline, pentagram and slider
    /// offsets.
    pub fn header(self) -> (i32, i32, i32, i32) {
        match self {
            MenuId::Game | MenuId::Options => (50, 39, 51, 0),
            _ => (45, 34, 49, 36),
        }
    }
}

/// What the menu asks of its host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    /// UI sound `cursor_pass` (1); sound is deferred, so it is only named.
    CursorPass,
    /// UI sound `cursor_select` (2).
    CursorSelect,
    /// Save and Exit Game (§O3).
    SaveAndExit,
    /// Return to Game: close the whole menu (§O1 r3).
    Close,
    /// Configure Controls (§O9 r1).
    ConfigureControls,
}

/// The slider value mapping of §O5 r7: `(min, max, n)` of a row.
fn slider_range(row: Row) -> (i64, i64, i64) {
    match row {
        Row::Gamma => (55, 255, 21),
        Row::Contrast => (0, 100, 100),
        _ => (0, 100, 21),
    }
}

/// Position from value (`0x0047CC90`): `trunc((v − min + 1)(n − 1) / (max − min))`.
pub fn pos_from_value(row: Row, v: i64) -> i64 {
    let (lo, hi, n) = slider_range(row);
    ((v - lo + 1) * (n - 1) / (hi - lo)).clamp(0, n - 1)
}

/// Value from position (`0x0047CD00`): `trunc(min + (max − min) p / (n − 1))`.
pub fn value_from_pos(row: Row, p: i64) -> i64 {
    let (lo, hi, n) = slider_range(row);
    lo + (hi - lo) * p / (n - 1)
}

/// The menu's state.
#[derive(Clone, Debug, Default)]
pub struct OptionsMenu {
    pub menu: MenuId,
    pub selected: usize,
    pub settings: Settings,
    /// A setting changed and the host has not written it yet.
    pub changed: bool,
    dragging: bool,
    latch: bool,
    last_ptr: Option<Point>,
    pub events: Vec<MenuEvent>,
}

impl OptionsMenu {
    /// Open on the Game menu, Return to Game selected (§O1 r2).
    pub fn open(&mut self) {
        self.go(MenuId::Game);
        self.dragging = false;
        self.latch = false;
    }

    /// Take the settings in force; Gamma and Contrast snap to their grid
    /// as at every game join (§O6 r11, r12: written back).
    pub fn set_settings(&mut self, s: Settings) {
        self.settings = s;
        for row in [Row::Gamma, Row::Contrast] {
            let v = self.raw(row);
            let snapped = value_from_pos(row, pos_from_value(row, v));
            if snapped != v {
                self.store(row, snapped);
                self.changed = true;
            }
        }
    }

    /// Back from Configure Controls: the Options menu with Previous Menu
    /// selected (the same entry rule as `go`).
    pub fn return_from_controls(&mut self) {
        self.go(MenuId::Options);
    }

    fn go(&mut self, m: MenuId) {
        self.menu = m;
        self.selected = m.rows().len() - 1;
    }

    pub fn rows(&self) -> &'static [RowDef] {
        self.menu.rows()
    }

    /// Enabled test (§O2 r3 with the stubs of §O8): titles never; 3D
    /// Sound, EAX, 3D Bias and Perspective are disabled in d2rs.
    pub fn enabled(&self, i: usize) -> bool {
        !matches!(
            self.rows()[i].row,
            Row::Title | Row::Sound3d | Row::Eax | Row::Bias3d | Row::Perspective
        )
    }

    /// The stored integer of a setting row (registry-style value).
    fn raw(&self, row: Row) -> i64 {
        let s = &self.settings;
        match row {
            Row::Sound => s.master_volume.into(),
            Row::Music => s.music_volume.into(),
            Row::Bias3d => s.positional_bias.into(),
            Row::Gamma => s.gamma.into(),
            Row::Contrast => s.contrast.into(),
            _ => 0,
        }
    }

    fn store(&mut self, row: Row, v: i64) {
        let s = &mut self.settings;
        match row {
            Row::Sound => s.master_volume = v as u8,
            Row::Music => s.music_volume = v as u8,
            Row::Bias3d => s.positional_bias = v as u8,
            Row::Gamma => s.gamma = v as u16,
            Row::Contrast => s.contrast = v as u8,
            _ => {}
        }
    }

    /// The choice index or slider position of row `i`.
    pub fn value(&self, i: usize) -> i64 {
        let row = self.rows()[i].row;
        let s = &self.settings;
        match row {
            Row::Sound | Row::Music | Row::Bias3d | Row::Gamma | Row::Contrast => {
                pos_from_value(row, self.raw(row))
            }
            Row::Sound3d => i64::from(matches!(s.mixer, 1 | 2)),
            Row::Eax => i64::from(s.mixer == 2),
            Row::NpcSpeech => s.npc_speech.into(),
            Row::Resolution => s.resolution.into(),
            Row::WindowMode => crate::app::config::WindowMode::ALL
                .iter()
                .position(|m| *m == s.window_mode)
                .unwrap_or(0) as i64,
            Row::LightQuality => s.light_quality.into(),
            Row::BlendShadow => s.blended_shadows.into(),
            // Shown Off: d2rs has no 3D renderer path (§O8).
            Row::Perspective => 0,
            Row::MapMode => s.automap_mode.into(),
            Row::MapFade => s.automap_fade.into(),
            Row::MapCenter => s.automap_centers.into(),
            Row::MapParty => s.automap_party.into(),
            // §O6 r5: party and names.
            Row::MapNames => i64::from(s.automap_party != 0 && s.automap_party_names != 0),
            _ => 0,
        }
    }

    /// The apply callback (§O6): store the new value, mark it for the
    /// host to write.
    fn apply(&mut self, i: usize, v: i64) {
        let row = self.rows()[i].row;
        match row {
            Row::Sound | Row::Music | Row::Bias3d | Row::Gamma | Row::Contrast => {
                self.store(row, value_from_pos(row, v));
            }
            Row::NpcSpeech => self.settings.npc_speech = v as u8,
            Row::Resolution => self.settings.resolution = v as u8,
            Row::WindowMode => {
                self.settings.window_mode = crate::app::config::WindowMode::ALL[v as usize];
            }
            Row::LightQuality => self.settings.light_quality = v as u8,
            Row::BlendShadow => self.settings.blended_shadows = v as u8,
            Row::MapMode => self.settings.automap_mode = v as u8,
            Row::MapFade => self.settings.automap_fade = v as u8,
            Row::MapCenter => self.settings.automap_centers = v as u8,
            Row::MapParty => self.settings.automap_party = v as u8,
            Row::MapNames => self.settings.automap_party_names = v as u8,
            _ => return,
        }
        self.changed = true;
    }

    // ---- geometry (§O4) ----

    fn pitch(&self) -> i32 {
        self.menu.header().0
    }

    fn n(&self) -> i32 {
        self.rows().len() as i32
    }

    /// y of the menu's top (`y0`).
    pub fn y0(&self) -> i32 {
        (H - 80) / 2 - (self.pitch() * self.n()) / 2
    }

    pub fn y_top(&self, i: usize) -> i32 {
        self.y0() + self.pitch() * i as i32
    }

    /// Label baseline of row `i`.
    pub fn baseline(&self, i: usize) -> i32 {
        self.y_top(i) + self.menu.header().1
    }

    /// Pentagram y of the selected row.
    pub fn pentagram_y(&self) -> i32 {
        self.y_top(self.selected) + self.menu.header().2
    }

    /// Slider base Y of row `i`.
    pub fn slider_y(&self, i: usize) -> i32 {
        self.y_top(i) + self.menu.header().3
    }

    /// Slider knob offset `t = trunc(265 p / (n − 1))` of row `i`.
    pub fn slider_t(&self, i: usize) -> i32 {
        let Kind::Slider { n, .. } = self.rows()[i].kind else {
            return 0;
        };
        (265 * self.value(i) / (i64::from(n) - 1)) as i32
    }

    /// The row under the pointer (`0x0047D520`, y only; §O5 r2).
    pub fn row_at(&self, y: i32) -> Option<usize> {
        let mid = (H - 80) / 2;
        let half = self.pitch() * self.n() / 2;
        if !(mid - half < y && y < mid + half) {
            return None;
        }
        let n = self.n();
        let i = (0..self.rows().len())
            .rev()
            .find(|&i| y >= self.y_top(i) + n)?;
        (self.rows()[i].kind != Kind::Title && self.enabled(i)).then_some(i)
    }

    // ---- input (§O5) ----

    fn push(&mut self, e: MenuEvent) {
        self.events.push(e);
    }

    /// Down (`step` = 1) or Up (`step` = −1): next enabled row, wrapping.
    fn step(&mut self, step: i32) {
        let n = self.rows().len() as i32;
        let mut i = self.selected as i32;
        loop {
            i = (i + step).rem_euclid(n);
            if self.enabled(i as usize) || i as usize == self.selected {
                break;
            }
        }
        self.selected = i as usize;
        self.push(MenuEvent::CursorPass);
    }

    pub fn key_down(&mut self) {
        self.step(1);
    }

    pub fn key_up(&mut self) {
        self.step(-1);
    }

    fn change(&mut self, delta: i64) {
        let i = self.selected;
        if !self.enabled(i) {
            return;
        }
        let old = self.value(i);
        let new = match self.rows()[i].kind {
            Kind::Choice(n) => (old + delta).rem_euclid(i64::from(n)),
            Kind::Slider { n, .. } => (old + delta).clamp(0, i64::from(n) - 1),
            _ => return,
        };
        if new != old {
            self.apply(i, new);
            self.push(MenuEvent::CursorPass);
        }
    }

    pub fn key_left(&mut self) {
        self.change(-1);
    }

    pub fn key_right(&mut self) {
        self.change(1);
    }

    pub fn key_enter(&mut self) {
        self.activate();
    }

    /// `0x0047D5C0`: disabled rows are ignored.
    fn activate(&mut self) {
        let i = self.selected;
        if !self.enabled(i) {
            return;
        }
        match self.rows()[i].kind {
            Kind::Action => {
                self.push(MenuEvent::CursorSelect);
                match self.rows()[i].row {
                    Row::Options => self.go(MenuId::Options),
                    Row::ExitGame => self.push(MenuEvent::SaveAndExit),
                    Row::ReturnToGame => self.push(MenuEvent::Close),
                    Row::SoundOptions => self.go(MenuId::Sound),
                    Row::VideoOptions => self.go(MenuId::Video),
                    Row::AutomapOptions => self.go(MenuId::Automap),
                    Row::ConfigureControls => self.push(MenuEvent::ConfigureControls),
                    // Previous Menu of Options → Game; of a sub-menu → Options.
                    _ => self.go(if self.menu == MenuId::Options {
                        MenuId::Game
                    } else {
                        MenuId::Options
                    }),
                }
            }
            Kind::Choice(n) => {
                let new = (self.value(i) + 1) % i64::from(n);
                self.apply(i, new);
                self.push(MenuEvent::CursorPass);
            }
            _ => {}
        }
    }

    /// Slider drag (`0x0047D670`) with the pointer at `p`.
    fn drag(&mut self, p: Point) {
        let i = self.selected;
        if !(self.latch || self.row_at(p.y) == Some(i)) || !self.enabled(i) {
            return;
        }
        let Kind::Slider { n, .. } = self.rows()[i].kind else {
            return;
        };
        if !self.latch && !(HALF - 59 < p.x && p.x < HALF + 230) {
            return;
        }
        let n = i64::from(n);
        let x0 = HALF - 48;
        let pos = if p.x < x0 {
            0
        } else if p.x > x0 + 265 {
            n - 1
        } else {
            let f = (265.0f64 / (n - 1) as f64 * 0.5) as f32;
            let a = ((p.x - x0) as f32 / f + 1.0) as i64;
            a / 2
        };
        self.latch = true;
        if pos != self.value(i) {
            self.apply(i, pos);
            self.push(MenuEvent::CursorPass);
        }
    }

    /// Left button down.
    pub fn press(&mut self, p: Point) {
        self.last_ptr = Some(p);
        if let Some(i) = self.row_at(p.y) {
            self.selected = i;
        }
        self.dragging = true;
        self.drag(p);
    }

    /// Left button up: activates when the pointer is still on the selected row.
    pub fn release(&mut self, p: Point) {
        self.last_ptr = Some(p);
        if self.dragging && self.row_at(p.y) == Some(self.selected) {
            self.activate();
        }
        self.dragging = false;
        self.latch = false;
    }

    /// Pointer moved (checked at each draw in the original): drag or hover.
    pub fn moved(&mut self, p: Point) {
        if self.last_ptr == Some(p) {
            return;
        }
        self.last_ptr = Some(p);
        if self.dragging {
            self.drag(p);
        } else if let Some(i) = self.row_at(p.y) {
            self.selected = i;
        }
    }

    pub fn take_events(&mut self) -> Vec<MenuEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn take_changed(&mut self) -> Option<Settings> {
        std::mem::take(&mut self.changed).then_some(self.settings)
    }
}

#[cfg(test)]
#[path = "options_menu_tests.rs"]
mod tests;
