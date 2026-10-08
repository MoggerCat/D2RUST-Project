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
//! stats.

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
}

/// A player's hands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hands {
    pub right: Option<UnitId>,
    pub left: Option<UnitId>,
    /// The weapon in use: the right-hand item with a hand class.
    pub weapon: Option<UnitId>,
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

    /// The hand class of the weapon in use (0 without one).
    pub fn hand_class(&self, u: UnitId) -> i32 {
        self.weapon(u)
            .and_then(|w| self.items.get(&w))
            .map_or(class::HAND_TO_HAND, |f| f.class)
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
    }
}

/// The play host's world sync ([`d2_server::adapters::SimGame::set_world_sync`]):
/// the hands of every player and the facts of the items in them.
pub fn sync<R, S>(
    _: &Game,
    sim: &mut WorldSim<LocalSeams>,
    world: &mut WiredWorld<R, S>,
) {
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
        hands.weapon = hands
            .right
            .filter(|r| w.items.get(r).is_some_and(|f| f.class != class::HAND_TO_HAND));
        if hands.right.is_some() || hands.left.is_some() {
            w.hands.insert(owner, hands);
        }
    }
    if sim.action.sys.hooks.x.weapons != w {
        sim.action.sys.hooks.x.weapons = w;
    }
}
