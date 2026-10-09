// Spec: specs/tools/scenario.md §4 r2 (a) (scripted C→S messages at the net send, after the duplicate filter)
//! Scripted client→server messages (`state-dump --send`,
//! `specs/tools/scenario-diff.md` §2 `at … send`): a link whose server can
//! take a message at its transport send implements [`InjectTarget`]; the
//! bridge forwards the request, so tools reach the game through the
//! bridge like everything else (CLAUDE.md rule 5).
//!
//! The message takes the path the bridge's own messages take after the
//! client's duplicate filter (`intents-events.md` §2.1 rules 3–4: the
//! classifier and the server queues), never the filter itself: a scripted
//! message is never dropped by the sender (`scenario.md` §4 r2 (a); on
//! 1.14d the call of the transport send `0x0052AE50`,
//! `tools/original-hooks.md` §1 rule 4). References are resolved on the
//! server's state after the last tick (`scenario.md` §3 rules 3–5).

use conformance::scenario::script::{StepMsg, Unresolved};

/// What an injected message became.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Injected {
    /// Handed to the transport send. `queued`: the classifier queued it
    /// (1.14d's EAX 1); false: it dropped it (EAX 0), `why` says how.
    Sent {
        bytes: Vec<u8>,
        queued: bool,
        why: Option<String>,
    },
    /// A reference did not resolve: nothing sent (`scenario.md` §3 r5).
    Unresolved(Unresolved),
}

/// A link whose server can take a scripted message between two frames.
pub trait InjectTarget {
    type Error;
    /// Resolves `msg` on the state after the last tick and hands its
    /// bytes to the transport send, after the duplicate filter, before
    /// the next frame's drain.
    fn inject(&mut self, msg: &StepMsg) -> Result<Injected, Self::Error>;
}

impl<L: InjectTarget> super::Bridge<L> {
    /// Sends `msg` to the server as a scripted message (no duplicate
    /// filter), between two frames.
    pub fn inject(&mut self, msg: &StepMsg) -> Result<Injected, L::Error> {
        self.link.inject(msg)
    }
}
