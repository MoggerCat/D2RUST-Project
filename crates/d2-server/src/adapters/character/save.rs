// Spec: specs/formats/d2s.md §2.8 r1, §2.8 r3, §8.1 r2, §8.1 r3, §8.1 r4, §8.1 r10, §8.4 r1; specs/formats/d2s-appearance.md §3 r1, §4 r1; specs/world/hirelings.md §10 r8
//! Character storage, write side: the parts of a save the writer
//! (`0x00568F20`) rebuilds from the game at save time rather than from
//! the loaded file: the appearance bytes (`formats/d2s.md` §2.8, the fill
//! of `formats/d2s-appearance.md`) and the hireling's item list (`jf`,
//! §8.4 rule 1, `0x005699A0`; its reload is `world/hirelings.md` §10
//! rule 8).
//!
//! The rules are `d2_formats::d2s` (header, sections, the fill) and
//! `d2_sim::items::bitstream` (one item entry). What is decided here is
//! which game values feed them: the item list in the §8.1 rule 3 order,
//! the items a writer skips (rule 4), each item's children (rule 10) and
//! the appearance inputs of each inventory item, read through
//! [`SaveItems`] ([`InvDesk`] in the game).

use d2_formats::d2s::appearance::{self, AppearanceTables, Equipment, EquippedItem, BODY_SLOTS};
use d2_formats::d2s::{D2s, ItemEntry};
use d2_sim::items::bitstream::{write_save, IscTable, StreamItem};
use d2_sim::items::inventory::node;
use d2_sim::items::ItemTables;
use d2_sim::units::lifecycle::LifecycleHooks;
use d2_sim::units::UnitId;
use d2_sim::wiring::inventory::{InvDesk, InvRest};

/// Unit +0xC8 bit of an item that writes 0 bytes (§8.1 rule 4).
pub const FLAGS2_NO_SAVE: u32 = 0x8000;

/// What the writer reads of the units and items (a view of the
/// inventory state, the item store and the unit records).
pub trait SaveItems {
    /// The owner's inventory item list, in link order (§8.1 rule 3.1;
    /// an item's own inventory for its children, rule 10).
    fn items(&self, owner: UnitId) -> Vec<UnitId>;
    /// The item at body location `loc` (grid 0, `0x0063DD90`).
    fn body_item(&self, owner: UnitId, loc: u8) -> Option<UnitId>;
    /// The weapon in use (inventory +0x1C, `0x0063BEF0`).
    fn weapon_in_use(&self, owner: UnitId) -> Option<UnitId>;
    /// The cursor item (inventory +0x20).
    fn cursor(&self, owner: UnitId) -> Option<UnitId>;
    /// The item's node kind (+0x69) and body location (+0x44).
    fn node(&self, item: UnitId) -> Option<(u8, u8)>;
    /// Unit +0xC8.
    fn flags2(&self, item: UnitId) -> u32;
    /// The item's stream view without children (`items/bitstream.md`
    /// Inputs); `None`: no item unit.
    fn stream(&self, item: UnitId) -> Option<StreamItem>;
    /// The item's appearance inputs (`d2s-appearance.md` Inputs), with
    /// `first_child` unset (the caller fills it from [`SaveItems::items`]).
    fn appearance(&self, item: UnitId) -> Option<EquippedItem>;
}

/// Why a save section could not be written.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SaveError {
    /// §8.1 rule 5: an item that does not fit (the original asserts).
    #[error("item entry overflows the save buffer (formats/d2s.md §8.1 rule 5)")]
    Overflow,
    /// The model has no body (the 335-byte stub) or no such item.
    #[error("{0}")]
    Model(&'static str),
}

/// The save of the game's state at save time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaveContext {
    /// Expansion game (`formats/d2s.md` §1 rule 2: `jf` / `kf`).
    pub expansion: bool,
    /// The player's weapon class index (`0x0064F380`, mode −1).
    pub weapon_class: i32,
    /// The states the player has (`d2s-appearance.md` §6 rule 1).
    pub states: Vec<u32>,
}

/// One item entry (§8.1 rule 2): the item's stream, then each item of its
/// own inventory, recursively (rule 10).
fn stream_tree(src: &dyn SaveItems, item: UnitId) -> Option<StreamItem> {
    let mut s = src.stream(item)?;
    s.children = src
        .items(item)
        .into_iter()
        .filter_map(|c| stream_tree(src, c))
        .collect();
    Some(s)
}

/// The items of `owner`'s list in the writer's order (§8.1 rule 3):
/// every list item except a body-node item at location 4 or 5, then the
/// hands (no weapon in use or the right hand in use → right, left; else
/// left, right), then the cursor item.
pub fn list_order(src: &dyn SaveItems, owner: UnitId) -> Vec<UnitId> {
    let mut out: Vec<UnitId> = src
        .items(owner)
        .into_iter()
        .filter(|&u| {
            !matches!(src.node(u), Some((node::BODY, loc))
                if loc == appearance::body::RIGHT_HAND || loc == appearance::body::LEFT_HAND)
        })
        .collect();
    let right = src.body_item(owner, appearance::body::RIGHT_HAND);
    let left = src.body_item(owner, appearance::body::LEFT_HAND);
    let hands = match src.weapon_in_use(owner) {
        None => [right, left],
        Some(w) if Some(w) == right => [right, left],
        Some(_) => [left, right],
    };
    out.extend(hands.into_iter().flatten());
    out.extend(src.cursor(owner));
    out
}

/// The item list of `owner` as save entries (§8.1 rules 2–4, 10): items
/// with unit +0xC8 bit 0x8000 write nothing and are not counted.
pub fn item_list(
    src: &dyn SaveItems,
    owner: UnitId,
    isc: &dyn IscTable,
) -> Result<Vec<ItemEntry>, SaveError> {
    let mut out = Vec::new();
    for u in list_order(src, owner) {
        if src.flags2(u) & FLAGS2_NO_SAVE != 0 {
            continue;
        }
        let s = stream_tree(src, u).ok_or(SaveError::Model("item without a stream view"))?;
        let (bytes, _) = write_save(&s, isc).map_err(|_| SaveError::Overflow)?;
        out.push(ItemEntry { bytes });
    }
    Ok(out)
}

/// The appearance provider (`d2s-appearance.md` §3 r1, §4 r1): the
/// player's item list with each item's inputs, the body grid slots 0–10
/// and the weapon in use as indices into that list.
pub fn equipment(src: &dyn SaveItems, player: UnitId, ctx: &SaveContext) -> Equipment {
    let list = src.items(player);
    let items = list
        .iter()
        .map(|&u| {
            let mut e = src.appearance(u).unwrap_or_default();
            e.first_child = src
                .items(u)
                .first()
                .and_then(|&c| src.appearance(c))
                .map(|c| c.record);
            e
        })
        .collect();
    let at = |u: Option<UnitId>| u.and_then(|u| list.iter().position(|&x| x == u));
    let mut body_grid = [None; BODY_SLOTS];
    for (loc, slot) in body_grid.iter_mut().enumerate() {
        *slot = at(src.body_item(player, loc as u8));
    }
    Equipment {
        items,
        body_grid,
        weapon_in_use: at(src.weapon_in_use(player)),
        weapon_class: ctx.weapon_class,
        states: ctx.states.clone(),
    }
}

/// The parts of `save` the writer rebuilds from the game: the appearance
/// bytes of `player` (§2.8 rules 1, 3) and, for a full save, the `jf`
/// section (§8.4 rule 1: in an expansion game the marker, then the item
/// list of `hireling` when the player has a hireling node).
pub fn rebuild(
    save: &mut D2s,
    src: &dyn SaveItems,
    player: UnitId,
    hireling: Option<UnitId>,
    ctx: &SaveContext,
    t: &AppearanceTables,
    isc: &dyn IscTable,
) -> Result<(), SaveError> {
    save.header
        .rebuild_appearance(&equipment(src, player, ctx), t);
    if let Some(body) = save.body.as_mut() {
        let list = hireling.map(|h| item_list(src, h, isc)).transpose()?;
        body.set_hireling_items(ctx.expansion, list);
    }
    Ok(())
}

/// Max sockets (`0x0062BC20`, `items/generation.md` §7.2) of the record
/// at item level `ilvl`.
fn max_sockets(t: &ItemTables, record: usize, ilvl: i32) -> i32 {
    let (Some(r), Some(it)) = (t.item(record), t.itype_of(record)) else {
        return 0;
    };
    let m = match ilvl.max(1) {
        ..=25 => it.maxsock1,
        26..=40 => it.maxsock25,
        _ => it.maxsock40,
    };
    i32::from(r.gemsockets.min(m))
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> SaveItems for InvDesk<'_, '_, H, R> {
    fn items(&self, owner: UnitId) -> Vec<UnitId> {
        self.state.items_of(owner)
    }
    fn body_item(&self, owner: UnitId, loc: u8) -> Option<UnitId> {
        self.state.of(owner)?.body_item(loc)
    }
    fn weapon_in_use(&self, owner: UnitId) -> Option<UnitId> {
        let g = self.state.of(owner)?.weapon_guid;
        self.state
            .items_of(owner)
            .into_iter()
            .find(|u| self.state.items.get(u).is_some_and(|d| d.guid == g))
    }
    fn cursor(&self, owner: UnitId) -> Option<UnitId> {
        self.state.cursor_of(owner)
    }
    fn node(&self, item: UnitId) -> Option<(u8, u8)> {
        self.state
            .items
            .get(&item)
            .map(|d| (d.node_kind, d.body_loc))
    }
    fn flags2(&self, item: UnitId) -> u32 {
        self.econ.units.get(item).map_or(0, |r| r.flags2)
    }
    fn stream(&self, item: UnitId) -> Option<StreamItem> {
        let d = self.state.items.get(&item)?;
        let mut s = self.stream_item(d.guid, 0, d.page)?;
        // The save-only 32 bits are the unit record's init seed
        // (`bitstream.md` §4.1 rule 7; as `InvDesk::save_view`).
        s.unit28 = self.econ.units.get(item)?.init_seed;
        Some(s)
    }
    fn appearance(&self, item: UnitId) -> Option<EquippedItem> {
        let d = self.state.items.get(&item)?;
        let it = self.econ.items.get(item)?;
        Some(EquippedItem {
            record: it.record,
            mode: d.mode,
            body_loc: d.body_loc,
            quality: it.quality,
            prefix: it.prefix,
            suffix: it.suffix,
            auto_affix: it.auto_affix,
            file_index: it.file_index,
            flags: it.flags,
            max_sockets: max_sockets(self.econ.tables, it.record, it.ilvl),
            first_child: None,
        })
    }
}
