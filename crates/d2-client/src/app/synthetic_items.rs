// Spec: specs/world/quests-act4.md §4.6, §4.7, §4.8; specs/items/inventory.md §1.3; specs/items/treasure.md §9
//! (q-a4-quest-items) The synthetic game's item tables: the Act IV
//! Hellforge's items (Mephisto's Soulstone `mss `, Hellforge Hammer `hfh `,
//! the gems, skulls and runes its event drops) with the inventory tables
//! over them, so the quest flow (soulstone in, three hammer hits, gem
//! drops) runs on the real item store and inventory model of the play
//! host. PROVISIONAL (REC-235): every row is `d2rs-own, unverified` (no
//! user table is read); the combined array is weapons (`hfh `), no armor,
//! then misc.

use d2_data::bin::BinTable;
use d2_data::fixup::maps::EquivMatrix;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Itemratio, Itemstatcost, Itemtypes, Record};
use d2_sim::items::bitstream::Isc;
use d2_sim::items::inventory::tables::{GridRec, InvItemRec, InvTypeRec};
use d2_sim::items::inventory::InvTables;
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{stat, ty, ItemTables};
use d2_sim::stats::{StatData, StatTable};
use d2_sim::treasure::{ItemData, TcEntry, TreasureClass, TreasureClasses};
use d2_sim::wiring::economy::DropTables;
use d2_sim::world::quests::act4::q3;

/// Rows of the synthetic `itemstatcost` and `itemtypes`.
const N_STATS: usize = 359;
const N_TYPES: usize = 80;

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// The runes the Hellforge drops (`r01` … `r25`).
fn rune_codes() -> impl Iterator<Item = [u8; 4]> {
    (1..=25u8).map(|n| [b'r', b'0' + n / 10, b'0' + n % 10, b' '])
}

/// The combined array's codes, with each row's type and invwidth ×
/// invheight: the hammer (weapons), then misc (the soulstone, gems,
/// skulls, runes).
fn rows() -> Vec<([u8; 4], u16, (u8, u8))> {
    let mut v = vec![
        (q3::HAMMER, ty::WEAP, (2, 3)),
        (q3::SOULSTONE, ty::MISC, (1, 1)),
    ];
    for code in q3::PERFECT_GEMS
        .iter()
        .chain(&q3::FLAWLESS_GEMS)
        .chain(&q3::STANDARD_GEMS)
    {
        v.push((*code, ty::GEM, (1, 1)));
    }
    v.extend(rune_codes().map(|c| (c, ty::RUNE, (1, 1))));
    v
}

/// Every type is its own and type 0's (as the fixtures' `equiv`).
fn equiv() -> EquivMatrix {
    let n = N_TYPES;
    let words = n.div_ceil(32);
    let mut m = EquivMatrix {
        n,
        words,
        bits: vec![0; n * words],
    };
    for i in 1..n {
        m.bits[i * words] |= 1;
        m.bits[i * words + i / 32] |= 1 << (i % 32);
    }
    m
}

/// The itemstatcost save columns of the stats an item's save record
/// carries (`items/bitstream.md` §4.4: defense, durability, maximum).
fn save_columns() -> Vec<Isc> {
    let mut t = vec![Isc::default(); N_STATS];
    let bits = |save_bits: u8, save_add: u32| Isc {
        save_bits,
        save_add,
        ..Isc::default()
    };
    t[usize::from(stat::ARMORCLASS)] = bits(11, 10);
    t[usize::from(stat::DURABILITY)] = bits(9, 0);
    t[usize::from(stat::MAXDURABILITY)] = bits(8, 0);
    t
}

/// The synthetic game's items.
pub fn item_tables() -> ItemTables {
    let mut ratio: Itemratio = blank();
    ratio.version = 0;
    let rows = rows();
    let items: Vec<ItemRec> = rows
        .iter()
        .map(|&(code, t, (w, h))| ItemRec {
            code,
            type_: t as i16,
            level: 1,
            invwidth: w,
            invheight: h,
            spawnable: 1,
            ..ItemRec::default()
        })
        .collect();
    let n = items.len();
    ItemTables {
        items,
        itemtypes: (0..N_TYPES)
            .map(|_| {
                let mut t: Itemtypes = blank();
                t.class = 0xFF;
                t.staffmods = 0xFF;
                // Empty `shoots`: the link miss (`inventory.md` §4.4 r2).
                t.shoots = 0xFFFF;
                t.rare = 1;
                t.normal = 1;
                t
            })
            .collect(),
        equiv: equiv(),
        itemratio: vec![ratio],
        valshift: vec![0; N_STATS],
        isc: save_columns(),
        stat_shift: 6,
        stat_mask: 0x3F,
        parts: [Some((0, 1)), None, Some((1, n - 1))],
        ..ItemTables::default()
    }
}

/// The inventory tables over [`item_tables`]: the grids of the fixtures
/// (`inventory.md` §1.3), the hammer wieldable in either hand (body
/// locations 4 and 5).
pub fn inv_tables(t: &ItemTables) -> InvTables {
    let g = |x, y| GridRec {
        grid_x: x,
        grid_y: y,
    };
    let mut grids = vec![g(10, 4); 16];
    grids[5] = g(10, 10);
    grids[8] = g(6, 4);
    grids[9] = g(3, 4);
    grids[12] = g(6, 8);
    grids[13] = g(0, 0);
    let mut itemtypes = vec![
        InvTypeRec {
            class: 7,
            ..InvTypeRec::default()
        };
        t.itemtypes.len()
    ];
    let weap = &mut itemtypes[usize::from(ty::WEAP)];
    weap.body = 1;
    weap.bodyloc1 = 4;
    weap.bodyloc2 = 5;
    InvTables {
        grids,
        belts: vec![12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16],
        items: t
            .items
            .iter()
            .map(|r| InvItemRec {
                code: r.code,
                type_: r.type_,
                invwidth: r.invwidth,
                invheight: r.invheight,
                ..InvItemRec::default()
            })
            .collect(),
        itemtypes,
        equiv: t.equiv.clone(),
    }
}

/// The quest drop's tables over [`item_tables`]: one treasure class that
/// picks nothing (the Hellforge drops by code, `treasure.md` §9).
pub fn drop_tables() -> DropTables {
    let items = item_tables();
    let treasure_items = items
        .items
        .iter()
        .map(|r| ItemData {
            code: r.code,
            ubercode: r.ubercode,
            ultracode: r.ultracode,
            version: r.version,
            level: r.level,
            type_: r.type_ as u16,
            type2: r.type2 as u16,
            unique: r.unique,
            quest: r.quest,
            spawnable: 1,
        })
        .collect();
    let none = TreasureClass {
        name: b"none".to_vec(),
        group: 0,
        level: 0,
        total_classic: 0,
        total_expansion: 0,
        picks: 1,
        nodrop: 0,
        mods: [0; 6],
        entries: Vec::<TcEntry>::new(),
    };
    DropTables {
        items,
        tcs: TreasureClasses {
            tcs: vec![none],
            group_offset: 0,
            chest: [None; 45],
            notes: Vec::new(),
        },
        treasure_items,
        superuniques: Vec::new(),
    }
}

/// A synthetic itemstatcost: 359 stats, no ops, fixed up. Without it no
/// stat is stored, so a player has no strength, dexterity or level to
/// wield an item with (`inventory.md` §4.2). d2rs-own, unverified.
pub fn stat_data() -> StatData {
    let (n, size) = (N_STATS, Itemstatcost::SIZE);
    let mut records = vec![0u8; n * size];
    for (s, r) in records.chunks_mut(size).enumerate() {
        for o in [0x32, 0x48, 0x4A, 0x56, 0x58, 0x5A, 0x5C] {
            r[o..o + 2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }
        r[0..2].copy_from_slice(&(s as u16).to_le_bytes());
    }
    let mut t = BinTable {
        name: "itemstatcost".into(),
        source: "synthetic".into(),
        count: n,
        record_size: size,
        records,
    };
    stat_ops(&mut t);
    StatData {
        stats: StatTable::from_fixed(&t).expect("synthetic itemstatcost"),
        ..StatData::default()
    }
}
