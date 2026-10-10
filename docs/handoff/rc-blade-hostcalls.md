# rc-blade-hostcalls

Branch `claude/rc-blade-hostcalls` (REC-2020..2022). Task: the Act III Blade real-data test
(`app_play_act3::the_blade_of_the_old_religion_from_hratli_to_ormus_and_asheara`) ended on host calls
with no provider.

## Result (`D2_GAME_DIR=$HOME/game`, `--ignored`)

| | Before | After |
|---|---|---|
| `app_play_act3` (7 tests) | blade fails: `sound 0 65`, `stat sent 0 6 ..`, `sound 422 10`, `state stat 505 105 172 2`, `join team 505 0`, `hireling ai 505` | 7 pass (282 s) |
| `d2-sim` nextest | - | 4749 passed |
| `d2-server` nextest | - | 398 passed |

No scenario-diff check was run (no check covers these calls): the ledger rows are NO-CHECK, size S.

## Base

`specs-staging-7` lacked `q-fix-rd-act3` (the test failed earlier at Hratli 466 ≠ 571), so that branch
is merged in first; two conflicts (player inventory at join) took staging-7's REC-1402 form.

## What changed (all read from 1.14d asm, own words)

- **Sound** (`0x00553380`): quest sounds queue the game's sound slot on the player with target = player
  (asm `0x005B95A9`); the NPC heal sound 10 targets none (asm `0x00578E4F`). Both are flushed as S→C
  0x2C by the existing unit update. `QuestRest::attach_sound` removed (`wiring/economy/quest_items.rs`,
  `wiring/interaction/npc_world.rs`).
- **Stat sent** (`0x00548520`): `set_stat_send` emits the 0x1D/0x1E/0x1F message through `send`
  (`NpcRest::stat_sent` removed).
- **Mercenary init** (`0x00573270`): `join_team` (`0x005B1900`: +0xD0 must be 11, group < 8, node after the
  head), alignment state list (shared `View::set_alignment`, allied mark), umod 19 and the monster data
  bytes +4,+5,+9,+10,+11 := 0 (components 0,1,5,6,7; new `UnitHooks::clear_hireling_components`,
  `set_alignment`). `HirelingRest::{set_state_stat, join_team, hireling_ai}` removed.
- Test fakes updated; `SoundEvents::queued` added; specs `hirelings.md` §3.2, `npc.md` §5, `quests-act3.md` §1.

## Open

- The Blade's `stat sent 0 6 ...` comes from the test's staged life poke, not from game code.
- Player head of the team list (`0x005B1880`) is the host's: `join_team` adds the node without testing it
  (S).
- `d2-client` non-ignored tests were not run (disk limit; clippy clean, ignored act3 tests pass).
- Other-owner files touched: none beyond fakes.
