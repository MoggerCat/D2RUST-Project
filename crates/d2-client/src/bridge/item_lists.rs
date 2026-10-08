// Spec: specs/client/stat-lists.md (Summary, §1 r2–r3, §2 r2–r4.1); specs/items/bitstream.md (§4.6–§4.7)
//! The stat lists of the item units (`client/stat-lists.md` §2): each
//! item owns the property lists of its 0x9C / 0x9D stream, and the
//! lists attached to a unit are added to its total
//! ([`ClientWorld::total`]). The server's stat messages carry the base
//! only; the client sums the equipped items itself.
//!
//! The decoded properties are kept in the item's kind data when the
//! record arrives ([`refresh`]); whether they count is decided when the
//! total is read ([`attached_to`]), from the item's last record, so a
//! move out of the body or grid detaches the list (§2 r4.1) and a move
//! onto the body attaches it (§2 r2) with no extra bookkeeping.
//!
//! PROVISIONAL (REC-188; `stat-lists.md` open question 2): which of an
//! item's lists are attached. d2rs adds every list the stream carries
//! (base-magic list, the set lists of the active set mask, the runeword
//! list), the set-bonus lists of §4.7 included; the original's choice
//! per set item is not written. The attach condition is d2rs's reading
//! of §2 r2: body locations 1-10 and 13+ (not the swap set 11 / 12,
//! `sim/stat-lists.md` §8.4), item flag 0x4000 clear, plus charms on
//! inventory page 0 (the server links those, `inventory.md` §5.7).
//! Socketed gems and runes apply through `0x0065FEC0` (§2 r2.2) and are
//! not added here.

use std::sync::Arc;

use super::items::{self, mode};
use super::world::{ClientWorld, KindData, UnitKey};

/// One property of an item's list: stat, layer (the stream's param) and
/// the list value (the stream value less `Save Add`, shifted by
/// `ValShift`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemProp {
    pub stat: u16,
    pub layer: u16,
    pub value: i32,
}

/// Decodes an item stream into its properties and whether the item is a
/// charm; implemented over the game's item tables outside the bridge
/// (`app::items::TableDecoder`), since the bridge names no server crate.
pub trait StreamProps: Send + Sync {
    /// Empty when the stream does not decode (a compact record has none).
    fn props(&self, stream: &[u8]) -> (Vec<ItemProp>, bool);
}

/// The decoder the model uses (`None` until the play app installs it).
/// Compares equal always: it is a handle, not model state.
#[derive(Clone, Default)]
pub struct ItemTablesRef(pub Option<Arc<dyn StreamProps>>);

impl std::fmt::Debug for ItemTablesRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ItemTablesRef({})", self.0.is_some())
    }
}

impl PartialEq for ItemTablesRef {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for ItemTablesRef {}

/// Decodes the last record of item `key` into its kind data. Called
/// after each 0x9C / 0x9D record is stored.
pub fn refresh(w: &mut ClientWorld, key: UnitKey) {
    let Some(t) = w.item_tables.0.clone() else {
        return;
    };
    let Some(KindData::Item(d)) = w.units.get_mut(&key).map(|u| &mut u.kind) else {
        return;
    };
    let (props, charm) = match &d.last {
        Some(r) => t.props(&r.stream),
        None => (Vec::new(), false),
    };
    d.props = props;
    d.charm = charm;
}

/// The unit whose list the item's properties are attached to, `None`
/// while detached (§2 r2.3, r4.1).
pub fn attached_to(w: &ClientWorld, key: UnitKey) -> Option<UnitKey> {
    let KindData::Item(d) = &w.units.get(&key)?.kind else {
        return None;
    };
    if d.props.is_empty() || d.flags & 0x4000 != 0 {
        return None;
    }
    let v = items::item(w, key)?;
    if v.store || v.on_ground() {
        return None;
    }
    let active = match v.mode {
        // The swap set (11 / 12) has its list detached (`sim/stat-lists.md` §8.4).
        mode::BODY => !matches!(v.body, 11 | 12),
        mode::STORED => d.charm && v.page == 0,
        _ => false,
    };
    active.then_some(v.owner).flatten()
}

impl ClientWorld {
    /// The sum of `stat` / `layer` over the item lists attached to
    /// `unit`.
    pub fn item_lists_total(&self, unit: UnitKey, stat: u16, layer: u16) -> i32 {
        self.units
            .iter()
            .filter(|(k, _)| k.unit_type == items::ITEM)
            .filter(|(&k, _)| attached_to(self, k) == Some(unit))
            .filter_map(|(_, u)| match &u.kind {
                KindData::Item(d) => Some(d),
                _ => None,
            })
            .flat_map(|d| d.props.iter())
            .filter(|p| p.stat == stat && p.layer == layer)
            .fold(0i32, |a, p| a.wrapping_add(p.value))
    }

    /// Installs the decoder the streams are decoded with.
    pub fn set_item_tables(&mut self, t: Arc<dyn StreamProps>) {
        self.item_tables = ItemTablesRef(Some(t));
    }
}
