// Spec: specs/ui/panels.md (§12), specs/ui/panels-2.md (§20 r2–r4); preview fills: docs/handoff/q-cube.md
//! The Horadric Cube (ui 0x1A) installed in the original UI: the
//! [`CubePanel`] art, close and transmute buttons, the page-3 items and
//! grid click ([`ItemsUi::draw_cube`], [`ItemsUi::press_cube`]) and the
//! press / release rules (`panels-2.md` §20 r2–r3, [`StashCubeInput`]).
//! S→C 0x77 0x15 opens the state (`msg_ui`); the C→S intents (0x4F 0x17,
//! 0x4F 0x18, the item moves) leave through the root. The client decides
//! nothing: the server transmutes.
//!
//! The transmute animation (§12.4, [`HoradricAnim`]) runs on the frame
//! tick and the open clears the close latch (`StashCubeInput::cube_opened`).
//!
//! Preview fills (d2rs-own, unverified, REC-267): the animation starts at
//! the transmute button release (the spec gives the start routine
//! `0x0048A540` but not its caller), the client frame counts 40 ms (the
//! 25 Hz client, as `game_messages`) for the 70 ms wall-clock step, and the
//! draw mode 3 of the cel is the sink's. The cube-gone close runs once per
//! pass ([`OriginalUi::cube_poll`]).

use super::{OriginalUi, OriginalUiError, SharedRef};
use crate::bridge::items;
use crate::bridge::world::ClientWorld;
use crate::ui::draw::{ImageRef, ImageRequest, UiDraw, UiDrawSink};
use crate::ui::geom::{Point, Rect};
use crate::ui::panel::{Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use crate::ui::panels;
use crate::ui::panels::cube_items::cube_player_ok;
use crate::ui::panels::stash_cube::{horadric_pos, CubePanel, STR_TRANSMUTE, UI_CUBE};
use crate::ui::panels::stash_input::{
    cube_close_hit, cube_tooltips, transmute_hit, Pointer, StashCubeInput,
};
use crate::ui::panels::PanelOutput;
use crate::ui::root::UiRoot;
use crate::ui::states::id;
use crate::ui::PointerButton;

/// `strClose` (`panels.md` §8 r1) and the tool tips' font.
const STR_CLOSE: u16 = 4144;
const TIP_FONT: u16 = 1;

/// `menu\horadric` (31 frames), registered with the panel files.
const HORADRIC_FILE: &str = "menu\\horadric";
/// Milliseconds per client frame (d2rs-own, unverified; see the module).
const FRAME_MS: u64 = 40;

pub(super) fn cube_files() -> [String; 1] {
    [HORADRIC_FILE.to_string()]
}

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
        // The dead / no-player close (§12.2, two 0x4F 0x17) is not sent
        // from a draw: outputs leave only after an event, so
        // [`OriginalUi::cube_poll`] sends it. The panel draws nothing
        // that frame.
        if !panel
            .draw(&sh.tables, &env, cube_player_ok(ctx.world), out)
            .is_empty()
        {
            return;
        }
        // §12.4: step on the (wrapping, 32-bit) millisecond tick, draw the
        // cel, and hold the grid back for the first 14 steps.
        let mut anim = sh.cube_anim.get();
        anim.step((ctx.tick.wrapping_mul(FRAME_MS)) as u32);
        sh.cube_anim.set(anim);
        if let (Some(n), Some(file)) = (anim.frame(), sh.tables.files.id(HORADRIC_FILE)) {
            let (x, y) = horadric_pos(&sh.config.screen);
            let s = sh.config.screen;
            out.push(UiDraw::Image(ImageRequest {
                image: ImageRef { file, frame: n },
                at: Point::new(x, y),
                clip: Rect::new(0, 0, s.w as u16, s.h as u16),
                look: crate::ui::CelLook::PLAIN,
            }));
        }
        if anim.grid_visible() {
            let g = sh.items.cube_grid(&sh.config.screen);
            sh.items.draw_cube(ctx.world, &sh.tables.files, &g, out);
        }
        // The button tool tips (§12 r5, `panels-2.md` §20 r3): the strict
        // button rectangles, `strClose` / `strUiMenu2` "Transmute". Drawn
        // when the string table is loaded (font 1, d2rs-own, unverified).
        let s = sh.config.screen;
        let tip = if cube_close_hit(&s, sh.mouse) {
            Some((STR_CLOSE, true))
        } else if transmute_hit(&s, sh.mouse) {
            Some((STR_TRANSMUTE, false))
        } else {
            None
        };
        if let Some(text) = tip.and_then(|(id, close)| Some((ctx.strings.get_id(id)?, close))) {
            let (t, close) = text;
            let w = sh
                .fonts
                .as_ref()
                .and_then(|f| f.width_a(TIP_FONT, t))
                .unwrap_or(0);
            let (c, m) = cube_tooltips(&s, w, w);
            let at = if close { c } else { m };
            out.push(panels::text(t.to_vec(), at.x, at.y, TIP_FONT, 0));
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
        if std::mem::take(&mut sh.cube_opened) {
            self.input.cube_opened();
        }
        let s = sh.config.screen;
        let was_transmute = self.input.transmute_pressed;
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
        // §12.4 start (flag, n := 0, stamp := now); the close clears it (§12 r7).
        if !down && was_transmute && !self.input.transmute_pressed {
            let mut a = sh.cube_anim.get();
            a.start((ctx.tick.wrapping_mul(FRAME_MS)) as u32);
            sh.cube_anim.set(a);
        }
        if eff
            .outputs
            .iter()
            .any(|o| matches!(o, PanelOutput::SetUi { .. }))
        {
            sh.cube_anim.set(Default::default());
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

impl OriginalUi {
    /// Once per pass: when ui 0x1A is open and the local player is missing
    /// or dead (mode 0x11), the close of `panels.md` §12 r2 /
    /// `panels-2.md` §20 r4 (a missing cube does not close it):
    /// `SetUIState(0x1A, off)`, then C→S 0x4F 0x17 twice (the latched
    /// close, then the unconditional one). The state was just open, so
    /// the close latch is the open's (clear). Nothing otherwise.
    pub fn cube_poll(
        &mut self,
        world: &ClientWorld,
        root: &mut UiRoot,
    ) -> Result<(), OriginalUiError> {
        if !self.is_open(id::CUBE) || cube_player_ok(world) {
            return Ok(());
        }
        for o in StashCubeInput::default().cube_gone(true) {
            match o {
                PanelOutput::Intent(i) => root.queue_intent(i),
                PanelOutput::SetUi { ui, mode, jump } => {
                    // The two 0x4F 0x17 above include the hook's.
                    self.set_ui_from(u32::from(ui), u32::from(mode), jump, Some(ui))?;
                }
                PanelOutput::ClickSound | PanelOutput::PlayerEvent(_) => {}
            }
        }
        self.flush_hooks(root);
        self.sync_root(root);
        Ok(())
    }
}

#[cfg(test)]
#[path = "cube_ui_tests.rs"]
mod tests;
