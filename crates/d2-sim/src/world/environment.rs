// Spec: specs/render/lighting.md §9.1, §9.2; specs/sim/intents-events.md §8.2 rule 4
//! The server's act environment record (act +0x04, 0x38 bytes,
//! `0x0061BE40`, `render/lighting.md` §9.1): the day/night period the
//! server reports to a client with S→C 0x53 (§9.2 rule 2; at a join,
//! `intents-events.md` §8.2 rule 4).
//!
//! Only the three fields 0x53 carries are kept (period index, ticks,
//! eclipse flag); the intensity and color are the client's (§9.2 rule 1).
//!
//! TODO(spec gap: sim/tick.md §3 step 1, render/lighting.md §9.3): the
//! server's per-tick advance `0x0061C040` runs "the same advance code" as
//! the client's, but its `A` (act index) and `L` (level id) arguments on
//! the server are not stated; nothing advances the record, so it keeps
//! its creation values.

/// The fields of one act's environment record that S→C 0x53 sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Environment {
    /// +0x00: period index 0–5.
    pub period: u32,
    /// +0x08: ticks.
    pub ticks: u32,
    /// +0x30: eclipse flag.
    pub eclipse: bool,
}

impl Environment {
    /// The record at its creation (§9.1 "Creation"): index 2, ticks 0
    /// (start × speed of the normal table's entry 2, start 0), eclipse 0.
    pub const CREATED: Self = Self {
        period: 2,
        ticks: 0,
        eclipse: false,
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
        };
        assert_eq!(e.message()[9], 1);
        assert_eq!(e.message()[1], 5);
    }
}
