// Spec: specs/ui/inventory.md (§1, §8 r4); specs/ui/panels.md (§9.4); specs/render/unit-composite.md (§9)
//! The play app's item parts (gap G16): the item art rows (`weapons`,
//! `armor`, `misc`: `invwidth`, `invheight`, `invfile`, `flippyfile`)
//! and the `inventory.bin` layouts, read from the user's tables and handed
//! to the inventory panel (`ui::panels::inv_items`) and the ground items
//! (`world_view::ground_items`).

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::prelude::*;
use d2_data::bin::TableFiles;
use d2_data::tables::{decode_all, Armor, Inventory, Misc, Record, Weapons};

use crate::assets::path::FileSource;
use crate::bridge::items::{ItemArtRow, ItemArtRows};
use crate::ui::original::hud_belt::BeltParts;
use crate::ui::original::OriginalUi;
use crate::ui::panels::inv_items::{equip_rects, inv_layout};
use crate::world_view::ground_items::GroundItems;
use crate::world_view::WorldViewState;

/// The item parts the original UI reads at install
/// ([`prepare_ui`]); inserted by [`add_items`].
#[derive(Resource, Clone, Debug, Default)]
pub struct ItemParts {
    pub art: ItemArtRows,
    pub inventory: Vec<Inventory>,
    /// The belt records and types (`ui::hud_belt`).
    pub belts: BeltParts,
    /// The item tool tips' tables ([`item_tips`]); none on synthetic data.
    pub tips: Option<crate::ui::item_tip::ItemTips>,
    /// The `weapons` codes with `2handed` set (the empty-slot pictures,
    /// `panels.md` §9.4).
    pub two_handed: std::collections::BTreeSet<[u8; 4]>,
    /// The measured frame size of each inventory graphic ([`inv_frame_sizes`]).
    pub frame_sizes: BTreeMap<String, (u32, u32)>,
}

/// The size of frame 0 of each art row's inventory graphic by `invfile`
/// (lower case): `inventory.md` §8 r4 draws the cel at (x, top + h). A
/// file no archive holds, or that does not parse, is left out (its size
/// is then estimated, `inv_items`).
pub fn inv_frame_sizes(source: &dyn FileSource, art: &ItemArtRows) -> BTreeMap<String, (u32, u32)> {
    let mut out = BTreeMap::new();
    for row in art.0.values() {
        let key = row.inv_file.to_ascii_lowercase();
        if key.is_empty() || out.contains_key(&key) {
            continue;
        }
        let path = crate::ui::inv_grid::inventory_path(&key);
        let Some(Ok(bytes)) = source.read_file(&path) else {
            continue;
        };
        if let Some(f) = d2_formats::dc6::Dc6::parse(&bytes)
            .ok()
            .and_then(|d| d.frames.first().map(|f| (f.width, f.height)))
        {
            out.insert(key, f);
        }
    }
    out
}

/// A table's string column (zero-terminated).
fn text(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).trim().to_owned()
}

fn add_rows<T: Record>(
    set: &d2_data::bin::BinSet,
    rows: &mut BTreeMap<[u8; 4], ItemArtRow>,
    f: impl Fn(&T) -> ([u8; 4], ItemArtRow),
) -> Result<(), String> {
    let table = set
        .table(T::TABLE)
        .ok_or_else(|| format!("{} not loaded", T::TABLE))?;
    let decoded: Vec<T> = decode_all(table).map_err(|e| e.to_string())?;
    for r in &decoded {
        let (code, row) = f(r);
        rows.entry(code).or_insert(row);
    }
    Ok(())
}

macro_rules! art {
    ($r:expr) => {
        (
            $r.code,
            ItemArtRow {
                inv_w: $r.invwidth,
                inv_h: $r.invheight,
                inv_file: text(&$r.invfile),
                flippy_file: text(&$r.flippyfile),
            },
        )
    };
}

/// The item parts of the user's tables (a load error is an error).
pub fn item_parts(archives: &dyn TableFiles) -> Result<ItemParts, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let mut rows = BTreeMap::new();
    add_rows::<Weapons>(&set, &mut rows, |r| art!(r))?;
    let weapons = set
        .table(Weapons::TABLE)
        .ok_or_else(|| format!("{} not loaded", Weapons::TABLE))?;
    let weapons: Vec<Weapons> = decode_all(weapons).map_err(|e| e.to_string())?;
    let two_handed = weapons
        .iter()
        .filter(|w| w.f_2handed != 0)
        .map(|w| w.code)
        .collect();
    add_rows::<Armor>(&set, &mut rows, |r| art!(r))?;
    add_rows::<Misc>(&set, &mut rows, |r| art!(r))?;
    let table = set.table("inventory").ok_or("inventory not loaded")?;
    let inventory: Vec<Inventory> = decode_all(table).map_err(|e| e.to_string())?;
    Ok(ItemParts {
        art: ItemArtRows(rows),
        inventory,
        belts: belt_parts(&set).unwrap_or_else(|e| {
            warn!("belt (d2rs-own, unverified): {e}; no belt row");
            BeltParts::default()
        }),
        tips: None,
        two_handed,
        frame_sizes: BTreeMap::new(),
    })
}

/// The item tool tips of the user's tables: the game's item tables for
/// the stream reader, the name and description columns, and the string
/// tables (`ui::item_tip`). d2rs-own, unverified: a load error is
/// logged by the caller and leaves the preview without tips.
pub fn item_tips(
    archives: &dyn TableFiles,
    items: d2_sim::items::ItemTables,
) -> Result<crate::ui::item_tip::ItemTips, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let strings = super::strings::TableStrings::load(archives, super::strings::LANG)
        .map_err(|e| e.to_string())?;
    crate::ui::item_tip::ItemTips::new(Arc::new(items), &set, Arc::new(strings))
}

/// The `belts.bin` records (`BeltRecord::from_bytes` of each 0x108-byte
/// record) and the worn belts' rows (`armor` `belt`).
fn belt_parts(set: &d2_data::bin::BinSet) -> Result<BeltParts, String> {
    use crate::ui::panels::control::belt::BeltRecord;
    let table = set.table("belts").ok_or("belts not loaded")?;
    let records = table
        .records
        .chunks(table.record_size.max(1))
        .filter_map(BeltRecord::from_bytes)
        .collect();
    let armor = set.table(Armor::TABLE).ok_or("armor not loaded")?;
    let armor: Vec<Armor> = decode_all(armor).map_err(|e| e.to_string())?;
    // `belt` 0 is also the default of non-belts: only belt items matter,
    // and the host looks up the worn item at body location 8.
    let types = armor.iter().map(|r| (r.code, r.belt)).collect();
    Ok(BeltParts { records, types })
}

/// Hands the item parts to the world view's ground items (with `source`
/// for the flippy files) and keeps them for [`prepare_ui`]. Call after
/// the world view state exists and before the original UI is added.
pub fn add_items(app: &mut App, source: Arc<dyn FileSource>, mut parts: ItemParts) {
    parts.frame_sizes = inv_frame_sizes(source.as_ref(), &parts.art);
    if let Some(mut state) = app.world_mut().get_resource_mut::<WorldViewState>() {
        state.ground_items = GroundItems::new(source, parts.art.clone());
    }
    app.insert_resource(parts);
}

/// The inventory panel's item art and layouts, before the UI's file list
/// is taken (`OriginalUi::files`); nothing without [`ItemParts`].
pub fn prepare_ui(app: &App, original: &mut OriginalUi) {
    let Some(parts) = app.world().get_resource::<ItemParts>() else {
        return;
    };
    original.set_item_art(parts.art.clone());
    original.set_inv_layouts(parts.inventory.iter().map(inv_layout).collect());
    original.set_equip_rects(
        parts.inventory.iter().map(equip_rects).collect(),
        parts.two_handed.clone(),
    );
    original.set_item_frame_sizes(parts.frame_sizes.clone());
    original.set_belt_parts(parts.belts.clone());
    if let Some(tips) = &parts.tips {
        original.set_item_tips(tips.clone());
    }
}

/// The item-stream decoder of the model over the game's item tables
/// (`bridge::item_lists`, REC-188).
pub struct TableDecoder(pub Arc<d2_sim::items::ItemTables>);

impl crate::bridge::item_lists::StreamProps for TableDecoder {
    fn props(&self, stream: &[u8]) -> (Vec<crate::bridge::item_lists::ItemProp>, bool) {
        use d2_proto::item_bits::{decode, ItemLookup};
        let t = &*self.0;
        let lookup = d2_server::adapters::item_bits::TablesLookup(t);
        let Ok(b) = decode(stream, &lookup) else {
            return (Vec::new(), false);
        };
        // An alt-code record carries its base code (`bitstream.md` §4.1 r4).
        let charm = lookup
            .code(b.base_code.unwrap_or(b.code))
            .is_some_and(|c| c.charm);
        // The armor's defense (stat 31, `bitstream.md` §4.4) is one of
        // the item's own values the reader stores in its list
        // (`client/stat-lists.md` §2 r1); it reaches the wearer's total
        // with the property lists. PROVISIONAL (REC-281, stat-lists.md
        // open question 2): durability, quantity and sockets are left
        // out (no total reads them).
        // `bitstream.md` §4.6 r4.3: grouped partners are sent unshifted.
        let shift = |s: u16| t.valshift.get(usize::from(s)).copied().unwrap_or(0);
        let props = b
            .defense
            .iter()
            .map(|d| vec![*d])
            .chain(b.lists.iter().flatten().cloned())
            .flat_map(|list| crate::ui::item_tip_props::stream_values(&list, shift))
            .map(|(stat, layer, value)| crate::bridge::item_lists::ItemProp {
                stat,
                layer: layer as u16,
                value,
            })
            .collect();
        (props, charm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::path::MemorySource;

    /// A one-frame DC6 of `w` × `h` (`formats/dc6.md`), every row one
    /// literal run.
    fn dc6(w: u32, h: u32) -> Vec<u8> {
        let mut rows = Vec::new();
        for _ in 0..h {
            rows.push(w as u8);
            rows.extend((0..w).map(|i| 1 + i as u8));
            rows.push(0x80);
        }
        let mut d = Vec::new();
        for v in [6i32, 1, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&[0xEE; 4]);
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&((d.len() + 4) as u32).to_le_bytes());
        for v in [0u32, w, h, 0, 0, 0, 0, rows.len() as u32] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&rows);
        d.extend_from_slice(&[0xEE; 3]);
        d
    }

    // The item cel is drawn at (x, top + h) with its frame's own height
    // (`inventory.md` §8 r4): the host measures frame 0 of each invfile;
    // a file no archive holds is left to the estimate.
    // Covers: specs/ui/inventory.md §8 r4
    #[test]
    fn inventory_graphics_are_measured() {
        let mut src = MemorySource::default();
        src.insert(r"data\global\items\invsst.dc6", dc6(28, 84));
        let row = |f: &str| ItemArtRow {
            inv_w: 1,
            inv_h: 4,
            inv_file: f.into(),
            flippy_file: String::new(),
        };
        let art = ItemArtRows(BTreeMap::from([
            (*b"sst ", row("invSST")),
            (*b"xxx ", row("invxxx")),
        ]));
        assert_eq!(
            inv_frame_sizes(&src, &art),
            BTreeMap::from([("invsst".to_string(), (28, 84))])
        );
    }
}
