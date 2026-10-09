// Spec: specs/flows/save-exit.md §2 r2, §3 r1; specs/sim/tick.md §6 r3; specs/sim/intents-events.md §2.5 r2
//! Character storage: the server's character save (`0x00532400` →
//! `0x00532240`, the `.d2s` written with `fopen` / `fwrite`).
//!
//! The server runs the save at the two points of `flows/save-exit.md`:
//! the leave handler of C→S 0x69, before S→C 0x05
//! ([`super::session_flow::SessionFlow::leave`]), and the tick's client
//! pass every 8192 frames (`d2_sim::game::Game::character_save_due`,
//! written after the tick by [`SimGame`]'s `Tick`). Both run
//! `0x0052CA10`: every client with a player, in client-list order
//! ([`SimGame::save_characters`]).
//!
//! What a save holds and where the file goes is the host's
//! [`CharacterStore`]: the game's rest of the player (quests, hireling,
//! the app's tables) is typed by the host that built the game, so the
//! host installs the writer ([`SimGame::set_storage`]); the server
//! decides when it runs.

use crate::seams::ClientId;

use super::SimGame;

/// Writes one client's character (`0x00532400`).
pub trait CharacterStore<D, W> {
    /// Saves the character of `client`'s player from the running game.
    fn save(&mut self, sim: &mut SimGame<D, W>, client: ClientId) -> Result<(), String>;
}

/// Why a client's character was not written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SaveFault {
    /// The game has no storage ([`SimGame::set_storage`] never ran: a
    /// test host, or a character without a save file).
    #[error("no character storage")]
    NoStorage,
    /// The storage refused (`0x00532240` logs "Unable to open player
    /// save file %s").
    #[error("{0}")]
    Failed(String),
}
