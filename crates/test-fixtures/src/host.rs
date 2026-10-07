// Spec: specs/sim/intents-events.md §1–§3; specs/sim/tick.md §3; specs/drlg/levels.md §3–§5; specs/drlg/rooms.md §4.1; specs/combat/vitals.md §1; specs/sim/pathing.md §1, §9; specs/world/waypoints.md §2, §7 (a joined single-player host over a GameData)
//! The wired single-player host over a [`GameData`]: the session the
//! live game-file test (`d2-server/tests/game_wired_host.rs`) and the
//! synthetic Act I e2e (`tests/act1_game.rs`) both drive.
//!
//! [`Session::new`]: a `WorldSim` with act 0 created
//! ([`GameData::world_sim`]), the town generated, the room of its
//! waypoint (the first town preset object whose `objects` row has
//! operate function 23, `waypoints.md` §7 rule 1) streamed, the
//! waypoint object and a player of the class allocated there (creation
//! stats, `vitals.md` §1), the client joined, the first frame run. Then
//! frames, messages through the host, walk legs on the sim's walk
//! handler, and the level of a unit.
//!
//! Nothing here adds a rule: the seams without a provider keep the
//! sim's defaults ([`Seams`]); the start offset, leg length and budgets
//! are fixture choices (no spec places a joining player, `levels.md`
//! §10 is not wired at join).

use d2_data::tables::{Levels, Objects};
use d2_server::adapters::handlers::world::ActionWorld;
use d2_server::adapters::{PlayerData, PlayerFields, ProtoSizes, SimGame, UnitFacts};
use d2_server::dispatch::Outcome as Dispatched;
use d2_server::host::{Handled, Host};
use d2_server::seams::{ClientId, Clock, MessageSink, PlayerGate, Pos, ResultCode, SessionHandler};
use d2_sim::combat::vitals::init_player_stats;
use d2_sim::drlg::preset::Ds1File;
use d2_sim::drlg::{Drlg, DrlgRoomId, TileRect};
use d2_sim::game::Game;
use d2_sim::stats::lists::NoHost;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::lists::client_state;
use d2_sim::units::{RoomId, UnitId, UnitType};
use d2_sim::wiring::interaction::{VitalsRest, VitalsView};
use d2_sim::wiring::worldgen::levels::SharedTypes;

use crate::game::{ActCreation, GameData, Seams, Sim};

/// The waypoint operate function (`waypoints.md` Constants).
pub const WAYPOINT_OPERATE: u8 = 23;
/// Sub-tiles per tile (`rooms.md` §9.2).
pub const SUB: i32 = 5;
/// Player modes the per-tick movement runs in (`pathing.md` §9.2: 2, 3,
/// 6, 19).
pub const MOVING: [u32; 4] = [2, 3, 6, 19];
/// The client of the session.
pub const CLIENT: ClientId = 0;

/// Fixture choices: the player's offset from the town waypoint (the
/// first that stays inside the waypoint's room), the walk's leg length
/// (below the 50 sub-tile target range and the 100 sub-tile limit of
/// `pathing.md` edge case 10), the frame and leg budgets. The frame
/// budget covers a leg at the slowest run velocity (`pathing.md` §8.1:
/// p at least 25, i.e. `WalkVelocity` · 64; the synthetic set has no
/// stat 67, so its run is that slow: about 11 frames per sub-tile).
pub const START_OFFSETS: [(i32, i32); 4] = [(5, 5), (-5, 5), (5, -5), (-5, -5)];
pub const LEG: i32 = 40;
pub const LEG_FRAMES: usize = 1000;
pub const MAX_LEGS: usize = 200;
pub const STUCK_LEGS: usize = 3;

pub struct NoRest;
impl VitalsRest for NoRest {
    fn refresh(&mut self, _: UnitId) {}
    fn level_up_notify(&mut self, _: UnitId) {}
    fn level_up_event(&mut self, _: UnitId) {}
}

#[derive(Default)]
pub struct NoSession;
impl SessionHandler for NoSession {
    fn system_message(&mut self, _: ClientId, _: &[u8], _: usize, _: &mut dyn MessageSink) {}
}

/// A clock the session advances by hand (40 ms per frame).
pub struct Ms(pub u32);
impl Clock for Ms {
    fn now_ms(&mut self) -> u32 {
        self.0
    }
}

pub type TestHost = Host<Sim, ProtoSizes, NoSession, Ms>;

/// How the session's game is made.
#[derive(Clone, Debug)]
pub struct Setup {
    pub creation: ActCreation,
    /// The DRLG init seed (`levels.md` §3 step 2).
    pub init_seed: u32,
    /// The town level (`levels.md` §3 step 8).
    pub town: u32,
    pub game_seed: u32,
    /// The player's class (charstats row).
    pub class: u32,
    /// Levels whose waypoint the player knows besides the town (staged:
    /// a new record knows only the town, `waypoints.md` §2).
    pub known_waypoints: Vec<u32>,
}

/// The first town preset unit of type 2 whose `objects` row has operate
/// function 23 (`waypoints.md` §7 rule 1): the DRLG room it lies in, its
/// absolute sub-tile position and the object class. A unit is on its
/// room's list (room-relative) once the room's tiles are built
/// (`preset.md` §9), on its map's list (absolute) once the map's DS1 is
/// loaded and filtered at the first activation (§7–§8), and before that
/// only in the map's picked DS1 file (file-relative, read here through
/// `Ds1File::from_input` without touching the game). All three are
/// searched, in that order.
pub fn town_waypoint(
    gd: &GameData,
    d: &Drlg,
    types: &SharedTypes,
    objects: &[Objects],
    town: u32,
) -> (DrlgRoomId, (i32, i32), u32) {
    let lv = d.find_level(town).expect("town allocated");
    let t = types.borrow();
    let presets = t.act_presets(0).expect("act 0 presets");
    let is_wp = |unit_type: u32, class: i32| {
        unit_type == 2
            && usize::try_from(class)
                .ok()
                .and_then(|c| objects.get(c))
                .is_some_and(|o| o.operatefn == WAYPOINT_OPERATE)
    };
    let rooms = d.level_rooms(lv);
    for &r in &rooms {
        let rect = d.room(r).rect;
        for u in presets.room_units(r) {
            if is_wp(u.unit_type, u.class) {
                return (r, (rect.x * SUB + u.x, rect.y * SUB + u.y), u.class as u32);
            }
        }
    }
    let room_at = |x: i32, y: i32| {
        rooms.iter().copied().find(|&r| {
            let t = d.room(r).rect;
            (t.x * SUB..(t.x + t.w) * SUB).contains(&x)
                && (t.y * SUB..(t.y + t.h) * SUB).contains(&y)
        })
    };
    for &m in presets.level_maps(lv) {
        let map = presets.map(m).expect("live map");
        let units: Vec<(u32, i32, i32, i32)> = if map.ds1.is_some() {
            map.units
                .iter()
                .map(|u| (u.unit_type, u.class, u.x, u.y))
                .collect()
        } else {
            let pd = &gd.level.preset;
            let path = usize::try_from(map.picked_file)
                .ok()
                .and_then(|f| pd.def(map.def).ok()?.file.get(f))
                .expect("the map's picked file");
            let input = gd.files.ds1.0.get(path).expect("the picked DS1 is loaded");
            let file = Ds1File::from_input(input, pd).expect("the picked DS1 builds");
            let (sx, sy) = (map.rect.x * SUB, map.rect.y * SUB);
            file.units
                .iter()
                .map(|u| (u.unit_type, u.class, u.x + sx, u.y + sy))
                .collect()
        };
        for (ty, class, x, y) in units {
            if let Some(r) = room_at(x, y).filter(|_| is_wp(ty, class)) {
                return (r, (x, y), class as u32);
            }
        }
    }
    panic!("no town preset object with operate function {WAYPOINT_OPERATE}");
}

/// After the room's tile build: the waypoint unit is on the room's
/// preset-unit list at the room-relative position (`preset.md` §9).
fn assert_transferred(
    types: &SharedTypes,
    room: DrlgRoomId,
    rect: d2_sim::drlg::TileRect,
    (x, y): (i32, i32),
    class: u32,
) {
    let t = types.borrow();
    let presets = t.act_presets(0).expect("act 0 presets");
    let units: Vec<(u32, i32, i32, i32)> = presets
        .room_units(room)
        .iter()
        .map(|u| (u.unit_type, u.class, u.x, u.y))
        .collect();
    let (rx, ry) = (x - rect.x * SUB, y - rect.y * SUB);
    assert!(
        units.contains(&(2, class as i32, rx, ry)),
        "waypoint {class} at ({rx}, {ry}) not on the streamed room's list {units:?}"
    );
}

/// A joined session.
pub struct Session {
    pub host: TestHost,
    pub player: UnitId,
    pub player_guid: u32,
    pub wp_guid: u32,
    pub wp_at: (i32, i32),
    pub start: (i32, i32),
    /// Every message the client received, in order.
    pub transcript: Vec<Vec<u8>>,
    /// Steps skipped or stopped, for the report.
    pub notes: Vec<String>,
}

impl Session {
    /// Game creation, the town waypoint and the player, the client
    /// joined (the first frame, no tick, has run).
    pub fn new(d: &GameData, setup: &Setup) -> Self {
        let (mut sim, types) = d
            .world_sim(
                setup.creation,
                setup.init_seed,
                setup.town,
                setup.game_seed,
                Seams::default(),
            )
            .unwrap_or_else(|e| panic!("game creation: {e}"));
        let vitals = d.vitals().expect("vitals tables");
        let levels: Vec<Levels> = d.rows().expect("levels");
        let objects: Vec<Objects> = d.rows().expect("objects");
        let town = setup.town;

        // The town generated (`levels.md` §3 step 8), its waypoint room
        // streamed.
        let mut game = Game::new();
        game.lists.ensure_act(0).unwrap();
        let (room_id, room, rect, wp_at, wp_class) = sim
            .action
            .hooks()
            .drlg
            .with_act(0, &mut game.lists, |dr, svc| {
                let lv = dr.get_or_alloc_level(svc.data, svc.types, town)?;
                if dr.level_rooms(lv).is_empty() {
                    dr.generate_level(svc.data, svc.types, lv)?;
                }
                let (r, at, c) = town_waypoint(d, dr, &types, &objects, town);
                let rect = dr.room(r).rect;
                Ok::<_, d2_sim::drlg::DrlgError>((r, dr.stream_room(svc, r)?, rect, at, c))
            })
            .expect("act 0 has a DRLG")
            .expect("town generated and streamed");
        let room = room.expect("the waypoint room is active");
        assert_transferred(&types, room_id, rect, wp_at, wp_class);
        assert_eq!(sim.errors(), Vec::<String>::new(), "game creation");

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
        let class = setup.class;
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
        // The creation life and mana (`vitals.md` §1: (vit + hpadd) << 8,
        // int << 8), from the class's row.
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
        for &level in &setup.known_waypoints {
            let index = levels[level as usize].waypoint;
            sim.action
                .hooks()
                .waypoints
                .entry(player)
                .or_default()
                .get_mut(0)
                .set(index.into())
                .unwrap_or_else(|e| panic!("waypoint index of level {level}: {e:?}"));
        }
        let wp_guid = game.lists.unit(object).unwrap().guid;
        let player_guid = game.lists.unit(player).unwrap().guid;

        let world = ActionWorld {
            waypoints: Some(d.waypoints().expect("waypoint tables")),
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
        let mut fx = Self {
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

    pub fn sim(&mut self) -> &mut Sim {
        &mut self.host.game
    }

    /// The player's position (its path, `path-placement.md` §2.1).
    pub fn pos(&mut self) -> (i32, i32) {
        let p = self.player;
        self.sim().events.action.hooks().path_position(p)
    }

    pub fn mode(&mut self) -> u32 {
        let p = self.player;
        self.sim().events.action.sys.units.get(p).unwrap().mode
    }

    /// The level id of a unit's room.
    pub fn unit_level(&mut self, unit: UnitId) -> Option<u32> {
        let room = self.sim().game.lists.unit(unit)?.room()?;
        self.room_level(room)
    }

    pub fn room_level(&mut self, room: RoomId) -> Option<u32> {
        let d = self.sim().events.action.hooks().drlg.dungeon.acts[0].as_ref()?;
        let r = d.drlg_room_of(room)?;
        Some(d.level(d.room(r).level).id)
    }

    /// The facts the host's unit-target and range checks read
    /// (`UnitFacts`, staged by the caller) follow the player's path.
    pub fn sync_facts(&mut self) {
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
    pub fn frame(&mut self) -> (Vec<(u8, ResultCode)>, Vec<Vec<u8>>) {
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
    pub fn send(&mut self, msg: &[u8]) -> (ResultCode, Vec<Vec<u8>>) {
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
    pub fn assert_stub(&mut self, msg: &[u8]) {
        let (code, _) = self.send(msg);
        assert_eq!(code, ResultCode::Done, "stub result 0");
        let last = self.sim().unhandled.last().copied();
        assert_eq!(last, Some((CLIENT, msg[0], msg.len())), "stub record");
    }

    /// No fault anywhere: tick faults, world faults, wiring errors.
    pub fn assert_clean(&mut self, when: &str) {
        let s = self.sim();
        assert!(s.tick_faults.is_empty(), "{when}: {:?}", s.tick_faults);
        assert!(s.world.faults.is_empty(), "{when}: {:?}", s.world.faults);
        assert_eq!(s.events.errors(), Vec::<String>::new(), "{when}");
    }

    /// A walk leg: C→S 0x03 through the host, handled by the server's
    /// walk handler (`d2_server::adapters::handlers::walk`: the sim's
    /// `walk_message`, `pathing.md` §1.1; result 0, rule 1), then frames
    /// until the player stops.
    pub fn leg(&mut self, (x, y): (i32, i32)) {
        let mut m = vec![0x03];
        m.extend_from_slice(&(x as u16).to_le_bytes());
        m.extend_from_slice(&(y as u16).to_le_bytes());
        let unhandled = self.sim().unhandled.len();
        let (code, _) = self.send(&m);
        assert_eq!(
            code,
            ResultCode::Done,
            "0x03 result 0 (pathing.md §10 rule 1)"
        );
        assert_eq!(
            self.sim().unhandled.len(),
            unhandled,
            "0x03 handled, not the stub"
        );
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
    pub fn walk(&mut self, goals: &[(i32, i32)], done: impl Fn(&mut Self) -> bool, what: &str) {
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

    /// The tile rect of an allocated level.
    pub fn level_rect(&mut self, id: u32) -> TileRect {
        let d = self.sim().events.action.hooks().drlg.dungeon.acts[0]
            .as_ref()
            .unwrap();
        let l = d.find_level(id).expect("level allocated at act creation");
        d.level(l).rect
    }
}

/// Chebyshev distance.
pub fn cheb(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// Goal points two tiles inside `to`, along the edge it shares with
/// `from`, every two tiles, nearest to `p` first (sub-tiles).
pub fn border_goals(from: TileRect, to: TileRect, p: (i32, i32)) -> Vec<(i32, i32)> {
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

/// FNV-1a, 64 bits.
pub fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}
