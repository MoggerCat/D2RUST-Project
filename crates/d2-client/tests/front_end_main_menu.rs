// Spec: specs/ui/frontend-menus.md (§F1.4, §F1.5)
//! The main menu on synthetic input, no game files.

use d2_client::ui::front_end::screens::main_menu::*;
use d2_client::ui::front_end::*;
use d2_client::ui::geom::Point;

fn menu(expansion: bool, saves: bool) -> FrontEnd {
    let mut f = FrontEnd::with_screens(expansion, Box::new(saves));
    f.start(
        false,
        &mut startup::MemProgress(Some(0x22)),
        &mut startup::RecordVideo::default(),
    );
    assert_eq!(f.current(), MAIN_MENU);
    f
}

fn click(f: &mut FrontEnd, x: i32, y: i32) {
    let p = Point::new(x, y);
    f.input(FrontInput::Down(p));
    f.input(FrontInput::Up(p));
    f.tick();
}

/// (string, x, y, w, h) of each button, in order.
fn buttons(f: &FrontEnd) -> Vec<(u32, i32, i32, u16, u16, bool)> {
    f.controls()
        .iter()
        .filter(|c| c.kind == ControlKind::Button)
        .map(|c| (c.string_id, c.x, c.y, c.w, c.h, c.enabled))
        .collect()
}

#[test]
fn expansion_layout_matches_table() {
    let f = menu(true, true);
    assert_eq!(
        buttons(&f),
        [
            // Battle.net, gateway, Other Multiplayer: enabled (§F1.4 r2:
            // disabled only when `0x004FAC90` ≠ 0; the 1.14d screenshot
            // draws them enabled), with no action (Phase 7+).
            (5106, 264, 324, 272, 35, true),
            (5107, 264, 366, 272, 35, true),
            (0, 264, 391, 272, 25, true),
            (5108, 264, 433, 272, 35, true),
            (5110, 264, 528, 135, 25, true),
            (5111, 402, 528, 135, 25, true),
            (5109, 264, 568, 272, 35, true),
        ]
    );
}

#[test]
fn classic_layout_is_100_higher_for_first_four() {
    let f = menu(false, true);
    let ys: Vec<i32> = buttons(&f).iter().map(|b| b.2).collect();
    assert_eq!(ys, [224, 266, 291, 333, 528, 528, 568]);
    let first = &f.draw()[0];
    assert!(matches!(first, DrawItem::Art { file, .. } if *file == BACKGROUND_CLASSIC));
}

#[test]
fn version_text_and_draw_order() {
    let f = menu(true, true);
    let d = f.draw();
    // The background: 12 tiles of 256 × 256 (§F1.2, 4 × 3), frames 0–11.
    for (t, it) in d[..12].iter().enumerate() {
        assert!(
            matches!(it, DrawItem::Art { file, frame, .. } if *file == BACKGROUND && *frame == t as u32),
            "{it:?}"
        );
    }
    assert!(matches!(&d[12], DrawItem::Art { file, .. } if *file == LOGO_LEFT));
    // §F1.5 r2: per logo half, the black base and then its fire overlay.
    assert!(matches!(&d[13], DrawItem::Blend { mode: 3, .. }));
    assert!(matches!(&d[14], DrawItem::Art { file, .. } if *file == LOGO_RIGHT));
    assert!(matches!(&d[15], DrawItem::Blend { mode: 3, .. }));
    match d.last().unwrap() {
        DrawItem::Text { text, .. } => assert_eq!(text, "v 1.14d"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn clicks_route_and_disabled_buttons_ignore() {
    // Disabled: Battle.net, gateway, Other Multiplayer.
    for y in [350, 380, 420] {
        let mut f = menu(true, true);
        click(&mut f, 300, y);
        assert_eq!(f.current(), MAIN_MENU, "y {y}");
    }
    let mut f = menu(true, true);
    click(&mut f, 300, 310);
    assert_eq!(f.current(), CHAR_SELECT);
    let mut f = menu(true, false);
    click(&mut f, 300, 310);
    assert_eq!(f.current(), CHAR_CREATE);
    let mut f = menu(true, true);
    click(&mut f, 300, 515);
    assert_eq!(f.current(), CREDITS);
    let mut f = menu(true, true);
    click(&mut f, 450, 515);
    assert_eq!(f.current(), CINEMATICS);
    let mut f = menu(true, true);
    click(&mut f, 300, 550);
    assert_eq!(f.outcome(), Some(Outcome::Exit));
    let mut f = menu(true, true);
    f.input(FrontInput::Key(27));
    f.tick();
    assert_eq!(f.outcome(), Some(Outcome::Exit));
}

#[test]
fn button_art_states() {
    let f = menu(true, true);
    let c = &f.controls()[3];
    assert_eq!(
        (button_frame(c, 0, false), button_frame(c, 1, false)),
        (0, 1)
    );
    assert_eq!((button_frame(c, 0, true), button_frame(c, 1, true)), (2, 3));
    // A disabled button never shows its pressed frames (r4).
    let mut dis = f.controls()[4].clone();
    dis.enabled = false;
    assert_eq!(button_frame(&dis, 0, true), 0);
}

#[test]
fn logo_fire_frames_loop_0_to_28() {
    let mut f = menu(true, true);
    let mut seen = Vec::new();
    for _ in 0..60 {
        f.tick();
        // d[12]: the left logo half, after the 12 background tiles.
        if let DrawItem::Art { frame, .. } = f.draw()[12] {
            seen.push(frame);
        }
    }
    // Built at tick 0; frame = tick mod 29, never 29.
    assert_eq!(seen[0], 1);
    assert_eq!(seen[27], 28);
    assert_eq!(seen[28], 0);
    assert!(seen.iter().all(|&x| x < 29));
}
