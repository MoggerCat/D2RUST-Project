// Spec: specs/ui/panels.md, specs/ui/panels-2.md (§17 r4), specs/client/stat-lists.md (§1 r3), specs/ui/text.md (§6), specs/client/msg-ui.md (via `msg_ui`)
//! The original panels wired into a [`UiRoot`] (§2–§10): [`OriginalUi`]
//! owns the 38 UI flags ([`UiStates`], the authority), the loaded panel
//! tables and the panel state; [`OriginalUi::install`] adds one
//! [`Panel`] adapter per wired panel to the root, in the §5 draw order,
//! with the panel's UI state id as its [`PanelId`] (so
//! [`UiRoot::sync_states`] mirrors the flags).
//!
//! Event flow (one UI frame, [`crate::world_view::run_ui_with`]): the
//! root routes each event to the adapters (top-most first, §A2); an
//! adapter runs the panel module's own press / release rules and parks
//! their [`PanelOutput`]s here. After each event [`OriginalUi::after_event`]
//! applies them in order: an intent is queued on the root (only the root
//! forwards it to the bridge), a `SetUIState` request runs
//! [`UiStates::set`] (gate, flags, open mode, §2–§4) and its effects, the
//! click sound is queued for the audio side. An event no panel took that
//! is a d2rs hotkey action (`ToggleInventory`, `ToggleCharacter`,
//! `ToggleSkillTree`) becomes `SetUIState(ui, toggle, jump 0)` (§4.3:
//! hot keys pass 0). The root then mirrors the flags.
//!
//! Wired: inventory (ui 1: art, close button, §9.3), skill tree (ui 4:
//! back art per class and tab, tabs, close button, §10; icons need the
//! skill list), character (ui 2: art and close button, §8.1–§8.3; with
//! the fonts' widths ([`FontMeasure`], [`OriginalUi::set_fonts`]) also
//! the name line (`panels-2.md` §17 r4) and the stat values bound to the
//! model's totals and bases, §8.4–§8.9, [`ModelCharacter`]), the
//! border and control panel base (§6, every frame). The model holds no
//! input for the rest; each stays out with its reason in [`PENDING`].
//! The bridge's UI outputs (S→C 0x5D, 0x63, 0x77; `client/bridge.md`
//! §10) are applied by [`OriginalUi::apply_output`] (`msg_ui`).
//! Nothing here decides an outcome: the client sends intents and draws.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use d2_formats::font::FontTable;

use super::draw::UiDrawSink;
use super::geom::{Point, Rect};
use super::item_tip;
use super::layout::{LayoutError, PanelKey, RowKind, Screen};
use super::panel::{ActionId, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use super::panels::border::draw_border_and_ctrlpnl;
use super::panels::char_details;
use super::panels::char_inputs;
use super::panels::character::{
    self, CharacterPanel, CharacterView, ResistEffect, STAT_STATPTS, UI_CHARACTER,
};
use super::panels::inv_gold;
use super::panels::inv_items::{InvLayout, ItemsUi};
use super::panels::inventory::{InventoryPanel, UI_INVENTORY};
use super::panels::skilltree::{SkillEntry, SkillTreePanel, SkillTreeView, UI_SKILLTREE};
use super::panels::{
    centered_in, emit_static_draws, no_extra, text, PanelEnv, PanelOutput, PanelTables,
    TextMeasure, UiFiles,
};
use super::root::{Routed, UiError, UiRoot};
use super::skill_tree_ui;
use super::states::{GateEnv, PlayerLife, UiEffect, UiStateError, UiStates};
use super::PointerButton;
use crate::assets::path::FileSource;
use crate::audio::driver::SoundRequest;
use crate::bridge::items::ItemArtRows;
use crate::bridge::msg::ui_npc::DialogCase;
use crate::bridge::output::NpcDialog;
use crate::bridge::world::{ClientWorld, KindData, UnitKey, PLAYER};
use crate::controls::Action;
use crate::rules::camera::OpenMode;

/// The border / control panel adapter's id: not a UI state (ids 0–37
/// are), so [`UiRoot::sync_states`] leaves it open.
pub const BORDER_PANEL: PanelId = PanelId(0x100);

/// The Esc-closable states (`panels.md` §2 r9, flag table `0x006D6378`
/// = 1), in the close-all's order i = 0 … 37.
pub const ESC_CLOSABLE: [u8; 27] = [
    1, 2, 3, 4, 5, 9, 0x0B, 0x0C, 0x0D, 0x0F, 0x10, 0x12, 0x14, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
    0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0x21, 0x24, 0x25,
];

/// The states the game menu's open remembers and its close reopens
/// (`frontend-options.md` §O1 r2: keep = 1).
pub const GAME_MENU_KEEP: [u8; 6] = [6, 7, 10, 17, 21, 35];

/// The step-10 overlays' adapter (hover tips, then the cursor item), drawn
/// after every other panel (`ui/panels.md` §5 step 10, `panels-3.md` §23
/// r9): not a UI state, open for good.
pub const TOP_PANEL: PanelId = PanelId(0x113);

/// The click sound of §10.2: `0x004B9A00(0, 0, 0)` = request id 0, no
/// unit, delay 0 (`audio/triggers.md` §1 r1).
pub const CLICK_SOUND_ID: i32 = 0;

/// Panels and inputs not wired, each with the input the client model or
/// the app does not hold yet (M02: named, not guessed).
pub const PENDING: &[(&str, &str)] = &[
    (
        "character shift-spend, popups and the skill-modified damage block (§8.6, §8.9; \
         `panels-2.md` §17 r5–r9)",
        "labels (string ids), the class line, next level, the resist / defense colours and a \
         weapon-only damage / attack-rating block are bound ([`char_feed`], PROVISIONAL \
         REC-269); the skill-specific `descdam` / `descatt` entries, the to-hit popups (need \
         `monstats`) are not (Shift spends in chunks of 32, `panels.md` §8 r5, REC-268); the language \
         is English (0, `ui/text.md` §1.2)",
    ),
    (
        "inventory gold button press / release and the gold dialog (`panels-2.md` §21 r3–r9)",
        "the gold value and button art are drawn ([`InventoryUi`]); the press / release and \
         the drop dialog (kind 1) are wired in [`gold_dialog`] (d2rs-own box, REC-103); the \
         press does not play sound 4 (deferred); the stash kinds 3 / 4 are wired (\
         `original_tests.rs::stash_gold_withdraw_and_deposit_send_0x4f`)",
    ),
    (
        "inventory tints (translucency) and empty-slot pictures (§9.4, `inventory.md` §2–§6)",
        "the equipped and grid item tints and the hover tint are painted by \
         `ItemsUi::draw_tints` as opaque `hudfill` tiles (REC-271: the UI sprite path has no \
         A2 blend); the shooter / quiver and cursor-item tints are not read and the \
         empty-slot pictures (§9.4 table) are not drawn",
    ),
    (
        "skill tree hover description and the no-points message (§10.5, §10.7)",
        "icons, level numbers, the flag mask and the point spend (C→S 0x3B) are wired \
         ([`skill_tree_ui`]); the point cost is 1 (`skpoints` formula not evaluated, \
         REC-270); the tab tool tips, the free-points box and the no-points message are \
         not wired",
    ),
    (
        "waypoint menu panel (ui 0x14, §13 r2–r7): tab gates",
        "the panel is installed and covered (`waypoint_ui`; \
         `app_play_npc.rs::clicking_the_waypoint_walks_there_and_interacts`; the row and tab \
         text come from the string table by id); the tab gates read the client quest flags \
         (`msg-ui.md` open question 4, tab 0 only): every act tab is shown (d2rs-own)",
    ),
    (
        "hotkeys for other states (escape menu, chat, automap, party; the quest log, ToggleQuests, is \
         bound: `quest_log_ui_tests.rs`)",
        "their panels are not specified (§Open questions 1); the original key table is \
         `ui/controls.md` (§Open questions 2)",
    ),
];

/// The screen and install facts the original UI reads (§Inputs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiConfig {
    /// W × H and the resolution mode (the 800 × 600 frame: [`Screen::R800`]).
    pub screen: Screen,
    /// `d2exp.mpq` present (`0x00408F20`).
    pub expansion_installed: bool,
}

/// One `inventory.bin` `inv` rectangle (§4.4, §9.2): left, right
/// (exclusive), top, bottom (inclusive).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvArea {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl InvArea {
    /// The right panel's click area (§4.4): left ≤ x < right, top ≤ y ≤
    /// bottom (`panels-2.md` §18 r2: bottom inclusive).
    pub fn rect(&self) -> Rect {
        let w = u16::try_from(self.right - self.left).unwrap_or(0);
        let h = u16::try_from(self.bottom - self.top + 1).unwrap_or(0);
        Rect::new(self.left, self.top, w, h)
    }
}

/// The `inventory.bin` record of a player class for the page-0 grid
/// (`items/inventory.md` §1.3: classes 0–4 → 0–4, 5 → 14, 6 → 15) plus
/// 16 at 800 × 600 (§9.2). `None` for a class outside 0–6.
pub fn inventory_record(class: u32, screen: &Screen) -> Option<usize> {
    let r = match class {
        0..=4 => class as usize,
        5 => 14,
        6 => 15,
        _ => return None,
    };
    Some(if screen.res2() { r + 16 } else { r })
}

/// The model facts a frame of the original UI reads, taken from the
/// client world before each event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Facts {
    /// Local player class (`[0x007A0522]`), when there is a local player.
    class: Option<u32>,
    player: Option<PlayerLife>,
    /// An expansion game (`[0x007A04F4]`).
    expansion_game: bool,
}

impl Facts {
    fn of(world: &ClientWorld) -> Self {
        let local = world.local().filter(|u| u.key.unit_type == PLAYER);
        Facts {
            class: local.map(|u| u.class),
            // §2.5 "P alive": type 0, mode neither 0 nor 0x11; flag 0x10000
            // is not in the model (`ClientUnit::is_dead`).
            player: local.map(|u| PlayerLife {
                alive: !u.is_dead(),
                dead: u.mode == 0x11,
            }),
            expansion_game: world.expansion != 0,
        }
    }
}

/// State shared by the controller and its panel adapters.
struct Shared {
    tables: PanelTables,
    states: UiStates,
    config: UiConfig,
    /// `inventory.bin` `inv` rectangles by record, when loaded.
    inv_areas: Option<Vec<InvArea>>,
    facts: Facts,
    /// The mouse as the UI last saw it (§4.3 reads it before a call).
    mouse: Point,
    /// Panel outputs of the event being routed, in order.
    outputs: Vec<PanelOutput>,
    /// The fonts' glyph widths (character values and name line); none:
    /// no text is drawn.
    fonts: Option<FontMeasure>,
    /// `difficultylevels` `ResistPenalty` by difficulty (§8.9, `0x00611D30`);
    /// an expansion game draws no values without it.
    resist_penalties: Option<Vec<i32>>,
    /// The character panel's extra tables ([`char_feed`]).
    char_tables: super::char_feed::CharTables,
    /// The control panel overlays' state ([`hud`]).
    hud: hud::HudState,
    /// The skill tree's class skill rows ([`skill_tree_ui`]).
    skill_tree: skill_tree_ui::SkillTreeTables,
    /// The inventory panel's item facts (`inv_items`).
    items: ItemsUi,
    /// The levels' waypoint indexes (`waypoint_ui`).
    waypoint_map: Option<d2_sim::world::waypoints::WaypointMap>,
    /// The waypoint menu S→C 0x63 opened last (`waypoint_ui`).
    waypoint_open: Option<WaypointOpen>,
    /// The Esc game menu's state ([`esc_menu`]).
    esc: esc_menu::EscState,
    /// The ui states the game menu's open closed and reopens at its close
    /// (`frontend-options.md` §O1 r2–r3, `0x00713060`).
    esc_kept: Vec<u8>,
    /// The quest log's inputs ([`quest_log_ui`]).
    quest: quest_log_ui::QuestInputs,
    /// The inventory gold button and the drop-gold dialog ([`gold_dialog`]).
    gold: gold_dialog::GoldState,
    /// The screen message list ([`game_messages`]).
    messages: game_messages::GameMessages,
    /// The overhead text bubbles ([`overhead_ui`]).
    bubbles: overhead_ui::Bubbles,
    /// S→C 0x77 0x15 opened the cube since the panel last looked: its
    /// close latch clears (`0x0048A460`, [`cube_ui`]).
    cube_opened: bool,
    /// The transmute animation (`panels.md` §12 r4, [`cube_ui`]).
    cube_anim: std::cell::Cell<super::panels::stash_cube::HoradricAnim>,
}

impl Shared {
    fn env(&self) -> PanelEnv {
        PanelEnv {
            screen: self.config.screen,
            open_mode: self.states.open_mode().get(),
            exp: self.facts.expansion_game && self.config.expansion_installed,
        }
    }

    /// The right panel's area (§4.4): the `inv` rectangle of the local
    /// player's record; none without the table or a player.
    fn right_area(&self) -> Option<Rect> {
        let class = self.facts.class?;
        let r = inventory_record(class, &self.config.screen)?;
        Some(self.inv_areas.as_ref()?.get(r)?.rect())
    }

    fn gate_env(&self) -> GateEnv {
        GateEnv {
            expansion: self.facts.expansion_game && self.config.expansion_installed,
            player: self.facts.player,
            // Only ui 5 / 9 read these two (§3.1); the controller never
            // requests them (`PENDING`).
            chat_blocked: false,
            input_hold: false,
            // d2rs has no modal text screen and no NPC interaction yet.
            modal_text: false,
            npc_active: false,
            screen: self.config.screen,
            mouse: self.mouse,
        }
    }
}

type SharedRef = Rc<RefCell<Shared>>;

/// What the controller did besides the root's intents, for the frame's
/// caller (world view, audio) and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiOutcome {
    /// `SetUIState` effects, in call order.
    pub effects: Vec<UiEffect>,
    /// Sound requests for the audio side, in order.
    pub sounds: Vec<SoundRequest>,
    /// Parts of the bridge outputs' UI dispatch not run because no spec
    /// gives their input or callee (`msg_ui::skip`), in order.
    pub skipped: Vec<&'static str>,
}

/// The original UI (`ui/panels.md`): flags, tables and panel state.
pub struct OriginalUi {
    shared: SharedRef,
    outcome: UiOutcome,
    /// The UI globals of the bridge outputs (`client/msg-ui.md`).
    msg: MsgUiState,
    /// The UI globals of the smaller outputs (`msg_ui::more`).
    more: MsgUiMore,
    /// The NPC text list `[0x007BF250]` (`client/msg-ui.md` §5 r2).
    npc_text: Option<NpcTextList>,
    /// The case of 0x28's dialog branch this UI chose for the last
    /// `NpcDialog`, for the bridge (`client/msg-ui.md` §16 r4.3, open
    /// question 10 decided as A).
    dialog_answer: Option<(Box<NpcDialog>, DialogCase)>,
    /// The hire list (`ui/hire_list.rs`, `menus.md` §3).
    pub(super) hire: super::hire_list::SharedHire,
    /// The NPC shop (`shop_ui`, `panels-2.md` §14 r4).
    shop: shop_ui::SharedShop,
    /// The NPC menu (`ui/npc_menu_ui.rs`, `menus.md` §2).
    pub(super) npcm: super::npc_menu_ui::SharedNpcMenu,
}

#[derive(Debug, thiserror::Error)]
pub enum OriginalUiError {
    #[error(transparent)]
    Layout(#[from] LayoutError),
    #[error(transparent)]
    Root(#[from] UiError),
    #[error(transparent)]
    State(#[from] UiStateError),
    /// The waypoint record's load copy refused its magic (fatal in
    /// 1.14d, `world/waypoints.md` §2 rule 5).
    #[error(transparent)]
    Waypoint(d2_sim::world::waypoints::WaypointError),
    /// 0x28's dialog branch found no NPC text list (fatal 0x1060 in
    /// 1.14d, `client/msg-ui.md` §16 r4.3).
    #[error("0x28 NPC dialog without an NPC text list (fatal 0x1060)")]
    NoNpcText,
    /// 0x27's list build asserts at a count of 8 or more (`0x00661557`,
    /// `client/msg-ui.md` §16 r9.1).
    #[error("0x27 NPC text list with count {0} (fatal assertion 0x00661557)")]
    NpcTextCount(u8),
}

impl OriginalUi {
    /// Loads the tables (`ui-states.tsv`, `panel-layout.tsv`); all flags
    /// 0, open mode 0. `inv_areas`: the `inventory.bin` `inv` rectangles
    /// by record (§9.2), `None` when the table is not loaded (the right
    /// panels then take no pointer event).
    pub fn new(config: UiConfig, inv_areas: Option<Vec<InvArea>>) -> Result<Self, LayoutError> {
        let mut tables = PanelTables::load()?;
        tables.files.extend(hud::hud_files());
        tables.files.extend(esc_art::esc_files());
        tables.files.extend(quest_log_ui::quest_files());
        tables.files.extend(cube_ui::cube_files());
        tables.files.extend(skill_tree_ui::icon_files());
        let shared = Shared {
            tables,
            states: UiStates::new()?,
            config,
            inv_areas,
            facts: Facts {
                class: None,
                player: None,
                expansion_game: false,
            },
            mouse: Point::new(0, 0),
            outputs: Vec::new(),
            fonts: None,
            resist_penalties: None,
            char_tables: Default::default(),
            hud: hud::HudState::default(),
            skill_tree: Default::default(),
            items: ItemsUi::default(),
            waypoint_map: None,
            waypoint_open: None,
            esc: esc_menu::EscState::default(),
            esc_kept: Vec::new(),
            quest: quest_log_ui::QuestInputs::default(),
            gold: gold_dialog::GoldState::default(),
            messages: Default::default(),
            bubbles: Default::default(),
            cube_opened: false,
            cube_anim: Default::default(),
        };
        Ok(Self {
            shared: Rc::new(RefCell::new(shared)),
            outcome: UiOutcome::default(),
            msg: MsgUiState::default(),
            more: MsgUiMore::default(),
            npc_text: None,
            dialog_answer: None,
            hire: super::hire_list::SharedHire::default(),
            shop: shop_ui::SharedShop::default(),
            npcm: Default::default(),
        })
    }

    /// Adds the wired panels to `root` in the §5 draw order: inventory
    /// family (step 5), skill tree then character (step 6), border and
    /// control panel (step 7).
    pub fn install(&self, root: &mut UiRoot) -> Result<(), UiError> {
        let sh = &self.shared;
        root.add(Box::new(InventoryUi {
            sh: sh.clone(),
            shop: self.shop.clone(),
            panel: InventoryPanel::default(),
        }))?;
        root.add(Box::new(SkillTreeUi {
            sh: sh.clone(),
            panel: SkillTreePanel::new(),
        }))?;
        root.add(Box::new(CharacterUi {
            sh: sh.clone(),
            panel: CharacterPanel::default(),
        }))?;
        root.add(Box::new(waypoint_ui::WaypointUi {
            sh: sh.clone(),
            panel: Default::default(),
            seq: 0,
        }))?;
        root.add(Box::new(quest_log_ui::QuestLogUi {
            sh: sh.clone(),
            log: Default::default(),
        }))?;
        root.add(Box::new(stash_ui::StashUi {
            sh: sh.clone(),
            input: Default::default(),
        }))?;
        root.add(Box::new(shop_ui::ShopUi {
            sh: sh.clone(),
            st: self.shop.clone(),
        }))?;
        root.add(Box::new(cube_ui::CubeUi {
            sh: sh.clone(),
            input: Default::default(),
        }))?;
        root.add(Box::new(BorderUi { sh: sh.clone() }))?;
        root.add(Box::new(super::hire_list::HireListUi {
            st: self.hire.clone(),
        }))?;
        root.open(super::hire_list::HIRE_PANEL)?;
        root.add(Box::new(super::npc_menu_ui::NpcMenuUi {
            st: self.npcm.clone(),
            hire: self.hire.clone(),
        }))?;
        root.open(super::npc_menu_ui::NPC_MENU_PANEL)?;
        root.add(Box::new(hud::HudUi { sh: sh.clone() }))?;
        root.add(Box::new(gold_dialog::GoldDialogUi { sh: sh.clone() }))?;
        root.add(Box::new(game_messages::MessagesUi { sh: sh.clone() }))?;
        root.open(game_messages::MESSAGES_PANEL)?;
        root.add(Box::new(overhead_ui::OverheadUi { sh: sh.clone() }))?;
        root.open(overhead_ui::OVERHEAD_PANEL)?;
        root.add(Box::new(esc_menu::EscMenuUi { sh: sh.clone() }))?;
        // Step 10 and the cursor draw: over the control panel overlays
        // (step 7–8) and every panel (`panels.md` §5, `panels-3.md` §23 r9).
        root.add(Box::new(TopUi { sh: sh.clone() }))?;
        // Not a UI state: open for good.
        root.open(BORDER_PANEL)?;
        root.open(hud::HUD_PANEL)?;
        root.open(TOP_PANEL)?;
        root.open(gold_dialog::GOLD_PANEL)?;
        root.sync_states(&sh.borrow().states);
        let sc = sh.borrow().config.screen;
        self.hire.borrow_mut().screen = (sc.w, sc.h);
        self.npcm.borrow_mut().screen = (sc.w, sc.h);
        Ok(())
    }

    /// The panel file registry ([`super::ImageRef::file`] ids).
    pub fn files(&self) -> UiFiles {
        self.shared.borrow().tables.files.clone()
    }

    /// The glyph widths of the fonts the panels measure (the character
    /// panel's values and name line). Without them those texts are not
    /// drawn. The draws name fonts ([`CHARACTER_FONTS`]) whose `.tbl` and
    /// DC6 the frame's assets must then hold
    /// ([`crate::world_view::ui_bind::TextAssetLoader`]).
    pub fn set_fonts(&mut self, fonts: FontMeasure) {
        self.shared.borrow_mut().fonts = Some(fonts);
    }

    /// `difficultylevels` `ResistPenalty` per difficulty, in row order
    /// (§8.9 expansion penalty, `0x00611D30`; `panels-2.md` §24 r2).
    pub fn set_resist_penalties(&mut self, penalties: Vec<i32>) {
        self.shared.borrow_mut().resist_penalties = Some(penalties);
    }

    /// The character panel's class keys, state flags and skilldesc rows.
    pub fn set_char_tables(&mut self, tables: super::char_feed::CharTables) {
        self.shared.borrow_mut().char_tables = tables;
    }

    /// The HUD's skill icon and experience tables ([`hud::HudTables`]).
    pub fn set_hud_tables(&mut self, tables: hud::HudTables) {
        self.shared.borrow_mut().hud.tables = tables;
    }

    /// The skill tree's class skill rows (`skill_tree_ui`); without them
    /// the tree shows no icons.
    pub fn set_skill_tree_tables(&mut self, tables: skill_tree_ui::SkillTreeTables) {
        self.shared.borrow_mut().skill_tree = tables;
    }

    /// The walk's run toggle for the run button (§6 r1); returns the
    /// toggles the run button asked for since the last call (§10 r2).
    pub fn sync_run(&mut self, running: bool) -> u32 {
        let mut sh = self.shared.borrow_mut();
        sh.hud.running = running;
        std::mem::take(&mut sh.hud.run_toggles)
    }

    /// Item-table art rows (`inv_items`): the inventory panel draws the
    /// local player's items and the cursor item with them; empty draws
    /// nothing. Registers their graphics in [`Self::files`], so call it
    /// before handing the files to the art loader.
    pub fn set_item_art(&mut self, art: ItemArtRows) {
        let mut sh = self.shared.borrow_mut();
        sh.items.art = art;
        let Shared { items, tables, .. } = &mut *sh;
        items.register_files(&mut tables.files);
    }

    /// `inventory.bin` layouts by record (`inv_items::inv_layout` of each
    /// row); without them the grid is the spec's measured record.
    pub fn set_inv_layouts(&mut self, layouts: Vec<InvLayout>) {
        self.shared.borrow_mut().items.layouts = Some(layouts);
    }

    /// The `belts.bin` records and the belts' types (`hud_belt`).
    pub fn set_belt_parts(&mut self, parts: hud_belt::BeltParts) {
        self.shared.borrow_mut().hud.belt.parts = parts;
    }

    /// The act palette (`render/composition.md` §4) the UI's tint colours
    /// are matched in (`ui/inventory.md` §2 r1), set by the host each frame.
    pub fn set_palette(&mut self, palette: &d2_formats::palette::Palette) {
        let p: Vec<[u8; 3]> = palette.colors.iter().map(|c| [c.r, c.g, c.b]).collect();
        self.shared.borrow_mut().items.tint_colors = super::inv_grid::tint_indices(&p);
    }

    /// The frame's local-player position and shake (the world view's
    /// one camera, `seams/world-screen.md` §2.2), set before each UI frame.
    pub fn set_frame_anchor(&mut self, anchor: Option<crate::rules::camera::FrameAnchor>) {
        self.shared.borrow_mut().bubbles.anchor = anchor;
    }

    /// The belt key labels follow the play bindings (`hud_belt`, REC-264).
    pub fn set_belt_keys(&mut self, b: &crate::controls::Bindings) {
        self.shared.borrow_mut().hud.belt.set_keys(b);
    }

    /// The item tool tips' tables and strings (`item_tip`).
    pub fn set_item_tips(&mut self, tips: item_tip::ItemTips) {
        self.shared.borrow_mut().items.tips = Some(tips);
    }

    /// Shift is held (set by the host each frame, `inv_items`).
    pub fn set_shift(&mut self, shift: bool) {
        self.shared.borrow_mut().items.shift = shift;
    }

    /// Measured item graphic frame sizes by `invfile` (lower case).
    pub fn set_item_frame_sizes(&mut self, sizes: BTreeMap<String, (u32, u32)>) {
        self.shared.borrow_mut().items.frame_sizes = sizes;
    }

    /// The flags.
    pub fn is_open(&self, ui: u8) -> bool {
        self.shared.borrow().states.is_open(ui)
    }

    /// The screen open mode (§4.2): the camera's (`camera.md` §1).
    pub fn open_mode(&self) -> OpenMode {
        self.shared.borrow().states.open_mode()
    }

    /// Reads the model facts of the next event (call before routing it).
    pub fn before_event(&mut self, e: UiEvent, world: &ClientWorld) {
        self.refresh_facts(world);
        if let Some(p) = e.at() {
            self.shared.borrow_mut().mouse = p;
        }
    }

    /// Reads the model facts the gate uses (§2.5, §3).
    fn refresh_facts(&mut self, world: &ClientWorld) {
        self.shared.borrow_mut().facts = Facts::of(world);
    }

    /// Applies what the routed event asked for (module doc): panel
    /// outputs in order, then a hotkey action no panel took; the root
    /// mirrors the flags after.
    pub fn after_event(
        &mut self,
        root: &mut UiRoot,
        e: UiEvent,
        routed: Routed,
    ) -> Result<(), OriginalUiError> {
        let outputs = std::mem::take(&mut self.shared.borrow_mut().outputs);
        for o in outputs {
            match o {
                PanelOutput::Intent(i) => root.queue_intent(i),
                // A refused call (returns 0) changes nothing (§2.4).
                // The mini panel's menu button opens the game menu through
                // `0x0047E090(1, 0)` (`frontend-options.md` §O1 r2).
                PanelOutput::SetUi { ui, mode: 0, .. }
                    if ui == super::states::id::ESC_MENU && !self.is_open(ui) =>
                {
                    self.open_game_menu()?;
                }
                PanelOutput::SetUi { ui, mode, jump } => {
                    let was_menu = ui == super::states::id::ESC_MENU && self.is_open(ui);
                    self.set_ui(u32::from(ui), u32::from(mode), jump)?;
                    // Return to Game closes the menu through
                    // `0x0047E200(1)` (`frontend-options.md` §O1 r3).
                    if was_menu && !self.is_open(ui) {
                        self.restore_game_menu_states()?;
                    }
                }
                PanelOutput::ClickSound => {
                    self.outcome.sounds.push(SoundRequest::Ui(CLICK_SOUND_ID))
                }
            }
        }
        if let (Routed::Unhandled, UiEvent::Action(a)) = (routed, e) {
            if a == ActionId(Action::GameMenu.index() as u16) {
                self.game_menu_key()?;
            } else if let Some(ui) = hotkey_state(a) {
                // §4.3: the Character, Inventory, Party, Skill Tree and
                // Hireling keys pass jump 1, every other hot key 0; mode 2
                // toggle.
                let jump = matches!(ui, 1 | 2 | 4 | 0x16 | 0x24);
                self.set_ui(u32::from(ui), 2, jump)?;
                // The quest log asks the server for the quest data when
                // it opens (`quest_log_ui`; d2rs-own, unverified).
                if ui == quest_log_ui::UI_QUEST_SCREEN && self.is_open(ui) {
                    root.queue_intent(quest_log_ui::request_quest_data());
                }
            }
        }
        root.sync_states(&self.shared.borrow().states);
        Ok(())
    }

    /// Esc (command 56, `frontend-options.md` §O1 r2–r4): with ui 9 open
    /// the menu closes and the remembered states reopen (§O1 r3); else
    /// the close-all `0x00456300(0, 1)` (`panels.md` §2 r9) runs and the
    /// menu opens only when it closed nothing (§O1 r2). The command's
    /// no-op while an NPC interaction or a modal text screen is active is
    /// not modelled (the NPC menu is not ui 8 in play).
    fn game_menu_key(&mut self) -> Result<(), OriginalUiError> {
        use super::states::id;
        if self.is_open(id::ESC_MENU) {
            self.set_ui(u32::from(id::ESC_MENU), 1, false)?;
            self.restore_game_menu_states()?;
            return Ok(());
        }
        let mut closed = false;
        for ui in ESC_CLOSABLE {
            if self.is_open(ui) {
                self.set_ui(u32::from(ui), 1, true)?;
                closed = true;
            }
        }
        if !closed {
            self.open_game_menu()?;
        }
        Ok(())
    }

    /// `0x0047E090(save 1, menu 0)` (`frontend-options.md` §O1 r2): every
    /// ui but 0 and 9 closes, the open ones of [`GAME_MENU_KEEP`] are
    /// remembered, then ui 9 opens.
    fn open_game_menu(&mut self) -> Result<(), OriginalUiError> {
        use super::states::id;
        let mut kept = Vec::new();
        for ui in 0..=37u8 {
            if ui == id::GAME || ui == id::ESC_MENU || !self.is_open(ui) {
                continue;
            }
            if GAME_MENU_KEEP.contains(&ui) {
                kept.push(ui);
            }
            self.set_ui(u32::from(ui), 1, false)?;
        }
        self.shared.borrow_mut().esc_kept = kept;
        self.set_ui(u32::from(id::ESC_MENU), 0, false)?;
        Ok(())
    }

    /// `0x0047E200(1)` after ui 9 closed (`frontend-options.md` §O1 r3):
    /// the states remembered at the open reopen with jump 0.
    fn restore_game_menu_states(&mut self) -> Result<(), OriginalUiError> {
        let kept = std::mem::take(&mut self.shared.borrow_mut().esc_kept);
        for ui in kept {
            self.set_ui(u32::from(ui), 0, false)?;
        }
        Ok(())
    }

    /// Whether "Save and Exit Game" was chosen since the last call.
    pub fn take_exit_request(&mut self) -> bool {
        std::mem::take(&mut self.shared.borrow_mut().esc.exit_requested)
    }

    /// Whether Configure Controls was chosen since the last call.
    pub fn take_controls_request(&mut self) -> bool {
        std::mem::take(&mut self.shared.borrow_mut().esc.controls_requested)
    }

    /// Open the Configure Controls screen over the menu when it was
    /// chosen (`controls_host`); `path` is the `controls.toml` to load and
    /// save. Returns whether it opened.
    pub fn service_controls(&mut self, expansion: bool, path: Option<std::path::PathBuf>) -> bool {
        if !self.take_controls_request() {
            return false;
        }
        self.shared.borrow_mut().esc.controls =
            Some(controls_host::ControlsHost::open(expansion, path));
        true
    }

    /// Whether the Configure Controls screen is open.
    pub fn controls_open(&self) -> bool {
        self.shared.borrow().esc.controls.is_some()
    }

    /// Whether the expansion is installed (`0x00408F20`: the Options and
    /// Configure Controls tables, `ui/frontend-options.md` §O2, §O9).
    pub fn expansion_installed(&self) -> bool {
        self.shared.borrow().config.expansion_installed
    }

    /// The open Controls screen's state (hosts and tests read it).
    pub fn controls_screen(
        &self,
    ) -> Option<crate::ui::front_end::screens::controls::ConfigureControls> {
        let sh = self.shared.borrow();
        sh.esc.controls.as_ref().map(|c| c.model().clone())
    }

    /// A raw key (Windows virtual key) for the open Controls screen;
    /// false when it is not open (the key is for the game).
    pub fn controls_key(&mut self, vk: u16, now_ms: u64) -> bool {
        let mut sh = self.shared.borrow_mut();
        let Some(c) = sh.esc.controls.as_mut() else {
            return false;
        };
        if let Some(f) = c.key(vk, now_ms) {
            sh.esc.close_controls(f);
        }
        true
    }

    /// The play bindings accepted on the Controls screen, once.
    pub fn take_accepted_bindings(&mut self) -> Option<crate::controls::Bindings> {
        self.shared.borrow_mut().esc.accepted.take()
    }

    /// The settings the Esc menu's Options page shows (`app::config`).
    pub fn set_settings(&mut self, s: crate::app::config::Settings) {
        self.shared.borrow_mut().esc.menu.set_settings(s);
    }

    /// The settings after a change on the Options page, once per change.
    pub fn take_settings_change(&mut self) -> Option<crate::app::config::Settings> {
        let mut sh = self.shared.borrow_mut();
        sh.esc.menu.take_changed()
    }

    /// `SetUIState(ui, mode, jump)` with the model's gate facts; effects
    /// are kept for [`Self::take_outcome`].
    pub fn set_ui(&mut self, ui: u32, mode: u32, jump: bool) -> Result<bool, UiStateError> {
        let mut sh = self.shared.borrow_mut();
        let mut env = sh.gate_env();
        let r = sh
            .states
            .set(ui, mode, jump, &mut env, &mut self.outcome.effects);
        // The Esc menu always reopens on its first page.
        if ui == u32::from(esc_menu::ESC_PANEL.0) {
            sh.esc.menu.open();
            sh.esc.controls = None;
        }
        r
    }

    /// The cursor jump the pending effects ask for (§4.3,
    /// [`UiEffect::CursorX`]): the frame position `(x, mouse y)` of the last
    /// one, which also becomes the UI's mouse. Leaves the effects in place.
    pub fn take_cursor_warp(&mut self) -> Option<Point> {
        let x = self.outcome.effects.iter().rev().find_map(|e| match e {
            UiEffect::CursorX(x) => Some(*x),
            _ => None,
        })?;
        let mut sh = self.shared.borrow_mut();
        sh.mouse.x = x;
        Some(sh.mouse)
    }

    /// The effects and sounds since the last call.
    pub fn take_outcome(&mut self) -> UiOutcome {
        std::mem::take(&mut self.outcome)
    }

    /// The sound requests since the last call (the rest of the outcome
    /// stays).
    pub fn take_sounds(&mut self) -> Vec<SoundRequest> {
        std::mem::take(&mut self.outcome.sounds)
    }

    /// The skipped parts since the last call (the rest of the outcome
    /// stays).
    pub fn take_skipped(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.outcome.skipped)
    }
}

/// The UI state a d2rs hotkey action toggles (the action names are the
/// d2rs controls' `Action`s; the original key table is `ui/controls.md`).
pub fn hotkey_state(a: ActionId) -> Option<u8> {
    let i = usize::from(a.0);
    [
        (Action::ToggleInventory, UI_INVENTORY),
        (Action::ToggleCharacter, UI_CHARACTER),
        (Action::ToggleSkillTree, UI_SKILLTREE),
        (Action::ToggleQuests, quest_log_ui::UI_QUEST_SCREEN),
    ]
    .into_iter()
    .find(|(action, _)| action.index() == i)
    .map(|(_, ui)| ui)
}

/// A press or release of the left button (the panel handlers' mouse
/// down / up); other buttons are consumed inside an area and do nothing
/// (§4.4; which button means what is `ui/controls.md`).
fn left(e: UiEvent) -> Option<(bool, Point)> {
    match e {
        UiEvent::Press {
            button: PointerButton::Left,
            at,
        } => Some((true, at)),
        UiEvent::Release {
            button: PointerButton::Left,
            at,
        } => Some((false, at)),
        _ => None,
    }
}

fn is_click(e: UiEvent) -> bool {
    matches!(e, UiEvent::Press { .. } | UiEvent::Release { .. })
}

const EMPTY: Rect = Rect::new(0, 0, 0, 0);

/// Inventory (ui 1, §9.3): art, the gold line and gold button (§9.6,
/// `panels-2.md` §21 r1: the local player's full stat 14, drawn with the
/// fonts bound as the character values are) and the close button.
struct InventoryUi {
    sh: SharedRef,
    shop: shop_ui::SharedShop,
    panel: InventoryPanel,
}

impl Panel for InventoryUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_INVENTORY))
    }

    fn rect(&self) -> Rect {
        self.sh.borrow().right_area().unwrap_or(EMPTY)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        // `0x00625480(P, 14, 0)`: the total, layer 0 (`stat-lists.md`
        // §1 r3). Font16 is drawn only with the fonts bound (their DC6
        // are then in the frame's assets).
        let gold = match (&sh.fonts, local_player(ctx.world)) {
            (Some(_), Some((key, _))) => Some(ctx.world.total(key, inv_gold::STAT_GOLD, 0)),
            _ => None,
        };
        // The gold button's pressed flag is the gold dialog module's.
        let panel = InventoryPanel {
            gold_pressed: sh.gold.buttons.inv_pressed,
            ..self.panel
        };
        panel.draw(&sh.tables, &sh.env(), gold, out);
        let class = Facts::of(ctx.world).class;
        if let Some(l) = sh.items.layout(class, &sh.config.screen) {
            sh.items.draw_tints(ctx.world, &l, sh.mouse, out);
            sh.items.draw_panel(ctx.world, &sh.tables.files, &l, out);
        }
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        if !is_click(e) {
            return UiResponse::Ignored;
        }
        let mut sh = self.sh.borrow_mut();
        let s = sh.config.screen;
        if let UiEvent::Press {
            button: PointerButton::Right,
            at,
        } = e
        {
            let class = Facts::of(ctx.world).class;
            if let Some(l) = sh.items.layout(class, &s) {
                let out = sh.items.right_press(ctx.world, &sh.tables.files, &l, at);
                sh.outputs.extend(out);
            }
            return UiResponse::Consumed;
        }
        match left(e) {
            Some((true, at)) => {
                self.panel.press(&sh.tables, &s, at);
                let class = Facts::of(ctx.world).class;
                if let Some(l) = sh.items.layout(class, &s) {
                    // The shop's repair button is down: repair, not pick up.
                    let under = sh.items.item_under(ctx.world, &l.grid, &l, at);
                    let taken = shop_ui::ShopUi::repair_click(
                        &mut sh,
                        &mut self.shop.borrow_mut(),
                        ctx.world,
                        under.as_ref(),
                    );
                    if taken {
                        return UiResponse::Consumed;
                    }
                    let out = sh.items.press(ctx.world, &sh.tables.files, &l, at);
                    sh.outputs.extend(out);
                }
            }
            Some((false, at)) => {
                let out = self.panel.release(&sh.tables, &s, at);
                sh.outputs.extend(out);
            }
            None => {
                // Right press: use the item under the mouse (REC-117).
                if let UiEvent::Press {
                    button: PointerButton::Right,
                    at,
                } = e
                {
                    let class = Facts::of(ctx.world).class;
                    if let Some(l) = sh.items.layout(class, &s) {
                        let out = sh.items.use_press(ctx.world, &l, at);
                        sh.outputs.extend(out);
                    }
                }
            }
        }
        UiResponse::Consumed
    }
}

/// The skill tree's view of the model: the class, the icon file, the free
/// points and the entries joined from the skill rows and the local
/// player's list ([`skill_tree_ui::entries`]).
struct ModelSkillTree {
    class: Option<u8>,
    icon_file: Option<u32>,
    free_points: i32,
    entries: Vec<SkillEntry>,
}

impl ModelSkillTree {
    fn of(sh: &Shared, world: &ClientWorld) -> Self {
        let class = class_u8(sh.facts.class);
        ModelSkillTree {
            class,
            icon_file: class
                .and_then(skill_tree_ui::icon_file_name)
                .and_then(|n| sh.tables.files.id(&n)),
            free_points: world.local().map_or(0, |u| u.stat(5)),
            entries: class
                .map(|c| skill_tree_ui::entries(&sh.skill_tree, world, c))
                .unwrap_or_default(),
        }
    }
}

impl SkillTreeView for ModelSkillTree {
    fn class(&self) -> Option<u8> {
        self.class
    }
    fn icon_file(&self) -> Option<u32> {
        self.icon_file
    }
    fn free_points(&self) -> i32 {
        self.free_points
    }
    fn flag_mask(&self) -> u8 {
        skill_tree_ui::FLAG_INGAME
    }
    fn skills(&self) -> &[SkillEntry] {
        &self.entries
    }
}

fn class_u8(class: Option<u32>) -> Option<u8> {
    class.and_then(|c| u8::try_from(c).ok()).filter(|&c| c < 7)
}

/// Skill tree (ui 4, §10): back art per class and tab, tabs, close.
struct SkillTreeUi {
    sh: SharedRef,
    panel: SkillTreePanel,
}

impl Panel for SkillTreeUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_SKILLTREE))
    }

    fn rect(&self) -> Rect {
        self.sh.borrow().right_area().unwrap_or(EMPTY)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let view = ModelSkillTree::of(&sh, ctx.world);
        self.panel.draw(&sh.tables, &sh.env(), &view, sh.mouse, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        if !is_click(e) {
            return UiResponse::Ignored;
        }
        let mut sh = self.sh.borrow_mut();
        let env = sh.env();
        let view = ModelSkillTree::of(&sh, ctx.world);
        match left(e) {
            Some((true, at)) => {
                let out = self.panel.mouse_down(&env, &view, at);
                sh.outputs.extend(out);
                UiResponse::Consumed
            }
            Some((false, at)) => {
                let r = self.panel.mouse_up(&env, &view, at);
                sh.outputs.extend(r.out);
                // §10.8: x ≤ W / 2 is not consumed.
                if r.consumed {
                    UiResponse::Consumed
                } else {
                    UiResponse::Ignored
                }
            }
            None => UiResponse::Consumed,
        }
    }
}

/// Character (ui 2, §8): art and close button; with the fonts and a
/// local player also the stat block, values and name line
/// ([`ModelCharacter`]; the rest is in `PENDING`).
struct CharacterUi {
    sh: SharedRef,
    panel: CharacterPanel,
}

const CHARACTER: PanelKey = PanelKey::Ui(UI_CHARACTER);

/// The fonts the character panel draws in: Font8 (0), Font16 (1), Font6
/// (6) (§8.4, §8.7, §8.8, `panels-2.md` §17 r4).
pub const CHARACTER_FONTS: [u16; 3] = [0, 1, 6];

/// Glyph widths of the loaded font tables (`ui/text.md` §6): width A for
/// centering (§1.6), max width for the popup width of §8.8.
#[derive(Clone, Debug, Default)]
pub struct FontMeasure {
    tables: BTreeMap<u16, FontTable>,
}

impl FontMeasure {
    /// Font `id`'s table.
    pub fn insert(&mut self, id: u16, table: FontTable) {
        self.tables.insert(id, table);
    }

    /// Reads the `.tbl` of each font id (`text-fonts.tsv`, §1.3) from
    /// `source`. A font in no archive or that does not parse is an error.
    pub fn load(source: &dyn FileSource, ids: &[u16]) -> Result<Self, String> {
        let mut m = FontMeasure::default();
        for &id in ids {
            let info = super::font_info(id).ok_or(format!("font id {id} is not 0–13"))?;
            let archive = info.tbl_path.replace('/', "\\");
            let table = crate::assets::path::read_font_table(source, &archive)
                .ok_or(format!("{archive}: in no archive"))?
                .map_err(|e| format!("{archive}: {e}"))?;
            m.insert(id, table);
        }
        Ok(m)
    }

    /// `Wrap(text, max)` (`ui/text.md` §10): the lines; `None` without
    /// the font.
    pub fn wrap(&self, font: u16, text: &[u16], max: i32) -> Option<Vec<Vec<u16>>> {
        let g = super::text::GlyphLookup::new(self.tables.get(&font)?);
        let lines = super::text::wrap(&g, text, max).ok()?;
        Some(lines.into_iter().map(<[u16]>::to_vec).collect())
    }

    /// Width A of `text` (`ui/text.md` §6); `None` without the font.
    pub fn width_a(&self, font: u16, text: &[u16]) -> Option<i32> {
        TextMeasure::width(self, font, text)
    }

    /// Width C (`0x00501730`) of the whole `text`.
    pub fn width_c(&self, font: u16, text: &[u16]) -> Option<i32> {
        let g = super::text::GlyphLookup::new(self.tables.get(&font)?);
        super::text::width_c(&g, text, 0, text.len()).ok()
    }

    /// `0x00501840` max width of `text` in font `font` (§6); `None`
    /// without the font or for a code with no glyph record.
    pub fn max_width(&self, font: u16, text: &[u16]) -> Option<i32> {
        let g = super::text::GlyphLookup::new(self.tables.get(&font)?);
        super::text::max_width(&g, text).ok()
    }
}

impl FontMeasure {
    /// The pop-up text draw of `0x00502280(text, at, k, centre)` in font
    /// `font` (`ui/control-panel.md` §5 r14); `None` without the font or
    /// for a code with no glyph record.
    pub fn popup(
        &self,
        font: u16,
        text: &[u16],
        at: Point,
        centre: bool,
        screen: (i32, i32),
    ) -> Option<super::text::FramedText> {
        let g = super::text::GlyphLookup::new(self.tables.get(&font)?);
        super::text::popup_text(&g, text, at, centre, screen).ok()
    }
}

impl TextMeasure for FontMeasure {
    /// Width A (`0x00501820`, §6), the centering width (§1.6).
    fn width(&self, font: u16, text: &[u16]) -> Option<i32> {
        let g = super::text::GlyphLookup::new(self.tables.get(&font)?);
        super::text::width_a(&g, text).ok()
    }
}

/// The character panel's view of the client model (§Inputs): the local
/// player's full values (`total`, `0x00625480`) and bases (`base`,
/// `0x006253B0`), layer 0 (`client/stat-lists.md` §1 r3).
pub struct ModelCharacter<'a> {
    pub world: &'a ClientWorld,
    pub key: UnitKey,
    /// The popup width's font table (§8.8: Font16).
    pub fonts: &'a FontMeasure,
    /// `difficultylevels` `ResistPenalty` by difficulty (expansion game).
    pub penalties: &'a [i32],
    /// The extra tables and the `experience` rows ([`char_feed`]).
    pub chars: &'a super::char_feed::CharTables,
    pub experience: &'a [[u32; 7]],
}

impl CharacterView for ModelCharacter<'_> {
    fn stat(&self, id: u16) -> i32 {
        self.world.total(self.key, id, 0)
    }

    fn base(&self, id: u16) -> i32 {
        self.world.base(self.key, id, 0)
    }

    fn alive(&self) -> bool {
        self.world
            .units
            .get(&self.key)
            .is_some_and(|u| !u.is_dead())
    }

    /// English (`ui/text.md` §1.2: the only locale in scope).
    fn language(&self) -> u8 {
        0
    }

    /// §8.9 as subtracted by the panel: the negated `panels-2.md` §24 r2
    /// adjustment of the client's difficulty.
    fn resist_penalty(&self) -> i32 {
        let w = self.world;
        -char_inputs::resist_value(
            0,
            w.expansion != 0,
            usize::from(w.difficulty),
            self.penalties,
        )
    }

    /// The state tests (`0x0063A570` family) by the `states` flags.
    fn resist_effect(&self, id: u16) -> ResistEffect {
        match self.world.units.get(&self.key) {
            Some(u) => self.chars.resist_effect(&u.states, id),
            None => ResistEffect::None,
        }
    }

    fn defense_color(&self) -> Option<u16> {
        self.chars
            .defense_color(&self.world.units.get(&self.key)?.states)
    }

    fn next_level(&self) -> Option<u32> {
        let u = self.world.units.get(&self.key)?;
        let level = u32::try_from(self.base(12)).unwrap_or(0);
        let stat30 = u32::try_from(self.stat(30)).unwrap_or(0);
        super::char_feed::next_level(self.experience, u.class, level, stat30)
    }

    /// §8.8: the max width of the value in Font16.
    fn popup_width(&self, text: &[u16]) -> Option<i32> {
        self.fonts.max_width(1, text)
    }
}

/// The local player's key and 0x59 name (up to its NUL), when the model
/// has a local player unit.
pub(super) fn local_player(world: &ClientWorld) -> Option<(UnitKey, &[u8])> {
    let u = world.local().filter(|u| u.key.unit_type == PLAYER)?;
    let name: &[u8] = match &u.kind {
        KindData::Player(p) => {
            let n = p.name.iter().position(|&b| b == 0).unwrap_or(16);
            &p.name[..n]
        }
        _ => &[],
    };
    Some((u.key, name))
}

/// `panels-2.md` §17 r4: the name line, its font by code-point count,
/// centered in [`sx + 13`, `sx + 160`] at y `H + sy − 455`, color 0.
/// Not drawn when the font has no width for it.
pub fn name_line(
    name: &[u8],
    s: &super::layout::Screen,
    measure: &dyn TextMeasure,
    out: &mut dyn UiDrawSink,
) {
    let font = char_details::name_font(char_details::name_code_points(name));
    // `0x0047A210` → u16 (128 units): each byte a unit (the d2rs names
    // are ASCII, `play --new`; a save's name bytes as Latin-1).
    let s16: Vec<u16> = name.iter().take(128).map(|&b| u16::from(b)).collect();
    let Some(w) = measure.width(font, &s16) else {
        return;
    };
    let (a, b) = char_details::name_span(s);
    out.push(text(
        s16,
        centered_in(a, b, w),
        char_details::line_y(s),
        font,
        0,
    ));
}

impl CharacterUi {
    /// Art and close button only (no fonts or no local player).
    fn draw_static(&self, sh: &Shared, out: &mut dyn UiDrawSink) {
        let env = sh.env();
        let t = &sh.tables;
        // Draw rows in file order: the close rows with the close
        // button's pressed flag; rows needing panel words (stat points)
        // do not apply without them.
        for r in t.rows(CHARACTER) {
            if r.kind != RowKind::Draw {
                continue;
            }
            let pressed = r.item == "close" && self.panel.close_pressed;
            let cond = env.cond(pressed, &no_extra);
            let one = |x: &super::layout::LayoutRow| std::ptr::eq(x, r);
            emit_static_draws(t, CHARACTER, &cond, None, &one, out);
        }
    }
}

impl Panel for CharacterUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_CHARACTER))
    }

    /// §4.4: x in [`sx`, `W / 2 − 1`], y in [`sy`, `H + sy − 49`].
    fn rect(&self) -> Rect {
        character::area(&self.sh.borrow().config.screen)
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let expansion = ctx.world.expansion != 0;
        let penalties = match (&sh.resist_penalties, expansion) {
            (Some(p), _) => Some(p.as_slice()),
            // A classic game's penalty is fixed (§8.9).
            (None, false) => Some(&[][..]),
            (None, true) => None,
        };
        let (Some(fonts), Some(penalties), Some((key, name))) =
            (&sh.fonts, penalties, local_player(ctx.world))
        else {
            self.draw_static(&sh, out);
            return;
        };
        let view = ModelCharacter {
            world: ctx.world,
            key,
            fonts,
            penalties,
            chars: &sh.char_tables,
            experience: &sh.hud.tables.experience,
        };
        let env = sh.env();
        self.panel
            .draw(&sh.tables, &env, &view, fonts, ctx.strings, out);
        super::char_feed::damage_block(
            ctx.world,
            key,
            &sh.char_tables,
            ctx.strings,
            &env.screen,
            fonts,
            out,
        );
        if let Some(u) = ctx.world.units.get(&key) {
            super::char_feed::class_line(
                u.class,
                &sh.char_tables,
                ctx.strings,
                &env.screen,
                fonts,
                out,
            );
        }
        name_line(name, &env.screen, fonts, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, ctx: &UiCtx) -> UiResponse {
        if !is_click(e) {
            return UiResponse::Ignored;
        }
        let mut sh = self.sh.borrow_mut();
        let s = sh.config.screen;
        // `panels-2.md` §17 r1: the base stat points gate the add
        // buttons; the add rows are drawn only with the fonts bound.
        let statpts = match (&sh.fonts, local_player(ctx.world)) {
            (Some(_), Some((key, _))) => ctx.world.base(key, STAT_STATPTS, 0),
            _ => 0,
        };
        match left(e) {
            Some((true, at)) => self.panel.press(&sh.tables, &s, at, statpts),
            Some((false, at)) => {
                // Shift is the host's per-frame flag (`set_shift`): all points.
                let shift = sh.items.shift;
                let out = self.panel.release(&sh.tables, &s, at, shift, statpts);
                sh.outputs.extend(out);
            }
            None => {}
        }
        UiResponse::Consumed
    }
}

/// Border and control panel base (§6), drawn every frame (§5 step 7).
/// Its area is empty: it takes no event.
struct BorderUi {
    sh: SharedRef,
}

impl Panel for BorderUi {
    fn id(&self) -> PanelId {
        BORDER_PANEL
    }

    fn rect(&self) -> Rect {
        EMPTY
    }

    fn draw(&self, _ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        draw_border_and_ctrlpnl(&sh.tables, &sh.env(), out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, _e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        UiResponse::Ignored
    }
}

/// The hover tips (`panels.md` §5 step 10, `0x00503000`) and the cursor
/// item (`panels-3.md` §23 r9, drawn after the UI pass): after the border
/// and control panel (step 7), the HUD overlays and every other panel, so
/// a held item or a tip over the control panel stays on top. Its area is
/// empty: it takes no event.
struct TopUi {
    sh: SharedRef,
}

impl Panel for TopUi {
    fn id(&self) -> PanelId {
        TOP_PANEL
    }

    fn rect(&self) -> Rect {
        EMPTY
    }

    fn draw(&self, ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        // The belt item's hover text (`hud_belt`, `control-panel.md` §5
        // r8) as the r14 pop-up, in the belt's font 1 (§5 r4).
        if let Some(t) = sh
            .items
            .tips
            .as_ref()
            .and_then(|tips| sh.hud.belt.hover_tip(ctx.world, tips))
        {
            let (w, h) = (sh.config.screen.w, sh.config.screen.h);
            hud_tips::push_popup(
                t.text,
                Point::new(t.x, t.y),
                u16::from(t.color),
                t.centered,
                (w, h),
                sh.fonts.as_ref(),
                out,
            );
        }
        // The item tool tip over everything (`item_tip`).
        if sh.states.is_open(UI_INVENTORY) {
            let class = Facts::of(ctx.world).class;
            let lines = sh.items.hover_lines(
                ctx.world,
                &sh.tables.files,
                &sh.config.screen,
                class,
                sh.mouse,
            );
            let (w, h) = (sh.config.screen.w, sh.config.screen.h);
            item_tip::draw_tip(
                &lines,
                sh.mouse,
                (w, h),
                sh.fonts.as_ref(),
                &sh.tables.files,
                out,
            );
        }
        // The cursor item last (`panels-3.md` §23 r9).
        sh.items
            .draw_cursor(ctx.world, &sh.tables.files, (29, 29), sh.mouse, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, _e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        UiResponse::Ignored
    }
}

#[path = "controls_host.rs"]
pub mod controls_host;
#[path = "esc_art.rs"]
pub mod esc_art;
#[path = "esc_menu.rs"]
pub mod esc_menu;
#[path = "game_messages.rs"]
pub mod game_messages;
#[path = "gold_dialog.rs"]
pub mod gold_dialog;
#[path = "hud.rs"]
pub mod hud;
#[path = "options_menu.rs"]
pub mod options_menu;
#[path = "overhead_ui.rs"]
pub mod overhead_ui;

#[path = "hud_belt.rs"]
pub mod hud_belt;
#[path = "hud_tips.rs"]
pub mod hud_tips;

#[path = "cube_ui.rs"]
pub(super) mod cube_ui;
#[path = "msg_ui.rs"]
pub mod msg_ui;
#[path = "quest_log_ui.rs"]
pub mod quest_log_ui;
#[cfg(test)]
#[path = "quest_log_ui_tests.rs"]
mod quest_log_ui_tests;
#[path = "shop_ui.rs"]
pub mod shop_ui;
#[path = "stash_ui.rs"]
pub(super) mod stash_ui;
#[path = "waypoint_ui.rs"]
pub mod waypoint_ui;
pub use msg_ui::{
    ChatAction, IntroEntry, MsgUiMore, MsgUiState, NpcTextList, OverheadText, WaypointMenuState,
};
pub use shop_ui::ShopPrices;
pub use waypoint_ui::WaypointOpen;

#[cfg(test)]
#[path = "original_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "char_feed_tests.rs"]
mod char_feed_tests;

#[cfg(test)]
mod character_bind_tests {
    use super::*;
    use crate::assets::path::MemorySource;
    use crate::bridge::world::{ClientUnit, PlayerData};
    use crate::ui::{ClientIntent, NoPanelRules, NoStrings, UiDraw};

    /// A synthetic `.tbl` (`formats/font-tbl.md`): 256 records, record `i`
    /// of width `w`.
    pub(crate) fn tbl(w: u8) -> Vec<u8> {
        let mut d = b"Woo!".to_vec();
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&256u16.to_le_bytes());
        d.extend_from_slice(&[10, 0]);
        for i in 0..256u16 {
            d.extend_from_slice(&i.to_le_bytes());
            d.extend_from_slice(&[0, w, 10, 0, 0, 0]);
            d.extend_from_slice(&i.to_le_bytes());
            d.extend_from_slice(&[0; 4]);
        }
        d
    }

    fn fonts() -> FontMeasure {
        let mut src = MemorySource::default();
        for id in CHARACTER_FONTS {
            src.insert(crate::ui::font_info(id).unwrap().tbl_path, tbl(6));
        }
        FontMeasure::load(&src, &CHARACTER_FONTS).unwrap()
    }

    fn player(name: &[u8], expansion: bool) -> (ClientWorld, UnitKey) {
        let mut w = ClientWorld::default();
        let key = UnitKey::new(PLAYER, 1);
        let mut u = ClientUnit::new(key);
        u.class = 4;
        u.mode = 1;
        let mut p = PlayerData::default();
        p.name[..name.len()].copy_from_slice(name);
        u.kind = KindData::Player(p);
        for (stat, v) in [(0u16, 30), (12, 7), (6, 55 << 8), (7, 55 << 8), (39, 10)] {
            u.stats.insert(stat, v);
        }
        // A state list raising strength: total 35 > base 30 (blue).
        u.state_lists
            .insert(1, [((0u16, 0u16), 5)].into_iter().collect());
        w.units.insert(key, u);
        w.local_player = Some(key);
        w.expansion = u32::from(expansion);
        (w, key)
    }

    fn open(fonts: Option<FontMeasure>) -> (OriginalUi, UiRoot) {
        let config = UiConfig {
            screen: Screen::R800,
            expansion_installed: true,
        };
        let mut ui = OriginalUi::new(config, None).unwrap();
        if let Some(f) = fonts {
            ui.set_fonts(f);
        }
        let mut root = UiRoot::new(Box::new(NoPanelRules));
        ui.install(&mut root).unwrap();
        ui.set_ui(u32::from(UI_CHARACTER), 2, false).unwrap();
        root.sync_states(&ui.shared.borrow().states);
        (ui, root)
    }

    fn texts(root: &UiRoot, w: &ClientWorld) -> Vec<(String, i32, i32, u16, u16)> {
        let ctx = UiCtx {
            tick: 0,
            world: w,
            strings: &NoStrings,
        };
        let mut out: Vec<UiDraw> = Vec::new();
        root.draw(&ctx, &mut out);
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
    }

    fn find<'a>(
        v: &'a [(String, i32, i32, u16, u16)],
        s: &str,
    ) -> Option<&'a (String, i32, i32, u16, u16)> {
        v.iter().find(|t| t.0 == s)
    }

    // Covers: specs/ui/panels.md §8 r7, §1 r6; specs/ui/panels-2.md §17 r4; specs/client/stat-lists.md §1 r3
    #[test]
    fn values_and_name_come_from_the_model() {
        let (w, _) = player(b"Conan", false);
        let (_ui, root) = open(Some(fonts()));
        let t = texts(&root, &w);
        let s = Screen::R800;
        let (sx, y0) = (s.sx(), s.h + s.sy());
        // Level (stat 12): `sx + 13`, w 41, y `H + sy − 421`; "7" is 6
        // wide: x = sx + 13 + ((41 − 6) >> 1).
        assert_eq!(find(&t, "7"), Some(&("7".into(), sx + 30, y0 - 421, 1, 0)));
        // Strength: total 35 over base 30 → blue (3).
        let st = find(&t, "35").expect("strength");
        assert_eq!((st.2, st.4), (y0 - 381, 3));
        // Life `>> 8`, color 0 (current life is not compared).
        assert_eq!(find(&t, "55").map(|v| v.4), Some(0));
        // Fire resist, classic Normal: no penalty.
        assert!(find(&t, "10").is_some());
        // Name line (§17 r4): 5 code points → Font16; 30 wide centered in
        // [sx + 13, sx + 160].
        assert_eq!(
            find(&t, "Conan"),
            Some(&("Conan".into(), sx + 13 + ((148 - 30) >> 1), y0 - 455, 1, 0))
        );
    }

    // Covers: specs/ui/panels-2.md §17 r4
    #[test]
    fn long_names_switch_font() {
        let f = fonts();
        let mut out: Vec<UiDraw> = Vec::new();
        name_line(b"ABCDEFGHIJKL", &Screen::R800, &f, &mut out);
        name_line(b"ABCDEFGHIJKLM", &Screen::R800, &f, &mut out);
        let fonts: Vec<u16> = out
            .iter()
            .map(|d| match d {
                UiDraw::Text(t) => t.style.font,
                _ => panic!("text"),
            })
            .collect();
        assert_eq!(fonts, [0, 6]);
    }

    // Covers: specs/ui/panels.md §8 r9
    #[test]
    fn an_expansion_game_needs_the_resist_penalties() {
        let (w, _) = player(b"Conan", true);
        let (mut ui, root) = open(Some(fonts()));
        assert!(texts(&root, &w).is_empty(), "no table: no values");
        // `ResistPenalty` 0 / −40 / −100; Normal adds 0.
        ui.set_resist_penalties(vec![0, -40, -100]);
        assert!(find(&texts(&root, &w), "10").is_some());
        let mut hell = w.clone();
        hell.difficulty = 2;
        // 10 − 100 = −90. Its color (§8.9: red when shown < 0) is
        // `panels/character.rs`'s `resist_value`, which still applies the
        // §8.7 compare (handoff `play-char.md`, findings).
        assert!(find(&texts(&root, &hell), "-90").is_some());
    }

    // Covers: specs/ui/panels.md §8 r4, §8 r5
    #[test]
    fn stat_points_show_the_box_and_spend_through_the_root() {
        let (mut w, key) = player(b"Conan", false);
        w.units.get_mut(&key).unwrap().stats.insert(STAT_STATPTS, 5);
        let (mut ui, mut root) = open(Some(fonts()));
        let t = texts(&root, &w);
        assert!(find(&t, "5").is_some(), "{t:?}");
        // Strength's add button (117, 105): b = H + sy − 480 + 105.
        let s = Screen::R800;
        let at = Point::new(s.sx() + 130, s.h + s.sy() - 480 + 105 - 10);
        let ctx = UiCtx {
            tick: 0,
            world: &w,
            strings: &NoStrings,
        };
        for e in [
            UiEvent::Press {
                button: PointerButton::Left,
                at,
            },
            UiEvent::Release {
                button: PointerButton::Left,
                at,
            },
        ] {
            ui.before_event(e, &w);
            let r = root.dispatch(e, &ctx);
            ui.after_event(&mut root, e, r).unwrap();
        }
        assert_eq!(root.intents(), &[ClientIntent(vec![0x3A, 0, 0])]);
    }

    // Covers: specs/ui/panels.md §8 r1
    #[test]
    fn without_fonts_only_the_art_draws() {
        let (w, _) = player(b"Conan", false);
        let (_ui, root) = open(None);
        assert!(texts(&root, &w).is_empty());
        let missing = FontMeasure::load(&MemorySource::default(), &[1]);
        assert!(missing.is_err());
    }
}
