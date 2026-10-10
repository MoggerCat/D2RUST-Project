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
use d2_data::tables::{decode_all, Armor, Inventory, Itemtypes, Misc, Record, Weapons};

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
    /// The measured frame size of each inventory graphic ([`inv_frame_sizes`]).
    pub frame_sizes: BTreeMap<String, (u32, u32)>,
    /// The inventory tables of the equip-box click (`ui::panels::inv_items`
    /// `equip`); none on synthetic data.
    pub inv_tables: Option<Arc<d2_sim::items::inventory::InvTables>>,
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
    ($r:expr, $t:expr) => {
        (
            $r.code,
            ItemArtRow {
                inv_w: $r.invwidth,
                inv_h: $r.invheight,
                inv_file: text(&$r.invfile),
                flippy_file: text(&$r.flippyfile),
                beltable: $t
                    .get(usize::from($r.type_))
                    .is_some_and(|t| t.beltable != 0)
                    && $r.invwidth == 1
                    && $r.invheight == 1,
            },
        )
    };
}

/// The item parts of the user's tables (a load error is an error).
pub fn item_parts(archives: &dyn TableFiles) -> Result<ItemParts, String> {
    let set = d2_data::bin::load_from(archives, "eng").map_err(|e| e.to_string())?;
    let mut rows = BTreeMap::new();
    let types: Vec<Itemtypes> =
        decode_all(set.table(Itemtypes::TABLE).ok_or("itemtypes not loaded")?)
            .map_err(|e| e.to_string())?;
    add_rows::<Weapons>(&set, &mut rows, |r| art!(r, types))?;
    add_rows::<Armor>(&set, &mut rows, |r| art!(r, types))?;
    add_rows::<Misc>(&set, &mut rows, |r| art!(r, types))?;
    let table = set.table("inventory").ok_or("inventory not loaded")?;
    let inventory: Vec<Inventory> = decode_all(table).map_err(|e| e.to_string())?;
    let mut belts = belt_parts(&set).unwrap_or_else(|e| {
        warn!("belt (d2rs-own, unverified): {e}; no belt row");
        BeltParts::default()
    });
    belts.beltable = rows
        .iter()
        .filter(|(_, r)| r.beltable)
        .map(|(c, _)| *c)
        .collect();
    Ok(ItemParts {
        art: ItemArtRows(rows),
        inventory,
        belts,
        tips: None,
        frame_sizes: BTreeMap::new(),
        inv_tables: None,
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
    Ok(BeltParts {
        records,
        types,
        beltable: Default::default(),
    })
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
    original.set_equip_rects(parts.inventory.iter().map(equip_rects).collect());
    original.set_item_frame_sizes(parts.frame_sizes.clone());
    original.set_belt_parts(parts.belts.clone());
    if let Some(tips) = &parts.tips {
        original.set_item_tips(tips.clone());
    }
    if let Some(t) = &parts.inv_tables {
        original.set_inv_tables(t.clone());
    }
}

/// The item-stream decoder of the model over the game's item tables
/// (`bridge::item_lists`, REC-188).
pub struct TableDecoder(pub Arc<d2_sim::items::ItemTables>);

impl crate::bridge::item_lists::StreamProps for TableDecoder {
    fn item_is_type(&self, code: [u8; 4], t: i16) -> bool {
        self.0.find_code(code).is_some_and(|i| self.0.is_type(i, t))
    }
    fn props(&self, stream: &[u8]) -> (Vec<crate::bridge::item_lists::ItemProp>, bool) {
        use d2_proto::item_bits::{decode, ItemLookup};
        let t = &*self.0;
        let lookup = d2_server::adapters::item_bits::TablesLookup(t);
        // A failed record makes no item (`bitstream-legacy.md` §3 r12).
        let Some(b) = decode(stream, &lookup).ok().filter(|b| !b.failed) else {
            return (Vec::new(), false);
        };
        // An alt-code record carries its base code (`bitstream.md` §4.1 r4).
        let charm = lookup
            .code(b.base_code.unwrap_or(b.code))
            .is_some_and(|c| c.charm);
        let shift = |s: u16| t.valshift.get(usize::from(s)).copied().unwrap_or(0);
        (stream_props(&b, shift), charm)
    }
}

/// `quantity` (`client/stat-lists.md` §2 r1.1).
const STAT_QUANTITY: u16 = 70;
/// `item_numsockets` (`client/stat-lists.md` §2 r1.1).
const STAT_SOCKETS: u16 = 194;

/// The values one item stream puts in the item's list: the item's own
/// §4.5 values (defense 31, durability 72 / max durability 73, quantity
/// 70, sockets 194: its base array, `client/stat-lists.md` §2 r1.1 table
/// row 1) and its §4.6 property lists, each stored as (field − `Save
/// Add`) << `ValShift`; grouped partners are sent unshifted
/// (`bitstream.md` §4.6 r4.3).
pub fn stream_props(
    b: &d2_proto::item_bits::ItemBits,
    shift: impl Fn(u16) -> u8 + Copy,
) -> Vec<crate::bridge::item_lists::ItemProp> {
    use crate::bridge::item_lists::ItemProp;
    let own = [b.defense, b.max_durability, b.durability];
    let mut props: Vec<ItemProp> = own
        .iter()
        .flatten()
        .map(|d| vec![*d])
        .chain(b.lists.iter().flatten().cloned())
        .flat_map(|list| crate::ui::item_tip_props::stream_values(&list, shift))
        .map(|(stat, layer, value)| ItemProp {
            stat,
            layer: layer as u16,
            value,
        })
        .collect();
    let plain = [
        (STAT_QUANTITY, b.quantity.map(i32::from)),
        (STAT_SOCKETS, b.sockets.map(i32::from)),
    ];
    for (stat, v) in plain {
        if let Some(v) = v {
            props.push(ItemProp {
                stat,
                layer: 0,
                value: v << shift(stat),
            });
        }
    }
    props
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

    // The item's own values (durability, max durability, quantity,
    // sockets, defense) land in its list beside the property list.
    // Covers: specs/client/stat-lists.md §2 r1
    #[test]
    fn the_items_own_values_are_in_its_list() {
        use d2_proto::item_bits::{ItemBits, Stat};
        let st = |stat: u16, raw: u32, save_add: u32| Stat {
            stat,
            param: 0,
            raw,
            save_add,
        };
        let b = ItemBits {
            defense: Some(st(31, 30, 10)),
            max_durability: Some(st(73, 20, 0)),
            durability: Some(st(72, 17, 0)),
            quantity: Some(250),
            sockets: Some(2),
            lists: vec![Some(vec![st(0, 5, 0)])],
            ..ItemBits::default()
        };
        let got: Vec<(u16, i32)> = stream_props(&b, |s| if s == 70 { 1 } else { 0 })
            .iter()
            .map(|p| (p.stat, p.value))
            .collect();
        assert_eq!(
            got,
            vec![(31, 20), (73, 20), (72, 17), (0, 5), (70, 500), (194, 2)]
        );
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
            beltable: false,
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
