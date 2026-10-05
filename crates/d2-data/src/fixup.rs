// Spec: specs/data/loading.md §7.4 (post-load fix-ups and runtime maps), §8 (superuniques hcIdx)
//! The post-load fix-ups: bytes the loaders fill or correct after a
//! `.bin` loads, and the runtime maps they build. Applied to a copy of the
//! live set; [`crate::bin::load`] keeps the shipped bytes (the txt → bin
//! cross-check compares those).
//!
//! Only the §7.4 rows whose rule is fully stated are applied; the others
//! wait for their algorithms (`loading.md` open questions 11 and 13) and
//! are listed in [`PENDING`].

use crate::bin::{cstr, item_code_map, u32_at, BinSet, BinTable};
use crate::compile::{code4, special_linker, CodeLinker, NameLinker};
use crate::strings::StringTables;

/// String id of a missing unique or set item name (§7.4).
pub const MISSING_ITEM_NAME: u16 = 5383;
/// hcIdx values the superunique map covers (§8).
pub const HC_INDICES: usize = 66;
/// Superunique rows the loader reads (§8).
pub const SUPERUNIQUE_ROWS: usize = 512;
/// Skills one pettype record lists (§7.4).
pub const PETTYPE_SKILLS: usize = 15;

/// §7.4 fix-ups not applied yet, with what they wait for.
pub const PENDING: &[(&str, &str)] = &[
    ("itemtypes", "type-equivalence bit matrix (loading.md OQ13)"),
    ("itemstatcost", "op-stat tables and flags +0x51–0x53 (OQ13)"),
    ("skills", "per-class skill lists (OQ13)"),
    ("charstats", "class-name strings (OQ13)"),
    ("setitems", "attachment of items to their sets (OQ13)"),
    (
        "gems",
        "code → id at +0x2C and items +0xF0 (field widths, OQ13)",
    ),
    ("gamble", "item level / index and the level sort (OQ13)"),
    (
        "monstats",
        "class chain +0x4A/+0x4B and AnimData speeds (OQ11, OQ13)",
    ),
    ("levels", "wide strings and monster list counts (OQ13)"),
    ("automap", "internal form (OQ13)"),
];

/// The fixed-up tables and the runtime maps.
#[derive(Debug, Clone)]
pub struct FixedSet {
    /// The 73 record tables, fixed up, in load order.
    pub tables: Vec<BinTable>,
    /// The itemtypes code link, rebuilt from the records.
    pub item_types: CodeLinker,
    /// The item code map over weapons, armor, misc.
    pub item_codes: CodeLinker,
    /// Unique and set item name links (add-always, record order).
    pub uniques: NameLinker,
    pub sets: NameLinker,
    /// hcIdx → first superunique row holding it.
    pub superunique_hc: [Option<u16>; HC_INDICES],
    /// itemstatcost record 0 `stuff` (+0x140), 6 when outside 1–8.
    pub stat_stuff: u32,
}

impl FixedSet {
    pub fn table(&self, name: &str) -> Option<&BinTable> {
        self.tables.iter().find(|t| t.name == name)
    }
}

/// A fix-up could not run (a table it needs is absent, or a name has a
/// byte ≥ 0x80).
#[derive(Debug, thiserror::Error)]
#[error("{table}: {detail}")]
pub struct FixupError {
    pub table: String,
    pub detail: String,
}

fn err(table: &str, detail: impl Into<String>) -> FixupError {
    FixupError {
        table: table.to_owned(),
        detail: detail.into(),
    }
}

fn set_u16(r: &mut [u8], o: usize, v: u16) {
    r[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn get_u16(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}

fn records_mut(t: &mut BinTable) -> impl Iterator<Item = &mut [u8]> {
    t.records.chunks_exact_mut(t.record_size.max(1))
}

/// `name` (NUL-terminated at `from`) → string id at `to` (u16), with the
/// given miss value.
fn name_id(t: &mut BinTable, strings: &StringTables, from: usize, to: usize, miss: u16) {
    let end = t.record_size;
    for r in records_mut(t) {
        let id = match strings.id(cstr(r, from..end)) {
            0 => miss,
            id => id as u16,
        };
        set_u16(r, to, id);
    }
}

/// Applies the fix-ups of every table, in load order.
pub fn apply(data: &BinSet) -> Result<FixedSet, FixupError> {
    let mut tables = data.tables.clone();
    let strings = &data.strings;
    let index = |tables: &[BinTable], n: &str| {
        tables
            .iter()
            .position(|t| t.name == n)
            .ok_or_else(|| err(n, "table not loaded"))
    };
    let mut out = FixedSet {
        tables: Vec::new(),
        item_types: CodeLinker::default(),
        item_codes: CodeLinker::default(),
        uniques: NameLinker::default(),
        sets: NameLinker::default(),
        superunique_hc: [None; HC_INDICES],
        stat_stuff: 0,
    };
    for i in 0..tables.len() {
        let (earlier, rest) = tables.split_at_mut(i);
        let t = &mut rest[0];
        match t.name.as_str() {
            "itemtypes" => {
                for r in t.iter() {
                    out.item_types.add(u32::from_le_bytes(code4(&r[..4])));
                }
            }
            "itemstatcost" => {
                for r in records_mut(t) {
                    if r[0x54] > 13 {
                        r[0x54] = 0;
                    }
                }
                let stuff = t.iter().next().map_or(0, |r| u32_at(r, 0x140));
                out.stat_stuff = if (1..=8).contains(&stuff) { stuff } else { 6 };
            }
            "missiles" => {
                for r in records_mut(t) {
                    r[0x183] = r[0x183].min(8);
                }
            }
            "skills" => {
                let p = index(earlier, "pettype")?;
                append_pet_skills(t, &mut earlier[p]);
            }
            "magicsuffix" | "magicprefix" | "automagic" | "lowqualityitems" => {
                name_id(t, strings, 0x00, 0x20, 0)
            }
            "raresuffix" | "rareprefix" => name_id(t, strings, 0x26, 0x0C, 0),
            "runes" => name_id(t, strings, 0x00, 0x82, 0),
            "qualityitems" => {
                name_id(t, strings, 0x2C, 0x6C, 0);
                name_id(t, strings, 0x4C, 0x6E, 0);
            }
            "uniqueitems" | "setitems" => {
                let unique = t.name == "uniqueitems";
                for (n, r) in records_mut(t).enumerate() {
                    set_u16(r, 0, n as u16);
                }
                let to = if unique { 0x22 } else { 0x24 };
                name_id(t, strings, 0x02, to, MISSING_ITEM_NAME);
                let (linker, _) = special_linker(t.iter(), 40, if unique { 52 } else { 48 })
                    .ok_or_else(|| err(&t.name, "item name byte >= 0x80"))?;
                if unique {
                    out.uniques = linker;
                } else {
                    out.sets = linker;
                }
            }
            "monstats" => {
                let count = t.count;
                for (n, r) in records_mut(t).enumerate() {
                    if get_u16(r, 0x02) as usize >= count {
                        set_u16(r, 0x02, n as u16);
                    }
                }
            }
            "superuniques" => {
                for (n, r) in t.iter().take(SUPERUNIQUE_ROWS).enumerate() {
                    let hc = u32_at(r, 0x08) as usize;
                    if hc < HC_INDICES && out.superunique_hc[hc].is_none() {
                        out.superunique_hc[hc] = Some(n as u16);
                    }
                }
            }
            "hireling" => {
                name_id(t, strings, 0xD3, 0x114, 0);
                name_id(t, strings, 0xF3, 0x116, 0);
            }
            "monequip" => {
                let m = index(earlier, "monstats")?;
                let items = item_code_map(earlier);
                link_monequip(t, &mut earlier[m], &items);
            }
            _ => {}
        }
        if t.name == "misc" {
            out.item_codes = item_code_map(&tables[..=i]);
        }
    }
    out.tables = tables;
    Ok(out)
}

/// skills → pettype (§7.4): each skill whose `pettype` byte (+0xBE) names
/// a pettype record is appended to that record's list (count +0xBC, u16
/// skill indices from +0xC0), at most 15 per record.
fn append_pet_skills(skills: &BinTable, pettype: &mut BinTable) {
    let size = pettype.record_size;
    for (s, r) in skills.iter().enumerate() {
        let p = usize::from(r[0xBE]);
        if p >= pettype.count {
            continue;
        }
        let rec = &mut pettype.records[p * size..(p + 1) * size];
        let n = usize::from(get_u16(rec, 0xBC));
        if n < PETTYPE_SKILLS {
            set_u16(rec, 0xC0 + 2 * n, s as u16);
            set_u16(rec, 0xBC, n as u16 + 1);
        }
    }
}

/// monequip (§7.4): monstats +0x2A := first monequip row of that monster
/// (else −1); loc bytes (+0x14..+0x17) outside 1–10, or whose item code
/// (+0x08, +0x0C, +0x10) is not in the item code map, are cleared.
fn link_monequip(monequip: &mut BinTable, monstats: &mut BinTable, items: &CodeLinker) {
    let size = monstats.record_size;
    for r in records_mut(monstats) {
        set_u16(r, 0x2A, 0xFFFF);
    }
    for (n, r) in records_mut(monequip).enumerate() {
        let m = usize::from(get_u16(r, 0x00));
        if m < monstats.count {
            let rec = &mut monstats.records[m * size..(m + 1) * size];
            if get_u16(rec, 0x2A) == 0xFFFF {
                set_u16(rec, 0x2A, n as u16);
            }
        }
        for k in 0..3 {
            let code = u32_at(r, 0x08 + 4 * k);
            let loc = &mut r[0x14 + k];
            if !(1..=10).contains(loc) || items.find(code).is_none() {
                *loc = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str, size: usize, records: Vec<Vec<u8>>) -> BinTable {
        BinTable {
            name: name.into(),
            source: "test".into(),
            count: records.len(),
            record_size: size,
            records: records
                .into_iter()
                .flat_map(|mut r| {
                    r.resize(size, 0);
                    r
                })
                .collect(),
        }
    }

    fn rec(size: usize, writes: &[(usize, &[u8])]) -> Vec<u8> {
        let mut r = vec![0; size];
        for (o, b) in writes {
            r[*o..*o + b.len()].copy_from_slice(b);
        }
        r
    }

    #[test]
    fn pet_skills_cap_at_15() {
        let mut pet = table("pettype", 224, vec![vec![]; 2]);
        let skills = table(
            "skills",
            572,
            (0..20)
                .map(|s| {
                    rec(
                        572,
                        &[(
                            0xBE,
                            &[if s == 3 {
                                1
                            } else if s == 4 {
                                0xFF
                            } else {
                                0
                            }],
                        )],
                    )
                })
                .collect(),
        );
        append_pet_skills(&skills, &mut pet);
        let p0 = pet.record(0);
        assert_eq!(get_u16(p0, 0xBC), 15);
        let listed: Vec<u16> = (0..15).map(|k| get_u16(p0, 0xC0 + 2 * k)).collect();
        assert_eq!(listed, [0, 1, 2, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        let p1 = pet.record(1);
        assert_eq!((get_u16(p1, 0xBC), get_u16(p1, 0xC0)), (1, 3));
    }

    #[test]
    fn monequip_links_and_clears() {
        let mut items = CodeLinker::default();
        items.add(u32::from_le_bytes(*b"hax "));
        let mut ms = table("monstats", 424, vec![vec![]; 3]);
        let mut me = table(
            "monequip",
            28,
            vec![
                rec(
                    28,
                    &[
                        (0, &[2, 0]),
                        (0x08, b"hax "),
                        (0x0C, b"zzz "),
                        (0x14, &[4, 4, 11]),
                    ],
                ),
                rec(28, &[(0, &[2, 0]), (0x08, b"hax "), (0x14, &[0, 0, 0])]),
                rec(28, &[(0, &[0, 0])]),
            ],
        );
        link_monequip(&mut me, &mut ms, &items);
        let at = |n| get_u16(ms.record(n), 0x2A);
        assert_eq!([at(0), at(1), at(2)], [2, 0xFFFF, 0]);
        assert_eq!(&me.record(0)[0x14..0x17], [4, 0, 0]);
    }
}
