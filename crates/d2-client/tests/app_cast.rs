// Spec: specs/skills/use.md (§3, §5, Test vectors: Fire Bolt L1), specs/sim/intents-events.md (§2.4 r4, §7.4)
//! A right-hand skill cast on the play host, headless, on the synthetic
//! single-player game: C→S 0x0C (the right skill at a point) → the
//! server's skill use → mana → the missile in the server's store →
//! S→C 0x4C and the vitals.
//!
//! Test-local fills: the synthetic game has no `skills`, `missiles` or
//! vitals tables, so the joined game gets one Fire Bolt row (id 3:
//! `mana` 5, shift 7, `anim` 10 = SC, `srvmissile` 1, no start / do
//! function, `use.md` Test vectors) and one missile row.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, Link, DEFAULT_SEED, PLAYER_CLASS};
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_client::bridge::LOCAL_CLIENT;
use d2_client::rules::unit_composite::code;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_data::bin::BinTable;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Charstats, Itemstatcost, Missiles, Monstats, Monstats2, Record, Skills};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_server::seams::{Clock, Pos};
use d2_sim::skills::list::ListOwner;
use d2_sim::stats::{StatData, StatLists, StatTable};

mod app_support;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

const FIRE_BOLT: usize = 3;
/// The sorceress casting: 8 frames at speed 256, the missile event (2)
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

/// The unit tables of the name rules: every class token `SO`, mode 10
/// token `SC`.
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

struct Game {
    link: ThreadLink<Link<StepClock>>,
    ms: Arc<AtomicU32>,
}

impl Game {
    fn joined() -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (link, _) = single_player::start(
            app_support::game_data(),
            DEFAULT_SEED,
            StepClock(ms.clone()),
        )
        .unwrap();
        let mut g = Self { link, ms };
        g.link
            .with(|l| {
                let h = l.host_mut().game.events.action.hooks();
                let mut t = (*h.tables).clone();
                t.skills.skills = vec![Skills::decode(&[0u8; Skills::SIZE]); 8];
                let fb = &mut t.skills.skills[FIRE_BOLT];
                fb.anim = 10;
                fb.range = 2;
                fb.mana = 5;
                fb.manashift = 7;
                fb.minmana = 1;
                fb.intown = true;
                fb.usemanaondo = true;
                fb.srvmissile = 1;
                t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
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
                l.host_mut().game.events.action.sys.stats = StatLists::new(stat_data());
            })
            .unwrap();
        let req = single_player::create_request();
        g.link.send(SendQueue::System, &req.encode()).unwrap();
        g.ticks(1);
        g.link.send(SendQueue::System, &[0x6B]).unwrap();
        g.ticks(3);
        g.link
            .with(|l| {
                let sim = &mut l.host_mut().game;
                let p = sim.player_of(LOCAL_CLIENT).expect("joined");
                let h = &mut sim.events.action.sys.hooks;
                let mut t = (*h.tables).clone();
                let mut c = Charstats::decode(&[0u8; Charstats::SIZE]);
                c.walkvelocity = 6;
                c.runvelocity = 9;
                t.combat.charstats = (0..7).map(|_| c.clone()).collect();
                t.combat.monstats = vec![Monstats::decode(&[0u8; Monstats::SIZE])];
                t.combat.monstats2 = vec![Monstats2::decode(&[0u8; Monstats2::SIZE])];
                h.tables = Arc::new(t);
                let rows = h.tables.skills.skills.clone();
                let list = h.skill_lists.entry(p).or_default();
                // The join ran the native skills on the synthetic `charstats`
                // (`client/msg-skills.md` §2 rule 8); this game's class skills
                // are the fixture's.
                *list = Default::default();
                list.init_player(
                    &rows,
                    ListOwner::player(PLAYER_CLASS as i32),
                    Some(&[
                        FIRE_BOLT as u16,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                        0xFFFF,
                    ]),
                )
                .unwrap();
                let i = list.view().iter().position(|e| e.skill == FIRE_BOLT as i32);
                list.right = i;
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

    fn mana(&mut self) -> i32 {
        self.link
            .with(|l| {
                let s = &mut l.host_mut().game;
                let p = s.player_of(LOCAL_CLIENT).unwrap();
                s.events.action.with(&mut s.game, |_, v| v.stat(p, 8))
            })
            .unwrap()
    }

    fn missiles(&mut self) -> usize {
        self.link
            .with(|l| {
                let h = l.host_mut().game.events.action.hooks();
                h.missiles.as_ref().map_or(0, |m| m.missiles().count())
            })
            .unwrap()
    }
}

// Covers: specs/skills/use.md §5
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn a_right_skill_at_a_point_costs_mana_and_creates_the_missile() {
    let mut g = Game::joined();
    let at = g.player_pos();
    let mut msg = vec![0x0C];
    msg.extend(((at.x + 6) as u16).to_le_bytes());
    msg.extend((at.y as u16).to_le_bytes());
    g.link.send(SendQueue::Game, &msg).unwrap();
    g.link.pump().unwrap();
    let handled = g
        .link
        .with(|l| format!("{:?}", l.last_frame().messages))
        .unwrap();
    assert!(handled.contains("Dispatched(Done)"), "{handled}");
    let mut got = Vec::new();
    let mut most = 0;
    for _ in 0..12 {
        got.extend(g.ticks(1));
        most = most.max(g.missiles());
    }
    let errors = g
        .link
        .with(|l| format!("{:?}", l.host_mut().game.events.action.hooks().errors))
        .unwrap();
    // Before: no action-frame route (no do step), no kept target (the
    // missile has no aim), no animation names (`Anim(NoRecord)`).
    assert!(g.mana() < MANA, "mana spent; errors: {errors}");
    assert!(most > 0, "the server made the missile; errors: {errors}");
    assert!(
        got.iter().any(|m| m.contains(&0x4D)),
        "the cast mode reaches the client: {got:?}; {errors}"
    );
}
