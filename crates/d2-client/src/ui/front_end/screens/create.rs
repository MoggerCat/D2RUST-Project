// Spec: specs/ui/frontend-menus.md (§F3.1–§F3.8)
//! The character-create screen (`0x00435580`): the campfire line-up, the
//! per-class animation state machine (§F3.3), hover text, the name box
//! (§F3.4), the Hardcore / Expansion check boxes (§F3.5) and OK / Exit
//! (§F3.6).
//!
//! The rules live in [`CreateState`] (plain data, tested on synthetic
//! timing); [`CreateScreen`] is the [`Screen`] glue. OK hands the choice to
//! the host through a shared [`NewCharacterSink`]: writing the 335-byte stub
//! save (§F3.6 r3) is the host's job, as the screen sees no file system.
//!
//! Preview fills are `// d2rs-own, unverified`; open points are PROVISIONAL
//! under REC-181.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::ui::front_end::control::{vk, Action, Control, ControlKind};
use crate::ui::front_end::flow::Trigger;
use crate::ui::front_end::screen::{FrontCtx, Screen};
use crate::ui::front_end::{DrawItem, Registry, CHAR_CREATE};
use crate::ui::geom::Point;

/// Creation flag bits `[+0x1EF]` (§F3.5).
pub const FLAG_HARDCORE: u8 = 0x04;
pub const FLAG_EXPANSION: u8 = 0x20;

/// Name box limits (§F3.4): at most 15 characters.
pub const NAME_MAX: usize = 15;
pub const NAME_MIN: usize = 2;

/// `Action::Custom` ids of this screen.
pub mod act {
    /// `CLASS + class id`: a click on that hero.
    pub const CLASS: u32 = 100;
    pub const OK: u32 = 20;
    pub const HARDCORE: u32 = 21;
    pub const EXPANSION: u32 = 22;
    pub const BACKSPACE: u32 = 23;
    pub const WARN_OK: u32 = 24;
    pub const WARN_CANCEL: u32 = 25;
}

/// Class ids (save +0x28, §F3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Amazon = 0,
    Sorceress = 1,
    Necromancer = 2,
    Paladin = 3,
    Barbarian = 4,
    Druid = 5,
    Assassin = 6,
}

impl Class {
    pub fn id(self) -> u8 {
        self as u8
    }

    pub fn from_id(id: u8) -> Option<Class> {
        use Class::*;
        [
            Amazon,
            Sorceress,
            Necromancer,
            Paladin,
            Barbarian,
            Druid,
            Assassin,
        ]
        .get(usize::from(id))
        .copied()
    }

    /// Assassin and Druid appear only with the expansion (§F3.2).
    pub fn is_expansion(self) -> bool {
        matches!(self, Class::Druid | Class::Assassin)
    }

    /// (name string id, description string id) (§F3.3 r7).
    pub fn strings(self) -> (u32, u32) {
        match self {
            Class::Amazon => (4011, 5128),
            Class::Sorceress => (4010, 5131),
            Class::Necromancer => (4009, 5129),
            Class::Paladin => (4008, 5132),
            Class::Barbarian => (4007, 5130),
            Class::Druid => (10097, 22518),
            Class::Assassin => (10098, 22519),
        }
    }
}

/// One hero of the line-up: class and cel position (x, bottom y), in
/// creation (= draw) order (§F3.2).
pub type Slot = (Class, i32, i32);

pub const CLASSIC: [Slot; 5] = [
    (Class::Barbarian, 400, 330),
    (Class::Necromancer, 301, 333),
    (Class::Sorceress, 521, 344),
    (Class::Amazon, 195, 341),
    (Class::Paladin, 610, 359),
];

pub const EXPANSION: [Slot; 7] = [
    (Class::Barbarian, 400, 330),
    (Class::Sorceress, 626, 353),
    (Class::Paladin, 521, 339),
    (Class::Necromancer, 301, 333),
    (Class::Assassin, 232, 364),
    (Class::Amazon, 100, 337),
    (Class::Druid, 720, 370),
];

/// The campfire (§F3.1): descriptor 178 (expansion) / 179 (classic).
pub const FIRE_EXPANSION: (i32, i32) = (345, 470);
pub const FIRE_CLASSIC: (i32, i32) = (345, 454);

/// Hero control size (§F3.2).
pub const HERO_W: u16 = 88;
pub const HERO_H: u16 = 184;

/// Animation states, the index into the anim table (§F3.3 r1).
pub const IDLE: u8 = 0;
pub const HOVER: u8 = 1;
pub const FORWARD: u8 = 2;
pub const SELECTED: u8 = 3;
pub const BACK: u8 = 4;

/// (frames, speed units of 40 ms) per state for a class (§F3.8).
pub fn anim_entry(class: Class, state: u8) -> (u32, u32) {
    // nu1, nu2, fw, nu3, bw
    let t: [(u32, u32); 5] = match class {
        Class::Assassin => [(31, 3), (31, 3), (91, 1), (31, 2), (51, 1)],
        Class::Druid => [(21, 2), (21, 2), (121, 1), (21, 2), (40, 1)],
        Class::Amazon => [(26, 3), (26, 3), (54, 1), (18, 2), (30, 1)],
        Class::Necromancer => [(12, 3), (12, 3), (38, 1), (12, 2), (28, 1)],
        Class::Barbarian => [(16, 2), (16, 2), (64, 1), (26, 1), (19, 1)],
        Class::Sorceress => [(32, 2), (32, 2), (52, 1), (12, 1), (30, 1)],
        Class::Paladin => [(26, 2), (26, 2), (80, 1), (9, 2), (40, 1)],
    };
    t[usize::from(state.min(4))]
}

/// The cel file of a class state, relative to `data\global\ui\` (§F3.8:
/// all under `FrontEnd\<class>\`).
pub fn anim_file(class: Class, state: u8) -> &'static str {
    const F: [[&str; 5]; 7] = [
        [
            "FrontEnd\\amazon\\amnu1",
            "FrontEnd\\amazon\\amnu2",
            "FrontEnd\\amazon\\amfw",
            "FrontEnd\\amazon\\amnu3",
            "FrontEnd\\amazon\\ambw",
        ],
        [
            "FrontEnd\\sorceress\\sonu1",
            "FrontEnd\\sorceress\\sonu2",
            "FrontEnd\\sorceress\\sofw",
            "FrontEnd\\sorceress\\sonu3",
            "FrontEnd\\sorceress\\sobw",
        ],
        [
            "FrontEnd\\necromancer\\nenu1",
            "FrontEnd\\necromancer\\nenu2",
            "FrontEnd\\necromancer\\nefw",
            "FrontEnd\\necromancer\\nenu3",
            "FrontEnd\\necromancer\\nebw",
        ],
        [
            "FrontEnd\\paladin\\panu1",
            "FrontEnd\\paladin\\panu2",
            "FrontEnd\\paladin\\pafw",
            "FrontEnd\\paladin\\panu3",
            "FrontEnd\\paladin\\pabw",
        ],
        [
            "FrontEnd\\barbarian\\banu1",
            "FrontEnd\\barbarian\\banu2",
            "FrontEnd\\barbarian\\bafw",
            "FrontEnd\\barbarian\\banu3",
            "FrontEnd\\barbarian\\babw",
        ],
        [
            "FrontEnd\\druid\\dznu1",
            "FrontEnd\\druid\\dznu2",
            "FrontEnd\\druid\\dzfw",
            "FrontEnd\\druid\\dznu3",
            "FrontEnd\\druid\\dzbw",
        ],
        [
            "FrontEnd\\assassin\\asnu1",
            "FrontEnd\\assassin\\asnu2",
            "FrontEnd\\assassin\\asfw",
            "FrontEnd\\assassin\\asnu3",
            "FrontEnd\\assassin\\asbw",
        ],
    ];
    F[usize::from(class.id())][usize::from(state.min(4))]
}

/// The overlay cel of a class state and its draw mode (§F3.8 "+file
/// mode"), drawn after the base cel with the same frame (§F3.3 r3).
pub fn anim_overlay(class: Class, state: u8) -> Option<(&'static str, u8)> {
    Some(match (class, state) {
        (Class::Necromancer, FORWARD) => ("FrontEnd\\necromancer\\nefws", 3),
        (Class::Necromancer, SELECTED) => ("FrontEnd\\necromancer\\nenu3s", 3),
        (Class::Necromancer, BACK) => ("FrontEnd\\necromancer\\nebws", 3),
        (Class::Barbarian, FORWARD) => ("FrontEnd\\barbarian\\bafws", 5),
        (Class::Sorceress, FORWARD) => ("FrontEnd\\sorceress\\sofws", 3),
        (Class::Sorceress, SELECTED) => ("FrontEnd\\sorceress\\sonu3s", 3),
        (Class::Sorceress, BACK) => ("FrontEnd\\sorceress\\sobws", 3),
        (Class::Paladin, FORWARD) => ("FrontEnd\\paladin\\pafws", 5),
        _ => return None,
    })
}

/// One hero's animation control (`0x00500850`, §F3.3 r2/r4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeroAnim {
    pub class: Class,
    pub state: u8,
    /// `[+0x48]`, ms.
    pub t0: u64,
}

impl HeroAnim {
    pub fn new(class: Class, now: u64) -> Self {
        Self {
            class,
            state: IDLE,
            t0: now,
        }
    }

    /// Frame count and frame duration (ms) of the current state.
    fn timing(&self) -> (u32, u64) {
        let (frames, speed) = anim_entry(self.class, self.state);
        (frames, u64::from(speed) * 1000 / 25)
    }

    /// Frame shown at `now` (§F3.3 r2): states 0, 1, 3 loop over
    /// `frames − 1` (the last frame never shows); 2 and 4 play once.
    pub fn frame(&self, now: u64) -> u32 {
        let (frames, dur) = self.timing();
        if frames <= 1 || dur == 0 {
            return 0;
        }
        let n = (now.saturating_sub(self.t0) / dur) as u32;
        match self.state {
            FORWARD | BACK => n.min(frames - 1),
            _ => n % (frames - 1),
        }
    }

    /// Update (`0x00500480`, before every draw): idle/hover follow the
    /// pointer (t0 kept); a finished walk moves on and restarts.
    pub fn update(&mut self, inside: bool, now: u64) {
        match self.state {
            IDLE | HOVER => self.state = if inside { HOVER } else { IDLE },
            FORWARD | BACK => {
                let (frames, _) = self.timing();
                if self.frame(now) + 2 >= frames {
                    self.state = if self.state == FORWARD {
                        SELECTED
                    } else {
                        HOVER
                    };
                    self.t0 = now;
                }
            }
            _ => {}
        }
    }
}

/// Whether `c` may be typed into the name box (§F3.4 r1–r2). `text` is the
/// current text; the caret is at its end (PROVISIONAL, REC-181).
pub fn accepts_char(text: &str, c: char) -> bool {
    if text.chars().count() >= NAME_MAX {
        return false;
    }
    match c {
        'A'..='Z' | 'a'..='z' => true,
        '-' | '_' => !text.is_empty() && !text.contains(['-', '_']),
        _ => false,
    }
}

/// OK enable rule (§F3.4 r4).
pub fn name_valid(name: &str) -> bool {
    let n = name.chars().count();
    let sep = |c: char| c == '-' || c == '_';
    (NAME_MIN..=NAME_MAX).contains(&n)
        && !name.starts_with(sep)
        && !name.ends_with(sep)
        && name.chars().filter(|&c| sep(c)).count() < 2
}

/// Whether `<dir>/<name>.d2s` exists, ignoring case (§F3.6 r2.2: the
/// Windows file system is case-insensitive).
pub fn name_taken_in(dir: &Path, name: &str) -> bool {
    let want = format!("{name}.d2s").to_ascii_lowercase();
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .any(|e| e.file_name().to_string_lossy().to_ascii_lowercase() == want)
        })
        .unwrap_or(false)
}

/// What OK produced: the host writes the stub save from it (§F3.6 r3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewCharacter {
    pub name: String,
    pub class: Class,
    pub hardcore: bool,
    pub expansion: bool,
}

/// Why OK did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OkResult {
    /// OK disabled (no class or invalid name): the click is ignored.
    Disabled,
    /// Popup 5165 "That character name is already taken."
    NameTaken,
    /// Hardcore popup 5303 up; answer with [`CreateState::warning_ok`] /
    /// [`CreateState::warning_cancel`].
    Warning,
    Created(NewCharacter),
}

pub const STR_NAME_TAKEN: u32 = 5165;
pub const STR_HARDCORE_WARNING: u32 = 5303;

/// The screen's data (§F3.1–§F3.6).
pub struct CreateState {
    expansion_installed: bool,
    pub heroes: Vec<HeroAnim>,
    pub selected: Option<Class>,
    /// Hovered class for the texts `[0x00708D0C]`.
    pub hovered: Option<Class>,
    pub name: String,
    pub flags: u8,
    pub warning: bool,
    pub name_taken: bool,
}

impl CreateState {
    /// Entry (`0x00435580` steps 2–5): nothing selected, flags 0.
    pub fn new(expansion_installed: bool, now: u64) -> Self {
        let slots: &[Slot] = if expansion_installed {
            &EXPANSION
        } else {
            &CLASSIC
        };
        Self {
            expansion_installed,
            heroes: slots.iter().map(|s| HeroAnim::new(s.0, now)).collect(),
            selected: None,
            hovered: None,
            name: String::new(),
            flags: 0,
            warning: false,
            name_taken: false,
        }
    }

    fn hero(&self, c: Class) -> Option<&HeroAnim> {
        self.heroes.iter().find(|h| h.class == c)
    }

    /// Per-frame update with the class under the pointer (if any).
    pub fn update(&mut self, under_pointer: Option<Class>, now: u64) {
        for h in &mut self.heroes {
            h.update(under_pointer == Some(h.class), now);
        }
    }

    /// Hover callbacks (§F3.3 r7): texts follow the pointer only while
    /// nothing is selected.
    pub fn hover(&mut self, class: Option<Class>) {
        if self.selected.is_some() {
            return;
        }
        self.hovered = class;
    }

    /// Texts 197 / 198 (name id, description id), if any.
    pub fn texts(&self) -> Option<((u32, u32), Class)> {
        let c = self.selected.or(self.hovered)?;
        Some((c.strings(), c))
    }

    /// A click on a hero (`0x00433BF0`, §F3.3 r6). Returns whether the
    /// click was accepted.
    pub fn click(&mut self, class: Class, now: u64) -> bool {
        let Some(i) = self.heroes.iter().position(|h| h.class == class) else {
            return false;
        };
        // PROVISIONAL (REC-181): the front end gets no pointer-move, but a
        // click lands where the pointer is, so an idle hero counts as hovered.
        let s = if self.heroes[i].state == IDLE {
            HOVER
        } else {
            self.heroes[i].state
        };
        self.heroes[i].state = s;
        // r6.2, in creation order: a selected hero walks back; a hero that
        // is already walking ends the click (heroes visited stay switched).
        for h in &mut self.heroes {
            match h.state {
                SELECTED => {
                    h.state = BACK;
                    h.t0 = now;
                }
                FORWARD | BACK => return false,
                _ => {}
            }
        }
        let prev = self.selected.take();
        // r6.5
        match s {
            HOVER if prev != Some(class) => self.selected = Some(class),
            FORWARD | SELECTED | BACK => self.selected = None,
            _ => self.selected = Some(class),
        }
        // click callback result: hover → forward (state 3 already went back).
        if s == HOVER {
            let h = &mut self.heroes[i];
            h.state = FORWARD;
            h.t0 = now;
        }
        self.hovered = None;
        // r6.6 / r6.7
        if self.selected.is_some() && self.expansion_installed {
            // every new selection re-checks Expansion
            self.flags |= FLAG_EXPANSION;
        }
        true
    }

    /// Name box and OK are shown only with a class selected (§F3.3 r6.6).
    pub fn name_box_visible(&self) -> bool {
        self.selected.is_some()
    }

    /// The hardcore pair is offered with a class selected (§F3.5 r2).
    pub fn hardcore_visible(&self) -> bool {
        self.selected.is_some()
    }

    /// Expansion check box visible; the grey box replaces it for
    /// Assassin / Druid (§F3.3 r6.7).
    pub fn expansion_box_visible(&self) -> bool {
        self.expansion_installed && self.selected.is_some_and(|c| !c.is_expansion())
    }

    pub fn expansion_grey_visible(&self) -> bool {
        self.expansion_installed && self.selected.is_some_and(Class::is_expansion)
    }

    /// OK enabled: a class and a valid name (§F3.4 r4, §F3.3 r6.6).
    pub fn ok_enabled(&self) -> bool {
        self.selected.is_some() && name_valid(&self.name) && !self.warning
    }

    /// A typed character; returns whether it was accepted. Only while the
    /// name box is shown.
    pub fn type_char(&mut self, c: char) -> bool {
        if !self.name_box_visible() || self.warning || !accepts_char(&self.name, c) {
            return false;
        }
        self.name.push(c);
        self.name_taken = false;
        true
    }

    pub fn backspace(&mut self) {
        if self.name_box_visible() && !self.warning {
            self.name.pop();
            self.name_taken = false;
        }
    }

    /// Hardcore box click (`0x00430730`).
    pub fn toggle_hardcore(&mut self) {
        if self.hardcore_visible() && !self.warning {
            self.flags ^= FLAG_HARDCORE;
        }
    }

    /// Expansion box click (`0x00430770`): no-op without the expansion, or
    /// for the classes that show the grey box.
    pub fn toggle_expansion(&mut self) {
        if self.expansion_box_visible() && !self.warning {
            self.flags ^= FLAG_EXPANSION;
        }
    }

    /// OK / Enter (`0x004369F0`, §F3.6 r2). `taken` is the duplicate-name
    /// check against the save folder.
    pub fn ok(&mut self, taken: &dyn Fn(&str) -> bool) -> OkResult {
        if !self.ok_enabled() {
            return OkResult::Disabled;
        }
        if taken(&self.name) {
            self.name_taken = true;
            return OkResult::NameTaken;
        }
        if self.flags & FLAG_HARDCORE != 0 {
            self.warning = true;
            return OkResult::Warning;
        }
        OkResult::Created(self.finish())
    }

    fn finish(&mut self) -> NewCharacter {
        let class = self.selected.unwrap_or(Class::Amazon);
        if class.is_expansion() {
            self.flags |= FLAG_EXPANSION;
        }
        NewCharacter {
            name: self.name.clone(),
            class,
            hardcore: self.flags & FLAG_HARDCORE != 0,
            expansion: self.flags & FLAG_EXPANSION != 0,
        }
    }

    /// The warning's OK (`0x00436360`).
    pub fn warning_ok(&mut self) -> Option<NewCharacter> {
        if !self.warning {
            return None;
        }
        self.warning = false;
        Some(self.finish())
    }

    /// The warning's CANCEL (`0x00436590`): the whole screen is rebuilt, so
    /// selection, name and flags are lost (§F3.6 r2.4).
    pub fn warning_cancel(&mut self, now: u64) {
        *self = Self::new(self.expansion_installed, now);
    }

    /// Frame of a hero for drawing.
    pub fn hero_frame(&self, class: Class, now: u64) -> Option<(u8, u32)> {
        self.hero(class).map(|h| (h.state, h.frame(now)))
    }
}

/// Where OK leaves the choice for the host.
pub type NewCharacterSink = Rc<RefCell<Option<NewCharacter>>>;

type TakenFn = Box<dyn Fn(&str) -> bool>;

/// The [`Screen`] glue.
pub struct CreateScreen {
    state: Option<CreateState>,
    taken: TakenFn,
    sink: NewCharacterSink,
    /// Indices into the built control list, for [`Screen::sync`].
    idx: Idx,
    /// Pointer seen: the heroes follow it (else the legacy click rule).
    under: Option<Option<Class>>,
}

/// Control positions in the built list that change with the state.
#[derive(Default)]
struct Idx {
    name_box: Vec<usize>,
    hardcore: Vec<usize>,
    expansion: Vec<usize>,
    grey: Vec<usize>,
    ok: Option<usize>,
    warn: Vec<usize>,
}

impl CreateScreen {
    pub fn new(sink: NewCharacterSink, taken: TakenFn) -> Self {
        Self {
            state: None,
            taken,
            sink,
            idx: Idx::default(),
            under: None,
        }
    }

    pub fn state(&self) -> Option<&CreateState> {
        self.state.as_ref()
    }
}

const BG: &str = "FrontEnd\\CharacterCreate";
/// Fonts of the text descriptors (§F1.1 r6): Font30 (`0x007089B4`),
/// Font16 (`0x007089C4`).
const FONT30: u16 = 2;
const FONT16: u16 = 1;
/// Positions of texts 197 and 198 in the built list.
const TEXT_NAME: usize = 2;
const TEXT_DESC: usize = 3;
const BG_EXP: &str = "FrontEnd\\charactercreationscreenEXP";

impl Screen for CreateScreen {
    fn build(&mut self, ctx: &mut FrontCtx) -> Vec<Control> {
        let st = CreateState::new(ctx.expansion, ctx.now_ms);
        self.under = None;
        self.idx = Idx::default();
        let mut v = vec![
            Control::new(ControlKind::Image, 0, 599, 800, 600).with_art(if ctx.expansion {
                BG_EXP
            } else {
                BG
            }),
            // 174 title, 197 class name, 198 description (§F3.1 table).
            Control::new(ControlKind::Text, 0, 80, 800, 50)
                .with_string(5127)
                .with_font(FONT30, 2),
            Control::new(ControlKind::Text, 0, 180, 800, 100).with_font(FONT30, 2),
            Control::new(ControlKind::Text, 250, 210, 300, 100).with_font(FONT16, 2),
            Control::new(ControlKind::Button, 33, 572, 128, 35)
                .with_art("FrontEnd\\MediumSelButtonBlank")
                .with_string(5101)
                .with_hotkey(vk::ESC)
                .with_action(Action::Trigger(Trigger::Exit)),
            Control::new(ControlKind::Image, 319, 519, 169, 26).with_art("FrontEnd\\textbox"),
            Control::new(ControlKind::Text, 321, 512, 200, 32)
                .with_string(5125)
                .with_font(FONT16, 0),
            {
                let mut e = Control::new(ControlKind::EditBox, 318, 510, 157, 16);
                e.font = 5;
                e
            },
        ];
        self.idx.name_box = vec![v.len() - 3, v.len() - 2, v.len() - 1];
        let (hx, hy, lx, ly) = if ctx.expansion {
            (319, 560, 339, 581)
        } else {
            (319, 540, 339, 561)
        };
        self.idx.hardcore = vec![v.len(), v.len() + 1];
        v.push(
            Control::new(ControlKind::Button, hx, hy, 15, 16)
                .with_art("FrontEnd\\clickbox")
                .with_action(Action::Custom(act::HARDCORE)),
        );
        v.push(
            Control::new(ControlKind::Text, lx, ly, 100, 32)
                .with_string(5126)
                .with_font(FONT16, 0),
        );
        if ctx.expansion {
            self.idx.expansion = vec![v.len(), v.len() + 1];
            self.idx.grey = vec![v.len() + 2];
            v.push(
                Control::new(ControlKind::Button, 319, 540, 15, 16)
                    .with_art("FrontEnd\\clickbox")
                    .with_action(Action::Custom(act::EXPANSION)),
            );
            v.push(
                Control::new(ControlKind::Text, 339, 561, 100, 32)
                    .with_string(22731)
                    .with_font(FONT16, 0),
            );
            v.push(
                Control::new(ControlKind::Image, 319, 540, 15, 16)
                    .with_art("FrontEnd\\joingameclickboxgrey"),
            );
        }
        self.idx.ok = Some(v.len());
        v.push(
            Control::new(ControlKind::Button, 627, 572, 128, 35)
                .with_art("FrontEnd\\MediumSelButtonBlank")
                .with_string(5102)
                .with_hotkey(vk::ENTER)
                .with_action(Action::Custom(act::OK)),
        );
        let (fx, fy) = if ctx.expansion {
            FIRE_EXPANSION
        } else {
            FIRE_CLASSIC
        };
        let slots: &[Slot] = if ctx.expansion { &EXPANSION } else { &CLASSIC };
        for &(class, x, y) in slots {
            // Drawn by `overlay` (state-dependent file and frame); the
            // control only takes the click.
            v.push(
                Control::new(ControlKind::Image, x, y, HERO_W, HERO_H)
                    .with_action(Action::Custom(act::CLASS + u32::from(class.id()))),
            );
        }
        v.push(Control::new(ControlKind::AnimImage, fx, fy, 110, 127).with_art("FrontEnd\\fire"));
        v.push(Control::key_only(
            vk::BACKSPACE,
            Action::Custom(act::BACKSPACE),
        ));
        // Hardcore warning (5303): YES / NO, shown while it is up.
        self.idx.warn = vec![v.len(), v.len() + 1];
        v.push(
            Control::new(ControlKind::Button, 270, 400, 128, 35)
                .with_art("FrontEnd\\MediumSelButtonBlank")
                .with_string(5166)
                .with_action(Action::Custom(act::WARN_OK)),
        );
        v.push(
            Control::new(ControlKind::Button, 410, 400, 128, 35)
                .with_art("FrontEnd\\MediumSelButtonBlank")
                .with_string(5167)
                .with_action(Action::Custom(act::WARN_CANCEL)),
        );
        self.state = Some(st);
        v
    }

    fn palette(&self) -> Option<[&'static str; 2]> {
        Some(crate::ui::front_end::FECHAR_PALETTE)
    }

    fn action(&mut self, ctx: &mut FrontCtx, id: u32) -> Option<Trigger> {
        let st = self.state.as_mut()?;
        let done = match id {
            act::HARDCORE => {
                st.toggle_hardcore();
                None
            }
            act::EXPANSION => {
                st.toggle_expansion();
                None
            }
            act::BACKSPACE => {
                st.backspace();
                None
            }
            act::OK => match st.ok(&*self.taken) {
                OkResult::Created(c) => Some(c),
                _ => None,
            },
            act::WARN_OK => st.warning_ok(),
            act::WARN_CANCEL => {
                st.warning_cancel(ctx.now_ms);
                None
            }
            n if n >= act::CLASS => {
                if let Some(c) = Class::from_id((n - act::CLASS) as u8) {
                    st.click(c, ctx.now_ms);
                }
                None
            }
            _ => None,
        };
        done.map(|c| {
            *self.sink.borrow_mut() = Some(c);
            Trigger::Ok
        })
    }

    fn tick(&mut self, ctx: &mut FrontCtx) -> Option<Trigger> {
        let st = self.state.as_mut()?;
        match self.under {
            // The pointer is known: `CreateState::update` (§F3.3 r4).
            Some(under) => st.update(under, ctx.now_ms),
            // No pointer yet: heroes keep their state and only the walks
            // advance.
            None => {
                for h in &mut st.heroes {
                    let inside = h.state == HOVER;
                    h.update(inside, ctx.now_ms);
                }
            }
        }
        None
    }

    fn pointer(&mut self, ctx: &mut FrontCtx, p: Point) {
        let slots: &[Slot] = if ctx.expansion { &EXPANSION } else { &CLASSIC };
        // The 88×184 descriptor box, bottom-left at (x, y) (REC-181).
        let under = slots
            .iter()
            .rev()
            .find(|&&(_, x, y)| {
                p.x >= x && p.x < x + i32::from(HERO_W) && p.y <= y && p.y > y - i32::from(HERO_H)
            })
            .map(|s| s.0);
        self.under = Some(under);
        if let Some(st) = self.state.as_mut() {
            st.hover(under);
        }
    }

    fn overlay(&mut self, now_ms: u64, _adv: &dyn Fn(u16, &[u16]) -> i32) -> Vec<DrawItem> {
        let Some(st) = self.state.as_ref() else {
            return Vec::new();
        };
        let slots: &[Slot] = if st.expansion_installed {
            &EXPANSION
        } else {
            &CLASSIC
        };
        let mut out = Vec::new();
        for &(class, x, y) in slots {
            if let Some((state, frame)) = st.hero_frame(class, now_ms) {
                let at = Point::new(x, y);
                out.push(DrawItem::Art {
                    file: anim_file(class, state),
                    frame,
                    at,
                });
                if let Some((file, mode)) = anim_overlay(class, state) {
                    out.push(DrawItem::Blend {
                        file,
                        frame,
                        at,
                        mode,
                        boxed: false,
                    });
                }
            }
        }
        // Check marks of the boxes.
        for (on, bx, by) in [
            (
                st.flags & FLAG_HARDCORE != 0,
                319,
                if st.expansion_installed { 560 } else { 540 },
            ),
            (
                st.flags & FLAG_EXPANSION != 0 && st.expansion_box_visible(),
                319,
                540,
            ),
        ] {
            if on {
                out.push(DrawItem::Art {
                    file: "FrontEnd\\clickbox",
                    frame: 1,
                    at: Point::new(bx, by),
                });
            }
        }
        if st.name_box_visible() {
            // d2rs-own, unverified: the caret is a trailing `_`.
            out.push(DrawItem::Text {
                label: None,
                string_id: 0,
                text: format!(
                    "{}{}",
                    st.name,
                    if (now_ms / 400).is_multiple_of(2) {
                        "_"
                    } else {
                        ""
                    }
                ),
                font: 5,
                at: Point::new(322, 510),
                color: 0,
                boxed: None,
            });
        }
        let popup = if st.warning {
            Some(STR_HARDCORE_WARNING)
        } else if st.name_taken {
            Some(STR_NAME_TAKEN)
        } else {
            None
        };
        if let Some(id) = popup {
            out.push(DrawItem::Text {
                label: None,
                string_id: id,
                text: String::new(),
                font: 1,
                at: Point::new(270, 360),
                color: 0,
                boxed: None,
            });
        }
        out
    }

    fn sync(&mut self, controls: &mut [Control]) {
        let Some(st) = self.state.as_ref() else {
            return;
        };
        let show = |controls: &mut [Control], ix: &[usize], on: bool| {
            for &i in ix {
                if let Some(c) = controls.get_mut(i) {
                    c.visible = on;
                }
            }
        };
        show(controls, &self.idx.name_box, st.name_box_visible());
        show(controls, &self.idx.hardcore, st.hardcore_visible());
        show(controls, &self.idx.expansion, st.expansion_box_visible());
        show(controls, &self.idx.grey, st.expansion_grey_visible());
        show(controls, &self.idx.warn, st.warning);
        // Texts 197 / 198: the hovered or selected class (§F3.3 r7).
        let (name, desc) = st.texts().map_or((0, 0), |(ids, _)| ids);
        for (i, id) in [(TEXT_NAME, name), (TEXT_DESC, desc)] {
            if let Some(c) = controls.get_mut(i) {
                c.string_id = id;
            }
        }
        if let Some(c) = self.idx.ok.and_then(|i| controls.get_mut(i)) {
            c.visible = st.name_box_visible();
            c.enabled = st.ok_enabled();
        }
    }

    fn char(&mut self, _ctx: &mut FrontCtx, unit: u16) {
        if let (Some(st), Some(c)) = (self.state.as_mut(), char::from_u32(u32::from(unit))) {
            st.type_char(c);
        }
    }

    fn loads_sky_palette(&self) -> bool {
        false
    }
}

/// Register with a host-supplied sink and duplicate-name check.
pub fn register_with(reg: &mut Registry, sink: NewCharacterSink, taken: TakenFn) {
    reg.register(CHAR_CREATE, Box::new(CreateScreen::new(sink, taken)));
}

/// Default registration: no save folder known, nothing is taken, and the
/// choice goes to a sink nobody reads (the host uses [`register_with`]).
pub fn register(reg: &mut Registry) {
    register_with(reg, Rc::default(), Box::new(|_| false));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_filter_vectors() {
        assert!(accepts_char("", 'Z'));
        assert!(!accepts_char("", '7'));
        assert!(!accepts_char("Zz", ' '));
        assert!(!accepts_char("Zz", 'é'));
        assert!(!accepts_char("", '-'));
        assert!(accepts_char("Zz", '-'));
        assert!(!accepts_char("a-b", '_'));
        assert!(!accepts_char(&"a".repeat(15), 'b'));
        assert!(accepts_char(&"a".repeat(14), 'b'));
    }

    #[test]
    fn ok_name_rule() {
        assert!(name_valid("Zz"));
        assert!(!name_valid("Z"));
        assert!(!name_valid("Zz-"));
        assert!(!name_valid("_Zz"));
        assert!(!name_valid("a-b_c"));
        assert!(name_valid("a-bc"));
        assert!(!name_valid(&"a".repeat(16)));
    }

    #[test]
    fn loop_never_shows_last_frame() {
        let a = HeroAnim::new(Class::Paladin, 0);
        // panu1: 26 frames, speed 2 = 80 ms.
        assert_eq!(a.frame(0), 0);
        assert_eq!(a.frame(79), 0);
        assert_eq!(a.frame(80), 1);
        assert_eq!(a.frame(80 * 25), 0);
    }
}
