// Spec: specs/skills/bodies.md (§3.7 Firestorm, §8.20 Tornado), specs/skills/use.md §5
//! The Druid's missile skills on the play host, headless, on the
//! synthetic single-player game (q-druid): C→S 0x0C → the right skill's
//! do step (srvdo 117 Firestorm, 118 Tornado) → several server missiles.
//!
//! Test-local fills as `app_cast.rs`: one skill row (id 3) and one
//! missile row; the do-step formula `calc1` is the constant 3
//! (`skillscode` bytes `07 03 00`).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link, DEFAULT_SEED, PLAYER_CLASS};
use d2_client::bridge::link::{SendQueue, ServerLink};
use d2_client::bridge::LOCAL_CLIENT;
use d2_client::rules::unit_composite::code;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_data::bin::BinTable;
use d2_data::fixup::maps::StateMaps;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{
    Charstats, Itemstatcost, Missiles, Monstats, Monstats2, Record, Skills, States,
};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_server::seams::{Clock, Pos};
use d2_sim::skills::list::ListOwner;
use d2_sim::stats::StateTable;
use d2_sim::stats::{StatData, StatLists, StatTable};

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
    // Three states (0, 1 = the shape state, 2), no groups, one word.
    let states = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: 3,
        record_size: States::SIZE,
        records: vec![0u8; 3 * States::SIZE],
    };
    let maps = StateMaps {
        words: 1,
        bitsets: vec![0; 40],
        ..StateMaps::default()
    };
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        states: StateTable::new(&states, &maps).expect("states"),
        ..StatData::default()
    })
}

const MANA: i32 = 100 << 8;

struct Game {
    link: ThreadLink<Link<StepClock>>,
    ms: Arc<AtomicU32>,
}

impl Game {
    fn joined(srvdofunc: u16) -> Self {
        Self::joined_with(srvdofunc, |_, _| {})
    }

    fn joined_with(
        srvdofunc: u16,
        row: impl FnOnce(&mut Skills, &mut Vec<Missiles>) + Send + 'static,
    ) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (link, _) =
            single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
        let mut g = Self { link, ms };
        g.link
            .with(move |l| {
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
                fb.srvmissilea = 1;
                fb.srvdofunc = srvdofunc;
                fb.calc1 = 0;
                t.skills.skills_code = vec![0x07, 3, 0x00];
                t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
                let mut m = Missiles::decode(&[0u8; Missiles::SIZE]);
                m.range = 20;
                m.vel = 16;
                m.maxvel = 16;
                m.town = true;
                m.srctown = true;
                let mut missiles = vec![Missiles::decode(&[0u8; Missiles::SIZE]), m];
                row(&mut t.skills.skills[FIRE_BOLT], &mut missiles);
                t.missiles = missiles;
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

fn cast_makes_missiles(srvdofunc: u16) -> usize {
    let mut g = Game::joined(srvdofunc);
    let at = g.player_pos();
    let mut msg = vec![0x0C];
    msg.extend(((at.x + 6) as u16).to_le_bytes());
    msg.extend((at.y as u16).to_le_bytes());
    g.link.send(SendQueue::Game, &msg).unwrap();
    g.link.pump().unwrap();
    let mut most = 0;
    for _ in 0..12 {
        g.ticks(1);
        most = most.max(g.missiles());
    }
    let errors = g
        .link
        .with(|l| format!("{:?}", l.host_mut().game.events.action.hooks().errors))
        .unwrap();
    assert!(g.mana() < MANA, "mana spent; errors: {errors}");
    most
}

// Covers: specs/skills/bodies.md §3.7
#[test]
fn firestorm_fans_out_missiles() {
    assert!(cast_makes_missiles(117) >= 3);
}

// Covers: specs/skills/bodies.md §8.20
#[test]
fn tornado_fans_out_missiles() {
    assert!(cast_makes_missiles(118) >= 3);
}

/// The shape state of the synthetic Werewolf row.
const WOLF: u16 = 1;

// Covers: specs/skills/bodies.md §8.15
#[test]
fn werewolf_turns_the_player_into_the_shape_and_the_seam_sees_it() {
    let mut g = Game::joined_with(116, |r, _| {
        r.aurastate = WOLF;
        r.auralencalc = 0;
        r.range = 0;
    });
    let at = g.player_pos();
    let mut msg = vec![0x0C];
    msg.extend(((at.x + 1) as u16).to_le_bytes());
    msg.extend((at.y as u16).to_le_bytes());
    g.link.send(SendQueue::Game, &msg).unwrap();
    g.link.pump().unwrap();
    g.ticks(12);
    let (on, shifted, errors) = g
        .link
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let p = sim.player_of(LOCAL_CLIENT).unwrap();
            let on = sim.events.action.sys.stats.has_state(p, u32::from(WOLF));
            let shifted = sim.events.action.sys.hooks.x.skills.shifted.contains(&p);
            (
                on,
                shifted,
                format!("{:?}", sim.events.action.hooks().errors),
            )
        })
        .unwrap();
    // Before: `shapeshifted` was always false (no form was ever seen).
    assert!(on, "the state is on; errors: {errors}");
    assert!(shifted, "the use pipeline sees the form");
}
