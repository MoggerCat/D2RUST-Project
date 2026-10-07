// Spec: specs/render/lighting.md §9.1, §9.2, §9.3; specs/sim/tick.md §3; specs/sim/intents-events.md §8.2 rule 4
//! The server's act environment record (act +0x04, 0x38 bytes,
//! `0x0061BE40`, `render/lighting.md` §9.1): the day/night period the
//! server reports to a client with S→C 0x53 (§9.2 rule 2; at a join,
//! `intents-events.md` §8.2 rule 4), and its per-tick advance
//! `0x0061C040(act, a)` (§9.3 rule 5, `sim/tick.md` §3 step 1).
//!
//! Kept: the three fields 0x53 carries (period index, ticks, eclipse
//! flag) and what the server advance reads (period type +0x04, speed
//! +0x28, last reported hour +0x34). The server never runs the
//! intensity or color steps (§9.3 rule 5), so `I` and R, G, B are the
//! client's (§9.2 rule 1).

/// One period table entry (`render/env-periods.tsv`): start degree, type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    pub start: u32,
    pub kind: u32,
}

const fn p(start: u32, kind: u32) -> Period {
    Period { start, kind }
}

/// The normal table `0x007443F0` (`render/env-periods.tsv` `normal`).
pub const NORMAL: [Period; 6] = [
    p(320, 3),
    p(340, 3),
    p(0, 0),
    p(160, 1),
    p(180, 1),
    p(200, 2),
];
/// The act 4 table `0x00744438` (`act4`).
pub const ACT4: [Period; 6] = [
    p(340, 3),
    p(350, 3),
    p(0, 0),
    p(180, 1),
    p(190, 1),
    p(200, 2),
];
/// The eclipse table `0x00744480` (`eclipse`).
pub const ECLIPSE: [Period; 6] = [
    p(300, 3),
    p(0, 0),
    p(60, 1),
    p(120, 2),
    p(180, 2),
    p(240, 2),
];

/// Speed (ticks per degree) `[0x007443E4]` (normal) and `[0x007443E8]`
/// (Tainted Sun) (§9.1 +0x28).
pub const SPEED_NORMAL: u32 = 128;
pub const SPEED_ECLIPSE: u32 = 4;

/// The fields of one act's environment record the server keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Environment {
    /// +0x00: period index 0–5.
    pub period: u32,
    /// +0x04: period type (from the table).
    pub kind: u32,
    /// +0x08: ticks.
    pub ticks: u32,
    /// +0x28: speed (ticks per degree).
    pub speed: u32,
    /// +0x30: eclipse flag.
    pub eclipse: bool,
    /// +0x34: last reported hour (`0x0061C040`).
    pub last_hour: i32,
}

impl Environment {
    /// The record at its creation (§9.1 "Creation"): index 2, type and
    /// ticks (start × speed = 0) from the normal table's entry 2, speed
    /// 128, eclipse 0; +0x34 zero (the record is zero-allocated).
    pub const CREATED: Self = Self {
        period: 2,
        kind: NORMAL[2].kind,
        ticks: 0,
        speed: SPEED_NORMAL,
        eclipse: false,
        last_hour: 0,
    };

    /// S→C 0x53 (10 bytes, §9.2 rule 2): u32@1 period index, u32@5 ticks,
    /// u8@9 eclipse.
    pub fn message(&self) -> [u8; 10] {
        let mut b = [0u8; 10];
        b[0] = 0x53;
        b[1..5].copy_from_slice(&self.period.to_le_bytes());
        b[5..9].copy_from_slice(&self.ticks.to_le_bytes());
        b[9] = u8::from(self.eclipse);
        b
    }

    /// The advance `0x0061BEE0` (§9.3 rules 1–3) with act index `a`.
    pub fn advance(&mut self, a: u8) {
        // Rule 1.
        let mut t = self.ticks.wrapping_add(1);
        if !self.eclipse {
            if a == 3 {
                t = t.wrapping_add(15);
            } else if NORMAL[self.period as usize % 6].kind == 2 {
                t = t.wrapping_add(1);
                if a == 2 {
                    t = t.wrapping_add(8);
                }
            }
        }
        // Rule 2.
        if u64::from(t) >= u64::from(self.speed) * 360 {
            t = 0;
        }
        self.ticks = t;
        // Rule 3.
        let next = (self.period as usize + 1) % 6;
        let table = if !self.eclipse {
            &NORMAL
        } else if a == 3 {
            &ACT4
        } else {
            &ECLIPSE
        };
        if u64::from(table[next].start) * u64::from(self.speed) < u64::from(self.ticks) {
            let entry = if self.eclipse {
                ECLIPSE[next]
            } else {
                NORMAL[next]
            };
            self.period = next as u32;
            self.kind = entry.kind;
            self.ticks = entry.start.wrapping_mul(self.speed);
        }
    }

    /// The server advance `0x0061C040(act, a)` (§9.3 rule 5, `sim/tick.md`
    /// §3 step 1): the advance with `A` = `a` (no intensity, no color),
    /// then the report: with speed ≠ 0 and |ticks / speed − +0x34| > 16
    /// (signed integer division), +0x34 := ticks / speed and true; else
    /// true when the index or the type changed.
    pub fn server_advance(&mut self, a: u8) -> bool {
        let (index, kind) = (self.period, self.kind);
        self.advance(a);
        if self.speed != 0 {
            let hour = (self.ticks as i32).wrapping_div(self.speed as i32);
            if (i64::from(hour) - i64::from(self.last_hour)).abs() > 16 {
                self.last_hour = hour;
                return true;
            }
        }
        self.period != index || self.kind != kind
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::CREATED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/render/lighting.md §9.1, §9.2 r2; specs/sim/intents-events.md §8.2 r4
    #[test]
    fn created_record_sends_the_recorded_join_message() {
        // `53 02000000 00000000 00` (`20261006-022633` seq 228, §9.2 r4.2;
        // `intents-events.md` §8.2 rule 4).
        assert_eq!(
            Environment::CREATED.message(),
            [0x53, 2, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        // Seq 146616: index 2, ticks 0x880.
        let e = Environment {
            ticks: 0x880,
            ..Environment::default()
        };
        assert_eq!(e.message(), [0x53, 2, 0, 0, 0, 0x80, 0x08, 0, 0, 0]);
        let e = Environment {
            period: 5,
            ticks: 0,
            eclipse: true,
            ..Environment::default()
        };
        assert_eq!(e.message()[9], 1);
        assert_eq!(e.message()[1], 5);
    }

    /// §9.3 rule 5 recorded vector: act 1 (a = 0, speed 128, +1 per frame
    /// at index 2) first reports with ticks 0x880 (2176 / 128 = 17 > 16);
    /// the 0x53 is `53 02000000 80080000 00` (`-022633` seq 146611).
    // Covers: specs/render/lighting.md §9.3 r1, §9.3 r5; specs/sim/tick.md §3
    #[test]
    fn server_advance_reports_every_17_degrees() {
        let mut e = Environment::CREATED;
        for _ in 0..2175 {
            assert!(!e.server_advance(0));
        }
        assert!(e.server_advance(0));
        assert_eq!(e.ticks, 0x880);
        assert_eq!(e.last_hour, 17);
        assert_eq!(e.message(), [0x53, 2, 0, 0, 0, 0x80, 0x08, 0, 0, 0]);
        // The next report 17 degrees later.
        for _ in 0..17 * 128 - 1 {
            assert!(!e.server_advance(0));
        }
        assert!(e.server_advance(0));
        assert_eq!(e.last_hour, 34);
    }

    /// §9.3 rule 1: +16 in act 4 (a = 3); at a type-2 index +2, and +10
    /// in act 3 (a = 2); with the eclipse flag +1 everywhere.
    // Covers: specs/render/lighting.md §9.3 r1
    #[test]
    fn advance_steps_by_act_and_type() {
        // Ticks inside the period (no period change).
        let at = |period: u32, eclipse: bool, a: u8| {
            let base = if period == 5 { 30_000 } else { 23_100 };
            let mut e = Environment {
                period,
                ticks: base,
                eclipse,
                ..Environment::CREATED
            };
            e.advance(a);
            e.ticks - base
        };
        assert_eq!(at(5, false, 3), 16);
        assert_eq!(at(5, false, 0), 2);
        assert_eq!(at(5, false, 2), 10);
        assert_eq!(at(4, false, 2), 1);
        assert_eq!(at(5, true, 3), 1);
        assert_eq!(at(5, true, 2), 1);
    }

    /// §9.3 rules 2–3: the wrap at speed × 360, and the period change
    /// when the next entry's start × speed < ticks (normal 2 → 3 at
    /// 160 × 128, ticks := 20480, type 1), which reports.
    // Covers: specs/render/lighting.md §9.3 r2, §9.3 r3, §9.3 r5
    #[test]
    fn advance_wraps_and_changes_period() {
        let mut e = Environment {
            ticks: 128 * 360 - 1,
            ..Environment::CREATED
        };
        e.advance(0);
        assert_eq!(e.ticks, 0);
        let mut e = Environment {
            ticks: 20_479,
            last_hour: 159,
            ..Environment::CREATED
        };
        assert!(!e.server_advance(0)); // 20480: not past the start
        assert_eq!((e.period, e.ticks), (2, 20_480));
        assert!(e.server_advance(0));
        assert_eq!((e.period, e.kind, e.ticks), (3, 1, 20_480));
        assert_eq!(e.last_hour, 159);
        // Period 1 (start 340) lasts one update: its successor starts at 0.
        let mut e = Environment {
            period: 1,
            kind: 3,
            ticks: 340 * 128,
            ..Environment::CREATED
        };
        e.advance(0);
        assert_eq!((e.period, e.kind, e.ticks), (2, 0, 0));
    }

    /// The eclipse cycle (`quests-act2-2.md` §5.2): speed 4; the
    /// eclipse table in acts other than 4, the act-4 table in act 4 for
    /// the "next" test, the type and ticks from the eclipse table.
    // Covers: specs/render/lighting.md §9.3 r3; specs/world/quests-act2-2.md §5.2
    #[test]
    fn eclipse_advance_uses_the_eclipse_tables() {
        let sun = Environment {
            period: 0,
            kind: 3,
            ticks: 1_200,
            speed: SPEED_ECLIPSE,
            eclipse: true,
            last_hour: 0,
        };
        // Act II: eclipse entry 1 starts at 0 < 1201 → index 1, type 0,
        // ticks 0.
        let mut e = sun;
        assert!(e.server_advance(1));
        assert_eq!((e.period, e.kind, e.ticks), (1, 0, 0));
        // Act IV: act-4 entry 1 starts at 350 × 4 = 1400 ≥ 1201: stays.
        let mut e = sun;
        e.advance(3);
        assert_eq!((e.period, e.kind, e.ticks), (0, 3, 1_201));
        // The wrap at 4 × 360.
        let mut e = Environment {
            period: 5,
            kind: 2,
            ticks: 1_439,
            ..sun
        };
        e.advance(1);
        assert_eq!((e.period, e.ticks), (5, 0));
    }
}
