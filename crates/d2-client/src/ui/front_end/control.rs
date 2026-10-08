// Spec: specs/ui/frontend-menus.md (§F1.1 control descriptors)
//! The control-descriptor model every front-end screen is built from.
//!
//! A [`Control`] is one descriptor of `0x00708D10`: type, position, size,
//! hotkey, string id, art, action. **y is the bottom edge** (r3): a hit box
//! is `x ≤ mx < x+w`, `y−h ≤ my < y`.

use crate::ui::geom::{Point, Rect};

use super::flow::Trigger;

/// Descriptor field [0], the control types the front end uses (r2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKind {
    /// 1 edit box.
    EditBox,
    /// 2 image.
    Image,
    /// 3 animated image (the logo halves).
    AnimImage,
    /// 4 text.
    Text,
    /// 6 button.
    Button,
    /// 8 timer (invisible).
    Timer,
}

/// What a control does when it fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing (a disabled or out-of-scope control).
    None,
    /// A screen-flow trigger, resolved by [`super::flow::next`].
    Trigger(Trigger),
    /// A screen-private action: handed to [`super::Screen::action`].
    Custom(u32),
}

/// One control descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    pub kind: ControlKind,
    /// Frame x of the left edge.
    pub x: i32,
    /// Frame y of the **bottom** edge (r3).
    pub y: i32,
    pub w: u16,
    pub h: u16,
    /// Hotkey virtual key (27 Esc, 13 Enter, 0 none), descriptor [5].
    pub hotkey: u16,
    /// Further virtual keys that fire the action (the key handler of the
    /// trademark image, `frontend-credits.md` C2).
    pub keys: &'static [u16],
    /// String id of the label, descriptor [6]; 0 none.
    pub string_id: u32,
    /// Fixed text (the version line) when there is no string id.
    pub text: Option<String>,
    /// Font of a text control (r6), descriptor [10].
    pub font: u16,
    /// The cel file, relative to `data\global\ui\` (r4, §F1.2); `None`: no art.
    pub art: Option<&'static str>,
    pub action: Action,
    /// Timer: whole seconds before it fires (r7), descriptor [1].
    pub seconds: u32,
    pub enabled: bool,
    pub visible: bool,
}

impl Control {
    pub fn new(kind: ControlKind, x: i32, y: i32, w: u16, h: u16) -> Self {
        Self {
            kind,
            x,
            y,
            w,
            h,
            hotkey: 0,
            keys: &[],
            string_id: 0,
            text: None,
            font: 0,
            art: None,
            action: Action::None,
            seconds: 0,
            enabled: true,
            visible: true,
        }
    }

    /// The pointer hit box in frame pixels (r3): top edge `y − h`.
    pub fn hit_box(&self) -> Rect {
        Rect::new(self.x, self.y - i32::from(self.h), self.w, self.h)
    }

    pub fn contains(&self, p: Point) -> bool {
        let r = self.hit_box();
        i64::from(p.x) >= i64::from(r.x)
            && i64::from(p.x) < r.right()
            && i64::from(p.y) >= i64::from(r.y)
            && i64::from(p.y) < r.bottom()
    }

    /// Whether the virtual key `vk` fires this control.
    pub fn takes_key(&self, vk: u16) -> bool {
        vk != 0 && (self.hotkey == vk || self.keys.contains(&vk))
    }

    pub fn with_action(mut self, a: Action) -> Self {
        self.action = a;
        self
    }

    pub fn with_art(mut self, art: &'static str) -> Self {
        self.art = Some(art);
        self
    }

    pub fn with_hotkey(mut self, vk: u16) -> Self {
        self.hotkey = vk;
        self
    }

    pub fn with_keys(mut self, keys: &'static [u16]) -> Self {
        self.keys = keys;
        self
    }

    pub fn with_string(mut self, id: u32) -> Self {
        self.string_id = id;
        self
    }

    /// A timer descriptor (type 8): fires `action` after `seconds` (r7).
    pub fn timer(seconds: u32, action: Action) -> Self {
        let mut c = Self::new(ControlKind::Timer, 0, 0, 0, 0);
        c.seconds = seconds;
        c.action = action;
        c.visible = false;
        c
    }

    /// An invisible key-only control (hotkey fires `action`).
    pub fn key_only(vk: u16, action: Action) -> Self {
        let mut c = Self::new(ControlKind::Button, 0, 0, 0, 0)
            .with_hotkey(vk)
            .with_action(action);
        c.visible = false;
        c
    }
}

/// Virtual-key codes the front end names.
pub mod vk {
    pub const BACKSPACE: u16 = 8;
    pub const TAB: u16 = 9;
    pub const ENTER: u16 = 13;
    pub const ESC: u16 = 27;
    pub const SPACE: u16 = 32;
}

/// Frames per state of a button cel cut in 256×256 tiles (r4):
/// `ceil(w/256)·ceil(h/256)`. Up = `0 … tiles−1`, pressed = `tiles … 2·tiles−1`.
pub fn button_tiles(w: u16, h: u16) -> u32 {
    u32::from(w).div_ceil(256) * u32::from(h).div_ceil(256)
}

/// Label font (r5): 9 (FontExocet10) if `h ≥ 35`, else 10 (FontRidiculous).
pub fn label_font(h: u16) -> u16 {
    if h >= 35 {
        9
    } else {
        10
    }
}

/// The baseline constant k of the label (r5): 4 (h ≥ 35), 3 (h = 32),
/// 2 (20 < h < 32), 1 (h ≤ 20). Heights 33–34 are not covered by the spec's
/// list; none of the front end's buttons has them (PROVISIONAL: 3, REC-230).
pub fn label_k(h: u16) -> i32 {
    match h {
        35.. => 4,
        32..=34 => 3,
        21..=31 => 2,
        _ => 1,
    }
}

/// The frame index a pressed or resting button draws (r4): `Some(frame)` for
/// tile `tile`; a disabled button draws from frame `tiles·2` if `flag_0x20`
/// (3-state art), else the up frames.
pub fn button_frame(tile: u32, tiles: u32, pressed: bool, enabled: bool, flag_0x20: bool) -> u32 {
    if !enabled && flag_0x20 {
        tiles * 2 + tile
    } else if pressed && enabled {
        tiles + tile
    } else {
        tile
    }
}
