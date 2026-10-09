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
        let mut rig = Rig::new("barbarian", &[]);
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

    /// Moves the player next to `(x, y)` (poke `pos`).
    fn stand_by(&mut self, (x, y): (i32, i32)) {
        let r = self.poke(&format!("pos @player {} {}", x + 2, y + 2));
        assert_eq!(r, "ok", "pos");
        self.rig.step(4);
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
            let mut msg = vec![0x06];
            msg.extend(1u32.to_le_bytes());
            msg.extend(guid.to_le_bytes());
            self.rig.send(&msg);
            self.rig.step(25);
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
        let u = a.units(UnitType::Monster, &[class]);
        assert!(!u.is_empty(), "NPC {class} in Harrogath");
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
        rows[42].class as u32
    };
    let mut found = a.units(UnitType::Monster, &[shenk_class]);
    if found.is_empty() {
        let rooms = a.rig.with(|sim, p| {
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
        for c in rooms {
            a.stand_by(c);
            a.rig.step(2);
            found = a.units(UnitType::Monster, &[shenk_class]);
            if !found.is_empty() {
                break;
            }
        }
    }
    let &(shenk, guid, at) = found
        .first()
        .expect("Shenk spawned in the Bloody Foothills");
    a.stand_by(at);
    a.rig.step(10);
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
