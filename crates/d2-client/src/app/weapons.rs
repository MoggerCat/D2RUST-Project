// Spec: specs/skills/bodies.md §2.3–§2.5 (hand class, ammo, bow missile); specs/items/inventory.md §1.2 (body locations)
//! The equipped weapon and ammo of the players, as the skill pipeline
//! reads them, for the play host (q-amazon).
//!
//! The skill bodies of the bow and javelin skills (`srvst 4` ammo check,
//! `srvdo 8` Multiple Shot, the bow missile pick, the ammo take) ask the
//! unit for its weapon in use, the item on a body location, the hand class
//! and the item's `shoots` / stack facts. The action wiring has no item
//! provider (`Pending`'s item seams are defaults), and the inventory model
//! is the wired host's, not the sim's. [`sync`] copies the facts from the
//! inventory model into [`Weapons`] (on [`LocalSeams`]) at each tick and
//! before each intent; the seams answer from the copy.
//!
//! Preview fills (`// d2rs-own, unverified`, REC-150): the hand class comes
//! from the item's type (the `wclass` column is not in the inventory's
//! table projection): `bow` 1, `xbow` 7, `jave` / `ajav` 3 (one-hand
//! thrust), any other `weap` 2 (one-hand swing), else 0 (hand to hand;
//! the numbering is the original's, `unit-composite.md` §2.1).
//! The weapon in use is the right-hand item (body location 4) with a
//! hand class; the skills, not combat, see it (`UseRest::skill_weapon`),
//! because the preview's equipped items do not feed the player's damage
//! stats; q-weapon-combat links the worn items' stat lists to the wearer
//! (`d2-sim` `InvDesk::link_item_stats`), so combat reads the same weapon
//! ([`LocalSeams`]'s `Pending::current_weapon`, `wield_type`).
//! d2rs-own, unverified (REC-158): the grip is 2 for a two-handed base
//! item (`0x0063D340` is not written for the preview).

use std::collections::BTreeMap;

use d2_server::adapters::handlers::world::WiredWorld;
use d2_sim::game::Game;
use d2_sim::items::inventory::tables::InvTables;
use d2_sim::items::inventory::{body, mode};
use d2_sim::units::UnitId;
use d2_sim::wiring::worldgen::WorldSim;

use super::single_player::LocalSeams;

/// Hand classes (`0x00623C60`; the numbering of the composit weapon
/// classes: 0 hand to hand, 1 bow, 3 one-hand thrust, 7 crossbow).
pub mod class {
    pub const HAND_TO_HAND: i32 = 0;
    pub const BOW: i32 = 1;
    pub const ONE_HAND_SWING: i32 = 2;
    pub const ONE_HAND_THRUST: i32 = 3;
    pub const CROSSBOW: i32 = 7;
}

/// What the skill pipeline asks of one item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemFacts {
    /// The `itemtypes` rows the item is of (its own and its parents').
    pub types: Vec<i16>,
    /// The hand class when held in the right hand.
    pub class: i32,
    /// The type has a `shoots` value (bows, crossbows).
    pub shoots: bool,
    pub stackable: bool,
    pub max_stack: i32,
    /// The grip (`0x0063D340`): 2 for a two-handed base item, else 1.
    pub grip: i32,
    /// A shield (type `shld`), with its armor `mindam` / `maxdam`
    /// (d2rs-own, unverified: q-skill-gaps, REC-176).
    pub shield: bool,
    pub dam: (i32, i32),
    /// `component` (items), `wclass` / `2handedwclass` and "of type 45
    /// `weap`", for the COF weapon class (`unit-composite.md` §2.1).
    pub component: u8,
    pub wclass: [u8; 4],
    pub wclass2: [u8; 4],
    pub weap: bool,
}

/// A player's hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hands {
    pub right: Option<UnitId>,
    pub left: Option<UnitId>,
    /// The weapon in use: the right-hand item with a hand class.
    pub weapon: Option<UnitId>,
    /// The COF weapon class (`0x0064F380`, `render/unit-composite.md`
    /// §2.1; the `sequences.md` class index 0–13).
    pub cof: i32,
}

/// The copy of the inventory model's weapon facts ([`sync`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Weapons {
    pub hands: BTreeMap<UnitId, Hands>,
    pub items: BTreeMap<UnitId, ItemFacts>,
}

impl Weapons {
    pub fn weapon(&self, u: UnitId) -> Option<UnitId> {
        self.hands.get(&u)?.weapon
    }

    /// The item on body location 4 (right hand) or 5 (left hand).
    pub fn item_at(&self, u: UnitId, loc: u8) -> Option<UnitId> {
        let h = self.hands.get(&u)?;
        match loc {
            body::RIGHT_HAND => h.right,
            body::LEFT_HAND => h.left,
            _ => None,
        }
    }

    /// The COF weapon class (`0x0064F380`); `hth` (0) without hands.
    pub fn cof_class(&self, u: UnitId) -> i32 {
        self.hands.get(&u).map_or(0, |h| h.cof)
    }

    /// The hand class of the weapon in use (0 without one).
    pub fn hand_class(&self, u: UnitId) -> i32 {
        self.weapon(u)
            .and_then(|w| self.items.get(&w))
            .map_or(class::HAND_TO_HAND, |f| f.class)
    }

    /// The shield in the left hand (`0x0063C8F0`).
    pub fn shield(&self, u: UnitId) -> Option<UnitId> {
        let l = self.hands.get(&u)?.left?;
        self.items.get(&l)?.shield.then_some(l)
    }

    pub fn facts(&self, item: UnitId) -> ItemFacts {
        self.items.get(&item).cloned().unwrap_or_default()
    }

    /// "Item is of type `itype`" (`0x00629BB0`).
    pub fn item_is(&self, item: UnitId, itype: i32) -> bool {
        self.items
            .get(&item)
            .is_some_and(|f| i16::try_from(itype).is_ok_and(|t| f.types.contains(&t)))
    }
}

/// The `itemtypes` rows whose code is one of `codes` (trailing spaces and
/// NULs of the 4-byte code ignored).
fn type_rows(t: &InvTables, codes: &[&[u8]]) -> Vec<i16> {
    t.itemtypes
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            let c = r.code.split(|&b| b == 0 || b == b' ').next().unwrap_or(&[]);
            codes.contains(&c)
        })
        .map(|(i, _)| i as i16)
        .collect()
}

fn is_any(t: &InvTables, record: usize, rows: &[i16]) -> bool {
    rows.iter().any(|&r| t.is_type(record, r))
}

/// The facts of an item of items-row `record`.
// d2rs-own, unverified (REC-150): see the module docs.
fn facts_of(t: &InvTables, record: usize) -> ItemFacts {
    let bow = type_rows(t, &[b"bow"]);
    let xbow = type_rows(t, &[b"xbow"]);
    let jave = type_rows(t, &[b"jave", b"ajav"]);
    let weap = type_rows(t, &[b"weap"]);
    let shld = type_rows(t, &[b"shld"]);
    let (class, shoots) = if is_any(t, record, &bow) {
        (class::BOW, true)
    } else if is_any(t, record, &xbow) {
        (class::CROSSBOW, true)
    } else if is_any(t, record, &jave) {
        (class::ONE_HAND_THRUST, false)
    } else if is_any(t, record, &weap) {
        (class::ONE_HAND_SWING, false)
    } else {
        (class::HAND_TO_HAND, false)
    };
    let grip = if t.item(record).is_some_and(|r| r.twohanded != 0) {
        2
    } else {
        1
    };
    let (stackable, max_stack) = t
        .item(record)
        .map_or((false, 0), |r| (r.stackable != 0, r.maxstack as i32));
    let types = (0..t.itemtypes.len() as i16)
        .filter(|&row| t.is_type(record, row))
        .collect();
    ItemFacts {
        types,
        class,
        shoots,
        stackable,
        max_stack,
        grip,
        shield: is_any(t, record, &shld),
        dam: t
            .item(record)
            .map_or((0, 0), |r| (i32::from(r.mindam), i32::from(r.maxdam))),
        component: t.item(record).map_or(0, |r| r.component),
        wclass: t.item(record).map_or([0; 4], |r| r.wclass),
        wclass2: t.item(record).map_or([0; 4], |r| r.wclass2),
        weap: is_any(t, record, &weap),
    }
}

/// The index of a COF weapon class code (`sequences::CLASSES`; trailing
/// spaces and NULs ignored); unknown → `hth` (0).
fn class_index(code: &[u8; 4]) -> i32 {
    let c = code.split(|&b| b == 0 || b == b' ').next().unwrap_or(&[]);
    d2_sim::skills::sequences::CLASSES
        .iter()
        .position(|k| k.as_bytes() == c)
        .map_or(0, |i| i as i32)
}

/// The type class `0x00629FE0` of r-dual (`unit-composite.md` §2.1): the
/// `wclass` in the table `0x007446A0`, else 0.
fn type_class(f: &ItemFacts) -> i32 {
    match class_index(&f.wclass) {
        c @ (1..=7 | 12) => c,
        _ => 0,
    }
}

/// `0x0064F380` for a player outside modes DT / DD (`unit-composite.md`
/// §2.1): `class` is the player class.
// PROVISIONAL (q-fix-real-item-type-test): every 1.14d `charstats`
// weapon class is `hth` (§2.1), so no hand item answers 0; the item
// validity test is not applied, and the grip is the copy's (REC-158).
fn cof_class(hands: &Hands, items: &BTreeMap<UnitId, ItemFacts>, class: u32) -> i32 {
    let fact = |i: Option<UnitId>| i.and_then(|i| items.get(&i).map(|f| (i, f)));
    let (right, left) = (fact(hands.right), fact(hands.left));
    let hand = right.filter(|(_, f)| matches!(f.component, 5 | 6)).or(left);
    let Some((_, h)) = hand.filter(|(_, f)| matches!(f.component, 5 | 6)) else {
        return 0;
    };
    if let (Some((ri, rf)), Some((_, lf))) = (right, left) {
        if rf.weap && lf.weap {
            match class {
                4 => {
                    // r-dual: A the weapon in use (else the right hand).
                    let a_id = hands.weapon.unwrap_or(ri);
                    let (a, b) = if a_id == ri { (rf, lf) } else { (lf, rf) };
                    return class_index(match (type_class(a), type_class(b)) {
                        (2, 3) => b"1js ",
                        (3, 3) => b"1jt ",
                        (3, 2) => b"1st ",
                        _ => b"1ss ",
                    });
                }
                6 => return class_index(b"ht2 "),
                _ => {}
            }
        }
    }
    if h.grip == 2 {
        class_index(&h.wclass2)
    } else {
        class_index(&h.wclass)
    }
}

/// The play host's world sync ([`d2_server::adapters::SimGame::set_world_sync`]):
/// the hands of every player and the facts of the items in them.
pub fn sync<R, S>(_: &Game, sim: &mut WorldSim<LocalSeams>, world: &mut WiredWorld<R, S>) {
    let Some(inv) = world.inventory.as_ref() else {
        return;
    };
    let mut w = Weapons::default();
    for (&owner, inventory) in &inv.state.inventories {
        let mut hands = Hands::default();
        for &item in inventory.items() {
            let Some(d) = inv.state.items.get(&item) else {
                continue;
            };
            if d.mode != mode::EQUIPPED || d.inv != Some(owner) {
                continue;
            }
            let slot = match d.body_loc {
                body::RIGHT_HAND => &mut hands.right,
                body::LEFT_HAND => &mut hands.left,
                _ => continue,
            };
            *slot = Some(item);
            w.items.insert(item, facts_of(&inv.tables, d.record));
        }
        hands.weapon = hands.right.filter(|r| {
            w.items
                .get(r)
                .is_some_and(|f| f.class != class::HAND_TO_HAND)
        });
        let class = sim.action.sys.units.get(owner).map_or(0, |u| u.class);
        hands.cof = cof_class(&hands, &w.items, class);
        if hands.right.is_some() || hands.left.is_some() {
            w.hands.insert(owner, hands);
        }
    }
    if sim.action.sys.hooks.x.weapons != w {
        sim.action.sys.hooks.x.weapons = w;
    }
}

#[cfg(test)]
mod tests {
    use d2_data::fixup::maps::EquivMatrix;
    use d2_sim::items::inventory::tables::{InvItemRec, InvTypeRec};

    use super::*;

    /// Types: 0 `weap`, 1 `bow` (a `weap`), 2 `jave` (a `weap`), 3 `aqv`
    /// (arrows). Items: 0 a bow, 1 a javelin, 2 arrows, 3 a sword (a `weap`).
    fn tables() -> InvTables {
        let n = 5;
        let mut m = EquivMatrix {
            n,
            words: 1,
            bits: vec![0; n],
        };
        for i in 0..n {
            m.bits[i] |= 1 << i;
        }
        m.bits[1] |= 1;
        m.bits[2] |= 1;
        let ty = |c: &[u8; 4]| InvTypeRec {
            code: *c,
            ..InvTypeRec::default()
        };
        let it = |t: i16, stack: u8| InvItemRec {
            type_: t,
            stackable: stack,
            maxstack: if stack != 0 { 500 } else { 0 },
            ..InvItemRec::default()
        };
        InvTables {
            itemtypes: vec![
                ty(b"weap"),
                ty(b"bow\0"),
                ty(b"jave"),
                ty(b"aqv\0"),
                ty(b"misc"),
            ],
            items: vec![it(1, 0), it(2, 0), it(3, 1), it(0, 0)],
            equiv: m,
            ..InvTables::default()
        }
    }

    // Covers: specs/skills/bodies.md §2.3
    #[test]
    fn a_bow_is_hand_class_1_and_shoots() {
        let f = facts_of(&tables(), 0);
        assert_eq!((f.class, f.shoots), (class::BOW, true));
        assert!(f.types.contains(&1) && f.types.contains(&0));
    }

    #[test]
    fn a_javelin_is_a_thrusting_weapon_that_does_not_shoot() {
        let f = facts_of(&tables(), 1);
        assert_eq!((f.class, f.shoots), (class::ONE_HAND_THRUST, false));
    }

    #[test]
    fn arrows_are_a_stack_and_no_weapon() {
        let f = facts_of(&tables(), 2);
        assert_eq!(f.class, class::HAND_TO_HAND);
        assert!(f.stackable);
        assert_eq!(f.max_stack, 500);
    }

    #[test]
    fn any_other_weapon_swings() {
        let t = tables();
        assert_eq!(facts_of(&t, 3).class, class::ONE_HAND_SWING);
    }

    #[test]
    fn the_hand_class_follows_the_weapon_in_use() {
        let t = tables();
        let (bow, quiver) = (UnitId(7), UnitId(8));
        let mut w = Weapons::default();
        w.items.insert(bow, facts_of(&t, 0));
        w.items.insert(quiver, facts_of(&t, 2));
        let p = UnitId(1);
        w.hands.insert(
            p,
            Hands {
                right: Some(bow),
                left: Some(quiver),
                weapon: Some(bow),
                ..Hands::default()
            },
        );
        assert_eq!(w.hand_class(p), class::BOW);
        assert_eq!(w.item_at(p, body::LEFT_HAND), Some(quiver));
        assert_eq!(w.item_at(p, body::HEAD), None);
        assert!(w.item_is(bow, 1) && !w.item_is(quiver, 1));
        assert_eq!(w.hand_class(UnitId(2)), class::HAND_TO_HAND);
    }

    // Covers: specs/combat/damage.md §3.2 r1
    #[test]
    fn combat_reads_the_weapon_in_use() {
        use d2_sim::wiring::action::Pending;
        let t = tables();
        let (sword, p) = (UnitId(7), UnitId(1));
        let mut seams = LocalSeams::default();
        assert_eq!(seams.current_weapon(p), None, "bare hands");
        let w = &mut seams.weapons;
        w.items.insert(sword, facts_of(&t, 3));
        w.hands.insert(
            p,
            Hands {
                right: Some(sword),
                left: None,
                weapon: Some(sword),
                ..Hands::default()
            },
        );
        assert_eq!(seams.current_weapon(p), Some(sword));
        assert_eq!(seams.weapon(p), Some(sword));
        assert_eq!(seams.item_at(p, body::RIGHT_HAND), Some(sword));
        assert_eq!(seams.wield_type(sword), 1);
        assert_eq!(seams.current_weapon(UnitId(2)), None);
    }

    // Covers: specs/render/unit-composite.md §2.1
    #[test]
    fn the_cof_class_follows_the_hand_items() {
        let claw = ItemFacts {
            component: 5,
            wclass: *b"ht1 ",
            wclass2: *b"ht1 ",
            weap: true,
            grip: 1,
            ..ItemFacts::default()
        };
        let axe = ItemFacts {
            wclass: *b"1hs ",
            ..claw.clone()
        };
        let buckler = ItemFacts {
            component: 7,
            ..ItemFacts::default()
        };
        let (a, b) = (UnitId(5), UnitId(6));
        let both = Hands {
            right: Some(a),
            left: Some(b),
            weapon: Some(a),
            cof: 0,
        };
        let items = |r: &ItemFacts, l: &ItemFacts| BTreeMap::from([(a, r.clone()), (b, l.clone())]);
        // One claw and a shield: its `wclass`.
        assert_eq!(cof_class(&both, &items(&claw, &buckler), 6), 12);
        // Two weapons: the assassin's `ht2`, the barbarian's r-dual
        // (1hs, 1hs) → `1ss`.
        assert_eq!(cof_class(&both, &items(&claw, &claw), 6), 13);
        assert_eq!(cof_class(&both, &items(&axe, &axe), 4), 10);
        // A two-handed grip: `2handedwclass`.
        let staff = ItemFacts {
            wclass: *b"stf ",
            wclass2: *b"2hs ",
            grip: 2,
            ..axe.clone()
        };
        let right = Hands { left: None, ..both };
        assert_eq!(cof_class(&right, &items(&staff, &buckler), 1), 5);
        // No hand item: the `charstats` class, `hth`.
        assert_eq!(cof_class(&Hands::default(), &BTreeMap::new(), 6), 0);
    }
}
