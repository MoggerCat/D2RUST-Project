// Spec: specs/combat/vitals.md §2–§5; specs/skills/levels.md §6.4
//! XP and level-up on the play server (synthetic data): experience gained
//! on the server reaches the client as stat messages (level, stat points,
//! skill points), C→S 0x3A spends a stat point and C→S 0x3B a skill point,
//! and the client model follows every message.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, Link, DEFAULT_SEED, PLAYER_CLASS};
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_data::bin::BinTable;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Charstats, Experience, Itemstatcost, Record, Skills};
use d2_sim::combat::vitals::{add_experience, VitalsTables};
use d2_sim::stats::{ClassStats, StatData, StatLists, StatTable};
use d2_sim::units::UnitId;
use d2_sim::wiring::action::View;

mod app_support;

struct StepClock(Arc<AtomicU32>);

impl d2_server::seams::Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// A synthetic `itemstatcost`: 359 stats, the first 16 saved.
fn stat_data() -> Arc<StatData> {
    let (n, size) = (359, Itemstatcost::SIZE);
    let mut records = vec![0u8; n * size];
    for (s, r) in records.chunks_mut(size).enumerate() {
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            r[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        r[0..2].copy_from_slice(&(s as u16).to_le_bytes());
        if s < 16 {
            r[5] |= 0x10;
        }
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
        classes: vec![ClassStats::default(); 7],
        ..StatData::default()
    })
}

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// Sorceress-like `charstats` (class 1) and `experience` rows
/// (500 × level²).
fn vitals() -> VitalsTables {
    let charstats = (0..7)
        .map(|_| {
            let mut c: Charstats = blank();
            c.str = 10;
            c.dex = 25;
            c.int = 35;
            c.vit = 10;
            c.lifeperlevel = 4;
            c.staminaperlevel = 4;
            c.manaperlevel = 8;
            c.lifepervitality = 8;
            c.staminapervitality = 4;
            c.manapermagic = 8;
            c.statperlevel = 5;
            c
        })
        .collect();
    let row = |v: u32| {
        let mut e: Experience = blank();
        e.amazon = v;
        e.sorceress = v;
        e.necromancer = v;
        e.paladin = v;
        e.barbarian = v;
        e.druid = v;
        e.assassin = v;
        e
    };
    let mut experience = vec![row(99)];
    experience.extend((0..=99).map(|l: u32| row(500 * l * l)));
    VitalsTables {
        charstats,
        experience,
    }
}

type Server = ThreadLink<Link<StepClock>>;

struct Game {
    link: Server,
    ms: Arc<AtomicU32>,
}

impl Game {
    /// Pumps one tick and returns the S→C messages of it.
    fn tick(&mut self) -> Vec<Vec<u8>> {
        self.ms.fetch_add(40, Ordering::SeqCst);
        self.link.pump().unwrap();
        self.link.receive()
    }
}

/// The synthetic game, joined, with vitals tables, a level-1 sorceress
/// with 100 life, and one class skill (id 1) in the tables.
fn joined() -> (Game, UnitId, u32) {
    let ms = Arc::new(AtomicU32::new(1000));
    let (mut link, _) = single_player::start(
        app_support::game_data(),
        DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    link.send(SendQueue::System, &single_player::create_request().encode())
        .unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    link.receive();
    link.send(SendQueue::System, &[0x6B]).unwrap();
    ms.fetch_add(40, Ordering::SeqCst);
    link.pump().unwrap();
    link.receive();
    let (p, guid) = link
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (p, g) = single_player::local_player(sim).expect("joined");
            let a = &mut sim.events.action;
            let t = Arc::make_mut(&mut a.hooks().tables);
            t.skills.skills.resize(2, blank::<Skills>());
            let s: &mut Skills = &mut t.skills.skills[1];
            s.charclass = single_player::PLAYER_CLASS as _;
            s.ingame = true;
            s.skpoints = d2_sim::skills::levels::NO_CALC;
            (s.reqskill1, s.reqskill2, s.reqskill3) = (0xFFFF, 0xFFFF, 0xFFFF);
            a.hooks().vitals = Some(Arc::new(vitals()));
            let sys = &mut a.sys;
            // The synthetic game has no `itemstatcost`: stats 0-15 saved.
            sys.stats = StatLists::new(stat_data());
            let ty = sys.units.get(p).unwrap().ty;
            sys.stats.alloc_extended(
                &mut sys.hooks,
                p,
                ty,
                g,
                u32::from(PLAYER_CLASS as u8),
                0,
                None,
            );
            for (s, v) in [(12u16, 1), (7, 100 << 8), (6, 100 << 8), (0, 10), (4, 0)] {
                sys.stats.unit_set(&mut sys.hooks, p, s, v, 0);
            }
            sys.hooks.enable_vitals_sync();
            (p, g)
        })
        .unwrap();
    let mut g = Game { link, ms };
    // The next ticks populate the town and put the client in game.
    for _ in 0..3 {
        g.tick();
    }
    (g, p, guid)
}

impl Game {
    /// Adds `gain` experience to the player on the server (the grant a
    /// kill ends in, `vitals.md` §4.5).
    fn gain(&mut self, p: UnitId, gain: u32) {
        self.link
            .with(move |l| {
                let a = &mut l.host_mut().game.events.action;
                let t = a.hooks().vitals.clone().unwrap();
                let s = &mut a.sys;
                let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
                add_experience(&mut v, &t, p, gain);
            })
            .unwrap();
    }

    fn stat(&mut self, p: UnitId, stat: u16) -> i32 {
        self.link
            .with(move |l| {
                l.host_mut()
                    .game
                    .events
                    .action
                    .sys
                    .stats
                    .unit_base(p, stat, 0)
            })
            .unwrap()
    }
}

fn has(msgs: &[Vec<u8>], m: &[u8]) -> bool {
    msgs.iter().any(|x| x == m)
}

// Covers: specs/combat/vitals.md §3, §4.5
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn experience_to_level_up_reaches_the_client_as_stat_messages() {
    let (mut g, p, _) = joined();
    g.tick();
    g.gain(p, 500);
    let got = g.tick();
    // Level 2, +5 stat points, +1 skill point, experience 500 (0x1B: +500).
    assert!(has(&got, &[0x1D, 12, 2]), "{got:02X?}");
    assert!(has(&got, &[0x1D, 4, 5]), "{got:02X?}");
    assert!(has(&got, &[0x1D, 5, 1]), "{got:02X?}");
    assert_eq!(g.stat(p, 12), 2);
    assert_eq!(g.stat(p, 13), 500);
    // Nothing repeats on the next tick.
    let again = g.tick();
    assert!(!has(&again, &[0x1D, 12, 2]), "{again:02X?}");
}

// Covers: specs/combat/vitals.md §2
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_stat_point_spends_on_the_server_and_is_sent_back() {
    let (mut g, p, _) = joined();
    g.gain(p, 500);
    g.tick();
    // Strength +1 (stat 0, count − 1 = 0).
    g.link.send(SendQueue::Game, &[0x3A, 0, 0]).unwrap();
    let got = g.tick();
    assert_eq!(g.stat(p, 0), 11);
    assert_eq!(g.stat(p, 4), 4);
    assert!(has(&got, &[0x1D, 0, 11]), "{got:02X?}");
    assert!(has(&got, &[0x1D, 4, 4]), "{got:02X?}");
    // Ten more asked, four left: the loop spends what it can, then fails
    // (`vitals.md` §2, result 2).
    g.link.send(SendQueue::Game, &[0x3A, 0, 9]).unwrap();
    g.tick();
    assert_eq!(g.stat(p, 4), 0);
    assert_eq!(g.stat(p, 0), 15);
}

// Covers: specs/skills/levels.md §6.4
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_skill_point_adds_the_skill_and_tells_the_client() {
    let (mut g, p, guid) = joined();
    g.gain(p, 500);
    g.tick();
    g.link.send(SendQueue::Game, &[0x3B, 1, 0]).unwrap();
    let got = g.tick();
    assert_eq!(g.stat(p, 5), 0, "the point is spent");
    let mut m = vec![0x21, 0, 0];
    m.extend_from_slice(&guid.to_le_bytes());
    m.extend_from_slice(&[1, 0, 1, 0, 0]);
    assert!(has(&got, &m), "{got:02X?}");
    assert!(has(&got, &[0x1D, 5, 0]), "{got:02X?}");
    // No point left: nothing more.
    g.link.send(SendQueue::Game, &[0x3B, 1, 0]).unwrap();
    g.tick();
    assert_eq!(g.stat(p, 5), 0);
}
