# rc-bloodlord-damage: hand-back (2026-10-10, branch `claude/rc-bloodlord-damage`)

Base: `claude/specs-staging-7` + `claude/rc-mon-modes` (merged; ledger/index regenerated).

## Checks

| Set | Before | After |
|---|---|---|
| play_act5 `baal_falls_and_the_game_is_finished` | fails at bloodlord5 kill (life stays 1<<8) | passes |
| `playthrough.py traces/playthrough/act5.play` | reached 12/13 (consecutive 6/13) | reached 12/13 (6/13) |
| d2-sim + d2-server nextest | n/a | 5148 pass |

No 1.14d-side check changed; no EQUAL counts moved (no ledger part).

## Causes (two, both not damage)

1. **bloodlord5 took no damage: the player's swing was out of reach.** Preview `LocalSeams::in_melee_range`
   (`d2-client/src/app/single_player.rs`, d2rs-own, unverified) allows max-axis distance 4 for the player's range 1;
   the test's `stand_by` accepts 5. The bloodlord stood 5 away (monster swings, range 3, reached). Test fix: `kill`
   stands within 1 (`stand_within`). Not weakened: the swing still has to hit.
2. **Next blocker, Tyrael's last portal (object 565) never placed.** NPC chat close ran `quest_chat_end` on the plain
   `EconomyQuests`, where `unit_position` has no room for the player (client rest stub) -> no spot -> no object. Fix:
   `InteractionState::{defer_chat_end, chat_ends}`; the server (`wired.rs` `npc`) queues the call and runs
   `npc_deactivate` through `quest_call` (HostQuests) right after the NPC call. Spec note in `quests-act5-2.md` §8.4.
   Ordering change: quest chat end now runs after the rest of `chat_close` (list removal), not before; unverified, S.

Also: `MonsterInfo.mode_chart` (rc-mon-modes) broke 15 test constructors in d2-server/d2-client; fixed (`mode_chart: false`).

## Open

- `in_melee_range` preview is not the real `0x00622C40` distance (needs the RangeWorld `distance`/line test). M.
- act5.play first blocker: `nihlathak-killed` (stuck f1000; player lv 124 at (12704,5018)). Not looked at. S-M.
- Other quest-chat callbacks (`npc_activate` etc.) also run on the plain economy; same gap if they need rooms. S.
