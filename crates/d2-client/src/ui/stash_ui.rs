// Spec: specs/ui/panels.md (§11), specs/ui/panels-2.md (§20 r1), specs/ui/inventory.md (§10); preview fills: docs/handoff/q-stash.md
//! The stash (ui 0x19) installed in the original UI: the
//! [`StashPanel`] art and close button (`panels.md` §11 r2, r4), the
//! page-4 items and grid click ([`ItemsUi::draw_stash`],
//! [`ItemsUi::press_stash`]) and the close press / release rules
//! (`panels-2.md` §20 r1, [`StashCubeInput`]). S→C 0x77 0x10 opens the
//! state (`msg_ui`); the C→S intents leave through the root. The client
//! decides nothing: the server validates every move.
//!
//! Preview fills (d2rs-own, unverified): the GoldMax line reads the
//! string table by id (`ctx.strings`); the stash gold button opens the
//! withdraw dialog (kind 4) and the inventory gold button the deposit
//! (kind 3, `gold_dialog`, whose frame-wide panel sees the right half).

use super::SharedRef;
use crate::bridge::items;
use crate::ui::draw::UiDrawSink;
use crate::ui::geom::{Point, Rect};
use crate::ui::gold::GoldKind;
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels::stash_cube::{StashPanel, UI_STASH};
use crate::ui::panels::stash_input::{stash_gold_rect_hit, Pointer, StashCubeInput, GOLD_MAX_FONT};
use crate::ui::panels::PanelOutput;
use crate::ui::PointerButton;
use d2_sim::world::stash::STASH_CAP;

/// The stash adapter (ui 0x19, left half above the control panel).
pub(super) struct StashUi {
    pub(super) sh: SharedRef,
    pub(super) input: StashCubeInput,
}

impl Panel for StashUi {
    /// `ui/panels.md` §9 r1 revision: drawn before the inventory.
    fn draw_before(&self) -> Option<PanelId> {
        Some(PanelId(u16::from(
            crate::ui::panels::inventory::UI_INVENTORY,
        )))
    }

    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_STASH))
    }

    /// The left half above the control panel, plus the inventory close
    /// rectangle the stash / cube mouse handlers test first (`panels.md`
    /// §11 r7, `panels-2.md` §20 r1–r3): the frame above the control
    /// panel, and [`Self::event`] lets every other point go on.
    fn rect(&self) -> Rect {
        let s = self.sh.borrow().config.screen;
        Rect::new(0, 0, s.w as u16, (s.h - 48) as u16)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let env = sh.env();
        let panel = StashPanel {
            close_pressed: self.input.stash_close_pressed,
        };
        // The cap is the fixed stash gold limit (`0x00623460`).
        panel.draw(&sh.tables, &env, ctx.strings, STASH_CAP, GOLD_MAX_FONT, out);
        let g = sh.items.stash_grid(env.exp, &sh.config.screen);
        sh.items.draw_stash(ctx.world, &sh.tables.files, &g, out);
        // `panels-2.md` §20 r1: in the inclusive close rectangle,
        // `strClose` queued at (X + 12 − w / 2, Y − 35), pop-up text in
        // font 1, centre 0 (as the cube's tips, §20 r3).
        let s = sh.config.screen;
        if crate::ui::panels::stash_cube::stash_close_hover(&s, env.exp, sh.mouse) {
            if let Some(t) = ctx.strings.get_id(super::cube_ui::STR_CLOSE) {
                let fonts = sh.fonts.as_ref();
                let w = fonts.and_then(|f| f.width_a(1, t)).unwrap_or(0);
                let (x, y) = crate::ui::panels::stash_cube::stash_close_pos(&s, env.exp);
                let at = Point::new(x + 12 - w / 2, y - 35);
                super::hud_tips::push_popup(t.to_vec(), at, 0, false, (s.w, s.h), fonts, out);
            }
        }
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
        // `0x00486E10`: the inventory close rectangle (the belt popup
        // covering it is not tested: d2rs-own, the popup is the HUD's).
        let in_inv_close = crate::ui::panels::inventory::close_rect(&sh.tables, &s)
            .is_some_and(|r| r.contains(at));
        let left = Rect::new(0, 0, (s.w / 2 + 1) as u16, (s.h - 48) as u16);
        if !in_inv_close && !left.contains(at) {
            return UiResponse::Ignored;
        }
        let ptr = Pointer {
            at,
            in_inv_close,
            cursor_item: items::cursor_item(ctx.world).is_some(),
        };
        let eff = if down {
            self.input.stash_down(&s, exp, &ptr)
        } else {
            self.input.stash_up(&s, exp, &ptr)
        };
        // The stash gold button opens the withdraw dialog (kind 4) on a
        // release inside it (`inventory.md` §11 r1).
        if !down && std::mem::take(&mut self.input.stash_gold) {
            if stash_gold_rect_hit(&s, exp, at) {
                let cursor = ptr.cursor_item;
                super::gold_dialog::open_dialog(&mut sh, ctx.world, GoldKind::Withdraw, cursor);
            }
            return UiResponse::Consumed;
        }
        if eff.sound4 {
            sh.outputs.push(PanelOutput::Sound(4));
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
