// Spec: specs/ui/inventory.md (§5 hover state, §8 item drawing), specs/ui/text.md (§8 framed text), specs/items/bitstream.md (§4), specs/items/generation.md (§1.1 quality ids)
//! Item tool tips: the hover box of an item (name, quality colour,
//! identified state, requirements, properties).
//!
//! The lines are built from the item's last 0x9C / 0x9D stream, decoded
//! with `d2_proto::item_bits::decode` over the game's item tables, and
//! the string tables ([`ItemTips`]). The hover anchor of
//! `inventory.md` §5 r1 is the cursor point here; the box is a preview
//! box over the synthetic fill file.
//!
//! d2rs-own, unverified (REC-114): no spec gives the text of an item
//! description (the builder behind `0x0048DD90` is unwritten), so
//! - the name line is the base name (`namestr`) with the magic prefix and
//!   suffix names around it, the rare / unique / set name above it, all
//!   in the quality colour (`ÿc` codes 3 blue, 9 yellow, 4 gold, 2 green,
//!   8 orange, 5 grey); an item whose identified flag (0x10) is clear
//!   shows its base name and a red "Unidentified" line;
//! - the labels `Defense:`, `Durability:`, `Quantity:`, `Required
//!   Strength:` / `Dexterity:` / `Level:` are English text (their string
//!   ids are not specified);
//! - each property is one line from its `itemstatcost` `descfunc`
//!   shape ([`super::item_tip_desc`], 1–28; REC-242) with `descval`
//!   placing the value, ordered by `descpriority` descending; a stat with
//!   no description string is skipped; a set item's set lists are green
//!   and the set's bonuses follow its name ([`super::item_tip_set`]);
//! - affix, unique and set names are table indices as sent (prefix /
//!   suffix id, file index) into the name tables, without the id offset
//!   rules of `affixes.md`.
//!
//! Nothing here counts as done (rule 10).

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::bin::BinSet;
use d2_data::tables::{
    decode_all, Armor, Charstats, Itemstatcost, Magicprefix, Magicsuffix, Misc, Monstats, Montype,
    Rareprefix, Raresuffix, Record, Setitems, Sets, Skilldesc, Skills, Uniqueitems, Weapons,
};
use d2_proto::item_bits::{decode, ItemBits, Stat};
use d2_server::adapters::item_bits::TablesLookup;
use d2_sim::items::ItemTables;

use super::draw::{ImageRef, ImageRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
use super::geom::{Point, Rect};
use super::item_tip_desc::{self as desc, StatDesc};
use super::item_tip_props::{self as props, StatList};
use super::item_tip_set as set;
use super::original::hud::{FILL_FILE, FILL_H, FILL_W};
use super::original::FontMeasure;
use super::panel::StringLookup;
use super::panels::UiFiles;
use super::text::TextOpts;
use super::FRAME;

/// Text colour indexes (`ÿc` codes, `ui/text.md` §5).
pub mod color {
    pub const WHITE: u16 = 0;
    pub const RED: u16 = 1;
    pub const GREEN: u16 = 2;
    pub const BLUE: u16 = 3;
    pub const GOLD: u16 = 4;
    pub const GREY: u16 = 5;
    pub const ORANGE: u16 = 8;
    pub const YELLOW: u16 = 9;
    /// `ÿc:` (tempered names, `ui/item-tips.md` §4 r1).
    pub const TEMPERED: u16 = 10;
}

/// Item quality ids (`generation.md` §1.1).
mod quality {
    pub const LOW: u8 = 1;
    pub const MAGIC: u8 = 4;
    pub const SET: u8 = 5;
    pub const RARE: u8 = 6;
    pub const UNIQUE: u8 = 7;
    pub const CRAFTED: u8 = 8;
    pub const TEMPERED: u8 = 9;
}

/// The font of the box (Font16) and the line step.
const FONT: u16 = 1;
/// d2rs-own, unverified: pixels per line.
const LINE_H: i32 = 18;
/// The fill frame of the box (`hud::fill_frames`: the darkest colour).
const DARK: u32 = 4;

/// One line of a tip: UTF-16 text and its colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TipLine {
    pub text: Vec<u16>,
    pub color: u16,
}

impl TipLine {
    fn new(s: impl AsRef<str>, color: u16) -> Self {
        TipLine {
            text: s.as_ref().encode_utf16().collect(),
            color,
        }
    }
}

/// The name and requirement columns of an items row.
#[derive(Clone, Copy, Debug, Default)]
struct CodeText {
    name_id: u16,
    req_str: u16,
    req_dex: u16,
    req_lvl: u8,
}

/// What the tips read from the tables and strings.
#[derive(Clone)]
pub struct ItemTips {
    lookup: Arc<ItemTables>,
    codes: BTreeMap<[u8; 4], CodeText>,
    stats: Vec<StatDesc>,
    /// The description list (`data/runtime-maps.md` §3).
    desc_order: Vec<u16>,
    /// String-table keys by table row.
    magic_prefix: Vec<String>,
    magic_suffix: Vec<String>,
    rare_prefix: Vec<String>,
    rare_suffix: Vec<String>,
    unique: Vec<String>,
    set: Vec<String>,
    /// Set of each setitems row, and each set's name string id (the
    /// set line at the foot of a set item's tip).
    set_of_item: Vec<u16>,
    set_names: Vec<u16>,
    /// Per skill: its skilldesc name string id (`None`: no skilldesc
    /// row) and its `charclass` (255: none).
    skills: Vec<(Option<u16>, u8)>,
    /// Per charstats row: the class strings (`item-tips.md` §7.2).
    class_strings: Vec<desc::ClassStrings>,
    /// Montype `strplur` and monstats `NameStr` per row.
    montype: Vec<u16>,
    monstats: Vec<u16>,
    strings: Arc<dyn StringLookup + Send + Sync>,
}

impl std::fmt::Debug for ItemTips {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ItemTips({} codes)", self.codes.len())
    }
}

fn key(b: &[u8]) -> String {
    let n = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).into_owned()
}

fn rows<T: Record>(set: &BinSet) -> Result<Vec<T>, String> {
    let t = set
        .table(T::TABLE)
        .ok_or_else(|| format!("{} not loaded", T::TABLE))?;
    decode_all(t).map_err(|e| e.to_string())
}

impl ItemTips {
    /// The item tables the tips decode with (shared with the model's
    /// item lists, `bridge::item_lists`).
    pub fn tables(&self) -> Arc<ItemTables> {
        self.lookup.clone()
    }

    /// The tips over `lookup` (the game's item tables, for the stream
    /// reader), the loaded tables `set` and `strings`.
    pub fn new(
        lookup: Arc<ItemTables>,
        set: &BinSet,
        strings: Arc<dyn StringLookup + Send + Sync>,
    ) -> Result<Self, String> {
        let mut codes = BTreeMap::new();
        macro_rules! add {
            ($t:ty) => {
                for r in rows::<$t>(set)? {
                    codes.entry(r.code).or_insert(CodeText {
                        name_id: r.namestr,
                        req_str: r.reqstr,
                        req_dex: r.reqdex,
                        req_lvl: r.levelreq,
                    });
                }
            };
        }
        add!(Weapons);
        add!(Armor);
        add!(Misc);
        let desc_order = set
            .table(Itemstatcost::TABLE)
            .map(d2_data::fixup::maps::desc_list)
            .unwrap_or_default();
        let stats = rows::<Itemstatcost>(set)?
            .iter()
            .map(|r| StatDesc {
                priority: r.descpriority,
                func: r.descfunc,
                val: r.descval,
                pos: r.descstrpos,
                neg: r.descstrneg,
                str2: r.descstr2,
                dgrp: r.dgrp,
                dgrpfunc: r.dgrpfunc,
                dgrpval: r.dgrpval,
                dgrppos: r.dgrpstrpos,
                dgrpneg: r.dgrpstrneg,
                dgrpstr2: r.dgrpstr2,
                op: r.op,
                op_param: r.op_param,
                op_base: r.op_base,
                valshift: r.valshift,
            })
            .collect();
        Ok(ItemTips {
            lookup,
            codes,
            stats,
            desc_order,
            magic_prefix: rows::<Magicprefix>(set)?
                .iter()
                .map(|r| key(&r.name))
                .collect(),
            magic_suffix: rows::<Magicsuffix>(set)?
                .iter()
                .map(|r| key(&r.name))
                .collect(),
            rare_prefix: rows::<Rareprefix>(set)?
                .iter()
                .map(|r| key(&r.name))
                .collect(),
            rare_suffix: rows::<Raresuffix>(set)?
                .iter()
                .map(|r| key(&r.name))
                .collect(),
            unique: rows::<Uniqueitems>(set)?
                .iter()
                .map(|r| key(&r.index))
                .collect(),
            set: rows::<Setitems>(set)?
                .iter()
                .map(|r| key(&r.index))
                .collect(),
            set_of_item: rows::<Setitems>(set)?.iter().map(|r| r.set).collect(),
            set_names: rows::<Sets>(set)?.iter().map(|r| r.name).collect(),
            skills: {
                let descs = rows::<Skilldesc>(set)?;
                rows::<Skills>(set)?
                    .iter()
                    .map(|r| {
                        let name = descs.get(usize::from(r.skilldesc)).map(|d| d.str_name);
                        (name, r.charclass)
                    })
                    .collect()
            },
            class_strings: rows::<Charstats>(set)?
                .iter()
                .map(|r| desc::ClassStrings {
                    all_skills: r.strallskills,
                    tabs: [r.strskilltab1, r.strskilltab2, r.strskilltab3],
                    class_only: r.strclassonly,
                })
                .collect(),
            montype: rows::<Montype>(set)?.iter().map(|r| r.strplur).collect(),
            monstats: rows::<Monstats>(set)?.iter().map(|r| r.namestr).collect(),
            strings,
        })
    }

    fn text_by_key(&self, names: &[String], i: usize) -> Option<String> {
        let k = names.get(i)?;
        let t = self.strings.get(k)?;
        Some(String::from_utf16_lossy(t))
    }

    /// The name of magic affix id `id` as sent (`items/affixes.md` §1 r1
    /// with `bitstream.md` §4.2: prefixes arrive as their magicprefix row
    /// + 1, suffixes as their magicsuffix row + 1; 0 = none).
    fn magic_name(&self, names: &[String], id: u16) -> Option<String> {
        let row = usize::from(id).checked_sub(1)?;
        self.text_by_key(names, row)
    }

    /// The rare name of rare id `id` (`items/affixes.md` §1 r1: combined
    /// index + 1 in the rare array, raresuffix rows first, then
    /// rareprefix; sent unchanged, `bitstream.md` §4.3 r5). `prefix`
    /// picks the part the slot holds; an id outside that part is none.
    fn rare_name(&self, id: u8, prefix: bool) -> Option<String> {
        let combined = usize::from(id).checked_sub(1)?;
        let n = self.rare_suffix.len();
        if prefix {
            self.text_by_key(&self.rare_prefix, combined.checked_sub(n)?)
        } else if combined < n {
            self.text_by_key(&self.rare_suffix, combined)
        } else {
            None
        }
    }

    fn base_name(&self, code: [u8; 4]) -> String {
        self.codes
            .get(&code)
            .and_then(|c| self.strings.get_id(c.name_id))
            .map(String::from_utf16_lossy)
            .unwrap_or_else(|| String::from_utf8_lossy(&code).trim_end().to_owned())
    }

    /// The decoded record of an item stream (`None` when it does not
    /// decode).
    pub fn bits(&self, stream: &[u8]) -> Option<ItemBits> {
        decode(stream, &TablesLookup(&self.lookup)).ok()
    }

    /// d2rs-own, unverified (REC-121; `0x0062BEB0` is not
    /// specified): the item with `code` is a gem (20), rune (74) or
    /// jewel (58), as the server's socket-filler test.
    pub fn is_socket_filler(&self, code: [u8; 4]) -> bool {
        use d2_sim::items::ty;
        self.lookup.find_code(code).is_some_and(|i| {
            [ty::GEM, ty::RUNE, ty::JEWL]
                .into_iter()
                .any(|t| self.lookup.is_type(i, t as i16))
        })
    }

    /// The tip of the item with last stream `stream`; empty when the
    /// stream does not decode.
    pub fn lines(&self, stream: &[u8]) -> Vec<TipLine> {
        match decode(stream, &TablesLookup(&self.lookup)) {
            Ok(bits) => self.lines_of(&bits),
            Err(_) => Vec::new(),
        }
    }

    fn lines_of(&self, b: &ItemBits) -> Vec<TipLine> {
        let mut out = Vec::new();
        let ident = b.flags & 0x10 != 0;
        let base = self.base_name(b.code);
        let q = b.quality;
        let name_color = self.name_color(b);
        let qf = &b.quality_fields;
        if ident {
            match q {
                quality::MAGIC => {
                    let (p, s) = qf.magic.unwrap_or((0, 0));
                    let pre = self.magic_name(&self.magic_prefix, p);
                    let suf = self.magic_name(&self.magic_suffix, s);
                    let mut n = String::new();
                    n.extend(pre.map(|t| t + " "));
                    n += &base;
                    n.extend(suf.map(|t| format!(" {t}")));
                    out.push(TipLine::new(n, name_color));
                }
                quality::RARE | quality::CRAFTED => {
                    if let Some((a, c)) = qf.rare_names {
                        let n = [self.rare_name(a, true), self.rare_name(c, false)]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(" ");
                        if !n.is_empty() {
                            out.push(TipLine::new(n, name_color));
                        }
                    }
                    out.push(TipLine::new(&base, name_color));
                }
                quality::UNIQUE | quality::SET => {
                    let names = if q == quality::UNIQUE {
                        &self.unique
                    } else {
                        &self.set
                    };
                    let idx = qf.file_index.unwrap_or(0) as usize;
                    if let Some(n) = self.text_by_key(names, idx) {
                        out.push(TipLine::new(n, name_color));
                    }
                    out.push(TipLine::new(&base, name_color));
                }
                _ => out.push(TipLine::new(&base, name_color)),
            }
        } else {
            out.push(TipLine::new(&base, name_color));
            out.push(TipLine::new("Unidentified", color::RED));
        }
        // Base facts.
        if let Some(d) = b.defense {
            out.push(TipLine::new(
                format!("Defense: {}", d.raw.saturating_sub(d.save_add)),
                color::WHITE,
            ));
        }
        if let (Some(m), Some(d)) = (b.max_durability, b.durability) {
            let (m, d) = (
                m.raw.saturating_sub(m.save_add),
                d.raw.saturating_sub(d.save_add),
            );
            if m > 0 {
                out.push(TipLine::new(
                    format!("Durability: {d} of {m}"),
                    color::WHITE,
                ));
            }
        }
        if let Some(n) = b.quantity {
            out.push(TipLine::new(format!("Quantity: {n}"), color::WHITE));
        }
        if let Some(c) = self.codes.get(&b.code) {
            for (label, v) in [
                ("Required Strength", u32::from(c.req_str)),
                ("Required Dexterity", u32::from(c.req_dex)),
                ("Required Level", u32::from(c.req_lvl)),
            ] {
                // `item-tips.md` §3.3 r4: the level line only when R > 1.
                let min = if label == "Required Level" { 1 } else { 0 };
                if v > min {
                    out.push(TipLine::new(format!("{label}: {v}"), color::WHITE));
                }
            }
        }
        if b.sockets.is_some_and(|s| s > 0) {
            out.push(TipLine::new(
                format!("Socketed ({})", b.sockets.unwrap_or(0)),
                color::BLUE,
            ));
        }
        // Properties, shown only for an identified item (`bitstream.md`
        // §4.3: the lists are not sent otherwise).
        if ident {
            let own: Vec<Stat> = b
                .lists
                .first()
                .into_iter()
                .flatten()
                .flatten()
                .copied()
                .collect();
            out.extend(self.stat_lines(&own, color::BLUE));
            // The set lists of a set item (green).
            let sets: Vec<Stat> = b
                .lists
                .iter()
                .skip(1)
                .flatten()
                .flatten()
                .copied()
                .collect();
            out.extend(self.stat_lines(&sets, color::GREEN));
        }
        // A set item names its set at the foot, then the bonuses of the
        // set (PROVISIONAL, REC-242: all steps shown; layout unverified).
        if ident && q == quality::SET {
            let name = qf
                .file_index
                .and_then(|i| self.set_of_item.get(i as usize))
                .and_then(|&s| self.set_names.get(usize::from(s)))
                .and_then(|&id| self.strings.get_id(id))
                .map(String::from_utf16_lossy);
            out.extend(name.map(|n| TipLine::new(n, color::GOLD)));
            if let Some(i) = qf.file_index {
                out.extend(self.set_lines(i as usize));
            }
        }
        out
    }

    /// The name colour (`ui/item-tips.md` §4 rules 1, 3, 4; later rules
    /// win). Rule 2 (an unidentified store item in modes 1–9 → 0) needs
    /// the store state, which the tip does not take.
    fn name_color(&self, b: &ItemBits) -> u16 {
        const ETHEREAL: u32 = 0x40_0000;
        const BROKEN: u32 = 0x100;
        const SPECIAL: [&[u8; 4]; 11] = [
            b"ceh ", b"bet ", b"fed ", b"tes ", b"toa ", b"dhn ", b"bey ", b"mbr ", b"pk1 ",
            b"pk2 ", b"pk3 ",
        ];
        let mut c = match b.quality {
            quality::MAGIC => color::BLUE,
            quality::SET => color::GREEN,
            quality::RARE => color::YELLOW,
            quality::UNIQUE => color::GOLD,
            quality::CRAFTED => color::ORANGE,
            quality::TEMPERED => color::TEMPERED,
            quality::LOW..=3 if b.flags & (0x800 | ETHEREAL) != 0 => color::GREY,
            _ => color::WHITE,
        };
        let rune = self
            .lookup
            .find_code(b.code)
            .is_some_and(|i| self.lookup.is_type(i, d2_sim::items::ty::RUNE as i16));
        if rune || SPECIAL.contains(&&b.code) {
            c = color::ORANGE;
        }
        if b.flags & BROKEN != 0 {
            c = color::RED;
        }
        c
    }

    /// The list values of a stream list (`bitstream.md` §4.6 r4): a
    /// stat's own entry is sent as value >> `ValShift`; the partners
    /// written after 17, 48, 50, 52, 54, 57 are sent unshifted.
    pub fn stat_list(&self, stats: &[Stat]) -> StatList {
        let mut l = StatList::default();
        let mut partners: &[u16] = &[];
        for s in stats {
            let partner = partners.first() == Some(&s.stat);
            partners = if partner {
                &partners[1..]
            } else {
                match s.stat {
                    17 => &[18],
                    48 => &[49],
                    50 => &[51],
                    52 => &[53],
                    54 => &[55, 56],
                    57 => &[58, 59],
                    _ => &[],
                }
            };
            let v = s.value() as i32;
            let shift = self
                .stats
                .get(usize::from(s.stat))
                .map_or(0, |d| d.valshift);
            l.add(s.stat, s.param, if partner { v } else { v << shift });
        }
        l
    }

    /// The set bonus lines under a set item's name (REC-242).
    fn set_lines(&self, item_row: usize) -> Vec<TipLine> {
        let Some(b) = set::bonuses(&self.lookup, item_row) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (pieces, stats) in &b.partial {
            let _ = pieces;
            out.extend(self.stat_lines(stats, color::GREEN));
        }
        out.extend(self.stat_lines(&b.full, color::ORANGE));
        out
    }

    /// The property lines (`item-tips.md` §6, multi-line) of `stats`,
    /// top line first.
    fn stat_lines(&self, stats: &[Stat], color: u16) -> Vec<TipLine> {
        let l = self.stat_list(stats);
        let args = props::PropArgs {
            undead: false,
            multi: true,
            label: &[],
        };
        let t = props::property_text(
            self,
            &desc::Viewer::default(),
            &props::PropItem::default(),
            &l,
            &args,
        );
        let mut lines: Vec<TipLine> = t
            .split(|&u| u == u16::from(b'\n'))
            .filter(|l| !l.is_empty())
            .map(|l| TipLine {
                text: l.to_vec(),
                color,
            })
            .collect();
        lines.reverse();
        lines
    }

    /// Whether a character with these base stats meets the requirements
    /// of an item with `code` (strength, dexterity, level; REC-242: item
    /// stat modifiers of the requirements are not applied).
    pub fn can_use(&self, code: [u8; 4], strength: i32, dexterity: i32, level: i32) -> bool {
        self.codes.get(&code).is_none_or(|c| {
            i32::from(c.req_str) <= strength
                && i32::from(c.req_dex) <= dexterity
                && i32::from(c.req_lvl) <= level
        })
    }

    /// The tip of a vendor's item ([`shop_marks`] over its lines).
    pub fn shop_lines(&self, stream: &[u8], price: u32, usable: bool) -> Vec<TipLine> {
        shop_marks(self.lines(stream), price, usable)
    }
}

/// A vendor tip: a price line at the foot and, when the player cannot use
/// the item, its white lines (requirements, base facts) in red
/// (REC-242: the price text and the colour are d2rs-own, unverified).
pub fn shop_marks(mut lines: Vec<TipLine>, price: u32, usable: bool) -> Vec<TipLine> {
    if lines.is_empty() {
        return lines;
    }
    if !usable {
        for l in lines.iter_mut().filter(|l| l.color == color::WHITE) {
            l.color = color::RED;
        }
    }
    lines.push(TipLine::new(
        format!("Price: {price}"),
        if usable { color::GOLD } else { color::RED },
    ));
    lines
}

impl desc::DescNames for ItemTips {
    fn string(&self, id: u16) -> Vec<u16> {
        self.strings
            .get_id(id)
            .map(<[u16]>::to_vec)
            .unwrap_or_default()
    }
    fn stat_desc(&self, s: u16) -> Option<StatDesc> {
        self.stats.get(usize::from(s)).copied()
    }
    fn skill_name(&self, skill: u32) -> Option<u16> {
        self.skills.get(skill as usize)?.0
    }
    fn skill_class(&self, skill: u32) -> Option<i8> {
        let (_, class) = self.skills.get(skill as usize)?;
        Some(*class as i8)
    }
    fn skill_count(&self) -> u32 {
        self.skills.len() as u32
    }
    fn class_strings(&self, class: u32) -> Option<desc::ClassStrings> {
        self.class_strings.get(class as usize).copied()
    }
    fn montype_name(&self, row: u32) -> Option<u16> {
        self.montype.get(row as usize).copied()
    }
    fn monstats_name(&self, row: u32) -> Option<u16> {
        self.monstats.get(row as usize).copied()
    }
    fn desc_list(&self) -> Vec<u16> {
        self.desc_order.clone()
    }
    fn group_members(&self, g: u16) -> Vec<u16> {
        (0..self.stats.len() as u16)
            .filter(|&s| self.stats[usize::from(s)].dgrp == g)
            .collect()
    }
}

/// Draws a tip box for `lines` near `mouse` on a frame of `screen`
/// (width, height): dark fill tiles and one centered line each, over
/// everything drawn before. d2rs-own, unverified: above the point when it
/// fits, else below.
pub fn draw_tip(
    lines: &[TipLine],
    mouse: Point,
    screen: (i32, i32),
    fonts: Option<&FontMeasure>,
    files: &UiFiles,
    out: &mut dyn UiDrawSink,
) {
    if lines.is_empty() {
        return;
    }
    let width = |l: &TipLine| {
        fonts
            .and_then(|f| f.max_width(FONT, &l.text))
            .unwrap_or(8 * l.text.len() as i32)
    };
    let w = lines.iter().map(width).max().unwrap_or(0) + 16;
    let h = LINE_H * lines.len() as i32 + 8;
    let x = (mouse.x - w / 2).clamp(0, (screen.0 - w).max(0));
    let mut y = mouse.y - h - 12;
    if y < 0 {
        y = (mouse.y + 28).min((screen.1 - h).max(0));
    }
    if let Some(file) = files.id(FILL_FILE) {
        let (tw, th) = (FILL_W as i32, FILL_H as i32);
        let mut ty = y;
        while ty < y + h {
            let mut tx = x;
            while tx < x + w {
                let cw = (x + w - tx).min(tw);
                let ch = (y + h - ty).min(th);
                out.push(UiDraw::Image(ImageRequest {
                    image: ImageRef { file, frame: DARK },
                    at: Point::new(tx, ty),
                    clip: Rect::new(tx, ty, cw as u16, ch as u16),
                }));
                tx += tw;
            }
            ty += th;
        }
    }
    for (i, l) in lines.iter().enumerate() {
        out.push(UiDraw::Text(TextRequest {
            text: l.text.clone(),
            at: Point::new(x, y + 4 + LINE_H * (i as i32 + 1) - 3),
            style: TextStyle {
                font: FONT,
                color: l.color,
            },
            opts: TextOpts::Draw {
                centered: true,
                block_w: Some(w),
                mode: 5,
            },
            clip: FRAME,
        }));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use d2_proto::item_bits::hflag;
    use d2_sim::items::tables::ItemRec;
    use std::collections::HashMap;

    struct Strs {
        keys: HashMap<String, Vec<u16>>,
        ids: HashMap<u16, Vec<u16>>,
    }

    impl StringLookup for Strs {
        fn get(&self, key: &str) -> Option<&[u16]> {
            self.keys.get(key).map(Vec::as_slice)
        }
        fn get_id(&self, id: u16) -> Option<&[u16]> {
            self.ids.get(&id).map(Vec::as_slice)
        }
    }

    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// A cap (name id 7, requires level 3), magicprefix row 0 `Sturdy`,
    /// magicsuffix row 0 `Fox` (wire ids 1), stat 1 described by string 9 (`descfunc` 1, value first).
    pub(crate) fn tips() -> ItemTips {
        tips_with(ItemTables::default())
    }

    fn tips_with(items: ItemTables) -> ItemTips {
        let items = ItemTables {
            items: vec![ItemRec {
                code: *b"cap ",
                ..ItemRec::default()
            }],
            ..items
        };
        let mut codes = BTreeMap::new();
        codes.insert(
            *b"cap ",
            CodeText {
                name_id: 7,
                req_lvl: 3,
                ..CodeText::default()
            },
        );
        let strs = Strs {
            keys: [
                ("Sturdy", "Sturdy"),
                ("Fox", "of the Fox"),
                ("Greymaker", "Greymaker"),
                ("Sigon's Visor", "Sigon's Visor"),
            ]
            .map(|(k, v)| (k.to_owned(), u16s(v)))
            .into(),
            ids: [
                (7, "Cap"),
                (9, "to Life"),
                (11, "Sigon's Steel"),
                (12, "to Mana"),
                (13, "Defense"),
                (desc::sid::SP, " "),
                (desc::sid::PLUS, "+"),
                (desc::sid::NL, "\n"),
            ]
            .map(|(k, v)| (k, u16s(v)))
            .into(),
        };
        ItemTips {
            lookup: Arc::new(items),
            codes,
            desc_order: vec![3, 2, 1],
            stats: vec![
                StatDesc::default(),
                StatDesc {
                    priority: 5,
                    func: 1,
                    val: 1,
                    pos: 9,
                    ..StatDesc::default()
                },
                StatDesc {
                    priority: 5,
                    func: 1,
                    val: 1,
                    pos: 12,
                    ..StatDesc::default()
                },
                StatDesc {
                    priority: 5,
                    func: 3,
                    val: 1,
                    pos: 13,
                    ..StatDesc::default()
                },
            ],
            // Row 0 of each (wire id 1, `affixes.md` §1 r1).
            magic_prefix: vec!["Sturdy".into()],
            magic_suffix: vec!["Fox".into()],
            rare_prefix: Vec::new(),
            rare_suffix: Vec::new(),
            unique: vec!["Greymaker".into()],
            set: vec!["Sigon's Visor".into()],
            set_of_item: vec![0],
            set_names: vec![11],
            skills: Vec::new(),
            class_strings: Vec::new(),
            montype: Vec::new(),
            monstats: Vec::new(),
            strings: Arc::new(strs),
        }
    }

    fn text(l: &TipLine) -> String {
        String::from_utf16_lossy(&l.text)
    }

    fn magic_cap(flags: u32) -> ItemBits {
        let mut b = ItemBits {
            flags,
            code: *b"cap ",
            quality: 4,
            ..ItemBits::default()
        };
        b.quality_fields.magic = Some((1, 1));
        b.lists = vec![Some(vec![Stat {
            stat: 1,
            param: 0,
            raw: 25,
            save_add: 10,
        }])];
        b
    }

    #[test]
    fn an_identified_magic_item_shows_name_requirements_and_properties() {
        let lines = tips().lines_of(&magic_cap(hflag::IDENTIFIED));
        let got: Vec<(String, u16)> = lines.iter().map(|l| (text(l), l.color)).collect();
        assert_eq!(
            got,
            [
                ("Sturdy Cap of the Fox".to_owned(), color::BLUE),
                ("Required Level: 3".to_owned(), color::WHITE),
                ("+15 to Life".to_owned(), color::BLUE),
            ]
        );
    }

    // "Required Level:" only when the level is above 1.
    // Covers: specs/ui/item-tips.md §3.3 r4
    #[test]
    fn a_level_1_requirement_shows_no_line() {
        let mut t = tips();
        t.codes.get_mut(b"cap ").expect("cap").req_lvl = 1;
        let lines = t.lines_of(&magic_cap(hflag::IDENTIFIED));
        assert!(lines.iter().all(|l| !text(l).starts_with("Required Level")));
        t.codes.get_mut(b"cap ").expect("cap").req_lvl = 2;
        let lines = t.lines_of(&magic_cap(hflag::IDENTIFIED));
        assert!(lines.iter().any(|l| text(l) == "Required Level: 2"));
    }

    // Wire affix ids are row + 1 (0 = none); rare ids index the rare
    // array with the raresuffix rows first.
    // Covers: specs/items/affixes.md §1 r1
    // Covers: specs/items/bitstream.md §4.2
    #[test]
    fn affix_ids_are_rows_plus_one() {
        let mut t = tips();
        t.magic_prefix = vec!["Sturdy".into(), "Strong".into()];
        t.magic_suffix = vec!["Fox".into(), "Wolf".into()];
        t.rare_suffix = vec!["Fox".into(), "Wolf".into()];
        t.rare_prefix = vec!["Sturdy".into(), "Strong".into()];
        let strs = Strs {
            keys: [
                ("Sturdy", "Sturdy"),
                ("Strong", "Strong"),
                ("Fox", "of the Fox"),
                ("Wolf", "of the Wolf"),
            ]
            .map(|(k, v)| (k.to_owned(), u16s(v)))
            .into(),
            ids: HashMap::new(),
        };
        t.strings = Arc::new(strs);
        let name = |p: u16, s: u16| t.magic_name(&t.magic_prefix, p).zip(Some(s));
        assert_eq!(t.magic_name(&t.magic_prefix, 0), None);
        assert_eq!(name(1, 0).map(|x| x.0).as_deref(), Some("Sturdy"));
        assert_eq!(t.magic_name(&t.magic_prefix, 2).as_deref(), Some("Strong"));
        assert_eq!(
            t.magic_name(&t.magic_suffix, 2).as_deref(),
            Some("of the Wolf")
        );
        assert_eq!(t.magic_name(&t.magic_suffix, 3), None);
        // Rare: suffix ids 1–2, prefix ids 3–4.
        assert_eq!(t.rare_name(0, false), None);
        assert_eq!(t.rare_name(1, false).as_deref(), Some("of the Fox"));
        assert_eq!(t.rare_name(2, false).as_deref(), Some("of the Wolf"));
        assert_eq!(t.rare_name(3, false), None);
        assert_eq!(t.rare_name(2, true), None);
        assert_eq!(t.rare_name(3, true).as_deref(), Some("Sturdy"));
        assert_eq!(t.rare_name(4, true).as_deref(), Some("Strong"));
    }

    #[test]
    fn an_unidentified_item_shows_its_base_name_and_no_properties() {
        let lines = tips().lines_of(&magic_cap(0));
        let got: Vec<(String, u16)> = lines.iter().map(|l| (text(l), l.color)).collect();
        assert_eq!(
            got,
            [
                ("Cap".to_owned(), color::BLUE),
                ("Unidentified".to_owned(), color::RED),
                ("Required Level: 3".to_owned(), color::WHITE),
            ]
        );
    }

    #[test]
    fn a_compact_stream_decodes_to_its_name() {
        // flags (compact | identified), version 0x65, mode 0, location
        // 0 / 0 / 0 / page 0, code `cap `.
        let bits: Vec<(u32, u32)> = vec![
            (hflag::COMPACT | hflag::IDENTIFIED, 32),
            (0x65, 10),
            (0, 3),
            (0, 4),
            (0, 4),
            (0, 4),
            (0, 3),
            (u32::from_le_bytes(*b"cap "), 32),
        ];
        let (mut out, mut acc, mut n) = (Vec::new(), 0u64, 0u32);
        for (v, w) in bits {
            acc |= u64::from(v) << n;
            n += w;
            while n >= 8 {
                out.push(acc as u8);
                acc >>= 8;
                n -= 8;
            }
        }
        if n > 0 {
            out.push(acc as u8);
        }
        let lines = tips().lines(&out);
        assert_eq!(lines.first().map(text).as_deref(), Some("Cap"));
        assert!(tips().lines(&[1, 2]).is_empty(), "garbage gives no tip");
    }

    #[test]
    fn the_box_has_one_centered_line_per_row_and_stays_on_screen() {
        let lines = tips().lines_of(&magic_cap(hflag::IDENTIFIED));
        let files = UiFiles::new(&[]);
        let mut out: Vec<UiDraw> = Vec::new();
        draw_tip(
            &lines,
            Point::new(790, 5),
            (800, 600),
            None,
            &files,
            &mut out,
        );
        let texts: Vec<&TextRequest> = out
            .iter()
            .filter_map(|d| match d {
                UiDraw::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), lines.len());
        for t in &texts {
            let TextOpts::Draw { block_w, .. } = t.opts else {
                panic!("draw call");
            };
            let w = block_w.unwrap();
            assert!(t.at.x >= 0 && t.at.x + w <= 800, "{:?}", t.at);
            assert!(t.at.y >= 0 && t.at.y <= 600);
        }
        assert_eq!(texts[0].style.color, color::BLUE);
        let mut none: Vec<UiDraw> = Vec::new();
        draw_tip(&[], Point::new(1, 1), (800, 600), None, &files, &mut none);
        assert!(none.is_empty());
    }

    fn quality_cap(quality: u8, index: u32) -> ItemBits {
        let mut b = ItemBits {
            flags: hflag::IDENTIFIED,
            code: *b"cap ",
            quality,
            ..ItemBits::default()
        };
        b.quality_fields.file_index = Some(index);
        b
    }

    // Covers: specs/ui/item-tips.md §4 r1, §4 r3, §4 r4
    #[test]
    fn the_name_colour_follows_quality_flags_codes_and_broken() {
        let t = tips();
        let c = |quality: u8, flags: u32, code: &[u8; 4]| {
            t.name_color(&ItemBits {
                flags,
                code: *code,
                quality,
                ..ItemBits::default()
            })
        };
        // r1: low / normal / superior are white, grey when socketed or
        // ethereal; tempered is `ÿc:`.
        assert_eq!(c(quality::LOW, 0, b"cap "), color::WHITE);
        assert_eq!(c(2, 0, b"cap "), color::WHITE);
        assert_eq!(c(quality::LOW, hflag::SOCKETED, b"cap "), color::GREY);
        assert_eq!(c(3, 0x40_0000, b"cap "), color::GREY);
        assert_eq!(c(quality::MAGIC, hflag::SOCKETED, b"cap "), color::BLUE);
        assert_eq!(c(quality::TEMPERED, 0, b"cap "), color::TEMPERED);
        // r3: the listed codes are orange; r4: broken is red (last).
        assert_eq!(c(2, 0, b"pk1 "), color::ORANGE);
        assert_eq!(c(quality::UNIQUE, 0, b"toa "), color::ORANGE);
        assert_eq!(c(quality::MAGIC, 0x100, b"cap "), color::RED);
        assert_eq!(c(2, 0x100, b"mbr "), color::RED);
    }

    #[test]
    fn a_unique_item_shows_its_name_in_gold() {
        let got: Vec<(String, u16)> = tips()
            .lines_of(&quality_cap(quality::UNIQUE, 0))
            .iter()
            .map(|l| (text(l), l.color))
            .collect();
        assert_eq!(
            got[..2],
            [
                ("Greymaker".to_owned(), color::GOLD),
                ("Cap".to_owned(), color::GOLD)
            ]
        );
    }

    #[test]
    fn a_set_item_shows_its_name_in_green_and_its_set_in_gold() {
        let got: Vec<(String, u16)> = tips()
            .lines_of(&quality_cap(quality::SET, 0))
            .iter()
            .map(|l| (text(l), l.color))
            .collect();
        assert_eq!(got[0], ("Sigon's Visor".to_owned(), color::GREEN));
        assert_eq!(got[1], ("Cap".to_owned(), color::GREEN));
        assert_eq!(got.last(), Some(&("Sigon's Steel".to_owned(), color::GOLD)));
    }

    /// A two-piece set: 2 pieces give +5 life and +3 mana, the full set
    /// (3 pieces) gives 9 defense; the item's own set list gives +4 life.
    #[test]
    fn a_set_item_shows_its_set_list_steps_and_full_bonus() {
        use d2_sim::items::tables::{PropRec, PropSlot, PropertyRec, SetItemRec, SetRec};
        let prop = |stat| {
            let mut p = PropertyRec::default();
            p.slots[0] = PropSlot {
                func: 1,
                stat,
                set: 0,
                val: 0,
            };
            p
        };
        let r = |code, v| PropRec {
            code,
            param: 0,
            min: v,
            max: v,
        };
        let mut partial = [PropRec::NONE; 8];
        partial[0] = r(0, 5);
        partial[1] = r(1, 3);
        let mut full = [PropRec::NONE; 8];
        full[0] = r(2, 9);
        let t = tips_with(ItemTables {
            properties: vec![prop(1), prop(2), prop(3)],
            valshift: vec![0; 8],
            sets: vec![SetRec {
                count: 3,
                partial,
                full,
            }],
            setitems: vec![SetItemRec::default()],
            ..ItemTables::default()
        });
        let mut b = quality_cap(quality::SET, 0);
        b.lists = vec![
            None,
            Some(vec![Stat {
                stat: 1,
                param: 0,
                raw: 4,
                save_add: 0,
            }]),
        ];
        let got: Vec<(String, u16)> = t.lines_of(&b).iter().map(|l| (text(l), l.color)).collect();
        assert_eq!(
            got,
            [
                ("Sigon's Visor".to_owned(), color::GREEN),
                ("Cap".to_owned(), color::GREEN),
                ("Required Level: 3".to_owned(), color::WHITE),
                ("+4 to Life".to_owned(), color::GREEN),
                ("Sigon's Steel".to_owned(), color::GOLD),
                ("+5 to Life".to_owned(), color::GREEN),
                ("+3 to Mana".to_owned(), color::GREEN),
                ("9 Defense".to_owned(), color::ORANGE),
            ]
        );
    }

    #[test]
    fn a_shop_tip_has_a_price_and_marks_unusable_items_red() {
        let lines = tips().lines_of(&magic_cap(hflag::IDENTIFIED));
        let ok = shop_marks(lines.clone(), 120, true);
        assert_eq!(ok.last(), Some(&TipLine::new("Price: 120", color::GOLD)));
        assert_eq!(ok[1].color, color::WHITE);
        let no = shop_marks(lines, 120, false);
        assert_eq!(no[1], TipLine::new("Required Level: 3", color::RED));
        assert_eq!(no.last(), Some(&TipLine::new("Price: 120", color::RED)));
        assert!(shop_marks(Vec::new(), 1, true).is_empty());
    }
}
