// Spec: specs/ui/frontend-menus.md (§F1.1, §F1.3)
//! The interface a front-end screen implements, and the registry that maps
//! a [`ScreenId`] to its implementation. A screen module registers itself
//! from its own `register` function (see `screens/mod.rs`); no shared code
//! changes when one is added.

use std::collections::BTreeMap;

use super::control::{vk, Action, Control};
use super::flow::{FlowCtx, Trigger};
use super::{DrawItem, ScreenId};
use crate::ui::geom::Point;

/// What a screen may read and change while it runs.
pub struct FrontCtx<'a> {
    /// Expansion installed (`0x00408F20` ≠ 0).
    pub expansion: bool,
    /// The flow facts; a screen sets `difficulties_open` before it fires
    /// [`Trigger::Ok`].
    pub flow: &'a mut FlowCtx,
    /// Milliseconds since the front end started (tick × 40, C0).
    pub now_ms: u64,
}

/// One screen. All methods have defaults, so a placeholder is empty.
pub trait Screen {
    /// The controls, in creation (= draw) order. Called on every (re)entry:
    /// re-entering rebuilds the controls (§F1.3 end).
    fn build(&mut self, _ctx: &mut FrontCtx) -> Vec<Control> {
        Vec::new()
    }

    /// A control with [`Action::Custom`] fired. May return a flow trigger.
    fn action(&mut self, _ctx: &mut FrontCtx, _id: u32) -> Option<Trigger> {
        None
    }

    /// Called once per 40 ms tick while the screen is current.
    fn tick(&mut self, _ctx: &mut FrontCtx) -> Option<Trigger> {
        None
    }

    /// A character typed (UTF-16 unit) while the screen is current.
    fn char(&mut self, _ctx: &mut FrontCtx, _unit: u16) {}

    /// The pointer moved (800×600 frame coordinates).
    fn pointer(&mut self, _ctx: &mut FrontCtx, _p: Point) {}

    /// Mouse wheel, `delta` in units of 120 per notch.
    fn wheel(&mut self, _ctx: &mut FrontCtx, _delta: i32) {}

    /// A key went up (virtual-key code).
    fn key_up(&mut self, _ctx: &mut FrontCtx, _vk: u16) {}

    /// The middle mouse button went down.
    fn middle_down(&mut self, _ctx: &mut FrontCtx) {}

    /// Items drawn after the controls: per-tick content the control list
    /// cannot hold (scrolling text, animated heroes). `adv` = text width in
    /// a font.
    fn overlay(&mut self, _now_ms: u64, _adv: &dyn Fn(u16, &[u16]) -> i32) -> Vec<DrawItem> {
        Vec::new()
    }

    /// Adjust the controls to the screen's state (after build and every
    /// tick): show / hide / enable.
    fn sync(&mut self, _controls: &mut [Control]) {}

    /// Whether entering the screen loads the sky palette (§F1.6 r1:
    /// screens built through `0x0043C4F0`). The character screens draw
    /// their own background and leave the palette as it was.
    fn loads_sky_palette(&self) -> bool {
        true
    }
}

/// A screen nobody implemented yet: Esc → [`Trigger::Exit`], Enter →
/// [`Trigger::Ok`] (the hotkeys of §F1.3), no art.
#[derive(Debug, Default)]
pub struct Placeholder;

impl Screen for Placeholder {
    fn build(&mut self, _ctx: &mut FrontCtx) -> Vec<Control> {
        vec![
            Control::key_only(vk::ESC, Action::Trigger(Trigger::Exit)),
            Control::key_only(vk::ENTER, Action::Trigger(Trigger::Ok)),
        ]
    }
}

/// `ScreenId` → implementation. A missing id runs as a [`Placeholder`].
#[derive(Default)]
pub struct Registry {
    screens: BTreeMap<&'static str, Box<dyn Screen>>,
}

impl Registry {
    pub fn register(&mut self, id: ScreenId, screen: Box<dyn Screen>) {
        self.screens.insert(id.0, screen);
    }

    pub fn is_registered(&self, id: ScreenId) -> bool {
        self.screens.contains_key(id.0)
    }

    pub(super) fn get_mut(&mut self, id: ScreenId) -> &mut dyn Screen {
        self.screens
            .entry(id.0)
            .or_insert_with(|| Box::new(Placeholder))
            .as_mut()
    }
}
