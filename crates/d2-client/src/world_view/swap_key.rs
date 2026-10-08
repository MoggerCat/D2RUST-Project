// Spec: specs/ui/key-commands.tsv (Cfgswapweapons, W), specs/sim/intents-events.md (§9 r14)
//! The weapon-swap key (W, `Cfgswapweapons`): the action no panel took
//! leaves as C→S 0x60 SwapWeapons. The server trades the hands with the
//! swap set and answers with the item updates and S→C 0x97; this module
//! only asks (CLAUDE.md rule 7). d2rs-own, unverified (REC-230): the
//! command's own handler (`0x00469140`) is not specified further.

use crate::bridge::link::ServerLink;
use crate::bridge::{Bridge, BridgeError};
use crate::controls::Action;
use crate::ui::{ActionId, UiEvent};

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
