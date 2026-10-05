//! Ownership gate, client side: checks that the folder the user points us at
//! holds a usable D2 LoD install. The required MPQs must exist, open as valid
//! archives and contain the files the engine loads. Phase 8 fills this in.
//!
//! It checks that the player *has* the game, not that their copy matches one
//! exact release: installs differ across Blizzard releases, so there is no
//! comparison against fixed hashes. It reads only the chosen game folder.
