// Spec: specs/ui/frontend-credits.md (C5, C6, C8)
//! The cinematics menu on synthetic input.

use std::cell::RefCell;
use std::rc::Rc;

use d2_client::ui::front_end::screens::cinematics::*;
use d2_client::ui::front_end::startup::{MemProgress, RecordVideo, StubVideo};
use d2_client::ui::front_end::*;
use d2_client::ui::geom::Point;

struct Rig {
    f: FrontEnd,
    store: Rc<RefCell<MemProgress>>,
    video: Rc<RefCell<RecordVideo>>,
    h: Rc<RefCell<Handles>>,
}

fn rig(expansion: bool, n: Option<u8>) -> Rig {
    let store = Rc::new(RefCell::new(MemProgress(n)));
    let video = Rc::new(RefCell::new(RecordVideo::default()));
    let h = Rc::new(RefCell::new(Handles {
        store: store.clone(),
        video: video.clone(),
        flushes: 0,
    }));
    let mut reg = Registry::default();
    register_with(&mut reg, h.clone());
    let mut f = FrontEnd::new(expansion, Box::new(true), reg);
    f.start(false, &mut MemProgress(None), &mut StubVideo);
    f.trigger(Trigger::Cinematics);
    assert_eq!(f.current(), CINEMATICS);
    Rig { f, store, video, h }
}

fn click(f: &mut FrontEnd, x: i32, y: i32) {
    let p = Point::new(x, y);
    f.input(FrontInput::Down(p));
    f.input(FrontInput::Up(p));
    f.tick();
}

#[test]
fn levels() {
    for (n, l) in [
        (0x22, 1),
        (0x26, 2),
        (0x62, 3),
        (0x2A, 4),
        (0xB2, 6),
        (0x32, 5),
        (0x23, 7),
    ] {
        assert_eq!(level(n), l, "{n:#x}");
    }
}

#[test]
fn enabled_entries_per_n() {
    for (n, l) in [
        (0x22u8, 1usize),
        (0x26, 2),
        (0x62, 3),
        (0x2A, 4),
        (0x32, 5),
        (0xB2, 6),
        (0x23, 7),
    ] {
        let r = rig(true, Some(n));
        let btn: Vec<_> =
            r.f.controls()
                .iter()
                .filter(|c| c.kind == ControlKind::Button && c.hotkey == 0)
                .collect();
        assert_eq!(btn.len(), 7);
        assert_eq!(btn.iter().filter(|c| c.enabled).count(), l, "exp {n:#x}");
        assert!(btn.iter().filter(|c| !c.enabled).all(|c| c.string_id == 0));
        let r = rig(false, Some(n));
        let btn: Vec<_> =
            r.f.controls()
                .iter()
                .filter(|c| c.kind == ControlKind::Button && c.hotkey == 0)
                .collect();
        assert_eq!(btn.len(), 5);
        assert_eq!(
            btn.iter().filter(|c| c.enabled).count(),
            l.min(5),
            "classic {n:#x}"
        );
        assert!(btn.iter().all(|c| c.string_id != 0));
    }
}

#[test]
fn missing_n_is_level_one_and_writes_default() {
    let r = rig(true, None);
    assert_eq!(r.store.borrow().0, Some(0x22));
    let on =
        r.f.controls()
            .iter()
            .filter(|c| c.kind == ControlKind::Button && c.hotkey == 0 && c.enabled)
            .count();
    assert_eq!(on, 1);
}

#[test]
fn click_plays_and_stays_on_menu_without_changing_n() {
    let mut r = rig(true, Some(0x26));
    click(&mut r.f, 300, 170); // entry 1 (y 181 bottom)
    click(&mut r.f, 300, 215); // entry 2
    click(&mut r.f, 300, 260); // entry 3: disabled
    assert_eq!(
        r.video.borrow().0,
        [
            r"DATA\LOCAL\video\ENG\d2intro640x292.bik",
            r"DATA\LOCAL\video\ENG\Act02start640x292.bik",
        ]
    );
    assert_eq!(r.f.current(), CINEMATICS);
    assert_eq!(r.store.borrow().0, Some(0x26));
    assert_eq!(r.h.borrow().flushes, 4);
}

#[test]
fn cancel_and_esc_return_to_main_menu() {
    let mut r = rig(true, Some(0x22));
    click(&mut r.f, 340, 480);
    assert_eq!(r.f.current(), MAIN_MENU);
    let mut r = rig(false, Some(0x22));
    r.f.input(FrontInput::Key(27));
    r.f.tick();
    assert_eq!(r.f.current(), MAIN_MENU);
}

#[test]
fn in_game_writer() {
    let cases = [
        (0x22u8, 2u32, 0x26u8),
        (0x22, 3, 0x62),
        (0x22, 4, 0x2A),
        (0x22, 5, 0xB2),
        (0x22, 6, 0xB2),
        (0x22, 7, 0x23),
        (0x26, 2, 0x26), // not greater: unchanged
        (0xB2, 5, 0xB2),
        (0x22, 99, 0x22),
    ];
    for (n, id, out) in cases {
        let mut s = MemProgress(Some(n));
        note_video_request(&mut s, id);
        assert_eq!(s.0, Some(out), "{n:#x} id {id}");
    }
    let mut s = MemProgress(None);
    note_video_request(&mut s, 7);
    assert_eq!(s.0, None);
}
