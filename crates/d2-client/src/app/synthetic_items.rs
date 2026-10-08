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
/// invheight: the hammer and the smoke weapons (weapons), the cap
/// (armor), then misc (the soulstone, gems, skulls, runes, then the
/// smoke misc items).
fn rows() -> Vec<([u8; 4], u16, (u8, u8))> {
    let mut v = vec![
        (q3::HAMMER, ty::WEAP, (2, 3)),
        (smoke::SWORD, ty::WEAP, (1, 3)),
        (smoke::AXE, ty::WEAP, (2, 3)),
        (smoke::CAP, ty::HELM, (2, 2)),
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
    v.extend([
        (smoke::POTION, smoke::HPOT, (1, 1)),
        (smoke::IDENTIFY, ty::SCRO, (1, 1)),
        (smoke::PORTAL, ty::SCRO, (1, 1)),
        (smoke::CHARM, ty::CHAR, (1, 1)),
        (smoke::GOLD, ty::GOLD, (1, 1)),
    ]);
    v
}

/// (q-smoke-items, REC-281) The items of the item smoke test: a
/// one-handed sword with two sockets, an axe with a strength
/// requirement, a cap, a healing potion (belt), the identify and town
/// portal scrolls, a small charm and gold. Every value is `d2rs-own,
/// unverified`; codes are the original's.
pub mod smoke {
    pub const SWORD: [u8; 4] = *b"ssd ";
    pub const AXE: [u8; 4] = *b"axe ";
    pub const CAP: [u8; 4] = *b"cap ";
    pub const POTION: [u8; 4] = *b"hp1 ";
    pub const IDENTIFY: [u8; 4] = *b"isc ";
    pub const PORTAL: [u8; 4] = *b"tsc ";
    pub const CHARM: [u8; 4] = *b"cm1 ";
    pub const GOLD: [u8; 4] = *b"gld ";
    /// The healing potion's type (a beltable type; d2rs-own index).
    pub const HPOT: u16 = 76;
    /// The axe's strength requirement (above a new sorceress's 10).
    pub const AXE_STR: u16 = 32;
    /// The weapon rows' damage (min, max).
    pub const SWORD_DAMAGE: (u8, u8) = (2, 7);
    pub const AXE_DAMAGE: (u8, u8) = (4, 11);
    /// The sword's sockets (`gemsockets`).
    pub const SWORD_SOCKETS: u8 = 2;
    /// The cap's defense range.
    pub const CAP_AC: (u32, u32) = (3, 5);
    /// The chest's and the smoke monster's treasure classes (indexes in
    /// [`super::drop_tables`]).
    pub const CHEST_TC: u16 = 1;
    pub const MONSTER_TC: u16 = 2;
    /// The smoke monster's class: killable, drops [`MONSTER_TC`].
    pub const MONSTER: u32 = 5;

    /// The one magic affix: a suffix for charms only, without a
    /// property.
    pub fn charm_suffix() -> d2_sim::items::tables::AffixRec {
        let none = d2_sim::items::tables::PropRec {
            code: -1,
            ..Default::default()
        };
        d2_sim::items::tables::AffixRec {
            spawnable: 1,
            frequency: 1,
            classspecific: 0xFF,
            itype: [d2_sim::items::ty::CHAR as i16, 0, 0, 0, 0, 0, 0],
            mods: [none; 3],
            ..Default::default()
        }
    }

    /// Gives the smoke monster its `monstats` row.
    pub fn monster(monstats: &mut [d2_data::tables::Monstats]) {
        let m = &mut monstats[MONSTER as usize];
        m.killable = true;
        m.treasureclass1 = MONSTER_TC;
    }
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
        .map(|&(code, t, (w, h))| {
            let mut r = ItemRec {
                code,
                type_: t as i16,
                level: 1,
                invwidth: w,
                invheight: h,
                spawnable: 1,
                ..ItemRec::default()
            };
            match code {
                smoke::SWORD => {
                    (r.mindam, r.maxdam) = smoke::SWORD_DAMAGE;
                    r.gemsockets = smoke::SWORD_SOCKETS;
                    r.durability = 24;
                }
                smoke::AXE => {
                    (r.mindam, r.maxdam) = smoke::AXE_DAMAGE;
                    r.durability = 24;
                }
                smoke::CAP => {
                    (r.minac, r.maxac) = smoke::CAP_AC;
                    r.durability = 12;
                }
                // Saved compact (the original's misc rows of these).
                smoke::GOLD | smoke::POTION | smoke::IDENTIFY | smoke::PORTAL => {
                    r.compactsave = 1;
                    r.nodurability = 1;
                }
                _ => {}
            }
            r
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
        parts: [Some((0, 3)), Some((3, 1)), Some((4, n - 4))],
        // One charm suffix with no property (REC-281): a charm is always
        // magic (`affixes.md` §5: no affix → fatal).
        magic: vec![smoke::charm_suffix()],
        n_suffix: 1,
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
    let helm = &mut itemtypes[usize::from(ty::HELM)];
    helm.body = 1;
    helm.bodyloc1 = 1;
    helm.bodyloc2 = 1;
    itemtypes[usize::from(smoke::HPOT)].beltable = 1;
    InvTables {
        grids,
        belts: vec![12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16],
        items: t
            .items
            .iter()
            .map(|r| {
                let mut i = InvItemRec {
                    code: r.code,
                    type_: r.type_,
                    invwidth: r.invwidth,
                    invheight: r.invheight,
                    mindam: r.mindam,
                    maxdam: r.maxdam,
                    ..InvItemRec::default()
                };
                match r.code {
                    smoke::AXE => i.reqstr = smoke::AXE_STR,
                    smoke::POTION | smoke::IDENTIFY | smoke::PORTAL => i.useable = 1,
                    _ => {}
                }
                i
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
    // The smoke treasure classes (REC-281): negative picks, one pick of
    // each entry in order (`treasure.md` §5.3), so the drops are fixed.
    let find = |c: [u8; 4]| items.items.iter().position(|r| r.code == c).unwrap() as u16;
    let fixed = |name: &[u8], codes: &[[u8; 4]]| {
        let entries: Vec<TcEntry> = codes
            .iter()
            .enumerate()
            .map(|(i, &c)| TcEntry {
                start_classic: i as i32,
                start_expansion: i as i32,
                id: find(c),
                // Gold: the multiplier (`treasure.md` §6), 1×.
                row: if c == smoke::GOLD { 1024 } else { 0 },
                flags: 0,
                mods: [0; 6],
            })
            .collect();
        TreasureClass {
            name: name.to_vec(),
            group: 0,
            level: 0,
            total_classic: entries.len() as i32,
            total_expansion: entries.len() as i32,
            picks: -(entries.len() as i32),
            nodrop: 0,
            mods: [0; 6],
            entries,
        }
    };
    let chest_tc = fixed(
        b"Act 1 Chest A",
        &[
            smoke::AXE,
            smoke::POTION,
            smoke::IDENTIFY,
            smoke::PORTAL,
            smoke::CHARM,
            q3::STANDARD_GEMS[0],
        ],
    );
    let monster_tc = fixed(b"smoke", &[smoke::SWORD, smoke::CAP, smoke::GOLD]);
    DropTables {
        items,
        tcs: TreasureClasses {
            tcs: vec![none, chest_tc, monster_tc],
            group_offset: 0,
            // Every act, difficulty and tier (REC-260's synthetic chest).
            chest: [Some(smoke::CHEST_TC); 45],
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
