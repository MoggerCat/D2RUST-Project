// Spec: specs/sim/stat-lists.md; specs/sim/stats.md
//! Stat lists of a stats recording (`stats-raw-1`, `record_stats.py`;
//! the format `check_stats.py` reads) replayed through
//! `d2_sim::stats::StatLists`.
//!
//! The recording hooks the entry of every list operation on server lists
//! and dumps each list tree the first time it is touched (`ssd`), then
//! periodically (`ssn`). The harness:
//!
//! 1. builds `StatData` from the recorded runtime tables (`stab`): the
//!    itemstatcost rows go through `d2_data::fixup::records::stat_ops`,
//!    whose derived columns (+0x51..+0x53, the dependants list +0x5E, the
//!    op table +0xDE) must equal the recorded ones; charstats and the
//!    states' `life` group from the same record;
//! 2. seeds every dumped tree into d2-sim through its own operations
//!    (allocate, set the base values, attach the chains oldest first,
//!    toggle the state bits, rebuild the mod array with `unit_set`), with
//!    value-change callbacks muted, then requires the full arrays and the
//!    mod arrays d2-sim derives to equal the dump (`stat-lists.md` §6,
//!    §11);
//! 3. applies every recorded operation through the d2-sim call of the
//!    same name (`ss` set, `sa` add, `sr` remove-all, `sat` attach, `sdt`
//!    detach, `sfr` free, `sdy` dynamic toggle, `sbt` by-time refresh,
//!    `stg` state toggle, `sxp` expiry) and requires the value-change
//!    callbacks d2-sim makes (`StatHost::on_callback`) to be the recorded
//!    `scb` records in order: list, key, old and new value, unit
//!    argument (§7);
//! 4. compares every snapshot list (`ssn`) with d2-sim: base, full and
//!    mod arrays, the owner's state bits, chain links (`par`, `prev`,
//!    `next`, `last`, `setl`) and the flag bits the stat code owns
//!    (DYNAMIC, PERMANENT); records that carry a list's flags (`fl`)
//!    check the same bits.
//!
//! Comparison is exact and stops at the first difference (METHODS M01),
//! named by the record's line index (as `check_stats.py` numbers them).
//!
//! What runs inside a recorded callback (between `scb` and its `scx`):
//! the writes d2-sim's own server callback makes (§7.2: the current
//! life, mana or stamina after a max change, `ss` of stat max − 1 on the
//! callback's list) are d2-sim's and are not replayed; every other record
//! there is work of systems the stat code calls through `StatHost` (item
//! events, skill and state handlers), which the harness applies after
//! the operation returns, with its own callbacks compared in turn. A
//! monster's HPREGEN write after a max-life change (§7.2, monstats
//! `DamageRegen`) is applied the same way: the recording has no monstats
//! column, so the harness's `StatData` holds no `DamageRegen`.
//!
//! Not replayed (counted): the regeneration entry points (`sgl`, `sgs`,
//! `sgm`, `sgx`; their writes follow as `ss` / `sa` and are applied):
//! d2-sim computes them in `units::dispatch::{player_regen,
//! monster_regen}`, which run on a whole `Sim` (game, units, timers).
//! Expiry (`sxp` … `sxe`) runs `StatLists::expire_lists`; the lists it
//! frees, in order, must be the recorded `sxf` records.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use d2_data::bin::BinTable;
use d2_data::fixup::maps;
use d2_data::fixup::records::stat_ops;
use d2_data::tables::{Itemstatcost, Record, States};
use d2_sim::stats::lists::{flag, RemoveCallback};
use d2_sim::stats::states::group;
use d2_sim::stats::{
    key, key_stat, CallbackEvent, ClassStats, ListId, StatData, StatHost, StatLists, StatTable,
    StateTable, ValueCallback, DEFAULT_RESCALE_PRECISION,
};
use d2_sim::units::{UnitId, UnitType};
use serde_json::Value;

use crate::raw::{bad, Fields, HarnessError, Mismatch, RawRecording, STATS_RAW};

/// The flag bits the stat code owns (`check_stats.py` MODEL_BITS).
const MODEL_BITS: u32 = flag::DYNAMIC | flag::PERMANENT;

/// The remove callback the harness puts on every list to see the order
/// of detaches (a host call; the stat code only passes it on).
const MARK: RemoveCallback = RemoveCallback(u32::MAX);

/// What a replay compared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatsStats {
    pub ticks: usize,
    /// Operations applied (set, add, remove-all, attach, detach, free,
    /// dynamic, by-time, state toggle).
    pub operations: usize,
    /// Of those, applied as host work from inside a callback.
    pub host_operations: usize,
    pub callbacks: usize,
    pub expiries: usize,
    pub expiry_frees: usize,
    /// Lists seeded from a dump (`ssd`, or a snapshot list not yet seen).
    pub seeded_lists: usize,
    pub snapshots: usize,
    /// Snapshot lists compared.
    pub lists_compared: usize,
    /// Regeneration entry points not replayed.
    pub regen_skipped: usize,
}

/// The [`StatHost`] of the replay.
#[derive(Debug, Default)]
struct Host {
    /// Seeding: callbacks are muted.
    seeding: bool,
    callbacks: Vec<CallbackEvent>,
    /// Lists detached with the harness's mark, in order.
    removed: Vec<ListId>,
    /// unit → act (the recording's act address).
    act: BTreeMap<UnitId, String>,
    /// act → base time (`senv`).
    env: BTreeMap<String, i32>,
}

impl StatHost for Host {
    fn act_time(&self, unit: UnitId) -> Option<i32> {
        self.env.get(self.act.get(&unit)?).copied()
    }

    fn on_callback(&mut self, _lists: &StatLists, ev: &CallbackEvent) {
        if !self.seeding {
            self.callbacks.push(*ev);
        }
    }

    fn list_removed(
        &mut self,
        _lists: &mut StatLists,
        _unit: UnitId,
        _state: u32,
        list: ListId,
        callback: RemoveCallback,
    ) {
        if callback == MARK && !self.seeding {
            self.removed.push(list);
        }
    }
}

/// The records a callback walk met.
#[derive(Debug, Default)]
struct Walk {
    /// The first record after the walk.
    next: usize,
    /// Host work to apply after the operation (record indexes).
    deferred: Vec<usize>,
    /// Expiry mode: the window's records at depth 0 that are the stat
    /// code's own (`sxf`, `sdt`, `sfr`).
    own: Vec<usize>,
}

struct Replay<'a> {
    name: &'a str,
    recs: &'a [Value],
    lists: Option<StatLists>,
    host: Host,
    addr: BTreeMap<String, ListId>,
    names: BTreeMap<ListId, String>,
    units: BTreeMap<String, UnitId>,
    unit_names: BTreeMap<UnitId, String>,
    stats: StatsStats,
}

/// Replays a `stats-raw-1` recording through `d2_sim::stats`.
pub fn replay_stats(rec: &RawRecording) -> Result<StatsStats, HarnessError> {
    if rec.format != STATS_RAW {
        return Err(bad(&rec.name, 0, format!("not a {STATS_RAW} recording")));
    }
    let mut r = Replay {
        name: &rec.name,
        recs: &rec.records,
        lists: None,
        host: Host::default(),
        addr: BTreeMap::new(),
        names: BTreeMap::new(),
        units: BTreeMap::new(),
        unit_names: BTreeMap::new(),
        stats: StatsStats::default(),
    };
    let mut i = 1;
    while i < r.recs.len() {
        i = r.record(i)?;
    }
    Ok(r.stats)
}

fn hexes(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

impl<'a> Replay<'a> {
    fn f(&self, i: usize) -> Fields<'a> {
        Fields {
            name: self.name,
            i,
            r: &self.recs[i],
        }
    }

    fn mismatch(&self, at: usize, field: impl Into<String>, detail: String) -> HarnessError {
        HarnessError::Mismatch(Mismatch {
            source: self.name.to_owned(),
            at: at as u64,
            field: field.into(),
            detail,
        })
    }

    fn lists(&mut self, i: usize) -> Result<&mut StatLists, HarnessError> {
        Ok(self.split(i)?.0)
    }

    /// The lists and the host, borrowed apart.
    fn split(&mut self, i: usize) -> Result<(&mut StatLists, &mut Host), HarnessError> {
        match self.lists.as_mut() {
            Some(l) => Ok((l, &mut self.host)),
            None => Err(bad(self.name, i, "list operation before the tables (stab)")),
        }
    }

    fn list(&self, i: usize, a: &str) -> Result<ListId, HarnessError> {
        let id = self
            .addr
            .get(a)
            .copied()
            .ok_or_else(|| bad(self.name, i, format!("list {a} was never seeded")))?;
        if !self.lists.as_ref().is_some_and(|l| l.is_live(id)) {
            return Err(self.mismatch(i, "list", format!("list {a} is freed in d2-sim")));
        }
        Ok(id)
    }

    fn unit(&mut self, uid: &str) -> UnitId {
        if let Some(&u) = self.units.get(uid) {
            return u;
        }
        let u = UnitId(self.units.len() as u32 + 1);
        self.units.insert(uid.to_owned(), u);
        self.unit_names.insert(u, uid.to_owned());
        u
    }

    fn known_unit(&self, i: usize, uid: &str) -> Result<UnitId, HarnessError> {
        self.units
            .get(uid)
            .copied()
            .ok_or_else(|| bad(self.name, i, format!("unit {uid} never seen")))
    }

    fn name_of(&self, l: Option<ListId>) -> Option<String> {
        l.map(|l| {
            self.names
                .get(&l)
                .cloned()
                .unwrap_or_else(|| "(not in the recording)".into())
        })
    }

    fn unit_name(&self, u: Option<UnitId>) -> Option<String> {
        u.map(|u| self.unit_names.get(&u).cloned().unwrap_or_default())
    }

    fn adopt_units(&mut self, units: &Value) {
        if let Some(m) = units.as_object() {
            for (uid, info) in m {
                let u = self.unit(uid);
                if let Some(act) = info["act"].as_str() {
                    self.host.act.insert(u, act.to_owned());
                } else {
                    self.host.act.remove(&u);
                }
            }
        }
    }

    fn map_list(&mut self, a: &str, id: ListId) {
        self.addr.insert(a.to_owned(), id);
        self.names.insert(id, a.to_owned());
    }

    fn unmap_list(&mut self, id: ListId) {
        if let Some(a) = self.names.remove(&id) {
            self.addr.remove(&a);
        }
    }

    /// Records with a list's flags: the stat code's bits must agree; the
    /// others (set by callers the recording does not hook, e.g. the expire
    /// frame's NEWLENGTH) are taken from the recording.
    fn check_flags(&mut self, i: usize, l: ListId) -> Result<(), HarnessError> {
        let Some(fl) = self.recs[i]["fl"].as_u64().map(|f| f as u32) else {
            return Ok(());
        };
        let lists = self.lists(i)?;
        let ours = lists.flags(l);
        if (ours ^ fl) & MODEL_BITS != 0 {
            let a = self.names.get(&l).cloned().unwrap_or_default();
            return Err(self.mismatch(
                i,
                "flags",
                format!("list {a}: recorded {fl:#x}, d2-sim {ours:#x} (bits {MODEL_BITS:#x})"),
            ));
        }
        set_flags_exact(lists, l, (fl & !MODEL_BITS) | (ours & MODEL_BITS));
        Ok(())
    }

    // ---- the record loop ------------------------------------------------

    /// Handles the record at `i` (top level); returns the next index.
    fn record(&mut self, i: usize) -> Result<usize, HarnessError> {
        let r = &self.recs[i];
        let k = r["k"].as_str().unwrap_or("");
        match k {
            "header" | "footer" | "step" => {}
            "tick" => self.stats.ticks += 1,
            "stab" => self.tables(i)?,
            "senv" => {
                self.host.env = r["env"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .filter_map(|(a, v)| {
                                Some((a.clone(), i32::try_from(v.as_i64()?).ok()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            "ssd" => {
                self.adopt_units(&r["units"]);
                let dumps = r["lists"].as_array().cloned().unwrap_or_default();
                self.seed(i, &dumps)?;
            }
            "ssn" => self.snapshot(i)?,
            "sax" => self.alloc_extended(i)?,
            "sxp" => return self.expiry(i),
            "sgl" | "sgs" | "sgm" | "sgx" => self.stats.regen_skipped += 1,
            "scb" => {
                let f = self.f(i);
                return Err(self.mismatch(
                    i,
                    "callback",
                    format!(
                        "recorded callback on {} key {:#x} {} -> {}; d2-sim made none",
                        f.str("L")?,
                        f.i64("key")?,
                        f.i64("old")?,
                        f.i64("new")?
                    ),
                ));
            }
            "scx" => return Err(bad(self.name, i, "callback return without a callback")),
            "sxf" | "sxe" => return Err(bad(self.name, i, format!("{k} outside an expiry"))),
            _ => return self.operation(i),
        }
        Ok(i + 1)
    }

    /// Applies the operation at `i`, compares its callbacks, applies the
    /// host work found inside them; returns the next index.
    fn operation(&mut self, i: usize) -> Result<usize, HarnessError> {
        self.host.callbacks.clear();
        if !self.apply(i)? {
            return Ok(i + 1);
        }
        self.stats.operations += 1;
        let cbs = std::mem::take(&mut self.host.callbacks);
        let walk = self.walk(i + 1, &cbs, false)?;
        self.stats.callbacks += cbs.len();
        for h in walk.deferred {
            self.stats.host_operations += 1;
            self.operation(h)?;
        }
        Ok(walk.next)
    }

    /// The d2-sim call of an operation record. False when the record is
    /// not an operation (unknown kinds are a format error).
    fn apply(&mut self, i: usize) -> Result<bool, HarnessError> {
        let f = self.f(i);
        let k = f.str("k")?.to_owned();
        match k.as_str() {
            "ss" | "sa" | "sr" => {
                let l = self.list(i, f.str("L")?)?;
                self.check_flags(i, l)?;
                let unit = match f.r["u"].as_str() {
                    Some(uid) => Some(self.known_unit(i, uid)?),
                    None => None,
                };
                let (s, v, layer) = if k == "sr" {
                    (0, 0, 0)
                } else {
                    (stat_of(&f)?, f.i32("v")?, layer_of(&f)?)
                };
                let host = &mut self.host;
                let lists = self.lists.as_mut().expect("checked by list()");
                match k.as_str() {
                    "ss" => {
                        lists.set(host, l, s, v, layer, unit);
                    }
                    "sa" => lists.add(host, l, s, v, layer),
                    _ => lists.remove_all(host, l),
                }
            }
            "sat" => {
                let l = self.list(i, f.str("L")?)?;
                self.check_flags(i, l)?;
                let u = self.known_unit(i, f.str("U")?)?;
                let reset = f.bool("r")?;
                let lists = self.lists.as_mut().expect("checked by list()");
                lists.attach(&mut self.host, u, l, reset);
            }
            "sdt" => {
                let l = self.list(i, f.str("L")?)?;
                self.check_flags(i, l)?;
                let lists = self.lists.as_mut().expect("checked by list()");
                lists.detach(&mut self.host, l);
            }
            "sfr" => {
                let l = self.list(i, f.str("L")?)?;
                let lists = self.lists.as_mut().expect("checked by list()");
                lists.free(&mut self.host, l);
                self.unmap_list(l);
            }
            "sdy" | "sbt" => {
                let u = self.known_unit(i, f.str("U")?)?;
                let item = self.known_unit(i, f.str("I")?)?;
                let on = k == "sdy" && f.bool("on")?;
                let (lists, host) = self.split(i)?;
                let Some(il) = lists.unit_list(item) else {
                    return Ok(false);
                };
                // Not attached to this unit: the original takes the equip
                // path, whose own records follow (`check_stats.py`).
                if lists.attached_unit(il) != Some(u) {
                    return Ok(false);
                }
                match (k.as_str(), on) {
                    ("sbt", _) => lists.by_time_refresh(host, u, il),
                    (_, true) => lists.make_dynamic(host, u, il, false),
                    (_, false) => lists.make_static(host, u, il, false),
                }
            }
            "ssd" => {
                self.adopt_units(&f.r["units"]);
                let dumps = f.r["lists"].as_array().cloned().unwrap_or_default();
                self.seed(i, &dumps)?;
                return Ok(false);
            }
            "sax" => {
                self.alloc_extended(i)?;
                return Ok(false);
            }
            "sgl" | "sgs" | "sgm" | "sgx" => {
                self.stats.regen_skipped += 1;
                return Ok(false);
            }
            "stg" => {
                let u = self.known_unit(i, f.str("U")?)?;
                let s = f.u32("s")?;
                let on = f.bool("on")?;
                self.lists(i)?.toggle_state(u, s, on);
            }
            _ => return Err(bad(self.name, i, format!("unknown record kind {k:?}"))),
        }
        Ok(true)
    }

    /// Walks the records from `j` pairing `scb` records with d2-sim's
    /// callbacks `cbs` (preorder, as both make them). Outside expiry mode
    /// the walk ends at the first record at depth 0 that is not a
    /// callback; in expiry mode at the `sxe` (inclusive).
    fn walk(
        &self,
        mut j: usize,
        cbs: &[CallbackEvent],
        expiry: bool,
    ) -> Result<Walk, HarnessError> {
        let mut out = Walk::default();
        let mut k = 0;
        // Open callbacks: (record, list, key).
        let mut stack: Vec<(usize, String, i32)> = Vec::new();
        loop {
            let Some(r) = self.recs.get(j) else {
                if let Some(c) = cbs.get(k) {
                    return Err(self.mismatch(
                        self.recs.len() - 1,
                        "callback",
                        format!(
                            "d2-sim callback {} not recorded (recording ended)",
                            self.cb_text(c)
                        ),
                    ));
                }
                if expiry || !stack.is_empty() {
                    return Err(bad(
                        self.name,
                        j,
                        "recording ended inside a callback or expiry",
                    ));
                }
                break;
            };
            let kind = r["k"].as_str().unwrap_or("");
            match kind {
                "scb" if !(stack.is_empty() && k == cbs.len() && !expiry) => {
                    let f = self.f(j);
                    let Some(c) = cbs.get(k) else {
                        return Err(self.mismatch(
                            j,
                            "callback",
                            format!(
                                "recorded callback on {} key {:#x} {} -> {}; d2-sim made no more",
                                f.str("L")?,
                                f.i64("key")?,
                                f.i64("old")?,
                                f.i64("new")?
                            ),
                        ));
                    };
                    self.compare_cb(j, c)?;
                    k += 1;
                    stack.push((j, f.str("L")?.to_owned(), f.i32("key")?));
                    j += 1;
                }
                "scx" if !stack.is_empty() => {
                    stack.pop();
                    j += 1;
                }
                _ if stack.is_empty() && !expiry => {
                    if let Some(c) = cbs.get(k) {
                        return Err(self.mismatch(
                            j,
                            "callback",
                            format!(
                                "d2-sim callback {} not recorded; the recording continues with {kind:?}",
                                self.cb_text(c)
                            ),
                        ));
                    }
                    break;
                }
                "sxe" if stack.is_empty() => {
                    if let Some(c) = cbs.get(k) {
                        return Err(self.mismatch(
                            j,
                            "callback",
                            format!(
                                "d2-sim callback {} not recorded in the expiry",
                                self.cb_text(c)
                            ),
                        ));
                    }
                    j += 1;
                    break;
                }
                "sxf" | "sdt" | "sfr" if stack.is_empty() => {
                    out.own.push(j);
                    j += 1;
                }
                _ => {
                    let own = stack.last().is_some_and(|(_, l, key)| {
                        let ks = key_stat(*key);
                        kind == "ss"
                            && r["L"].as_str() == Some(l.as_str())
                            && matches!(ks, 7 | 9 | 11)
                            && r["s"].as_u64() == Some(u64::from(ks) - 1)
                    });
                    if own {
                        j += 1;
                    } else {
                        out.deferred.push(j);
                        j = self.skip_windows(j + 1)?;
                    }
                }
            }
        }
        out.next = j;
        Ok(out)
    }

    /// Skips the callback windows (`scb` … `scx`, nested) starting at `j`.
    fn skip_windows(&self, mut j: usize) -> Result<usize, HarnessError> {
        while self.recs.get(j).is_some_and(|r| r["k"] == "scb") {
            let mut depth = 0usize;
            loop {
                let Some(r) = self.recs.get(j) else {
                    return Err(bad(self.name, j, "recording ended inside a callback"));
                };
                match r["k"].as_str() {
                    Some("scb") => depth += 1,
                    Some("scx") => depth -= 1,
                    _ => {}
                }
                j += 1;
                if depth == 0 {
                    break;
                }
            }
        }
        Ok(j)
    }

    fn cb_text(&self, c: &CallbackEvent) -> String {
        format!(
            "on {} key {:#x} {} -> {} (unit {:?})",
            self.name_of(Some(c.list)).unwrap_or_default(),
            c.key,
            c.old,
            c.new,
            self.unit_name(c.unit)
        )
    }

    fn compare_cb(&self, j: usize, c: &CallbackEvent) -> Result<(), HarnessError> {
        let f = self.f(j);
        let ours_list = self.name_of(Some(c.list)).unwrap_or_default();
        let ours_unit = self.unit_name(c.unit);
        let mut fields: Vec<(&str, String, String)> = vec![
            ("L", f.str("L")?.to_owned(), ours_list),
            (
                "key",
                format!("{:#x}", f.i64("key")?),
                format!("{:#x}", c.key),
            ),
            ("old", f.i64("old")?.to_string(), c.old.to_string()),
            ("new", f.i64("new")?.to_string(), c.new.to_string()),
        ];
        // `record_stats.py` always writes the unit argument (null for
        // none); a record without the field (hand-built) does not say.
        if f.r.get("u").is_some() {
            fields.push((
                "u",
                format!("{:?}", f.r["u"].as_str()),
                format!("{:?}", ours_unit.as_deref()),
            ));
        }
        for (field, recorded, ours) in fields {
            if recorded != ours {
                return Err(self.mismatch(
                    j,
                    format!("callback.{field}"),
                    format!(
                        "recorded {recorded}, d2-sim {ours} (d2-sim callback {})",
                        self.cb_text(c)
                    ),
                ));
            }
        }
        Ok(())
    }

    // ---- tables -----------------------------------------------------------

    fn tables(&mut self, i: usize) -> Result<(), HarnessError> {
        let r = &self.recs[i];
        let rows = r["isc"]
            .as_array()
            .ok_or_else(|| bad(self.name, i, "stab without isc"))?;
        let n = rows.len();
        let size = Itemstatcost::SIZE;
        let mut records = vec![0u8; n * size];
        for (s, row) in rows.iter().enumerate() {
            let rec = &mut records[s * size..(s + 1) * size];
            let num = |k: usize| {
                row[k]
                    .as_i64()
                    .ok_or_else(|| bad(self.name, i, format!("isc[{s}][{k}]")))
            };
            put16(rec, 0, s as u16);
            for o in [0x32, 0x48, 0x4A] {
                put16(rec, o, 0xFFFF);
            }
            rec[4..8].copy_from_slice(&(num(0)? as u32).to_le_bytes());
            rec[0x18] = num(1)? as u8;
            rec[0x2C..0x30].copy_from_slice(&(num(2)? as i32).to_le_bytes());
            rec[0x50] = num(3)? as u8;
            rec[0x54] = num(4)? as u8;
            rec[0x55] = num(5)? as u8;
            put16(rec, 0x56, num(6)? as u16);
            for k in 0..3 {
                let v = row[7][k].as_i64().unwrap_or(0xFFFF);
                put16(rec, 0x58 + 2 * k, v as u16);
            }
        }
        let mut t = BinTable {
            name: "itemstatcost".into(),
            source: self.name.to_owned(),
            count: n,
            record_size: size,
            records,
        };
        stat_ops(&mut t);
        // The derived columns must be the recorded runtime ones.
        for (s, row) in rows.iter().enumerate() {
            let rec = &t.records[s * size..(s + 1) * size];
            let deps: Vec<i64> = (0..64)
                .map(|m| i64::from(get16(rec, 0x5E + 2 * m)))
                .take_while(|&v| v != 0xFFFF)
                .collect();
            let table: Vec<Vec<i64>> = (0..16)
                .map(|m| 0xDE + 6 * m)
                .map(|e| {
                    vec![
                        i64::from(get16(rec, e)),
                        i64::from(get16(rec, e + 2)),
                        i64::from(rec[e + 4]),
                        i64::from(rec[e + 5]),
                    ]
                })
                .filter(|e| e[2] != 0)
                .collect();
            let ints = |v: &Value| -> Vec<i64> {
                v.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_i64)
                    .collect()
            };
            let rec_table: Vec<Vec<i64>> =
                row[11].as_array().into_iter().flatten().map(ints).collect();
            let checks: [(&str, String, String); 5] = [
                ("a51", row[8].to_string(), rec[0x51].to_string()),
                ("a52", row[9].to_string(), rec[0x52].to_string()),
                ("a53", row[10].to_string(), rec[0x53].to_string()),
                ("optable", format!("{rec_table:?}"), format!("{table:?}")),
                ("deps", format!("{:?}", ints(&row[12])), format!("{deps:?}")),
            ];
            for (field, recorded, ours) in checks {
                if recorded != ours {
                    return Err(self.mismatch(
                        i,
                        format!("isc[{s}].{field}"),
                        format!("recorded {recorded}, d2-data stat_ops {ours}"),
                    ));
                }
            }
        }
        let stats = StatTable::from_fixed(&t).map_err(|e| bad(self.name, i, e))?;
        let mut classes = Vec::new();
        if let Some(cs) = r["cs"].as_object() {
            for (c, v) in cs {
                let c: usize = c.parse().map_err(|_| bad(self.name, i, "cs key"))?;
                if classes.len() <= c {
                    classes.resize(c + 1, ClassStats::default());
                }
                let b = |k: usize| v[k].as_u64().unwrap_or(0) as u8;
                classes[c] = ClassStats {
                    mana_regen: b(0),
                    life_per_vitality: b(1),
                    stamina_per_vitality: b(2),
                    mana_per_magic: b(3),
                };
            }
        }
        let life: BTreeSet<usize> = r["life_states"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|s| s as usize))
            .collect();
        let nstates = r["nstates"]
            .as_u64()
            .map(|n| n as usize)
            .or_else(|| life.iter().max().map(|m| m + 1))
            .unwrap_or(0);
        let mut srecs = vec![0u8; nstates * States::SIZE];
        for &s in &life {
            if s < nstates {
                // Flag bit k of a state is byte 0x10 + k / 8 (runtime-maps.md §4).
                srecs[s * States::SIZE + 0x10 + group::LIFE / 8] |= 1 << (group::LIFE % 8);
            }
        }
        let st = BinTable {
            name: "states".into(),
            source: self.name.to_owned(),
            count: nstates,
            record_size: States::SIZE,
            records: srecs,
        };
        let states = StateTable::new(&st, &maps::states(&st)).map_err(|e| bad(self.name, i, e))?;
        let data = StatData {
            stats,
            classes,
            states,
            damage_regen: Vec::new(),
            aurastate: Vec::new(),
            rescale_precision: DEFAULT_RESCALE_PRECISION,
        };
        self.lists = Some(StatLists::new(Arc::new(data)));
        Ok(())
    }

    // ---- allocation and seeding ------------------------------------------

    fn alloc_extended(&mut self, i: usize) -> Result<(), HarnessError> {
        let f = self.f(i);
        let a = f.str("L")?.to_owned();
        let uid = f.str("U")?.to_owned();
        let fl = f.u32("fl")?;
        let og = f.u32("og")?;
        let ty = unit_type(&self.f(i), "ot")?;
        let class = f.r["ui"]["c"].as_u64().unwrap_or(0) as u32;
        let cb = f.bool("cb")?;
        let act = f.r["ui"]["act"].as_str().map(str::to_owned);
        let u = self.unit(&uid);
        match act {
            Some(act) => self.host.act.insert(u, act),
            None => self.host.act.remove(&u),
        };
        if let Some(&old) = self.addr.get(&a) {
            self.unmap_list(old);
        }
        let (lists, host) = self.split(i)?;
        let callback = cb.then_some(ValueCallback::Server);
        let id = lists.alloc_extended(host, u, ty, og, class, fl, callback);
        set_flags_exact(lists, id, fl);
        lists.set_remove_callback(id, Some(MARK));
        self.map_list(&a, id);
        Ok(())
    }

    /// Seeds dumped lists (an `ssd`, or snapshot lists never seen) into
    /// d2-sim and checks what d2-sim derives from them.
    fn seed(&mut self, i: usize, dumps: &[Value]) -> Result<(), HarnessError> {
        if self.lists.is_none() {
            return Err(bad(self.name, i, "seed before the tables (stab)"));
        }
        self.host.seeding = true;
        let r = self.seed_inner(i, dumps);
        self.host.seeding = false;
        r
    }

    fn seed_inner(&mut self, i: usize, dumps: &[Value]) -> Result<(), HarnessError> {
        let by_addr: BTreeMap<String, &Value> = dumps
            .iter()
            .filter_map(|d| Some((d["L"].as_str()?.to_owned(), d)))
            .collect();
        // 1. Create the lists not yet known, with their base values.
        let mut new = Vec::new();
        for d in dumps {
            let f = Fields {
                name: self.name,
                i,
                r: d,
            };
            let a = f.str("L")?.to_owned();
            if let Some(&id) = self.addr.get(&a) {
                if self.lists.as_ref().is_some_and(|l| l.is_live(id)) {
                    continue;
                }
                self.unmap_list(id);
            }
            let fl = f.u32("fl")?;
            let ext = f.bool("ext")?;
            let og = f.u32("og")?;
            let lists_ty = unit_type(&f, "ot")?;
            let id = if ext {
                let ow = f
                    .str("ow")
                    .map_err(|_| bad(self.name, i, format!("extended list {a} without owner")))?;
                let u = self.unit(ow);
                let class = self.recs[i]["units"][ow]["c"].as_u64().unwrap_or(0) as u32;
                let cb = f
                    .bool("cb")
                    .unwrap_or(false)
                    .then_some(ValueCallback::Server);
                let lists = self.lists.as_mut().expect("checked");
                lists.alloc_extended(&mut self.host, u, lists_ty, og, class, fl, cb)
            } else {
                let ex = f.i32("ex")?;
                let ot = f.u32("ot")?;
                self.lists.as_mut().expect("checked").alloc(fl, ex, ot, og)
            };
            let lists = self.lists.as_mut().expect("checked");
            lists.set_state(id, f.u32("st")?);
            lists.set_expire(id, f.i32("ex")?);
            set_flags_exact(lists, id, fl);
            lists.set_remove_callback(id, Some(MARK));
            for e in f.r["b"].as_array().into_iter().flatten() {
                let (s, layer, v) = entry(e)
                    .ok_or_else(|| bad(self.name, i, format!("list {a}: base entry {e}")))?;
                lists.set(&mut self.host, id, s, v, layer, None);
            }
            self.map_list(&a, id);
            new.push((a, id));
            self.stats.seeded_lists += 1;
        }
        // 2. Attach the chains of each new extended list, children first
        //    (reverse dump order), oldest member first.
        for (a, id) in new.iter().rev() {
            let d = by_addr[a];
            if !d["ext"].as_bool().unwrap_or(false) {
                continue;
            }
            let owner = self.lists.as_ref().and_then(|l| l.owner(*id));
            let Some(owner) = owner else { continue };
            for head in ["last", "setl"] {
                let mut chain = Vec::new();
                let mut cur = hexes(&d[head]);
                while let Some(c) = cur {
                    if chain.contains(&c) || chain.len() > 4096 {
                        return Err(bad(self.name, i, format!("list {a}: cyclic {head} chain")));
                    }
                    cur = by_addr.get(&c).and_then(|cd| hexes(&cd["prev"]));
                    chain.push(c);
                }
                for c in chain.iter().rev() {
                    let Some(&cid) = self.addr.get(c) else {
                        return Err(bad(
                            self.name,
                            i,
                            format!("chain member {c} of {a} not dumped"),
                        ));
                    };
                    let cfl = by_addr.get(c).and_then(|cd| cd["fl"].as_u64()).unwrap_or(0) as u32;
                    let lists = self.lists.as_mut().expect("checked");
                    lists.attach(&mut self.host, owner, cid, cfl & flag::DYNAMIC == 0);
                }
            }
        }
        // A lone list (seeded below a tree d2-sim already has): attach it
        // to its recorded parent as the newest member.
        for (a, id) in &new {
            let d = by_addr[a];
            let Some(par) = hexes(&d["par"]) else {
                continue;
            };
            if by_addr.contains_key(&par) {
                continue;
            }
            let Some(&pid) = self.addr.get(&par) else {
                continue;
            };
            let lists = self.lists.as_mut().expect("checked");
            if let Some(owner) = lists.owner(pid) {
                let fl = d["fl"].as_u64().unwrap_or(0) as u32;
                lists.attach(&mut self.host, owner, *id, fl & flag::DYNAMIC == 0);
            }
        }
        // 3. Exact flags and base values (attach and the muted callbacks'
        //    bodies may have changed them), state bits, mod arrays.
        for (a, id) in &new {
            let d = by_addr[a];
            let fl = d["fl"].as_u64().unwrap_or(0) as u32;
            let want: Vec<(i32, i32)> = {
                let mut v: Vec<(i32, i32)> = d["b"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(entry)
                    .map(|(s, l, v)| (key(s, l), v))
                    .collect();
                v.sort_unstable();
                v
            };
            let lists = self.lists.as_mut().expect("checked");
            set_flags_exact(lists, *id, fl);
            for (k, v) in lists.base_entries(*id) {
                if !want.iter().any(|&(wk, _)| wk == k) {
                    lists.set(&mut self.host, *id, key_stat(k), 0, k as u16, None);
                } else if let Some(&(_, wv)) = want.iter().find(|&&(wk, _)| wk == k) {
                    if wv != v {
                        lists.set(&mut self.host, *id, key_stat(k), wv, k as u16, None);
                    }
                }
            }
            for &(k, v) in &want {
                if !lists.base_entries(*id).iter().any(|&(bk, _)| bk == k) {
                    lists.set(&mut self.host, *id, key_stat(k), v, k as u16, None);
                }
            }
            if !d["ext"].as_bool().unwrap_or(false) {
                continue;
            }
            let Some(owner) = lists.owner(*id) else {
                continue;
            };
            if let Some(sb) = d["sb"].as_array() {
                for s in sb.iter().filter_map(Value::as_u64) {
                    lists.toggle_state(owner, s as u32, true);
                }
            }
            lists.clear_mods(owner);
            for m in d["m"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_i64)
            {
                let k = m as u32 as i32;
                let v = lists.base(*id, key_stat(k), k as u16);
                lists.unit_set(&mut self.host, owner, key_stat(k), v, k as u16);
            }
        }
        // 4. What d2-sim derived must be the dump.
        for (a, _) in &new {
            self.compare_list(i, by_addr[a], "seed")?;
        }
        Ok(())
    }

    // ---- snapshots -----------------------------------------------------------

    fn snapshot(&mut self, i: usize) -> Result<(), HarnessError> {
        self.stats.snapshots += 1;
        let r = &self.recs[i];
        self.adopt_units(&r["units"]);
        let dumps = r["lists"].as_array().cloned().unwrap_or_default();
        let unseen: Vec<Value> = dumps
            .iter()
            .filter(|d| {
                d["L"].as_str().is_none_or(|a| {
                    !self
                        .addr
                        .get(a)
                        .is_some_and(|&id| self.lists.as_ref().is_some_and(|l| l.is_live(id)))
                })
            })
            .cloned()
            .collect();
        if !unseen.is_empty() {
            self.seed(i, &unseen)?;
        }
        for d in &dumps {
            self.compare_list(i, d, "snapshot")?;
            self.stats.lists_compared += 1;
        }
        Ok(())
    }

    /// Compares one dumped list with d2-sim.
    fn compare_list(&self, i: usize, d: &Value, what: &str) -> Result<(), HarnessError> {
        let f = Fields {
            name: self.name,
            i,
            r: d,
        };
        let a = f.str("L")?;
        let id = self.list(i, a)?;
        let lists = self.lists.as_ref().expect("checked by list()");
        let mismatch = |field: String, recorded: String, ours: String| {
            self.mismatch(
                i,
                format!("{what} {a} {field}"),
                format!("recorded {recorded}, d2-sim {ours}"),
            )
        };
        let entries = |key: &str| -> Vec<(i32, i32)> {
            let mut v: Vec<(i32, i32)> = d[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(entry)
                .map(|(s, l, v)| (key_of(s, l), v))
                .collect();
            v.sort_unstable();
            v
        };
        let diff = |name: &str, rec: &[(i32, i32)], ours: &[(i32, i32)]| {
            let n = rec.len().max(ours.len());
            (0..n).find(|&k| rec.get(k) != ours.get(k)).map(|k| {
                let show = |e: Option<&(i32, i32)>| {
                    e.map_or("(none)".into(), |&(k, v)| {
                        format!("{}/{}={v}", key_stat(k), k as u16)
                    })
                };
                mismatch(format!("{name}[{k}]"), show(rec.get(k)), show(ours.get(k)))
            })
        };
        if let Some(e) = diff("base", &entries("b"), &lists.base_entries(id)) {
            return Err(e);
        }
        let fl = f.u32("fl")?;
        let ours_fl = lists.flags(id);
        if (fl ^ ours_fl) & MODEL_BITS != 0 {
            return Err(mismatch(
                "flags".into(),
                format!("{fl:#x}"),
                format!("{ours_fl:#x} (bits {MODEL_BITS:#x})"),
            ));
        }
        let ext = f.bool("ext")?;
        if ext {
            if let Some(e) = diff("full", &entries("F"), &lists.full_entries(id)) {
                return Err(e);
            }
            let rec_mods: Vec<i32> = {
                let mut v: Vec<i32> = d["m"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_i64)
                    .map(|m| m as u32 as i32)
                    .collect();
                v.sort_unstable();
                v
            };
            let ours_mods = lists.mods(id);
            if rec_mods != ours_mods {
                let show = |m: &[i32]| -> Vec<String> {
                    m.iter()
                        .map(|&k| format!("{}/{}", key_stat(k), k as u16))
                        .collect()
                };
                return Err(mismatch(
                    "mod".into(),
                    format!("{:?}", show(&rec_mods)),
                    format!("{:?}", show(&ours_mods)),
                ));
            }
            if let (Some(sb), Some(owner)) = (d["sb"].as_array(), lists.owner(id)) {
                let rec_sb: Vec<u64> = sb.iter().filter_map(Value::as_u64).collect();
                let ours_sb: Vec<u64> = lists
                    .state_bits(owner)
                    .map(|(w, _)| {
                        w.iter()
                            .enumerate()
                            .flat_map(|(n, &x)| {
                                (0..32)
                                    .filter(move |b| x >> b & 1 != 0)
                                    .map(move |b| 32 * n as u64 + b)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if rec_sb != ours_sb {
                    return Err(mismatch(
                        "state bits".into(),
                        format!("{rec_sb:?}"),
                        format!("{ours_sb:?}"),
                    ));
                }
            }
        }
        let (last, setl) = lists.heads(id);
        let links = [
            ("par", lists.parent(id), true),
            ("prev", lists.prev(id), true),
            ("next", lists.next(id), true),
            ("last", last, ext),
            ("setl", setl, ext),
        ];
        for (field, ours, applies) in links {
            if !applies || d.get(field).is_none() {
                continue;
            }
            let rec = hexes(&d[field]);
            let ours = self.name_of(ours);
            if rec != ours {
                return Err(mismatch(
                    field.into(),
                    format!("{rec:?}"),
                    format!("{ours:?}"),
                ));
            }
        }
        Ok(())
    }

    // ---- expiry --------------------------------------------------------------

    fn expiry(&mut self, i: usize) -> Result<usize, HarnessError> {
        let f = self.f(i);
        let u = self.known_unit(i, f.str("U")?)?;
        let frame = f.i32("f")?;
        // The chain's flags and expire frames as the expiry saw them
        // (their setters are not hooked).
        for e in f.r["lists"].as_array().into_iter().flatten() {
            let (Some(a), Some(fl), Some(ex)) = (e[0].as_str(), e[1].as_u64(), e[2].as_i64())
            else {
                return Err(bad(self.name, i, "sxp list entry"));
            };
            if let Some(&id) = self.addr.get(a) {
                let lists = self.lists(i)?;
                if lists.is_live(id) {
                    let ours = lists.flags(id);
                    lists.set_expire(id, ex as i32);
                    set_flags_exact(lists, id, (fl as u32 & !MODEL_BITS) | (ours & MODEL_BITS));
                }
            }
        }
        self.stats.expiries += 1;
        self.host.callbacks.clear();
        self.host.removed.clear();
        let before: Vec<ListId> = self.names.keys().copied().collect();
        let (lists, host) = self.split(i)?;
        if let Err(e) = lists.expire_lists(host, u, frame) {
            // The recording finished the walk; 1.14d would still loop.
            return Err(self.mismatch(i, "expiry", e.to_string()));
        }
        let cbs = std::mem::take(&mut self.host.callbacks);
        let removed = std::mem::take(&mut self.host.removed);
        let walk = self.walk(i + 1, &cbs, true)?;
        self.stats.callbacks += cbs.len();
        // The lists the expiry freed, in order, against the recorded
        // `sxf`; then every recorded detach and free is d2-sim's.
        let lists = self.lists.as_ref().expect("checked");
        let freed: Vec<ListId> = removed
            .iter()
            .copied()
            .filter(|l| !lists.is_live(*l))
            .collect();
        let mut n = 0;
        for &j in &walk.own {
            let r = &self.recs[j];
            let a = r["L"].as_str().unwrap_or("");
            match r["k"].as_str() {
                Some("sxf") => {
                    let ours = freed.get(n).copied();
                    if self.addr.get(a).copied() != ours {
                        return Err(self.mismatch(
                            j,
                            "expiry",
                            format!(
                                "recorded free of {a}, d2-sim frees {:?}",
                                self.name_of(ours)
                            ),
                        ));
                    }
                    n += 1;
                    self.stats.expiry_frees += 1;
                }
                Some("sdt") | Some("sfr") if self.addr.get(a).is_none_or(|l| lists.is_live(*l)) => {
                    return Err(self.mismatch(
                        j,
                        "expiry",
                        format!("recorded {} of {a}; d2-sim's expiry keeps it", r["k"]),
                    ));
                }
                _ => {}
            }
        }
        if let Some(&l) = freed.get(n) {
            return Err(self.mismatch(
                walk.next - 1,
                "expiry",
                format!(
                    "d2-sim also frees {:?}; not recorded",
                    self.name_of(Some(l))
                ),
            ));
        }
        let dead: Vec<ListId> = before.into_iter().filter(|&l| !lists.is_live(l)).collect();
        for l in dead {
            self.unmap_list(l);
        }
        for h in walk.deferred {
            self.stats.host_operations += 1;
            self.operation(h)?;
        }
        Ok(walk.next)
    }
}

/// Sets a list's flags to exactly `fl` (the extended bit stays d2-sim's).
fn set_flags_exact(lists: &mut StatLists, l: ListId, fl: u32) {
    let ours = lists.flags(l);
    let keep = flag::EXTENDED;
    lists.set_flags(l, ours & !fl & !keep, false);
    lists.set_flags(l, fl & !ours & !keep, true);
}

/// A dump entry `[stat, layer, value]`.
fn entry(e: &Value) -> Option<(u16, u16, i32)> {
    Some((
        u16::try_from(e[0].as_u64()?).ok()?,
        u16::try_from(e[1].as_u64()?).ok()?,
        i32::try_from(e[2].as_i64()?).ok()?,
    ))
}

fn key_of(s: u16, layer: u16) -> i32 {
    key(s, layer)
}

fn stat_of(f: &Fields<'_>) -> Result<u16, HarnessError> {
    let s = f.u32("s")?;
    u16::try_from(s).map_err(|_| bad(f.name, f.i, format!("stat {s}")))
}

fn layer_of(f: &Fields<'_>) -> Result<u16, HarnessError> {
    Ok(f.u32("l")? as u16)
}

fn unit_type(f: &Fields<'_>, key: &str) -> Result<UnitType, HarnessError> {
    let t = f.u32(key)? as usize;
    UnitType::ALL
        .get(t)
        .copied()
        .ok_or_else(|| bad(f.name, f.i, format!("unit type {t}")))
}

fn put16(r: &mut [u8], o: usize, v: u16) {
    r[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn get16(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}
