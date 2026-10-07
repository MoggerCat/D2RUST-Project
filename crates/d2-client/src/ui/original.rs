// Spec: specs/ui/panels.md, specs/client/msg-ui.md (via `msg_ui`)
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
//! skill list), character (ui 2: art and close button, §8.1–§8.3), the
//! border and control panel base (§6, every frame). The model holds no
//! input for the rest; each stays out with its reason in [`PENDING`].
//! The bridge's UI outputs (S→C 0x5D, 0x63, 0x77; `client/bridge.md`
//! §10) are applied by [`OriginalUi::apply_output`] (`msg_ui`).
//! Nothing here decides an outcome: the client sends intents and draws.

use std::cell::RefCell;
use std::rc::Rc;

use super::draw::UiDrawSink;
use super::geom::{Point, Rect};
use super::layout::{LayoutError, PanelKey, RowKind, Screen};
use super::panel::{ActionId, Panel, PanelId, UiCtx, UiEvent, UiResponse, WidgetId};
use super::panels::border::draw_border_and_ctrlpnl;
use super::panels::character::{self, CharacterPanel, UI_CHARACTER};
use super::panels::inventory::{InventoryPanel, UI_INVENTORY};
use super::panels::skilltree::{SkillEntry, SkillTreePanel, SkillTreeView, UI_SKILLTREE};
use super::panels::{emit_static_draws, no_extra, PanelEnv, PanelOutput, PanelTables, UiFiles};
use super::root::{Routed, UiError, UiRoot};
use super::states::{GateEnv, PlayerLife, UiEffect, UiStateError, UiStates};
use super::PointerButton;
use crate::audio::driver::SoundRequest;
use crate::bridge::msg::ui_npc::DialogCase;
use crate::bridge::output::NpcDialog;
use crate::bridge::world::{ClientWorld, PLAYER};
use crate::controls::Action;
use crate::rules::camera::OpenMode;

/// The border / control panel adapter's id: not a UI state (ids 0–37
/// are), so [`UiRoot::sync_states`] leaves it open.
pub const BORDER_PANEL: PanelId = PanelId(0x100);

/// The click sound of §10.2: `0x004B9A00(0, 0, 0)` = request id 0, no
/// unit, delay 0 (`audio/triggers.md` §1 r1).
pub const CLICK_SOUND_ID: i32 = 0;

/// Panels and inputs not wired, each with the input the client model or
/// the app does not hold yet (M02: named, not guessed).
pub const PENDING: &[(&str, &str)] = &[
    (
        "character values, labels, stat-point box and add buttons (§8.4–§8.9)",
        "the totals and bases are the model's (`ClientWorld::total` / `base`, \
         `client/stat-lists.md` §1 r3), but resist effects (`0x0063A570` family), the \
         expansion resist penalty (`0x00611D30`), the language, the popup width \
         (`0x00502520`) and the string lookup by id are not wired into the original UI; add \
         buttons stay inactive",
    ),
    (
        "inventory equipment backgrounds (§9.4)",
        "equipped items per body location: the item stream is not decoded \
         (`msg-stats-items.md` open question 3)",
    ),
    (
        "skill tree icons and level numbers (§10.3–§10.5)",
        "the skill list is in the model (`client/msg-skills.md`), but the icon file prefix \
         `CC` (spec open), the flag mask `[0x006CE270]` and the fields the tree shows \
         (`msg-skills.md` open question 3) are not",
    ),
    (
        "waypoint menu panel (ui 0x14, §13 r2–r7)",
        "S→C 0x63 opens it (flag, GUID, record: `msg_ui`), but the row rebuild, the tab \
         gates (client quest flags, `msg-ui.md` open question 4) and the tab / row click \
         rectangles (spec OQ 7) are not specified",
    ),
    (
        "stash and cube panels (ui 0x19, 0x1A; §11 r2–r6, §12 r2–r6)",
        "S→C 0x77 opens and closes them (flag and inventory mode: `msg_ui`); their art, \
         grids and buttons are not wired",
    ),
    (
        "NPC menu, NPC shop (ui 8, 0x0C; §14)",
        "their openers (NPC interaction messages) have no client handler",
    ),
    (
        "cursor jump (§4.3, `UiEffect::CursorX`)",
        "the waypoint menu passes jump 1 (S→C 0x63); the effect is reported but there is \
         no cursor-warp edge",
    ),
    (
        "hotkeys for other states (escape menu, chat, automap, quests, party)",
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
/// (exclusive), top, bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvArea {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl InvArea {
    /// The right panel's click area (§4.4). Bottom is read exclusive like
    /// right: the spec does not say (handoff `impl-ui-panels` §3).
    pub fn rect(&self) -> Rect {
        let w = u16::try_from(self.right - self.left).unwrap_or(0);
        let h = u16::try_from(self.bottom - self.top).unwrap_or(0);
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
        let shared = Shared {
            tables: PanelTables::load()?,
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
        };
        Ok(Self {
            shared: Rc::new(RefCell::new(shared)),
            outcome: UiOutcome::default(),
            msg: MsgUiState::default(),
            more: MsgUiMore::default(),
            npc_text: None,
            dialog_answer: None,
        })
    }

    /// Adds the wired panels to `root` in the §5 draw order: inventory
    /// family (step 5), skill tree then character (step 6), border and
    /// control panel (step 7).
    pub fn install(&self, root: &mut UiRoot) -> Result<(), UiError> {
        let sh = &self.shared;
        root.add(Box::new(InventoryUi {
            sh: sh.clone(),
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
        root.add(Box::new(BorderUi { sh: sh.clone() }))?;
        // Not a UI state: open for good.
        root.open(BORDER_PANEL)?;
        root.sync_states(&sh.borrow().states);
        Ok(())
    }

    /// The panel file registry ([`super::ImageRef::file`] ids).
    pub fn files(&self) -> UiFiles {
        self.shared.borrow().tables.files.clone()
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
                PanelOutput::SetUi { ui, mode, jump } => {
                    self.set_ui(u32::from(ui), u32::from(mode), jump)?;
                }
                PanelOutput::ClickSound => {
                    self.outcome.sounds.push(SoundRequest::Ui(CLICK_SOUND_ID))
                }
            }
        }
        if let (Routed::Unhandled, UiEvent::Action(a)) = (routed, e) {
            if let Some(ui) = hotkey_state(a) {
                // §4.3: hot keys pass jump 0; mode 2 toggle.
                self.set_ui(u32::from(ui), 2, false)?;
            }
        }
        root.sync_states(&self.shared.borrow().states);
        Ok(())
    }

    /// `SetUIState(ui, mode, jump)` with the model's gate facts; effects
    /// are kept for [`Self::take_outcome`].
    pub fn set_ui(&mut self, ui: u32, mode: u32, jump: bool) -> Result<bool, UiStateError> {
        let mut sh = self.shared.borrow_mut();
        let mut env = sh.gate_env();
        sh.states
            .set(ui, mode, jump, &mut env, &mut self.outcome.effects)
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

/// Inventory (ui 1, §9.3): art and close button.
struct InventoryUi {
    sh: SharedRef,
    panel: InventoryPanel,
}

impl Panel for InventoryUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_INVENTORY))
    }

    fn rect(&self) -> Rect {
        self.sh.borrow().right_area().unwrap_or(EMPTY)
    }

    fn draw(&self, _ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        self.panel.draw(&sh.tables, &sh.env(), out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        if !is_click(e) {
            return UiResponse::Ignored;
        }
        let mut sh = self.sh.borrow_mut();
        let s = sh.config.screen;
        match left(e) {
            Some((true, at)) => self.panel.press(&sh.tables, &s, at),
            Some((false, at)) => {
                let out = self.panel.release(&sh.tables, &s, at);
                sh.outputs.extend(out);
            }
            None => {}
        }
        UiResponse::Consumed
    }
}

/// The skill tree's view of the model: the class only. The model holds
/// no skill list, so there are no icons; free points and the flag mask
/// are read only for icons (inert, `PENDING`).
struct ModelSkillTree {
    class: Option<u8>,
}

impl SkillTreeView for ModelSkillTree {
    fn class(&self) -> Option<u8> {
        self.class
    }
    fn icon_file(&self) -> Option<u32> {
        None
    }
    fn free_points(&self) -> i32 {
        0
    }
    fn flag_mask(&self) -> u8 {
        0
    }
    fn skills(&self) -> &[SkillEntry] {
        &[]
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

    fn draw(&self, _ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
        let view = ModelSkillTree {
            class: class_u8(sh.facts.class),
        };
        self.panel.draw(&sh.tables, &sh.env(), &view, sh.mouse, out);
    }

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        if !is_click(e) {
            return UiResponse::Ignored;
        }
        let mut sh = self.sh.borrow_mut();
        let env = sh.env();
        let view = ModelSkillTree {
            class: class_u8(sh.facts.class),
        };
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

/// Character (ui 2, §8.1–§8.3): art and close button (the rest is in
/// `PENDING`).
struct CharacterUi {
    sh: SharedRef,
    panel: CharacterPanel,
}

const CHARACTER: PanelKey = PanelKey::Ui(UI_CHARACTER);

impl Panel for CharacterUi {
    fn id(&self) -> PanelId {
        PanelId(u16::from(UI_CHARACTER))
    }

    /// §4.4: x in [`sx`, `W / 2 − 1`], y in [`sy`, `H + sy − 49`].
    fn rect(&self) -> Rect {
        character::area(&self.sh.borrow().config.screen)
    }

    fn draw(&self, _ctx: &UiCtx, out: &mut dyn UiDrawSink) {
        let sh = self.sh.borrow();
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

    fn hit(&self, _p: Point) -> Option<WidgetId> {
        None
    }

    fn event(&mut self, e: UiEvent, _ctx: &UiCtx) -> UiResponse {
        if !is_click(e) {
            return UiResponse::Ignored;
        }
        let mut sh = self.sh.borrow_mut();
        let s = sh.config.screen;
        // Stat points unknown (`PENDING`): 0 keeps the add buttons out,
        // as their rows are not drawn.
        match left(e) {
            Some((true, at)) => self.panel.press(&sh.tables, &s, at, 0),
            Some((false, at)) => {
                let out = self.panel.release(&sh.tables, &s, at, false, 0);
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

#[path = "msg_ui.rs"]
pub mod msg_ui;
pub use msg_ui::{
    ChatAction, IntroEntry, MsgUiMore, MsgUiState, NpcTextList, OverheadText, WaypointMenuState,
};

#[cfg(test)]
#[path = "original_tests.rs"]
mod tests;
