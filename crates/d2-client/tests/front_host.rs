// Spec: specs/ui/frontend-menus.md (§F1.3), specs/ui/frontend-credits.md (C0)
//! The front-end host headless: a minimal app reaches the main menu from
//! the trademark, then the game start from the flow; no game files.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use d2_client::app::front_host::{
    add_front_end, compose, frame_point, vk, FrontHost, HEIGHT, WIDTH,
};
use d2_client::ui::front_end::{FrontInput, Outcome, Trigger, MAIN_MENU, TRADEMARK};

fn app(first: bool) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
    ))
    .init_asset::<Image>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        40,
    )));
    // The real systems: input feed, 40 ms driver, draw, finish.
    add_front_end(&mut app, FrontHost::new(true, Box::new(true), None, first));
    app
}

#[test]
fn reaches_main_menu_then_game_start() {
    let mut a = app(true);
    assert_eq!(a.world().non_send::<FrontHost>().front.current(), TRADEMARK);
    // Click on the trademark image (Continue on button-up).
    {
        let mut h = a.world_mut().non_send_mut::<FrontHost>();
        let p = d2_client::ui::geom::Point::new(100, 100);
        h.front.input(FrontInput::Down(p));
        h.front.input(FrontInput::Up(p));
    }
    a.update();
    a.update();
    assert_eq!(a.world().non_send::<FrontHost>().front.current(), MAIN_MENU);
    a.world_mut()
        .non_send_mut::<FrontHost>()
        .front
        .trigger(Trigger::SinglePlayer);
    a.world_mut()
        .non_send_mut::<FrontHost>()
        .front
        .trigger(Trigger::Ok);
    a.update();
    a.update();
    let out = a.world().non_send::<FrontHost>().outcome;
    assert!(
        matches!(out, Some(Outcome::GameLoad(g)) if !g.new_character),
        "{out:?}"
    );
}

#[test]
fn later_entry_is_the_main_menu_and_esc_exits() {
    let mut a = app(false);
    assert_eq!(a.world().non_send::<FrontHost>().front.current(), MAIN_MENU);
    a.world_mut()
        .non_send_mut::<FrontHost>()
        .front
        .input(FrontInput::Key(vk(KeyCode::Escape).unwrap()));
    a.update();
    a.update();
    a.update();
    assert_eq!(
        a.world().non_send::<FrontHost>().outcome,
        Some(Outcome::Exit)
    );
}

#[test]
fn input_mapping_and_compose() {
    assert_eq!(vk(KeyCode::KeyA), Some(b'A' as u16));
    assert_eq!(vk(KeyCode::Digit5), Some(b'5' as u16));
    assert_eq!(vk(KeyCode::Enter), Some(13));
    let p = frame_point(Vec2::new(640.0, 480.0), Vec2::new(1280.0, 960.0));
    assert_eq!((p.x, p.y), (400, 300));
    let px = compose(&[], None);
    assert_eq!(px.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_eq!(&px[..4], &[0, 0, 0, 255]);
}
