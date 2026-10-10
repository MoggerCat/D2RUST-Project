// Spec: specs/ui/panels-3.md (§23 r3–r11, r14)
//! The mouse cursor in play: the §23 machine ([`Cursor`]) fed with the
//! UI's pointer events (move r4, button down r5, up r6, leaving the
//! window r14) and the cursor item (r7), drawn once per frame after the
//! whole UI pass by the top adapter (r9: the item's graphic, else the
//! type's cel, r10; the step runs after the cel draw). The OS cursor is
//! hidden by the play window (`app::play`).
// d2rs-own, unverified (M22):
// - the clock (`GetTickCount`) is the client frame count at 40 ms;
// - the step's seed draws use a UI copy of the local player's client
//   seed (`ClientUnit::seed`), taken when the cursor first sees it; the
//   draws are not written back to the model (r13: the original changes
//   the player's seed);
// - the shop cursors `0x00468010` / `0x00468040` (type 6) are not driven
//   (their callers are the shop mouse handlers, `ui/menus.md` §4);
// - the mouse-move clamp of r4 is off (no `SetCursorPos` in the d2rs
//   window).

use super::Shared;
use crate::bridge::items;
use crate::bridge::world::ClientWorld;
use crate::ui::cursor::{CursorDraw, TYPES};
use crate::ui::draw::{CelLook, ImageRef, ImageRequest, UiDraw, UiDrawSink};
use crate::ui::geom::Point;
use crate::ui::panel::{UiCtx, UiEvent};

use super::OriginalUi;

/// The cel names the art loader resolves (`cursor\<name>`, §23 r1).
pub(super) fn cursor_files() -> Vec<String> {
    TYPES
        .iter()
        .map(|t| format!("cursor\\{}", t.name))
        .collect()
}

/// The client clock of the cursor (module doc).
fn now(world: &ClientWorld) -> u32 {
    (world.frames as u32).wrapping_mul(40)
}

impl OriginalUi {
    /// The pointer events the cursor's window handlers see (§23 r14): the
    /// move, the button down and up (none is consumed), and the pointer
    /// leaving the window (`WM_NCMOUSEMOVE`: not drawn).
    pub(super) fn cursor_event(&mut self, e: UiEvent, world: &ClientWorld) {
        let sh = self.shared.borrow();
        let (w, h) = (sh.config.screen.w, sh.config.screen.h);
        let mut c = sh.cursor.borrow_mut();
        let t = now(world);
        match e {
            UiEvent::CursorMoved(p) => {
                c.mouse_move(p.x, p.y, t, w, h, false);
            }
            UiEvent::Press { at, .. } => c.button_down(at.x, at.y, t),
            UiEvent::Release { at, .. } => c.button_up(at.x, at.y, t),
            UiEvent::CursorLeft => {
                c.nc_mouse_move(0);
            }
            _ => {}
        }
    }
}

/// The cursor draw `0x004684C0` (§23 r9–r11), after every other UI draw.
pub(super) fn draw(sh: &Shared, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
    let mut c = sh.cursor.borrow_mut();
    // r7: every change of the cursor item.
    let item = items::cursor_item(ctx.world);
    if item.is_some() != c.item {
        c.set_item(item.is_some());
    }
    let me = ctx.world.local();
    // r13: the step draws on the local player's client seed, the one the
    // sound and weather draws step; without the shared seed (no sound
    // link) it draws on a UI copy.
    let mut held = sh
        .client_seed
        .as_ref()
        .map(|s| s.lock().unwrap_or_else(|e| e.into_inner()));
    let shared = held.as_mut().and_then(|h| h.seed().map(|s| *s));
    let mut seed = match shared {
        Some(s) => u64::from(s.lo) | (u64::from(s.hi) << 32),
        None => sh.cursor_seed.get().unwrap_or_else(|| {
            me.and_then(|u| u.seed)
                .map_or(0, |(lo, hi)| u64::from(lo) | (u64::from(hi) << 32))
        }),
    };
    let gfx = item
        .as_ref()
        .and_then(|it| sh.items.cursor_graphic_size(&sh.tables.files, it));
    let (w, h) = (sh.config.screen.w, sh.config.screen.h);
    let d = c.draw(w, h, gfx, now(ctx.world), me.is_some(), &mut seed);
    if shared.is_some() {
        if let Some(m) = held.as_mut().and_then(|h| h.seed()) {
            m.lo = seed as u32;
            m.hi = (seed >> 32) as u32;
        }
    } else if me.is_some() {
        sh.cursor_seed.set(Some(seed));
    }
    drop(held);
    match d {
        Ok(Some(CursorDraw::Item { .. })) => {
            // The item's graphic, centred on the mouse (`inv_items`).
            sh.items
                .draw_cursor(ctx.world, &sh.tables.files, (29, 29), sh.mouse, out);
        }
        Ok(Some(CursorDraw::Cel { t, frame, x, y })) => {
            let name = format!("cursor\\{}", TYPES[usize::from(t)].name);
            if let Some(file) = sh.tables.files.id(&name) {
                out.push(UiDraw::Image(ImageRequest {
                    image: ImageRef { file, frame },
                    at: Point::new(x, y),
                    clip: sh.config.screen.rect(),
                    look: CelLook::PLAIN,
                    call: crate::ui::draw::CelCall::Draw,
                }));
            }
        }
        // Not drawn (r9), or a fatal state of the step (r8): nothing.
        Ok(None) | Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::bridge::world::{ClientUnit, ClientWorld, UnitKey, PLAYER};
    use crate::ui::draw::UiDraw;
    use crate::ui::layout::Screen;
    use crate::ui::original::{OriginalUi, UiConfig};
    use crate::ui::panel::{NoStrings, UiCtx, UiEvent};
    use crate::ui::{NoPanelRules, Point, PointerButton, UiRoot};

    fn setup() -> (OriginalUi, UiRoot, ClientWorld) {
        let config = UiConfig {
            screen: Screen::R800,
            expansion_installed: true,
        };
        let ui = OriginalUi::new(config, None).unwrap();
        let mut root = UiRoot::new(Box::new(NoPanelRules));
        ui.install(&mut root).unwrap();
        let mut w = ClientWorld::default();
        let me = UnitKey::new(PLAYER, 1);
        let mut p = ClientUnit::new(me);
        p.mode = 1;
        p.position = Some((1000, 1000));
        p.seed = Some((1, 2));
        w.units.insert(me, p);
        w.local_player = Some(me);
        (ui, root, w)
    }

    fn send(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld, e: UiEvent) {
        let ctx = UiCtx {
            tick: w.frames,
            world: w,
            strings: &NoStrings,
        };
        ui.before_event(e, w);
        let r = root.dispatch(e, &ctx);
        ui.after_event(root, e, r).unwrap();
    }

    /// The last draw of the frame: (file name, frame, x, y).
    fn last(ui: &OriginalUi, root: &UiRoot, w: &ClientWorld) -> (String, u32, i32, i32) {
        let ctx = UiCtx {
            tick: w.frames,
            world: w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        root.draw(&ctx, &mut out);
        let files = ui.files();
        match out.last() {
            Some(UiDraw::Image(i)) => (
                files.name(i.image.file).unwrap().to_string(),
                i.image.frame,
                i.at.x,
                i.at.y,
            ),
            d => panic!("the cursor is the last draw: {d:?}"),
        }
    }

    // The play UI draws the §23 cursor last: protate (t 5) at the mouse
    // after a move, ppress (t 4) while a button is down, protate again on
    // the release, and ohand (t 2) after 5 s without input.
    // Covers: specs/ui/panels-3.md §23 r3, §23 r4, §23 r5, §23 r6, §23 r8, §23 r10
    #[test]
    fn the_cursor_cel_follows_the_pointer_and_the_buttons() {
        let (mut ui, mut root, mut w) = setup();
        let at = Point::new(400, 300);
        send(&mut ui, &mut root, &w, UiEvent::CursorMoved(at));
        assert_eq!(
            last(&ui, &root, &w),
            ("cursor\\protate".into(), 0, 400, 300)
        );
        let button = PointerButton::Left;
        send(&mut ui, &mut root, &w, UiEvent::Press { button, at });
        assert_eq!(last(&ui, &root, &w).0, "cursor\\ppress");
        send(&mut ui, &mut root, &w, UiEvent::Release { button, at });
        assert_eq!(last(&ui, &root, &w).0, "cursor\\protate");
        // r8: in s = 1 past idle + 5000 ms the step starts ohand.
        w.frames += 126;
        last(&ui, &root, &w);
        assert_eq!(last(&ui, &root, &w).0, "cursor\\ohand");
    }
}
