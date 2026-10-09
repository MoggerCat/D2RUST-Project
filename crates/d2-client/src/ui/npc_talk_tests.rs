use super::*;
use crate::bridge::output::Output;
use crate::bridge::world::{ClientUnit, MONSTER, PLAYER};
use crate::ui::layout::Screen;
use crate::ui::original::UiConfig;
use crate::ui::{NoPanelRules, UiRoot};

const AKARA: u32 = 148;
const CHARSI: u32 = 154;
const NPC: u32 = 77;
const ME: UnitKey = UnitKey {
    unit_type: PLAYER,
    guid: 1,
};
/// The gossip text: speed line "4", then two lines.
const GOSSIP: u16 = 900;

struct Strs(Vec<(u16, Vec<u16>)>);

fn w16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl Strs {
    fn new() -> Self {
        let mut v = vec![
            (3381, w16("Talk")),
            (3399, w16("introduction")),
            (3395, w16("gossip")),
            (3400, w16("cancel")),
            (3724, w16("Invalid Quest Value")),
            (GOSSIP, w16("4\nHello\nThere")),
        ];
        // Gossip plays a text record of Akara's intro entry (facts): every
        // one of them reads as the same two lines here.
        for r in &crate::ui::messages::npc_facts::intro_table().entries[0].records {
            v.push((r.text(), w16("4\nHello\nThere")));
        }
        Strs(v)
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

fn world(class: u32) -> ClientWorld {
    let mut w = ClientWorld::default();
    let mut p = ClientUnit::new(ME);
    p.position = Some((1000, 1000));
    p.stats.insert(12, 1);
    // Mode 1 (neutral): alive for the gate (`panels.md` §2.5).
    p.mode = 1;
    w.units.insert(ME, p);
    w.local_player = Some(ME);
    let mut n = ClientUnit::new(UnitKey::new(MONSTER, NPC));
    n.class = class;
    n.position = Some((1004, 1000));
    w.units.insert(n.key, n);
    w.frames = 100;
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

fn send(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld, e: UiEvent) {
    let s = Strs::new();
    let ctx = UiCtx {
        tick: w.frames,
        world: w,
        strings: &s,
    };
    ui.before_event(e, w);
    let r = root.dispatch(e, &ctx);
    ui.after_event(root, e, r).unwrap();
}

fn click(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld, at: Point) {
    let button = PointerButton::Left;
    send(ui, root, w, UiEvent::Press { button, at });
    send(ui, root, w, UiEvent::Release { button, at });
}

fn poll(ui: &mut OriginalUi, root: &mut UiRoot, w: &ClientWorld) {
    ui.npc_menu_poll(w, root, &Strs::new());
}

/// S→C 0x27 type 1 for the NPC: entries (kind, string id).
fn text_list(entries: &[(u8, u16)]) -> Output {
    let mut b = [0u8; 40];
    b[0] = 0x27;
    b[1] = 1;
    b[2..6].copy_from_slice(&NPC.to_le_bytes());
    b[6] = entries.len() as u8;
    for (k, &(kind, id)) in entries.iter().enumerate() {
        b[6 + 2 + 4 * k] = kind;
        b[6 + 4 + 4 * k..6 + 6 + 4 * k].copy_from_slice(&id.to_le_bytes());
    }
    Output::NpcText {
        bytes: b,
        present: true,
        object_class: 0,
    }
}

fn texts(v: &[Vec<u16>]) -> Vec<String> {
    v.iter().map(|t| String::from_utf16_lossy(t)).collect()
}

/// Akara's menu with a text list of one kind-0 and one kind-2 entry,
/// then Talk.
fn talk_to_akara() -> (OriginalUi, UiRoot, ClientWorld) {
    let (mut ui, mut root) = setup();
    let w = world(AKARA);
    ui.apply_output(&text_list(&[(0, GOSSIP), (2, 501)]), &w)
        .unwrap();
    ui.open_npc_menu(NPC, AKARA, 1, &w);
    poll(&mut ui, &mut root, &w);
    let p = ui.npc_menu_row_point(0).unwrap();
    click(&mut ui, &mut root, &w, p);
    (ui, root, w)
}

// Talk opens the topic box (§6 r3): the caption, "introduction" (Akara
// has one), "gossip", one item per kind-2 entry (captioned through the
// 527-pair table: not in the repository, so the not-found 3724), cancel.
// Covers: specs/ui/messages.md §6 r3; specs/ui/panels-2.md §14 r9
#[test]
fn talk_opens_the_topic_box_of_the_text_list() {
    let (ui, mut root, _) = talk_to_akara();
    assert!(ui.npc_menu_box().is_none(), "the menu box closed");
    assert!(root.take_intents().is_empty(), "talk sends nothing");
    let topics = ui.npc_topics().expect("the topic box");
    assert_eq!(
        texts(&topics),
        [
            "Talk",
            "introduction",
            "gossip",
            "Invalid Quest Value",
            "cancel"
        ]
    );
}

// A topic plays in the dialog panel (§7): top centre ((W − 325) / 2, 12),
// the first line is the speed and is not shown; its backing rectangle;
// the scroll advances with the clock; a button press skips (§7 r7) and
// the end callback brings the topic box back.
// Covers: specs/ui/messages.md §7 r1, §7 r2, §7 r3, §7 r4, §7 r7
#[test]
fn a_topic_plays_in_the_dialog_panel_and_a_press_skips_it() {
    let (mut ui, mut root, mut w) = talk_to_akara();
    // "gossip" (the second selectable item) plays a text record of the
    // NPC's intro entry (§6 r4–r5), not the list's kind-0 entry.
    let p = ui.npc_topic_point(1).unwrap();
    click(&mut ui, &mut root, &w, p);
    assert!(ui.npc_topics().is_none(), "the topic box is freed");
    let lines = ui.npc_dialog_lines().expect("the dialog panel");
    assert_eq!(texts(&lines), ["Hello", "There"]);
    let s = Strs::new();
    let draw = |ui: &OriginalUi, root: &UiRoot, w: &ClientWorld| {
        let _ = ui;
        let ctx = UiCtx {
            tick: w.frames,
            world: w,
            strings: &s,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        root.draw(&ctx, &mut out);
        out
    };
    let x = (800 - 325) / 2;
    let out = draw(&ui, &root, &w);
    assert!(out.contains(&UiDraw::Rect(RectRequest::sized(x, 12 - 5, 325, 122, 0, 1))));
    // The scroll runs on the clock: no whole line at first, then a line.
    poll(&mut ui, &mut root, &w);
    for _ in 0..80 {
        w.frames += 1;
        poll(&mut ui, &mut root, &w);
    }
    let out = draw(&ui, &root, &w);
    assert!(
        out.iter().any(|d| matches!(d, UiDraw::Text(t)
            if String::from_utf16_lossy(&t.text) == "Hello" && t.style.font == PANEL_FONT)),
        "the first line scrolled in"
    );
    // A press skips (after the 100 ms grace) and the topic box is back.
    click(&mut ui, &mut root, &w, Point::new(400, 300));
    poll(&mut ui, &mut root, &w);
    assert!(ui.npc_dialog_lines().is_none());
    assert!(ui.npc_topics().is_some(), "the topic box again");
}

// Charsi's Imbue row opens the item-socket dialog (§11, NPC mode, ui
// 0x0E; ui 8 off first); with an item placed, the imbue button sends C→S
// 0x38 [0][NPC][item] and the note opens (step 3); S→C 0x58 code 6
// closes it and ends the interaction (0x30).
// Covers: specs/ui/messages.md §11 r2, §11 r3, §11 r6
#[test]
fn imbue_runs_the_item_socket_dialog() {
    let (mut ui, mut root) = setup();
    let mut w = world(CHARSI);
    ui.open_npc_menu(NPC, CHARSI, 10, &w);
    poll(&mut ui, &mut root, &w);
    let rows = ui.npc_menu().unwrap().rows;
    let k = rows
        .iter()
        .position(|r| r.kind == Some(crate::ui::layout::OptionKind::Imbue))
        .unwrap();
    let p = ui.npc_menu_row_point(k).unwrap();
    click(&mut ui, &mut root, &w, p);
    assert!(!ui.is_open(8) && ui.is_open(UI_SOCKET));
    assert!(root.open_panels().contains(&SOCKET_PANEL));
    assert_eq!(ui.socket_step(), Some(Step::Open));
    // Its text is clipped to the configured screen.
    {
        let s = Strs::new();
        let ctx = UiCtx {
            tick: w.frames,
            world: &w,
            strings: &s,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        root.draw(&ctx, &mut out);
        assert!(out
            .iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(t.clip),
                _ => None,
            })
            .all(|c| c == Screen::R800.rect()));
    }
    ui.imbue_place(555);
    // §11 r5: no click within 400 ms of the open.
    w.frames += 10;
    // Button 0 (hit 122–154 × 224–256).
    click(&mut ui, &mut root, &w, Point::new(130, 230));
    let mut want = vec![0x38, 0, 0, 0, 0];
    want.extend_from_slice(&NPC.to_le_bytes());
    want.extend_from_slice(&555u32.to_le_bytes());
    assert_eq!(
        root.take_intents()
            .iter()
            .map(|i| i.0.clone())
            .collect::<Vec<_>>(),
        vec![want]
    );
    assert_eq!(ui.socket_step(), Some(Step::Waiting));
    ui.apply_output(
        &Output::OpenUi {
            guid: NPC,
            code: 6,
            arg: 0,
        },
        &w,
    )
    .unwrap();
    poll(&mut ui, &mut root, &w);
    assert_eq!(ui.socket_step(), None);
    assert!(!ui.is_open(UI_SOCKET));
    assert_eq!(root.take_intents()[0].0[0], 0x30, "the interaction ends");
}
