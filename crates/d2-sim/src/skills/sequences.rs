// Spec: specs/skills/sequences.md
//! Player skill sequences: the table `0x007483B8` (§4, compiled from
//! `specs/skills/sequences.tsv`, 23 sequences × 14 COF weapon classes)
//! and its lookup `0x00663310` for a player (§1 rule 4).

use std::sync::OnceLock;

/// The table as dumped from the 1.14d image (§4).
pub const SEQUENCES_TSV: &str = include_str!("../../../../specs/skills/sequences.tsv");

/// COF weapon class codes, `weaponclass.txt` order (§4 `classes`).
pub const CLASSES: [&str; 14] = [
    "hth", "bow", "1hs", "1ht", "stf", "2hs", "2ht", "xbw", "1js", "1jt", "1ss", "1st", "ht1",
    "ht2",
];

/// Player mode codes in `plrmode` row order (a frame's byte +2).
pub const PLAYER_MODES: [&str; 20] = [
    "DT", "NU", "WL", "RN", "GH", "TN", "TW", "A1", "A2", "BL", "SC", "TH", "KK", "S1", "S2", "S3",
    "S4", "DD", "SQ", "KB",
];

/// One 6-byte frame record (§3): +2 drawn mode, +3 drawn frame, +5 event
/// byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeqFrame {
    pub mode: u8,
    pub frame: u8,
    pub event: u8,
}

/// Slot `s` (0–23) per weapon class: the frame list, `None` for a null
/// list (§1 rule 3, §4 `count` 0).
pub type SeqTable = Vec<[Option<Vec<SeqFrame>>; 14]>;

/// The compiled table (24 slots, slot 0 null).
pub fn table() -> &'static SeqTable {
    static T: OnceLock<SeqTable> = OnceLock::new();
    T.get_or_init(|| parse(SEQUENCES_TSV).expect("specs/skills/sequences.tsv"))
}

/// Parses `sequences.tsv` (§4 columns).
pub fn parse(tsv: &str) -> Result<SeqTable, String> {
    let mut t: SeqTable = (0..24).map(|_| Default::default()).collect();
    for (n, line) in tsv.lines().enumerate().skip(1) {
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 6 {
            return Err(format!("line {}: {} columns", n + 1, cols.len()));
        }
        let s: usize = cols[0]
            .parse()
            .map_err(|_| format!("line {}: seqnum", n + 1))?;
        if s == 0 || s >= t.len() {
            return Err(format!("line {}: seqnum {s}", n + 1));
        }
        let count: usize = cols[2]
            .parse()
            .map_err(|_| format!("line {}: count", n + 1))?;
        let mut frames = Vec::with_capacity(count);
        for f in cols[3].split_whitespace() {
            let (m, i) = f
                .split_once('.')
                .ok_or_else(|| format!("line {}: frame {f}", n + 1))?;
            let mode = PLAYER_MODES
                .iter()
                .position(|c| *c == m)
                .ok_or_else(|| format!("line {}: mode {m}", n + 1))?;
            let frame: u8 = i
                .parse()
                .map_err(|_| format!("line {}: frame {f}", n + 1))?;
            frames.push(SeqFrame {
                mode: mode as u8,
                frame,
                event: 0,
            });
        }
        if frames.len() != count {
            return Err(format!(
                "line {}: {} frames, count {count}",
                n + 1,
                frames.len()
            ));
        }
        for e in cols[4].split(',').filter(|e| !e.is_empty()) {
            let (i, c) = e
                .split_once(':')
                .ok_or_else(|| format!("line {}: event {e}", n + 1))?;
            let i: usize = i
                .parse()
                .map_err(|_| format!("line {}: event {e}", n + 1))?;
            let c: u8 = c
                .parse()
                .map_err(|_| format!("line {}: event {e}", n + 1))?;
            frames
                .get_mut(i)
                .ok_or_else(|| format!("line {}: event index {i}", n + 1))?
                .event = c;
        }
        for c in cols[1].split(',') {
            let k = CLASSES
                .iter()
                .position(|x| *x == c)
                .ok_or_else(|| format!("line {}: class {c}", n + 1))?;
            if t[s][k].is_some() {
                return Err(format!("line {}: class {c} twice", n + 1));
            }
            t[s][k] = (count > 0).then(|| frames.clone());
        }
    }
    Ok(t)
}

/// `0x00663310` for a player (§1 rules 3–4): the frame list of sequence
/// `seqnum` for COF weapon class `class`; `None` for `seqnum` 0, a slot
/// past the table, a class past the 14 (the original's fatal assertion
/// at line 0x533), or a null list.
pub fn lookup(seqnum: u8, class: usize) -> Option<&'static [SeqFrame]> {
    table().get(usize::from(seqnum))?.get(class)?.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/skills/sequences.md §4
    #[test]
    fn the_table_compiles_and_names_every_class_of_each_sequence_once() {
        let t = parse(SEQUENCES_TSV).unwrap();
        // Leap (13): 15 frames S1 0–14, events 5:1, 11:1, 14:1 (§4 table).
        let leap = t[13][0].as_ref().unwrap();
        assert_eq!(leap.len(), 15);
        assert!(leap.iter().all(|f| f.mode == 13));
        let ev: Vec<(usize, u8)> = leap
            .iter()
            .enumerate()
            .filter(|(_, f)| f.event != 0)
            .map(|(i, f)| (i, f.event))
            .collect();
        assert_eq!(ev, vec![(5, 1), (11, 1), (14, 1)]);
        // Whirlwind (10) has no record for bow / xbw.
        assert!(t[10][1].is_none() && t[10][7].is_none());
        assert_eq!(t[10][0].as_ref().map(Vec::len), Some(8));
        assert!(t[0].iter().all(Option::is_none));
    }

    // Covers: specs/skills/sequences.md §1
    #[test]
    fn the_lookup_refuses_slot_zero_and_classes_past_the_table() {
        assert!(lookup(0, 0).is_none());
        assert!(lookup(13, 14).is_none());
        assert!(lookup(200, 0).is_none());
        assert_eq!(lookup(13, 4).map(<[SeqFrame]>::len), Some(15));
    }

    // M08: the parser reports a wrong frame count.
    #[test]
    fn a_wrong_count_is_an_error() {
        let bad = "seqnum\tclasses\tcount\tframes\tevents\tlist_va\n13\thth\t2\tS1.0\t\t0x1";
        assert!(parse(bad).unwrap_err().contains("count"));
    }
}
