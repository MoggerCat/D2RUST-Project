// Spec: specs/items/generation.md Inputs, §1.2; specs/items/quality.md §8.1; specs/items/treasure.md Inputs
//! The game fields item creation, treasure and the cube read
//! ([`crate::items::ItemGame`], [`crate::treasure::GameFacts`]). No
//! written spec places them in [`crate::game::Game`] yet (the
//! game-creation spec is not written), so the wiring holds them.
//!
//! A host that runs the economy on the action wiring keeps the creation
//! fields in one place, the action wiring's (`ActionHooks::game_seed`,
//! `ActionHooks::ai_info`, `UnitData::expansion`), and builds a
//! [`GameFields`] from them for each call ([`GameFields::from_action`]);
//! [`GameFields::ai_info`] is the inverse for game creation.

use crate::items::{ItemGame, UniqueBits};
use crate::monsters::ai::GameInfo;
use crate::rng::Seed;
use crate::treasure::GameFacts;

/// Game-creation fields (D2MOO `D2GameStrc` offsets).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameFields {
    /// The game seed (`sim/rng.md` §5.2).
    pub seed: Seed,
    /// +0x6D: 0 normal, 1 nightmare, 2 hell.
    pub difficulty: u8,
    /// +0x70.
    pub expansion: bool,
    /// +0x6A (3 = single player; `items/quality.md` §8 reads it as a
    /// ladder flag when ≠ 0).
    pub game_type: u8,
    /// +0x74.
    pub ladder: bool,
    /// +0x1B24.
    pub uniques: UniqueBits,
}

impl GameFields {
    /// Fields with the given game seed, normal difficulty, no ladder.
    pub fn new(seed: Seed, expansion: bool) -> Self {
        Self {
            seed,
            difficulty: 0,
            expansion,
            game_type: 0,
            ladder: false,
            uniques: UniqueBits::default(),
        }
    }

    /// The fields of a game whose game seed is `seed`, whose AI fields
    /// are `ai` (game +0x6A, +0x74 as `game_type_ex`, +0x6D) and whose
    /// expansion flag (+0x70) is `expansion`, with the unique bits
    /// `uniques`. +0x74 is read as a flag (`quality.md` §8: "≠ 0").
    pub fn from_action(seed: Seed, ai: &GameInfo, expansion: bool, uniques: UniqueBits) -> Self {
        Self {
            seed,
            difficulty: ai.difficulty,
            expansion,
            game_type: ai.game_type,
            ladder: ai.game_type_ex != 0,
            uniques,
        }
    }

    /// The AI's copy of these fields (game +0x6A, +0x74, +0x6D).
    pub fn ai_info(&self) -> GameInfo {
        GameInfo {
            game_type: self.game_type,
            game_type_ex: u32::from(self.ladder),
            difficulty: self.difficulty,
        }
    }

    /// The treasure walk's game facts (`items/treasure.md` Inputs). The
    /// living-player count and the `players` setting come from the host.
    pub fn treasure_facts(&self, living_players: i32, players_setting: i32) -> GameFacts {
        GameFacts {
            expansion: self.expansion,
            difficulty: self.difficulty,
            game_type: self.game_type,
            living_players,
            players_setting,
            item_format: u32::from(ItemGame::item_format(self)),
        }
    }
}

impl ItemGame for GameFields {
    fn seed(&mut self) -> &mut Seed {
        &mut self.seed
    }
    fn difficulty(&self) -> u8 {
        self.difficulty
    }
    fn expansion(&self) -> bool {
        self.expansion
    }
    /// `items/quality.md` §8: "game +0x6A ≠ 0 or game +0x74 ≠ 0".
    fn ladder_flags(&self) -> (bool, bool) {
        (self.game_type != 0, self.ladder)
    }
    fn uniques(&mut self) -> &mut UniqueBits {
        &mut self.uniques
    }
}
