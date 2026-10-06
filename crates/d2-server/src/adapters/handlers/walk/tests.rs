// Spec: specs/sim/pathing.md §1.1, §10; specs/sim/path-placement.md §10, §11; specs/world/waypoints.md §7
//! C→S 0x01–0x04 through `SimGame::handle` and the game tick on the
//! wired sim (`ActionSim` with the path provider on, a synthetic act 0:
//! Cold Plains as rooms A and B side by side, the town (level 1) as room
//! C). Only `Pending` (interaction, the transport) is a test provider.
//! Positions are the path records' (`path-placement.md` §2.3); the
//! message bytes are built with `d2-proto`'s typed builders.

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::bin::BinTable;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Charstats, Itemstatcost, Levels, Objects, Record};
use d2_proto::server::{MapReveal, PlayerMove, PlayerStop, PlayerToTarget, ReassignPlayer};
use d2_sim::combat::CombatTables;
use d2_sim::drlg::room::LinkAt;
use d2_sim::drlg::{
    CellGrid, Drlg, DrlgData, DrlgError, DrlgRoomId, Dungeon, GridPass, LevelDef, LevelIdx,
    LevelTypes, RoomGrids, RoomKind, TileInfo, TileRect, TileSource,
};
use d2_sim::game::Game;
use d2_sim::path::{CollisionRooms, DynamicPath};
use d2_sim::rng::Seed;
use d2_sim::skills::SkillTables;
use d2_sim::stats::{StatData, StatTable};
use d2_sim::units::hooks::UnitData;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{RoomId, UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionSim, ActionTables, DrlgWorld, Pending};
use d2_sim::world::waypoints::{WaypointData, NO_WAYPOINT};

use super::{enable_paths, form, Form, WALK_IDS};
use crate::adapters::handlers::world::{ActionWorld, Outbox};
use crate::adapters::{PlayerData, PlayerFields, SimGame};
use crate::buffers::QueueError;
use crate::seams::{ClientId, Intents, MessageSink, PlayerGate, ResultCode, Tick};

// ---- fixture -----------------------------------------------------------------------------

/// The seams with no provider: interaction info and the transport.
#[derive(Default)]
struct TestPending {
    interact: BTreeMap<UnitId, (u8, u32)>,
    sent: Vec<(UnitId, Vec<u8>)>,
}

impl Pending for TestPending {
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
}

impl Outbox for TestPending {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

/// Every message queued, in order.
#[derive(Default)]
struct Sink(Vec<(ClientId, Vec<u8>)>);

impl MessageSink for Sink {
    fn queue(&mut self, client: ClientId, msg: &[u8]) -> Result<(), QueueError> {
        self.0.push((client, msg.to_vec()));
        Ok(())
    }
}

/// The listed rooms per level, every cell a floor.
struct Types(BTreeMap<u32, Vec<TileRect>>);

impl LevelTypes for Types {
    fn generate(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        level: LevelIdx,
    ) -> Result<(), DrlgError> {
        let id = drlg.level(level).id;
        for rect in self.0.get(&id).cloned().unwrap_or_default() {
            let r = drlg.alloc_room(level, RoomKind::Preset, rect);
            drlg.room_mut(r).dt1_mask = 1;
            drlg.link_room(r, LinkAt::Tail);
        }
        Ok(())
    }
    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        _: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        use d2_sim::drlg::tiles::cell;
        let r = drlg.room(room).rect;
        let (w, h) = (r.w as usize + 1, r.h as usize + 1);
        let mut g = CellGrid::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                g.set(x, y, cell::FLOOR | if edge { cell::LINKED } else { 0 });
            }
        }
        Ok(RoomGrids {
            passes: vec![GridPass {
                cells: g,
                orientation: None,
                fill_blanks: false,
            }],
            ..RoomGrids::default()
        })
    }
}

struct Tiles(BTreeMap<Vec<u8>, Vec<TileInfo>>);

impl TileSource for Tiles {
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]> {
        self.0.get(path).map(Vec::as_slice)
    }
}

fn tile(o: u32, main: u32, sub: u32, rarity: u32) -> TileInfo {
    TileInfo {
        orientation: o,
        main,
        sub,
        rarity,
        material: 0,
        subtile_flags: [0; 25],
    }
}

fn tiles() -> Tiles {
    use d2_sim::drlg::tiles::FIXED_LIBRARY;
    let mut t = BTreeMap::new();
    t.insert(b"floor.dt1".to_vec(), vec![tile(0, 0, 0, 1)]);
    let blank_tile = |sub| {
        let mut x = tile(0, 30, sub, 0);
        x.subtile_flags = [0x20; 25];
        x
    };
    t.insert(
        FIXED_LIBRARY[0].to_vec(),
        vec![blank_tile(0), blank_tile(1)],
    );
    t.insert(FIXED_LIBRARY[1].to_vec(), vec![]);
    t.insert(FIXED_LIBRARY[2].to_vec(), vec![tile(10, 0, 0, 0)]);
    Tiles(t)
}

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// Cold Plains (rooms A, B; waypoint index 1) and the town, level 1
/// (room C; index 0).
const COLD_PLAINS: u32 = 3;
const TOWN: u32 = 1;
/// The players' class: sorceress.
const CLASS: u32 = 1;
/// `stamina` (8.8 fixed), `velocitypercent` (`pathing.md` §8.1, its
/// creation value 100, `combat/vitals.md` §1).
const STAT_STAMINA: u16 = 10;
const STAT_VELOCITY: u16 = 67;
/// Charstats of [`CLASS`]: `WalkVelocity` 6 (vector V1: 0x600),
/// `RunVelocity` 9, `RunDrain` 20.
const WALK_VELOCITY: u8 = 6;
const RUN_DRAIN: u8 = 20;

fn stat_data() -> Arc<StatData> {
    let (n, size) = (359, Itemstatcost::SIZE);
    let mut records = vec![0u8; n * size];
    for (s, r) in records.chunks_mut(size).enumerate() {
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            r[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        r[0..2].copy_from_slice(&(s as u16).to_le_bytes());
    }
    let mut t = BinTable {
        name: "itemstatcost".into(),
        source: "synthetic".into(),
        count: n,
        record_size: size,
        records,
    };
    stat_ops(&mut t);
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        ..StatData::default()
    })
}

fn drlg() -> DrlgWorld {
    let mut data = DrlgData {
        levels: vec![LevelDef::default(); 150],
        ..DrlgData::default()
    };
    for l in &mut data.levels {
        l.warp = [-1; 8];
    }
    let mut files = vec![Vec::new(); 32];
    files[0] = b"floor.dt1".to_vec();
    data.lvltypes = vec![vec![Vec::new(); 32], files];
    for id in [TOWN, COLD_PLAINS] {
        data.levels[id as usize].drlg_type = 2;
        data.levels[id as usize].level_type = 1;
    }
    let mut types = Types(BTreeMap::from([
        (
            COLD_PLAINS,
            vec![TileRect::new(0, 0, 8, 8), TileRect::new(8, 0, 8, 8)],
        ),
        (TOWN, vec![TileRect::new(0, 16, 8, 8)]),
    ]));
    let mut dungeon = Dungeon::default();
    dungeon.acts[0] = Some(Drlg::create(0, 1, 0, 0, false, &data, &mut types).unwrap());
    DrlgWorld {
        dungeon,
        data: Arc::new(data),
        tiles: Box::new(tiles()),
        types: Box::new(types),
    }
}

fn tables() -> ActionTables {
    let mut charstats = vec![blank::<Charstats>(); 7];
    charstats[CLASS as usize].walkvelocity = WALK_VELOCITY;
    charstats[CLASS as usize].runvelocity = 9;
    charstats[CLASS as usize].rundrain = RUN_DRAIN;
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
            charstats,
            difficultylevels: Vec::new(),
            monstats: Vec::new(),
            monstats2: Vec::new(),
            hitclass: Vec::new(),
        },
        levels: Vec::new(),
        skill_modes: Vec::new(),
    }
}

/// `levels` rows (count 150) and one waypoint object class 0.
fn waypoint_data() -> WaypointData {
    let mut levels = vec![blank::<Levels>(); 150];
    for l in &mut levels {
        l.waypoint = NO_WAYPOINT;
    }
    levels[TOWN as usize].waypoint = 0;
    levels[COLD_PLAINS as usize].waypoint = 1;
    let mut o: Objects = blank();
    o.operatefn = 23;
    o.initfn = 17;
    o.framecnt1 = 15 << 8;
    WaypointData::new(&levels, &[o])
}

/// Messages queued, in order.
type Queued = Vec<(ClientId, Vec<u8>)>;
/// (precise x, precise y, mode, queued) after one tick.
type Frame = (u32, u32, u32, Queued);

type Sim = SimGame<ActionSim<TestPending>, ActionWorld>;

struct Fx {
    sim: Sim,
    /// Active rooms A, B (Cold Plains) and C (town).
    a: RoomId,
    b: RoomId,
    c: RoomId,
    out: Sink,
}

const ALIVE: PlayerGate = PlayerGate {
    mode: 1,
    uninterruptable: false,
};

impl Fx {
    /// The game with the path provider on (before any allocation) and
    /// every room streamed.
    fn new() -> Self {
        let hooks = ActionHooks::new(
            Arc::new(tables()),
            drlg(),
            Seed::init_low(1234),
            TestPending::default(),
        );
        let mut events = ActionSim::new(stat_data(), UnitData::default(), hooks);
        enable_paths(&mut events).expect("embedded path tables");
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let mut active = Vec::new();
        for id in [COLD_PLAINS, TOWN] {
            let r = events
                .hooks()
                .drlg
                .with_act(0, &mut game.lists, |d, svc| {
                    let l = d.get_or_alloc_level(svc.data, svc.types, id)?;
                    d.generate_level(svc.data, svc.types, l)?;
                    let mut out = Vec::new();
                    for r in d.level_rooms(l) {
                        out.push(d.stream_room(svc, r)?.expect("active"));
                    }
                    Ok::<_, DrlgError>(out)
                })
                .unwrap()
                .unwrap();
            active.extend(r);
        }
        let mut sim: Sim = SimGame::with_events(game, events);
        sim.world.waypoints = Some(waypoint_data());
        Self {
            sim,
            a: active[0],
            b: active[1],
            c: active[2],
            out: Sink::default(),
        }
    }

    fn alloc(&mut self, ty: UnitType, room: RoomId, x: i32, y: i32) -> UnitId {
        let req = AllocRequest {
            ty,
            class: if ty == UnitType::Player { CLASS } else { 0 },
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: ty == UnitType::Player,
        };
        self.sim
            .events
            .with(&mut self.sim.game, |g, v| v.allocate(g, &req, x, y))
            .expect("allocated")
    }

    /// A player (neutral, velocity percent 100, stamina 0x6400) for
    /// `client` in `room` at (x, y).
    fn player(&mut self, client: ClientId, room: RoomId, x: i32, y: i32) -> UnitId {
        let p = self.alloc(UnitType::Player, room, x, y);
        // Players are allocated in mode 0; neutral (`units.md` §2).
        self.sim.events.sys.units.get_mut(p).unwrap().mode = 1;
        self.sim.events.with(&mut self.sim.game, |_, v| {
            v.set_base(p, STAT_VELOCITY, 100);
            v.set_base(p, STAT_STAMINA, 0x6400);
        });
        self.sim
            .join(client, Some(p), Some(room), client_state::IN_GAME)
            .unwrap();
        self.sim.set_player(
            p,
            PlayerFields {
                gate: ALIVE,
                data: Some(PlayerData { last_accept: 0 }),
            },
        );
        p
    }

    fn guid(&self, u: UnitId) -> u32 {
        self.sim.game.lists.unit(u).unwrap().guid
    }

    fn path(&mut self, u: UnitId) -> DynamicPath {
        self.sim
            .events
            .hooks()
            .paths
            .as_ref()
            .unwrap()
            .dynamic(u)
            .unwrap()
            .clone()
    }

    fn mode(&self, u: UnitId) -> u32 {
        self.sim.events.sys.units.get(u).unwrap().mode
    }

    fn stamina(&mut self, u: UnitId) -> i32 {
        self.sim
            .events
            .with(&mut self.sim.game, |_, v| v.stat(u, STAT_STAMINA))
    }

    /// One message from `client` through `SimGame::handle` (the
    /// dispatcher's checks are not part of this module): the result and
    /// what was queued.
    fn handle(&mut self, client: ClientId, msg: &[u8]) -> (ResultCode, Vec<(ClientId, Vec<u8>)>) {
        let r = self.sim.handle(client, msg, msg.len(), &mut self.out);
        (r, std::mem::take(&mut self.out.0))
    }

    /// One game tick; what was queued during it.
    fn tick(&mut self) -> Vec<(ClientId, Vec<u8>)> {
        self.sim.tick(&mut self.out);
        std::mem::take(&mut self.out.0)
    }

    /// Ticks while `u` is in `mode`, at most `max`: (precise x, precise
    /// y, mode, messages) after each tick.
    fn run(&mut self, u: UnitId, mode: u32, max: usize) -> Vec<Frame> {
        let mut out = Vec::new();
        for _ in 0..max {
            let sent = self.tick();
            let d = self.path(u);
            let m = self.mode(u);
            out.push((d.precise_x, d.precise_y, m, sent));
            if m != mode {
                break;
            }
        }
        out
    }

    fn assert_clean(&mut self) {
        assert!(
            self.sim.world.faults.is_empty(),
            "{:?}",
            self.sim.world.faults
        );
        assert!(self.sim.unhandled.is_empty(), "{:?}", self.sim.unhandled);
        assert!(self.sim.tick_faults.is_empty());
        let h = self.sim.events.hooks();
        assert!(h.errors.is_empty(), "{:?}", h.errors);
        assert!(self.sim.events.sys.errors.is_empty());
    }
}

fn point(id: u8, x: u16, y: u16) -> Vec<u8> {
    let mut m = vec![id];
    m.extend_from_slice(&x.to_le_bytes());
    m.extend_from_slice(&y.to_le_bytes());
    m
}

fn unit_msg(id: u8, ty: u32, guid: u32) -> Vec<u8> {
    let mut m = vec![id];
    m.extend_from_slice(&ty.to_le_bytes());
    m.extend_from_slice(&guid.to_le_bytes());
    m
}

/// Precise x of sub-tile `x`'s centre (`path-placement.md` §1).
fn centre(x: u32) -> u32 {
    (x << 16) | 0x8000
}

/// The movement of every walk / run in this file: (26, 10) → (31, 10)
/// at velocity 0x600 (`pathing.md` vector M1 translated): +0x6000 per
/// tick for 13 ticks, the 14th lands on the target centre and stops
/// (mode 1).
fn assert_m1(ticks: &[Frame], mode: u32) {
    assert_eq!(ticks.len(), 14);
    for (k, t) in ticks.iter().take(13).enumerate() {
        assert_eq!(t.0, 0x1AE000 + k as u32 * 0x6000, "tick {}", k + 1);
        assert_eq!((t.1, t.2), (centre(10), mode), "tick {}", k + 1);
    }
    assert_eq!(
        (ticks[13].0, ticks[13].1, ticks[13].2),
        (centre(31), centre(10), 1)
    );
}

/// Two players in room A: `p` at (26, 10) for client 0, `q` at (26, 14)
/// for client 1, and an object at (31, 10).
fn two_players() -> (Fx, UnitId, UnitId, UnitId) {
    let mut fx = Fx::new();
    let a = fx.a;
    let o = fx.alloc(UnitType::Object, a, 31, 10);
    let p = fx.player(0, a, 26, 10);
    let q = fx.player(1, a, 26, 14);
    (fx, p, q, o)
}

// Covers: specs/sim/pathing.md §1.1, §1.2, §10 r1, §10 r2
#[test]
fn walk_to_point_moves_the_player_and_tells_the_other_client() {
    let (mut fx, p, _, _) = two_players();
    let guid = fx.guid(p);
    // §10 rule 1: the request replies nothing, result 0.
    assert_eq!(
        fx.handle(0, &point(0x01, 31, 10)),
        (ResultCode::Done, vec![])
    );
    assert_eq!(fx.mode(p), 2);
    let d = fx.path(p);
    assert_eq!((d.target_x, d.target_y, d.target_unit), (31, 10, None));
    assert_eq!(d.velocity, 0x600);
    let ticks = fx.run(p, 2, 20);
    assert_m1(&ticks, 2);
    // §10 rule 2 in the update pass of tick 1 (the mode set queued the
    // player): client 1 gets 0x0F (walk code 1, target, the cell); the
    // walking player's own client gets nothing (R4–R6).
    let want = PlayerMove {
        type_: 0,
        guid,
        code: 1,
        target_x: 31,
        target_y: 10,
        zero: 0,
        x: 26,
        y: 10,
    };
    assert_eq!(ticks[0].3, vec![(1, want.encode().to_vec())]);
    for t in &ticks[1..] {
        assert!(t.3.is_empty(), "{:?}", t.3);
    }
    // The stop's mode set (neutral) queues it again: mode 1 has no walk
    // row, nothing is sent.
    assert!(fx.tick().is_empty());
    assert_eq!(fx.path(p).cell(), d2_sim::path::Point::new(31, 10));
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §1.1, §1.5 r2, §9.9 r2, §9.9 r3, §10 r2
#[test]
fn run_to_point_drains_stamina_and_sends_the_run_code() {
    let (mut fx, p, _, _) = two_players();
    let guid = fx.guid(p);
    assert_eq!(
        fx.handle(0, &point(0x03, 31, 10)),
        (ResultCode::Done, vec![])
    );
    assert_eq!(fx.mode(p), 3);
    // The run stat list (`pathing.md` §8.2, `0x00620E80`) is not wired
    // (`docs/handoff/wire-path-sim.md` §5): velocity stays 0x600.
    let ticks = fx.run(p, 3, 20);
    assert_m1(&ticks, 3);
    let want = PlayerMove {
        type_: 0,
        guid,
        code: 0x17,
        target_x: 31,
        target_y: 10,
        zero: 0,
        x: 26,
        y: 10,
    };
    assert_eq!(ticks[0].3, vec![(1, want.encode().to_vec())]);
    // §9.9: 2 · RunDrain = 40 per running tick, 14 ticks.
    assert_eq!(fx.stamina(p), 0x6400 - 14 * 2 * i32::from(RUN_DRAIN));
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §1.1, §1.2 r1, §1.2 r5, §10 r2
#[test]
fn walk_and_run_to_a_unit_send_0x10() {
    for (id, mode, code) in [(0x02u8, 2u32, 0u8), (0x04, 3, 0x18)] {
        let (mut fx, p, _, o) = two_players();
        let (guid, og) = (fx.guid(p), fx.guid(o));
        assert_eq!(
            fx.handle(0, &unit_msg(id, 2, og)),
            (ResultCode::Done, vec![])
        );
        assert_eq!(fx.mode(p), mode);
        let t = fx.path(p).target_unit.expect("target unit");
        assert_eq!((t.unit, t.ty, t.guid), (o, UnitType::Object, og));
        let ticks = fx.run(p, mode, 20);
        assert_m1(&ticks, mode);
        let want = PlayerToTarget {
            type_: 0,
            guid,
            code,
            target_type: 2,
            target_guid: og,
            x: 26,
            y: 10,
        };
        assert_eq!(ticks[0].3, vec![(1, want.encode().to_vec())], "{id:#04x}");
        assert!(ticks[1..].iter().all(|t| t.3.is_empty()));
        fx.assert_clean();
    }
}

// Covers: specs/sim/pathing.md §9.6 r9
#[test]
fn walking_into_room_b_changes_the_players_room() {
    // Room A is sub-tiles x 0..40, B 40..80: 6 sub-tiles at 0x6000 are
    // 16 ticks; the step into x = 40 moves the player to B's unit list.
    let mut fx = Fx::new();
    let (a, b) = (fx.a, fx.b);
    let p = fx.player(0, a, 37, 10);
    assert_eq!(
        fx.handle(0, &point(0x01, 43, 10)),
        (ResultCode::Done, vec![])
    );
    let ticks = fx.run(p, 2, 30);
    assert_eq!(ticks.len(), 16);
    assert_eq!(
        (ticks[15].0, ticks[15].1, ticks[15].2),
        (centre(43), centre(10), 1)
    );
    // One client: nothing is sent while walking (R4–R6).
    assert!(ticks.iter().all(|t| t.3.is_empty()));
    let d = fx.path(p);
    assert_eq!((d.room, d.prev_room), (Some(b), Some(a)));
    assert_eq!(fx.sim.game.lists.unit(p).unwrap().room(), Some(b));
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §1.1, §1.2 r1, §1.3 r3
#[test]
fn refused_requests_still_return_0() {
    let (mut fx, p, _, _) = two_players();
    // No unit of that GUID: a log line, nothing else (§1.2 rule 1).
    assert_eq!(
        fx.handle(0, &unit_msg(0x02, 2, 999)),
        (ResultCode::Done, vec![])
    );
    assert_eq!(fx.mode(p), 1);
    // Mode DT (0) refuses every movement (§1.3 table).
    fx.sim.events.sys.units.get_mut(p).unwrap().mode = 0;
    assert_eq!(
        fx.handle(0, &point(0x01, 31, 10)),
        (ResultCode::Done, vec![])
    );
    assert_eq!(fx.mode(p), 0);
    assert_eq!(fx.path(p).cell(), d2_sim::path::Point::new(26, 10));
    let ticks: Vec<_> = (0..3).map(|_| fx.tick()).collect();
    assert!(ticks.iter().all(Vec::is_empty));
    fx.assert_clean();
}

#[test]
fn without_the_path_provider_the_ids_stay_stubs() {
    // M08 for the routing: the same game with the provider off leaves
    // 0x01–0x04 to the stub (recorded, result 0, nothing moves).
    let mut fx = Fx::new();
    fx.sim.events.hooks().paths = None;
    let a = fx.a;
    let p = fx.player(0, a, 26, 10);
    for m in [
        point(0x01, 31, 10),
        unit_msg(0x02, 0, 0),
        point(0x03, 1, 1),
        unit_msg(0x04, 0, 0),
    ] {
        assert_eq!(fx.handle(0, &m), (ResultCode::Done, vec![]));
    }
    assert_eq!(fx.mode(p), 1);
    let ids: Vec<u8> = fx.sim.unhandled.iter().map(|u| u.1).collect();
    assert_eq!(ids, [0x01, 0x02, 0x03, 0x04]);
}

/// The waypoint `o` at (20, 20) in room A, the player at (22, 20) knowing
/// the town and Cold Plains, the menu open; C→S 0x49 to `level`.
fn travel(fx: &mut Fx, level: u32) -> (UnitId, (ResultCode, Queued)) {
    let a = fx.a;
    let o = fx.alloc(UnitType::Object, a, 20, 20);
    let p = fx.player(0, a, 22, 20);
    let wp = fx.guid(o);
    let h = fx.sim.events.hooks();
    let rec = h.waypoints.entry(p).or_default().get_mut(0);
    rec.set(0).unwrap();
    rec.set(1).unwrap();
    h.x.interact.insert(p, (2, wp));
    let mut m = vec![0x49];
    m.extend_from_slice(&wp.to_le_bytes());
    m.extend_from_slice(&(level as u16).to_le_bytes());
    m.extend_from_slice(&[0, 0]);
    let r = fx.handle(0, &m);
    (p, r)
}

// Covers: specs/sim/path-placement.md §10 r6, §11; specs/world/waypoints.md §7 r5, §7 r7
#[test]
fn waypoint_to_the_town_places_the_player_and_sends_0x0d() {
    // The e2e step-6 condition: the same-act warp places the player in
    // the destination's spawn room (C, the town's one room), so rule 7
    // runs: mode request 2 at its own position (no path → neutral, town:
    // 5) and S→C 0x0D at x + 3, y + 3. The placement's 0x07 comes first
    // (`waypoints.md` §8 rule 3).
    let mut fx = Fx::new();
    let c = fx.c;
    let (p, r) = travel(&mut fx, TOWN);
    let guid = fx.guid(p);
    let d = fx.path(p);
    assert_eq!(d.room, Some(c));
    assert_eq!(fx.sim.game.lists.unit(p).unwrap().room(), Some(c));
    let rect = fx.sim.events.hooks().drlg.subtile_rect(c).unwrap();
    assert!(rect.contains(d.x(), d.y()));
    assert_eq!(fx.mode(p), 5);
    let (x, y) = (d.x() as u16, d.y() as u16);
    let reveal = MapReveal {
        x: 0,
        y: 16,
        level: TOWN as u8,
    };
    let stop = PlayerStop {
        type_: 0,
        guid,
        a: 1,
        x: x + 3,
        y: y + 3,
        b: 0,
        life_pct: 0,
    };
    assert_eq!(
        r,
        (
            ResultCode::Done,
            vec![(0, reveal.encode().to_vec()), (0, stop.encode().to_vec())]
        )
    );
    assert_ne!(fx.sim.events.sys.units.get(p).unwrap().flags2 & 0x10000, 0);
    // Finding (`docs/handoff/wire-path-server.md` §4): no 0x15 follows.
    // The placement queued the player in C's update queue, but tick 1's
    // per-client update walks the client's room (still A) adjacency
    // before the room switch (`tick.md` §6.5), and step 6 clears C's
    // queue; the recording R3 (`path-placement.md`) has 0x15 the next
    // tick. Spec question, not a fix here.
    for _ in 0..3 {
        assert!(fx.tick().is_empty());
    }
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §10 r3; specs/sim/path-placement.md §10 r6
#[test]
fn a_warp_within_the_level_sends_0x15_in_the_next_update_pass() {
    // Travel to Cold Plains itself: placed in room A (the client's room),
    // so tick 1's update pass sends 0x15 (flags 2 bit 0x10000 → flag 1)
    // at the player's cell. The arrival's room test fails (the spawn
    // search's room is not A: `wire-path-sim.md` §6), so no 0x0D.
    let mut fx = Fx::new();
    let a = fx.a;
    let (p, r) = travel(&mut fx, COLD_PLAINS);
    let guid = fx.guid(p);
    let reveal = MapReveal {
        x: 0,
        y: 0,
        level: COLD_PLAINS as u8,
    };
    assert_eq!(r, (ResultCode::Done, vec![(0, reveal.encode().to_vec())]));
    let d = fx.path(p);
    assert_eq!(d.room, Some(a));
    let reassign = ReassignPlayer {
        type_: 0,
        guid,
        x: d.x() as u16,
        y: d.y() as u16,
        flag: 1,
    }
    .encode()
    .to_vec();
    assert_eq!(fx.tick(), vec![(0, reassign.clone())]);
    assert!(fx.tick().is_empty());
    // TODO(spec: `0x00553220` clean-up flags): flags 2 bit 0x10000 is
    // never cleared, so the next queueing (the walk's mode set) sends
    // 0x15 again. A spec answer changes this line.
    assert_eq!(
        fx.handle(0, &point(0x01, 26, 22)),
        (ResultCode::Done, vec![])
    );
    assert_eq!(fx.tick(), vec![(0, reassign)]);
    fx.assert_clean();
}

// ---- the update pass (`pathing.md` §10 rules 2–3, `tick.md` §6.5) -----------------------

/// S→C 0x15 for `u` at its path cell with `flag`.
fn reassign(fx: &mut Fx, u: UnitId, flag: u8) -> Vec<u8> {
    let d = fx.path(u);
    ReassignPlayer {
        type_: 0,
        guid: fx.guid(u),
        x: d.x() as u16,
        y: d.y() as u16,
        flag,
    }
    .encode()
    .to_vec()
}

/// S→C 0x0F for `u` walking from (26, 10) to (31, 10) with `code`.
fn m1_move(fx: &Fx, u: UnitId, code: u8) -> Vec<u8> {
    PlayerMove {
        type_: 0,
        guid: fx.guid(u),
        code,
        target_x: 31,
        target_y: 10,
        zero: 0,
        x: 26,
        y: 10,
    }
    .encode()
    .to_vec()
}

fn set_flags2(fx: &mut Fx, u: UnitId, bits: u32) {
    fx.sim.events.sys.units.get_mut(u).unwrap().flags2 |= bits;
}

// Covers: specs/sim/pathing.md §10 r2, §10 r3
#[test]
fn bit_0x800_tells_only_the_other_clients_before_the_mode_update() {
    // Rule 3: flags 2 bit 0x800 → 0x15 with flag 0 to every client whose
    // player is not the unit; the unit's own client gets nothing. Rule 3
    // runs before rule 2 in the same pass, so client 1 gets 0x15, then
    // the walk's 0x0F.
    let (mut fx, p, _, _) = two_players();
    set_flags2(&mut fx, p, 0x800);
    let x15 = reassign(&mut fx, p, 0);
    assert_eq!(
        fx.handle(0, &point(0x01, 31, 10)),
        (ResultCode::Done, vec![])
    );
    let ticks = fx.run(p, 2, 20);
    assert_m1(&ticks, 2);
    assert_eq!(ticks[0].3, vec![(1, x15), (1, m1_move(&fx, p, 1))]);
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §10 r2, §10 r3
#[test]
fn bit_0x10000_tells_every_client_before_the_mode_update() {
    // Rule 3: bit 0x10000 → 0x15 with flag 1, the own client included;
    // bit 0x10000 wins over 0x800 (flag 1, and the own client still gets
    // it). Then rule 2 for the other client only.
    let (mut fx, p, _, _) = two_players();
    set_flags2(&mut fx, p, 0x10000 | 0x800);
    let x15 = reassign(&mut fx, p, 1);
    assert_eq!(
        fx.handle(0, &point(0x01, 31, 10)),
        (ResultCode::Done, vec![])
    );
    let ticks = fx.run(p, 2, 20);
    assert_m1(&ticks, 2);
    let mut got = ticks[0].3.clone();
    // Clients run in client-list order (`unit-order.md` §7); per client
    // the order of rules 3 then 2 is the spec's.
    got.sort_by_key(|m| m.0);
    assert_eq!(
        got,
        vec![(0, x15.clone()), (1, x15), (1, m1_move(&fx, p, 1))]
    );
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §1.3, §10 r2
#[test]
fn town_walk_sends_the_walk_code() {
    // In the town the walk request sets mode 6 (town walk); its row
    // shares the walk's code 1 (rule 2).
    let mut fx = Fx::new();
    let c = fx.c;
    let rect = fx.sim.events.hooks().drlg.subtile_rect(c).unwrap();
    let (x, y) = (rect.x + 10, rect.y + 10);
    let p = fx.player(0, c, x, y);
    fx.player(1, c, x, y + 4);
    assert_eq!(
        fx.handle(0, &point(0x01, (x + 5) as u16, y as u16)),
        (ResultCode::Done, vec![])
    );
    assert_eq!(fx.mode(p), 6);
    let want = PlayerMove {
        type_: 0,
        guid: fx.guid(p),
        code: 1,
        target_x: (x + 5) as u16,
        target_y: y as u16,
        zero: 0,
        x: x as u16,
        y: y as u16,
    };
    let ticks = fx.run(p, 6, 20);
    // The stop sets town neutral (5).
    assert_eq!(ticks.last().unwrap().2, 5);
    assert_eq!(ticks[0].3, vec![(1, want.encode().to_vec())]);
    assert!(ticks[1..].iter().all(|t| t.3.is_empty()));
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §10 r2, §10 r3
#[test]
fn only_players_get_the_movement_messages() {
    // An object with a player's flags in a walk mode, queued in the
    // clients' room: rules 2–3 are the player's (monsters: rule 4, the
    // monster update's, not written; other unit types: no rule), nothing
    // is sent.
    let (mut fx, _, _, o) = two_players();
    let r = fx.sim.events.sys.units.get_mut(o).unwrap();
    r.mode = 2;
    r.flags |= d2_sim::units::record::flags::CHANGED;
    r.flags2 |= 0x10000 | 0x800;
    fx.sim.game.lists.queue_update(o).unwrap();
    assert!(fx.tick().is_empty());
    fx.assert_clean();
}

#[test]
fn the_update_pass_sends_nothing_without_the_path_provider() {
    // M08 for the update pass: the same queued player with bit 0x10000
    // sends 0x15 with the provider on, nothing with it off.
    for on in [true, false] {
        let (mut fx, p, _, _) = two_players();
        set_flags2(&mut fx, p, 0x10000);
        let x15 = reassign(&mut fx, p, 1);
        if !on {
            fx.sim.events.hooks().paths = None;
        }
        fx.sim.game.lists.queue_update(p).unwrap();
        let mut got = fx.tick();
        got.sort_by_key(|m| m.0);
        if on {
            assert_eq!(got, vec![(0, x15.clone()), (1, x15)]);
        } else {
            assert!(got.is_empty(), "{got:?}");
        }
    }
}

// ---- id table ----------------------------------------------------------------------------

const CLIENT_TSV: &str = include_str!("../../../../../../specs/sim/client-messages.tsv");

/// The table's problems against the TSV rows: each id must be a `sim`
/// `handler` row of the same name, with the layout of its form, and
/// the dispatcher's point / unit parse must agree with the form.
fn problems(ids: &[(u8, &str, &str, Form, u32)], tsv: &str) -> Vec<String> {
    let mut lines = tsv.lines();
    let head: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split('\t').collect()).collect();
    let mut out = Vec::new();
    for w in ids.windows(2) {
        if w[0].0 >= w[1].0 {
            out.push(format!("order {:#04x}", w[1].0));
        }
    }
    for &(id, name, _, f, _) in ids {
        let key = format!("0x{id:02X}");
        let Some(r) = rows.iter().find(|r| r[col("id")] == key) else {
            out.push(format!("{id:#04x} missing"));
            continue;
        };
        let layout = match f {
            Form::Point => "x:u16@1 y:u16@3",
            Form::Unit => "type:u32@1 id:u32@5",
        };
        if r[col("name")] != name {
            out.push(format!("{id:#04x} name"));
        }
        if r[col("kind")] != "handler" || r[col("scope")] != "sim" {
            out.push(format!("{id:#04x} not a sim handler"));
        }
        if r[col("layout")] != layout {
            out.push(format!("{id:#04x} layout"));
        }
        let parsed = match f {
            Form::Point => crate::dispatch::is_point(id),
            Form::Unit => crate::dispatch::is_unit(id),
        };
        if !parsed {
            out.push(format!("{id:#04x} parse"));
        }
    }
    out
}

#[test]
fn walk_ids_match_client_messages_tsv() {
    assert_eq!(problems(WALK_IDS, CLIENT_TSV), Vec::<String>::new());
    assert_eq!(form(0x03), Some(Form::Point));
    assert_eq!(form(0x04), Some(Form::Unit));
    assert_eq!(form(0x05), None);
    // Perturbations (METHODS M08): a stubbed row, a changed layout, a
    // swapped form are each reported.
    let stub = CLIENT_TSV.replace(
        "\thandler\talive\twalk to a unit",
        "\tstub0\talive\twalk to a unit",
    );
    assert_eq!(problems(WALK_IDS, &stub), ["0x02 not a sim handler"]);
    let moved = CLIENT_TSV.replace(
        "0x03\tRun\t5\t==5\tx:u16@1 y:u16@3",
        "0x03\tRun\t5\t==5\tx:u16@3 y:u16@1",
    );
    assert_eq!(problems(WALK_IDS, &moved), ["0x03 layout"]);
    let mut swapped = WALK_IDS.to_vec();
    swapped[3].3 = Form::Point;
    assert_eq!(
        problems(&swapped, CLIENT_TSV),
        ["0x04 layout", "0x04 parse"]
    );
}
