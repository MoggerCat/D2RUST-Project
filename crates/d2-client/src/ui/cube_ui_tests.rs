// Spec: specs/ui/panels.md (§12 r2), specs/ui/panels-2.md (§20 r4)
//! The cube-gone close of [`OriginalUi::cube_poll`] on a synthetic model.

use super::*;
use crate::bridge::items::mode;
use crate::bridge::world::ClientWorld;
use crate::ui::geom::Point;
use crate::ui::layout::Screen;
use crate::ui::original::UiConfig;
use crate::ui::panel::{UiCtx, UiEvent};
use crate::ui::panels::inv_items::tests::world;
use crate::ui::NoPanelRules;

fn ui() -> (OriginalUi, UiRoot) {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: true,
    };
    let ui = OriginalUi::new(config, None).unwrap();
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    ui.install(&mut root).unwrap();
    (ui, root)
}

// Covers: specs/ui/panels.md §12 r2; specs/ui/panels-2.md §20 r4
#[test]
fn the_cube_panel_closes_when_the_cube_leaves_the_inventory() {
    let (mut u, mut root) = ui();
    let with_cube = world(&[(7, mode::STORED, (0, 0, 0, 1), b"box ")], None);
    u.set_ui(u32::from(id::CUBE), 0, false).unwrap();
    u.sync_root(&mut root);
    // The cube is still there: the panel stays, nothing is sent.
    u.cube_poll(&with_cube, &mut root).unwrap();
    assert!(u.is_open(id::CUBE));
    assert!(root.intents().is_empty());
    // The cube moved out (here: gone from the model): the panel closes
    // and 0x4F 0x17 leaves twice.
    let without = world(&[(8, mode::STORED, (0, 2, 0, 1), b"hp1 ")], None);
    u.cube_poll(&without, &mut root).unwrap();
    assert!(!u.is_open(id::CUBE));
    let sent: Vec<Vec<u8>> = root.take_intents().into_iter().map(|i| i.0).collect();
    assert_eq!(sent.len(), 2);
    assert!(sent.iter().all(|b| b[..2] == [0x4F, 0x17]), "{sent:?}");
    // Closed: nothing more on the next pass.
    u.cube_poll(&without, &mut root).unwrap();
    assert!(root.intents().is_empty());
}

struct Strs;
impl crate::ui::panel::StringLookup for Strs {
    fn get(&self, _: &str) -> Option<&[u16]> {
        None
    }
    fn get_id(&self, id: u16) -> Option<&[u16]> {
        const CLOSE: [u16; 5] = [
            b'C' as u16,
            b'l' as u16,
            b'o' as u16,
            b's' as u16,
            b'e' as u16,
        ];
        const TRANSMUTE: [u16; 3] = [b'T' as u16, b'r' as u16, b'x' as u16];
        match id {
            4144 => Some(&CLOSE),
            3341 => Some(&TRANSMUTE),
            _ => None,
        }
    }
}

fn tips(u: &mut OriginalUi, root: &UiRoot, w: &ClientWorld, at: Point) -> Vec<String> {
    use crate::ui::draw::UiDraw;
    let e = UiEvent::CursorMoved(at);
    u.before_event(e, w);
    let ctx = UiCtx {
        tick: 0,
        world: w,
        strings: &Strs,
    };
    let mut out: Vec<UiDraw> = Vec::new();
    root.draw(&ctx, &mut out);
    out.iter()
        .filter_map(|d| match d {
            UiDraw::Text(t) => Some(String::from_utf16_lossy(&t.text)),
            _ => None,
        })
        .collect()
}

// Covers: specs/ui/panels.md §12 r5; specs/ui/panels-2.md §20 r3
#[test]
fn the_cube_buttons_show_their_tool_tips_on_hover() {
    let (mut u, mut root) = ui();
    let w = world(&[(7, mode::STORED, (0, 0, 0, 1), b"box ")], None);
    u.set_ui(u32::from(id::CUBE), 0, false).unwrap();
    u.sync_root(&mut root);
    let s = Screen::R800;
    let close = Point::new(s.sx() + 290, s.h + s.sy() - 80);
    let trans = Point::new(s.sx() + 160, s.h + s.sy() - 200);
    assert_eq!(tips(&mut u, &root, &w, close), ["Close"]);
    assert_eq!(tips(&mut u, &root, &w, trans), ["Trx"]);
    assert!(tips(&mut u, &root, &w, Point::new(5, 5)).is_empty());
}
