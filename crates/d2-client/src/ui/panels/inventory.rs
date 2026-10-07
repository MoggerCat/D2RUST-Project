// Spec: specs/ui/panels.md
//! Inventory panel (ui 1, right slot; §9, `0x0048EDF0`): the parts this
//! spec owns — panel art and close button (§9.3), the close rectangle and
//! its mouse handlers (`0x00486E10`, `0x00489190`, `0x00486EF0`), the
//! empty equipment-slot backgrounds (§9.4) and the dead weapon-swap tab
//! code (§9.5, a no-op). The grid, equipped items, gold and cursor item
//! belong to `ui/inventory.md` (§9.6).
//!
//! Open (spec gaps):
//! - §9.3 does not say whether mouse up clears the close button's pressed
//!   flag `[0x007BCE90]` (§8.3 says so for the character panel only): the
//!   flag is left as it is.
//! - the `panel\inv_*` files of §9.4 are not rows of `panel-layout.tsv`,
//!   so [`super::UiFiles`] has no ids for them: the caller passes a file
//!   resolver.
//! - mouse up outside the close rectangle "ends a pending cursor action"
//!   (§9.3): owner `ui/inventory.md`; nothing is output here.

use super::super::draw::UiDrawSink;
use super::super::geom::{Point, Rect};
use super::super::layout::{PanelKey, RowKind, Screen};
use super::{cel, emit_static_draws, no_extra, PanelEnv, PanelOutput, PanelTables};

/// The ui id of the inventory panel.
pub const UI_INVENTORY: u8 = 1;
const PANEL: PanelKey = PanelKey::Ui(UI_INVENTORY);

/// One `inventory.bin` equipment rectangle (§9.2: left, right, top,
/// bottom; w and h are not read here).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BinRect {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl BinRect {
    pub const fn new(left: i32, right: i32, top: i32, bottom: i32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }
}

/// The ten equipment rectangles of the active `inventory.bin` record
/// (§9.2), in the record's slot order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquipRects {
    pub r_arm: BinRect,
    pub torso: BinRect,
    pub l_arm: BinRect,
    pub head: BinRect,
    pub neck: BinRect,
    pub r_hand: BinRect,
    pub l_hand: BinRect,
    pub belt: BinRect,
    pub feet: BinRect,
    pub gloves: BinRect,
}

/// Body locations 1–10 (§9.4).
pub mod body_loc {
    pub const HEAD: u8 = 1;
    pub const NECK: u8 = 2;
    pub const TORSO: u8 = 3;
    pub const RIGHT_HAND: u8 = 4;
    pub const LEFT_HAND: u8 = 5;
    pub const RIGHT_RING: u8 = 6;
    pub const LEFT_RING: u8 = 7;
    pub const BELT: u8 = 8;
    pub const FEET: u8 = 9;
    pub const GLOVES: u8 = 10;
}

/// What the player wears, as §9.4 reads it (`0x0063BDE0`, `0x0063D340`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquipState {
    /// `occupied[loc]`: an item is in body location `loc` (1–10; index 0
    /// unused).
    pub occupied: [bool; 11],
    /// The item in the right hand (loc 4) is two-handed (`0x0063D340` = 2).
    pub right_two_handed: bool,
    /// The item in the left hand (loc 5) is two-handed.
    pub left_two_handed: bool,
}

#[derive(Clone, Copy)]
enum Slot {
    RArm,
    Torso,
    LArm,
    Head,
    Neck,
    RHand,
    LHand,
    Belt,
    Feet,
    Gloves,
}

/// §9.4 table (offsets `0x007220B0`), in draw order: body loc, file,
/// frame, rectangle, (dx, dy).
const BACKGROUNDS: [(u8, &str, u32, Slot, i32, i32); 10] = [
    (body_loc::TORSO, "panel\\inv_armor", 0, Slot::Torso, 2, -2),
    (body_loc::BELT, "panel\\inv_belt", 0, Slot::Belt, 0, -1),
    (body_loc::FEET, "panel\\inv_boots", 0, Slot::Feet, -1, -4),
    (
        body_loc::HEAD,
        "panel\\inv_helm_glove",
        1,
        Slot::Head,
        0,
        -1,
    ),
    (
        body_loc::GLOVES,
        "panel\\inv_helm_glove",
        0,
        Slot::Gloves,
        -1,
        -2,
    ),
    (
        body_loc::NECK,
        "panel\\inv_ring_amulet",
        0,
        Slot::Neck,
        0,
        -1,
    ),
    (
        body_loc::RIGHT_RING,
        "panel\\inv_ring_amulet",
        1,
        Slot::RHand,
        0,
        -1,
    ),
    (
        body_loc::LEFT_RING,
        "panel\\inv_ring_amulet",
        1,
        Slot::LHand,
        0,
        -1,
    ),
    (
        body_loc::LEFT_HAND,
        "panel\\inv_weapons",
        0,
        Slot::LArm,
        0,
        -2,
    ),
    (
        body_loc::RIGHT_HAND,
        "panel\\inv_weapons",
        0,
        Slot::RArm,
        0,
        -2,
    ),
];

impl EquipRects {
    fn get(&self, s: Slot) -> BinRect {
        match s {
            Slot::RArm => self.r_arm,
            Slot::Torso => self.torso,
            Slot::LArm => self.l_arm,
            Slot::Head => self.head,
            Slot::Neck => self.neck,
            Slot::RHand => self.r_hand,
            Slot::LHand => self.l_hand,
            Slot::Belt => self.belt,
            Slot::Feet => self.feet,
            Slot::Gloves => self.gloves,
        }
    }
}

/// §9.4: whether body location `loc`'s background is drawn.
fn background_shown(loc: u8, eq: &EquipState) -> bool {
    if eq.occupied[usize::from(loc)] {
        return false;
    }
    let other_blocks = |other: u8, two_handed: bool| eq.occupied[usize::from(other)] && two_handed;
    match loc {
        body_loc::RIGHT_HAND => !other_blocks(body_loc::LEFT_HAND, eq.left_two_handed),
        body_loc::LEFT_HAND => !other_blocks(body_loc::RIGHT_HAND, eq.right_two_handed),
        _ => true,
    }
}

/// The empty equipment-slot backgrounds (§9.4) in table order: cel draws
/// at (slot `left` + dx, slot `bottom` + dy). `file_id` names the
/// `panel\inv_*` files (lower case, no extension); a file it does not
/// know is not drawn.
pub fn equip_backgrounds(
    rects: &EquipRects,
    eq: &EquipState,
    file_id: &dyn Fn(&str) -> Option<u32>,
    out: &mut dyn UiDrawSink,
) {
    for (loc, file, frame, slot, dx, dy) in BACKGROUNDS {
        if !background_shown(loc, eq) {
            continue;
        }
        let Some(id) = file_id(file) else {
            continue;
        };
        let r = rects.get(slot);
        out.push(cel(id, frame, r.left + dx, r.bottom + dy));
    }
}

/// The weapon-swap tabs (`Panel\invchar6Tab`) and the second hand pass of
/// §9.5: gated by `[0x007BCC4C]`, which 1.14d never sets non-zero, so they
/// never draw. Reproduced as a no-op.
pub fn swap_tabs(_out: &mut dyn UiDrawSink) {}

/// The close rectangle (§9.3, `0x00486E10`; row `close` `hit`): x in
/// [`W − sx − 302`, `W − sx − 270`], y in [`H + sy − 96`, `H + sy − 64`].
pub fn close_rect(t: &PanelTables, s: &Screen) -> Option<Rect> {
    t.item(PANEL, "close", RowKind::Hit)
        .find_map(|r| r.hit_rect(s))
}

/// Pressed state of the inventory close button (`[0x007BCE90]`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InventoryPanel {
    pub close_pressed: bool,
}

impl InventoryPanel {
    /// Panel art (frames 4–7 as right quads, `InvChar6` / `InvChar` per
    /// §8.1) and the close button (frame 10, 11 pressed), §9.3, in
    /// `panel-layout.tsv` row order.
    pub fn draw(&self, t: &PanelTables, env: &PanelEnv, out: &mut dyn UiDrawSink) {
        let cenv = env.cond(self.close_pressed, &no_extra);
        emit_static_draws(t, PANEL, &cenv, None, &|_| true, out);
    }

    /// Mouse down (`0x00489190`): in the close rectangle sets pressed.
    pub fn press(&mut self, t: &PanelTables, s: &Screen, at: Point) {
        if close_rect(t, s).is_some_and(|r| r.contains(at)) {
            self.close_pressed = true;
        }
    }

    /// Mouse up (`0x00486EF0`): in the close rectangle,
    /// `SetUIState(1, off, 0)` without checking pressed (§Edge cases).
    pub fn release(&mut self, t: &PanelTables, s: &Screen, at: Point) -> Vec<PanelOutput> {
        if close_rect(t, s).is_some_and(|r| r.contains(at)) {
            vec![PanelOutput::SetUi {
                ui: UI_INVENTORY,
                mode: 1,
                jump: false,
            }]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::draw::UiDraw;
    use super::*;

    fn tables() -> PanelTables {
        PanelTables::load().expect("tables")
    }

    fn env(screen: Screen, exp: bool) -> PanelEnv {
        PanelEnv {
            screen,
            open_mode: 1,
            exp,
        }
    }

    fn images(d: &[UiDraw]) -> Vec<(u32, u32, i32, i32)> {
        d.iter()
            .filter_map(|d| match d {
                UiDraw::Image(i) => Some((i.image.file, i.image.frame, i.at.x, i.at.y)),
                _ => None,
            })
            .collect()
    }

    const FILES: [&str; 6] = [
        "panel\\inv_armor",
        "panel\\inv_belt",
        "panel\\inv_boots",
        "panel\\inv_helm_glove",
        "panel\\inv_ring_amulet",
        "panel\\inv_weapons",
    ];

    fn file_id(n: &str) -> Option<u32> {
        FILES.iter().position(|f| *f == n).map(|i| 100 + i as u32)
    }

    // Covers: specs/ui/panels.md §1 r4
    #[test]
    fn art_640() {
        let t = tables();
        let mut out: Vec<UiDraw> = Vec::new();
        InventoryPanel::default().draw(&t, &env(Screen::R640, true), &mut out);
        let f = t.files.id("panel\\invchar6").unwrap();
        let btn = t.files.id("panel\\buysellbtn").unwrap();
        assert_eq!(
            images(&out),
            vec![
                (f, 4, 320, 256),
                (f, 5, 576, 256),
                (f, 6, 320, 432),
                (f, 7, 576, 432),
                (btn, 10, 338, 416),
            ]
        );
    }

    // Covers: specs/ui/panels.md §9 r3
    #[test]
    fn art_800_and_close_button() {
        let t = tables();
        let mut out: Vec<UiDraw> = Vec::new();
        InventoryPanel::default().draw(&t, &env(Screen::R800, true), &mut out);
        let f = t.files.id("panel\\invchar6").unwrap();
        let btn = t.files.id("panel\\buysellbtn").unwrap();
        assert_eq!(
            images(&out),
            vec![
                (f, 4, 400, 316),
                (f, 5, 656, 316),
                (f, 6, 400, 492),
                (f, 7, 656, 492),
                (btn, 10, 418, 476),
            ]
        );
        let mut out: Vec<UiDraw> = Vec::new();
        let p = InventoryPanel {
            close_pressed: true,
        };
        p.draw(&t, &env(Screen::R800, false), &mut out);
        let c = t.files.id("panel\\invchar").unwrap();
        assert_eq!(images(&out)[0], (c, 4, 400, 316));
        assert_eq!(images(&out)[4], (btn, 11, 418, 476));
        // Close rect x [418, 450], y [444, 476].
        assert_eq!(
            close_rect(&t, &Screen::R800),
            Some(Rect::new(418, 444, 33, 33))
        );
        // Down there sets pressed; up there closes without checking it.
        let s = Screen::R800;
        let mut q = InventoryPanel::default();
        q.press(&t, &s, Point::new(450, 444));
        assert!(q.close_pressed);
        let mut r = InventoryPanel::default();
        assert_eq!(
            r.release(&t, &s, Point::new(418, 476)),
            vec![PanelOutput::SetUi {
                ui: 1,
                mode: 1,
                jump: false
            }]
        );
        assert!(r.release(&t, &s, Point::new(417, 476)).is_empty());
    }

    // Covers: specs/ui/panels.md §9 r3, §7 r3
    #[test]
    fn close_release_without_press() {
        let t = tables();
        let s = Screen::R640;
        let mut p = InventoryPanel::default();
        // 640: x [338, 370], y [384, 416].
        assert_eq!(
            p.release(&t, &s, Point::new(370, 384)),
            vec![PanelOutput::SetUi {
                ui: 1,
                mode: 1,
                jump: false
            }]
        );
        assert!(p.release(&t, &s, Point::new(371, 384)).is_empty());
        assert!(p.release(&t, &s, Point::new(338, 417)).is_empty());
        p.press(&t, &s, Point::new(337, 400));
        assert!(!p.close_pressed);
        p.press(&t, &s, Point::new(338, 416));
        assert!(p.close_pressed);
    }

    fn record16() -> EquipRects {
        EquipRects {
            head: BinRect::new(535, 589, 68, 119),
            ..EquipRects::default()
        }
    }

    #[test]
    fn empty_head_record16() {
        let mut eq = EquipState {
            occupied: [true; 11],
            ..EquipState::default()
        };
        eq.occupied[usize::from(body_loc::HEAD)] = false;
        let mut out: Vec<UiDraw> = Vec::new();
        equip_backgrounds(&record16(), &eq, &file_id, &mut out);
        assert_eq!(images(&out), vec![(103, 1, 535, 118)]);
    }

    // Covers: specs/ui/panels.md §9 r4
    #[test]
    fn backgrounds_order_offsets_and_two_handed() {
        let r = |i: i32| BinRect::new(10 * i, 10 * i + 5, 100 * i, 100 * i + 50);
        let rects = EquipRects {
            r_arm: r(1),
            torso: r(2),
            l_arm: r(3),
            head: r(4),
            neck: r(5),
            r_hand: r(6),
            l_hand: r(7),
            belt: r(8),
            feet: r(9),
            gloves: r(10),
        };
        let mut out: Vec<UiDraw> = Vec::new();
        equip_backgrounds(&rects, &EquipState::default(), &file_id, &mut out);
        assert_eq!(
            images(&out),
            vec![
                (100, 0, 22, 248),  // torso (2, −2)
                (101, 0, 80, 849),  // belt (0, −1)
                (102, 0, 89, 946),  // feet (−1, −4)
                (103, 1, 40, 449),  // head (0, −1)
                (103, 0, 99, 1048), // gloves (−1, −2)
                (104, 0, 50, 549),  // neck (0, −1)
                (104, 1, 60, 649),  // right ring, rHand rect
                (104, 1, 70, 749),  // left ring, lHand rect
                (105, 0, 30, 348),  // left hand, lArm rect (0, −2)
                (105, 0, 10, 148),  // right hand, rArm rect
            ]
        );
        // Right hand holds a two-handed weapon: the left hand stays bare.
        let mut eq = EquipState::default();
        eq.occupied[usize::from(body_loc::RIGHT_HAND)] = true;
        eq.right_two_handed = true;
        let mut out: Vec<UiDraw> = Vec::new();
        equip_backgrounds(&rects, &eq, &file_id, &mut out);
        assert!(!images(&out).iter().any(|i| i.0 == 105));
        // One-handed: the left hand gets its picture.
        eq.right_two_handed = false;
        let mut out: Vec<UiDraw> = Vec::new();
        equip_backgrounds(&rects, &eq, &file_id, &mut out);
        assert_eq!(
            images(&out)
                .iter()
                .filter(|i| i.0 == 105)
                .collect::<Vec<_>>(),
            vec![&(105, 0, 30, 348)]
        );
    }

    // Covers: specs/ui/panels.md §9 r5
    #[test]
    fn swap_tabs_draw_nothing() {
        let mut out: Vec<UiDraw> = Vec::new();
        swap_tabs(&mut out);
        assert!(out.is_empty());
    }
}
