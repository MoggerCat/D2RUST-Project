// Spec: specs/sim/intents-events.md §1–§3; specs/sim/tick.md §3; specs/drlg/levels.md §3–§5; specs/drlg/rooms.md §4.1; specs/drlg/outdoor.md (Test vectors); specs/combat/vitals.md §1; specs/sim/pathing.md §1, §9; specs/world/waypoints.md §6, §7, §8 (game-file checks of the wired single-player host)
//! The whole wired single-player host on the live 1.14d tables and MPQs
//! (`#[ignore]`, `D2_GAME_DIR`), once per class of `charstats`:
//!
//! 1. game creation: act 0 through `WorldTypes` (the `d2_server::world_data`
//!    providers) on the recorded init seed, `WorldSim` with the population
//!    and init tables (population on: the room pass runs; room population
//!    `population.md` §3 has no coordinate-list provider, so only presets
//!    place units), the path provider on, `ActionWorld` with the live
//!    waypoint tables; the town generated, its waypoint room streamed;
//! 2. the town waypoint object (the preset unit of the town whose
//!    `objects` row has operate function 23) and the player of the class
//!    (`vitals.md` §1 creation stats) allocated there; the client joins;
//! 3. 500 host frames (one tick each) with no fault;
//! 4. the walk to the town's Blood Moor exit: the target is computed from
//!    the generated level rects (the shared edge of the Rogue Encampment
//!    and the Blood Moor), each leg is a C→S 0x03 through the host (no
//!    server handler owns 0x01–0x04 yet: asserted stub) and the same
//!    request on the sim's walk handler (`wiring::path::walk::walk_message`,
//!    `pathing.md` §1.1), whose path the wiring computes; ticks until the
//!    player stops; repeated until the player stands in a Blood Moor room;
//! 5. the kill: if a monster stands within the target range, the class's
//!    `StartSkill` is selected (0x3C) and cast on it (0x0D) through the
//!    host; the live host has no skill-use provider (`SkillRest` exists
//!    only as the synthetic e2e fixture), so both stop at the asserted
//!    stub; without a monster the step is skipped and says so;
//! 6. the pick-up (0x16) of a ground item in reach: the one-item-store
//!    change (unify-items) has not landed and the live host has no
//!    inventory parts, so it stops at the asserted stub (skipped without
//!    an item);
//! 7. back to the town waypoint, then C→S 0x49 to the Cold Plains
//!    (`waypoints.md` §6–§7): the player in a Cold Plains room and S→C
//!    0x0D with the bytes §7 rule 7 gives (type 0, GUID, 1, x + 3, y + 3,
//!    0, 0), after a 0x07 (§8 rule 3);
//! 8. a digest of the state (units, positions, stats, seeds, DRLG room
//!    counts, the client's transcript) printed per class; the whole run
//!    is done twice in the test and the digests must be equal
//!    (determinism on real data); two local runs compare the printed
//!    digests.
//!
//! Expected values: only the ones a spec states (creation stats, Blood
//! Moor / Cold Plains room allocations, the 0x0D bytes and order, the
//! 0x49 range); everything else is an invariant (no fault, no wiring
//! error, positions inside their rooms, active rooms within their level,
//! determinism). The walk's success (reaching Blood Moor, back to the
//! waypoint) is the task's goal, not a spec value: a failure prints the
//! position and is a wiring finding (`docs/handoff/game-tests-wired-host.md`).
//! **Unconfirmed** until the first local run (`docs/HANDOFF.md` §8): no
//! `Covers:` claim.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use d2_data::bin::{self, BinSet};
use d2_data::fixup::{self, FixedSet};
use d2_data::tables::{
    decode_all, Difficultylevels, Levels, Missiles, Monequip, Monlvl, Monprop, Monstats, Monstats2,
    Monumod, Objects, Record, Superuniques,
};
use d2_formats::animdata::AnimData;
use d2_formats::mpq::ArchiveSet;
use d2_server::adapters::handlers::world::{ActionWorld, Outbox};
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::dispatch::Outcome as Dispatched;
use d2_server::host::{Handled, Host};
use d2_server::seams::{ClientId, Clock, MessageSink, PlayerGate, Pos, ResultCode, SessionHandler};
use d2_server::world_data::tables::LevelTables;
use d2_server::world_data::{archive, WorldFiles};
use d2_sim::combat::vitals::{init_player_stats, VitalsTables};
use d2_sim::combat::CombatTables;
use d2_sim::drlg::maze::Maze;
use d2_sim::drlg::{Drlg, DrlgData, DrlgRoomId, Dungeon, TileRect};
use d2_sim::game::Game;
use d2_sim::monsters::ai::skill_modes;
use d2_sim::monsters::init::{component_counts, monstats_extra, GameInfo, NamedIds};
use d2_sim::monsters::population::PopTables;
use d2_sim::path::CollisionRooms;
use d2_sim::rng::Seed;
use d2_sim::skills::{SkillTables, LEVEL_CAP_114D};
use d2_sim::stats::lists::NoHost;
use d2_sim::stats::{StatData, StateTable};
use d2_sim::units::hooks::UnitData;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{RoomId, UnitId, UnitType};
use d2_sim::wiring::action::{ActionHooks, ActionTables, DrlgWorld, Pending};
use d2_sim::wiring::interaction::{VitalsRest, VitalsView};
use d2_sim::wiring::path::walk::walk_message;
use d2_sim::wiring::worldgen::dispatch::WorldSim;
use d2_sim::wiring::worldgen::levels::{SharedTypes, WorldTypes};
use d2_sim::wiring::worldgen::{WorldPending, WorldState, WorldTables};
use d2_sim::world::waypoints::WaypointData;

// ---- live data ------------------------------------------------------------------------

struct Live {
    bin: BinSet,
    anim: AnimData,
    fixed: FixedSet,
    tables: LevelTables,
    files: WorldFiles,
}

fn live() -> &'static Live {
    static L: OnceLock<Live> = OnceLock::new();
    L.get_or_init(|| {
        let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
        let set = ArchiveSet::open_dir(dir).expect("archives open");
        let bin = bin::load(&set, bin::DEFAULT_LANGUAGE).expect("live .bin set loads");
        let anim = fixup::read_animdata(&set).expect("AnimData.d2 reads");
        let fixed = fixup::apply(&bin, &anim).expect("fix-ups apply");
        let tables = LevelTables::from_fixed(&fixed).expect("level tables");
        let files = WorldFiles::load(
            &tables.drlg,
            &tables.preset,
            &tables.outdoor,
            archive::reader(&set),
        )
        .expect("every DRLG file loads and parses");
        Live {
            bin,
            anim,
            fixed,
            tables,
            files,
        }
    })
}

fn table(name: &str) -> &'static d2_data::bin::BinTable {
    live()
        .fixed
        .table(name)
        .unwrap_or_else(|| panic!("table {name} in the live set"))
}

fn rows<T: Record>() -> Vec<T> {
    decode_all(table(T::TABLE)).unwrap_or_else(|e| panic!("{}: {e}", T::TABLE))
}

// ---- constants (each from the spec named) ----------------------------------------------

/// The recorded Act I creation (`outdoor.md` Test vectors): DRLG init seed.
const INIT: u32 = 644_409_375;
/// Rogue Encampment, the server's town level (`levels.md` §3 step 8).
const TOWN: u32 = 1;
/// Blood Moor and Cold Plains (`outdoor.md` Test vectors: L2, L3).
const BLOOD_MOOR: u32 = 2;
const COLD_PLAINS: u32 = 3;
/// Room allocations of one Blood Moor / Cold Plains build (`outdoor.md`
/// Test vectors, `0x0066B42E`).
const BLOOD_MOOR_ROOMS: usize = 81;
const COLD_PLAINS_ROOMS: usize = 98;
/// The waypoint operate function (`waypoints.md` Constants).
const WAYPOINT_OPERATE: u8 = 23;
/// 0x49 range in sub-tiles: 22 for the sorceress (class 1), else 10
/// (`waypoints.md` §6.2 step 3).
fn waypoint_range(class: u32) -> i32 {
    if class == 1 {
        22
    } else {
        10
    }
}
/// The point / unit target range of `intents-events.md` §2.4 rule 3
/// (`dispatch::in_range`, 50 sub-tiles).
const TARGET_RANGE: i32 = 50;
/// Sub-tiles per tile (`rooms.md` §9.2).
const SUB: i32 = 5;
/// Player modes the per-tick movement runs in (`pathing.md` §9.2: 2, 3,
/// 6, 19).
const MOVING: [u32; 4] = [2, 3, 6, 19];

/// Fixture choices (no spec places a joining player, `levels.md` §10 is
/// not wired at join): the game seed, the player's offset from the town
/// waypoint (the first that stays inside the waypoint's room), the
/// walk's leg length (below the 50 sub-tile target range and the 100
/// sub-tile limit of `pathing.md` edge case 10), the frame and leg
/// budgets.
const GAME_SEED: u32 = 1234;
const START_OFFSETS: [(i32, i32); 4] = [(5, 5), (-5, 5), (5, -5), (-5, -5)];
const LEG: i32 = 40;
const LEG_FRAMES: usize = 400;
const MAX_LEGS: usize = 200;
const STUCK_LEGS: usize = 3;
const IDLE_FRAMES: usize = 500;

const CLIENT: ClientId = 0;

// ---- seams ----------------------------------------------------------------------------

/// The action and world-generation seams the live host has no provider
/// for keep their defaults (`Pending`, `WorldPending`), except two
/// bookkeeping answers no rule decides: the interaction owner (the
/// player's interact unit, kept as set) and the transport (`send`
/// collected and handed to the host through [`Outbox`]).
#[derive(Default)]
struct Seams {
    interact: BTreeMap<UnitId, (u8, u32)>,
    sent: Vec<(UnitId, Vec<u8>)>,
}

impl Pending for Seams {
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

impl WorldPending for Seams {}

impl Outbox for Seams {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

struct NoRest;
impl VitalsRest for NoRest {
    fn refresh(&mut self, _: UnitId) {}
    fn level_up_notify(&mut self, _: UnitId) {}
    fn level_up_event(&mut self, _: UnitId) {}
}

#[derive(Default)]
struct NoSession;
impl SessionHandler for NoSession {
    fn system_message(&mut self, _: ClientId, _: &[u8], _: usize, _: &mut dyn MessageSink) {}
}

struct Ms(u32);
impl Clock for Ms {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

type Sim = SimGame<WorldSim<Seams>, ActionWorld>;
type TestHost = Host<Sim, ProtoSizes, NoSession, Ms>;

// ---- the fixture ----------------------------------------------------------------------

struct Fx {
    host: TestHost,
    player: UnitId,
    player_guid: u32,
    wp_guid: u32,
    wp_at: (i32, i32),
    start: (i32, i32),
    /// Every message client 0 received, in order.
    transcript: Vec<Vec<u8>>,
    /// Steps skipped or stopped, for the report.
    notes: Vec<String>,
}

fn level_types() -> (Arc<DrlgData>, SharedTypes) {
    let l = live();
    let data = Arc::new(l.tables.drlg.clone());
    let types = SharedTypes::new(WorldTypes::new(
        data.clone(),
        Maze::new(l.tables.maze.clone()),
        l.tables.preset.clone(),
        l.tables.outdoor.clone(),
        Box::new(l.files.ds1.clone()),
        Box::new(l.files.subs.clone()),
    ));
    (data, types)
}

/// The first town preset unit of type 2 whose `objects` row has operate
/// function 23 (`waypoints.md` §7 rule 1): (DRLG room, its room-relative
/// sub-tile x, y, the object class).
fn town_waypoint(
    d: &Drlg,
    types: &SharedTypes,
    objects: &[Objects],
) -> (DrlgRoomId, i32, i32, u32) {
    let lv = d.find_level(TOWN).expect("town allocated");
    let t = types.borrow();
    let presets = t.act_presets(0).expect("act 0 presets");
    for r in d.level_rooms(lv) {
        for u in presets.room_units(r) {
            let is_wp = u.unit_type == 2
                && usize::try_from(u.class)
                    .ok()
                    .and_then(|c| objects.get(c))
                    .is_some_and(|o| o.operatefn == WAYPOINT_OPERATE);
            if is_wp {
                return (r, u.x, u.y, u.class as u32);
            }
        }
    }
    panic!("no town preset object with operate function {WAYPOINT_OPERATE}");
}

impl Fx {
    /// Steps 1–2: game creation, the town waypoint and the player of
    /// `class`, the client joined (the first frame, no tick, has run).
    fn new(class: u32) -> Self {
        let l = live();
        let (data, types) = level_types();
        let mut handle = types.clone();
        let drlg = Drlg::create(0, INIT, 0, TOWN, false, &data, &mut handle).expect("act 0");
        let mut dungeon = Dungeon::default();
        dungeon.acts[0] = Some(drlg);
        let world = DrlgWorld {
            dungeon,
            data,
            tiles: Box::new(l.files.dt1.clone()),
            types: Box::new(handle),
        };
        let levels = rows::<Levels>();
        let tables = ActionTables {
            missiles: rows::<Missiles>(),
            skills: SkillTables::from_bin(&l.bin, LEVEL_CAP_114D).expect("skill tables"),
            combat: CombatTables::from_bin(&l.bin).expect("combat tables"),
            levels: levels.clone(),
            skill_modes: skill_modes(table("monstats")),
        };
        let vitals = Arc::new(VitalsTables::from_bin(&l.bin).expect("vitals tables"));
        let mut hooks = ActionHooks::new(
            Arc::new(tables),
            world,
            Seed::init_low(GAME_SEED),
            Seams::default(),
        );
        hooks.anim_data = Some(Arc::new(l.anim.clone()));
        hooks.vitals = Some(vitals.clone());
        hooks.enable_paths().expect("path tables");

        let (monstats, monstats2) = (rows::<Monstats>(), rows::<Monstats2>());
        let superuniques = rows::<Superuniques>();
        let wt = WorldTables {
            pop: PopTables::from_records(&levels, &monstats, &monstats2, &superuniques)
                .with_bins(table("monstats"), table("monstats2")),
            monstats,
            monstats2,
            monlvl: rows::<Monlvl>(),
            levels: levels.clone(),
            monprop: rows::<Monprop>(),
            monequip: rows::<Monequip>(),
            monumod: rows::<Monumod>(),
            superuniques,
            difficultylevels: rows::<Difficultylevels>(),
            monstats_extra: monstats_extra(table("monstats")),
            components: component_counts(table("monstats2")),
            ids: NamedIds::default(),
            montype_equiv: live().fixed.montype_equiv.clone(),
        };
        let info = GameInfo {
            expansion: true,
            ..GameInfo::default()
        };
        let state = WorldState::new(types.clone(), Arc::new(wt), info);
        let states = StateTable::new(table("states"), &l.fixed.states).expect("states");
        let stat_data = Arc::new(
            StatData::new(
                table("itemstatcost"),
                table("charstats"),
                states,
                table("monstats"),
                table("skills"),
            )
            .expect("stat data"),
        );
        let unit_data = UnitData {
            expansion: true,
            ..UnitData::new(table("monstats"), table("monstats2")).expect("unit data")
        };
        let mut sim = WorldSim::new(stat_data, unit_data, hooks, state);
        sim.create_regions();

        // The town generated (`levels.md` §3 step 8), its waypoint room
        // streamed.
        let objects = rows::<Objects>();
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let (room, rect, wx, wy, wp_class) = sim
            .action
            .hooks()
            .drlg
            .with_act(0, &mut game.lists, |d, svc| {
                let lv = d.get_or_alloc_level(svc.data, svc.types, TOWN)?;
                if d.level_rooms(lv).is_empty() {
                    d.generate_level(svc.data, svc.types, lv)?;
                }
                let (r, x, y, c) = town_waypoint(d, &types, &objects);
                let rect = d.room(r).rect;
                Ok::<_, d2_sim::drlg::DrlgError>((d.stream_room(svc, r)?, rect, x, y, c))
            })
            .expect("act 0 has a DRLG")
            .expect("town generated and streamed");
        let room = room.expect("the waypoint room is active");
        assert_eq!(sim.errors(), Vec::<String>::new(), "game creation");

        // Room-relative preset sub-tiles + the room's sub-tile origin
        // (`population.md` §9: x = preset x + room sub-tile x).
        let wp_at = (rect.x * SUB + wx, rect.y * SUB + wy);
        let inside = |(x, y): (i32, i32)| {
            x >= rect.x * SUB
                && x < (rect.x + rect.w) * SUB
                && y >= rect.y * SUB
                && y < (rect.y + rect.h) * SUB
        };
        let start = START_OFFSETS
            .iter()
            .map(|&(dx, dy)| (wp_at.0 + dx, wp_at.1 + dy))
            .find(|&p| inside(p))
            .expect("a start point inside the waypoint's room");
        let mut alloc = |ty, class, (x, y): (i32, i32)| {
            let req = AllocRequest {
                ty,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: ty == UnitType::Player,
            };
            sim.action
                .with(&mut game, |g, v| v.allocate(g, &req, x, y))
                .expect("allocated")
        };
        let object = alloc(UnitType::Object, wp_class, wp_at);
        let player = alloc(UnitType::Player, class, start);
        // Players are allocated in mode 0; neutral (`units.md` §2).
        sim.action.sys.units.get_mut(player).unwrap().mode = 1;
        {
            let s = &mut sim.action.sys;
            let mut v = VitalsView {
                units: &s.units,
                stats: &mut s.stats,
                hooks: &mut NoHost,
                rest: &mut NoRest,
            };
            init_player_stats(&mut v, &vitals, player, 0);
        }
        // The creation life and mana of every class (`vitals.md` §1:
        // (vit + hpadd) << 8, int << 8), from the live row.
        let cs = vitals.charstats(class as i32).expect("charstats row");
        let life = (i32::from(cs.vit) + i32::from(cs.hpadd)) << 8;
        let mana = i32::from(cs.int) << 8;
        for (stat, want) in [(6, life), (7, life), (8, mana), (9, mana)] {
            assert_eq!(
                sim.action.sys.stats.unit_total(player, stat, 0),
                want,
                "stat {stat} at creation (vitals.md §1)"
            );
        }
        // The Cold Plains waypoint known (staged: a new record knows only
        // the town, `waypoints.md` §2; no spec activates another here).
        let cp_index = levels[COLD_PLAINS as usize].waypoint;
        sim.action
            .hooks()
            .waypoints
            .entry(player)
            .or_default()
            .get_mut(0)
            .set(cp_index.into())
            .expect("Cold Plains waypoint index");
        let wp_guid = game.lists.unit(object).unwrap().guid;
        let player_guid = game.lists.unit(player).unwrap().guid;

        let world = ActionWorld {
            waypoints: Some(WaypointData::new(&levels, &objects)),
            ..ActionWorld::default()
        };
        let mut s: Sim = SimGame::with_world(game, sim, world);
        s.join(CLIENT, Some(player), None, client_state::IN_GAME)
            .expect("join");
        s.set_player(
            player,
            PlayerFields {
                gate: PlayerGate {
                    mode: 1,
                    uninterruptable: false,
                },
                data: Some(PlayerData { last_accept: 0 }),
            },
        );
        s.set_unit(
            object,
            UnitFacts {
                act: 0,
                pos: Pos {
                    x: wp_at.0,
                    y: wp_at.1,
                },
                owner: None,
            },
        );
        let mut host = Host::new(s, ProtoSizes, NoSession, Ms(1000));
        host.connect(CLIENT);
        host.frame().expect("first frame");
        let mut fx = Fx {
            host,
            player,
            player_guid,
            wp_guid,
            wp_at,
            start,
            transcript: Vec::new(),
            notes: Vec::new(),
        };
        // The position the allocation gave (the path placement may move
        // it off the requested point).
        fx.start = fx.pos();
        fx.sync_facts();
        fx
    }

    fn sim(&mut self) -> &mut Sim {
        &mut self.host.game
    }

    /// The player's position (its path, `path-placement.md` §2.1).
    fn pos(&mut self) -> (i32, i32) {
        let p = self.player;
        self.sim().events.action.hooks().path_position(p)
    }

    fn mode(&mut self) -> u32 {
        let p = self.player;
        self.sim().events.action.sys.units.get(p).unwrap().mode
    }

    /// The level id of a unit's room.
    fn unit_level(&mut self, unit: UnitId) -> Option<u32> {
        let room = self.sim().game.lists.unit(unit)?.room()?;
        self.room_level(room)
    }

    fn room_level(&mut self, room: RoomId) -> Option<u32> {
        let d = self.sim().events.action.hooks().drlg.dungeon.acts[0].as_ref()?;
        let r = d.drlg_room_of(room)?;
        Some(d.level(d.room(r).level).id)
    }

    /// The facts the host's unit-target and range checks read
    /// (`UnitFacts`, staged by the caller) follow the player's path.
    fn sync_facts(&mut self) {
        let (x, y) = self.pos();
        let p = self.player;
        self.sim().set_unit(
            p,
            UnitFacts {
                act: 0,
                pos: Pos { x, y },
                owner: None,
            },
        );
    }

    /// One host frame (drain → one tick → flush); what the client
    /// received goes to the transcript and is returned.
    fn frame(&mut self) -> (Vec<(u8, ResultCode)>, Vec<Vec<u8>>) {
        self.host.clock.0 += 40;
        let r = self.host.frame().expect("frame");
        assert!(r.ticked, "one tick per frame");
        let codes = r
            .messages
            .iter()
            .map(|m| match m.handled {
                Handled::Game(Dispatched::Dispatched(c)) => (m.id, c),
                ref h => panic!("message {:#04x} not dispatched: {h:?}", m.id),
            })
            .collect();
        let got = self.host.receive(CLIENT);
        self.transcript.extend(got.iter().cloned());
        self.sync_facts();
        (codes, got)
    }

    /// One message through a host frame: its result code and what the
    /// client received in that frame. A repeat the client's duplicate
    /// filter drops (`intents-events.md` §2.1 rule 1: 200 ms for 0x03)
    /// is sent again after one more frame, as a client would.
    fn send(&mut self, msg: &[u8]) -> (ResultCode, Vec<Vec<u8>>) {
        let mut tries = 0;
        while self.host.send_game(CLIENT, msg).expect("send").is_none() {
            tries += 1;
            assert!(tries <= 6, "dropped by the duplicate filter (§2.1)");
            self.frame();
        }
        let (codes, got) = self.frame();
        assert_eq!(codes.len(), 1, "{codes:?}");
        assert_eq!(codes[0].0, msg[0]);
        (codes[0].1, got)
    }

    /// The message reached the stub (`intents-events.md` §2.4 rule 2:
    /// recorded, result 0).
    fn assert_stub(&mut self, msg: &[u8]) {
        let (code, _) = self.send(msg);
        assert_eq!(code, ResultCode::Done, "stub result 0");
        let last = self.sim().unhandled.last().copied();
        assert_eq!(last, Some((CLIENT, msg[0], msg.len())), "stub record");
    }

    /// No fault anywhere: tick faults, world faults, wiring errors.
    fn assert_clean(&mut self, when: &str) {
        let s = self.sim();
        assert!(s.tick_faults.is_empty(), "{when}: {:?}", s.tick_faults);
        assert!(s.world.faults.is_empty(), "{when}: {:?}", s.world.faults);
        assert_eq!(s.events.errors(), Vec::<String>::new(), "{when}");
    }

    /// Step 4 leg: C→S 0x03 through the host (asserted stub), the same
    /// request on the sim's walk handler, frames until the player stops.
    fn leg(&mut self, (x, y): (i32, i32)) {
        let mut m = vec![0x03];
        m.extend_from_slice(&(x as u16).to_le_bytes());
        m.extend_from_slice(&(y as u16).to_le_bytes());
        self.assert_stub(&m);
        let p = self.player;
        let s = self.sim();
        s.events.action.with(&mut s.game, |g, v| {
            walk_message(v, g, p, 0x03, x as u32, y as u32)
        });
        for _ in 0..LEG_FRAMES {
            if !MOVING.contains(&self.mode()) {
                break;
            }
            self.frame();
        }
        assert!(
            !MOVING.contains(&self.mode()),
            "the player stopped within {LEG_FRAMES} frames"
        );
        self.assert_clean("walk leg");
    }

    /// Legs toward each goal in turn until `done`; a goal with no
    /// progress for [`STUCK_LEGS`] legs gives way to the next.
    fn walk(&mut self, goals: &[(i32, i32)], done: impl Fn(&mut Self) -> bool, what: &str) {
        let mut legs = 0;
        for &g in goals {
            let mut stuck = 0;
            while stuck < STUCK_LEGS && legs < MAX_LEGS {
                if done(self) {
                    return;
                }
                let p = self.pos();
                let before = cheb(p, g);
                let t = (
                    p.0 + (g.0 - p.0).clamp(-LEG, LEG),
                    p.1 + (g.1 - p.1).clamp(-LEG, LEG),
                );
                self.leg(t);
                legs += 1;
                let after = cheb(self.pos(), g);
                stuck = if after < before { 0 } else { stuck + 1 };
            }
        }
        assert!(
            done(self),
            "{what}: not reached after {legs} legs; player at {:?} in level {:?}",
            self.pos(),
            self.unit_level(self.player)
        );
    }

    fn level_rect(&mut self, id: u32) -> TileRect {
        let d = self.sim().events.action.hooks().drlg.dungeon.acts[0]
            .as_ref()
            .unwrap();
        let l = d.find_level(id).expect("level allocated at act creation");
        d.level(l).rect
    }
}

fn cheb(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Goal points two tiles inside `to`, along the edge it shares with
/// `from`, every two tiles, nearest to `p` first (sub-tiles).
fn border_goals(from: TileRect, to: TileRect, p: (i32, i32)) -> Vec<(i32, i32)> {
    let overlap = |a0: i32, a1: i32, b0: i32, b1: i32| (a0.max(b0), a1.min(b1));
    let mut out = Vec::new();
    let inset = 2;
    let (fx1, fy1, tx1, ty1) = (from.x + from.w, from.y + from.h, to.x + to.w, to.y + to.h);
    if tx1 == from.x || fx1 == to.x {
        let x = if tx1 == from.x {
            tx1 - inset
        } else {
            to.x + inset
        };
        let (y0, y1) = overlap(from.y, fy1, to.y, ty1);
        out.extend((y0 + 1..y1).step_by(2).map(|y| (x * SUB, y * SUB)));
    } else if ty1 == from.y || fy1 == to.y {
        let y = if ty1 == from.y {
            ty1 - inset
        } else {
            to.y + inset
        };
        let (x0, x1) = overlap(from.x, fx1, to.x, tx1);
        out.extend((x0 + 1..x1).step_by(2).map(|x| (x * SUB, y * SUB)));
    }
    assert!(
        !out.is_empty(),
        "levels share no edge: {from:?} / {to:?} (outdoor.md placement)"
    );
    out.sort_by_key(|&g| cheb(g, p));
    out
}

// ---- the run -----------------------------------------------------------------------

/// FNV-1a, 64 bits.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

struct Run {
    digest: u64,
    state: String,
    notes: Vec<String>,
}

/// Steps 3–8 for one class.
fn run(class: u32) -> Run {
    let mut fx = Fx::new(class);
    let player = fx.player;

    // 3. 500 ticks with no fault; the first tick's room change gives the
    // client a room (`rooms.md` §4.1).
    let frame0 = fx.sim().game.frame;
    for _ in 0..IDLE_FRAMES {
        fx.frame();
    }
    assert_eq!(fx.sim().game.frame, frame0 + 500, "500 ticks");
    fx.assert_clean("500 ticks");
    assert!(fx.sim().game.lists.unit(player).is_some(), "player in game");
    let sc = fx.sim().sim_client(CLIENT).expect("client joined");
    assert!(
        fx.sim()
            .game
            .lists
            .client(sc)
            .and_then(|c| c.room)
            .is_some(),
        "the client has a room (rooms.md §4.1)"
    );
    assert_eq!(fx.pos(), fx.start, "standing still for 500 ticks");

    // 4. The town's Blood Moor exit.
    let town = fx.level_rect(TOWN);
    let moor = fx.level_rect(BLOOD_MOOR);
    let p = fx.pos();
    let goals = border_goals(town, moor, p);
    fx.walk(
        &goals,
        |f| f.unit_level(f.player) == Some(BLOOD_MOOR),
        "Blood Moor",
    );
    println!(
        "class {class}: in Blood Moor at {:?}, frame {}",
        fx.pos(),
        fx.sim().game.frame
    );

    // 5. The kill: a monster within the target range, if any.
    let me = fx.pos();
    let mut target = None;
    for m in fx.sim().game.lists.units_of_type(UnitType::Monster) {
        let at = fx.sim().events.action.hooks().path_position(m);
        let alive = !fx.sim().events.action.sys.units.is_dead(m);
        if alive && cheb(at, me) <= TARGET_RANGE && fx.unit_level(m) != Some(TOWN) {
            target = Some((m, at));
            break;
        }
    }
    match target {
        Some((m, at)) => {
            let guid = fx.sim().game.lists.unit(m).unwrap().guid;
            fx.sim().set_unit(
                m,
                UnitFacts {
                    act: 0,
                    pos: Pos { x: at.0, y: at.1 },
                    owner: None,
                },
            );
            let skill = u32::from(
                live_vitals()
                    .charstats(class as i32)
                    .expect("charstats")
                    .startskill,
            );
            let mut select = vec![0x3C];
            select.extend_from_slice(&(skill & 0x7FFF_FFFF).to_le_bytes());
            select.extend_from_slice(&u32::MAX.to_le_bytes());
            fx.assert_stub(&select);
            let mut cast = vec![0x0D];
            cast.extend_from_slice(&1u32.to_le_bytes());
            cast.extend_from_slice(&guid.to_le_bytes());
            fx.assert_stub(&cast);
            let mclass = fx.sim().events.action.sys.units.get(m).unwrap().class;
            fx.notes.push(format!(
                "kill: monster class {mclass} at {at:?}; 0x3C / 0x0D stubs (no SkillRest on the live host)"
            ));
        }
        None => fx
            .notes
            .push("kill: no monster within range (room population §3 not provided)".into()),
    }

    // 6. The pick-up of a ground item in reach, if any.
    let me = fx.pos();
    let item = fx
        .sim()
        .game
        .lists
        .units_of_type(UnitType::Item)
        .into_iter()
        .find(|&i| {
            let at = fx.host.game.events.action.hooks().path_position(i);
            cheb(at, me) <= TARGET_RANGE
        });
    match item {
        Some(i) => {
            let guid = fx.sim().game.lists.unit(i).unwrap().guid;
            let mut pick = vec![0x16];
            pick.extend_from_slice(&4u32.to_le_bytes());
            pick.extend_from_slice(&guid.to_le_bytes());
            pick.extend_from_slice(&0u32.to_le_bytes());
            fx.assert_stub(&pick);
            fx.notes
                .push("pick-up: 0x16 stub (unify-items not landed, no inventory parts)".into());
        }
        None => fx.notes.push("pick-up: no ground item in reach".into()),
    }

    // 7. Back to the town waypoint, then 0x49 to the Cold Plains.
    let (wp_at, range, start) = (fx.wp_at, waypoint_range(class), fx.start);
    fx.walk(
        &[start],
        move |f| {
            let p = f.pos();
            (p.0 - wp_at.0).abs() <= range && (p.1 - wp_at.1).abs() <= range
        },
        "town waypoint",
    );
    let mut m = vec![0x49];
    m.extend_from_slice(&fx.wp_guid.to_le_bytes());
    m.extend_from_slice(&(COLD_PLAINS as u16).to_le_bytes());
    m.extend_from_slice(&[0, 0]);
    let (code, got) = fx.send(&m);
    assert_eq!(code, ResultCode::Done, "0x49 accepted (§6.2 step 9)");
    fx.assert_clean("waypoint");
    assert_eq!(
        fx.unit_level(player),
        Some(COLD_PLAINS),
        "travel ends in a Cold Plains room (§7 rule 6)"
    );
    // The 0x0D of §7 rule 7: the position after placement + 3. The
    // frame's tick runs after the drain; the player stands in mode 2 at
    // its own position (§7 rule 7), so the position is unchanged.
    let (x, y) = fx.pos();
    let mut want = vec![0x0D, 0x00];
    want.extend_from_slice(&fx.player_guid.to_le_bytes());
    want.push(1);
    want.extend_from_slice(&((x + 3) as u16).to_le_bytes());
    want.extend_from_slice(&((y + 3) as u16).to_le_bytes());
    want.extend_from_slice(&[0, 0]);
    let at_0d = got
        .iter()
        .position(|g| g.first() == Some(&0x0D))
        .unwrap_or_else(|| panic!("no 0x0D among {got:02x?}"));
    assert_eq!(got[at_0d], want, "0x0D bytes (§7 rule 7)");
    assert!(
        got[..at_0d].iter().any(|g| g.first() == Some(&0x07)),
        "0x07 before 0x0D (§8 rule 3): {got:02x?}"
    );

    // Invariants: room counts, positions inside rooms.
    let mut levels = Vec::new();
    {
        let d = fx.host.game.events.action.hooks().drlg.dungeon.acts[0]
            .as_ref()
            .unwrap();
        for l in d.level_list() {
            let rooms = d.level_rooms(l);
            if rooms.is_empty() {
                continue;
            }
            let active = rooms
                .iter()
                .filter(|&&r| d.active_room(r).is_some())
                .count();
            levels.push((d.level(l).id, rooms.len(), active));
        }
    }
    for &(id, n, active) in &levels {
        assert!(active <= n, "level {id}: {active} active of {n}");
        match id {
            BLOOD_MOOR => assert_eq!(n, BLOOD_MOOR_ROOMS, "outdoor.md room allocations"),
            COLD_PLAINS => assert_eq!(n, COLD_PLAINS_ROOMS, "outdoor.md room allocations"),
            _ => {}
        }
    }
    let mut units = Vec::new();
    for ty in UnitType::ALL {
        for u in fx.sim().game.lists.units_of_type(ty) {
            let room = fx.sim().game.lists.unit(u).and_then(|e| e.room());
            let h = fx.host.game.events.action.hooks();
            let at = h.path_position(u);
            if let Some(r) = room.filter(|_| h.path_has(u)) {
                if let Some(rect) = h.drlg.subtile_rect(r) {
                    assert!(
                        at.0 >= rect.x
                            && at.0 < rect.x + rect.w
                            && at.1 >= rect.y
                            && at.1 < rect.y + rect.h,
                        "{ty:?} {u:?} at {at:?} outside its room {rect:?}"
                    );
                }
            }
            let r = fx.host.game.events.action.sys.units.get(u).unwrap();
            units.push((ty, r.class, r.guid, r.mode, r.seed, at, room));
        }
    }
    let stats: Vec<i32> = (0..=15)
        .map(|s| {
            fx.host
                .game
                .events
                .action
                .sys
                .stats
                .unit_total(player, s, 0)
        })
        .collect();
    let seed = fx.host.game.events.action.hooks().game_seed;
    let state = format!(
        "frame {} seed {seed:?}\nlevels {levels:?}\nunits {units:?}\nstats {stats:?}\nunhandled {:?}\ntranscript {:?}",
        fx.host.game.game.frame, fx.host.game.unhandled, fx.transcript
    );
    Run {
        digest: fnv(state.as_bytes()),
        state,
        notes: fx.notes,
    }
}

fn live_vitals() -> &'static VitalsTables {
    static V: OnceLock<VitalsTables> = OnceLock::new();
    V.get_or_init(|| VitalsTables::from_bin(&live().bin).expect("vitals tables"))
}

/// The whole run twice: equal digests (same seed, same data → same state
/// and transcript); prints the digest for the run-to-run comparison.
fn twice(class: u32) {
    let a = run(class);
    let b = run(class);
    for n in &a.notes {
        println!("class {class}: {n}");
    }
    if a.digest != b.digest {
        let line = a
            .state
            .lines()
            .zip(b.state.lines())
            .position(|(x, y)| x != y);
        panic!("class {class}: two runs differ (first differing line {line:?})");
    }
    println!("class {class}: digest {:016x}", a.digest);
}

// ---- one test per class (charstats rows 0–6) ------------------------------------------

// Intended claim (unconfirmed until the first local run): none (an integration run; the spec-stated values it asserts are claimed by their own tests).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_amazon() {
    twice(0);
}

// Intended claim (unconfirmed until the first local run): none (integration run).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_sorceress() {
    twice(1);
}

// Intended claim (unconfirmed until the first local run): none (integration run).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_necromancer() {
    twice(2);
}

// Intended claim (unconfirmed until the first local run): none (integration run).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_paladin() {
    twice(3);
}

// Intended claim (unconfirmed until the first local run): none (integration run).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_barbarian() {
    twice(4);
}

// Intended claim (unconfirmed until the first local run): none (integration run).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_druid() {
    twice(5);
}

// Intended claim (unconfirmed until the first local run): none (integration run).
#[test]
#[ignore = "needs original game files in D2_GAME_DIR"]
fn wired_host_assassin() {
    twice(6);
}
