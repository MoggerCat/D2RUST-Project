// Spec: specs/ui/key-commands.tsv (Cfgswapweapons, W), specs/sim/intents-events.md (§9 r14)
//! The weapon-swap key (W, `Cfgswapweapons`): the action no panel took
//! leaves as C→S 0x60 SwapWeapons. The server trades the hands with the
//! swap set and answers with the item updates and S→C 0x97; this module
//! only asks (CLAUDE.md rule 7). d2rs-own, unverified (REC-177): the
//! command's own handler (`0x00469140`) is not specified further.

use crate::bridge::link::ServerLink;
use crate::bridge::{Bridge, BridgeError};
use crate::controls::Action;
use crate::ui::{ActionId, UiEvent};

/// The UI states that block the swap command (`ui/controls.md` §3 row
/// 44, `0x00469140`: NPC shop, trade, stash).
pub const SWAP_BLOCKING: [u8; 3] = [0x0C, 0x17, 0x19];

/// Sends C→S 0x60 for each swap-weapons action in `unhandled`; returns
/// how many left.
pub fn send_swaps<L: ServerLink>(
    unhandled: &[UiEvent],
    bridge: &mut Bridge<L>,
) -> Result<usize, BridgeError> {
    let swap = UiEvent::Action(ActionId(Action::SwapWeapons.index() as u16));
    let mut sent = 0;
    for _ in unhandled.iter().filter(|e| **e == swap) {
        bridge.send(&d2_proto::client::SwapWeapons)?;
        sent += 1;
    }
    Ok(sent)
}

/// Whether the swap command runs (`ui/controls.md` §3 row 44): none of
/// [`SWAP_BLOCKING`] is open.
pub fn swap_allowed(is_open: &dyn Fn(u8) -> bool) -> bool {
    !SWAP_BLOCKING.iter().any(|&s| is_open(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/ui/controls.md §3 row29
    #[test]
    fn no_swap_with_the_shop_trade_or_stash_open() {
        assert!(swap_allowed(&|_| false));
        for s in [0x0C, 0x17, 0x19] {
            assert!(!swap_allowed(&|u| u == s), "ui {s:#x}");
        }
        // The inventory or the cube do not block it.
        assert!(swap_allowed(&|u| u == 1 || u == 0x1A));
    }
}
