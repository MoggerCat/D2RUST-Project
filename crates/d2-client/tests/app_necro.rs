// Spec: specs/skills/bodies.md (§4.3, §8.4, §8.6, §8.9), specs/skills/use.md §5
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
use d2_sim::skills::use_::bodies::{BodyStat, BodyTables};
use d2_sim::stats::{StatData, StatLists, StatTable, StateTable};

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

const TEETH: usize = 67;
const BONE_ARMOR: usize = 68;
const POISON_NOVA: usize = 92;
const CLAY_GOLEM: usize = 75;
const BONE_STATE: u16 = 20;
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
    let n_states = 256;
    let states = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: n_states,
        record_size: States::SIZE,
        records: vec![0u8; n_states * States::SIZE],
    };
    let maps = StateMaps {
        words: n_states / 32,
        bitsets: vec![0; 40 * (n_states / 32)],
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
    /// `skill` is the row under test; `fill` sets its columns.
    fn joined(skill: usize, fill: impl FnOnce(&mut Skills) + Send + 'static) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (link, _) =
            single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
        let mut g = Self { link, ms };
        g.link
            .with(move |l| {
                let h = l.host_mut().game.events.action.hooks();
                let mut t = (*h.tables).clone();
                t.skills.skills = vec![Skills::decode(&[0u8; Skills::SIZE]); 100];
                let fb = &mut t.skills.skills[skill];
                fb.srvmissile = 0xFFFF;
                fb.aurastate = 0xFFFF;
                fb.auratargetstate = 0xFFFF;
                fb.srvoverlay = 0xFFFF;
                fb.tgtoverlay = 0xFFFF;
                fb.passivestate = 0xFFFF;
                fb.delay = 0xFFFF_FFFF;
                fb.perdelay = 0xFFFF_FFFF;
                fb.anim = 10;
                fb.range = 2;
                fb.mana = 5;
                fb.manashift = 7;
                fb.minmana = 1;
                fb.intown = true;
                fb.usemanaondo = true;
                fill(fb);
                // Formulas `push i16 v; end`: offset 0 = 1, 4 = 500, 8 = 2.
                t.skills.skills_code = [1i16, 500, 2]
                    .iter()
                    .flat_map(|v| [0x08, v.to_le_bytes()[0], v.to_le_bytes()[1], 0x00])
                    .collect();
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
                h.bodies = Some(Arc::new(BodyTables {
                    stats: vec![BodyStat::default(); 359],
                    state_group: vec![0; 256],
                    state_aura: vec![false; 256],
                    pettype_count: 3,
                    pettype_group: vec![0; 3],
                    ..BodyTables::default()
                }));
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
            .with(move |l| {
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
                        skill as u16,
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
                let i = list.view().iter().position(|e| e.skill == skill as i32);
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

impl Game {
    fn cast_at_point(&mut self, dx: i32) -> (Vec<Vec<u8>>, usize, String) {
        let at = self.player_pos();
        let mut msg = vec![0x0C];
        msg.extend(((at.x + dx) as u16).to_le_bytes());
        msg.extend((at.y as u16).to_le_bytes());
        self.link.send(SendQueue::Game, &msg).unwrap();
        let mut got = Vec::new();
        let mut most = 0;
        for _ in 0..14 {
            got.extend(self.ticks(1));
            most = most.max(self.missiles());
        }
        let errors = self
            .link
            .with(|l| format!("{:?}", l.host_mut().game.events.action.hooks().errors))
            .unwrap();
        (got, most, errors)
    }

    fn monsters(&mut self) -> usize {
        self.link
            .with(|l| {
                let s = &mut l.host_mut().game;
                s.game
                    .lists
                    .units_of_type(d2_sim::units::UnitType::Monster)
                    .len()
            })
            .unwrap()
    }

    fn player_state(&mut self, state: u16) -> bool {
        self.link
            .with(move |l| {
                let s = &mut l.host_mut().game;
                let p = s.player_of(LOCAL_CLIENT).unwrap();
                s.events
                    .action
                    .with(&mut s.game, |_, v| v.stats.has_state(p, state.into()))
            })
            .unwrap()
    }
}

// Covers: specs/skills/bodies.md §8.6
#[test]
fn teeth_spends_mana_and_makes_missiles() {
    let mut g = Game::joined(TEETH, |r| {
        r.srvdofunc = 8;
        r.srvmissilea = 1;
        r.srvmissileb = 0xFFFF;
        r.calc1 = 0;
    });
    let (got, most, errors) = g.cast_at_point(6);
    assert!(g.mana() < MANA, "mana spent; {errors}");
    assert!(most > 0, "teeth made missiles; {errors}");
    assert!(got.iter().any(|m| m.contains(&0x4D)), "{errors}");
}

// Covers: specs/skills/bodies.md §8.4
#[test]
fn poison_nova_makes_a_ring_of_missiles() {
    let mut g = Game::joined(POISON_NOVA, |r| {
        r.srvdofunc = 22;
        r.srvmissile = 1;
    });
    let (_, most, errors) = g.cast_at_point(0);
    assert!(g.mana() < MANA, "mana spent; {errors}");
    assert!(most > 1, "the ring made missiles: {most}; {errors}");
}

// Covers: specs/skills/bodies.md §4.3
#[test]
fn bone_armor_turns_its_state_on() {
    let mut g = Game::joined(BONE_ARMOR, |r| {
        r.srvdofunc = 18;
        r.aurastate = BONE_STATE;
        r.auralencalc = 4;
    });
    let (got, _, errors) = g.cast_at_point(0);
    assert!(g.mana() < MANA, "mana spent; {errors}");
    assert!(g.player_state(BONE_STATE), "state on; {errors}");
    assert!(got.iter().any(|m| m.contains(&0xA8)), "0xA8 sent: {errors}");
}

// Covers: specs/skills/bodies.md §8.9
#[test]
fn clay_golem_summons_a_pet() {
    let mut g = Game::joined(CLAY_GOLEM, |r| {
        r.srvdofunc = 56;
        r.summon = 0;
        r.summode = 1;
        r.pettype = 2;
        r.petmax = 0;
    });
    let before = g.monsters();
    let (got, _, errors) = g.cast_at_point(2);
    assert!(g.mana() < MANA, "mana spent; {errors}");
    assert_eq!(g.monsters(), before + 1, "a golem exists; {errors}");
    assert!(
        got.iter().any(|m| m.first() == Some(&0x7A)),
        "0x7A: {errors}"
    );
}
