# rc-skill-div-a hand-back (REC-3020..3039: none used)

Ledger `skill.ass.*` + `skill.bar.*` (60 rows): EQUAL 0 -> 57, DIVERGED 3. Part `docs/handoff/ledger/rc-0skill-div-a.tsv`
(named to sort before the other rc-* parts: equal-rank parts tie-break by name) and the same rows replaced in `rc-promote.tsv`
(rank 3, it held the stale DIVERGED verdicts). `ledger.py --fix` also refreshed contradictory rows of a few q-*/rc-run parts.
Rule: every check of the row has packets MATCH and state PARTIAL only from the d2rs client-gap header (REC-2055/2056).

## Checks (60 gen-skill-bar/ass + 53 plain bar-/ass-, 1.14d re-recorded under Wine, orig-cache reused)
Regression runs after each batch: gen-skill-* 210, gen-monskill 64, gen-umod 42, gen-mon 312, plain ama/ass/bar/dru/nec/pal/sor:
0 ledger-EQUAL checks regressed; gen-skill MATCH 159 -> 176. 11 plain ass/bar checks got a `packets` channel (they have input).

## Causes (first divergence -> fix), all in d2-sim
1. Join 0xA8 passive lists carry the item type as param (`passiveitype` layer, 0x00646D60): fixed on integ-r23 by rc-a8-setstate; 42 rows.
2. `BodyEffect::Umod` was never applied: summon `sumumod` (sentries 32, shadows 42) now runs 0x005A4850 with unique 1 -> 0xAC type block.
3. Monsters sent no 0x21 for monstats `SendSkills`/`Skill<i>` slots (`monster_sent_skills`, monster_add.rs).
4. 0x00639DB0 / 0x00639E30 end with the update-queue insert 0x0064C040: skill-delay list (A7 state 121), `free_aura_state`,
   `mark_state_changed` now queue the unit; passive refresh marks the changed bit (level changes resend A8).
5. Player update step 4 now sends the pending event records (0xA5 landing of Leap / Whirlwind) before the state messages.
6. 0xAC source link reads the unit's owner fields (shadows).
7. Shout/BO/BC: `MissileWorld::shout_state` (0x005D8290) + `ally_test` (0x00554DE0) implemented; `buff_refresh` = 0x0056DE40
   (passive refresh + player resync); caster is hit by its own ring (second A8 at f29).
8. Blade Sentinel: AI `skill_missile` (0x0056EDE0), `link_owner` (0x00621CE0, missile owner) and `AiCommand` (0x0058EF40) now run.
SHARED with rc-skill-div-b (merge this branch, do not re-fix): 2, 3, 4, 5, 7, 8 (summons, monsters, any player event record).

## Open (3 rows, sizes)
- skill.ass.shadow-warrior (M): two S2C 0x9D action 6 (Equip) for the monster's equipment items (0x00534F80 monster inventory
  messages; `Pending::inventory_messages` has no provider; the server sends ground items through `announce_item_as`).
- skill.ass.shadow-master (M): the same + master AI init 0x005EB490 adds the owner's class skills (`AiSummons::assign_skill` /
  `class_skills` unprovided; 0x00647110 turns each passive state on; 0xAA lists states 152/155 with their stat lists; attack at f48).
- skill.ass.blade-sentinel (S): first walk point 5148 (1.14d) vs 5150 (d2rs) for a command point (5151,4262): path around the cow;
  stop distance 0 on both sides; suspect the class-413 footprint / move mask.
- Disk: traces/raw/suite grows ~6 MB per check; the session filled the 252 GB allowance once (a glob `rm` was refused by the
  sandbox, left to the owner: `traces/raw/suite/*` is scratch).
