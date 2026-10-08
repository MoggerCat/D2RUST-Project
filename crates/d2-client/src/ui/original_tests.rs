// Spec: specs/ui/panels.md
//! The original UI wired into a root: hotkeys, the gate and open mode,
//! the panel adapters' draws and clicks (spec §Test vectors where the
//! client model holds the inputs).

use super::*;
use crate::bridge::world::{ClientUnit, UnitKey};
use crate::ui::draw::UiDraw;
use crate::ui::{ClientIntent, NoPanelRules, NoStrings};

const AMAZON: u32 = 0;

fn areas() -> Vec<InvArea> {
    let mut v = vec![InvArea::default(); 32];
    // §Test vectors: `inventory.bin` records 0 and 16 (amazon).
    v[0] = InvArea {
        left: 320,
        right: 640,
        top: 0,
        bottom: 441,
    };
    v[16] = InvArea {
        left: 400,
        right: 720,
        top: 60,
        bottom: 501,
    };
    v
}

fn world(class: u32, mode: u32, expansion: bool) -> ClientWorld {
    let mut w = ClientWorld::default();
    let key = UnitKey::new(PLAYER, 1);
    let mut u = ClientUnit::new(key);
    u.class = class;
    u.mode = mode;
    w.units.insert(key, u);
    w.local_player = Some(key);
    w.expansion = u32::from(expansion);
    w
}

struct Ui {
    ui: OriginalUi,
    root: UiRoot,
}

fn ui(inv: Option<Vec<InvArea>>, installed: bool) -> Ui {
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: installed,
    };
    let ui = OriginalUi::new(config, inv).unwrap();
    let mut root = UiRoot::new(Box::new(NoPanelRules));
    ui.install(&mut root).unwrap();
    Ui { ui, root }
}

impl Ui {
    fn send(&mut self, w: &ClientWorld, e: UiEvent) -> Routed {
        let ctx = UiCtx {
            tick: 0,
            world: w,
            strings: &NoStrings,
        };
        self.ui.before_event(e, w);
        let r = self.root.dispatch(e, &ctx);
        self.ui.after_event(&mut self.root, e, r).unwrap();
        r
    }

    fn root_char(&mut self, w: &ClientWorld, c: u16) -> Routed {
        self.send(w, UiEvent::Char(c))
    }

    fn key(&mut self, w: &ClientWorld, a: Action) -> Routed {
        self.send(w, UiEvent::Action(ActionId(a.index() as u16)))
    }

    fn click(&mut self, w: &ClientWorld, at: Point) -> (Routed, Routed) {
        let b = PointerButton::Left;
        (
            self.send(w, UiEvent::Press { button: b, at }),
            self.send(w, UiEvent::Release { button: b, at }),
        )
    }

    /// (file name, frame, x, y) of every image the open panels draw.
    fn images(&self, w: &ClientWorld) -> Vec<(String, u32, i32, i32)> {
        let ctx = UiCtx {
            tick: 0,
            world: w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        self.root.draw(&ctx, &mut out);
        let files = self.ui.files();
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Image(i) => Some((
                    files.name(i.image.file).unwrap().to_string(),
                    i.image.frame,
                    i.at.x,
                    i.at.y,
                )),
                _ => None,
            })
            .collect()
    }
}

fn panel_images(v: &[(String, u32, i32, i32)], prefix: &str) -> Vec<(u32, i32, i32)> {
    v.iter()
        .filter(|(n, ..)| n.starts_with(prefix))
        .map(|(_, f, x, y)| (*f, *x, *y))
        .collect()
}

// Covers: specs/ui/panels.md §5, §6 r1, §6 r2
#[test]
fn install_mirrors_the_flags_and_keeps_the_border_open() {
    let u = ui(Some(areas()), true);
    assert_eq!(
        u.root.open_panels(),
        vec![
            BORDER_PANEL,
            crate::ui::hire_list::HIRE_PANEL,
            crate::ui::npc_menu_ui::NPC_MENU_PANEL,
            hud::HUD_PANEL,
            crate::ui::original::gold_dialog::GOLD_PANEL,
            crate::ui::original::game_messages::MESSAGES_PANEL,
            crate::ui::original::overhead_ui::OVERHEAD_PANEL
        ]
    );
    let w = world(AMAZON, 1, true);
    let img = u.images(&w);
    // Mode 0: no border; the 800 × 600 control panel base, six frames.
    assert!(panel_images(&img, "panel\\800borderframe").is_empty());
    let ctrl = panel_images(&img, "panel\\800ctrlpnl7");
    assert_eq!(ctrl.len(), 6);
    assert_eq!(ctrl[0], (0, 0, 600));
    assert_eq!(ctrl[5], (5, 800 - 117, 600));
}

// Covers: specs/ui/panels.md §2 r2, §2 r5, §2 r6, §4 r2, §4 r3
#[test]
fn hotkeys_toggle_their_state_with_jump_0() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    // The mouse at x 500 would jump with jump 1 (§4.3 vector); hot keys
    // pass 0.
    u.send(&w, UiEvent::CursorMoved(Point::new(500, 300)));
    assert_eq!(u.key(&w, Action::ToggleInventory), Routed::Unhandled);
    assert!(u.ui.is_open(UI_INVENTORY));
    assert_eq!(u.ui.open_mode().get(), 1);
    assert_eq!(
        u.ui.take_outcome().effects,
        vec![
            UiEffect::Opened(1),
            UiEffect::InventoryHook,
            UiEffect::OpenMode(OpenMode::new(1).unwrap())
        ]
    );
    assert_eq!(
        u.root.open_panels(),
        vec![
            PanelId(1),
            BORDER_PANEL,
            crate::ui::hire_list::HIRE_PANEL,
            crate::ui::npc_menu_ui::NPC_MENU_PANEL,
            hud::HUD_PANEL,
            crate::ui::original::gold_dialog::GOLD_PANEL,
            crate::ui::original::game_messages::MESSAGES_PANEL,
            crate::ui::original::overhead_ui::OVERHEAD_PANEL
        ],
        "the root mirrors the flag"
    );
    // The border shows on the right (mode 1): frames 5–9.
    let border = panel_images(&u.images(&w), "panel\\800borderframe");
    assert_eq!(
        border.iter().map(|b| b.0).collect::<Vec<_>>(),
        vec![5, 6, 7, 8, 9]
    );
    u.key(&w, Action::ToggleInventory);
    assert!(!u.ui.is_open(UI_INVENTORY));
    assert_eq!(u.ui.open_mode().get(), 0);
    // An action without a state does nothing.
    u.key(&w, Action::ToggleRun);
    assert_eq!(
        u.root.open_panels(),
        vec![
            BORDER_PANEL,
            crate::ui::hire_list::HIRE_PANEL,
            crate::ui::npc_menu_ui::NPC_MENU_PANEL,
            hud::HUD_PANEL,
            crate::ui::original::gold_dialog::GOLD_PANEL,
            crate::ui::original::game_messages::MESSAGES_PANEL,
            crate::ui::original::overhead_ui::OVERHEAD_PANEL
        ]
    );
}

// Covers: specs/ui/panels.md §3 r3, §4 r2
#[test]
fn conflict_table_vectors() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    // Inventory open, character on: C[1][2] = 0, both open, mode 3.
    u.key(&w, Action::ToggleInventory);
    u.key(&w, Action::ToggleCharacter);
    assert!(u.ui.is_open(UI_INVENTORY) && u.ui.is_open(UI_CHARACTER));
    assert_eq!(u.ui.open_mode().get(), 3);
    // Character off, then the skill tree: C[1][4] = 1 closes the
    // inventory (mode 0), then the skill tree opens (mode 1).
    u.key(&w, Action::ToggleCharacter);
    u.ui.take_outcome();
    u.key(&w, Action::ToggleSkillTree);
    assert!(!u.ui.is_open(UI_INVENTORY) && u.ui.is_open(UI_SKILLTREE));
    assert_eq!(u.ui.open_mode().get(), 1);
    let fx = u.ui.take_outcome().effects;
    assert_eq!(fx[0], UiEffect::Closed(1));
    assert_eq!(
        *fx.last().unwrap(),
        UiEffect::OpenMode(OpenMode::new(1).unwrap())
    );
}

// Covers: specs/ui/panels.md §2 r5
#[test]
fn a_dead_player_cannot_toggle_a_panel() {
    let mut u = ui(Some(areas()), true);
    let dead = world(AMAZON, 0x11, true);
    u.key(&dead, Action::ToggleInventory);
    assert!(!u.ui.is_open(UI_INVENTORY));
    // No local player: allowed (§2.5 "no P").
    u.key(&ClientWorld::default(), Action::ToggleInventory);
    assert!(u.ui.is_open(UI_INVENTORY));
}

// Covers: specs/ui/panels.md §4 r4, §9 r3, §7 r2
#[test]
fn inventory_draws_its_art_and_its_close_button_closes_it() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    u.key(&w, Action::ToggleInventory);
    // §Test vectors, 800 × 600: frames 4–7 at (400, 316), (656, 316),
    // (400, 492), (656, 492); close button (418, 476).
    let img = u.images(&w);
    assert_eq!(
        panel_images(&img, "panel\\invchar6"),
        vec![(4, 400, 316), (5, 656, 316), (6, 400, 492), (7, 656, 492)]
    );
    assert_eq!(
        panel_images(&img, "panel\\buysellbtn"),
        vec![(10, 418, 476)]
    );
    // Press: frame 11; release in the close rectangle: SetUIState(1, off).
    let at = Point::new(430, 460);
    let b = PointerButton::Left;
    assert_eq!(
        u.send(&w, UiEvent::Press { button: b, at }),
        Routed::Panel(PanelId(1))
    );
    assert_eq!(
        panel_images(&u.images(&w), "panel\\buysellbtn"),
        vec![(11, 418, 476)]
    );
    u.send(&w, UiEvent::Release { button: b, at });
    assert!(!u.ui.is_open(UI_INVENTORY));
    assert_eq!(u.ui.open_mode().get(), 0);
    // A click elsewhere in the area is consumed and closes nothing.
    u.key(&w, Action::ToggleInventory);
    assert_eq!(
        u.click(&w, Point::new(600, 200)),
        (Routed::Panel(PanelId(1)), Routed::Panel(PanelId(1)))
    );
    assert!(u.ui.is_open(UI_INVENTORY));
    // Outside the `inv` rectangle: not the panel's.
    assert_eq!(u.click(&w, Point::new(100, 200)).0, Routed::Unhandled);
}

// Covers: specs/ui/panels.md §4 r4
#[test]
fn without_the_inventory_table_the_right_panels_take_no_click() {
    let mut u = ui(None, true);
    let w = world(AMAZON, 1, true);
    u.key(&w, Action::ToggleInventory);
    assert_eq!(
        u.click(&w, Point::new(430, 460)),
        (Routed::Unhandled, Routed::Unhandled)
    );
    assert!(u.ui.is_open(UI_INVENTORY));
}

// Covers: specs/ui/panels.md §10 r1, §10 r2, §10 r6, §10 r8
#[test]
fn skill_tree_art_tabs_and_close() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    u.key(&w, Action::ToggleSkillTree);
    // §Test vectors: amazon tab 1, frames 0–3 then 4–7 at (400, 316) …;
    // close button at (571, 477).
    let img = u.images(&w);
    let art = panel_images(&img, "spells\\skltree_a_back");
    assert_eq!(
        art,
        vec![
            (0, 400, 316),
            (1, 656, 316),
            (2, 400, 492),
            (3, 656, 492),
            (4, 400, 316),
            (5, 656, 316),
            (6, 400, 492),
            (7, 656, 492)
        ]
    );
    assert_eq!(
        panel_images(&img, "panel\\buysellbtn"),
        vec![(10, 571, 477)]
    );
    // Tab 2 (mouse down in its rectangle): the click sound, frames 8–11.
    let tab2 = Point::new(650, 300);
    assert_eq!(skilltree_tab(tab2), Some(2));
    u.click(&w, tab2);
    assert_eq!(
        u.ui.take_outcome().sounds,
        vec![crate::audio::driver::SoundRequest::Ui(CLICK_SOUND_ID)]
    );
    let art = panel_images(&u.images(&w), "spells\\skltree_a_back");
    assert_eq!(
        art[4..].iter().map(|a| a.0).collect::<Vec<_>>(),
        vec![8, 9, 10, 11]
    );
    // The same tab again: no sound.
    u.click(&w, tab2);
    assert!(u.ui.take_outcome().sounds.is_empty());
    // Close: amazon tab 2 offset −220 → button at (500, 477); release in
    // its rectangle toggles ui 4 off.
    u.click(&w, Point::new(510, 460));
    assert!(!u.ui.is_open(UI_SKILLTREE));
}

fn skilltree_tab(p: Point) -> Option<u8> {
    super::super::panels::skilltree::tab_at(&Screen::R800, p)
}

// Covers: specs/ui/panels.md §8 r1, §8 r2, §8 r3, §4 r4
#[test]
fn character_art_and_close_button() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    u.key(&w, Action::ToggleCharacter);
    assert_eq!(u.ui.open_mode().get(), 2);
    // §Test vectors: quads at (80, 316), (336, 316), (80, 492), (336, 492).
    let img = u.images(&w);
    assert_eq!(
        panel_images(&img, "panel\\invchar6"),
        vec![(0, 80, 316), (1, 336, 316), (2, 80, 492), (3, 336, 492)]
    );
    assert_eq!(
        panel_images(&img, "panel\\buysellbtn"),
        vec![(10, 208, 480)]
    );
    // No stat-point box or add buttons (`PENDING`).
    assert!(panel_images(&img, "panel\\skillpoints").is_empty());
    // Only the control panel's two closed level buttons (frame 2, §8).
    assert_eq!(
        panel_images(&img, "panel\\level"),
        vec![(2, 206, 592), (2, 563, 592)]
    );
    // A classic install draws `InvChar`.
    let mut c = ui(Some(areas()), false);
    c.key(&w, Action::ToggleCharacter);
    assert_eq!(panel_images(&c.images(&w), "panel\\invchar").len(), 4);
    assert!(panel_images(&c.images(&w), "panel\\invchar6").is_empty());
    // Release in the close rectangle: SetUIState(2, off).
    u.click(&w, Point::new(215, 460));
    assert!(!u.ui.is_open(UI_CHARACTER));
    assert_eq!(u.ui.open_mode().get(), 0);
}

// Covers: specs/ui/panels.md §15
#[test]
fn panel_intents_leave_through_the_root_in_order() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    let a = ClientIntent(vec![0x3A, 0, 0]);
    let b = ClientIntent(vec![0x3B, 6, 0]);
    u.ui.shared.borrow_mut().outputs.extend([
        PanelOutput::Intent(a.clone()),
        PanelOutput::SetUi {
            ui: UI_INVENTORY,
            mode: 0,
            jump: false,
        },
        PanelOutput::Intent(b.clone()),
    ]);
    u.ui.after_event(&mut u.root, UiEvent::CursorLeft, Routed::Unhandled)
        .unwrap();
    assert_eq!(u.root.intents(), &[a, b]);
    assert!(u.ui.is_open(UI_INVENTORY));
    let _ = w;
}

// Covers: specs/ui/panels.md §2 r2
#[test]
fn a_bad_state_request_is_an_error() {
    let mut u = ui(None, true);
    assert!(matches!(
        u.ui.set_ui(0x26, 0, false),
        Err(UiStateError::BadState(0x26))
    ));
    u.ui.shared.borrow_mut().outputs.push(PanelOutput::SetUi {
        ui: 1,
        mode: 3,
        jump: false,
    });
    assert!(u
        .ui
        .after_event(&mut u.root, UiEvent::CursorLeft, Routed::Unhandled)
        .is_err());
}

// Covers: specs/ui/panels.md §4 r3; specs/items/inventory.md §1.3
#[test]
fn hotkey_states_and_records() {
    assert_eq!(
        hotkey_state(ActionId(Action::ToggleInventory.index() as u16)),
        Some(1)
    );
    assert_eq!(
        hotkey_state(ActionId(Action::ToggleCharacter.index() as u16)),
        Some(2)
    );
    assert_eq!(
        hotkey_state(ActionId(Action::ToggleSkillTree.index() as u16)),
        Some(4)
    );
    assert_eq!(
        hotkey_state(ActionId(Action::GameMenu.index() as u16)),
        None
    );
    // items/inventory.md §1.3 class records, + 16 at 800 × 600.
    assert_eq!(inventory_record(0, &Screen::R800), Some(16));
    assert_eq!(inventory_record(5, &Screen::R640), Some(14));
    assert_eq!(inventory_record(6, &Screen::R800), Some(31));
    assert_eq!(inventory_record(7, &Screen::R800), None);
    assert!(PENDING
        .iter()
        .all(|(what, why)| !what.is_empty() && !why.is_empty()));
}

// Covers: specs/ui/panels-2.md §21 r1
#[test]
fn inventory_draws_the_gold_value_and_button_from_the_model() {
    let mut u = ui(Some(areas()), true);
    let mut w = world(AMAZON, 1, true);
    let key = w.local_player.unwrap();
    w.units.get_mut(&key).unwrap().stats.insert(14, 4321);
    // A state list adds 9: the line shows the full value.
    w.units
        .get_mut(&key)
        .unwrap()
        .state_lists
        .insert(1, [((14u16, 0u16), 9)].into_iter().collect());
    u.key(&w, Action::ToggleInventory);
    // The button needs no player: (W − sx − 236, H + sy − 71).
    assert_eq!(
        panel_images(&u.images(&w), "panel\\goldcoinbtn"),
        vec![(0, 484, 469)]
    );
    let gold_texts = |u: &Ui| -> Vec<(String, i32, i32, u16, u16)> {
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        u.root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some((
                    String::from_utf16_lossy(&t.text),
                    t.at.x,
                    t.at.y,
                    t.style.font,
                    t.style.color,
                )),
                _ => None,
            })
            .collect()
    };
    // Without the fonts bound no text is drawn.
    assert!(gold_texts(&u).is_empty());
    let mut f = FontMeasure::default();
    f.insert(
        1,
        FontTable::parse(&character_bind_tests::tbl(6)).expect("tbl"),
    );
    u.ui.set_fonts(f);
    // `%d` of stat 14 total, Font16, color 0, at (W − sx − 212, H + sy − 72).
    assert_eq!(gold_texts(&u), vec![("4330".to_string(), 508, 468, 1, 0)]);
}

// d2rs-own, unverified: Esc order (controls.md §3 row 56)
#[test]
fn esc_opens_the_game_menu_closes_panels_first_and_closes_it_again() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    assert!(!u.ui.is_open(9));
    // Nothing open: Esc opens the menu.
    u.key(&w, Action::GameMenu);
    assert!(u.ui.is_open(9));
    assert!(u.root.open_panels().contains(&esc_menu::ESC_PANEL));
    // Esc again closes it.
    u.key(&w, Action::GameMenu);
    assert!(!u.ui.is_open(9));
    // A panel open: Esc closes the panel only; the next Esc opens the menu.
    u.key(&w, Action::ToggleInventory);
    assert!(u.ui.is_open(1));
    u.key(&w, Action::GameMenu);
    assert!(!u.ui.is_open(1) && !u.ui.is_open(9));
    u.key(&w, Action::GameMenu);
    assert!(u.ui.is_open(9));
}

// Covers: specs/ui/panels.md §3
#[test]
fn esc_without_a_player_opens_nothing() {
    let mut u = ui(Some(areas()), true);
    u.key(&ClientWorld::default(), Action::GameMenu);
    assert!(!u.ui.is_open(9));
}

// The rows of the tree replace the old three-entry / Options page; the
// Game menu rows are now at the spec's y tops 185 / 235 / 285.
// Covers: specs/ui/frontend-options.md §o3-save-and-exit-game-0x0047f2d0 r1, §o5-input-handler-table-0x006d6030-7-entries-registered-while-ui-9-is-open r6, §o7-settings-storage-and-the-d2rs-config-mapping r2
#[test]
fn the_menu_tree_returns_saves_exits_and_swallows_clicks() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    let texts = |u: &Ui| -> Vec<String> {
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        u.root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(String::from_utf16_lossy(&t.text)),
                _ => None,
            })
            .collect()
    };
    assert!(texts(&u).is_empty());
    u.key(&w, Action::GameMenu);
    assert_eq!(
        texts(&u),
        vec!["Options", "Save and Exit Game", "Return to Game"]
    );
    // Rows: Game menu tops 185 / 235 / 285 (click inside the 50 px row).
    let row = |i: i32| Point::new(400, 185 + 50 * i + 20);
    let (_, r) = u.click(&w, row(0));
    assert_ne!(r, Routed::Unhandled);
    assert!(u.ui.is_open(9) && !u.ui.take_exit_request());
    assert_eq!(
        texts(&u),
        vec![
            "Sound Options",
            "Video Options",
            "Automap Options",
            "Configure Controls",
            "Previous Menu"
        ]
    );
    // Options rows (tops 135, 185, ...): Video Options.
    let orow = |i: i32| Point::new(400, 135 + 50 * i + 20);
    u.click(&w, orow(1));
    let t = texts(&u);
    assert_eq!(t[0], "Video Options");
    assert!(t.contains(&"Window Mode".to_string()));
    // Video rows (tops 70 + 45 k for 9 rows): Window Mode is row 2.
    let m = u.ui.shared.borrow().esc.menu.clone();
    let wm = Point::new(400, m.y_top(2) + 20);
    // Window Mode cycles and reports the change once; the value round-trips
    // through the config text (§O7).
    assert!(u.ui.take_settings_change().is_none());
    u.click(&w, wm);
    let s = u.ui.take_settings_change().unwrap();
    assert_eq!(s.window_mode, crate::app::config::WindowMode::Borderless);
    assert!(u.ui.take_settings_change().is_none());
    let back = crate::app::config::parse_settings(&crate::app::config::write_settings(&s));
    assert_eq!(back.unwrap(), s);
    // Previous Menu (last row) twice: Options, then the Game menu.
    for _ in 0..2 {
        let m = u.ui.shared.borrow().esc.menu.clone();
        let last = m.rows().len() - 1;
        u.click(&w, Point::new(400, m.y_top(last) + 20));
    }
    assert_eq!(
        texts(&u),
        vec!["Options", "Save and Exit Game", "Return to Game"]
    );
    // A click outside the rows does not reach the world either.
    let (_, r) = u.click(&w, Point::new(10, 10));
    assert_ne!(r, Routed::Unhandled);
    // Arrow keys and Enter: Down wraps to Options; Right on a choice row.
    u.root_char(&w, 0xF028);
    u.root_char(&w, 0x0D);
    assert_eq!(texts(&u)[0], "Sound Options");
    // Configure Controls is requested once.
    for _ in 0..4 {
        u.root_char(&w, 0xF028);
    }
    u.root_char(&w, 0x0D);
    assert!(u.ui.take_controls_request());
    assert!(!u.ui.take_controls_request());
    // Esc closes the whole menu from a sub-menu (§O1 r4).
    u.key(&w, Action::GameMenu);
    assert!(!u.ui.is_open(9));
    // Reopened: first page, Save and Exit asks the host once.
    u.key(&w, Action::GameMenu);
    u.click(&w, row(1));
    assert!(u.ui.take_exit_request());
    assert!(!u.ui.take_exit_request());
    // Return closes.
    u.click(&w, row(2));
    assert!(!u.ui.is_open(9));
}

// Covers: specs/ui/control-panel.md §9
#[test]
fn the_mini_panel_game_menu_button_opens_it() {
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    u.ui.set_ui(0x15, 0, false).unwrap();
    u.root.sync_states(&u.ui.shared.borrow().states);
    // Row 7 of the mini panel; the exact rectangle is the spec's, so
    // find it by scanning the panel strip for a click that opens ui 9.
    let mut opened = false;
    'scan: for y in 440..600 {
        for x in (250..560).step_by(2) {
            u.click(&w, Point::new(x, y));
            if u.ui.is_open(9) {
                opened = true;
                break 'scan;
            }
            if !u.ui.is_open(0x15) {
                u.ui.set_ui(0x15, 0, false).unwrap();
                u.root.sync_states(&u.ui.shared.borrow().states);
            }
        }
    }
    assert!(opened);
}

// d2rs-own, unverified: the drop-gold dialog (REC-103); the button
// and the OK request are panels-2.md §21 r3–r8
#[test]
fn the_gold_button_opens_the_dialog_and_ok_sends_drop_gold() {
    let mut u = ui(Some(areas()), true);
    let mut w = world(AMAZON, 1, true);
    let key = w.local_player.unwrap();
    w.units.get_mut(&key).unwrap().stats.insert(14, 5000);
    u.key(&w, Action::ToggleInventory);
    let texts = |u: &Ui| -> Vec<String> {
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        u.root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(String::from_utf16_lossy(&t.text)),
                _ => None,
            })
            .collect()
    };
    assert!(texts(&u).is_empty());
    // §21 r3: x in [W − sx − 237, W − sx − 217], y in [H + sy − 87, H + sy − 69].
    let button = Point::new(493, 462);
    let b = PointerButton::Left;
    // A press sets the flag (the button's frame 1, one row lower).
    u.send(
        &w,
        UiEvent::Press {
            button: b,
            at: button,
        },
    );
    assert_eq!(
        panel_images(&u.images(&w), "panel\\goldcoinbtn"),
        vec![(1, 484, 470)]
    );
    // The release in the rectangle opens the dialog.
    u.send(
        &w,
        UiEvent::Release {
            button: b,
            at: button,
        },
    );
    assert_eq!(
        panel_images(&u.images(&w), "panel\\goldcoinbtn"),
        vec![(0, 484, 469)]
    );
    assert!(texts(&u).contains(&"do you want to drop?".to_string()));
    // Digits fill the edit box, letters are ignored; past the maximum
    // (stat 14) the box takes the maximum.
    for c in "15x00".chars() {
        let r = u.send(&w, UiEvent::Char(c as u16));
        assert_ne!(r, Routed::Unhandled);
    }
    assert!(texts(&u).contains(&"1500_".to_string()));
    // The belt keys and the menu key do not reach the game while open.
    assert_ne!(u.key(&w, Action::BeltSlot1), Routed::Unhandled);
    // Enter is OK: C→S 0x50 [player GUID][1500]; the dialog closes.
    u.send(&w, UiEvent::Char(0x0D));
    let guid = key.guid.to_le_bytes();
    let mut want = vec![0x50];
    want.extend_from_slice(&guid);
    want.extend_from_slice(&1500u32.to_le_bytes());
    assert_eq!(u.root.intents(), &[ClientIntent(want)]);
    assert!(!texts(&u).contains(&"do you want to drop?".to_string()));
    // Cancel (Esc) sends nothing and the Esc menu stays shut.
    u.send(
        &w,
        UiEvent::Press {
            button: b,
            at: button,
        },
    );
    u.send(
        &w,
        UiEvent::Release {
            button: b,
            at: button,
        },
    );
    for c in "99999".chars() {
        u.send(&w, UiEvent::Char(c as u16));
    }
    assert!(texts(&u).contains(&"5000_".to_string()));
    assert_ne!(u.key(&w, Action::GameMenu), Routed::Unhandled);
    assert!(!u.ui.is_open(9));
    assert_eq!(u.root.intents().len(), 1);
    assert!(!texts(&u).contains(&"do you want to drop?".to_string()));
    // Zero is not sent (§21 r8).
    u.send(
        &w,
        UiEvent::Press {
            button: b,
            at: button,
        },
    );
    u.send(
        &w,
        UiEvent::Release {
            button: b,
            at: button,
        },
    );
    u.send(&w, UiEvent::Char(0x0D));
    assert_eq!(u.root.intents().len(), 1);
}

// Covers: specs/ui/inventory.md §11 r1, §11 r2, §11 r4
// d2rs-own, unverified: the stash gold dialogs (REC-240).
#[test]
fn stash_gold_withdraw_and_deposit_send_0x4f() {
    let mut u = ui(Some(areas()), true);
    let mut w = world(AMAZON, 1, true);
    let key = w.local_player.unwrap();
    w.units.get_mut(&key).unwrap().stats.insert(14, 5000);
    w.units.get_mut(&key).unwrap().stats.insert(15, 70000);
    u.ui.set_ui(0x19, 0, false).unwrap();
    u.root.sync_states(&u.ui.shared.borrow().states);
    assert!(u.ui.is_open(0x19));
    let texts = |u: &Ui| -> Vec<String> {
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        u.root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(String::from_utf16_lossy(&t.text)),
                _ => None,
            })
            .collect()
    };
    // The stash gold button (kind 4, withdraw): typed 1234 -> 0x4F 0x13.
    let s = Screen::R800;
    let btn = Point::new(s.sx() + 80, s.h + s.sy() - 455 + 5);
    u.click(&w, btn);
    assert!(texts(&u).contains(&"_".to_string()));
    for c in "1234".chars() {
        u.send(&w, UiEvent::Char(c as u16));
    }
    u.send(&w, UiEvent::Char(0x0D));
    assert_eq!(
        u.root.intents(),
        &[ClientIntent(vec![0x4F, 0x13, 0, 0, 0, 0xD2, 0x04])]
    );
    // Over the stash maximum nothing grows: the field takes the stat 15.
    u.click(&w, btn);
    for c in "9999999".chars() {
        u.send(&w, UiEvent::Char(c as u16));
    }
    assert!(texts(&u).contains(&"70000_".to_string()));
    u.send(&w, UiEvent::Char(0x1B));
    assert_eq!(u.root.intents().len(), 1);
    // The inventory gold button with the stash open (kind 3, deposit):
    // the field is pre-filled with the carried gold -> 0x4F 0x14.
    let inv_btn = Point::new(493, 462);
    u.click(&w, inv_btn);
    assert!(texts(&u).contains(&"5000_".to_string()));
    u.send(&w, UiEvent::Char(0x0D));
    assert_eq!(
        u.root.intents().last(),
        Some(&ClientIntent(vec![0x4F, 0x14, 0, 0, 0, 0x88, 0x13]))
    );
}

// d2rs-own, unverified: the stash GoldMax line reads string 4051 through
// `ctx.strings` (REC-238); cap = the fixed stash limit.
#[test]
fn stash_gold_max_line_resolves_its_string_id() {
    struct Strs(Vec<u16>);
    impl crate::ui::StringLookup for Strs {
        fn get(&self, _: &str) -> Option<&[u16]> {
            None
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            (id == 4051).then_some(self.0.as_slice())
        }
    }
    let mut u = ui(Some(areas()), true);
    let w = world(AMAZON, 1, true);
    u.ui.apply_output(
        &crate::bridge::output::Output::TradeAction {
            code: 0x10,
            dead_or_absent: false,
        },
        &w,
    )
    .unwrap();
    // Mirror the state flags into the root (no action: no hotkey runs).
    let e = UiEvent::Press {
        button: PointerButton::Right,
        at: Point::new(0, 0),
    };
    u.ui.after_event(&mut u.root, e, Routed::Unhandled).unwrap();
    let texts = |s: &dyn crate::ui::StringLookup| -> Vec<String> {
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: s,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        u.root.draw(&ctx, &mut out);
        out.iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(String::from_utf16_lossy(&t.text)),
                _ => None,
            })
            .collect()
    };
    assert!(texts(&NoStrings).is_empty());
    assert_eq!(
        texts(&Strs("Gold Max: %d".encode_utf16().collect())),
        vec!["Gold Max: 2500000"]
    );
}
