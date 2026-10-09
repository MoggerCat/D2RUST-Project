use super::*;
use crate::bridge::output::Output;
use crate::bridge::world::{ClientUnit, MONSTER, PLAYER};
use crate::controls::Action;
use crate::ui::layout::Screen;
use crate::ui::original::UiConfig;
use crate::ui::panel::ActionId;
use crate::ui::{NoPanelRules, Routed};

const KASHYA: u32 = 150;
const AKARA: u32 = 148;
const CAIN: u32 = 244;
const NPC: u32 = 77;
const ME: UnitKey = UnitKey {
    unit_type: PLAYER,
    guid: 1,
};

struct Strs(Vec<(u16, Vec<u16>)>);

impl Strs {
    fn new() -> Self {
        let w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
        Strs(vec![
            (3381, w("Talk")),
            (3396, w("Trade")),
            (3397, w("Hire")),
            (4142, w("cancel")),
            (4021, w("Identify Items: ")),
            (22696, w("Resurrect %s: %d")),
            (11021, w("Rogue")),
        ])
    }
}

impl StringLookup for Strs {
    fn get(&self, _: &str) -> Option<&[u16]> {
        None
    }
    fn get_id(&self, id: u16) -> Option<&[u16]> {
        self.0.iter().find(|(i, _)| *i == id).map(|(_, t)| &t[..])
    }
}

fn utf(t: &[u16]) -> String {
    String::from_utf16_lossy(t)
}

/// A player at (1000, 1000) level `level` and the NPC of `class` beside.
fn world(class: u32, level: i32) -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut p = ClientUnit::new(ME);
    p.position = Some((1000, 1000));
    p.stats.insert(12, level);
    w.units.insert(ME, p);
    w.local_player = Some(ME);
    let mut n = ClientUnit::new(UnitKey::new(MONSTER, NPC));
    n.class = class;
    n.position = Some((1004, 1000));
    w.units.insert(n.key, n);
    w
}

fn setup() -> (OriginalUi, UiRoot) {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: true,
    };
    let ui = OriginalUi::new(config, None).unwrap();
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    ui.install(&mut root).unwrap();
    (ui, root)
}

/// One event through the root and the original UI.
fn send(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld, e: UiEvent) -> Routed {
    let s = Strs::new();
    let ctx = UiCtx {
        tick: 0,
        world: w,
        strings: &s,
    };
    ui.before_event(e, w);
    let r = root.dispatch(e, &ctx);
    ui.after_event(root, e, r).unwrap();
    r
}

fn click(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld, at: Point) {
    let button = PointerButton::Left;
    send(ui, root, w, UiEvent::Press { button, at });
    send(ui, root, w, UiEvent::Release { button, at });
}

fn open(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld, class: u32, level: i32) {
    ui.open_npc_menu(NPC, class, level, w);
    ui.npc_menu_poll(w, root, &Strs::new());
}

fn bytes(i: &ClientIntent) -> Vec<u8> {
    i.0.clone()
}

// The menu is ui 8 (the root mirrors it), Kashya's build sends the
// hire-list request 0x38 [3][NPC][player] (§2.2), and the box is the
// auto-laid-out spec box above the NPC: the name item (color 4, not
// selectable), the options, then `lowercasecancel`.
// Covers: specs/ui/menus.md §2 r1, §2 r2, §2 r4, §2 r6; specs/ui/panels-2.md §14 r2
#[test]
fn kashyas_menu_is_ui_8_with_the_hire_request_and_the_box_above_her() {
    let (mut ui, mut root) = setup();
    let w = world(KASHYA, 10);
    open(&mut ui, &mut root, &w, KASHYA, 10);
    assert!(ui.is_open(8), "SetUIState(8, on)");
    assert!(root.open_panels().contains(&NPC_MENU_PANEL));
    let mut want = vec![0x38, 3, 0, 0, 0];
    want.extend_from_slice(&NPC.to_le_bytes());
    want.extend_from_slice(&ME.guid.to_le_bytes());
    assert_eq!(
        root.take_intents().iter().map(bytes).collect::<Vec<_>>(),
        vec![want]
    );
    let m = ui.npc_menu().expect("the box is built");
    let kinds: Vec<_> = m.rows.iter().map(|r| r.kind).collect();
    // Level 10 > 7: hire in slot 1 (§14.2).
    assert_eq!(
        kinds,
        vec![Some(OptionKind::Talk), Some(OptionKind::Hire), None]
    );
    let (r, items) = ui.npc_menu_box().unwrap();
    let texts: Vec<(String, u8)> = items.iter().map(|(t, c)| (utf(t), *c)).collect();
    assert_eq!(
        texts,
        vec![
            (String::new(), 4),
            ("Talk".into(), 0),
            ("Hire".into(), 0),
            ("cancel".into(), 0)
        ]
    );
    // §2.4 with no fonts (8 px a unit): w = 8 × 6 + 20, h = 21 + 3 × 15 +
    // 15; x = anchor x − w / 2, y = anchor y − h / 3.
    let (ax, ay) = npc_anchor(&w, NPC, 0).unwrap();
    assert_eq!((r.r - r.l, r.b - r.t), (68, 81));
    assert_eq!((r.l, r.t), (ax - 34, ay - 27));
}

// Cancel ends the interaction: C→S 0x30 [1][GUID] and SetUIState(8, off).
// Covers: specs/ui/menus.md §2 r2; specs/ui/panels-2.md §14 r9
#[test]
fn cancel_sends_0x30_and_turns_ui_8_off() {
    let (mut ui, mut root) = setup();
    let w = world(AKARA, 1);
    open(&mut ui, &mut root, &w, AKARA, 1);
    assert!(root.take_intents().is_empty(), "no hire request for Akara");
    let p = ui.npc_menu_row_point(2).unwrap();
    click(&mut ui, &mut root, &w, p);
    let mut want = vec![0x30, 1, 0, 0, 0];
    want.extend_from_slice(&NPC.to_le_bytes());
    assert_eq!(
        root.take_intents().iter().map(bytes).collect::<Vec<_>>(),
        vec![want]
    );
    assert!(!ui.is_open(8));
    assert!(ui.npc_menu().is_none());
}

// Esc (and Space) with the NPC menu up: the box consumes the key and runs
// p1, the interaction ends (C->S 0x30), and the game menu stays shut.
// Covers: specs/ui/panels-3.md §28 r2
// Covers: specs/ui/frontend-options.md §O1 r2
#[test]
fn esc_ends_the_interaction_and_does_not_open_the_game_menu() {
    for key in [Action::GameMenu, Action::ClearScreen] {
        let (mut ui, mut root) = setup();
        let w = world(AKARA, 1);
        open(&mut ui, &mut root, &w, AKARA, 1);
        root.take_intents();
        send(
            &mut ui,
            &mut root,
            &w,
            UiEvent::Action(ActionId(key.index() as u16)),
        );
        let mut want = vec![0x30, 1, 0, 0, 0];
        want.extend_from_slice(&NPC.to_le_bytes());
        assert_eq!(
            root.take_intents().iter().map(bytes).collect::<Vec<_>>(),
            vec![want],
            "{key:?}"
        );
        assert!(!ui.is_open(8));
        assert!(!ui.is_open(9), "the game menu stays shut");
        assert!(ui.npc_menu().is_none());
    }
}

// With ui 8 open the gate refuses the inventory hot key (C[8][1] = 2).
// Covers: specs/ui/panels.md §3 r3
#[test]
fn the_inventory_key_is_refused_while_the_menu_is_up() {
    let (mut ui, mut root) = setup();
    let w = world(AKARA, 1);
    open(&mut ui, &mut root, &w, AKARA, 1);
    let a = UiEvent::Action(ActionId(Action::ToggleInventory.index() as u16));
    send(&mut ui, &mut root, &w, a);
    assert!(!ui.is_open(1), "refused by the NPC menu");
}

// An NPC class without a record gets record 0 (Akara's talk, trade).
// Covers: specs/ui/panels-2.md §14 r7
#[test]
fn a_class_without_a_record_gets_record_0() {
    let (mut ui, mut root) = setup();
    let w = world(999, 1);
    open(&mut ui, &mut root, &w, 999, 1);
    let kinds: Vec<_> = ui.npc_menu().unwrap().rows.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![Some(OptionKind::Talk), Some(OptionKind::Trade), None]
    );
}

// Without the NPC in the model the build ends the interaction (§2.2).
// Covers: specs/ui/menus.md §2 r2
#[test]
fn no_npc_ends_the_interaction() {
    let (mut ui, mut root) = setup();
    let mut w = world(AKARA, 1);
    w.units.remove(&UnitKey::new(MONSTER, NPC));
    open(&mut ui, &mut root, &w, AKARA, 1);
    assert!(ui.npc_menu().is_none());
    assert!(!ui.is_open(8));
    assert_eq!(root.take_intents()[0].0[0], 0x30);
}

// The Resurrect slot reads `hireresurrect2` with the mercenary's name
// (id 0x421 → string 11021) and the cost.
// Covers: specs/ui/menus.md §2 r3
#[test]
fn resurrect_row_is_captioned_by_hireresurrect2_with_the_name() {
    let (mut ui, mut root) = setup();
    let mut w = world(KASHYA, 10);
    w.expansion = 1;
    {
        let mut st = ui.npcm.borrow_mut();
        st.resurrect = Some(500);
        st.merc_name = 0x421;
    }
    open(&mut ui, &mut root, &w, KASHYA, 10);
    let (_, items) = ui.npc_menu_box().unwrap();
    let texts: Vec<String> = items.iter().map(|(t, _)| utf(t)).collect();
    assert!(
        texts.contains(&"Resurrect Rogue: 500".to_string()),
        "{texts:?}"
    );
    let m = ui.npc_menu().unwrap();
    let r = m
        .rows
        .iter()
        .find(|r| r.kind == Some(OptionKind::Resurrect))
        .unwrap();
    assert_eq!(r.cost, Some(500));
}

// Identify sends C→S 0x34 and waits (state 10); S→C 0x2A result 3 resets
// Cain's counts and rebuilds the menu, whose identify caption reads the
// count again.
// Covers: specs/ui/panels-2.md §14 r10; specs/client/msg-ui.md §18 r2; specs/ui/menus.md §2 r3
#[test]
fn identify_waits_for_0x2a_and_the_result_rebuilds_the_menu() {
    let (mut ui, mut root) = setup();
    let w = world(CAIN, 1);
    ui.open_npc_menu_with(NPC, CAIN, 1, 2, &w);
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    let m = ui.npc_menu().unwrap();
    let k = m
        .rows
        .iter()
        .position(|r| r.kind == Some(OptionKind::Identify))
        .unwrap();
    assert_eq!(m.rows[k].cost, Some(200));
    let (_, items) = ui.npc_menu_box().unwrap();
    assert!(items.iter().any(|(t, _)| utf(t) == "Identify Items: 200"));
    let p = ui.npc_menu_row_point(k).unwrap();
    click(&mut ui, &mut root, &w, p);
    let mut want = vec![0x34];
    want.extend_from_slice(&NPC.to_le_bytes());
    assert_eq!(
        root.take_intents().iter().map(bytes).collect::<Vec<_>>(),
        vec![want]
    );
    assert_eq!(ui.npc_menu_state(), MENU_WAITING);
    assert!(ui.npc_menu().is_none(), "the box closed for the send");
    let mut b = [0u8; 15];
    b[0] = 0x2A;
    b[2] = 3;
    ui.apply_output(&Output::NpcTransaction { bytes: b, gold: 0 }, &w)
        .unwrap();
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    assert_eq!(ui.npc_menu_state(), 1);
    assert!(ui.npc_menu().is_some(), "the menu is built again");
    assert!(ui.is_open(8));
}

// Talk closes the box (§14.9); the talk's end with flag 1 builds the menu
// again (§14.8).
// Covers: specs/ui/panels-2.md §14 r8, §14 r9
#[test]
fn talk_closes_the_box_and_its_end_rebuilds_the_menu() {
    let (mut ui, mut root) = setup();
    let w = world(AKARA, 1);
    open(&mut ui, &mut root, &w, AKARA, 1);
    let p = ui.npc_menu_row_point(0).unwrap();
    click(&mut ui, &mut root, &w, p);
    let m = ui.npc_menu().unwrap();
    assert!(m.talking);
    assert!(ui.npc_menu_box().is_none());
    assert!(root.take_intents().is_empty(), "talk sends nothing");
    let p = ui.npc_topic_cancel_point().expect("the topic box");
    click(&mut ui, &mut root, &w, p);
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    assert!(ui.npc_menu_box().is_some());
    assert!(!ui.npc_menu().unwrap().talking);
}

// Hire opens the hire list (the box closes); its Back builds the NPC
// menu again (§3.2).
// Covers: specs/ui/menus.md §3 r1, §3 r2
#[test]
fn hire_opens_the_list_and_back_rebuilds_the_menu() {
    let (mut ui, mut root) = setup();
    let w = world(KASHYA, 10);
    open(&mut ui, &mut root, &w, KASHYA, 10);
    root.take_intents();
    let p = ui.npc_menu_row_point(1).unwrap();
    click(&mut ui, &mut root, &w, p);
    assert_eq!(ui.hire_list().up, Some(NPC));
    assert!(ui.npc_menu_box().is_none());
    // Back: the box's Back item at (W − 490) / 2, y = (H − 40) / 2 − 195
    // + 21 + 315.
    click(&mut ui, &mut root, &w, Point::new(400, 85 + 21 + 315 - 2));
    assert_eq!(ui.hire_list().up, None);
    assert!(root.take_intents().is_empty());
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    assert!(ui.npc_menu_box().is_some(), "Back built the menu again");
}

// A press outside the box ends the interaction (d2rs-own until the
// window handlers are specified) and the selection follows the pointer.
// Covers: specs/ui/menus.md §2 r5
#[test]
fn the_pointer_selects_and_an_outside_press_ends_the_chat() {
    let (mut ui, mut root) = setup();
    let w = world(AKARA, 1);
    open(&mut ui, &mut root, &w, AKARA, 1);
    assert_eq!(
        ui.npcm.borrow().bx.as_ref().unwrap().selected,
        1,
        "the first selectable item"
    );
    let p = ui.npc_menu_row_point(1).unwrap();
    send(&mut ui, &mut root, &w, UiEvent::CursorMoved(p));
    assert_eq!(ui.npcm.borrow().bx.as_ref().unwrap().selected, 2);
    // Style 1: the selected item draws in color 3.
    let s = Strs::new();
    let ctx = UiCtx {
        tick: 0,
        world: &w,
        strings: &s,
    };
    let mut out: Vec<UiDraw> = Vec::new();
    root.draw(&ctx, &mut out);
    let trade = out
        .iter()
        .find_map(|d| match d {
            UiDraw::Text(t) if utf(&t.text) == "Trade" => Some(t.style.color),
            _ => None,
        })
        .unwrap();
    assert_eq!(trade, 3);
    click(&mut ui, &mut root, &w, Point::new(5, 5));
    assert_eq!(root.take_intents()[0].0[0], 0x30);
    assert!(!ui.is_open(8));
}

// A player who has a hireling: the hire row opens the confirm dialog
// (kind 5, `VerifyTransaction9`, Yes / No) instead of sending; No brings
// the hire list back, Yes sends C→S 0x36 and opens the waiting note.
// Covers: specs/ui/menus.md §3 r4, §4 r4, §2 r7
#[test]
fn hiring_over_a_hireling_asks_first() {
    let (mut ui, mut root) = setup();
    let mut w = world(KASHYA, 10);
    w.expansion = 1;
    w.pets.push(crate::bridge::world::PetRecord {
        class: 271,
        pet_type: crate::bridge::world::PET_HIRELING,
        pet: 900,
        owner: ME.guid,
        f1c: 100,
        gone: false,
        extra: None,
    });
    open(&mut ui, &mut root, &w, KASHYA, 10);
    root.take_intents();
    ui.apply_output(&Output::HireListReset, &w).unwrap();
    ui.apply_output(
        &Output::HireOffer {
            name: 3000,
            seed: 5,
        },
        &w,
    )
    .unwrap();
    let p = ui.npc_menu_row_point(1).unwrap();
    click(&mut ui, &mut root, &w, p);
    assert_eq!(ui.hire_list().up, Some(NPC));
    // Row 0 of the list at 800 × 600: list y 120.
    click(&mut ui, &mut root, &w, Point::new(200, 120 + 8));
    assert!(root.take_intents().is_empty(), "nothing sent yet");
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    let (kind, _) = ui.shop_state().confirm().expect("the confirm dialog");
    assert_eq!(kind, TxKind::Hire);
    let no = ui.shop_confirm_point(false).unwrap();
    click(&mut ui, &mut root, &w, no);
    assert!(ui.shop_state().confirm().is_none());
    assert_eq!(ui.hire_list().up, Some(NPC), "No: the hire list again");
    assert!(root.take_intents().is_empty());
    click(&mut ui, &mut root, &w, Point::new(200, 120 + 8));
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    let yes = ui.shop_confirm_point(true).unwrap();
    click(&mut ui, &mut root, &w, yes);
    assert_eq!(
        root.take_intents().iter().map(bytes).collect::<Vec<_>>(),
        vec![crate::ui::hire_list::hire_intent(NPC, 3000).0]
    );
    ui.npc_menu_poll(&w, &mut root, &Strs::new());
    assert_eq!(ui.npc_menu_state(), MENU_WAITING);
    assert!(ui.npc_waiting_note());
}

// The box background is `0x0046EFD0(x, y, w, h, 0, 1)`: the sink's
// rectangle over the box, color 0, mode 1 (no cel).
// Covers: specs/ui/menus.md §2 r5
#[test]
fn the_box_frame_is_the_dark_rectangle() {
    let (mut ui, mut root) = setup();
    let w = world(AKARA, 1);
    open(&mut ui, &mut root, &w, AKARA, 1);
    let (r, _) = ui.npc_menu_box().unwrap();
    let s = Strs::new();
    let ctx = UiCtx {
        tick: 0,
        world: &w,
        strings: &s,
    };
    let mut out: Vec<UiDraw> = Vec::new();
    root.draw(&ctx, &mut out);
    let want = RectRequest::sized(r.l, r.t, r.r - r.l, r.b - r.t, 0, 1);
    assert!(out.contains(&UiDraw::Rect(want)), "{out:?}");
}
