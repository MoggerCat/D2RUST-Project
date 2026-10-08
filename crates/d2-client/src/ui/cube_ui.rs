// Spec: specs/ui/panels.md (§12), specs/ui/panels-2.md (§20 r2–r4); preview fills: docs/handoff/q-cube.md
//! The Horadric Cube (ui 0x1A) installed in the original UI: the
//! [`CubePanel`] art, close and transmute buttons, the page-3 items and
//! grid click ([`ItemsUi::draw_cube`], [`ItemsUi::press_cube`]) and the
//! press / release rules (`panels-2.md` §20 r2–r3, [`StashCubeInput`]).
//! S→C 0x77 0x15 opens the state (`msg_ui`); the C→S intents (0x4F 0x17,
//! 0x4F 0x18, the item moves) leave through the root. The client decides
//! nothing: the server transmutes.
//!
//! Preview fills (d2rs-own, unverified, REC-116): the transmute animation
//! (§12.4), the tool tips and the cube-gone close are not done.

use super::SharedRef;
use crate::bridge::items;
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::cube_items::cube_present;
use crate::ui::panels::stash_cube::{CubePanel, UI_CUBE};
use crate::ui::panels::stash_input::{Pointer, StashCubeInput};
use crate::ui::panels::PanelOutput;
use crate::ui::PointerButton;

/// The cube adapter (ui 0x1A, left half above the control panel).
pub(super) struct CubeUi {
    pub(super) sh: SharedRef,
    pub(super) input: StashCubeInput,
}

impl Panel for CubeUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_CUBE))
    }

    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, (s.w / 2 + 1) as u16, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let env = sh.env();
        let panel = CubePanel {
            close_pressed: self.input.cube_close_pressed,
            transmute_pressed: self.input.transmute_pressed,
        };
        // The cube-gone close (§12.2, two 0x4F 0x17) is not sent from a
        // draw: outputs leave only after an event. The panel draws
        // nothing while the cube item is absent (d2rs-own, unverified).
        if !panel
            .draw(&sh.tables, &env, cube_present(ctx.world), out)
            .is_empty()
        {
            return;
        }
        let g = sh.items.cube_grid(&sh.config.screen);
        sh.items.draw_cube(ctx.world, &sh.tables.files, &g, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        let (down, at) = match e {
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            } => (true, at),
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            } => (false, at),
            _ => return UiResponse::Ignored,
        };
        let mut sh = self.sh.borrow_mut();
        let s = sh.config.screen;
        let ptr = Pointer {
            at,
            in_inv_close: false,
            cursor_item: items::cursor_item(ctx.world).is_some(),
        };
        let eff = if down {
            self.input.cube_down(&s, &ptr)
        } else {
            self.input.cube_up(&s, &ptr)
        };
        if eff.sound4 {
            sh.outputs.push(PanelOutput::ClickSound);
        }
        sh.outputs.extend(eff.outputs);
        if eff.consumed {
            return UiResponse::Consumed;
        }
        if down {
            let g = sh.items.cube_grid(&s);
            let out = sh.items.press_cube(ctx.world, &sh.tables.files, &g, at);
            sh.outputs.extend(out);
        }
        UiResponse::Consumed
    }
}
