// Spec: specs/formats/d2s.md §2.4 rule 2 (item index), §8.4 (hireling items), §8.5 (golem item); specs/world/hirelings.md §5 rule 4
//! Who owns what at save time (q-save-gaps): the units whose item lists a
//! save writes besides the player's (the hireling `jf` and the Iron Golem
//! `kf` lists, `d2s.md` §8.4 / §8.5) and the item index of a mouse skill's
//! owner item (`d2s.md` §2.4 rule 2). Reads only; the lists themselves go
//! through [`WiredWorld::save_items`] / [`WiredWorld::load_items`].

use d2_sim::game::Game;
use d2_sim::units::{UnitId, UnitType};

use super::{ActionEvents, WiredWorld};

/// Class id of the Iron Golem monster (`d2s.md` §8.5 rule 1).
pub const IRON_GOLEM_CLASS: u32 = 0x123;
/// The golem's pet type (`d2s.md` §8.5 rule 1: "pet node (type 3)").
const GOLEM_PET_TYPE: usize = 3;

impl<R, S> WiredWorld<R, S> {
    /// The player's hireling unit: the first node of its hireling list
    /// (`hirelings.md` §5 rule 4, dead nodes included) whose unit exists.
    pub fn hireling_unit(&self, game: &Game, player: UnitId) -> Option<UnitId> {
        self.state
            .hirelings
            .list(player)?
            .nodes
            .iter()
            .find_map(|n| game.lists.find_unit(UnitType::Monster, n.guid))
    }

    /// The player's Iron Golem unit (§8.5 rule 1: the golem pet node
    /// whose unit class is 0x123).
    pub fn golem_unit<D: ActionEvents>(
        &self,
        game: &Game,
        events: &mut D,
        player: UnitId,
    ) -> Option<UnitId> {
        let guids: Vec<i32> = events
            .action()
            .hooks()
            .pet_lists
            .get(&player)?
            .entries
            .get(GOLEM_PET_TYPE)?
            .nodes
            .iter()
            .map(|n| n.guid)
            .collect();
        guids.into_iter().find_map(|g| {
            let u = game
                .lists
                .find_unit(UnitType::Monster, u32::try_from(g).ok()?)?;
            let class = events.action().sys.units.get(u)?.class;
            (class == IRON_GOLEM_CLASS).then_some(u)
        })
    }

    /// The GUIDs of `owner`'s inventory item list in link order
    /// (`d2s.md` §2.4 rule 2: position + 1 is an item index).
    pub fn item_guids(&self, owner: UnitId) -> Vec<u32> {
        let Some(inv) = self.inventory.as_ref() else {
            return Vec::new();
        };
        inv.state
            .items_of(owner)
            .into_iter()
            .filter_map(|u| inv.state.items.get(&u).map(|d| d.guid))
            .collect()
    }
}
