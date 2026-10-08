// Spec: specs/ui/inventory.md (§3, §6, §8, §10), specs/ui/panels-3.md (§23 r9)
//! Synthetic fixtures for `inv_items`: no game files.

use super::*;
use crate::bridge::items::ItemArtRow;
use crate::bridge::world::{ClientUnit, ItemData, ItemRecord, KindData, PlayerData, UnitKey};
use crate::ui::draw::{ImageRef, UiDraw};

pub(crate) const PLAYER: UnitKey = UnitKey::new(0, 1);

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
pub(crate) type Fixture<'a> = (u32, u8, (u8, u16, u16, u8), &'a [u8; 4]);

pub(crate) fn world(items: &[Fixture], cursor: Option<u32>) -> ClientWorld {
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
                seq: 0,
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

// Covers: specs/items/inventory-moves.md §7.11
#[test]
fn a_right_click_on_a_grid_item_sends_0x20_with_the_player_point() {
    let (u, _) = ui();
    let mut w = world(&[(7, mode::STORED, (0, 2, 3, 1), b"hp1 ")], None);
    w.units.get_mut(&PLAYER).unwrap().position = Some((300, 400));
    let out = u.use_press(&w, &layout(), Point::new(160, 290));
    assert_eq!(
        intents(&out),
        vec![vec![0x20, 7, 0, 0, 0, 0x2C, 1, 0, 0, 0x90, 1, 0, 0]]
    );
    // An empty cell, or an item on the cursor: nothing.
    assert!(u.use_press(&w, &layout(), Point::new(101, 201)).is_empty());
    let c = world(&[(7, mode::STORED, (0, 2, 3, 1), b"hp1 ")], Some(9));
    assert!(u.use_press(&c, &layout(), Point::new(160, 290)).is_empty());
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

fn stash_grid() -> GridRecord {
    GridRecord {
        grid_x: 6,
        grid_y: 8,
        left: 154,
        right: 154 + 6 * 29,
        top: 142,
        bottom: 142 + 8 * 29,
        cell_w: 29,
        cell_h: 29,
    }
}

// Covers: specs/items/inventory.md §1.3
#[test]
fn the_stash_grid_is_record_12_or_8_plus_16_at_800() {
    use crate::ui::panels::stash_items::{fallback_stash_grid, stash_record};
    assert_eq!(stash_record(true, &Screen::R640), 12);
    assert_eq!(stash_record(true, &Screen::R800), 28);
    assert_eq!(stash_record(false, &Screen::R800), 24);
    let g = fallback_stash_grid(true, &Screen::R800);
    assert_eq!((g.left, g.top, g.grid_x, g.grid_y), (154, 142, 6, 8));
    // Layouts of the install win over the fallback.
    let mut u = ui().0;
    let mut l = vec![layout(); 29];
    l[28].grid = stash_grid();
    u.layouts = Some(l);
    assert_eq!(u.stash_grid(true, &Screen::R800), stash_grid());
}

// Covers: specs/ui/inventory.md §3 r1; specs/ui/inventory.md §8 r4
#[test]
fn a_stash_item_draws_at_its_stash_cell_and_not_in_the_inventory() {
    let (u, files) = ui();
    let w = world(
        &[
            (7, mode::STORED, (0, 2, 3, 5), b"hp1 "),
            (8, mode::STORED, (0, 0, 0, 1), b"hp1 "),
        ],
        None,
    );
    let mut out = Vec::new();
    u.draw_stash(&w, &files, &stash_grid(), &mut out);
    let file = files.id(&item_file_name("invhp1")).unwrap();
    // Cell (2, 3): (154 + 58, 142 + 87), drawn at top + frame height 29.
    assert_eq!(images(&out), vec![(file, 0, 212, 258)]);
    let mut inv = Vec::new();
    u.draw_panel(&w, &files, &layout(), &mut inv);
    assert_eq!(images(&inv).len(), 1, "only the page-0 item");
}

// Covers: specs/ui/inventory.md §10 r3; specs/ui/inventory.md §10 r4
#[test]
fn stash_grid_clicks_send_the_page_4_intents() {
    let (u, files) = ui();
    // Cursor item onto the empty stash cell (4, 1): 0x18 with page 4.
    let w = world(&[(9, mode::CURSOR, (0, 0, 0, 0), b"hp1 ")], Some(9));
    let at = Point::new(154 + 4 * 29 + 3, 142 + 29 + 3);
    let out = u.press_stash(&w, &files, &stash_grid(), at);
    let want = ClientIntent::from_message(&items::insert(9, 4, 1, 4)).0;
    assert_eq!(intents(&out), vec![want]);
    // A stash item without a cursor item lifts (0x19); a page-0 item at
    // the same cell is not under the stash mouse.
    let w = world(
        &[
            (7, mode::STORED, (0, 2, 3, 5), b"hp1 "),
            (8, mode::STORED, (0, 2, 3, 1), b"hp1 "),
        ],
        None,
    );
    let at = Point::new(154 + 2 * 29 + 3, 142 + 3 * 29 + 3);
    let out = u.press_stash(&w, &files, &stash_grid(), at);
    assert_eq!(intents(&out), vec![vec![0x19, 7, 0, 0, 0]]);
    // Outside the grid: nothing.
    assert!(u
        .press_stash(&w, &files, &stash_grid(), Point::new(5, 5))
        .is_empty());
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

    /// The fixture items as compact records (the tip decodes those
    /// without the extended fields).
    fn compact(mut w: ClientWorld) -> ClientWorld {
        for u in w.units.values_mut() {
            if let KindData::Item(d) = &mut u.kind {
                if let Some(r) = d.last.as_mut() {
                    r.stream[2] |= (d2_proto::item_bits::hflag::COMPACT >> 16) as u8;
                }
            }
        }
        w
    }

    // Covers: specs/ui/control-panel.md §5 r8, §5 r12
    #[test]
    fn hovering_a_belt_item_yields_its_tip_at_the_box() {
        let tips = crate::ui::item_tip::tests::tips();
        let mut b = HudBelt {
            parts: parts(),
            ..Default::default()
        };
        let w = compact(world(&[(7, mode::BELT, (0, 1, 0, 0), b"cap ")], None));
        // Nothing hovered yet: no tip.
        assert!(b.hover_tip(&w, &tips).0.is_empty());
        // Over box 1 (left 461, top 562): the tip anchors at (left + 14, top).
        b.draw_list(&w, (800, 600), false, (470, 570), true);
        let (lines, at) = b.hover_tip(&w, &tips);
        assert!(!lines.is_empty(), "the hovered potion has a tip");
        assert_eq!(at, (475, 562));
        // An item on the cursor hides it (§5 r8).
        let w = compact(world(
            &[
                (7, mode::BELT, (0, 1, 0, 0), b"cap "),
                (9, mode::CURSOR, (0, 0, 0, 0), b"cap "),
            ],
            Some(9),
        ));
        assert!(b.hover_tip(&w, &tips).0.is_empty());
        // Off the belt the hover ends.
        let w = compact(world(&[(7, mode::BELT, (0, 1, 0, 0), b"cap ")], None));
        b.draw_list(&w, (800, 600), false, (10, 10), true);
        assert!(b.hover_tip(&w, &tips).0.is_empty());
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

    // Covers: specs/ui/control-panel.md §5 r4, §5 r8
    #[test]
    fn the_belt_draw_list_has_key_labels_and_the_hover_rect() {
        use crate::ui::panels::control::belt::{BeltColor, BeltDraw};
        let w = world(
            &[
                (7, mode::BELT, (0, 1, 0, 0), b"hp1 "),
                (8, mode::BELT, (0, 3, 0, 0), b"hp1 "),
            ],
            None,
        );
        let mut b = HudBelt {
            parts: parts(),
            ..Default::default()
        };
        // Off the belt: labels at (left + 2, bottom - 2), color 4, no rect.
        let d = b.draw_list(&w, (800, 600), false, (0, 0), true);
        let labels: Vec<_> = d
            .iter()
            .filter_map(|d| match d {
                BeltDraw::Label(l) => Some((l.text.clone(), l.x, l.y, l.color)),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            vec![
                (vec![u16::from(b'2')], 463, 588, 4),
                (vec![u16::from(b'4')], 525, 588, 4)
            ]
        );
        assert!(!d.iter().any(|d| matches!(d, BeltDraw::Box { .. })));
        // Over box 1: the item's green 29 x 29 hover rectangle at its box.
        let d = b.draw_list(&w, (800, 600), false, (470, 570), true);
        let boxes: Vec<_> = d
            .iter()
            .filter_map(|d| match d {
                BeltDraw::Box { rect, color } => Some((rect.x, rect.y, rect.w, rect.h, *color)),
                _ => None,
            })
            .collect();
        assert_eq!(boxes, vec![(461, 562, 29, 29, BeltColor::Green)]);
        assert_eq!(b.state.hover_item, Some(7));
    }

    // Covers: specs/ui/control-panel.md §5 r4
    #[test]
    fn a_hovered_belt_slot_paints_its_highlight_rect() {
        use crate::ui::original::hud::{BELT_FILL_BASE, FILL_FILE};
        use crate::ui::panels::control::belt::BeltColor;
        let (u, mut files) = ui();
        files.add(FILL_FILE);
        let w = world(&[(7, mode::BELT, (0, 1, 0, 0), b"hp1 ")], None);
        let mut b = HudBelt {
            parts: parts(),
            ..Default::default()
        };
        let fill = files.id(FILL_FILE).expect("fill file");
        let green = BELT_FILL_BASE + BeltColor::Green as u32;
        let tiles = |out: &[UiDraw]| -> Vec<(i32, i32, u16, u16)> {
            out.iter()
                .filter_map(|d| match d {
                    UiDraw::Image(i)
                        if i.image
                            == (ImageRef {
                                file: fill,
                                frame: green,
                            }) =>
                    {
                        Some((i.clip.x, i.clip.y, i.clip.w, i.clip.h))
                    }
                    _ => None,
                })
                .collect()
        };
        let mut out: Vec<UiDraw> = Vec::new();
        b.draw(&w, &u, &files, (800, 600), false, (0, 0), true, &mut out);
        assert!(tiles(&out).is_empty(), "no hover, no rectangle");
        out.clear();
        b.draw(
            &w,
            &u,
            &files,
            (800, 600),
            false,
            (470, 570),
            true,
            &mut out,
        );
        // 29 x 29 at (461, 562): an 18-high tile and an 11-high tile.
        assert_eq!(tiles(&out), vec![(461, 562, 29, 18), (461, 580, 29, 11)]);
    }

    // Covers: specs/ui/control-panel.md §5 r4
    #[test]
    fn rebinding_a_belt_key_changes_its_label() {
        use crate::controls::{Action, Key, Preset};
        let w = world(&[(7, mode::BELT, (0, 1, 0, 0), b"hp1 ")], None);
        let mut b = HudBelt {
            parts: parts(),
            ..Default::default()
        };
        let label = |b: &mut HudBelt| -> Vec<Vec<u16>> {
            b.draw_list(&w, (800, 600), false, (0, 0), true)
                .into_iter()
                .filter_map(|d| match d {
                    crate::ui::panels::control::belt::BeltDraw::Label(l) => Some(l.text),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(label(&mut b), vec![vec![u16::from(b'2')]]);
        let mut bind = Preset::Dev.bindings().unwrap();
        assert_eq!(bind.inputs(Action::BeltSlot2), &[Key::Digit2]);
        bind.set(Action::BeltSlot2, &[Key::F]);
        b.set_keys(&bind);
        assert_eq!(label(&mut b), vec!["F".encode_utf16().collect::<Vec<_>>()]);
        bind.set(Action::BeltSlot2, &[]);
        b.set_keys(&bind);
        assert!(label(&mut b).is_empty(), "unbound: no label");
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

// Covers: specs/ui/inventory.md §10 r1
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

// Covers: specs/world/cube.md §1
#[test]
fn a_right_press_on_the_cube_sends_0x20_with_the_player_position() {
    let (u, files) = ui();
    let mut w = world(
        &[
            (7, mode::STORED, (0, 0, 0, 1), b"box "),
            (8, mode::STORED, (0, 2, 0, 1), b"hp1 "),
        ],
        None,
    );
    w.units.get_mut(&PLAYER).unwrap().position = Some((10, 20));
    let l = layout();
    // A potion is not opened by a right press.
    assert!(u.right_press(&w, &files, &l, in_cell(2, 0)).is_empty());
    let out = u.right_press(&w, &files, &l, in_cell(0, 0));
    let want = [
        &[0x20u8][..],
        &7u32.to_le_bytes(),
        &10u32.to_le_bytes(),
        &20u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(intents(&out), vec![want]);
    // With a cursor item nothing is used.
    let mut w = world(
        &[
            (7, mode::STORED, (0, 0, 0, 1), b"box "),
            (9, mode::CURSOR, (0, 0, 0, 0), b"hp1 "),
        ],
        Some(9),
    );
    w.units.get_mut(&PLAYER).unwrap().position = Some((10, 20));
    assert!(u.right_press(&w, &files, &l, in_cell(0, 0)).is_empty());
}

// Covers: specs/items/inventory-moves.md §7.11
#[test]
fn a_right_press_on_a_town_portal_scroll_or_tome_sends_0x20() {
    let (u, files) = ui();
    let mut w = world(
        &[
            (7, mode::STORED, (0, 0, 0, 1), b"tsc "),
            (8, mode::STORED, (0, 2, 0, 1), b"tbk "),
        ],
        None,
    );
    w.units.get_mut(&PLAYER).unwrap().position = Some((10, 20));
    let l = layout();
    for (cell, guid) in [(0, 7u32), (2, 8)] {
        let out = u.right_press(&w, &files, &l, in_cell(cell, 0));
        let want = [
            &[0x20u8][..],
            &guid.to_le_bytes(),
            &10u32.to_le_bytes(),
            &20u32.to_le_bytes(),
        ]
        .concat();
        assert_eq!(intents(&out), vec![want]);
    }
}

// Covers: specs/ui/panels.md §12 r3
#[test]
fn the_cube_grid_draws_page_3_and_sends_the_page_3_intents() {
    use crate::ui::panels::cube_items::{cube_present, cube_record, fallback_cube_grid};
    let (u, files) = ui();
    assert_eq!(cube_record(&Screen::R640), 9);
    assert_eq!(cube_record(&Screen::R800), 25);
    let g = fallback_cube_grid(&Screen::R800);
    assert_eq!(u.cube_grid(&Screen::R800), g);
    let w = world(
        &[
            (6, mode::STORED, (0, 0, 0, 1), b"box "),
            // The stream's page is the grid page + 1.
            (7, mode::STORED, (0, 1, 2, 4), b"hp1 "),
            (8, mode::STORED, (0, 0, 0, 1), b"hp1 "),
        ],
        None,
    );
    assert!(cube_present(&w));
    let mut out = Vec::new();
    u.draw_cube(&w, &files, &g, &mut out);
    let file = files.id(&item_file_name("invhp1")).unwrap();
    // Cell (1, 2): (left + 29, top + 58), drawn at top + frame height 29.
    assert_eq!(images(&out), vec![(file, 0, g.left + 29, g.top + 58 + 29)]);
    // Lift the cube item (0x19); a page-0 item is not under the cube mouse.
    let at = Point::new(g.left + 29 + 3, g.top + 58 + 3);
    assert_eq!(
        intents(&u.press_cube(&w, &files, &g, at)),
        vec![vec![0x19, 7, 0, 0, 0]]
    );
    // A cursor item onto the empty cell (0, 0): 0x18 with page 3.
    let w = world(&[(9, mode::CURSOR, (0, 0, 0, 0), b"hp1 ")], Some(9));
    assert!(!cube_present(&w));
    let at = Point::new(g.left + 3, g.top + 3);
    let want = ClientIntent::from_message(&items::insert(9, 0, 0, 3)).0;
    assert_eq!(intents(&u.press_cube(&w, &files, &g, at)), vec![want]);
    assert!(u.press_cube(&w, &files, &g, Point::new(5, 5)).is_empty());
}

/// The (x, y, w, h) of the tiles of fill frame `frame`.
fn tiles(files: &UiFiles, d: &[UiDraw], frame: u32) -> Vec<(i32, i32, u16, u16)> {
    let fill = files.id(crate::ui::original::hud::FILL_FILE).expect("fill");
    d.iter()
        .filter_map(|d| match d {
            UiDraw::Image(i) if i.image.file == fill && i.image.frame == frame => {
                Some((i.clip.x, i.clip.y, i.clip.w, i.clip.h))
            }
            _ => None,
        })
        .collect()
}

/// A cap (requires level 3) worn in the head box, the player at `level`;
/// the mouse at `mouse`.
fn cap_tints(level: i32, mouse: Point) -> (UiFiles, Vec<UiDraw>) {
    let (mut u, mut files) = ui();
    u.art.0.insert(
        *b"cap ",
        ItemArtRow {
            inv_w: 2,
            inv_h: 2,
            inv_file: "invcap".into(),
            flippy_file: String::new(),
        },
    );
    u.tips = Some(crate::ui::item_tip::tests::tips());
    files.add(crate::ui::original::hud::FILL_FILE);
    u.register_files(&mut files);
    let mut w = world(&[(8, mode::BODY, (1, 0, 0, 0), b"cap ")], None);
    w.units.get_mut(&PLAYER).unwrap().stats.insert(12, level);
    let mut l = layout();
    l.equip[1] = EquipBox {
        left: 30,
        top: 40,
        w: 58,
        h: 18,
    };
    let mut out: Vec<UiDraw> = Vec::new();
    u.draw_tints(&w, &files, &l, mouse, &mut out);
    (files, out)
}

// Covers: specs/ui/inventory.md §6 r4, §2 r1
#[test]
fn an_equipped_item_over_the_players_level_paints_the_red_tint() {
    use crate::ui::original::hud::BELT_FILL_BASE;
    let (files, out) = cap_tints(1, Point::new(0, 0));
    assert_eq!(tiles(&files, &out, BELT_FILL_BASE), vec![(30, 40, 58, 18)]);
}

// Covers: specs/ui/inventory.md §6 r4
#[test]
fn a_usable_identified_equipped_item_paints_no_tint() {
    let (files, out) = cap_tints(3, Point::new(0, 0));
    assert!(out.is_empty(), "{} fills", out.len());
    let _ = files;
}

// Covers: specs/ui/inventory.md §6 r4, §2 r1
#[test]
fn a_hovered_equipped_item_paints_the_green_tint() {
    use crate::ui::original::hud::BELT_FILL_BASE;
    let (files, out) = cap_tints(1, Point::new(40, 45));
    assert_eq!(
        tiles(&files, &out, BELT_FILL_BASE + 1),
        vec![(30, 40, 58, 18)]
    );
    assert!(tiles(&files, &out, BELT_FILL_BASE).is_empty());
}

// Covers: specs/ui/inventory.md §3 r3
#[test]
fn grid_items_paint_blue_usable_and_red_refused_tints() {
    use crate::ui::original::hud::BELT_FILL_BASE;
    let (mut u, mut files) = ui();
    u.art.0.insert(
        *b"cap ",
        ItemArtRow {
            inv_w: 1,
            inv_h: 1,
            inv_file: "invcap".into(),
            flippy_file: String::new(),
        },
    );
    u.tips = Some(crate::ui::item_tip::tests::tips());
    files.add(crate::ui::original::hud::FILL_FILE);
    let w = |level| {
        let mut w = world(&[(7, mode::STORED, (0, 2, 3, 1), b"cap ")], None);
        w.units.get_mut(&PLAYER).unwrap().stats.insert(12, level);
        w
    };
    let l = layout();
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    u.draw_tints(&w(3), &files, &l, Point::new(0, 0), &mut ok);
    u.draw_tints(&w(1), &files, &l, Point::new(0, 0), &mut bad);
    // Cell (2, 3): (158, 287), 29 x 29 = an 18-high and an 11-high tile.
    let cell = vec![(158, 287, 29, 18), (158, 305, 29, 11)];
    assert_eq!(tiles(&files, &ok, BELT_FILL_BASE + 2), cell);
    assert_eq!(tiles(&files, &bad, BELT_FILL_BASE), cell);
}

// Covers: specs/ui/inventory.md §2 r1
#[test]
fn the_red_fill_frame_is_the_palette_entry_nearest_to_0x80_0_0() {
    use crate::ui::original::hud::{fill_frames, BELT_FILL_BASE};
    let mut colors = [d2_formats::palette::Rgb::default(); 256];
    colors[9] = d2_formats::palette::Rgb {
        r: 0x80,
        g: 0,
        b: 0,
    };
    let pal = d2_formats::palette::Palette { colors };
    let frames = fill_frames(&pal);
    let red = &frames[BELT_FILL_BASE as usize];
    assert!(red.pixels.iter().all(|&p| p == 9));
}
