// Spec: specs/sim/intents-events.md §3.5 rule 6, §7.3 rule 2 step 8, §7.9 rule 1
//! The state-change messages `0x005711D0(unit, client)`: for every state
//! whose state-changed bit is set on the unit (set by the state toggle,
//! cleared by the room clean-up, §7.5 step 4), S→C 0xA8 SetState when the
//! unit has it and its stat list has entries, 0xA7 DelayedState when it
//! has it without entries, 0xA9 EndState when it no longer has it.
//!
//! Where it runs: the monster update's step 8 (§7.3 rule 2). For a
//! player the spec names no step; d2rs sends them in the same per-client
//! update (PROVISIONAL, REC in `docs/HANDOFF.md` §7), so the local
//! player sees its own auras and the others' states.

use crate::units::messages::{self, SendStat};
use crate::units::{UnitId, UnitType};

use super::{Pending, View};

impl<X: Pending> View<'_, X> {
    /// Sends the changed-state messages of `unit` to `receiver`.
    pub fn state_change_messages(&mut self, receiver: UnitId, unit: UnitId) {
        let Some(r) = self.units.get(unit) else {
            return;
        };
        if !matches!(r.ty, UnitType::Player | UnitType::Monster) {
            return;
        }
        let (ty, guid) = (r.ty as u8, r.guid);
        let Some((bits, changed)) = self.stats.state_bits(unit) else {
            return;
        };
        let count = self.stats.data().states.count();
        let mut out: Vec<Vec<u8>> = Vec::new();
        for (w, cw) in changed.iter().enumerate() {
            for b in 0..32 {
                let s = w * 32 + b;
                if cw & (1 << b) == 0 || s >= count || self.state_nosend(s) {
                    continue;
                }
                let has = bits.get(w).is_some_and(|x| x & (1 << b) != 0);
                if !has {
                    out.push(messages::state_ref(0xA9, ty, guid, s as u8).to_vec());
                    continue;
                }
                let entries: Vec<(u16, u16, i32)> = self
                    .state_list(unit, s as u16)
                    .map(|l| {
                        self.stats
                            .base_entries(l)
                            .into_iter()
                            .map(|(k, v)| (k as u32 as u16, crate::stats::key_stat(k), v))
                            .collect()
                    })
                    .unwrap_or_default();
                if entries.is_empty() {
                    out.push(messages::state_ref(0xA7, ty, guid, s as u8).to_vec());
                } else {
                    let bodies = self.h.bodies.as_deref();
                    out.push(messages::set_state(ty, guid, s as u8, &entries, |id| {
                        bodies
                            .and_then(|b| b.stat(i32::from(id)))
                            .map(|r| SendStat {
                                bits: r.send_bits,
                                param_bits: r.send_param_bits,
                                signed: r.signed,
                            })
                    }));
                }
            }
        }
        for m in out {
            self.h.x.send(receiver, &m);
        }
    }

    fn state_nosend(&self, s: usize) -> bool {
        self.h
            .bodies
            .as_deref()
            .and_then(|b| b.state_nosend.get(s))
            .copied()
            .unwrap_or(false)
    }
}
