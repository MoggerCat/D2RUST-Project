// Spec: specs/sim/intents-events.md (§2.2–§2.4, §4 rule 1)
//! Robustness properties (METHODS M07, `CLAUDE.md` hard rule 7) of the
//! intent handlers on the wired host: every C→S game id, with payload
//! fields drawn from the real unit GUIDs and from out-of-range values
//! (unit types, GUIDs, skill and stat ids, coordinates, item positions),
//! goes through the dispatcher into `SimGame::handle` without a panic and
//! with a frame that does not fail.
//!
//! Two hosts, as the server's handler tests build them:
//! - the action host: `SimGame<ActionSim<_>, ActionWorld>` with the wired
//!   skill handlers (`WiredSkills`) and waypoint tables: a player, a
//!   monster, a ground item and a waypoint object;
//! - the item host: `SimGame` with an `ItemWorld` (the cube handlers): a
//!   player, a cube in the inventory and a ring.
//!
//! The dispatcher's own rejections (§2.3 rule 2, §2.4 rules 1–4: null
//! handler, stub, wrong size, unit type, point range) run before any
//! handler, so the game state is compared before and after them. A
//! handler's refusal is not checked that way: its spec may order state
//! changes before it (`cube.md` §2 step 3.1).
//!
//! Default case counts are small so `cargo test` stays fast; set
//! `PROPTEST_CASES` to hunt harder (it overrides every default here).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use d2_data::bin::BinTable;
use d2_data::fixup::maps::{EquivMatrix, StateMaps};
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{
    Charstats, Experience, Itemratio, Itemstatcost, Itemtypes, Levels, Objects, Record, Skilldesc,
    Skills, States,
};
use d2_proto::schema::FieldType;
use d2_proto::CLIENT_MESSAGES;
use d2_server::adapters::handlers::items::{Inventory, ItemHooks, ItemPending, ItemWorld, Staged};
use d2_server::adapters::handlers::skills::seams::SkillSeams;
use d2_server::adapters::handlers::skills::wired::WiredSkills;
use d2_server::adapters::handlers::world::{ActionWorld, Outbox};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::buffers::ClientBuffers;
use d2_server::dispatch::{dispatch, gate, is_point, is_unit, kind, Gate, Kind};
use d2_server::seams::{PlayerGate, Pos, ResultCode, Tick};
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::combat::{CombatTables, RoomKind};
use d2_sim::drlg::data::DrlgData;
use d2_sim::drlg::{Dungeon, NoLevelTypes, TileInfo, TileSource};
use d2_sim::game::Game;
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{q, ty, ItemRequest, ItemTables};
use d2_sim::rng::Seed;
use d2_sim::skills::use_::{MissileAim, UseState};
use d2_sim::skills::{SkillEntry, SkillTables, LEVEL_CAP_114D};
use d2_sim::stats::{ClassStats, StatData, StatLists, StatTable, StateTable};
use d2_sim::tick::EventDispatch;
use d2_sim::units::hooks::{MonsterInfo, Sim, UnitData};
use d2_sim::units::lifecycle::{allocate, AllocRequest};
use d2_sim::units::lists::client_state;
use d2_sim::units::record::Units;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables, DrlgWorld, Pending};
use d2_sim::wiring::economy::{GameFields, ItemSpawn, ItemStore};
use d2_sim::world::cube::{
    input_flags, kind as cube_kind, CraftMod, CubeData, InputSlot, ItemRecord, OutputSlot, Recipe,
};
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};
use proptest::prelude::*;
use proptest::test_runner::Config;

/// Proptest config with `default` cases, or `PROPTEST_CASES` when set.
fn config(default: u32) -> Config {
    let cases = std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default);
    Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    }
}

const N_STATS: usize = 359;

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

fn set_u16(r: &mut [u8], o: usize, v: u16) {
    r[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

/// A plain itemstatcost (359 stats, no ops, no shifts) through the
/// d2-data fix-up, with an empty states table.
fn stat_data() -> Arc<StatData> {
    let size = Itemstatcost::SIZE;
    let mut records = vec![0u8; N_STATS * size];
    for (s, r) in records.chunks_mut(size).enumerate() {
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

// ---- the action host -------------------------------------------------------------------

struct NoTiles;
impl TileSource for NoTiles {
    fn dt1(&self, _: &[u8]) -> Option<&[TileInfo]> {
        None
    }
}

/// A skills record with no formulas and no required skills.
fn skill_rec() -> Skills {
    let mut s: Skills = blank();
    for f in [
        &mut s.auralencalc,
        &mut s.aurarangecalc,
        &mut s.aurastatcalc1,
        &mut s.calc1,
        &mut s.calc2,
        &mut s.calc3,
        &mut s.calc4,
        &mut s.passivecalc1,
        &mut s.passivecalc2,
        &mut s.passivecalc3,
        &mut s.passivecalc4,
        &mut s.passivecalc5,
        &mut s.petmax,
        &mut s.skpoints,
        &mut s.tohitcalc,
        &mut s.dmgsympercalc,
        &mut s.edmgsympercalc,
        &mut s.elensympercalc,
        &mut s.delay,
        &mut s.perdelay,
    ] {
        *f = 0xFFFF_FFFF;
    }
    s.skilldesc = 0xFFFF;
    s.charclass = 0xFF;
    s.reqskill1 = 0xFFFF;
    s.reqskill2 = 0xFFFF;
    s.reqskill3 = 0xFFFF;
    s.itypea1 = 0xFFFF;
    s.srvmissile = 0xFFFF;
    s.intown = true;
    s.ingame = true;
    s
}

/// Attack (0), Multiple Shot (1: srvst 4, mana), Might (2: aura), a
/// learnable skill (3: max level 3).
fn skills() -> SkillTables {
    let mut v: Vec<Skills> = (0..4).map(|_| skill_rec()).collect();
    let m = &mut v[1];
    (m.srvstfunc, m.mana, m.lvlmana, m.manashift) = (4, 4, 1, 8);
    let m = &mut v[2];
    (m.aura, m.immediate, m.perdelay, m.srvdofunc, m.aurastate) = (true, true, 0, 65, 33);
    v[3].maxlvl = 3;
    SkillTables {
        skills: v,
        skilldesc: vec![blank::<Skilldesc>()],
        missiles: Vec::new(),
        skills_code: vec![0x07, 50, 0x00],
        miss_code: Vec::new(),
        level_cap: LEVEL_CAP_114D,
        stat_count: N_STATS as _,
    }
}

/// `vitals.md` Constants: 1.14d `charstats`, `experience` MaxLvl 99.
fn vitals() -> VitalsTables {
    let rows: [[u8; 13]; 7] = [
        [20, 25, 15, 20, 84, 30, 8, 4, 6, 12, 4, 6, 5],
        [10, 25, 35, 10, 74, 30, 4, 4, 8, 8, 4, 8, 5],
        [15, 25, 25, 15, 79, 30, 6, 4, 8, 8, 4, 8, 5],
        [25, 20, 15, 25, 89, 30, 8, 4, 6, 12, 4, 6, 5],
        [30, 20, 10, 25, 92, 30, 8, 4, 4, 16, 4, 4, 5],
        [15, 20, 20, 25, 84, 30, 6, 4, 8, 8, 4, 8, 5],
        [20, 20, 25, 20, 95, 30, 8, 5, 6, 12, 5, 7, 5],
    ];
    let charstats = rows
        .iter()
        .map(|r| {
            let mut c: Charstats = blank();
            (c.str, c.dex, c.int, c.vit, c.stamina, c.hpadd) = (r[0], r[1], r[2], r[3], r[4], r[5]);
            (c.lifeperlevel, c.staminaperlevel, c.manaperlevel) = (r[6], r[7], r[8]);
            (c.lifepervitality, c.staminapervitality, c.manapermagic) = (r[9], r[10], r[11]);
            c.statperlevel = r[12];
            c
        })
        .collect();
    let mut row: Experience = blank();
    row.amazon = 99;
    VitalsTables {
        charstats,
        experience: vec![row],
    }
}

/// The skill seams no written spec provides: a fixed skill list (every
/// skill of the table), the right hand on Multiple Shot.
#[derive(Default)]
struct Inner {
    list: Vec<SkillEntry>,
    left: Option<SkillEntry>,
    right: Option<SkillEntry>,
    used: Option<SkillEntry>,
}

#[derive(Clone, Default)]
struct Book(Arc<Mutex<Inner>>);

impl Book {
    fn get(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap()
    }
}

impl SkillSeams for Book {
    fn skill_list(&self, _: UnitId) -> Vec<SkillEntry> {
        self.get().list.clone()
    }
    fn used_skill(&self, _: UnitId) -> Option<SkillEntry> {
        self.get().used
    }
    fn set_used_skill(&mut self, _: UnitId, e: Option<SkillEntry>) {
        self.get().used = e;
    }
    fn left_skill(&self, _: UnitId) -> Option<SkillEntry> {
        self.get().left
    }
    fn right_skill(&self, _: UnitId) -> Option<SkillEntry> {
        self.get().right
    }
    fn set_left_skill(&mut self, _: UnitId, e: SkillEntry) {
        self.get().left = Some(e);
    }
    fn set_right_skill(&mut self, _: UnitId, e: SkillEntry) {
        self.get().right = Some(e);
    }
    fn find_entry(&self, _: UnitId, skill: i32) -> Option<SkillEntry> {
        self.get().list.iter().copied().find(|e| e.skill == skill)
    }
    fn find_entry_owned(&self, _: UnitId, skill: i32, owner: i32) -> Option<SkillEntry> {
        self.get()
            .list
            .iter()
            .copied()
            .find(|e| e.skill == skill && e.owner_guid == owner)
    }
    fn entry_mode(&self, _: UnitId, _: &SkillEntry) -> u32 {
        0
    }
    fn use_state(&mut self, _: UnitId, _: &SkillEntry) -> UseState {
        UseState::Usable
    }
    fn srvst(&mut self, _: u16, _: UnitId, _: i32, _: i32) -> i32 {
        1
    }
    fn srvdo(&mut self, _: u16, _: UnitId, _: i32, _: i32, _: bool, _: bool, _: bool) -> i32 {
        1
    }
    fn create_skill_missile(&mut self, _: UnitId, _: i32, _: i32, _: u16, _: bool, _: MissileAim) {}
    fn is_class_skill(&self, _: UnitId, skill: i32) -> bool {
        skill == 3
    }
    fn add_skill_level(&mut self, _: UnitId, _: i32, _: i32) {}
    fn after_skill_point(&mut self, _: UnitId) {}
    fn room(&self, _: UnitId) -> RoomKind {
        RoomKind::Field
    }
    fn refresh(&mut self, _: UnitId) {}
}

/// The action wiring's seams without a provider (positions, interaction,
/// warp, arrival mode, transport), as the bridge's local tests give them.
#[derive(Default)]
struct TestPending {
    pos: BTreeMap<UnitId, (i32, i32)>,
    interact: BTreeMap<UnitId, (u8, u32)>,
    sent: Vec<(UnitId, Vec<u8>)>,
}

impl Pending for TestPending {
    fn position(&self, unit: UnitId) -> (i32, i32) {
        self.pos.get(&unit).copied().unwrap_or_default()
    }
    fn place(&mut self, unit: UnitId, x: i32, y: i32) {
        self.pos.insert(unit, (x, y));
    }
    fn set_interact(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        self.interact.entry(player).or_insert((unit_type, guid));
    }
    fn reset_interact(&mut self, player: UnitId) {
        self.interact.remove(&player);
    }
    fn interact_guid(&self, player: UnitId) -> Option<u32> {
        self.interact.get(&player).map(|i| i.1)
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
    fn warp(&mut self, _: &mut Game, _: UnitId, _: u32, _: u8) {}
    fn set_player_mode_arrival(&mut self, _: &mut Game, _: UnitId) {}
}

impl Outbox for TestPending {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

/// `levels` (150 rows, acts by id; waypoints on levels 1 and 3, and 40 in
/// act 1) and one waypoint object class 0.
fn waypoint_data() -> WaypointData {
    let mut levels = vec![blank::<Levels>(); 150];
    for (i, l) in levels.iter_mut().enumerate() {
        l.waypoint = NO_WAYPOINT;
        l.act = if i >= 40 { 1 } else { 0 };
    }
    levels[1].waypoint = 0;
    levels[3].waypoint = 1;
    levels[40].waypoint = 9;
    let mut o: Objects = blank();
    o.operatefn = 23;
    o.initfn = 17;
    o.framecnt1 = 15 << 8;
    WaypointData::new(&levels, &[o])
}

const ALIVE: PlayerGate = PlayerGate {
    mode: 1,
    uninterruptable: false,
};

type ActionGame = SimGame<ActionSim<TestPending>, ActionWorld>;

/// The player (class `class`, mode NU, at (100, 100)) for client 0, a
/// monster at (120, 100), an item on the ground at (101, 100) and a
/// waypoint object at (102, 100).
fn action_host(class: u32) -> (ActionGame, Vec<u32>) {
    let book = Book::default();
    {
        let mut b = book.get();
        b.list = (0..4)
            .map(|skill| SkillEntry {
                skill,
                base: 1,
                owner_guid: -1,
                ..SkillEntry::default()
            })
            .collect();
        b.right = Some(b.list[1]);
        b.left = Some(b.list[0]);
    }
    let tables = ActionTables {
        missiles: Vec::new(),
        skills: skills(),
        combat: CombatTables {
            charstats: Vec::new(),
            difficultylevels: Vec::new(),
            monstats: Vec::new(),
            monstats2: Vec::new(),
            hitclass: Vec::new(),
        },
        levels: Vec::new(),
        skill_modes: Vec::new(),
    };
    let drlg = DrlgWorld {
        dungeon: Dungeon::default(),
        data: Arc::new(DrlgData::default()),
        tiles: Box::new(NoTiles),
        types: Box::new(NoLevelTypes),
    };
    let hooks = ActionHooks::new(
        Arc::new(tables),
        drlg,
        Seed::init_low(1234),
        TestPending::default(),
    );
    let data = UnitData {
        monsters: vec![MonsterInfo {
            enabled: true,
            aidel: [15; 3],
            moves: 0,
        }],
        ..UnitData::default()
    };
    let mut events = ActionSim::new(stat_data(), data, hooks);
    let mut game = Game::new();
    game.lists.ensure_act(0).unwrap();
    let mut alloc = |ty, class| {
        let req = AllocRequest {
            ty,
            class,
            room: None,
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        events
            .with(&mut game, |g, v| v.allocate(g, &req, 0, 0))
            .expect("allocated")
    };
    let player = alloc(UnitType::Player, class);
    let monster = alloc(UnitType::Monster, 0);
    let item = alloc(UnitType::Item, 0);
    let wp = alloc(UnitType::Object, 0);
    events.sys.units.get_mut(player).unwrap().mode = 1;
    // Stat points (4) and skill points (5) to spend, so 0x3A and 0x3B get
    // past their first check (`vitals.md` §2, `levels.md` §6.4).
    events.with(&mut game, |_, v| {
        v.set_base(player, 4, 5);
        v.set_base(player, 5, 5);
    });
    let mut sim: ActionGame = SimGame::with_events(game, events);
    sim.world.waypoints = Some(waypoint_data());
    sim.join(0, Some(player), None, client_state::IN_GAME)
        .unwrap();
    sim.set_player(
        player,
        PlayerFields {
            gate: ALIVE,
            data: Some(PlayerData { last_accept: 0 }),
        },
    );
    let at = |x, y| UnitFacts {
        act: 0,
        pos: Pos { x, y },
        owner: None,
    };
    sim.set_unit(player, at(100, 100));
    sim.set_unit(monster, at(120, 100));
    sim.set_unit(item, at(101, 100));
    sim.set_unit(wp, at(102, 100));
    for (u, x) in [(player, 100), (monster, 120), (item, 101), (wp, 102)] {
        sim.events.hooks().x.place(u, x, 100);
    }
    sim.skills = Some(Box::new(WiredSkills::new(vitals(), book)));
    let guids = [player, monster, item, wp]
        .iter()
        .map(|&u| sim.game.lists.unit(u).unwrap().guid)
        .collect();
    (sim, guids)
}

// ---- the item host ---------------------------------------------------------------------

const N_TYPES: usize = 80;
const T_RING: u16 = 10;
const T_BOX: u16 = 11;
const T_AMULET: u16 = 12;
const CUBE: usize = 0;
const RING: usize = 1;
const AMULET: usize = 2;

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
fn item_tables() -> ItemTables {
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
fn cube_data(t: &ItemTables) -> CubeData {
    let mut inputs = [InputSlot::default(); 7];
    inputs[0] = InputSlot {
        flags: input_flags::USEANY,
        item: RING as u16,
        ..InputSlot::default()
    };
    let out = OutputSlot {
        kind: cube_kind::ITEMCODE,
        item: AMULET as u16,
        quality: q::NORMAL,
        mods: [CraftMod {
            property: -1,
            ..CraftMod::default()
        }; 5],
        ..OutputSlot::default()
    };
    let recipe = Recipe {
        enabled: 1,
        class: 0xFF,
        numinputs: 1,
        inputs,
        outputs: [out, OutputSlot::default(), OutputSlot::default()],
        ..Recipe::default()
    };
    CubeData {
        recipes: vec![recipe],
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

/// [`ItemPending`] stand-in: placement appends to the list, removal drops
/// from it.
struct ItemRest;

impl ItemPending for ItemRest {
    fn inventory_pass(&mut self, _: UnitId, _: &mut Vec<Vec<u8>>) {}
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

fn new_item(w: &mut ItemWorld, game: &mut Game, class: usize, mode: u32) -> UnitId {
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
    w.economy(game).create_item(&mut rq, false, spawn).unwrap()
}

/// A player (class 2) for client 0 with the cube stored in its inventory
/// and a ring in mode `ring_mode`.
fn item_host(ring_mode: u32) -> (SimGame, Vec<u32>) {
    let tables = item_tables();
    let mut world = ItemWorld {
        units: Units::new(),
        stats: StatLists::new(stat_data()),
        data: UnitData {
            expansion: true,
            ..UnitData::default()
        },
        hooks: ItemHooks,
        fields: GameFields::new(Seed::init_low(0x5EED), true),
        cube: cube_data(&tables),
        tables,
        items: ItemStore::new(),
        staged: Staged {
            local_date: (15, 3),
            ..Staged::default()
        },
        creation: BTreeMap::new(),
        pending: Box::new(ItemRest),
        errors: Vec::new(),
    };
    let mut game = Game::new();
    let req = AllocRequest {
        ty: UnitType::Player,
        class: 2,
        room: None,
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let player = {
        let mut sim = Sim {
            game: &mut game,
            units: &mut world.units,
            stats: &mut world.stats,
            data: &world.data,
        };
        allocate(&mut sim, &mut world.hooks, &mut world.fields.seed, &req)
            .unwrap()
            .unwrap()
    };
    let cube = new_item(&mut world, &mut game, CUBE, 0);
    let ring = new_item(&mut world, &mut game, RING, ring_mode);
    world.items.get_mut(cube).unwrap().inv_page = 0;
    world.items.get_mut(ring).unwrap().inv_page = 0;
    world.staged.inventories.insert(
        player,
        Inventory {
            items: vec![cube],
            cursor: None,
        },
    );
    let mut sim = SimGame::new(game);
    sim.join(0, Some(player), None, client_state::IN_GAME)
        .unwrap();
    sim.set_player(
        player,
        PlayerFields {
            gate: ALIVE,
            data: Some(PlayerData { last_accept: 0 }),
        },
    );
    let at = |x, y| UnitFacts {
        act: 0,
        pos: Pos { x, y },
        owner: None,
    };
    sim.set_unit(player, at(100, 100));
    sim.set_unit(ring, at(100, 100));
    let guids = [player, cube, ring]
        .iter()
        .map(|&u| world.units.get(u).unwrap().guid)
        .collect();
    sim.items = Some(world);
    (sim, guids)
}

// ---- messages --------------------------------------------------------------------------

/// One generated C→S game message, built against the host's GUIDs.
#[derive(Clone, Debug)]
struct Gen {
    id: u8,
    /// Per layout field: a pool index and a random value.
    picks: Vec<(u8, u32)>,
    raw: Vec<u8>,
    /// 0: exact size; 1: one byte short; 2: one byte long.
    len: u8,
}

fn gen() -> impl Strategy<Value = Gen> {
    (
        1u8..0x67,
        prop::collection::vec((any::<u8>(), any::<u32>()), 8),
        prop::collection::vec(any::<u8>(), 0..300),
        prop_oneof![8 => Just(0u8), 1 => Just(1u8), 1 => Just(2u8)],
    )
        .prop_map(|(id, picks, raw, len)| Gen {
            id,
            picks,
            raw,
            len,
        })
}

/// Field values: from the field's domain (by layout name), or the edges
/// of every field's range, unit types 0..=7,
/// skill and stat ids around the tables' counts, coordinates near and
/// far from the player at (100, 100), item positions, the host's GUIDs.
fn pick(name: &str, guids: &[u32], (sel, rnd): (u8, u32)) -> u32 {
    // Half the time, a value from the field's own domain, so the
    // handlers get past the dispatcher's parse and their first checks.
    if sel & 0x80 != 0 {
        // A host GUID, or -1 ("no unit", e.g. 0x3C's item).
        let guid = match rnd as usize % (guids.len() + 1) {
            i if i < guids.len() => guids[i],
            _ => u32::MAX,
        };
        return match name {
            "x" | "y" => 100 + rnd % 101 - 50,
            "type" => rnd % 6,
            "id" | "item" | "cube" | "unit" | "npc" | "wp" | "target" | "merc" | "book"
            | "scroll" | "socketable" | "cursor" | "player" => guid,
            "skill" => rnd % 5,
            "cost" => rnd % 6000,
            "action" | "msg" | "quest" => rnd % 8,
            "stat" => rnd % 8,
            "left" => rnd % 2,
            "button" | "page" | "bodyloc" | "slot" | "level" | "tab" => rnd % 20,
            _ => rnd % 4,
        };
    }
    const POOL: &[u32] = &[
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        15,
        16,
        0xFF,
        0xFFFF,
        0x7FFF,
        0x8000,
        0x7FFF_FFFF,
        0x8000_0000,
        u32::MAX,
        u32::MAX - 1,
        49,
        50,
        51,
        100,
        101,
        120,
        150,
        151,
        358,
        359,
        360,
    ];
    let n = POOL.len() + guids.len() + 1;
    match sel as usize % n {
        i if i < POOL.len() => POOL[i],
        i if i < POOL.len() + guids.len() => guids[i - POOL.len()],
        _ => rnd,
    }
}

/// The message bytes for `g`: its id's fixed size (0x14: 4..=275; 0x15:
/// its raw bytes) with each layout field set from the pool.
fn build(g: &Gen, guids: &[u32]) -> Vec<u8> {
    let row = &CLIENT_MESSAGES[g.id as usize];
    let mut m = g.raw.clone();
    match row.transport_size.fixed() {
        Some(n) => m.resize(n, 0),
        None if g.id == 0x14 => m.resize(m.len().clamp(4, 275), 0),
        None => m.truncate(0x200 - 1),
    }
    if m.is_empty() {
        m.push(0);
    }
    m[0] = g.id;
    for (f, &p) in row.layout.iter().zip(&g.picks) {
        let Some(off) = f.offset else { continue };
        let off = off as usize;
        let v = pick(f.name, guids, p);
        let (w, mask) = match f.ty {
            FieldType::U8 => (1, 0xFF),
            FieldType::U16 => (2, 0xFFFF),
            FieldType::U32 => (4, u32::MAX),
            FieldType::Bits(n) => (4, (1u32 << n) - 1),
            FieldType::Bit(n) => (4, 1u32 << n),
            _ => continue,
        };
        if off + w > m.len() {
            continue;
        }
        let mut word = [0u8; 4];
        word[..w].copy_from_slice(&m[off..off + w]);
        let old = u32::from_le_bytes(word);
        let v = match f.ty {
            FieldType::Bit(n) => (v & 1) << n,
            _ => v,
        };
        let new = (old & !mask) | (v & mask);
        m[off..off + w].copy_from_slice(&new.to_le_bytes()[..w]);
    }
    match g.len {
        1 if m.len() > 1 => {
            m.pop();
        }
        2 => m.push(0),
        _ => {}
    }
    m
}

/// The game state a rejected intent must leave alone, and the fatal
/// paths (1.14d asserts) the handlers record instead of panicking.
trait Snapshot {
    fn snapshot(&self) -> String;
    fn faults(&self) -> Vec<String>;
}

impl Snapshot for ActionGame {
    fn snapshot(&self) -> String {
        let s = &self.events.sys;
        format!("{:?}{:?}{:?}", self.game, s.units, s.stats)
    }
    fn faults(&self) -> Vec<String> {
        let mut v: Vec<String> = self.world.faults.iter().map(|f| format!("{f:?}")).collect();
        v.extend(self.events.sys.errors.iter().map(|e| format!("{e:?}")));
        v
    }
}

impl Snapshot for SimGame {
    fn snapshot(&self) -> String {
        let w = self.items.as_ref().unwrap();
        format!(
            "{:?}{:?}{:?}{:?}{:?}",
            self.game, w.units, w.stats, w.items, w.staged
        )
    }
    fn faults(&self) -> Vec<String> {
        let w = self.items.as_ref().unwrap();
        w.errors.iter().map(|e| format!("{e:?}")).collect()
    }
}

/// Runs `msgs` one per frame on `sim`: dispatch, then one tick. Every
/// dispatch returns; a stub returns 0 (§2.4 rule 2); a wrong size is 3,
/// a unit type ≥ 6 is 2 and a point target out of range is 1 (§2.4
/// rules 1, 3, 4); the dispatcher's rejections leave the state as it
/// was; no handler or tick meets a fatal path (a 1.14d assert).
fn run<D, W>(sim: &mut SimGame<D, W>, guids: &[u32], msgs: &[Gen]) -> Result<(), TestCaseError>
where
    D: EventDispatch + d2_sim::tick::TickHooks,
    W: d2_server::adapters::handlers::world::WorldHost<D>,
    SimGame<D, W>: Snapshot,
{
    let mut out = ClientBuffers::new();
    out.add_client(0);
    for g in msgs {
        let m = build(g, guids);
        let (id, size) = (m[0], m.len());
        let before = sim.snapshot();
        let stubs = sim.unhandled.len();
        let code = dispatch(sim, &ProtoSizes, &mut out, 0, ALIVE, &m, size);
        if sim.unhandled.len() > stubs {
            prop_assert_eq!(code, ResultCode::Done, "{:02X?}", m);
            prop_assert_eq!(kind(id), Kind::Handler);
        }
        let row = &CLIENT_MESSAGES[id as usize];
        // The player is alive: only the dead gate (0x41) is closed, and
        // a closed gate returns 0 before the size check (§2.3 rule 3).
        let open = gate(id) != Gate::Dead;
        if !open {
            prop_assert_eq!(code, ResultCode::Done, "{:02X?}", m);
        }
        // The dispatcher's own rejections, before any handler (§2.3 rule
        // 2, §2.4 rules 1–4): they must change nothing. (A handler's
        // refusal may follow state changes its spec orders first, e.g.
        // `cube.md` §2 step 3.1's targeting reset before a 3.)
        let mut rejected = open && kind(id) != Kind::Handler && code == ResultCode::Malformed;
        let handler = open && kind(id) == Kind::Handler;
        if handler && row.transport_size.fixed().is_some_and(|n| n != size) {
            prop_assert_eq!(code, ResultCode::Malformed, "{:02X?}", m);
            rejected = true;
        } else if handler && is_unit(id) {
            if u32::from_le_bytes([m[1], m[2], m[3], m[4]]) >= 6 {
                prop_assert_eq!(code, ResultCode::Invalid, "{:02X?}", m);
                rejected = true;
            }
        } else if handler && is_point(id) {
            // The player is staged at (100, 100) and never moved.
            let at = |o: usize| i32::from(u16::from_le_bytes([m[o], m[o + 1]]));
            if (at(1) - 100).abs() > 50 || (at(3) - 100).abs() > 50 {
                prop_assert_eq!(code, ResultCode::Refused, "{:02X?}", m);
                rejected = true;
            }
        }
        if rejected {
            prop_assert!(
                sim.snapshot() == before,
                "{:02X?} was rejected and changed the state",
                m
            );
        }
        sim.tick(&mut out);
        let faults = sim.faults();
        prop_assert!(faults.is_empty(), "{:02X?}: {:?}", m, faults);
    }
    Ok(())
}

proptest! {
    #![proptest_config(config(48))]

    /// Every game id with arbitrary fields on the action host.
    #[test]
    fn action_host_any_intents(class in 0u32..7, msgs in prop::collection::vec(gen(), 1..16)) {
        let (mut sim, guids) = action_host(class);
        run(&mut sim, &guids, &msgs)?;
    }

    /// Every game id with arbitrary fields on the item host.
    #[test]
    fn item_host_any_intents(ring_mode in 0u32..6, msgs in prop::collection::vec(gen(), 1..16)) {
        let (mut sim, guids) = item_host(ring_mode);
        run(&mut sim, &guids, &msgs)?;
    }
}

/// Each handled id alone, many times, on fresh hosts: the ids the
/// handler modules own (skills 0x05–0x11, 0x3A–0x3C; waypoints 0x49;
/// cube 0x2A, 0x4F) get most of the cases.
fn owned_id() -> impl Strategy<Value = u8> {
    prop::sample::select(vec![
        0x05u8, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x2A, 0x3A,
        0x3B, 0x3C, 0x49, 0x4F,
    ])
}

proptest! {
    #![proptest_config(config(96))]

    #[test]
    fn handled_ids_any_fields(
        ids in prop::collection::vec(owned_id(), 1..8),
        gens in prop::collection::vec(gen(), 8),
        class in 0u32..7,
    ) {
        let msgs: Vec<Gen> = ids.iter().zip(gens).map(|(&id, g)| Gen { id, ..g }).collect();
        let (mut sim, guids) = action_host(class);
        run(&mut sim, &guids, &msgs)?;
        let (mut sim, guids) = item_host(class % 6);
        run(&mut sim, &guids, &msgs)?;
    }
}

/// The fixture is live: the hosts' handlers answer (a known message
/// reaches each handler module), so the properties exercise them.
#[test]
fn hosts_reach_the_handlers() {
    let (mut sim, guids) = action_host(1);
    let mut out = ClientBuffers::new();
    out.add_client(0);
    // 0x3C select skill 1 on the right hand: the skill handler.
    let mut m = vec![0x3C, 1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
    let code = dispatch(&mut sim, &ProtoSizes, &mut out, 0, ALIVE, &m, 9);
    assert_eq!(code, ResultCode::Done);
    assert!(sim.unhandled.is_empty(), "{:?}", sim.unhandled);
    // 0x49 at the waypoint object: the waypoint handler.
    m = vec![0x49];
    m.extend_from_slice(&guids[3].to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    dispatch(&mut sim, &ProtoSizes, &mut out, 0, ALIVE, &m, 9);
    assert!(sim.unhandled.is_empty(), "{:?}", sim.unhandled);
    let (mut sim, guids) = item_host(0);
    // 0x2A with the ring and the cube: the cube handler.
    m = vec![0x2A];
    m.extend_from_slice(&guids[2].to_le_bytes());
    m.extend_from_slice(&guids[1].to_le_bytes());
    dispatch(&mut sim, &ProtoSizes, &mut out, 0, ALIVE, &m, 9);
    assert!(sim.unhandled.is_empty(), "{:?}", sim.unhandled);
}

// ---- the trade host --------------------------------------------------------------------
//
// `SimGame<ActionSim<_>, TradeWorld<_>>` as `d2-client`'s
// `e2e_vendor.rs` builds it: the NPC, vendor and quest handlers on
// `wiring::interaction` over the action sim's own units. Akara (class
// 148) next to the player; the player owns a buckler and a cap and
// carries 5000 gold.

mod trade {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use d2_data::tables::{Itemratio, Itemtypes, Monstats};
    use d2_server::adapters::handlers::world::{ActionWorld, Outbox, TradeWorld};
    use d2_server::adapters::{PlayerData, PlayerFields, SimGame, UnitFacts};
    use d2_server::seams::Pos;
    use d2_sim::combat::CombatTables;
    use d2_sim::drlg::data::DrlgData;
    use d2_sim::drlg::{Dungeon, NoLevelTypes};
    use d2_sim::game::Game;
    use d2_sim::items::tables::ItemRec;
    use d2_sim::items::{ty, ItemRequest, ItemTables};
    use d2_sim::rng::Seed;
    use d2_sim::skills::SkillTables;
    use d2_sim::units::hooks::{MonsterInfo, UnitData};
    use d2_sim::units::lifecycle::AllocRequest;
    use d2_sim::units::lists::client_state;
    use d2_sim::units::{UnitId, UnitType};
    use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables, DrlgWorld, Pending};
    use d2_sim::wiring::economy::{GameFields, ItemSpawn, QuestRest};
    use d2_sim::wiring::interaction::{NpcRest, PlayerQuestsRef, VendorRest};
    use d2_sim::world::npc::{self, class, ImbueMods, InvEntry, ItemFacts, MercInit, NpcControl};
    use d2_sim::world::quests::{
        PlayerQuests, QuestChain, QuestControl, QuestTables, TextList, UnitKind,
    };
    use d2_sim::world::vendors::price::Bonus;
    use d2_sim::world::vendors::{
        NpcPrices, Transaction, TypeRec, VendorItem, VendorTables, NO_CODE, XXX,
    };

    use super::{blank, stat_data, NoTiles, Snapshot, ALIVE, N_STATS};

    const N_TYPES: usize = 80;
    const N_MONSTATS: usize = 400;
    const CAP: usize = 0;
    const BUC: usize = 1;

    /// The action wiring's seams: `Pending`'s defaults; sends kept.
    #[derive(Default)]
    pub struct ActionRest {
        sent: Vec<(UnitId, Vec<u8>)>,
    }

    impl Pending for ActionRest {
        fn send(&mut self, player: UnitId, msg: &[u8]) {
            self.sent.push((player, msg.to_vec()));
        }
    }

    impl Outbox for ActionRest {
        fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
            std::mem::take(&mut self.sent)
        }
    }

    /// The interaction seams no written spec provides, answered as in
    /// `e2e_vendor.rs` (talk range, the staged inventory, room in the
    /// NPC grid, no item copy).
    #[derive(Default)]
    pub struct Rest {
        interact: BTreeMap<UnitId, (u8, u32)>,
        quests: BTreeMap<UnitId, PlayerQuests>,
        inventory: BTreeSet<UnitId>,
        last_bought: BTreeMap<UnitId, u32>,
        sent: Vec<(UnitId, Vec<u8>)>,
    }

    impl Outbox for Rest {
        fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
            std::mem::take(&mut self.sent)
        }
    }

    impl PlayerQuestsRef for Rest {
        fn quests_ref(&self, player: UnitId) -> Option<&PlayerQuests> {
            self.quests.get(&player)
        }
    }

    impl NpcRest for Rest {
        fn item_format(&self) -> u16 {
            1
        }
        fn distance(&self, _: UnitId, _: UnitId) -> i32 {
            3
        }
        fn axis_check(&self, _: UnitId, _: UnitId) -> u32 {
            0
        }
        fn unit_check(&self, _: UnitId, _: u32) -> u32 {
            0
        }
        fn clear_path(&mut self, _: UnitId) {}
        fn approach(&mut self, _: UnitId, _: UnitId) {}
        fn player_busy(&self, _: UnitId) -> u32 {
            0
        }
        fn start_allowed(&self, _: UnitId, _: UnitId) -> bool {
            true
        }
        fn tristram_cain_busy(&self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn interact_unit(&self, player: UnitId) -> Option<(u8, u32)> {
            self.interact.get(&player).copied()
        }
        fn set_interact(&mut self, player: UnitId, t: u8, guid: u32) {
            self.interact.insert(player, (t, guid));
        }
        fn reset_interact(&mut self, player: UnitId) {
            self.interact.remove(&player);
        }
        fn pet(&self, _: UnitId, _: u8, _: u8) -> Option<UnitId> {
            None
        }
        fn pets(&self, _: UnitId) -> Vec<UnitId> {
            Vec::new()
        }
        fn player_name(&self, _: UnitId) -> Vec<u8> {
            b"tester".to_vec()
        }
        fn reset_stats(&mut self, _: UnitId) {}
        fn reset_skills(&mut self, _: UnitId) {}
        fn act_change(&mut self, _: UnitId, _: u32, _: u32) {}
        fn activate_waypoint(&mut self, _: UnitId, _: u32) {}
        fn npc_ai_param(&mut self, _: UnitId, _: u32) {}
        fn stat_sent(&mut self, _: UnitId, _: u16, _: u32) {}
        fn respec_sound(&mut self, _: UnitId) {}
        fn encode_text_list(&self, _: &TextList) -> [u8; 34] {
            [0; 34]
        }
        fn socket_granted(&mut self, _: UnitId) {}
        fn personalize_granted(&mut self, _: UnitId) {}
        fn inventory_entries(&self, _: UnitId) -> Vec<InvEntry> {
            Vec::new()
        }
        fn identify(&mut self, _: UnitId) {}
        fn cursor_item(&self, _: UnitId) -> Option<UnitId> {
            None
        }
        fn item_facts(&self, _: UnitId) -> ItemFacts {
            ItemFacts::default()
        }
        fn put_back(&mut self, _: UnitId, _: UnitId) {}
        fn remove_cursor_item(&mut self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn duplicate(&mut self, _: UnitId, _: UnitId) -> Option<UnitId> {
            None
        }
        fn create_imbued(&mut self, _: UnitId, _: UnitId, _: &ImbueMods) -> Option<UnitId> {
            None
        }
        fn item_refresh(&mut self, _: UnitId) {}
        fn personal_name(&self, _: UnitId) -> Vec<u8> {
            Vec::new()
        }
        fn set_personal_name(&mut self, _: UnitId, _: &[u8]) {}
        fn place_or_drop(&mut self, _: UnitId, _: UnitId) {}
        fn set_mode(&mut self, _: UnitId, _: u8) {}
        fn spawn_mercenary(&mut self, _: UnitId, _: u32, _: u8) -> Option<UnitId> {
            None
        }
        fn init_mercenary(&mut self, _: UnitId, _: UnitId, _: &MercInit) {}
        fn revive_mercenary(&mut self, _: UnitId, _: UnitId) {}
    }

    impl VendorRest for Rest {
        fn players_in_level(&self, _: u16) -> i32 {
            1
        }
        fn player_level_id(&self, _: UnitId) -> u16 {
            1
        }
        fn gold_cap(&self, _: UnitId) -> i32 {
            100_000
        }
        fn stash_cap(&self, _: UnitId) -> i32 {
            100_000
        }
        fn drop_gold(&mut self, _: UnitId, _: i32) {}
        fn last_bought(&self, p: UnitId) -> u32 {
            self.last_bought.get(&p).copied().unwrap_or(u32::MAX)
        }
        fn set_last_bought(&mut self, p: UnitId, guid: u32) {
            self.last_bought.insert(p, guid);
        }
        fn has_cursor_item(&self, _: UnitId) -> bool {
            false
        }
        fn copy_item(&mut self, _: UnitId) -> Option<UnitId> {
            None
        }
        fn has_filled_sockets(&self, _: UnitId) -> bool {
            false
        }
        fn socketed(&self, _: UnitId) -> Vec<UnitId> {
            Vec::new()
        }
        fn price_bonuses(&self, _: UnitId) -> Vec<Bonus> {
            Vec::new()
        }
        fn recharge(&mut self, _: UnitId) {}
        fn repair_broken(&mut self, _: UnitId) {}
        fn send_item_stat(&mut self, _: UnitId, _: UnitId, _: u16) {}
        fn send_transaction(&mut self, p: UnitId, t: Transaction) {
            let m = npc::transaction(t.kind, t.code, t.guid, t.gold as u32);
            self.sent.push((p, m.to_vec()));
        }
        fn new_store_inventory(&mut self, _: u16, _: Option<UnitId>) {}
        fn place_in_store(&mut self, _: u16, _: UnitId) -> bool {
            true
        }
        fn remove_store_item(&mut self, _: u16, _: UnitId) {}
        fn take_from_store(&mut self, _: u16, _: UnitId) {}
        fn place_in_gamble(&mut self, _: u16, _: u32, _: UnitId) -> bool {
            true
        }
        fn remove_gamble_item(&mut self, _: u16, _: u32, _: UnitId) {}
        fn refresh_npc_inventory(&mut self, _: UnitId) {}
        fn add_trade_inventory(&mut self, _: u16, _: UnitId) {}
        fn owns_item(&self, _: UnitId, item: UnitId) -> bool {
            self.inventory.contains(&item)
        }
        fn in_inventory(&self, _: UnitId, item: UnitId) -> bool {
            self.inventory.contains(&item)
        }
        fn equipped_items(&self, _: UnitId) -> Vec<UnitId> {
            Vec::new()
        }
        fn find_tome(&self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
            None
        }
        fn add_to_tome(&mut self, _: UnitId, _: i32) {}
        fn find_partial_stack(&self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
            None
        }
        fn can_belt(&self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn put_in_belt(&mut self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn equip_ammo(&mut self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn place_in_backpack(&mut self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn take_from_cursor(&mut self, _: UnitId, _: UnitId) -> bool {
            false
        }
        fn lower_book_skill(&mut self, _: UnitId, _: UnitId, _: i32) {}
        fn remove_stored(&mut self, _: UnitId, item: UnitId) {
            self.inventory.remove(&item);
        }
        fn unequip(&mut self, _: UnitId, _: UnitId) -> bool {
            false
        }
    }

    impl QuestRest for Rest {
        fn has_act2(&self) -> bool {
            false
        }
        fn players(&self) -> Vec<UnitId> {
            self.quests.keys().copied().collect()
        }
        fn first_client_player(&self) -> Option<UnitId> {
            self.quests.keys().next().copied()
        }
        fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests> {
            self.quests.get_mut(&player)
        }
        fn player_byte_4c(&self, _: UnitId) -> u8 {
            0
        }
        fn set_player_byte_4c(&mut self, _: UnitId, _: u8) {}
        fn quest_chain(&mut self, _: UnitId) -> Option<&mut QuestChain> {
            None
        }
        fn unit_act(&self, _: UnitId) -> Option<u8> {
            Some(0)
        }
        fn unit_level(&self, _: UnitId) -> Option<u32> {
            Some(1)
        }
        fn unit_kind(&self, _: UnitId) -> UnitKind {
            UnitKind::Other
        }
        fn players_near(&self, _: UnitId) -> Vec<UnitId> {
            Vec::new()
        }
        fn attach_sound(&mut self, _: UnitId, _: u16) {}
        fn send(&mut self, player: UnitId, msg: &[u8]) {
            self.sent.push((player, msg.to_vec()));
        }
        fn send_text_list(&mut self, _: UnitId, _: UnitId, _: &[(u16, u32)]) {}
        fn inventory(&self, _: UnitId) -> Vec<UnitId> {
            Vec::new()
        }
        fn delete_item(&mut self, _: UnitId, _: [u8; 4]) {}
        fn reward_item(&mut self, _: UnitId, _: [u8; 4], _: i32, _: u8, _: bool) -> Option<UnitId> {
            None
        }
        fn drop_item_at(&mut self, _: UnitId, _: [u8; 4], _: u8) -> bool {
            false
        }
        fn den_region(&self) -> (u32, u32, u32, u32) {
            (0, 0, 0, 0)
        }
        fn true_tomb_level(&self) -> u32 {
            0
        }
        fn free_spot(&mut self, _: UnitId, _: u32, _: u32, _: u32, _: u32) -> Option<(i32, i32)> {
            None
        }
        fn create_portal(&mut self, _: UnitId, _: i32, _: i32, _: u16, _: u32) -> bool {
            false
        }
        fn schedule_quest_event(&mut self, _: UnitId, _: i32) {}
        fn set_object_opened(&mut self, _: UnitId) {}
        fn mercenary_reward(&mut self, _: UnitId, _: u16) {}
        fn unhandled(&mut self, _: u8, _: u32) {}
    }

    /// Every type is its own and type 0's; helm and shield are armor.
    fn equiv() -> d2_data::fixup::maps::EquivMatrix {
        let n = N_TYPES;
        let words = n.div_ceil(32);
        let mut m = d2_data::fixup::maps::EquivMatrix {
            n,
            words,
            bits: vec![0; n * words],
        };
        let mut set = |i: usize, j: usize| m.bits[i * words + j / 32] |= 1 << (j % 32);
        for i in 1..n {
            set(i, 0);
            set(i, i);
        }
        set(usize::from(ty::HELM), usize::from(ty::ARMO));
        set(usize::from(ty::SHIE), usize::from(ty::ARMO));
        m
    }

    /// A cap (helm) and a buckler (shield), durability 12.
    fn item_tables() -> ItemTables {
        let mut ratio: Itemratio = blank();
        ratio.version = 0;
        let rec = |t: u16, code: &[u8; 4], (minac, maxac): (u32, u32)| ItemRec {
            code: *code,
            type_: t as i16,
            level: 1,
            durability: 12,
            minac,
            maxac,
            ..ItemRec::default()
        };
        ItemTables {
            items: vec![
                rec(ty::HELM, b"cap ", (3, 5)),
                rec(ty::SHIE, b"buc ", (4, 6)),
            ],
            itemtypes: (0..N_TYPES)
                .map(|_| {
                    let mut t: Itemtypes = blank();
                    t.class = 0xFF;
                    t.staffmods = 0xFF;
                    t.rare = 1;
                    t
                })
                .collect(),
            equiv: equiv(),
            itemratio: vec![ratio],
            valshift: vec![0; N_STATS],
            stat_shift: 6,
            stat_mask: 0x3F,
            ..ItemTables::default()
        }
    }

    /// Akara's column holds the cap (permanent) and the buckler (1–3).
    fn vendor_tables() -> VendorTables {
        let item = |code: &[u8; 4], t: u16, cost: u32, (minac, maxac): (u32, u32)| VendorItem {
            code: *code,
            normcode: *code,
            ubercode: NO_CODE,
            ultracode: NO_CODE,
            cost,
            type_: t as i16,
            type2: -1,
            level: 1,
            spawnable: 1,
            durability: 12,
            minac,
            maxac,
            nightmare_upgrade: XXX,
            hell_upgrade: XXX,
            ..VendorItem::default()
        };
        let mut cap = item(b"cap ", ty::HELM, 100, (3, 5));
        cap.perm_store = 1;
        cap.columns[0] = [1, 1, 0, 0, 0];
        let mut buc = item(b"buc ", ty::SHIE, 80, (4, 6));
        buc.columns[0] = [1, 3, 0, 0, 0];
        let mut itemtypes = vec![
            TypeRec {
                repair: 1,
                class: 7,
                storepage: 3,
                staffmods: 0xFF,
                ..TypeRec::default()
            };
            N_TYPES
        ];
        itemtypes[usize::from(ty::HELM)].storepage = 0;
        itemtypes[usize::from(ty::SHIE)].storepage = 0;
        VendorTables {
            items: vec![cap, buc],
            itemtypes,
            equiv: equiv(),
            stat_shift: 6,
            stat_mask: 0x3F,
            monster_levels: vec![[1, 1, 1]; N_MONSTATS],
            interact: vec![class::AKARA],
            npc: vec![NpcPrices {
                class: u32::from(class::AKARA),
                sell: 1024,
                buy: 512,
                rep: 128,
                quests: [(0, 0, 0, 0); 3],
                max_buy: [5000; 3],
            }],
            difficulty: vec![Default::default(); 3],
            ..VendorTables::default()
        }
    }

    fn monstats() -> Vec<Monstats> {
        let mut v: Vec<Monstats> = (0..N_MONSTATS).map(|_| blank()).collect();
        v[usize::from(class::AKARA)].npc = true;
        v[usize::from(class::AKARA)].interact = true;
        v
    }

    pub type TradeGame = SimGame<ActionSim<ActionRest>, TradeWorld<Rest>>;

    impl Snapshot for TradeGame {
        fn snapshot(&self) -> String {
            let s = &self.events.sys;
            format!(
                "{:?}{:?}{:?}{:?}",
                self.game, s.units, s.stats, self.world.items
            )
        }
        fn faults(&self) -> Vec<String> {
            let s = &self.events.sys;
            let mut e: Vec<String> = s.hooks.errors.iter().map(|e| format!("{e:?}")).collect();
            e.extend(s.errors.iter().map(|e| format!("{e:?}")));
            e.extend(self.world.state.errors.iter().map(|e| format!("{e:?}")));
            e.extend(self.world.action.faults.iter().map(|f| format!("{f:?}")));
            e
        }
    }

    /// The host and its GUIDs: player, Akara, buckler, cap.
    pub fn trade_host(game_seed: u32) -> (TradeGame, Vec<u32>) {
        let tables = ActionTables {
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
        };
        let drlg = DrlgWorld {
            dungeon: Dungeon::default(),
            data: Arc::new(DrlgData::default()),
            tiles: Box::new(NoTiles),
            types: Box::new(NoLevelTypes),
        };
        let hooks = ActionHooks::new(
            Arc::new(tables),
            drlg,
            Seed::init_low(game_seed),
            ActionRest::default(),
        );
        let data = UnitData {
            monsters: vec![
                MonsterInfo {
                    enabled: true,
                    aidel: [15; 3],
                    moves: 0,
                };
                N_MONSTATS
            ],
            ..UnitData::default()
        };
        let mut events = ActionSim::new(stat_data(), data, hooks);
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let mut seed = events.hooks().game_seed;
        let ctl = NpcControl::new(&monstats(), Vec::new(), false, 0, &mut seed).expect("npc");
        let quests = QuestControl::new(&QuestTables::load().unwrap(), &mut seed).unwrap();
        events.hooks().game_seed = seed;
        let mut alloc = |ty, class| {
            let req = AllocRequest {
                ty,
                class,
                room: None,
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: ty == UnitType::Player,
            };
            events
                .with(&mut game, |g, v| v.allocate(g, &req, 0, 0))
                .expect("allocated")
        };
        let npc = alloc(UnitType::Monster, u32::from(class::AKARA));
        let player = alloc(UnitType::Player, 1);
        events.sys.units.get_mut(player).unwrap().mode = 1;
        events.with(&mut game, |_, v| {
            v.set_base(player, 12, 1);
            v.set_base(player, 14, 5000);
        });
        let mut rest = Rest::default();
        rest.quests.insert(player, PlayerQuests::default());
        let mut world = TradeWorld::new(
            ActionWorld::default(),
            GameFields::new(Seed::init_low(game_seed), false),
            item_tables(),
            quests,
            ctl,
            vendor_tables(),
            rest,
            1000,
        );
        world.state.add_npc(npc);
        let (buckler, cap) = world.with_economy(&mut game, &mut events, |econ, _| {
            let mut make = |record: usize| {
                let mut rq = ItemRequest {
                    item: record as i32,
                    ilvl: 1,
                    quality: 2,
                    format: 1,
                    ..ItemRequest::default()
                };
                let spawn = ItemSpawn {
                    room: None,
                    mode: 0,
                    init_flags: 1,
                };
                econ.create_item(&mut rq, false, spawn).expect("item")
            };
            (make(BUC), make(CAP))
        });
        world.rest.inventory.extend([buckler, cap]);
        let mut s: TradeGame = SimGame::with_world(game, events, world);
        s.join(0, Some(player), None, client_state::IN_GAME)
            .unwrap();
        s.set_player(
            player,
            PlayerFields {
                gate: ALIVE,
                data: Some(PlayerData { last_accept: 0 }),
            },
        );
        let at = UnitFacts {
            act: 0,
            pos: Pos { x: 100, y: 100 },
            owner: None,
        };
        s.set_unit(player, at);
        s.set_unit(npc, at);
        let guids = [player, npc, buckler, cap]
            .iter()
            .map(|&u| s.game.lists.unit(u).unwrap().guid)
            .collect();
        (s, guids)
    }
}

/// The NPC, vendor and quest ids `TradeWorld` handles.
fn trade_id() -> impl Strategy<Value = u8> {
    prop::sample::select(vec![
        0x13u8, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x40, 0x58, 0x62,
    ])
}

/// Talk (0x13), chat (0x2F) and trade (0x38 action 1) with Akara, so the
/// vendor paths past "no interaction" are reached.
fn open_trade(npc: u32) -> Vec<Vec<u8>> {
    let mut talk = vec![0x13, 1, 0, 0, 0];
    talk.extend_from_slice(&npc.to_le_bytes());
    let mut chat = vec![0x2F, 1, 0, 0, 0];
    chat.extend_from_slice(&npc.to_le_bytes());
    let mut trade = vec![0x38, 1, 0, 0, 0];
    trade.extend_from_slice(&npc.to_le_bytes());
    trade.extend_from_slice(&[0; 4]);
    vec![talk, chat, trade]
}

proptest! {
    #![proptest_config(config(64))]

    /// Every game id on the trade host, optionally after opening a trade
    /// with Akara; the NPC / vendor / quest ids get most of the cases.
    #[test]
    fn trade_host_any_intents(
        seed in any::<u32>(),
        open in any::<bool>(),
        ids in prop::collection::vec(prop_oneof![3 => trade_id(), 1 => 1u8..0x67], 1..12),
        gens in prop::collection::vec(gen(), 12),
    ) {
        let (mut sim, mut guids) = trade::trade_host(seed);
        if open {
            let mut out = ClientBuffers::new();
            out.add_client(0);
            for m in open_trade(guids[1]) {
                let code = dispatch(&mut sim, &ProtoSizes, &mut out, 0, ALIVE, &m, m.len());
                prop_assert_eq!(code, ResultCode::Done, "{:02X?}", m);
            }
            // The store items the trade generated join the GUID pool.
            let mut items = sim.game.lists.units_of_type(d2_sim::units::UnitType::Item);
            items.sort();
            for u in items {
                let g = sim.game.lists.unit(u).unwrap().guid;
                if !guids.contains(&g) {
                    guids.push(g);
                }
            }
        }
        let msgs: Vec<Gen> = ids.iter().zip(gens).map(|(&id, g)| Gen { id, ..g }).collect();
        run(&mut sim, &guids, &msgs)?;
    }
}
