// Spec: specs/skills/bodies.md (§4.3, §8.4, §8.12), specs/skills/bodies-2.md (§3.2), specs/skills/bodies-2b.md (§6.2, §6.3, §6.5), specs/skills/use.md §5
//! The Sorceress's skills cast on the play host, headless, on the
//! synthetic single-player game: C→S 0x0C (the right skill at a point) →
//! the server's skill use → the body's effect (missiles, states, the
//! teleport). One test per skill; rows are test-local (the synthetic game
//! has no `skills`, `missiles` or `states` tables).

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
use d2_sim::stats::states::StateTable;
use d2_sim::stats::{StatData, StatLists, StatTable};
use d2_sim::wiring::path::{place, PathCtx};

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Real 1.14d skill ids of the Sorceress's starting row.
const FIRE_BOLT: usize = 36;
const CHARGED_BOLT: usize = 38;
const FROZEN_ARMOR: usize = 40;
const ICE_BOLT: usize = 45;
const NOVA: usize = 48;
const FIRE_WALL: usize = 51;
const ENCHANT: usize = 52;
const TELEPORT: usize = 54;
const METEOR: usize = 56;
const BLIZZARD: usize = 59;
const SORC_SKILLS: [u16; 10] = [36, 38, 45, 40, 54, 48, 59, 51, 56, 52];
/// A state id that is a valid row of the synthetic `states` table.
const STATE: u16 = 20;
const MANA: i32 = 100 << 8;

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

/// A synthetic itemstatcost (359 stats, no ops, fixed up) and a
/// `states` table of 160 blank rows.
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
    let states = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: 160,
        record_size: States::SIZE,
        records: vec![0u8; 160 * States::SIZE],
    };
    Arc::new(StatData {
        stats: StatTable::from_fixed(&t).expect("itemstatcost"),
        states: StateTable::new(&states, &maps::states(&states)).expect("states"),
        ..StatData::default()
    })
}

/// Formula constants: `f(v)` is the offset of `push i16 v; end`.
struct Code(Vec<u8>);

impl Code {
    fn f(&mut self, v: i16) -> u32 {
        let at = self.0.len() as u32;
        self.0.push(0x08);
        self.0.extend(v.to_le_bytes());
        self.0.push(0x00);
        at
    }
}

/// A blank skills row: every formula "none", every state / stat / missile
/// reference invalid (the shape of the tables' own blank rows).
fn blank_row() -> Skills {
    let mut r = Skills::decode(&[0u8; Skills::SIZE]);
    for f in [
        &mut r.auralencalc,
        &mut r.aurarangecalc,
        &mut r.aurastatcalc1,
        &mut r.calc1,
        &mut r.calc2,
        &mut r.calc3,
        &mut r.calc4,
        &mut r.petmax,
        &mut r.skpoints,
        &mut r.tohitcalc,
        &mut r.dmgsympercalc,
        &mut r.edmgsympercalc,
        &mut r.elensympercalc,
    ] {
        *f = 0xFFFF_FFFF;
    }
    for s in [
        &mut r.srvmissile,
        &mut r.srvmissilea,
        &mut r.srvmissileb,
        &mut r.srvmissilec,
        &mut r.aurastate,
        &mut r.auratargetstate,
        &mut r.srvoverlay,
        &mut r.aurastat1,
        &mut r.aurastat2,
        &mut r.auraevent1,
        &mut r.auraevent2,
        &mut r.auraevent3,
    ] {
        *s = 0xFFFF;
    }
    r.skilldesc = 0xFFFF;
    r.charclass = 0xFF;
    r.reqskill1 = 0xFFFF;
    r.reqskill2 = 0xFFFF;
    r.reqskill3 = 0xFFFF;
    r.itypea1 = 0xFFFF;
    r.anim = 10;
    r.range = 2;
    r.mana = 5;
    r.manashift = 7;
    r.minmana = 1;
    r.intown = true;
    r.usemanaondo = true;
    r
}

/// The sorceress rows (`skills.txt` shape: `srvdofunc` / `srvmissile*` /
/// `aurastate`), test-local. Missile 1 is the generic flying missile.
// d2rs-own, unverified: the column values are the 1.14d skills.txt shape
// as the bodies' specs read them (functions.tsv), not a checked table.
fn rows(code: &mut Code) -> Vec<Skills> {
    let mut v = vec![Skills::decode(&[0u8; Skills::SIZE]); 64];
    let mut set = |id: usize, f: &dyn Fn(&mut Skills)| {
        let mut r = blank_row();
        f(&mut r);
        v[id] = r;
    };
    set(FIRE_BOLT, &|r| r.srvmissile = 1);
    set(ICE_BOLT, &|r| r.srvmissile = 1);
    let three = code.f(3);
    set(CHARGED_BOLT, &|r| {
        r.srvdofunc = 17;
        r.srvmissilea = 1;
        r.calc1 = three;
    });
    let dur = code.f(250);
    set(FROZEN_ARMOR, &|r| {
        r.srvdofunc = 18;
        r.aurastate = STATE;
        r.auralencalc = dur;
    });
    set(ENCHANT, &|r| {
        r.srvdofunc = 25;
        r.aurastate = STATE + 1;
        r.auralencalc = dur;
    });
    set(TELEPORT, &|r| r.srvdofunc = 27);
    set(NOVA, &|r| {
        r.srvdofunc = 22;
        r.srvmissilea = 1;
    });
    set(BLIZZARD, &|r| {
        r.srvdofunc = 28;
        r.srvmissilea = 1;
    });
    set(METEOR, &|r| {
        r.srvdofunc = 28;
        r.srvmissilea = 1;
    });
    set(FIRE_WALL, &|r| {
        r.srvdofunc = 24;
        r.srvmissilea = 1;
    });
    v
}

struct Game {
    link: ThreadLink<Link<StepClock>>,
    ms: Arc<AtomicU32>,
}

impl Game {
    /// A joined game whose right skill is `right`.
    fn joined(right: usize) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let (link, _) =
            single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
        let mut g = Self { link, ms };
        g.link
            .with(|l| {
                let h = l.host_mut().game.events.action.hooks();
                let mut t = (*h.tables).clone();
                let mut code = Code(Vec::new());
                t.skills.skills = rows(&mut code);
                t.skills.skills_code = code.0;
                t.skills.level_cap = d2_sim::skills::LEVEL_CAP_114D;
                t.skills.stat_count = 359;
                // levels.txt: the Blood Moor (2) allows teleporting, the
                // camp (1) does not.
                let mut lv = vec![Levels::decode(&[0u8; Levels::SIZE]); 3];
                lv[2].teleport = 1;
                t.levels = lv;
                let mut m = Missiles::decode(&[0u8; Missiles::SIZE]);
                m.range = 20;
                m.vel = 16;
                m.maxvel = 16;
                m.town = true;
                m.srctown = true;
                m.dmgsympercalc = 0xFFFF_FFFF;
                m.edmgsympercalc = 0xFFFF_FFFF;
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
                    Some(&SORC_SKILLS),
                )
                .unwrap();
                let i = list.view().iter().position(|e| e.skill == right as i32);
                list.right = i;
                sim.events.action.with(&mut sim.game, |_, v| {
                    v.set_base(p, 9, MANA);
                    v.set_base(p, 8, MANA);
                });
                // Out of the camp: Fire Wall and Teleport refuse in town.
                let warped = sim.events.action.with(&mut sim.game, |g, v| {
                    place::level_warp(PathCtx::of(v, g), p, 2, 0)
                });
                assert_eq!(warped, Some(true), "the Blood Moor");
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
            .with(|l| {
                let h = l.host_mut().game.events.action.hooks();
                format!("{:?} {:?}", h.errors, h.x.skills.log)
            })
            .unwrap()
    }

    /// Cast the right skill at the point 6 subtiles east and run `n`
    /// ticks; the most missiles alive at once, and every S→C message.
    fn cast(&mut self, n: usize) -> (usize, Vec<Vec<u8>>) {
        let at = self.player_pos();
        let mut msg = vec![0x0C];
        msg.extend(((at.x + 6) as u16).to_le_bytes());
        msg.extend((at.y as u16).to_le_bytes());
        self.link.send(SendQueue::Game, &msg).unwrap();
        let (mut got, mut most) = (Vec::new(), 0);
        for _ in 0..n {
            got.extend(self.ticks(1));
            most = most.max(
                self.link
                    .with(|l| {
                        let h = l.host_mut().game.events.action.hooks();
                        h.missiles.as_ref().map_or(0, |m| m.missiles().count())
                    })
                    .unwrap(),
            );
        }
        (most, got)
    }
}

// Covers: specs/skills/use.md §5
#[test]
fn fire_bolt_and_ice_bolt_cost_mana_and_fly() {
    for id in [FIRE_BOLT, ICE_BOLT] {
        let mut g = Game::joined(id);
        let (most, got) = g.cast(12);
        assert!(g.mana() < MANA, "{id}: mana; {}", g.errors());
        assert!(most > 0, "{id}: missile; {}", g.errors());
        assert!(got.iter().any(|m| m.contains(&0x4D)), "{id}: 0x4D");
    }
}

// Covers: specs/skills/bodies-2.md §3.2
#[test]
fn charged_bolt_makes_its_bolts() {
    let mut g = Game::joined(CHARGED_BOLT);
    let (most, _) = g.cast(12);
    assert!(g.mana() < MANA, "mana; {}", g.errors());
    assert!(most >= 3, "three bolts, got {most}; {}", g.errors());
}

// Covers: specs/skills/bodies.md §4.3
#[test]
fn frozen_armor_sets_its_state() {
    let mut g = Game::joined(FROZEN_ARMOR);
    let (_, got) = g.cast(12);
    assert!(g.has_state(STATE), "state; {}", g.errors());
    // q-states-auras: the toggle reaches the client (S→C 0xA8, the state).
    assert!(
        got.iter().any(|m| m.contains(&0xA8)),
        "0xA8 reaches the client: {got:?}"
    );
}

// Covers: specs/skills/bodies-2b.md §6.3
#[test]
fn enchant_sets_its_state() {
    let mut g = Game::joined(ENCHANT);
    g.cast(12);
    assert!(g.has_state(STATE + 1), "state; {}", g.errors());
}

// Covers: specs/skills/bodies-2b.md §6.5
#[test]
fn teleport_moves_the_caster() {
    let mut g = Game::joined(TELEPORT);
    let from = g.player_pos();
    g.cast(12);
    let to = g.player_pos();
    assert_ne!((from.x, from.y), (to.x, to.y), "moved; {}", g.errors());
    assert!(
        from.x.abs_diff(to.x) <= 8,
        "to the point, not across the map"
    );
}

// Covers: specs/skills/bodies.md §8.4
#[test]
fn nova_makes_a_ring() {
    let mut g = Game::joined(NOVA);
    let (most, _) = g.cast(12);
    assert!(most >= 8, "ring, got {most}; {}", g.errors());
}

// Covers: specs/skills/bodies.md §8.12
#[test]
fn blizzard_and_meteor_make_a_missile_at_the_point() {
    for id in [BLIZZARD, METEOR] {
        let mut g = Game::joined(id);
        let (most, _) = g.cast(12);
        assert!(most > 0, "{id}: missile; {}", g.errors());
    }
}

// Covers: specs/skills/bodies-2b.md §6.2
#[test]
fn fire_wall_makes_its_wall() {
    let mut g = Game::joined(FIRE_WALL);
    let (most, _) = g.cast(12);
    assert!(most >= 2, "wall, got {most}; {}", g.errors());
}
