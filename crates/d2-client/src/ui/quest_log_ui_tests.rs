// Spec: specs/world/quests-status.md (§3, §4, §5), specs/world/quests.md (§6.2)
//! The quest log panel on a synthetic model: Q opens it and asks for the
//! quest data; the status list, the flag records and the counters of the
//! quest messages become rows, icons and text.

use super::quest_log_ui::{flags_of, request_quest_data};
use super::*;
use crate::bridge::output::Output;
use crate::bridge::world::{ClientUnit, UnitKey, PLAYER};
use crate::ui::draw::UiDraw;
use crate::ui::{ActionId, NoPanelRules, NoStrings, StringLookup, UiCtx, UiEvent};

struct Strings;

impl StringLookup for Strings {
    fn get(&self, _: &str) -> Option<&[u16]> {
        None
    }
    fn get_id(&self, id: u16) -> Option<&'static [u16]> {
        // Leaked once per call: a test table of three strings.
        let s: &str = match id {
            3714 => "Den of Evil",
            3739 => "one left",
            _ => return None,
        };
        Some(Box::leak(
            s.encode_utf16().collect::<Vec<_>>().into_boxed_slice(),
        ))
    }
}

fn world() -> ClientWorld {
    let mut w = ClientWorld::default();
    let key = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(key);
    u.mode = 1;
    w.units.insert(key, u);
    w.local_player = Some(key);
    w
}

fn setup() -> (OriginalUi, UiRoot) {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: false,
    };
    let ui = OriginalUi::new(config, None).unwrap();
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    ui.install(&mut root).unwrap();
    (ui, root)
}

fn press_q(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld) {
    let e = UiEvent::Action(ActionId(Action::ToggleQuests.index() as u16));
    let ctx = UiCtx {
        tick: 0,
        world: w,
        strings: &NoStrings,
    };
    ui.before_event(e, w);
    let r = root.dispatch(e, &ctx);
    ui.after_event(root, e, r).unwrap();
}

fn drawn(ui: &OriginalUi, root: &UiRoot, w: &ClientWorld) -> (Vec<(String, u32)>, Vec<String>) {
    let ctx = UiCtx {
        tick: 0,
        world: w,
        strings: &Strings,
    };
    let mut out: Vec<UiDraw> = Vec::new();
    root.draw(&ctx, &mut out);
    let files = ui.files();
    let mut images = Vec::new();
    let mut texts = Vec::new();
    for d in out {
        match d {
            UiDraw::Image(i) => {
                images.push((files.name(i.image.file).unwrap().to_string(), i.image.frame))
            }
            UiDraw::Text(t) => texts.push(String::from_utf16_lossy(&t.text)),
        }
    }
    (images, texts)
}

// Covers: specs/world/quests.md §6.2
#[test]
fn q_opens_the_log_and_asks_for_the_quest_data_once() {
    let (mut ui, mut root) = setup();
    let w = world();
    press_q(&mut ui, &mut root, &w);
    assert!(ui.is_open(0x0F));
    assert_eq!(root.take_intents(), [request_quest_data()]);
    press_q(&mut ui, &mut root, &w);
    assert!(!ui.is_open(0x0F));
    assert!(root.take_intents().is_empty(), "closing asks for nothing");
}

// Covers: specs/world/quests-status.md §3 r2, §4 r7, §5 r4
#[test]
fn a_started_den_of_evil_draws_its_icon_title_and_count_text() {
    let (mut ui, mut root) = setup();
    let w = world();
    let mut status = [0u8; 41];
    status[1] = 3;
    ui.apply_output(&Output::QuestLog { status }, &w).unwrap();
    ui.apply_output(
        &Output::QuestSpecial {
            code: 1,
            words: [1, 0, 0, 0, 0, 0],
        },
        &w,
    )
    .unwrap();
    press_q(&mut ui, &mut root, &w);
    let (images, texts) = drawn(&ui, &root, &w);
    assert!(images.contains(&("menu\\questbackground".to_string(), 0)));
    assert!(
        images.contains(&("menu\\a1q1".to_string(), 0)),
        "the in-progress icon: {images:?}"
    );
    assert!(texts.contains(&"Den of Evil".to_string()), "{texts:?}");
    // Status 3 with one monster left: text 3739 (§4 r7.1).
    assert!(texts.contains(&"one left".to_string()), "{texts:?}");
}

#[test]
fn flags_of_reads_48_little_endian_slots() {
    let mut r = [0u8; 96];
    r[2] = 0x04;
    r[3] = 0x80;
    let f = flags_of(&r);
    assert_eq!(f.word(1), 0x8004);
    assert!(f.bit(1, 2) && f.bit(1, 15) && !f.bit(0, 0));
}
