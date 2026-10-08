# Handoff: q-difficulty (`claude/q-difficulty`)

## Links connected
`play --difficulty normal|nightmare|hell|0-2` (default Normal) now sets the
game's difficulty everywhere `GAME_SETUP.difficulty` was used as a constant:
- C→S 0x67 difficulty (`create_request_for`), S→C 0x01 byte 1, `GameInfo` and
  `GameFields` (so the sim's `ai_info`, `pop_info`, `init_info` and the data copy
  carry it). The sim's monster level, stat scaling, resist penalty and treasure
  class column (`monsters/init/calc.rs`, `treasure/drop.rs::tc_column`) were
  already keyed on that value; they now run on Nightmare / Hell.
- `Character::difficulty()` / `with_difficulty()`; `NewCharacter.difficulty`;
  a save's difficulty is its `LoadContext` (`load_character(.., difficulty)`,
  `LiveData::read_save(.., difficulty)`, whose header check refuses a save with
  no town on that difficulty).
- New character: save town slot (`save::fresh`) and the Cold Plains waypoint go
  into that difficulty's record (`waypoints.get_mut(difficulty)`).
- Quest flags and waypoints were already per difficulty in the save
  (`Live.quests` 3 records); unchanged.

Tests: `tests/app_new_character.rs` (Hell join: 0x67, 0x01, `ai_info`, save town;
name parsing). Gate: fmt, clippy, 1766 d2-client tests, coverage check all pass.

## PROVISIONAL / left
- A new character is started in Act 1 on the chosen difficulty without
  requiring the previous difficulty to be beaten (preview only).
- Not wired: monster spawns are what the preview currently has, so scaling is
  only visible once monsters exist in the preview map. d2-server / d2-sim
  tests were not run (untouched).
- No REC id added.

## Local check
`d2-client play --new barbarian Conan --difficulty hell` prints
`play: difficulty 2`. Once monsters appear, their life/damage should be higher
than on the default and resist values on the character panel (C) show the
Hell penalty (`difficultylevels.ResistPenalty`, already bound).
