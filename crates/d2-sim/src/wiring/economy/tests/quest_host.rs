//! The quest helpers' host seams on [`HostQuests`] (`quests.md` §9.1,
//! `quests-helpers.md` §4.2, §5, §7, `quests-act3.md` §6) over the action
//! wiring's fixture, with a recording inventory model.

use std::collections::BTreeMap;

use crate::items::flag;
use crate::items::moves::Spot;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::tests::{Fx, TestPending};
use crate::wiring::action::ActionHooks;
use crate::wiring::economy::{
    Economy, EconomyError, EconomyQuests, GameFields, HostQuests, ItemStore, QuestInventory,
};
use crate::wiring::interaction::tests::Rest;
use crate::wiring::inventory::tests::item_tables;
use crate::world::npc::InteractionList;
use crate::world::quests::QuestWorld;

/// A recording inventory model: `place` answers [`Inv::place`].
#[derive(Default)]
struct Inv {
    place: bool,
    log: Vec<String>,
    drops: Vec<(UnitId, Spot)>,
    update: Vec<u32>,
}

impl<H> QuestInventory<H> for Inv {
    fn place(&mut self, _: &mut Economy<'_, H>, player: UnitId, item: UnitId) -> bool {
        self.log.push(format!("place {} {}", player.0, item.0));
        self.place
    }
    fn drop_at(&mut self, _: &mut Economy<'_, H>, item: UnitId, spot: Spot) {
        self.drops.push((item, spot));
    }
    fn inventory_pass(&mut self, _: &mut Economy<'_, H>, player: UnitId) {
        self.log.push(format!("pass {}", player.0));
    }
    fn fault(&mut self, error: EconomyError) {
        panic!("{error:?}");
    }
    fn items_of(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn update_guids(&self, _: UnitId) -> Vec<u32> {
        self.update.clone()
    }
    fn cursor_of(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn weapon_in_use(&self, _: &Economy<'_, H>, _: UnitId) -> Option<UnitId> {
        None
    }
    fn delete(&mut self, _: &mut Economy<'_, H>, player: UnitId, item: UnitId) {
        self.log.push(format!("delete {} {}", player.0, item.0));
    }
}

type Host<'e, 'a> = HostQuests<'e, 'a, TestPending, Rest>;

/// Runs `f` on [`HostQuests`] over the fixture's economy (the game's item
/// store lent for the call and put back), with the parts given.
fn host<T>(
    fx: &mut Fx,
    rest: &mut Rest,
    inv: Option<&mut Inv>,
    chats: Option<&mut BTreeMap<UnitId, InteractionList>>,
    f: impl FnOnce(&mut Host<'_, '_>) -> T,
) -> T {
    let tables = item_tables();
    let s = &mut fx.sim.sys;
    let mut fields = GameFields::new(s.hooks.game_seed, true);
    let mut items = std::mem::replace(&mut s.hooks.items, ItemStore::new());
    let out = {
        let mut econ = Economy {
            game: &mut fx.game,
            units: &mut s.units,
            stats: &mut s.stats,
            data: &s.data,
            hooks: &mut s.hooks,
            fields: &mut fields,
            tables: &tables,
            items: &mut items,
        };
        let mut w = HostQuests::new(EconomyQuests::new(&mut econ, rest));
        w.inventory = inv.map(|i| i as &mut dyn QuestInventory<ActionHooks<TestPending>>);
        w.chats = chats;
        f(&mut w)
    };
    s.hooks.items = items;
    out
}

fn player(fx: &mut Fx) -> UnitId {
    let a = fx.a;
    fx.spawn(UnitType::Player, 0, a, 10, 10)
}

// Covers: specs/world/quests.md §9.1
#[test]
fn a_reward_the_inventory_takes_is_returned_identified() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let mut inv = Inv {
        place: true,
        ..Inv::default()
    };
    let item = host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.reward_item(p, *b"cap ", 0, 2, true)
    })
    .expect("placed");
    assert_eq!(inv.log, [format!("place {} {}", p.0, item.0)]);
    assert!(inv.drops.is_empty());
    let it = fx
        .sim
        .sys
        .hooks
        .items
        .get(item)
        .expect("in the game's store");
    assert_ne!(it.flags & flag::IDENTIFIED, 0);
    assert!(rest.log.is_empty(), "{:?}", rest.log);
    fx.assert_clean();
}

// Covers: specs/world/quests.md §9.1
#[test]
fn a_reward_with_no_free_spot_stays_unplaced() {
    let mut fx = Fx::new();
    let a = fx.a;
    // Far outside every room: the 100-ring search accepts nothing.
    let p = fx.spawn(UnitType::Player, 0, a, 100_000, 100_000);
    let mut rest = Rest::new();
    let mut inv = Inv::default();
    let item = host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.reward_item(p, *b"cap ", 0, 2, true)
    })
    .expect("returned");
    // No drop message, not freed, in no room.
    assert!(inv.drops.is_empty());
    assert!(fx.sim.sys.hooks.items.get(item).is_some());
    assert!(fx.sim.sys.units.get(item).is_some());
    assert_eq!(fx.game.lists.unit(item).and_then(|u| u.room()), None);
}

// Covers: specs/world/quests.md §9.1; specs/world/quests-helpers.md §1
#[test]
fn a_refused_reward_is_dropped_near_the_player_or_freed() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let mut inv = Inv::default();
    // Droppable: `0x00545340` from the player's position and room.
    let item = host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.reward_item(p, *b"cap ", 0, 2, true)
    })
    .expect("dropped");
    let [(dropped, spot)] = inv.drops[..] else {
        panic!("{:?}", inv.drops);
    };
    assert_eq!(dropped, item);
    assert!(spot.room == fx.a || spot.room == fx.b);
    assert!((spot.x - 10).abs() <= 100 && (spot.y - 10).abs() <= 100);
    assert!(fx.sim.sys.hooks.items.get(item).is_some());
    // Not droppable: freed, none returned.
    let mut inv = Inv::default();
    let out = host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.reward_item(p, *b"key ", 0, 2, false)
    });
    assert_eq!(out, None);
    assert!(inv.drops.is_empty());
    let freed: Vec<UnitId> = inv
        .log
        .iter()
        .map(|l| UnitId(l.rsplit(' ').next().unwrap().parse().unwrap()))
        .collect();
    assert_eq!(freed.len(), 1);
    assert!(fx.sim.sys.hooks.items.get(freed[0]).is_none());
    assert!(fx.sim.sys.units.get(freed[0]).is_none());
    // An unknown code: nothing is created or placed.
    let mut inv = Inv::default();
    let out = host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.reward_item(p, *b"zzz ", 0, 2, true)
    });
    assert_eq!(out, None);
    assert!(inv.log.is_empty());
    fx.assert_clean();
}

// Covers: specs/world/quests.md §9.1
#[test]
fn without_an_inventory_model_the_reward_is_the_rests() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let out = host(&mut fx, &mut rest, None, None, |w| {
        w.reward_item(p, *b"cap ", 0, 2, true)
    });
    assert_eq!(out, None);
    assert!(fx.sim.sys.hooks.items.is_empty());
}

// Covers: specs/world/quests-helpers.md §5 r2; specs/items/inventory.md §5.5
#[test]
fn closing_the_cube_runs_the_inventory_pass() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let mut inv = Inv::default();
    host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.close_cube(p)
    });
    assert_eq!(inv.log, [format!("pass {}", p.0)]);
    host(&mut fx, &mut rest, None, None, |w| w.close_cube(p));
    assert_eq!(rest.log, ["unhandled 254 0x567330"]);
}

// Covers: specs/world/quests-helpers.md §5 r2; specs/world/vendors-2.md §10.1
#[test]
fn the_trade_cancel_button_follows_the_dispatch() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let a = fx.a;
    let q = fx.spawn(UnitType::Player, 0, a, 12, 12);
    let q_guid = fx.sim.sys.units.get(q).unwrap().guid;
    let mut rest = Rest::new();
    let run = |fx: &mut Fx, rest: &mut Rest, interact: Option<(u8, u32)>, button: u8| {
        fx.sim.sys.units.get_mut(p).unwrap().interact.reset();
        if let Some((t, g)) = interact {
            fx.sim.sys.units.get_mut(p).unwrap().interact.set(t, g);
        }
        rest.sent.clear();
        host(fx, rest, None, None, |w| w.trade_button(p, button));
        rest.sent.iter().map(|m| m.1.clone()).collect::<Vec<_>>()
    };
    // Rule 2: no active interaction → 0x77 0x0C.
    assert_eq!(run(&mut fx, &mut rest, None, 6), [vec![0x77, 0x0C]]);
    // Rule 5: an interaction with a non-player → 0x77 0x0D.
    assert_eq!(run(&mut fx, &mut rest, Some((1, 5)), 6), [vec![0x77, 0x0D]]);
    // Rule 5: the partner gone, `0x00597A20` = 0 → 0x77 0x0C.
    assert_eq!(
        run(&mut fx, &mut rest, Some((0, 0xDEAD)), 6),
        [vec![0x77, 0x0C]]
    );
    // Rule 6: flags 2 bit 0x800000 (or 0x400000) → nothing sent.
    for bit in [0x80_0000, 0x40_0000] {
        fx.sim.sys.units.get_mut(p).unwrap().flags2 |= bit;
        assert!(run(&mut fx, &mut rest, Some((0, 0xDEAD)), 6).is_empty());
        fx.sim.sys.units.get_mut(p).unwrap().flags2 &= !bit;
    }
    assert_eq!(
        run(&mut fx, &mut rest, Some((0, 0xDEAD)), 6),
        [vec![0x77, 0x0C]]
    );
    // Rule 6: a live item on the inventory update list → nothing sent; a
    // dead GUID does not count.
    let mut inv = Inv {
        place: true,
        ..Inv::default()
    };
    let item = host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
        w.reward_item(p, *b"cap ", 0, 2, true)
    })
    .expect("item");
    let ig = fx.sim.sys.units.get(item).unwrap().guid;
    fx.sim.sys.units.get_mut(p).unwrap().interact.set(0, 0xDEAD);
    for (guid, sent) in [(ig, 0), (0x7777_7777, 1)] {
        inv.update = vec![guid];
        rest.sent.clear();
        host(&mut fx, &mut rest, Some(&mut inv), None, |w| {
            w.trade_button(p, 6)
        });
        assert_eq!(rest.sent.len(), sent);
    }
    // §10.3: button 6 with the partner → nothing.
    assert!(run(&mut fx, &mut rest, Some((0, q_guid)), 6).is_empty());
    assert!(rest.log.is_empty(), "{:?}", rest.log);
    // The cube and player-trade buttons stay their owners'.
    run(&mut fx, &mut rest, Some((0, q_guid)), 4);
    run(&mut fx, &mut rest, Some((4, 1)), 0x17);
    assert_eq!(
        rest.log,
        ["unhandled 254 0x568060", "unhandled 254 0x568060"]
    );
}

// Covers: specs/world/quests-helpers.md §5 r2; specs/world/quests-act5.md §5.7
#[test]
fn chat_nodes_are_freed_on_the_npcs_list() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let (n, other) = (UnitId(900), UnitId(901));
    let mut chats = BTreeMap::new();
    chats.insert(
        n,
        InteractionList {
            nodes: vec![(other, 0), (p, 1)],
        },
    );
    let mut rest = Rest::new();
    host(&mut fx, &mut rest, None, Some(&mut chats), |w| {
        w.free_chat_node(n, p);
        // A unit without a list (not an `interact` monster): nothing.
        w.free_chat_node(other, p);
    });
    assert_eq!(chats[&n].nodes, [(other, 0)]);
    host(&mut fx, &mut rest, None, Some(&mut chats), |w| {
        w.clear_npc_chats(n)
    });
    assert!(chats[&n].nodes.is_empty());
    assert!(rest.log.is_empty(), "{:?}", rest.log);
    // No lists lent: reported.
    host(&mut fx, &mut rest, None, None, |w| w.free_chat_node(n, p));
    assert_eq!(rest.log, ["unhandled 254 0x572e00"]);
}

// Covers: specs/world/quests-helpers.md §5 r2; specs/world/quests-act2-2.md §3.2, §3.3
#[test]
fn the_obelisk_close_is_the_insert_cancel() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let a = fx.a;
    let o = fx.spawn(UnitType::Object, 80, a, 14, 14);
    let g = fx.sim.sys.units.get(o).unwrap().guid.to_le_bytes();
    fx.sim.sys.units.get_mut(p).unwrap().interact.set(2, 7);
    let mut rest = Rest::new();
    host(&mut fx, &mut rest, None, None, |w| w.obelisk_close(p, o));
    assert_eq!(rest.sent, [(p, vec![0x58, g[0], g[1], g[2], g[3], 1, 0])]);
    assert_eq!(fx.sim.sys.units.get(p).unwrap().interact.get(), None);
}

// Covers: specs/world/quests-helpers.md §7
#[test]
fn the_town_portal_guid_is_player_data() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let a = fx.a;
    let o = fx.spawn(UnitType::Object, 59, a, 14, 14);
    let mut rest = Rest::new();
    let (gp, go) = host(&mut fx, &mut rest, None, None, |w| {
        (w.town_portal_guid(p), w.town_portal_guid(o))
    });
    // `Pending::object_portal_guid` (the fixture's default 0); an object
    // has no player data.
    assert_eq!((gp, go), (Some(0), None));
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}

// Covers: specs/world/quests-act3.md §6; specs/world/quests-helpers.md §4.3
#[test]
fn the_orb_mode_request_runs_the_monster_mode_change() {
    let mut fx = Fx::new();
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, 10, 10);
    fx.sim.sys.units.get_mut(m).unwrap().mode = 2;
    let p = player(&mut fx);
    let mut rest = Rest::new();
    host(&mut fx, &mut rest, None, None, |w| {
        w.monster_mode_at(m, 1, 0, 0);
        // Not a monster: reported.
        w.monster_mode_at(p, 0, 0, 0);
    });
    assert_eq!(fx.sim.sys.units.get(m).unwrap().mode, 1);
    assert_eq!(rest.log, ["unhandled 255 0x5ddfc0"]);
}

// Covers: specs/world/quests-helpers.md §4.2
#[test]
fn the_path_target_needs_the_path_provider() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let t = host(&mut fx, &mut rest, None, None, |w| w.path_target_xy(p));
    assert_eq!(t, None);
    assert_eq!(rest.log, ["unhandled 254 0x56d2c0"]);
}

// -- Act IV / V seams (`quests-act4.md`, `quests-act5.md`,
// `quests-act5-2.md`) on the action wiring.

// Covers: specs/world/quests-act5.md §4.6, §4.7
#[test]
fn the_act5_unit_reads_come_from_the_lists() {
    let mut fx = Fx::new();
    let a = fx.a;
    let m1 = fx.spawn(UnitType::Monster, 0, a, 10, 10);
    let m2 = fx.spawn(UnitType::Monster, 0, a, 13, 10);
    fx.sim.sys.units.get_mut(m1).unwrap().mode = 12;
    let mut rest = Rest::new();
    let (mode, dist, back, units) = host(&mut fx, &mut rest, None, None, |w| {
        (
            w.unit_mode(m1),
            w.distance_between(m1, m2),
            w.distance_between(m2, m1),
            w.adjacent_units(a),
        )
    });
    assert_eq!(mode, 12);
    assert!(dist.is_some_and(|d| d >= 0), "{dist:?}");
    assert_eq!(dist, back);
    // The room itself is in its adjacency array; list order kept.
    let i1 = units.iter().position(|&u| u == m1).expect("m1 listed");
    let i2 = units.iter().position(|&u| u == m2).expect("m2 listed");
    assert_ne!(i1, i2);
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}

// Covers: specs/world/quests-act5.md §1.1, §5.7; specs/world/quests-act5-2.md §7.6
#[test]
fn killed_in_place_and_removed_ancients_leave_the_lists() {
    let mut fx = Fx::new();
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, 10, 10);
    let n = fx.spawn(UnitType::Monster, 0, a, 12, 12);
    let mut rest = Rest::new();
    host(&mut fx, &mut rest, None, None, |w| {
        w.kill_in_place(m);
        w.remove_ancient(n);
    });
    assert!(fx.game.lists.unit(m).is_none());
    assert!(fx.game.lists.unit(n).is_none());
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}

// Covers: specs/world/quests-act5.md §5.7 (open question 3: lists stack)
#[test]
fn the_resist_list_adds_the_four_resists_and_stacks() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let after = |fx: &Fx| [39u16, 41, 43, 45].map(|s| fx.sim.sys.stats.unit_total(p, s, 0));
    host(&mut fx, &mut rest, None, None, |w| w.add_resist_list(p, 10));
    assert_eq!(after(&fx), [10; 4]);
    host(&mut fx, &mut rest, None, None, |w| w.add_resist_list(p, 20));
    assert_eq!(after(&fx), [30; 4]);
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}

// Covers: specs/world/quests-act5-2.md §6.8
#[test]
fn a_waypoint_is_active_once_activated() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    let (before, after) = host(&mut fx, &mut rest, None, None, |w| {
        let before = w.waypoint_active(p, 123);
        let d = w.difficulty();
        w.activate_waypoint(p, 123, d);
        (before, w.waypoint_active(p, 123))
    });
    // No records yet: not active.
    assert!(!before);
    assert!(after);
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}

// Covers: specs/world/quests-act5-2.md §7.7; specs/combat/vitals.md §3, §4.1
#[test]
fn the_experience_reads_are_the_vitals_tables() {
    let mut fx = Fx::new();
    let p = player(&mut fx);
    let mut rest = Rest::new();
    // Without the vitals tables: the rest's.
    host(&mut fx, &mut rest, None, None, |w| w.max_level(p));
    assert_eq!(rest.log, ["unhandled 254 0x611830"]);
    rest.log.clear();
    fx.sim.sys.hooks.vitals = Some(std::sync::Arc::new(
        crate::wiring::action::tests::fight::vitals(),
    ));
    let (max, t1, t2) = host(&mut fx, &mut rest, None, None, |w| {
        (
            w.max_level(p),
            w.experience_threshold(p, 1),
            w.experience_threshold(p, 2),
        )
    });
    assert_eq!((max, t1, t2), (3, 100, 1500));
    // 100 experience reaches level 2 (`vitals.md` §3).
    fx.sim
        .sys
        .stats
        .unit_set(&mut fx.sim.sys.hooks, p, 13, 100, 0);
    host(&mut fx, &mut rest, None, None, |w| w.level_up(p));
    assert_eq!(fx.sim.sys.stats.unit_base(p, 12, 0), 2);
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}

// Covers: specs/world/quests-act5-2.md §8.8 (open question 4)
#[test]
fn the_zoo_reads_the_monstats_zoo_column() {
    let mut fx = Fx::new();
    let mut rest = Rest::new();
    let (rows, zoo0, past) = host(&mut fx, &mut rest, None, None, |w| {
        (w.monstats_rows(), w.zoo_eligible(0), w.zoo_eligible(99))
    });
    assert_eq!((rows, zoo0, past), (1, false, false));
    std::sync::Arc::make_mut(&mut fx.sim.sys.hooks.tables)
        .combat
        .monstats[0]
        .zoo = true;
    assert!(host(&mut fx, &mut rest, None, None, |w| w.zoo_eligible(0)));
    assert!(rest.log.is_empty(), "{:?}", rest.log);
}
