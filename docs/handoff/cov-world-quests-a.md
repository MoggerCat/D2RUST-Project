# Coverage batch 7: world quests / hirelings / vendors / waypoints / npc

Base bf602bf. Unit-claim "any" counts via `tools/coverage.py` (uncovered before -> after, quests-status and vendors-2 §10 excluded from the work).

| Spec | uncovered before | after |
|---|---|---|
| hirelings-2 | 6 | 5 |
| hirelings-ai | 2 | 1 |
| hirelings | 23 | 13 |
| npc | 3 | 2 |
| quests-act1-rest | 11 | 7 |
| quests-act1 | 2 | 0 |
| quests-act2-2 | 13 | 7 |
| quests-act2 | 9 | 4 |
| quests-act3-2 | 2 | 1 |
| quests-act3 | 2 | 1 |
| quests-act4 | 5 | 3 |
| quests-act5-2 | 5 | 3 |
| quests-act5 | 5 | 3 |
| quests-helpers | 10 | 2 |
| quests | 24 | 18 |
| vendors-2 | 24 | 8 |
| vendors | 6 | 3 |
| waypoints | 14 | 9 |

(Before = list at the task start; some rules were also covered meanwhile by the rebase onto bf602bf's exemptions, which remove non-behavior rules from the count.)

## Code fixes found by tests
- waypoints `travel`: level 0 or the object's own level now only closes (spec §7 r2, edge r6); the old test asserted a warp. Test corrected.
- Item decoder (`items::bitstream::read`): compact records get ilvl 1, quality 2, seed 0, suffix slot 0 (tsc 0 / isc 1); full-record ilvl < 1 reads as 1; unique index >= uniques count is -1 (vendors-2 §7.3.1 r4, r5).
- `item_from_record` (copy / load): rebuilds weapon base speed and damage (quality 1 3/4 with floors, ethereal 3/2), armor block and speed, stat 17/18 raise-to-column, stat 57 sets poison_count (vendors-2 §7.3.1 r1-r3). Previously copies had no base damage.
- New `QuestControl::not_intro_test` (`0x005444B0`, quests §2.3 r4); exemption of §2.3 r4 removed.
- vendors repair handler result: upstream bf602bf already resolved it (routine result), my change dropped in the rebase.

## Left, with reason
- Narration / provenance / pointers / dead code (no behavior): hirelings §6 r8, §10 r9, §11 r8, §14, hirelings-2 §12 style items, npc §10 (dead code), act files' edge-case "no caller" notes (act3 r17, act4 r18), waypoints §9, §10, edge text, §8 r1.
- Conflict with existing claimed rule: waypoints edge r7 (says busy operate does not set the bit; §5.2 r4 and its test say it does).
- npc edge r13 (personalize with failed duplicate): spec itself flags the d2rs policy as "to reconcile"; the existing test asserts the one-0x58 policy.
- Need wiring not present (bucket C): quests §9.1/§9.2 reward item creation / delete (seams unwired), §9.6 object init 46 / 59 and operate 43 (no spawn/fit-A/placement seams), act1-rest §9 r12 refresh-room (needs DRLG fixture), act1-rest §8 r1 (+0x38 not kept), hirelings §6 r3 / §10 r8 / edge r8 (no callers: act change, save restore, hire-list slot marking), hirelings §8 r5/r6, edge r10/r13, vendors §1 r6 (no-record paths unreachable by types), vendors-2 §7.3.1 r6 text, act2-2 §1 r11/r14/r15/r16, §2 text, act2 §5.9 r1/r3/l2 r1 (speed fields not modelled), §6.10, act5 §1.4 table, act5 r7/r11, act5-2 r10/r12/r14, act4 r19/r21, act1-rest §4.1/§9 r3/r6, helpers §8 r1/edge r2.
- vendors-2 §10: other session.

## Notes
- Claims on exempt rules removed (§8 text, §5.3 text, §3 r3); `tools/coverage.py --check` passes.
- Pre-existing failures at this base, unrelated: `world::objects::tests::{routes_match_function_table,route_check_catches_perturbations}`.
