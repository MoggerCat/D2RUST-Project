// Spec: specs/skills/bodies.md (§6.9, §8.3, §8.21), specs/skills/use.md (§5)
//! Assassin skills on the play host (q-assassin), headless, on the
//! synthetic single-player game: C→S 0x0C (the right skill at a point)
//! → the start check → the cast mode → the do step. Test-local fills:
//! the synthetic game has no `skills`, `missiles`, `states` or pet
//! types; each test installs the rows it needs.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link, DEFAULT_SEED, PLAYER_CLASS};
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_client::bridge::LOCAL_CLIENT;
use d2_client::rules::unit_composite::code;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_data::bin::BinTable;
use d2_data::fixup::maps;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{
    Charstats, Itemstatcost, Levels, Missiles, Monstats, Monstats2, Record, Skills, States,
};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_server::seams::{Clock, Pos};
use d2_sim::skills::list::ListOwner;
use d2_sim::skills::use_::bodies::{BodyStat, BodyTables};
use d2_sim::stats::StateTable;
use d2_sim::stats::{StatData, StatLists, StatTable};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The attack animation: 8 frames at speed 256, the missile event (2)
/// on frame 4 (`animdata.md` §2).
fn anim_data() -> AnimData {
    let mut a = AnimData {
        buckets: vec![Vec::new(); animdata::BUCKETS],
    };
    let mut events = [0u8; animdata::EVENTS];
    events[4] = 2;
    let name = *b"SOSCHTH\0";
    a.buckets[animdata::hash(&name[..7])].push(AnimRecord {
        name,
        frames: 8,
        speed: 256,
        events,
    });
    a
}

/// The unit tables of the name rules: every class token `SO`, mode 7
/// token `A1`.
fn looks() -> UnitLooks {
    let mut modes = vec![code(b"NU"); 20];
    modes[10] = code(b"SC");
    UnitLooks {
        player_tokens: vec![code(b"SO"); 7],
        player_modes: modes,
        ..UnitLooks::default()
    }
}

/// A synthetic itemstatcost (359 stats, no ops, fixed up): the
/// synthetic game has none, so no stat could be set.
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

const MANA: i32 = 100 << 8;

/// Progressive (`pgsv`, flag bit 4) states of the synthetic table.
const STATES: usize = 12;
const PGSV: [usize; 2] = [8, 9];

/// A skill row that starts at once, costs nothing and works in the camp.
fn row(srvst: u16, srvdo: u16) -> Skills {
    let mut s = Skills::decode(&[0u8; Skills::SIZE]);
    s.anim = 10;
    s.range = 1;
    s.intown = true;
    s.srvstfunc = srvst;
    s.srvdofunc = srvdo;
    s.srvmissile = 0xFFFF;
    s.aurastate = 0xFFFF;
    s.auratargetstate = 0xFFFF;
    s.srvoverlay = 0xFFFF;
    s.tgtoverlay = 0xFFFF;
    s.passivestate = 0xFFFF;
    s.delay = 0xFFFF_FFFF;
    s.perdelay = 0xFFFF_FFFF;
    s.hitshift = 8;
    s.srcdam = 128;
    s
}

/// A synthetic states table of [`STATES`] rows, [`PGSV`] progressive.
fn state_table() -> StateTable {
    let size = States::SIZE;
    let mut records = vec![0u8; STATES * size];
    for s in PGSV {
        records[s * size + 0x10] |= 1 << 4;
    }
    let t = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: STATES,
        record_size: size,
        records,
    };
    StateTable::new(&t, &maps::states(&t)).expect("states")
}

fn stat_data_with_states() -> Arc<StatData> {
    let base = stat_data();
    Arc::new(StatData {
        stats: base.stats.clone(),
        states: state_table(),
        ..StatData::default()
    })
}

struct Game {
    link: ThreadLink<Link<StepClock>>,
    ms: Arc<AtomicU32>,
}

impl Game {
    /// `rows`: (skill id, row); the first is selected on the right button.
    fn joined(rows: Vec<(usize, Skills)>) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (link, _) =
            single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
        let mut g = Self { link, ms };
        let rows2 = rows.clone();
        g.link
            .with(move |l| {
                let h = l.host_mut().game.events.action.hooks();
                let mut t = (*h.tables).clone();
                t.skills.skills = vec![Skills::decode(&[0u8; Skills::SIZE]); 16];
                for (i, r) in rows2 {
                    t.skills.skills[i] = r;
                }
                t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
                t.skills.stat_count = 359;
                // levels.txt: every level allows a teleport (Dragon
                // Flight lands through the level's `Teleport`).
                let mut lv = vec![Levels::decode(&[0u8; Levels::SIZE]); 3];
                for l in &mut lv {
                    l.teleport = 1;
                }
                t.levels = lv;
                let mut m = Missiles::decode(&[0u8; Missiles::SIZE]);
                m.range = 20;
                m.vel = 16;
                m.maxvel = 16;
                m.town = true;
                m.srctown = true;
                t.missiles = vec![Missiles::decode(&[0u8; Missiles::SIZE]), m];
                t.skills.missiles = t.missiles.clone();
                h.tables = Arc::new(t);
                h.anim_data = Some(Arc::new(anim_data()));
                h.x.looks = Some(Arc::new(looks()));
                h.bodies = Some(Arc::new(BodyTables {
                    stats: vec![BodyStat::default(); 359],
                    state_group: vec![0; 256],
                    state_aura: vec![false; 256],
                    pettype_count: 3,
                    pettype_group: vec![0; 3],
                    ..BodyTables::default()
                }));
                l.host_mut().game.events.action.sys.stats = StatLists::new(stat_data_with_states());
            })
            .unwrap();
        let req = single_player::create_request();
        g.link.send(SendQueue::System, &req.encode()).unwrap();
        g.ticks(1);
        g.link.send(SendQueue::System, &[0x6B]).unwrap();
        g.ticks(3);
        g.link
            .with(move |l| {
                let sim = &mut l.host_mut().game;
                let p = sim.player_of(LOCAL_CLIENT).expect("joined");
                let h = &mut sim.events.action.sys.hooks;
                let mut t = (*h.tables).clone();
                let mut c = Charstats::decode(&[0u8; Charstats::SIZE]);
                c.walkvelocity = 6;
                c.runvelocity = 9;
                t.combat.charstats = (0..7).map(|_| c.clone()).collect();
                let mut ms = Monstats::decode(&[0u8; Monstats::SIZE]);
                ms.killable = true;
                t.combat.monstats = vec![ms];
                t.combat.monstats2 = vec![Monstats2::decode(&[0u8; Monstats2::SIZE])];
                h.tables = Arc::new(t);
                let all = h.tables.skills.skills.clone();
                let mut ids = [0xFFFFu16; 10];
                for (k, (i, _)) in rows.iter().enumerate() {
                    ids[k] = *i as u16;
                }
                let list = h.skill_lists.entry(p).or_default();
                // The join ran the native skills on the synthetic `charstats`
                // (`client/msg-skills.md` §2 rule 8); this game's class skills
                // are the fixture's.
                *list = Default::default();
                list.init_player(&all, ListOwner::player(PLAYER_CLASS as i32), Some(&ids))
                    .unwrap();
                let first = rows[0].0 as i32;
                list.right = list.view().iter().position(|e| e.skill == first);
                sim.events.action.with(&mut sim.game, |_, v| {
                    v.set_base(p, 9, MANA);
                    v.set_base(p, 8, MANA);
                });
            })
            .unwrap();
        g
    }

    fn ticks(&mut self, n: usize) -> Vec<Vec<u8>> {
        let mut got = Vec::new();
        for _ in 0..n {
            self.link.pump().unwrap();
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.link.pump().unwrap();
            got.extend(self.link.receive());
        }
        got
    }

    fn player_pos(&mut self) -> Pos {
        self.link
            .with(|l| {
                let s = &mut l.host_mut().game;
                let p = s.player_of(LOCAL_CLIENT).unwrap();
                let (x, y) = s.events.action.hooks().path_position(p);
                Pos { x, y }
            })
            .unwrap()
    }

    /// A monster of class 0 beside the player with `life` life; its GUID.
    fn monster(&mut self, life: i32) -> (UnitId, u32) {
        self.link
            .with(move |l| {
                let sim = &mut l.host_mut().game;
                let p = sim.player_of(LOCAL_CLIENT).unwrap();
                let (x, y) = sim.events.action.hooks().path_position(p);
                let room = sim.game.lists.unit(p).and_then(|e| e.room());
                let req = AllocRequest {
                    ty: UnitType::Monster,
                    class: 0,
                    room,
                    add: true,
                    fixed_guid: None,
                    mode: 1,
                    allied: false,
                };
                let m = sim
                    .events
                    .action
                    .with(&mut sim.game, |g, v| v.allocate(g, &req, x + 2, y))
                    .expect("a monster");
                // Monster init sets the targetable flag (`use.md` §5.3 step 2).
                sim.events.action.sys.units.get_mut(m).unwrap().flags |= 2;
                sim.events.action.with(&mut sim.game, |_, v| {
                    v.set_base(m, 12, 10);
                    v.set_base(p, 12, 10);
                    v.set_base(p, 19, 1000);
                    v.set_base(p, 21, 20);
                    v.set_base(p, 22, 30);
                    v.set_base(m, 7, life << 8);
                    v.set_base(m, 6, life << 8);
                });
                (m, sim.events.action.sys.units.get(m).unwrap().guid)
            })
            .unwrap()
    }

    fn player_mode(&mut self) -> u32 {
        self.link
            .with(|l| {
                let s = &mut l.host_mut().game;
                let p = s.player_of(LOCAL_CLIENT).unwrap();
                s.events.action.sys.units.get(p).map_or(0, |u| u.mode)
            })
            .unwrap()
    }

    /// Has the local player the state (a charge or a buff)?
    fn has_state(&mut self, state: u16) -> bool {
        self.link
            .with(move |l| {
                let s = &mut l.host_mut().game;
                let p = s.player_of(LOCAL_CLIENT).unwrap();
                s.events.action.sys.stats.has_state(p, u32::from(state))
            })
            .unwrap()
    }

    fn errors(&mut self) -> String {
        self.link
            .with(|l| format!("{:?}", l.host_mut().game.events.action.hooks().errors))
            .unwrap()
    }

    fn cast_at(&mut self, dx: i32) -> Vec<Vec<u8>> {
        let at = self.player_pos();
        let mut msg = vec![0x0C];
        msg.extend(((at.x + dx) as u16).to_le_bytes());
        msg.extend((at.y as u16).to_le_bytes());
        self.link.send(SendQueue::Game, &msg).unwrap();
        self.link.pump().unwrap();
        let mut got = Vec::new();
        let mut modes = vec![self.player_mode()];
        for _ in 0..14 {
            got.extend(self.ticks(1));
            modes.push(self.player_mode());
        }
        eprintln!("modes {modes:?}");
        got
    }

    fn cast_on(&mut self, guid: u32) -> Vec<Vec<u8>> {
        let mut msg = vec![0x0D];
        msg.extend(1u32.to_le_bytes());
        msg.extend(guid.to_le_bytes());
        self.link.send(SendQueue::Game, &msg).unwrap();
        self.link.pump().unwrap();
        self.ticks(14)
    }
}

// Covers: specs/skills/bodies.md §8.3, §6.9
#[test]
fn a_sentry_trap_is_laid_and_listed_as_a_pet() {
    let mut s = row(0, 45);
    s.summon = 0;
    s.summode = 1;
    s.pettype = 2;
    let mut g = Game::joined(vec![(3, s)]);
    let got = g.cast_at(5);
    let errors = g.errors();
    assert!(
        got.iter().any(|m| m.contains(&0x7A)),
        "pet add; errors: {errors}"
    );
}

// Covers: specs/skills/bodies.md §8.21
#[test]
fn shadow_warrior_summons_the_shadow() {
    let mut s = row(0, 49);
    s.summon = 0;
    s.summode = 1;
    s.pettype = 2;
    let mut g = Game::joined(vec![(3, s)]);
    let got = g.cast_at(5);
    let errors = g.errors();
    assert!(
        got.iter().any(|m| m.contains(&0x7A)),
        "pet add; errors: {errors}"
    );
}

// Covers: specs/skills/bodies.md §8.8, §2.14
#[test]
fn tiger_strike_adds_a_charge_on_a_hit() {
    let mut s = row(23, 34);
    s.aurastate = PGSV[0] as u16;
    s.aurastat1 = 10;
    s.range = 1;
    let mut g = Game::joined(vec![(3, s)]);
    let (_, guid) = g.monster(500);
    g.cast_on(guid);
    let errors = g.errors();
    assert!(
        g.has_state(PGSV[0] as u16),
        "a charge; errors: {errors}; mode {}",
        g.player_mode()
    );
}

// Covers: specs/skills/bodies.md §4.3
#[test]
fn burst_of_speed_turns_its_state_on() {
    let mut s = row(0, 18);
    s.aurastate = PGSV[1] as u16;
    s.auralencalc = 0;
    let mut g = Game::joined(vec![(3, s)]);
    g.cast_at(0);
    let errors = g.errors();
    assert!(g.has_state(PGSV[1] as u16), "state on; errors: {errors}");
}

// Covers: specs/skills/bodies.md §2.14, §8.10
// (asserts the charge and no fault, not the finisher damage)
#[test]
fn a_finisher_after_a_charge_runs_without_faults() {
    let charge = row(23, 34);
    let mut charge = charge;
    charge.aurastate = PGSV[0] as u16;
    charge.aurastat1 = 10;
    let finisher = row(23, 35);
    let mut g = Game::joined(vec![(3, charge), (4, finisher)]);
    let (_, guid) = g.monster(500);
    g.cast_on(guid);
    assert!(g.has_state(PGSV[0] as u16), "charged; {}", g.errors());
    g.link
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let p = sim.player_of(LOCAL_CLIENT).unwrap();
            let list = sim.events.action.hooks().skill_lists.get_mut(&p).unwrap();
            list.right = list.view().iter().position(|e| e.skill == 4);
        })
        .unwrap();
    g.cast_on(guid);
    assert!(g.errors() == "[]", "no faults: {}", g.errors());
}

// Covers: specs/skills/bodies-2b.md §7.20
#[test]
fn dragon_flight_moves_the_assassin_to_the_monster() {
    let mut g = Game::joined(vec![(3, row(0, 52))]);
    let (_, guid) = g.monster(500);
    let from = g.player_pos();
    g.cast_on(guid);
    let to = g.player_pos();
    let errors = g.errors();
    assert!(to.x > from.x, "flew east: {from:?} -> {to:?}; {errors}");
}
