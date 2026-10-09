// Spec: specs/world/quests-act5.md; specs/world/quests-act5-2.md; specs/tools/poke.md §1
//! Act V played on the user's install (q-play-act5): the play app's own
//! game and client, headless, a barbarian moved to Harrogath by the act
//! change queue, then each Act V quest driven through the server the way
//! a player reaches it (talk, warp to the quest's level with a poke,
//! attack, operate), checking the quest records, units, warps and the
//! messages the client gets. The pokes and the strengthening are test
//! staging (`specs/tools/poke.md`), not game rules.

use bevy::prelude::*;
use d2_client::app::single_player;
use d2_sim::stats::stat;
use d2_sim::units::{UnitId, UnitType};

mod app_support;
mod real_rig;
use real_rig::Rig;

/// Harrogath.
const HARROGATH: u32 = single_player::ACT5_TOWN;

struct Act5 {
    rig: Rig,
}

impl Act5 {
    /// A barbarian in Harrogath, strong enough to kill a boss in a few
    /// swings.
    fn new() -> Act5 {
        // Attack (skill 0) and the barbarian's Bash (126).
        let mut rig = Rig::new("barbarian", &[0, 126]);
        if std::env::var_os("PLAY_DEBUG").is_some() {
            let b = &rig
                .app
                .world()
                .resource::<d2_client::bridge::BridgeResource>()
                .0;
            eprintln!("after join: {:?}", b.log().rejected);
        }
        // Both hands on Attack, as a new character's (the rig's list
        // reset leaves them unset).
        rig.with(|sim, p| {
            let list = sim
                .events
                .action
                .sys
                .hooks
                .skill_lists
                .get_mut(&p)
                .expect("list");
            let i = list
                .view()
                .iter()
                .position(|e| e.skill == 0)
                .expect("attack");
            list.left = Some(i);
            list.right = Some(i);
        });
        rig.with(|sim, p| {
            sim.events
                .action
                .hooks()
                .act_changes
                .push((p, HARROGATH, 0));
        });
        for _ in 0..400 {
            if rig.level() == Some(HARROGATH) {
                break;
            }
            rig.step(1);
        }
        assert_eq!(
            rig.level(),
            Some(HARROGATH),
            "the act change reaches Harrogath"
        );
        rig.step(30);
        let mut a = Act5 { rig };
        a.strengthen();
        a
    }

    /// Life, level and to-hit high enough for the Act V bosses (staging).
    fn strengthen(&mut self) {
        self.rig.with(|sim, p| {
            sim.events.action.with(&mut sim.game, |_, v| {
                for st in [stat::HITPOINTS, stat::MAXHP] {
                    v.set_base(p, st, 30000 << 8);
                }
                v.set_base(p, stat::LEVEL, 90);
                v.set_base(p, 19, 30000);
            });
        });
    }

    /// Runs one poke directive now (`d2-client play --poke`).
    fn poke(&mut self, directive: &str) -> String {
        let e = d2_client::app::poke::parse_poke_arg(&format!("1 {directive}")).expect("poke");
        self.rig.with(move |sim, _| {
            let r = d2_client::app::poke::apply_now(sim, &e.op);
            r.code().to_string()
        })
    }

    /// Warps to `level` (poke `warp`) and lets the rooms come in.
    fn warp(&mut self, level: u32) {
        let r = self.poke(&format!("warp {level}"));
        assert_eq!(r, "ok", "warp {level}");
        for _ in 0..200 {
            if self.rig.level() == Some(level) {
                break;
            }
            self.rig.step(1);
        }
        assert_eq!(self.rig.level(), Some(level), "warped into {level}");
        self.rig.step(20);
    }

    /// (state, status) of quest chain `n`.
    fn chain(&mut self, n: u8) -> Option<(u8, u8)> {
        self.rig.with(move |sim, _| {
            let r = sim.world.quests.record(n)?;
            Some((r.state, r.status))
        })
    }

    /// The player's quest flag `slot`.`bit` on Normal.
    fn flag(&mut self, slot: u8, bit: u8) -> bool {
        self.rig.with(move |sim, p| {
            sim.world
                .rest
                .quests
                .get(&p)
                .is_some_and(|q| q.flags[0].get(slot, bit))
        })
    }

    /// Units of type `ty` and a class of `classes`, nearest first.
    fn units(&mut self, ty: UnitType, classes: &[u32]) -> Vec<(UnitId, u32, (i32, i32))> {
        let classes = classes.to_vec();
        self.rig.with(move |sim, p| {
            let a = &mut sim.events.action;
            let at = a.sys.hooks.path_position(p);
            let mut v: Vec<_> = sim
                .game
                .lists
                .units_of_type(ty)
                .into_iter()
                .filter_map(|u| {
                    let class = a.sys.units.get(u)?.class;
                    classes.contains(&class).then(|| {
                        let guid = sim.game.lists.unit(u).map_or(0, |e| e.guid);
                        (u, guid, a.sys.hooks.path_position(u))
                    })
                })
                .collect();
            v.sort_by_key(|&(_, _, q)| test_fixtures::host::cheb(at, q));
            v
        })
    }

    /// Puts the player at the first free spot of a small ring around
    /// `(x, y)` (poke `pos`, which reaches the player's room and its
    /// neighbours). False: none.
    fn hop(&mut self, x: i32, y: i32) -> bool {
        for r in [0, 2, 4, 7] {
            for (dx, dy) in [
                (r, r),
                (r, 0),
                (0, r),
                (-r, 0),
                (0, -r),
                (-r, -r),
                (r, -r),
                (-r, r),
            ] {
                if self.poke(&format!("pos @player {} {}", x + dx, y + dy)) == "ok" {
                    self.rig.step(3);
                    return true;
                }
            }
        }
        false
    }

    /// Moves the player next to `(x, y)` in hops of at most 16 sub-tiles
    /// (staging: the rooms on the way come into play as on foot).
    fn stand_by(&mut self, (x, y): (i32, i32)) {
        for _ in 0..200 {
            let (px, py) = self.rig.pos();
            let (dx, dy) = (x + 2 - px, y + 2 - py);
            if dx.abs() <= 3 && dy.abs() <= 3 {
                return;
            }
            let (sx, sy) = (dx.clamp(-16, 16), dy.clamp(-16, 16));
            if !self.hop(px + sx, py + sy) && !self.hop(px + sx / 2, py + sy / 2) {
                // Blocked straight on: sidestep.
                if !self.hop(px + sy.signum() * 8, py - sx.signum() * 8) {
                    panic!("stuck at ({px}, {py}) on the way to ({x}, {y})");
                }
            }
        }
        panic!("did not reach ({x}, {y})");
    }

    /// The centres of the player's level's rooms, nearest first.
    fn level_rooms(&mut self) -> Vec<(i32, i32)> {
        let p = self.rig.pos();
        let mut v = self.rig.with(|sim, p| {
            let h = sim.events.action.hooks();
            let room = sim.game.lists.unit(p).and_then(|e| e.room());
            let lvl = room.and_then(|r| h.drlg.level_id(&sim.game, r)).unwrap();
            let d = h.drlg.dungeon.acts[4].as_ref().unwrap();
            let l = d.find_level(lvl).unwrap();
            d.level_rooms(l)
                .into_iter()
                .map(|r| {
                    let t = d.room(r).rect;
                    ((t.x * 2 + t.w) * 5 / 2, (t.y * 2 + t.h) * 5 / 2)
                })
                .collect::<Vec<_>>()
        });
        v.sort_by_key(|&c| test_fixtures::host::cheb(c, p));
        v
    }

    /// Visits the level's rooms until a unit of `ty` / `classes` exists;
    /// then stands next to it. Its id, GUID and position.
    fn find(&mut self, ty: UnitType, classes: &[u32]) -> (UnitId, u32, (i32, i32)) {
        let mut found = self.units(ty, classes);
        if found.is_empty() {
            for c in self.level_rooms() {
                self.stand_by(c);
                self.rig.step(2);
                found = self.units(ty, classes);
                if !found.is_empty() {
                    break;
                }
            }
        }
        let &(u, guid, at) = found
            .first()
            .unwrap_or_else(|| panic!("unit {ty:?} {classes:?} in the level"));
        self.stand_by(at);
        self.rig.step(10);
        (u, guid, self.units(ty, classes).first().map_or(at, |e| e.2))
    }

    /// Attacks the monster (C→S 0x06, the left skill on a unit) until it
    /// dies; its life is lowered first so a few swings do (staging).
    fn kill(&mut self, m: UnitId, guid: u32) {
        self.rig.with(move |sim, _| {
            sim.events.action.with(&mut sim.game, |_, v| {
                v.set_base(m, stat::HITPOINTS, 1 << 8);
            });
        });
        for _ in 0..40 {
            if self.dead(m) {
                break;
            }
            let at = self
                .rig
                .with(move |sim, _| sim.events.action.sys.hooks.path_position(m));
            self.stand_by(at);
            self.strengthen();
            let mut msg = vec![0x06];
            msg.extend(1u32.to_le_bytes());
            msg.extend(guid.to_le_bytes());
            self.rig.send(&msg);
            self.rig.step(25);
            if std::env::var_os("PLAY_DEBUG").is_some() {
                let hp = self.rig.life(m);
                let pm = self.rig.mode();
                let mm = self
                    .rig
                    .with(move |sim, _| sim.events.action.sys.units.get(m).map(|u| u.mode));
                let pos = self.rig.pos();
                let me = self.rig.player();
                let php = self.rig.life(me);
                eprintln!("player hp {php}");
                eprintln!(
                    "kill {guid}: hp {hp} mode {mm:?} at {at:?}; player mode {pm} at {pos:?}; {}",
                    self.rig.errors()
                );
            }
        }
        assert!(self.dead(m), "monster {guid} killed");
    }

    fn dead(&mut self, m: UnitId) -> bool {
        self.rig.with(move |sim, _| {
            sim.events
                .action
                .sys
                .units
                .get(m)
                .is_none_or(|u| u.mode == 0 || u.mode == 12)
                || sim.game.lists.unit(m).is_none()
        })
    }

    /// The client's model holds a unit `(type, guid)`.
    fn client_has(&self, ty: u8, guid: u32) -> bool {
        self.rig
            .app
            .world()
            .resource::<d2_client::bridge::BridgeResource>()
            .0
            .world()
            .units
            .keys()
            .any(|k| k.unit_type == ty && k.guid == guid)
    }

    fn rejected(&self) -> Vec<String> {
        let b = &self
            .rig
            .app
            .world()
            .resource::<d2_client::bridge::BridgeResource>()
            .0;
        b.log().rejected.iter().map(|r| format!("{r:?}")).collect()
    }
}

// Covers: specs/world/quests-act5.md §2
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn harrogath_holds_its_npcs_and_the_act_v_records() {
    let mut a = Act5::new();
    for n in 31..=36 {
        assert!(a.chain(n).is_some(), "chain {n} exists");
    }
    // Larzuk, Anya (in town only after her rescue), Malah, Nihlathak,
    // Qual-Kehk, Cain.
    for class in [511u32, 513, 514, 515, 520] {
        let (_, guid, _) = a.find(UnitType::Monster, &[class]);
        assert!(a.client_has(1, guid), "the client holds NPC {class}");
    }
    assert!(a.rejected().is_empty(), "{:?}", a.rejected());
}

// Covers: specs/world/quests-act5.md §3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn shenk_dies_and_the_siege_completes() {
    let mut a = Act5::new();
    a.warp(110);
    // Shenk (superunique 42) stands at the siege's end of the Bloody
    // Foothills: walk the level's rooms until his preset spawns.
    let shenk_class = {
        let d = app_support::live();
        let rows: Vec<d2_data::tables::Superuniques> = d.tables.rows().expect("superuniques");
        rows[42].class
    };
    let (shenk, guid, _) = a.find(UnitType::Monster, &[shenk_class]);
    assert!(a.client_has(1, guid), "the client sees Shenk");
    a.kill(shenk, guid);
    a.rig.step(30);
    let c = a.chain(31);
    assert!(
        a.flag(35, 1) || a.flag(35, 0),
        "Siege quest flags after Shenk: {c:?}"
    );
    // Back to Larzuk: the reward (35.0) is given at his chat.
    a.warp(HARROGATH);
    assert!(a.rejected().is_empty(), "{:?}", a.rejected());
}

// Covers: specs/world/quests-act5.md §4.6, §4.7, §4.10
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_caged_barbarians_are_rescued() {
    let mut a = Act5::new();
    a.warp(111);
    // Walk the Frigid Highlands' rooms; at each cage (object 473) break
    // its door (monster 434) and wait for its five barbarians to leave.
    let mut done: Vec<(i32, i32)> = Vec::new();
    for c in a.level_rooms() {
        if done.len() == 3 {
            break;
        }
        a.stand_by(c);
        a.rig.step(2);
        let cages: Vec<_> = a
            .units(UnitType::Object, &[473])
            .into_iter()
            .filter(|k| !done.iter().any(|&d| test_fixtures::host::cheb(d, k.2) < 10))
            .collect();
        let Some(&(_, _, at)) = cages.first() else {
            continue;
        };
        done.push(at);
        a.stand_by(at);
        a.rig.step(5);
        let near = |a: &mut Act5, class: u32| -> Vec<(UnitId, u32, (i32, i32))> {
            a.units(UnitType::Monster, &[class])
                .into_iter()
                .filter(|d| test_fixtures::host::cheb(d.2, at) < 20)
                .collect()
        };
        assert_eq!(
            near(&mut a, 534).len(),
            5,
            "five barbarians at the cage {at:?}"
        );
        let &(door, guid, _) = near(&mut a, 434).first().expect("the cage's door");
        a.kill(door, guid);
        // The barbarians see the open door and a portal opens by it.
        a.rig.step(60);
        // Out of their way (the player standing by the portal blocks it).
        let me = a.rig.pos();
        a.stand_by((me.0 + 12, me.1 + 12));
        // They walk out.
        for _ in 0..60 {
            a.strengthen();
            a.rig.step(25);
            if std::env::var_os("PLAY_DEBUG").is_some() {
                let dm = a
                    .rig
                    .with(move |sim, _| sim.events.action.sys.units.get(door).map(|u| u.mode));
                let portals: Vec<_> = a
                    .units(UnitType::Object, &[189])
                    .iter()
                    .map(|o| o.2)
                    .collect();
                let bpos: Vec<_> = near(&mut a, 534).iter().map(|b| b.2).collect();
                eprintln!("  portal at {portals:?}, barbarians at {bpos:?}");
                let barbs: Vec<_> = near(&mut a, 534)
                    .iter()
                    .map(|b| {
                        let u = b.0;
                        a.rig
                            .with(move |sim, _| sim.events.action.sys.units.get(u).map(|r| r.mode))
                    })
                    .collect();
                let rescue = a
                    .rig
                    .with(|sim, _| sim.events.action.sys.hooks.x.rescue.len());
                let p = a.rig.pos();
                eprintln!("cage {at:?} me {p:?}: door {dm:?} portals {portals:?} barbs {barbs:?} published {rescue} chain {:?}", a.chain(32));
            }
            if near(&mut a, 534).is_empty() {
                break;
            }
        }
        assert!(
            near(&mut a, 534).is_empty(),
            "the cage's barbarians left; chain 32 {:?}",
            a.chain(32)
        );
    }
    assert_eq!(done.len(), 3, "three cages in the Frigid Highlands");
    assert!(
        a.flag(36, 1),
        "rescue done (36.1); chain 32 {:?}",
        a.chain(32)
    );
    assert!(a.rejected().is_empty(), "{:?}", a.rejected());
}

impl Act5 {
    /// C→S 0x13: interact with the unit (type, GUID).
    fn interact(&mut self, ty: u32, guid: u32) {
        let mut m = vec![0x13];
        m.extend(ty.to_le_bytes());
        m.extend(guid.to_le_bytes());
        self.rig.send(&m);
        self.rig.step(10);
    }

    /// C→S 0x31: the NPC's scroll message `msg` ends (`quests.md` §4).
    fn quest_message(&mut self, npc: u32, msg: u32) {
        let mut m = vec![0x31];
        m.extend(npc.to_le_bytes());
        m.extend(msg.to_le_bytes());
        self.rig.send(&m);
        self.rig.step(10);
    }

    /// Talks to the NPC of `class` in the player's level: walks to it,
    /// interacts, then sends each scroll message of `msgs`. Its GUID.
    fn talk(&mut self, class: u32, msgs: &[u32]) -> u32 {
        let (_, guid, _) = self.find(UnitType::Monster, &[class]);
        self.interact(1, guid);
        for &m in msgs {
            self.quest_message(guid, m);
        }
        // C→S 0x30: end of the chat.
        let mut m = vec![0x30];
        m.extend(1u32.to_le_bytes());
        m.extend(guid.to_le_bytes());
        self.rig.send(&m);
        self.rig.step(10);
        guid
    }

    /// The client model holds an item of `code` owned by the player
    /// (`bridge::items::local_items`).
    fn holds(&mut self, code: [u8; 4]) -> bool {
        let w = self
            .rig
            .app
            .world()
            .resource::<d2_client::bridge::BridgeResource>()
            .0
            .world();
        d2_client::bridge::items::local_items(w)
            .iter()
            .any(|i| i.code == Some(code))
    }
}

// Covers: specs/world/quests-act5.md §5.4, §5.6, §5.7
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn anya_is_thawed_and_returns_to_harrogath() {
    let mut a = Act5::new();
    // Malah starts the quest and gives the thawing potion.
    a.talk(513, &[20116, 20127]);
    assert!(
        a.holds(*b"ice "),
        "Malah's potion; chain 33 {:?}",
        a.chain(33)
    );
    // Frozen Anya (object 558) in the Frozen River.
    a.warp(114);
    let (_, statue, _) = a.find(UnitType::Object, &[558]);
    a.interact(2, statue);
    a.rig.step(60);
    assert!(
        a.flag(37, 1),
        "Anya freed (37.1); chain 33 {:?}",
        a.chain(33)
    );
    assert!(!a.holds(*b"ice "), "the potion is used");
    // Back in town: Malah's scroll, Anya in town, her item.
    a.warp(HARROGATH);
    a.talk(513, &[20132]);
    assert!(a.holds(*b"tr2 "), "the Scroll of Resistance");
    a.rig.step(40);
    a.talk(512, &[20136]);
    assert!(
        a.flag(37, 0),
        "Prison of Ice done (37.0); chain 33 {:?}",
        a.chain(33)
    );
    assert!(a.rejected().is_empty(), "{:?}", a.rejected());
}

// Covers: specs/monsters/population.md §11.3; specs/world/quests-act5-2.md §8.8
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn baal_sits_on_his_throne() {
    let mut a = Act5::new();
    a.warp(131);
    let (_, guid, at) = a.find(UnitType::Monster, &[543]);
    assert!(a.client_has(1, guid), "the client sees Baal at {at:?}");
    let portal = a.units(UnitType::Object, &[563]);
    assert!(!portal.is_empty(), "the Worldstone Chamber portal");
}

// Covers: specs/world/quests-act5.md §5.5 r3, §5.9; specs/monsters/population.md §11.4
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn nihlathak_waits_in_the_halls_of_vaught() {
    let mut a = Act5::new();
    a.warp(124);
    let (_, guid, at) = a.find(UnitType::Monster, &[526]);
    assert!(a.client_has(1, guid), "the client sees Nihlathak at {at:?}");
}
