// Spec: specs/client/bridge.md §3; specs/sim/pathing.md §1.1, §9, §10; specs/sim/path-placement.md §10, §11; specs/world/waypoints.md §6, §7 (end to end)
//! End-to-end walk and waypoint: the bridge (`d2_client::bridge`) on its
//! local link over the in-process `d2-server` host, whose game is
//! `SimGame` on the wired `d2-sim` (`ActionSim` with the path provider
//! on, `ActionWorld` host), from a synthetic act 0 (no game files):
//! Cold Plains as rooms A (sub-tiles x 0..40) and B (x 40..80), the
//! town (level 1) as room C.
//!
//! The single player:
//!
//! 1. walks (C→S 0x01) from (22, 20) in room A across the room edge to
//!    (45, 20) in room B: per-tick positions through the real timer
//!    queue (event 0, `pathing.md` §9), the room change at x = 40;
//! 2. runs back (C→S 0x03) to (24, 20), stamina drained per tick (§9.9);
//! 3. walks to the waypoint object (C→S 0x02, unit form) and stops on
//!    its position;
//! 4. takes the waypoint to the town (C→S 0x49): the same-act warp places
//!    the player in the town's spawn room (`path-placement.md` §10, §11),
//!    so `waypoints.md` §7 rule 7 holds and the client receives S→C 0x07
//!    then 0x0D with the exact bytes.
//!
//! The walking player's own client receives nothing while walking
//! (`pathing.md` §10 rule 2, recordings R4–R6). After the warp no S→C
//! 0x15 arrives (`docs/handoff/wire-path-server.md` §4 finding 1).

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_client::bridge::dispatch::Dispatch;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::{Bridge, FrameReport};
use d2_data::bin::BinTable;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Charstats, Itemstatcost, Levels, Objects, Record};
use d2_proto::client::{Run, TakeOrCloseWp, Walk, WalkToUnit};
use d2_proto::server::{MapReveal, PlayerStop};
use d2_server::adapters::handlers::walk::enable_paths;
use d2_server::adapters::handlers::world::{ActionWorld, Outbox};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::dispatch::Outcome;
use d2_server::host::{Handled, Host};
use d2_server::seams::{Clock, PlayerGate, Pos, ResultCode};
use d2_sim::combat::CombatTables;
use d2_sim::drlg::room::LinkAt;
use d2_sim::drlg::tiles::{cell, FIXED_LIBRARY};
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

// ---- the action wiring's unprovided seams ------------------------------------------------

/// Interaction info and the transport (handed to the host through
/// [`Outbox`]); every path seam is the provider's.
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

// ---- synthetic act 0 ---------------------------------------------------------------------

/// Cold Plains (rooms A, B; waypoint index 1) and the town (room C,
/// index 0).
const COLD_PLAINS: u32 = 3;
const TOWN: u32 = 1;
/// Sorceress; charstats `WalkVelocity` 6 (velocity 0x600), `RunDrain` 20.
const CLASS: u32 = 1;
const RUN_DRAIN: i32 = 20;
const STAT_STAMINA: u16 = 10;
const STAT_VELOCITY: u16 = 67;
const STAMINA: i32 = 0x6400;
/// The waypoint object and the player's start.
const WP_AT: (i32, i32) = (20, 20);
const START: (i32, i32) = (22, 20);

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
        roof_height: 0,
        height: 0,
    }
}

fn tiles() -> Tiles {
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

fn tables() -> ActionTables {
    let mut charstats = vec![blank::<Charstats>(); 7];
    charstats[CLASS as usize].walkvelocity = 6;
    charstats[CLASS as usize].runvelocity = 9;
    charstats[CLASS as usize].rundrain = RUN_DRAIN as u8;
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

// ---- host and bridge ---------------------------------------------------------------------

/// (precise x, precise y, mode, chunks received) after one frame.
type Step = (u32, u32, u32, Vec<Vec<u8>>);

type Sim = SimGame<ActionSim<TestPending>, ActionWorld>;

struct Ms(u32);

impl Clock for Ms {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

type Link = LocalLink<Sim, ProtoSizes, PendingSession, Ms>;

/// The local link, recording every S→C chunk the bridge receives.
struct Tap {
    inner: Link,
    chunks: Vec<Vec<u8>>,
}

impl ServerLink for Tap {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let got = self.inner.receive();
        self.chunks.extend(got.iter().cloned());
        got
    }
}

const LOCAL_CLIENT: u32 = d2_client::bridge::LOCAL_CLIENT;

struct Fx {
    bridge: Bridge<Tap>,
    player: UnitId,
    wp: UnitId,
    /// Active rooms A, B, C.
    rooms: [RoomId; 3],
}

impl Fx {
    /// The game (path provider on before any allocation, every room
    /// streamed), the waypoint object and the player in room A, the
    /// local client joined; the first bridge frame (no tick) has run.
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
        let mut rooms = Vec::new();
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
            rooms.extend(r);
        }
        let rooms = [rooms[0], rooms[1], rooms[2]];
        let mut alloc = |ty: UnitType, class: u32, (x, y): (i32, i32)| {
            let req = AllocRequest {
                ty,
                class,
                room: Some(rooms[0]),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: ty == UnitType::Player,
            };
            events
                .with(&mut game, |g, v| v.allocate(g, &req, x, y))
                .expect("allocated")
        };
        let wp = alloc(UnitType::Object, 0, WP_AT);
        let player = alloc(UnitType::Player, CLASS, START);
        // Players are allocated in mode 0; neutral (`units.md` §2).
        events.sys.units.get_mut(player).unwrap().mode = 1;
        events.with(&mut game, |_, v| {
            v.set_base(player, STAT_VELOCITY, 100);
            v.set_base(player, STAT_STAMINA, STAMINA);
        });
        let rec = events
            .hooks()
            .waypoints
            .entry(player)
            .or_default()
            .get_mut(0);
        rec.set(0).unwrap();
        rec.set(1).unwrap();
        let mut sim: Sim = SimGame::with_events(game, events);
        sim.world.waypoints = Some(waypoint_data());
        sim.join(
            LOCAL_CLIENT,
            Some(player),
            Some(rooms[0]),
            client_state::IN_GAME,
        )
        .unwrap();
        sim.set_player(
            player,
            PlayerFields {
                gate: PlayerGate {
                    mode: 1,
                    uninterruptable: false,
                },
                data: Some(PlayerData { last_accept: 0 }),
            },
        );
        let link = LocalLink::new(Host::new(
            sim,
            ProtoSizes,
            PendingSession::default(),
            Ms(1000),
        ));
        let tap = Tap {
            inner: link,
            chunks: Vec::new(),
        };
        let bridge = Bridge::with_dispatch(tap, Dispatch::from_spec().unwrap()).unwrap();
        let mut fx = Self {
            bridge,
            player,
            wp,
            rooms,
        };
        fx.stage_position();
        let r = fx.bridge.frame().unwrap();
        assert!(!r.ticked);
        fx
    }

    fn sim(&mut self) -> &mut Sim {
        &mut self.bridge.link_mut().inner.host_mut().game
    }

    fn path(&mut self) -> DynamicPath {
        let p = self.player;
        self.sim()
            .events
            .hooks()
            .paths
            .as_ref()
            .unwrap()
            .dynamic(p)
            .unwrap()
            .clone()
    }

    fn guid(&mut self, u: UnitId) -> u32 {
        self.sim().game.lists.unit(u).unwrap().guid
    }

    fn mode(&mut self) -> u32 {
        let p = self.player;
        self.sim().events.sys.units.get(p).unwrap().mode
    }

    fn room(&mut self) -> Option<RoomId> {
        let p = self.player;
        self.sim().game.lists.unit(p).unwrap().room()
    }

    /// The dispatcher's range checks read the staged unit facts
    /// (`SimGame::set_unit`, `intents-events.md` §2.4 rules 3–4), not the
    /// path record: staged from the path before each request.
    fn stage_position(&mut self) {
        let (p, o) = (self.player, self.wp);
        let d = self.path();
        let pos = Pos { x: d.x(), y: d.y() };
        let s = self.sim();
        s.set_unit(
            p,
            UnitFacts {
                act: 0,
                pos,
                owner: None,
            },
        );
        let at = Pos {
            x: WP_AT.0,
            y: WP_AT.1,
        };
        s.set_unit(
            o,
            UnitFacts {
                act: 0,
                pos: at,
                owner: None,
            },
        );
    }

    /// One bridge frame after 40 ms (one host tick): the drained
    /// messages' results, the S→C chunks the client received, the
    /// report.
    fn frame(&mut self) -> (Vec<(u8, ResultCode)>, Vec<Vec<u8>>, FrameReport) {
        self.bridge.link_mut().inner.host_mut().clock.0 += 40;
        let report = self.bridge.frame().unwrap();
        let codes = self
            .bridge
            .link()
            .inner
            .last_frame()
            .messages
            .iter()
            .map(|h| match h.handled {
                Handled::Game(Outcome::Dispatched(c)) => (h.id, c),
                ref other => panic!("not dispatched: {other:?}"),
            })
            .collect();
        let chunks = std::mem::take(&mut self.bridge.link_mut().chunks);
        assert!(report.ticked);
        (codes, chunks, report)
    }

    /// Sends `msg`, then runs frames while the player keeps the mode the
    /// request started (at most `max`): per frame the precise position,
    /// the mode and the chunks received. The first frame's result codes
    /// must be the one 0.
    fn walk(&mut self, msg: &[u8], id: u8, max: usize) -> Vec<Step> {
        self.stage_position();
        assert_eq!(self.bridge.send_bytes(msg).unwrap(), Sent::Queued);
        let mut out = Vec::new();
        let mut moving = None;
        for i in 0..max {
            let (codes, chunks, _) = self.frame();
            if i == 0 {
                assert_eq!(codes, [(id, ResultCode::Done)]);
                moving = Some(self.mode());
            } else {
                assert!(codes.is_empty());
            }
            let d = self.path();
            let m = self.mode();
            out.push((d.precise_x, d.precise_y, m, chunks));
            if Some(m) != moving {
                break;
            }
        }
        out
    }

    fn errors(&mut self) -> Vec<String> {
        let s = self.sim();
        let mut e: Vec<String> = s.world.faults.iter().map(|f| format!("{f:?}")).collect();
        e.extend(s.tick_faults.iter().map(|f| format!("{f:?}")));
        e.extend(s.unhandled.iter().map(|f| format!("unhandled {f:?}")));
        let h = s.events.hooks();
        e.extend(h.errors.iter().map(|f| format!("{f:?}")));
        e.extend(s.events.sys.errors.iter().map(|f| format!("{f:?}")));
        e
    }
}

fn centre(x: i32) -> u32 {
    ((x as u32) << 16) | 0x8000
}

fn bytes<M: d2_proto::FixedMessage>(m: &M) -> Vec<u8> {
    d2_client::bridge::intent::encode(m)
}

/// Everything one run leaves behind, compared between runs.
#[derive(Debug, PartialEq, Eq)]
struct Transcript {
    /// Per walk: (precise x, precise y, mode, chunks) per frame.
    walks: Vec<Vec<Step>>,
    /// The waypoint frame's chunks and the frames after it.
    travel: Vec<Vec<Vec<u8>>>,
    /// The player's cell, room and mode at the end; its stamina.
    end: ((i32, i32), Option<RoomId>, u32, i32),
    unowned: BTreeMap<u8, u64>,
    errors: Vec<String>,
}

fn run() -> Transcript {
    let mut fx = Fx::new();
    let [a, b, c] = fx.rooms;
    let (p, wp) = (fx.player, fx.wp);
    let mut walks = Vec::new();

    // 1. Walk from (22, 20) in A to (45, 20) in B: 23 sub-tiles at
    // 0x6000 per tick (`pathing.md` §9.4): 61 frames of +0x6000, the 62nd
    // lands on the target centre and stops (neutral). No message reaches
    // the walking player's own client.
    let w = fx.walk(&bytes(&Walk { x: 45, y: 20 }), 0x01, 80);
    assert_eq!(w.len(), 62);
    for (k, f) in w.iter().take(61).enumerate() {
        assert_eq!(f.0, centre(START.0) + (k as u32 + 1) * 0x6000, "frame {k}");
        assert_eq!((f.1, f.2), (centre(20), 2), "frame {k}");
    }
    assert_eq!((w[61].0, w[61].1, w[61].2), (centre(45), centre(20), 1));
    assert!(w.iter().all(|f| f.3.is_empty()));
    let d = fx.path();
    assert_eq!((d.room, d.prev_room), (Some(b), Some(a)));
    assert_eq!(fx.room(), Some(b));
    walks.push(w);

    // 2. Run back to (24, 20): 21 sub-tiles, 56 frames; the run list is
    // not wired (velocity 0x600, `wire-path-sim.md` §5); stamina −40
    // per running tick (§9.9). Back in room A.
    let w = fx.walk(&bytes(&Run { x: 24, y: 20 }), 0x03, 80);
    assert_eq!(w.len(), 56);
    assert_eq!(w[0].2, 3);
    assert_eq!((w[55].0, w[55].1, w[55].2), (centre(24), centre(20), 1));
    assert!(w.iter().all(|f| f.3.is_empty()));
    assert_eq!(fx.room(), Some(a));
    walks.push(w);

    // 3. Walk to the waypoint object (unit form): it stops on the
    // object's position (objects have no footprint).
    let og = fx.guid(wp);
    let w = fx.walk(&bytes(&WalkToUnit { type_: 2, id: og }), 0x02, 30);
    assert_eq!(w.len(), 11);
    assert_eq!(
        (w[10].0, w[10].1, w[10].2),
        (centre(WP_AT.0), centre(WP_AT.1), 1)
    );
    assert!(w.iter().all(|f| f.3.is_empty()));
    walks.push(w);

    // 4. The waypoint menu is open (interaction staged: the operate path
    // is the object-interaction spec's); travel to the town.
    fx.sim().events.hooks().x.interact.insert(p, (2, og));
    fx.stage_position();
    let take = TakeOrCloseWp {
        wp: og,
        level: TOWN as u16,
    };
    assert_eq!(fx.bridge.send(&take).unwrap(), Sent::Queued);
    let (codes, chunks, report) = fx.frame();
    assert_eq!(codes, [(0x49, ResultCode::Done)]);
    // The player stands in C, the town's spawn room (the e2e step-6
    // condition), neutral in town (rule 7: no path at its own position).
    let d = fx.path();
    assert_eq!((d.room, fx.room()), (Some(c), Some(c)));
    let rect = fx.sim().events.hooks().drlg.subtile_rect(c).unwrap();
    assert!(rect.contains(d.x(), d.y()));
    assert_eq!(fx.mode(), 5);
    let guid = fx.guid(p);
    let reveal = MapReveal {
        x: 0,
        y: 16,
        level: TOWN as u8,
    };
    let stop = PlayerStop {
        type_: 0,
        guid,
        a: 1,
        x: d.x() as u16 + 3,
        y: d.y() as u16 + 3,
        b: 0,
        life_pct: 0,
    };
    // In the drain: the placement's 0x07 and the arrival 0x0D; then the
    // tick's room switch sends 0x07 for each room of C's adjacency array
    // (`path-placement.md` §11 "Recipients"; the one-room town: C itself).
    let mut want = reveal.encode().to_vec();
    want.extend_from_slice(&stop.encode());
    want.extend_from_slice(&reveal.encode());
    assert_eq!(chunks.concat(), want);
    // 0x07 needs the client act (no 0x03 was sent: fatal 0x58A,
    // `client/model.md` §9 rule 1) and 0x0D's player was never announced
    // (no 0x59: dropped, §4 rule 1).
    assert_eq!(
        (
            report.messages,
            report.rejected,
            report.dropped,
            report.unowned
        ),
        (3, 2, 1, 0)
    );
    let mut travel = vec![chunks];
    // The frames after: nothing (finding 1 of the handoff: no 0x15).
    for _ in 0..3 {
        let (codes, chunks, _) = fx.frame();
        assert!(codes.is_empty() && chunks.is_empty());
        travel.push(chunks);
    }
    let unowned = fx.bridge.log().unowned.clone();
    assert_eq!(unowned, BTreeMap::new());
    assert_eq!(fx.bridge.log().dropped, BTreeMap::from([(0x0D, 1)]));
    let rejected: Vec<(u8, String)> = fx
        .bridge
        .log()
        .rejected
        .iter()
        .map(|r| (r.id, r.error.to_string()))
        .collect();
    assert_eq!(rejected, vec![(0x07, "fatal assert 0x58A".to_owned()); 2]);
    let stamina = {
        let s = fx.sim();
        s.events.with(&mut s.game, |_, v| v.stat(p, STAT_STAMINA))
    };
    assert_eq!(stamina, STAMINA - 56 * 2 * RUN_DRAIN);
    let errors = fx.errors();
    assert_eq!(errors, Vec::<String>::new());
    let d = fx.path();
    Transcript {
        walks,
        travel,
        end: ((d.x(), d.y()), fx.room(), fx.mode(), stamina),
        unowned,
        errors,
    }
}

// Covers: specs/sim/pathing.md §1.1, §9.6 r9, §9.9 r3, §10 r2; specs/world/waypoints.md §7 r5, §7 r7
#[test]
fn walk_across_the_level_and_take_the_waypoint() {
    run();
}

#[test]
fn same_run_same_transcript() {
    assert_eq!(run(), run());
}
