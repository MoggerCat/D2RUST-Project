use super::*;

fn menu_at(m: MenuId) -> OptionsMenu {
    let mut o = OptionsMenu::default();
    o.set_settings(Settings::default());
    o.open();
    o.go(m);
    o.take_events();
    o.take_changed();
    o
}

fn row_of(o: &OptionsMenu, r: Row) -> usize {
    o.rows().iter().position(|d| d.row == r).unwrap()
}

fn pt(x: i32, y: i32) -> Point {
    Point::new(x, y)
}

// Covers: specs/ui/frontend-options.md §o2-menu-records-and-the-tree r5, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r4, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r6
#[test]
fn opens_on_return_to_game_and_the_tree_navigates() {
    let mut o = OptionsMenu::default();
    o.open();
    assert_eq!((o.menu, o.selected), (MenuId::Game, 2));
    assert_eq!(o.pentagram_y(), 285 + 51);
    // Down wraps to Options.
    o.key_down();
    assert_eq!(o.selected, 0);
    assert_eq!(o.take_events(), vec![MenuEvent::CursorPass]);
    o.key_enter();
    assert_eq!((o.menu, o.selected), (MenuId::Options, 4));
    assert_eq!(o.take_events(), vec![MenuEvent::CursorSelect]);
    for (i, m) in [(0, MenuId::Sound), (1, MenuId::Video), (2, MenuId::Automap)] {
        o.selected = i;
        o.key_enter();
        assert_eq!((o.menu, o.selected), (m, m.rows().len() - 1));
        o.key_enter(); // SPrevious
        assert_eq!(o.menu, MenuId::Options);
    }
    o.selected = 3;
    o.take_events();
    o.key_enter();
    assert_eq!(
        o.take_events(),
        vec![MenuEvent::CursorSelect, MenuEvent::ConfigureControls]
    );
    o.selected = 4;
    o.key_enter();
    assert_eq!((o.menu, o.selected), (MenuId::Game, 2));
}

// Covers: specs/ui/frontend-options.md §o3-save-and-exit-game-0x0047f2d0 r1
#[test]
fn save_and_exit_and_return_are_events() {
    let mut o = menu_at(MenuId::Game);
    o.selected = 1;
    o.key_enter();
    assert!(o.take_events().contains(&MenuEvent::SaveAndExit));
    o.selected = 2;
    o.key_enter();
    assert!(o.take_events().contains(&MenuEvent::Close));
}

// Covers: specs/ui/frontend-options.md §o4-draw-0x0047e3d0-while-ui-9-is-open-from-the-ui-draw-0x00456f46 r4
#[test]
fn row_tops_follow_the_position_table() {
    let tops = |m| {
        let o = menu_at(m);
        (0..o.rows().len()).map(|i| o.y_top(i)).collect::<Vec<_>>()
    };
    assert_eq!(tops(MenuId::Game), [185, 235, 285]);
    assert_eq!(tops(MenuId::Options), [135, 185, 235, 285, 335]);
    assert_eq!(tops(MenuId::Sound), [80, 125, 170, 215, 260, 305, 350, 395]);
    assert_eq!(tops(MenuId::Automap), [103, 148, 193, 238, 283, 328, 373]);
}

// Covers: specs/ui/frontend-options.md §o4-draw-0x0047e3d0-while-ui-9-is-open-from-the-ui-draw-0x00456f46 r4
#[test]
fn row_tops_at_640_follow_the_position_table() {
    let tops = |m| {
        let mut o = menu_at(m);
        o.screen = Screen::R640;
        (0..o.rows().len()).map(|i| o.y_top(i)).collect::<Vec<_>>()
    };
    // The 640 × 480 column of the table (h = 320).
    assert_eq!(tops(MenuId::Game), [125, 175, 225]);
    assert_eq!(tops(MenuId::Options), [75, 125, 175, 225, 275]);
    assert_eq!(tops(MenuId::Sound), [20, 65, 110, 155, 200, 245, 290, 335]);
    assert_eq!(tops(MenuId::Automap), [43, 88, 133, 178, 223, 268, 313]);
    let mut o = menu_at(MenuId::Game);
    assert_eq!(o.half(), 400);
    o.screen = Screen::R640;
    assert_eq!(o.half(), 320);
    // Row hit-test follows the same y: the second row's top at 640.
    assert_eq!(o.row_at(o.y_top(1) + 3 + 3), Some(1));
}

// Covers: specs/ui/frontend-options.md §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r4
#[test]
fn up_skips_the_disabled_rows_of_the_sound_menu() {
    let mut o = menu_at(MenuId::Sound);
    assert_eq!(o.selected, 7);
    o.key_up();
    assert_eq!(o.selected, 6); // NPC Speech
    o.key_up();
    assert_eq!(o.selected, 2); // Music: Bias, EAX, 3D Sound disabled
    o.key_up();
    o.key_up(); // Sound, then wrap past the title to SPrevious
    assert_eq!(o.selected, 7);
}

// Covers: specs/ui/frontend-options.md §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r4, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r7
#[test]
fn slider_keys_step_and_clamp() {
    let mut o = menu_at(MenuId::Sound);
    o.selected = row_of(&o, Row::Music);
    assert_eq!(o.value(o.selected), 10);
    o.key_left();
    assert_eq!((o.value(o.selected), o.settings.music_volume), (9, 45));
    assert_eq!(o.take_events(), vec![MenuEvent::CursorPass]);
    assert!(o.take_changed().is_some());
    // Sound at 20: Right changes nothing, no sound.
    o.selected = row_of(&o, Row::Sound);
    assert_eq!(o.value(o.selected), 20);
    o.key_right();
    assert_eq!(o.settings.master_volume, 100);
    assert!(o.take_events().is_empty() && o.take_changed().is_none());
}

// Covers: specs/ui/frontend-options.md §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r4, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r6, §o6-row-effects-apply-0x114-init-0x118-registry-writes-are-reg-dword r8
#[test]
fn choice_keys_wrap_and_enter_cycles() {
    let mut o = menu_at(MenuId::Video);
    o.selected = row_of(&o, Row::LightQuality);
    assert_eq!(o.settings.light_quality, 2);
    o.key_right();
    assert_eq!(o.settings.light_quality, 0);
    o.key_left();
    assert_eq!(o.settings.light_quality, 2);
    let mut a = menu_at(MenuId::Automap);
    a.settings.automap_fade = 3;
    a.selected = row_of(&a, Row::MapFade);
    a.key_enter();
    assert_eq!(a.settings.automap_fade, 0);
    assert_eq!(a.take_events(), vec![MenuEvent::CursorPass]);
}

// Covers: specs/ui/frontend-options.md §o6-row-effects-apply-0x114-init-0x118-registry-writes-are-reg-dword r11, §o6-row-effects-apply-0x114-init-0x118-registry-writes-are-reg-dword r12, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r7
#[test]
fn gamma_and_contrast_mapping() {
    let mut o = menu_at(MenuId::Video);
    o.selected = row_of(&o, Row::Gamma);
    assert_eq!(o.value(o.selected), 10);
    o.key_right();
    assert_eq!((o.value(o.selected), o.settings.gamma), (11, 165));
    // A stored 160 snaps to 155 on game join and is marked for writing.
    let mut j = OptionsMenu::default();
    j.set_settings(Settings {
        gamma: 160,
        ..Settings::default()
    });
    assert_eq!(j.settings.gamma, 155);
    assert!(j.take_changed().is_some());
    // Contrast 100 is position 99; Left gives 98.
    o.selected = row_of(&o, Row::Contrast);
    assert_eq!(o.value(o.selected), 99);
    o.key_left();
    assert_eq!((o.value(o.selected), o.settings.contrast), (98, 98));
}

// Covers: specs/ui/frontend-options.md §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r5
#[test]
fn drag_vectors_on_music() {
    let mut o = menu_at(MenuId::Sound);
    let y = 180; // Music row (top 170)
    for (x, p, vol) in [(352, 0, 0), (400, 4, 20), (617, 20, 100), (618, 20, 100)] {
        let mut m = o.clone();
        m.press(pt(x, y));
        assert_eq!(m.selected, 2);
        assert_eq!((m.value(2), m.settings.music_volume), (p, vol), "x {x}");
    }
    // Press outside the strict bounds: no drag.
    o.press(pt(341, y));
    assert_eq!(o.settings.music_volume, 50);
    // A drag keeps going after the pointer leaves the row (latch).
    o.release(pt(341, y));
    o.press(pt(400, y));
    o.moved(pt(450, 500));
    // x 450: trunc(trunc(98 / 6.625 + 1) / 2) = 7.
    assert_eq!(o.value(2), 7);
}

// Covers: specs/ui/frontend-options.md §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r2
#[test]
fn pointer_rows_follow_the_sound_menu_vectors() {
    let o = menu_at(MenuId::Sound);
    assert_eq!(o.row_at(175), Some(1));
    assert_eq!(o.row_at(180), Some(2));
    assert_eq!(o.row_at(300), None); // EAX disabled
    assert_eq!(o.row_at(80), None);
    assert_eq!(o.row_at(440), None);
}

// Covers: specs/ui/frontend-options.md §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r3, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r5
#[test]
fn click_activates_on_release_and_hover_selects() {
    let mut o = menu_at(MenuId::Game);
    o.moved(pt(400, 200));
    assert_eq!(o.selected, 0);
    o.press(pt(400, 200));
    assert_eq!(o.menu, MenuId::Game); // not yet
    o.release(pt(400, 200));
    assert_eq!(o.menu, MenuId::Options);
    // Released elsewhere: nothing.
    o.press(pt(400, 150));
    o.release(pt(400, 340));
    assert_eq!(o.menu, MenuId::Options);
}

// Party Names is enabled only while Show Party is on (`0x004577B0`).
// Covers: specs/ui/frontend-options.md §o2-menu-records-and-the-tree r3
#[test]
fn party_names_is_enabled_only_while_the_party_is_shown() {
    let mut o = menu_at(MenuId::Automap);
    let names = row_of(&o, Row::MapNames);
    let mut s = Settings {
        automap_party: 1,
        ..Settings::default()
    };
    o.set_settings(s);
    assert!(o.enabled(names));
    s.automap_party = 0;
    o.set_settings(s);
    assert!(!o.enabled(names));
}
