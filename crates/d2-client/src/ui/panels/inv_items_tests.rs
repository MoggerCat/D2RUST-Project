// Spec: specs/ui/inventory.md (§3, §6, §8, §10), specs/ui/panels-3.md (§23 r9)
//! Synthetic fixtures for `inv_items`: no game files.

use super::*;
use crate::bridge::items::ItemArtRow;
use crate::bridge::world::{ClientUnit, ItemData, ItemRecord, KindData, PlayerData, UnitKey};
use crate::ui::draw::UiDraw;

const PLAYER: UnitKey = UnitKey::new(0, 1);

/// An item stream head (`items/bitstream.md` §2): flags, version, mode,
/// location (body, x, y, page + 1), code.
fn stream(m: u8, loc: (u8, u16, u16, u8), code: &[u8; 4]) -> Vec<u8> {
    let bits: Vec<(u32, u32)> = vec![
        (0x10, 32),
        (0x65, 10),
        (u32::from(m), 3),
        (u32::from(loc.0), 4),
        (u32::from(loc.1), 4),
        (u32::from(loc.2), 4),
        (u32::from(loc.3), 3),
        (u32::from_le_bytes(*code), 32),
    ];
    let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0u32);
    for (v, w) in bits {
        acc |= u64::from(v) << n;
        n += w;
        while n >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            n -= 8;
        }
    }
    if n > 0 {
        out.push(acc as u8);
    }
    out
}

/// A world with the local player (class 0) and items (guid, mode,
/// location, code); `cursor` names the cursor item.
/// guid, mode, location (body, x, y, page + 1), code.
type Fixture<'a> = (u32, u8, (u8, u16, u16, u8), &'a [u8; 4]);

fn world(items: &[Fixture], cursor: Option<u32>) -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut p = ClientUnit::new(PLAYER);
    p.kind = KindData::Player(PlayerData {
        cursor_item: cursor,
        ..PlayerData::default()
    });
    w.units.insert(PLAYER, p);
    w.local_player = Some(PLAYER);
    for &(guid, m, loc, code) in items {
        let k = UnitKey::new(items::ITEM, guid);
        let mut u = ClientUnit::new(k);
        u.kind = KindData::Item(ItemData {
            last: Some(ItemRecord {
                id: 0x9C,
                action: 0x04,
                category: 0,
                owner: None,
                stream: stream(m, loc, code),
            }),
            ..ItemData::default()
        });
        w.units.insert(k, u);
    }
    w
}

fn layout() -> InvLayout {
    let mut equip = [NO_BOX; 11];
    // Torso (body location 3).
    equip[3] = EquipBox {
        left: 10,
        top: 20,
        w: 58,
        h: 87,
    };
    InvLayout {
        grid: GridRecord {
            grid_x: 10,
            grid_y: 4,
            left: 100,
            right: 390,
            top: 200,
            bottom: 316,
            cell_w: 29,
            cell_h: 29,
        },
        equip,
    }
}

fn ui() -> (ItemsUi, UiFiles) {
    let mut art = ItemArtRows::default();
    let row = |w, h, f: &str| ItemArtRow {
        inv_w: w,
        inv_h: h,
        inv_file: f.into(),
        flippy_file: String::new(),
    };
    art.0.insert(*b"hp1 ", row(1, 1, "invhp1"));
    art.0.insert(*b"qui ", row(2, 3, "invqlt"));
    let u = ItemsUi {
        art,
        layouts: Some(vec![layout()]),
        frame_sizes: BTreeMap::new(),
        shift: false,
        ..ItemsUi::default()
    };
    let mut files = UiFiles::new(&[]);
    u.register_files(&mut files);
    (u, files)
}

fn images(d: &[UiDraw]) -> Vec<(u32, u32, i32, i32)> {
    d.iter()
        .filter_map(|d| match d {
            UiDraw::Image(i) => Some((i.image.file, i.image.frame, i.at.x, i.at.y)),
            _ => None,
        })
        .collect()
}

fn intents(o: &[PanelOutput]) -> Vec<Vec<u8>> {
    o.iter()
        .filter_map(|o| match o {
            PanelOutput::Intent(i) => Some(i.0.clone()),
            _ => None,
        })
        .collect()
}

// Covers: specs/ui/inventory.md §3 r1, §1 r3, §8 r4
#[test]
fn a_grid_item_draws_at_its_cell() {
    let (u, files) = ui();
    let w = world(&[(7, mode::STORED, (0, 2, 3, 1), b"hp1 ")], None);
    let l = u.layout(Some(0), &Screen::R640).unwrap();
    let mut out: Vec<UiDraw> = Vec::new();
    u.draw_panel(&w, &files, &l, &mut out);
    let f = files.id("*items\\invhp1").unwrap();
    // Cell (2, 3): top-left (100 + 58, 200 + 87); cel at top + h (29).
    assert_eq!(images(&out), vec![(f, 0, 158, 316)]);
}

// Covers: specs/ui/inventory.md §6 r2
#[test]
fn an_equipped_item_draws_in_its_box() {
    let (u, files) = ui();
    let w = world(&[(8, mode::BODY, (3, 0, 0, 0), b"qui ")], None);
    let mut out: Vec<UiDraw> = Vec::new();
    u.draw_panel(&w, &files, &layout(), &mut out);
    let f = files.id("*items\\invqlt").unwrap();
    // x = 10 + (58 − 58) / 2 + 3 (torso), y = 20 + (87 − 87) / 2; + h 87.
    assert_eq!(images(&out), vec![(f, 0, 13, 107)]);
}

// Covers: specs/ui/inventory.md §8 r2
#[test]
fn empty_art_draws_nothing_and_item_files_map_to_items() {
    let w = world(&[(7, mode::STORED, (0, 0, 0, 1), b"hp1 ")], Some(7));
    let u = ItemsUi::default();
    let files = UiFiles::new(&[]);
    let mut out: Vec<UiDraw> = Vec::new();
    u.draw_panel(&w, &files, &layout(), &mut out);
    u.draw_cursor(&w, &files, (29, 29), Point::new(5, 5), &mut out);
    assert!(out.is_empty());
    assert_eq!(
        crate::world_view::panel_art::archive_name(&item_file_name("invHp1")),
        "data\\global\\items\\invhp1.dc6"
    );
    assert_eq!(
        crate::world_view::panel_art::archive_name("panel\\invchar6"),
        "data\\global\\ui\\panel\\invchar6.dc6"
    );
}

// Covers: specs/ui/panels-3.md §23 r9
#[test]
fn the_cursor_item_draws_centred_on_the_mouse() {
    let (u, files) = ui();
    let w = world(&[(9, mode::CURSOR, (0, 0, 0, 0), b"qui ")], Some(9));
    let mut out: Vec<UiDraw> = Vec::new();
    u.draw_cursor(&w, &files, (29, 29), Point::new(300, 200), &mut out);
    let f = files.id("*items\\invqlt").unwrap();
    // 58 × 87: top-left (300 − 29, 200 − 43), cel at top + 87.
    assert_eq!(images(&out), vec![(f, 0, 271, 244)]);
}

// Covers: specs/ui/inventory.md §10 r4
#[test]
fn a_cursor_item_on_an_empty_cell_sends_0x18() {
    let (u, files) = ui();
    let w = world(&[(9, mode::CURSOR, (0, 0, 0, 0), b"hp1 ")], Some(9));
    // Mouse in cell (4, 1).
    let out = u.press(
        &w,
        &files,
        &layout(),
        Point::new(100 + 4 * 29 + 3, 200 + 29 + 3),
    );
    let want = ClientIntent::from_message(&items::insert(9, 4, 1, 0)).0;
    assert_eq!(want[0], 0x18);
    assert_eq!(intents(&out), vec![want]);
}

// Covers: specs/ui/inventory.md §10 r3
#[test]
fn a_grid_item_without_a_cursor_item_sends_0x19() {
    let (u, files) = ui();
    let w = world(&[(7, mode::STORED, (0, 2, 3, 1), b"hp1 ")], None);
    let out = u.press(&w, &files, &layout(), Point::new(160, 290));
    assert_eq!(intents(&out), vec![vec![0x19, 7, 0, 0, 0]]);
    // An empty cell sends nothing.
    assert!(u
        .press(&w, &files, &layout(), Point::new(101, 201))
        .is_empty());
}

// Covers: specs/ui/inventory.md §5 r3
#[test]
fn the_cursor_cell_centres_an_even_item() {
    let g = layout().grid;
    // 2 × 3 item, graphic 58 × 87, mouse at (200, 250): c = (14 − 100 +
    // 200) / 29 = 3, minus 1 → 2; r = (250 − 200) / 29 = 1, minus 1 → 0.
    assert_eq!(
        cursor_cell_for(&g, Point::new(200, 250), (2, 3), (58, 87)),
        Some((2, 0))
    );
    // Leaves the grid on the right: none.
    assert_eq!(
        cursor_cell_for(&g, Point::new(385, 250), (2, 3), (58, 87)),
        None
    );
}

// Covers: specs/ui/inventory.md §10 r5
#[test]
fn equipment_box_clicks_send_equip_swap_and_unequip() {
    let (u, files) = ui();
    let at = Point::new(30, 50);
    let w = world(&[(9, mode::CURSOR, (0, 0, 0, 0), b"qui ")], Some(9));
    let want = ClientIntent::from_message(&items::equip(9, 3)).0;
    assert_eq!(want[0], 0x1A);
    assert_eq!(intents(&u.press(&w, &files, &layout(), at)), vec![want]);
    let w = world(
        &[
            (9, mode::CURSOR, (0, 0, 0, 0), b"qui "),
            (8, mode::BODY, (3, 0, 0, 0), b"qui "),
        ],
        Some(9),
    );
    assert_eq!(intents(&u.press(&w, &files, &layout(), at))[0][0], 0x1D);
    let w = world(&[(8, mode::BODY, (3, 0, 0, 0), b"qui ")], None);
    assert_eq!(
        intents(&u.press(&w, &files, &layout(), at)),
        vec![vec![0x1C, 3, 0]]
    );
}

mod belt {
    use super::*;
    use crate::ui::original::hud_belt::{BeltParts, HudBelt};
    use crate::ui::panels::control::belt::{BeltBox, BeltRecord};

    fn parts() -> BeltParts {
        // Four boxes in the strip (y 562..590, x 430 + 31 i), record
        // 0 · 7 + type 2.
        let boxes = (0..4)
            .map(|i| BeltBox {
                left: 430 + 31 * i,
                right: 458 + 31 * i,
                top: 562,
                bottom: 590,
            })
            .collect();
        let mut records = vec![BeltRecord { boxes: vec![] }; 2];
        records.push(BeltRecord { boxes });
        BeltParts {
            records,
            types: BTreeMap::new(),
        }
    }

    // Covers: specs/ui/control-panel.md §5 r4
    #[test]
    fn a_belt_item_draws_in_its_box() {
        let (u, files) = ui();
        let w = world(&[(7, mode::BELT, (0, 1, 0, 0), b"hp1 ")], None);
        let mut b = HudBelt {
            parts: parts(),
            ..Default::default()
        };
        let mut out: Vec<UiDraw> = Vec::new();
        b.draw(&w, &u, &files, (800, 600), false, (0, 0), true, &mut out);
        let f = files.id("*items\\invhp1").unwrap();
        // Box 1: left 461, top 562; cel at top + 29.
        assert_eq!(images(&out), vec![(f, 0, 461, 591)]);
    }

    #[test]
    fn clicking_the_belt_takes_and_puts_potions() {
        let b = HudBelt {
            parts: parts(),
            ..Default::default()
        };
        // No cursor item: 0x24 [guid] on the occupied box 1.
        let w = world(&[(7, mode::BELT, (0, 1, 0, 0), b"hp1 ")], None);
        let sent: Vec<Vec<u8>> = b
            .click(&w, false, (475, 570))
            .into_iter()
            .map(|i| i.0)
            .collect();
        assert_eq!(sent, vec![vec![0x24, 7, 0, 0, 0]]);
        // A potion on the cursor and an empty box 2: 0x23 [guid][slot].
        let w = world(&[(9, mode::CURSOR, (0, 0, 0, 0), b"hp1 ")], Some(9));
        let sent: Vec<Vec<u8>> = b
            .click(&w, false, (506, 570))
            .into_iter()
            .map(|i| i.0)
            .collect();
        assert_eq!(sent, vec![vec![0x23, 9, 0, 0, 0, 2, 0, 0, 0]]);
        // Outside the boxes: nothing.
        assert!(b.click(&w, false, (10, 10)).is_empty());
    }

    #[test]
    fn shift_click_sends_0x63() {
        let (mut u, files) = ui();
        u.shift = true;
        let w = world(&[(7, mode::STORED, (0, 2, 3, 1), b"hp1 ")], None);
        let out = u.press(&w, &files, &layout(), Point::new(160, 290));
        assert_eq!(intents(&out), vec![vec![0x63, 7, 0, 0, 0]]);
    }
}

fn in_cell(c: i32, r: i32) -> Point {
    Point::new(100 + 29 * c + 5, 200 + 29 * r + 5)
}

// Covers: specs/ui/inventory.md §10 r1 (cursor state 6)
#[test]
fn a_right_press_on_identify_then_a_grid_click_sends_0x27() {
    let (u, files) = ui();
    let w = world(
        &[
            (7, mode::STORED, (0, 0, 0, 1), b"isc "),
            (8, mode::STORED, (0, 2, 0, 1), b"hp1 "),
        ],
        None,
    );
    let l = layout();
    // A right press on a non-identify item does nothing.
    u.right_press(&w, &files, &l, in_cell(2, 0));
    assert!(!u.identify_pending());
    u.right_press(&w, &files, &l, in_cell(0, 0));
    assert!(u.identify_pending());
    // Another right press cancels.
    u.right_press(&w, &files, &l, in_cell(0, 0));
    assert!(!u.identify_pending());
    u.right_press(&w, &files, &l, in_cell(0, 0));
    let out = u.press(&w, &files, &l, in_cell(2, 0));
    // 0x27 target, used.
    assert_eq!(
        intents(&out),
        vec![[&[0x27u8][..], &8u32.to_le_bytes(), &7u32.to_le_bytes()].concat()]
    );
    assert!(!u.identify_pending(), "the state ends with the click");
    // Without the state a press lifts the item (0x19).
    let out = u.press(&w, &files, &l, in_cell(2, 0));
    assert_eq!(intents(&out)[0][0], 0x19);
}

// Covers: specs/ui/inventory.md §5 r1
#[test]
fn the_hovered_item_is_the_grid_or_equipped_item_under_the_mouse() {
    let (u, files) = ui();
    let w = world(
        &[
            (7, mode::STORED, (0, 0, 0, 1), b"hp1 "),
            (8, mode::BODY, (3, 0, 0, 0), b"qui "),
        ],
        None,
    );
    let l = layout();
    let guid = |p| u.item_at(&w, &files, &l, p).map(|i| i.key.guid);
    assert_eq!(guid(in_cell(0, 0)), Some(7));
    assert_eq!(guid(in_cell(1, 0)), None);
    assert_eq!(guid(Point::new(20, 50)), Some(8));
    // No tips data, or a cursor item: no lines.
    assert!(u
        .hover_lines(&w, &files, &Screen::R640, Some(0), in_cell(0, 0))
        .is_empty());
}
