//! The item handlers through the real host frame (drain → tick →
//! flush) on the wired host: `SimGame<ActionSim, WiredWorld>` whose
//! [`CubeParts`] come from synthetic tables (the economy wiring's
//! fixture shape), the real `d2-proto` sizes, real item units from
//! `Economy::create_item` on the action sim's own unit records and stat
//! lists (one unit world). The player's interaction is the wired host's
//! one owner (the quest tests' staged `Rest`). Only [`ItemPending`] (no
//! owner spec) is a fake: it logs.

use std::sync::{Arc, Mutex};

use d2_data::bin::BinTable;
use d2_data::fixup::maps::{EquivMatrix, StateMaps};
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Itemratio, Itemstatcost, Itemtypes, Record, States};
use d2_sim::combat::CombatTables;
use d2_sim::game::Game;
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{flag, q, ty, ItemRequest, ItemTables};
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::{ClassStats, StatData, StatTable, StateTable};
use d2_sim::units::hooks::UnitData;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::record::Units;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables};
use d2_sim::wiring::economy::{GameFields, ItemSpawn, ItemStore};
use d2_sim::world::cube::{
    input_flags, kind, CraftMod, CubeData, InputSlot, ItemRecord, OutputSlot, Recipe, CUBE_PAGE,
};
use d2_sim::world::npc::NpcControl;
use d2_sim::world::quests::{QuestControl, QuestTables};
use d2_sim::world::vendors::VendorTables;

use super::*;
use crate::adapters::handlers::world::tests::trade_quests::{ActionRest, Rest};
use crate::adapters::handlers::world::tests::waypoints::field_drlg;
use crate::adapters::handlers::world::{ActionEvents, ActionWorld, WiredWorld};
use crate::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use crate::dispatch::Outcome;
use crate::host::{Handled, Host};
use crate::seams::{Clock, PlayerGate, Pos, SessionHandler};

const N_STATS: usize = 359;
const N_TYPES: usize = 80;
const T_RING: u16 = 10;
const T_BOX: u16 = 11;
const T_AMULET: u16 = 12;
/// Item records (combined index).
const CUBE: usize = 0;
const RING: usize = 1;
const AMULET: usize = 2;
const GAME_SEED: u32 = 0x5EED;

fn set_u16(r: &mut [u8], o: usize, v: u16) {
    r[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

/// A plain itemstatcost through the d2-data fix-up.
fn stat_data() -> Arc<StatData> {
    let size = Itemstatcost::SIZE;
    let mut records = vec![0u8; N_STATS * size];
    for s in 0..N_STATS {
        let r = &mut records[s * size..(s + 1) * size];
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            set_u16(r, o, 0xFFFF);
        }
        set_u16(r, 0, s as u16);
    }
    let mut t = BinTable {
        name: "itemstatcost".into(),
        source: "synthetic".into(),
        count: N_STATS,
        record_size: size,
        records,
    };
    stat_ops(&mut t);
    let states = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: 0,
        record_size: States::SIZE,
        records: Vec::new(),
    };
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        classes: vec![ClassStats::default(); 7],
        states: StateTable::new(&states, &StateMaps::default()).expect("states"),
        damage_regen: vec![0; 8],
        aurastate: vec![0; 8],
        rescale_precision: d2_sim::stats::DEFAULT_RESCALE_PRECISION,
    })
}

fn equiv() -> EquivMatrix {
    let n = N_TYPES;
    let words = n.div_ceil(32);
    let mut m = EquivMatrix {
        n,
        words,
        bits: vec![0; n * words],
    };
    let mut set = |i: usize, j: usize| m.bits[i * words + j / 32] |= 1 << (j % 32);
    for i in 1..n {
        set(i, 0);
        set(i, i);
    }
    for c in [T_RING, T_BOX, T_AMULET] {
        set(usize::from(c), usize::from(ty::MISC));
    }
    m
}

fn item_rec(t: u16, code: &[u8; 4]) -> ItemRec {
    ItemRec {
        code: *code,
        type_: t as i16,
        level: 1,
        ..ItemRec::default()
    }
}

/// Items: the cube (`box `), a ring, an amulet.
fn tables() -> ItemTables {
    let mut ratio = Itemratio::decode(&[0u8; Itemratio::SIZE]);
    ratio.version = 1;
    let itemtypes = (0..N_TYPES)
        .map(|_| {
            let mut t = Itemtypes::decode(&[0u8; Itemtypes::SIZE]);
            t.class = 0xFF;
            t.staffmods = 0xFF;
            t.rare = 1;
            t
        })
        .collect();
    ItemTables {
        items: vec![
            item_rec(T_BOX, b"box "),
            item_rec(T_RING, b"rin "),
            item_rec(T_AMULET, b"amu "),
        ],
        itemtypes,
        equiv: equiv(),
        itemratio: vec![ratio],
        valshift: vec![0; N_STATS],
        stat_shift: 6,
        stat_mask: 0x3F,
        ..ItemTables::default()
    }
}

/// `cube.md` V12 recipe shape without mods: one ring → a normal amulet.
fn ring_to_amulet() -> Recipe {
    let mut inputs = [InputSlot::default(); 7];
    inputs[0] = InputSlot {
        flags: input_flags::USEANY,
        item: RING as u16,
        ..InputSlot::default()
    };
    let out = OutputSlot {
        kind: kind::ITEMCODE,
        item: AMULET as u16,
        quality: q::NORMAL,
        mods: [CraftMod {
            property: -1,
            ..CraftMod::default()
        }; 5],
        ..OutputSlot::default()
    };
    Recipe {
        enabled: 1,
        class: 0xFF,
        numinputs: 1,
        inputs,
        outputs: [out, OutputSlot::default(), OutputSlot::default()],
        ..Recipe::default()
    }
}

fn cube_data(t: &ItemTables) -> CubeData {
    CubeData {
        recipes: vec![ring_to_amulet()],
        items: t
            .items
            .iter()
            .map(|r| ItemRecord {
                code: r.code,
                level: r.level,
                spawnable: 1,
                ..ItemRecord::default()
            })
            .collect(),
        valshift: vec![0; N_STATS],
        max_level: 99,
    }
}

/// [`ItemPending`] fake: placement appends to the list, removal drops
/// from it; every call is logged.
#[derive(Clone, Default)]
struct Pending(Arc<Mutex<Vec<String>>>);

impl Pending {
    fn log(&self, s: String) {
        self.0.lock().unwrap().push(s);
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

impl ItemPending for Pending {
    fn inventory_pass(&mut self, _: UnitId, _: &mut Vec<Vec<u8>>) {
        self.log("inventory_pass".into());
    }
    fn place(
        &mut self,
        inv: &mut Inventory,
        _: UnitId,
        item: UnitId,
        _: &mut Vec<Vec<u8>>,
    ) -> bool {
        inv.items.push(item);
        if inv.cursor == Some(item) {
            inv.cursor = None;
        }
        self.log(format!("place {}", item.0));
        true
    }
    fn remove_cube_item(
        &mut self,
        inv: &mut Inventory,
        _: UnitId,
        item: UnitId,
        _: &mut Vec<Vec<u8>>,
    ) {
        inv.items.retain(|&i| i != item);
        self.log(format!("remove {}", item.0));
    }
    fn socketed(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn duplicate(&mut self, _: UnitId, _: bool) -> Option<UnitId> {
        None
    }
    fn tempered_affix(&mut self, _: UnitId, _: bool) -> u16 {
        0
    }
    fn drop_runeword_stats(&mut self, _: UnitId) {}
    fn repair(&mut self, _: UnitId) {}
    fn recharge(&mut self, _: UnitId) {}
    fn quest_item_hook(&mut self, _: UnitId, _: UnitId, _: [u8; 4]) {}
    fn cow_portal(&mut self, _: UnitId) -> bool {
        false
    }
}

#[derive(Default)]
struct Session;

impl SessionHandler for Session {
    fn system_message(&mut self, _: ClientId, _: &[u8], _: usize, _: &mut dyn MessageSink) {}
}

struct Manual(u32);

impl Clock for Manual {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

type Sim = SimGame<ActionSim<ActionRest>, WiredWorld<Rest>>;
type TestHost = Host<Sim, ProtoSizes, Session, Manual>;

/// A host for client 0 with a player (class 2), a cube and a ring in
/// mode `ring_mode`; the cube is stored (mode 0) in the player's
/// inventory list. One frame has run (it only sets the tick driver).
struct T {
    host: TestHost,
    player: UnitId,
    cube: UnitId,
    ring: UnitId,
    pending: Pending,
}

const ALIVE: PlayerFields = PlayerFields {
    gate: PlayerGate {
        mode: 1,
        uninterruptable: false,
    },
    data: Some(PlayerData { last_accept: 0 }),
};

fn item(sim: &mut Sim, class: usize, mode: u32) -> UnitId {
    let mut rq = ItemRequest {
        item: class as i32,
        format: 101,
        ilvl: 5,
        quality: q::NORMAL,
        ..ItemRequest::default()
    };
    let spawn = ItemSpawn {
        room: None,
        mode,
        init_flags: 1,
    };
    let (game, events, world) = (&mut sim.game, &mut sim.events, &mut sim.world);
    world
        .with_economy(game, events, |econ, _| {
            econ.create_item(&mut rq, false, spawn)
        })
        .unwrap()
}

fn action_tables() -> ActionTables {
    ActionTables {
        missiles: Vec::new(),
        skills: SkillTables {
            skills: Vec::new(),
            skilldesc: Vec::new(),
            missiles: Vec::new(),
            skills_code: Vec::new(),
            miss_code: Vec::new(),
            level_cap: 0,
            stat_count: 0,
        },
        combat: CombatTables {
            charstats: Vec::new(),
            difficultylevels: Vec::new(),
            monstats: Vec::new(),
            monstats2: Vec::new(),
            hitclass: Vec::new(),
        },
        levels: Vec::new(),
        skill_modes: Vec::new(),
    }
}

/// Game creation on the action sim (expansion, the game seed), the
/// NPC control and the quests on the game seed, the wired host with the
/// cube's parts; a player (class 2), a cube and a ring in mode
/// `ring_mode`, real units of the action sim; the cube is stored (mode
/// 0) in the player's inventory list.
fn setup(ring_mode: u32) -> T {
    let pending = Pending::default();
    let tables = tables();
    let hooks = ActionHooks::new(
        Arc::new(action_tables()),
        field_drlg(),
        Seed::init(),
        ActionRest::default(),
    );
    let mut events = ActionSim::new(stat_data(), UnitData::default(), hooks);
    events.create_game(&GameFields::new(Seed::init_low(GAME_SEED), true));
    let mut game = Game::new();
    game.lists.ensure_act(0).unwrap();
    let mut seed = events.hooks().game_seed;
    let npc = NpcControl::new(&[], Vec::new(), true, 0, &mut seed).unwrap();
    let quests = QuestControl::new(&QuestTables::load().unwrap(), &mut seed).unwrap();
    events.hooks().game_seed = seed;
    let mut world = WiredWorld::new(
        ActionWorld::default(),
        tables,
        quests,
        npc,
        VendorTables::default(),
        Rest::default(),
        1000,
    );
    let mut parts = CubeParts::new(cube_data(&world.tables), Box::new(pending.clone()));
    parts.staged.local_date = (15, 3);
    world.cube = Some(parts);
    let req = AllocRequest {
        ty: UnitType::Player,
        class: 2,
        room: None,
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let player = events
        .with(&mut game, |g, v| v.allocate(g, &req, 0, 0))
        .unwrap();
    let mut sim: Sim = SimGame::with_world(game, events, world);
    let cube = item(&mut sim, CUBE, 0);
    let ring = item(&mut sim, RING, ring_mode);
    sim.world.items.get_mut(cube).unwrap().inv_page = 0;
    sim.world.items.get_mut(ring).unwrap().inv_page = 0;
    sim.world.cube.as_mut().unwrap().staged.inventories.insert(
        player,
        Inventory {
            items: vec![cube],
            cursor: None,
        },
    );
    sim.join(0, Some(player), None, client_state::IN_GAME)
        .unwrap();
    sim.set_player(player, ALIVE);
    let at = |x, y| UnitFacts {
        act: 0,
        pos: Pos { x, y },
        owner: None,
    };
    sim.set_unit(player, at(100, 100));
    sim.set_unit(ring, at(100, 100));
    let mut host = Host::new(sim, ProtoSizes, Session, Manual(1000));
    host.connect(0);
    host.frame().unwrap();
    T {
        host,
        player,
        cube,
        ring,
        pending,
    }
}

impl T {
    fn world(&mut self) -> &mut WiredWorld<Rest> {
        &mut self.host.game.world
    }
    /// The cube's parts of the host.
    fn cube(&mut self) -> &mut CubeParts {
        self.world().cube.as_mut().unwrap()
    }
    /// The game's one item store.
    fn items(&mut self) -> &mut ItemStore {
        &mut self.world().items
    }
    /// The action sim's unit records (the game's one unit store).
    fn units(&mut self) -> &mut Units {
        &mut self.host.game.events.sys.units
    }
    fn guid(&mut self, u: UnitId) -> u32 {
        self.units().get(u).unwrap().guid
    }
    fn inventory(&mut self) -> &mut Inventory {
        let p = self.player;
        self.cube().staged.inventories.get_mut(&p).unwrap()
    }
    /// The player's interaction, at its one owner.
    fn interact(&mut self, unit_type: u8, guid: u32) {
        let p = self.player;
        self.world().rest.interact.insert(p, (unit_type, guid));
    }
    /// 0x2A with the ring and the cube.
    fn put_msg(&mut self) -> Vec<u8> {
        let mut m = vec![ITEM_TO_CUBE];
        let (r, c) = (self.ring, self.cube);
        m.extend_from_slice(&self.guid(r).to_le_bytes());
        m.extend_from_slice(&self.guid(c).to_le_bytes());
        m
    }
    /// One frame (drain → tick → flush) with `msg` sent: the dispatch
    /// result and the bytes client 0 receives. Frames are 240 ms apart,
    /// past the client's 200 ms duplicate filter (`intents-events.md`
    /// §2.1 rule 1), so a test can resend a message.
    fn frame(&mut self, msg: &[u8]) -> (ResultCode, Vec<Vec<u8>>) {
        self.host.clock.0 += 240;
        self.host.send_game(0, msg).unwrap();
        let r = self.host.frame().unwrap();
        assert!(r.ticked);
        assert_eq!(r.messages.len(), 1, "{:?}", r.messages);
        let Handled::Game(Outcome::Dispatched(code)) = r.messages[0].handled else {
            panic!("{:?}", r.messages[0]);
        };
        if let Some(c) = &self.host.game.world.cube {
            assert_eq!(c.errors, Vec::new());
        }
        assert!(self.host.game.events.sys.hooks.errors.is_empty());
        (code, self.host.receive(0))
    }
    fn page(&mut self, u: UnitId) -> u8 {
        self.items().get(u).unwrap().inv_page
    }
}

fn click(button: u16) -> [u8; 7] {
    let b = button.to_le_bytes();
    [CLICK_BUTTON, b[0], b[1], 0, 0, 0, 0]
}

const NO_BYTES: Vec<Vec<u8>> = Vec::new();

/// 0x2A with the cursor item: checks pass, the targeting reset clears
/// flag 0x4 on the inventory items, the item gets page 3 and goes to
/// placement; result 0, nothing sent.
// Covers: specs/world/cube.md §2 r1, §2 r2, §2 r3, §2 r4
#[test]
fn item_to_cube_puts_the_cursor_item_in() {
    let mut t = setup(4);
    let (ring, cube, player) = (t.ring, t.cube, t.player);
    t.inventory().cursor = Some(ring);
    t.items().get_mut(cube).unwrap().flags |= 0x4 | flag::IDENTIFIED;
    let m = t.put_msg();
    assert_eq!(t.frame(&m), (ResultCode::Done, NO_BYTES));
    assert_eq!(t.page(ring), CUBE_PAGE);
    assert_eq!(t.items().get(cube).unwrap().flags & 0x14, flag::IDENTIFIED);
    assert_eq!(t.pending.take(), [format!("place {}", ring.0)]);
    assert_eq!(t.cube().staged.targeting_resets, [player]);
    assert_eq!(t.inventory().items, [cube, ring]);
    assert!(t.host.game.unhandled.is_empty());
}

/// V25: the item stored in the backpack (mode 0) passes step 1 and is
/// refused at step 3.4 → 3; nothing moves.
// Covers: specs/world/cube.md §2 r3, §2 r4
#[test]
fn item_to_cube_stored_item_is_refused() {
    let mut t = setup(0);
    let ring = t.ring;
    t.inventory().items.push(ring);
    let m = t.put_msg();
    assert_eq!(t.frame(&m), (ResultCode::Malformed, NO_BYTES));
    assert_eq!(t.page(ring), 0);
    assert!(t.pending.take().is_empty());
    assert_eq!(t.cube().staged.targeting_resets.len(), 1);
}

/// Step 1 and 2 refusals, before the targeting reset.
// Covers: specs/world/cube.md §2 r1, §2 r2
#[test]
fn item_to_cube_checks() {
    // Missing item → 1.
    let mut t = setup(4);
    let mut m = t.put_msg();
    m[1..5].copy_from_slice(&0xDEADu32.to_le_bytes());
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    // Cursor-mode item that is not the cursor item → 1.
    let m = t.put_msg();
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    // Mode > 4 → 1.
    let ring = t.ring;
    t.units().get_mut(ring).unwrap().mode = 5;
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    // Cube not in the inventory → 1.
    t.units().get_mut(ring).unwrap().mode = 4;
    t.inventory().cursor = Some(ring);
    t.inventory().items.clear();
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    // Cube not stored → 1.
    let cube = t.cube;
    t.inventory().items.push(cube);
    t.units().get_mut(cube).unwrap().mode = 4;
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    assert!(t.cube().staged.targeting_resets.is_empty());
    assert!(t.pending.take().is_empty());
}

/// Ground items (mode 3): another act → 2; the range-10 test on both
/// axes (10 passes, 11 fails).
// Covers: specs/world/cube.md §2 r1
#[test]
fn item_to_cube_ground_item() {
    let mut t = setup(3);
    let ring = t.ring;
    let m = t.put_msg();
    let at = |act, x, y| UnitFacts {
        act,
        pos: Pos { x, y },
        owner: None,
    };
    t.host.game.set_unit(ring, at(1, 100, 100));
    assert_eq!(t.frame(&m).0, ResultCode::Invalid);
    t.host.game.set_unit(ring, at(0, 111, 100));
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    t.host.game.set_unit(ring, at(0, 100, 89));
    assert_eq!(t.frame(&m).0, ResultCode::Refused);
    assert_eq!(t.page(ring), 0);
    t.host.game.set_unit(ring, at(0, 110, 90));
    assert_eq!(t.frame(&m), (ResultCode::Done, NO_BYTES));
    assert_eq!(t.page(ring), CUBE_PAGE);
    assert_eq!(t.pending.take(), [format!("place {}", ring.0)]);
}

/// Trading and the cube's page ≠ 0: sound event 19, result 0, nothing
/// moved (§2 step 3.3).
// Covers: specs/world/cube.md §2 r3
#[test]
fn item_to_cube_while_trading() {
    let mut t = setup(4);
    let (ring, cube, player) = (t.ring, t.cube, t.player);
    t.inventory().cursor = Some(ring);
    let g = t.guid(player);
    t.interact(0, g);
    t.items().get_mut(cube).unwrap().inv_page = 1;
    let m = t.put_msg();
    assert_eq!(t.frame(&m), (ResultCode::Done, NO_BYTES));
    assert_eq!(t.cube().staged.sounds, [(player, 19)]);
    assert_eq!(t.page(ring), 0);
    assert!(t.pending.take().is_empty());
    // Page 0: not refused.
    t.items().get_mut(cube).unwrap().inv_page = 0;
    assert_eq!(t.frame(&m).0, ResultCode::Done);
    assert_eq!(t.page(ring), CUBE_PAGE);
}

/// V24: 0x4F with no active interaction queues 0x77 0x0C (any button);
/// result 0. The exact bytes reach the client after the tick's flush.
// Covers: specs/world/cube.md §1
#[test]
fn click_button_without_interaction() {
    let mut t = setup(4);
    for button in [0x18, 0x17, 0x01] {
        assert_eq!(
            t.frame(&click(button)),
            (ResultCode::Done, vec![vec![0x77, 0x0C]])
        );
    }
    assert!(t.host.game.unhandled.is_empty());
}

/// Buttons 0x17 / 0x18 with an interaction that is not the cube's → 3;
/// another button with an active interaction is not the cube's: it
/// stays the stub.
// Covers: specs/world/cube.md §1
#[test]
fn click_button_other_interaction() {
    let mut t = setup(4);
    t.interact(2, 77);
    assert_eq!(t.frame(&click(0x17)), (ResultCode::Malformed, NO_BYTES));
    assert_eq!(t.frame(&click(0x18)), (ResultCode::Malformed, NO_BYTES));
    assert_eq!(t.frame(&click(0x01)), (ResultCode::Done, NO_BYTES));
    assert_eq!(t.host.game.unhandled, [(0, CLICK_BUTTON, 7)]);
    assert!(t.pending.take().is_empty());
}

/// 0x17 with the cube open: the interaction resets (GUID −1, type 6,
/// inactive), then the inventory pass; result 0.
// Covers: specs/world/cube.md §1
#[test]
fn click_button_closes_the_cube() {
    let mut t = setup(4);
    let (cube, player) = (t.cube, t.player);
    let g = t.guid(cube);
    t.interact(4, g);
    assert_eq!(t.frame(&click(0x17)), (ResultCode::Done, NO_BYTES));
    // Reset (GUID −1, type 6, inactive) at the owner: no interaction.
    assert_eq!(t.world().rest.interact.get(&player), None);
    assert_eq!(t.pending.take(), ["inventory_pass"]);
    // Closed: the next click has no interaction.
    assert_eq!(
        t.frame(&click(0x18)),
        (ResultCode::Done, vec![vec![0x77, 0x0C]])
    );
}

/// 0x18 with the cube open: the ring in the cube matches the recipe;
/// the amulet is created on a real unit, the ring is removed and freed,
/// sound 4, the amulet is placed with page 3 and identified (§8).
/// Empty cube: nothing (§3 step 1). Any type-4 GUID transmutes.
// Covers: specs/world/cube.md §1, §3 r1, §8 r1, §8 r2, §8 r3
#[test]
fn click_button_transmutes() {
    let mut t = setup(0);
    let (ring, player) = (t.ring, t.player);
    t.interact(4, 12345);
    assert_eq!(t.frame(&click(0x18)), (ResultCode::Done, NO_BYTES));
    assert!(t.pending.take().is_empty());
    assert!(t.cube().staged.sounds.is_empty());

    t.inventory().items.push(ring);
    t.items().get_mut(ring).unwrap().inv_page = CUBE_PAGE;
    assert_eq!(t.frame(&click(0x18)), (ResultCode::Done, NO_BYTES));
    let log = t.pending.take();
    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(log[0], format!("remove {}", ring.0));
    assert!(!t.items().contains(ring));
    let amu = *t.inventory().items.last().unwrap();
    assert_eq!(log[1], format!("place {}", amu.0));
    let it = t.items().get(amu).unwrap().clone();
    assert_eq!(
        (it.record, it.quality, it.inv_page),
        (AMULET, q::NORMAL, CUBE_PAGE)
    );
    assert_ne!(it.flags & flag::IDENTIFIED, 0);
    assert_eq!(t.cube().staged.sounds, [(player, 4)]);
}

/// Item ids no written spec owns stay stubs (recorded, result 0), as do
/// owned ids whose system the host lacks (0x17: this host has no
/// inventory parts, `moves`); the table names an owner for exactly the
/// handled ids.
// Covers: specs/sim/intents-events.md §4 r1
#[test]
fn unowned_item_ids_stay_stubs() {
    let mut t = setup(4);
    let g = t.guid(t.ring);
    let mut drop = vec![0x17];
    drop.extend_from_slice(&g.to_le_bytes());
    assert_eq!(t.frame(&drop), (ResultCode::Done, NO_BYTES));
    let mut tmog = vec![0x4C];
    tmog.extend_from_slice(&g.to_le_bytes());
    assert_eq!(t.frame(&tmog), (ResultCode::Done, NO_BYTES));
    assert_eq!(t.host.game.unhandled, [(0, 0x17, 5), (0, 0x4C, 5)]);
    let owned: Vec<u8> = ITEM_IDS
        .iter()
        .filter(|(_, o)| o.is_some())
        .map(|&(id, _)| id)
        .collect();
    let mut want: Vec<u8> = moves::MOVE_IDS.iter().map(|&(id, _, _)| id).collect();
    want.extend([ITEM_TO_CUBE, CLICK_BUTTON]);
    want.sort_unstable();
    assert_eq!(owned, want);
    assert!(ITEM_IDS.windows(2).all(|w| w[0].0 < w[1].0));

    // Without the cube on the host the owned ids are stubs too.
    t.host.game.world.cube = None;
    assert_eq!(t.frame(&click(0x18)), (ResultCode::Done, NO_BYTES));
    assert_eq!(t.host.game.unhandled.last(), Some(&(0, CLICK_BUTTON, 7)));
}
