// Spec: specs/ui/panels.md (§11), specs/ui/panels-2.md (§20 r1), specs/ui/inventory.md (§10); preview fills: docs/handoff/q-stash.md
//! The stash (ui 0x19) installed in the original UI: the
//! [`StashPanel`] art and close button (`panels.md` §11 r2, r4), the
//! page-4 items and grid click ([`ItemsUi::draw_stash`],
//! [`ItemsUi::press_stash`]) and the close press / release rules
//! (`panels-2.md` §20 r1, [`StashCubeInput`]). S→C 0x77 0x10 opens the
//! state (`msg_ui`); the C→S intents leave through the root. The client
//! decides nothing: the server validates every move.
//!
//! Preview fills (d2rs-own, unverified): the GoldMax line needs the
//! string table by id (`NoStrings` in play), so only the art is drawn;
//! the stash gold button and dialog are not wired (`panels-2.md` §21,
//! stitch-hud `PENDING`).

use super::SharedRef;
use crate::bridge::items;
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{NoStrings, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::stash_cube::{StashPanel, UI_STASH};
use crate::ui::panels::stash_input::{Pointer, StashCubeInput, GOLD_MAX_FONT};
use crate::ui::panels::PanelOutput;
use crate::ui::PointerButton;

/// The stash adapter (ui 0x19, left half above the control panel).
pub(super) struct StashUi {
    pub(super) sh: SharedRef,
    pub(super) input: StashCubeInput,
}

impl Panel for StashUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_STASH))
    }

    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, (s.w / 2 + 1) as u16, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let env = sh.env();
        let panel = StashPanel {
            close_pressed: self.input.stash_close_pressed,
        };
        // The cap needs the stat table (`0x00623460`); the line is not
        // drawn without strings anyway.
        panel.draw(&sh.tables, &env, &NoStrings, 0, GOLD_MAX_FONT, out);
        let g = sh.items.stash_grid(env.exp, &sh.config.screen);
        sh.items.draw_stash(ctx.world, &sh.tables.files, &g, out);
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
        let exp = sh.env().exp;
        let ptr = Pointer {
            at,
            in_inv_close: false,
            cursor_item: items::cursor_item(ctx.world).is_some(),
        };
        let eff = if down {
            self.input.stash_down(&s, exp, &ptr)
        } else {
            self.input.stash_up(&s, exp, &ptr)
        };
        if eff.sound4 {
            sh.outputs.push(PanelOutput::ClickSound);
        }
        sh.outputs.extend(eff.outputs);
        if eff.consumed {
            return UiResponse::Consumed;
        }
        if down {
            let g = sh.items.stash_grid(exp, &s);
            let out = sh.items.press_stash(ctx.world, &sh.tables.files, &g, at);
            sh.outputs.extend(out);
        }
        UiResponse::Consumed
    }
}
