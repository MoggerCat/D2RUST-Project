# rc-c022-playerstate hand-back (2026-10-10)

Causes C022 / C029 / C032 / C043 / C054 / C056 (player-state first divergences).
Method: re-ran the clusters on integ-r23 head (suite.py with the orig-cache, no Wine
needed for cached recordings), grouped by first divergence, settled what is now green,
then read the rest.

## Settled (merged into claude/integ-r23; ledger part `rc-c022-playerstate.tsv`)
- **C022** `state player q` (8 rows): 18 checks (a1/a2/a3/a5 warps + npc talks,
  act-travel, join-act2-quests, milestone-act3/4/5, items-drops-nor-13) now 2146/2146
  ticks, state PARTIAL (client gap only), packets/rng/items MATCH, no `ignore` line.
  Rows EQUAL: cov.monster.175/195/196/201, net.s2c.0x5d/0x8a, level.a2.40.act-2-town,
  system.act.travel (waypoint.9.lut-gholein and system.flows.act-change.2 re-verdicted).
  gen-sysc-flows-act-change-1/3 and gen-sysc-client-msg-ui-20-0x61 keep one divergence,
  on the packets channel (s2c #18 0x0a vs 0xfe): that is rc-net-div's C022.
- **C032 / C043** (`player m 4->0`, `0->1`): gen-su-26/28/29/30/63, gen-boss-544 and
  gen-ai-highpriest are 150/150. Rows EQUAL: monster.superunique.{baal-subject-2,
  bremm-sparkfist, ismail-vilehand, toorc-icefist, wyand-voidfinger}, monster.ai.highpriest,
  monster.boss.baalcrab (checks column set to gen-boss-544).
- `ledger.py --fix` reconciled `last_verdict` (derived from checks-status.md) in a few
  other parts; no states or notes of other sessions changed.

## Not settled
1. **monster.deathmauler1-5** (gen-mon-529..533): 150/150 + rng MATCH, but the check files
   carry `ignore q seed` (line 14). Not EQUAL under rule 10 until the line is removed and
   1.14d re-recorded (Wine). 5 rows.
2. **C054 / C029** (gen-qkill-blood-raven f64, gen-ai-bloodraven f94; gen-su-8 and
   gen-qkill-baal are the same family): the player takes 1144 (d2rs) vs 632 (1.14d) from a
   Blood Raven `raven1` missile (class 135, owner 267). d2rs: Raven stats 4..6 HP, physical
   1057 + poison 41 + leech 5 = total 1103 (+41 poison tick). The 1.14d physical part is
   exactly 512 (2.0 HP) lower. The first `raven1` hit (f65, 1308) matches, so the missile
   record and roll are right; only the later hit is 512 lower. d2rs' player has
   `dr_normal` = 0 at both hits (damage.md §4.1). Because 1144 >= maxhp/16 the d2rs hit
   then draws the player seed (`no_get_hit` mask(2)) and enters GH (mode 4); 1.14d's 632
   stays under the divisor: no draw, no GH. So the seed (C029) and mode (C054) divergences
   are consequences of the damage amount. Next step: record the rng channel for
   gen-ai-bloodraven (Wine; a check copy with `channels rng`) and read which 1.14d site
   subtracts the 512 (candidates: a flat physical reduction on the player after f65, or a
   missile-hit step that `missiles/hit.rs::damage_tail` skips).
3. **C056 gen-su-12 f54** (clawviper3, SerpentCharge): 1.14d applies the charge hit
   (player mode 19, hp 7507) at f54 with the monster still at (5147,4262); d2rs steps
   once more to (5145,4263) and hits at f55. In 1.14d the charge `do` (bodies-2.md §5.4
   step 5) runs with E flags bit 2 (range 3), i.e. the path step returns "finished"
   before moving; d2rs' charge path ends one step later (type-7 path built in
   `wiring/interaction/body_path.rs`, 0x00649970). Needs the 1.14d stop rule for
   target-unit paths.

No code was changed in crates/ (instrumentation used for the analysis was reverted).
