# Spec: Skills — Monster skill rows (per-skill index and checks)

- **Status:** draft: the index below is read from the 1.14d `skills.txt`
  and `monstats.txt` and `functions.tsv`; every body it points at is already
  specified (`bodies*.md`, read from the 1.14d `Game.exe`). The per-skill
  checks (`gen-monskill-<Id>`) compare the cast against the original; their
  verdicts are in `docs/handoff/rc-spec-monskill.md` and
  `docs/handoff/ledger/rc-spec-monskill.tsv`.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::skills::use_::bodies` (no new code; wiring only)
- **Related specs:** `skills/use.md` (§2–§5 intent, start, do), `skills/bodies.md`,
  `skills/bodies-2.md`, `skills/bodies-3.md`, `skills/bodies-4.md`,
  `skills/functions.tsv`, `monsters/ai.md`, `monsters/ai-bodies.md`,
  `tools/scenario-diff.md`, `tools/poke.md`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 32–40 |
| Inputs | 41–48 |
| Outputs / state changes | 49–53 |
| Rules | 54–86 |
| Per-skill index | 87–158 |
| Constants & data dependencies | 159–163 |
| Randomness | 164–168 |
| Edge cases & original bugs | 169–172 |
| Test vectors | 173–177 |
| Provenance | 178–182 |
| Open questions | 183–195 |
<!-- /index -->

## Summary

The 64 monster / NPC rows of `skills.txt` (charclass blank, Id 156–216 and
281–356) that had no per-skill record: for each, which start and do
function runs, which missile it fires, which monster classes cast it at
which skill level, and the check that compares the cast with 1.14d. The
behaviour of each body is in the spec named in the table; this file adds
nothing to those bodies, it joins a skill row to them.

## Inputs

| Name | Type | Source |
|---|---|---|
| `skills.txt` row | `srvstfunc`, `srvdofunc`, `srvmissile`, `monanim`, `summon` | the 1.14d patch table |
| `monstats.txt` row | `Skill1..8`, `Sk1mode..Sk8mode`, `Sk1lvl..Sk8lvl`, `enabled`, `boss` | the 1.14d patch table |
| `functions.tsv` | slot address, body spec | `skills/functions.tsv` |

## Outputs / state changes

None of its own: the cast runs through `skills/use.md` §5 and the bodies
named below.

## Rules

1. A monster casts a skill only through its `monstats.txt` row: slot `k`
   (1–8) holds the skill name, `Sk<k>mode` the monster sequence / mode used
   for the cast, `Sk<k>lvl` the skill level. Which slot the AI picks, and when,
   is the AI think function's business (`monsters/ai.md`,
   `monsters/ai-bodies.md`), not the skill row's.
2. The cast then follows the use pipeline unchanged: mode from `monanim`
   (`use.md` §5.1), the start function if `srvstfunc` is set, the frame
   events, then the do wrapper `0x0056FC50` (`use.md` §5.4): the do function
   if `srvdofunc` is set, otherwise (no row has both) the `srvmissile` class
   is created at the target (step 7) and the do reports success. A row with
   no function and no missile (SkeletonRaise, 158) does nothing server-side
   in the do (step 8, result 0): it only names the skeletons' raise
   animation; the raise itself is the necromancer's skill
   (`monsters/ai-bodies.md`).
3. Function numbers in the table are the `functions.tsv` slot (`srvst` /
   `srvdo`), their 1.14d address, and the spec section holding the body.
   Several skills share a body (srvdo 28 `Meteor`, 95 MonInferno, 98
   MonTeleport, 110 Hireable / Rogue missile, 22 Nova, 24 Firewall, 17 Charged
   Bolt, 30 Curse): the sharing is shown by equal numbers.
4. Check selection (`tools/check-gen/check_gen.py`, family `monskill`): one
   check per row, `gen-monskill-<Id>`. The monster class is the first
   `monstats` row (enabled, not a boss, lowest `hcIdx`; else enabled boss; else
   the lowest) that lists the skill. It is spawned normal at (+3, -2) from the
   player in the Blood Moor (variant `blood-moor-empty`), with game and unit
   seeds fixed at frame 30, and run 500 ticks. The AI casts the skill on its
   own; the check does not force it. The `state` channel compares the units,
   their modes, states, stats and missiles frame by frame. A check whose
   monster never casts inside the window proves the idle / think frames only;
   such rows are listed in the hand-back as "no cast reached" and stay
   PROVISIONAL (REC-2600).

## Per-skill index

Mon. users: `hcIdx id [Sk mode] [L level]` of the first three enabled
classes (+ count of the rest).

| Id | Skill | Start (srvst) | Do (srvdo) | srvmissile | monanim | Monster users | Check |
|---|---|---|---|---|---|---|---|
| 156 | Fire Hit | 42 `0x005CAE40` bodies-3.md §5.4 | 83 `0x005CAF30` bodies-3.md §5.5 | - | xx | 29 sandraider1 seq_firehit L1, 30 sandraider2 seq_firehit L2, 31 sandraider3 seq_firehit L3 (+7) | `gen-monskill-156` |
| 157 | UnHolyBolt | - | 85 `0x005CB0C0` bodies-3.md §4.9 | - | xx | 101 unraveler1 seq_mummyres L1, 102 unraveler2 seq_mummyres L2, 103 unraveler3 seq_mummyres L3 (+3) | `gen-monskill-157` |
| 158 | SkeletonRaise | - | - | - | xx | 0 skeleton1 seq_skeletonraise L1, 1 skeleton2 seq_skeletonraise L1, 2 skeleton3 seq_skeletonraise L1 (+55) | `gen-monskill-158` |
| 159 | MaggotEgg | 43 `0x005CAF80` bodies-3.md §5.6 | 84 `0x005CAFA0` bodies-3.md §5.7 | - | xx | 190 maggotegg1 seq_maggotegg L2, 191 maggotegg2 seq_maggotegg L3, 192 maggotegg3 seq_maggotegg L4 (+3) | `gen-monskill-159` |
| 161 | MagottUp | 44 `0x005CB170` bodies-3.md §5.8 | - | - | xx | 68 sandmaggot1 seq_maggotup L1, 69 sandmaggot2 seq_maggotup L1, 70 sandmaggot3 seq_maggotup L1 (+9) | `gen-monskill-161` |
| 162 | MagottDown | 45 `0x005CB270` bodies-3.md §5.9 | 86 `0x005CB300` bodies-3.md §5.10 | - | xx | 68 sandmaggot1 seq_maggotdown L1, 69 sandmaggot2 seq_maggotdown L1, 70 sandmaggot3 seq_maggotdown L1 (+9) | `gen-monskill-162` |
| 163 | MagottLay | - | 87 `0x005CB3C0` bodies-3.md §5.11 | - | xx | 68 sandmaggot1 seq_maggotlay L1, 69 sandmaggot2 seq_maggotlay L1, 70 sandmaggot3 seq_maggotlay L1 (+9) | `gen-monskill-163` |
| 166 | Swarm Move | 48 `0x005CBBF0` bodies-3.md §5.15 | 90 `0x005CBC80` bodies-3.md §5.16 | - | xx | 87 swarm1 seq_swarmmove L1, 88 swarm2 seq_swarmmove L1, 89 swarm3 seq_swarmmove L1 (+2) | `gen-monskill-166` |
| 167 | Nest | 49 `0x005CBD10` bodies-3.md §4.6 | 91 `0x005CBE00` bodies-3.md §4.7 | - | xx | 206 crownest1 seq_nestlay L1, 207 crownest2 seq_nestlay L1, 208 crownest3 seq_nestlay L1 (+11) | `gen-monskill-167` |
| 169 | VampireFireball | - | - | vampirefireball | SC | 131 vampire1 SC L2, 132 vampire2 SC L3, 133 vampire3 SC L4 (+6) | `gen-monskill-169` |
| 171 | VampireMeteor | - | 28 `0x005CA3E0` bodies.md §8.12 | - | SC | 131 vampire1 SC L2, 132 vampire2 SC L3, 133 vampire3 SC L4 (+6) | `gen-monskill-171` |
| 173 | SpiderLay | - | 23 `0x005C9C10` bodies.md §8.17 | - | A2 | 122 arach1 A2 L1, 123 arach2 A2 L2, 124 arach3 A2 L3 (+4) | `gen-monskill-173` |
| 176 | Submerge | 51 `0x005CC1D0` bodies-3.md §5.20 | 94 `0x005CC1F0` bodies-3.md §5.21 | - | xx | 247 frogdemon1 seq_froghidden L1, 248 frogdemon2 seq_froghidden L1, 249 frogdemon3 seq_froghidden L1 (+6) | `gen-monskill-176` |
| 177 | FetishAura | - | 111 `0x005CE670` bodies-3.md §5.22 | - | SC | 278 fetishshaman1 A1 L1, 279 fetishshaman2 A1 L2, 280 fetishshaman3 A1 L3 (+5) | `gen-monskill-177` |
| 178 | FetishInferno | 53 `0x005CC240` bodies-3.md §4.1 | 95 `0x005CC4E0` bodies-3.md §4.2 | - | A1 | 278 fetishshaman1 A1 L7, 279 fetishshaman2 A1 L8, 280 fetishshaman3 A1 L9 (+5) | `gen-monskill-178` |
| 179 | ZakarumHeal | - | 96 `0x005CC840` bodies-3.md §4.10 | - | S1 | 238 cantor1 S1 L1, 239 cantor2 S1 L2, 240 cantor3 S1 L3 (+7) | `gen-monskill-179` |
| 180 | Emerge | 52 `0x005CC220` bodies-3.md §5.23 | - | - | S1 | 247 frogdemon1 S1 L1, 248 frogdemon2 S1 L1, 249 frogdemon3 S1 L1 (+6) | `gen-monskill-180` |
| 181 | Resurrect | - | 97 `0x005CCB10` bodies-3.md §4.11 | - | xx | 58 fallenshaman1 seq_shamanresurrect L1, 59 fallenshaman2 seq_shamanresurrect L1, 60 fallenshaman3 seq_shamanresurrect L1 (+5) | `gen-monskill-181` |
| 182 | Bestow | - | 96 `0x005CC840` bodies-3.md §4.10 | - | xx | 101 unraveler1 seq_mummyres L1, 102 unraveler2 seq_mummyres L1, 103 unraveler3 seq_mummyres L1 (+11) | `gen-monskill-182` |
| 184 | MonTeleport | - | 98 `0x005CCC80` bodies-3.md §4.3 | - | S1 | 238 cantor1 S1 L1, 239 cantor2 S1 L1, 240 cantor3 S1 L1 (+4) | `gen-monskill-184` |
| 196 | FingerMageSpider | - | 101 `0x005CCFA0` bodies-3.md §5.27 | - | S1 | 304 fingermage1 S1 L3, 305 fingermage2 S1 L4, 306 fingermage3 S1 L5 (+2) | `gen-monskill-196` |
| 205 | MonBlizzard | - | 28 `0x005CA3E0` bodies.md §8.12 | - | S1 | 238 cantor1 S1 L1, 239 cantor2 S1 L1, 240 cantor3 S1 L1 (+3) | `gen-monskill-205` |
| 206 | Mosquito | 55 `0x005CD910` bodies-4.md §3.1 | 107 `0x005CDA00` bodies-4.md §3.2 | - | xx | 114 mosquito1 seq_mosquitoskill L1, 115 mosquito2 seq_mosquitoskill L1, 116 mosquito3 seq_mosquitoskill L1 (+1) | `gen-monskill-206` |
| 210 | MonBoneArmor | - | 18 `0x005C9480` bodies.md §4.3 | - | S1 | 311 doomknight2 S1 L5, 312 doomknight3 S1 L6, 701 dkmag1 S1 L6 (+2) | `gen-monskill-210` |
| 211 | MonBoneSpirit | - | 10 `0x005DB6D0` bodies.md §8.19 | - | S1 | 311 doomknight2 S1 L2, 312 doomknight3 S1 L2, 701 dkmag1 S1 L2 (+2) | `gen-monskill-211` |
| 212 | MonCurseCast | - | 112 `0x005CE2B0` bodies-4.md §3.3 | - | S2 | 312 doomknight3 S2 L3, 701 dkmag1 S2 L3, 702 dkmag2 S2 L3 (+1) | `gen-monskill-212` |
| 214 | RegurgitatorEat | - | 108 `0x005CDC10` bodies-4.md §3.4 | - | S1 | 307 regurgitator1 S1 L1, 308 regurgitator2 S1 L2, 309 regurgitator3 S1 L3 (+1) | `gen-monskill-214` |
| 215 | MonFrenzy | 64 `0x005CDF00` bodies-3.md §4.12 | 109 `0x005CDF10` bodies-3.md §4.13 | - | A2 | 127 thornhulk1 A2 L1, 128 thornhulk2 A2 L2, 129 thornhulk3 A2 L3 (+2) | `gen-monskill-215` |
| 282 | Imp Inferno | 59 `0x005D1280` bodies-4.md §3.6 | 126 `0x005D1350` bodies-4.md §3.7 | - | SC | 492 imp1 SC L1, 493 imp2 SC L2, 494 imp3 SC L3 (+5) | `gen-monskill-282` |
| 283 | Imp Fireball | - | - | impfireball | S2 | 492 imp1 S2 L1, 493 imp2 S2 L1, 494 imp3 S2 L2 (+5) | `gen-monskill-283` |
| 284 | Baal Taunt | - | 28 `0x005CA3E0` bodies.md §8.12 | - | A1 | 545 baaltaunt A1 L1, 574 worldstoneeffect A1 L1 | `gen-monskill-284` |
| 290 | Cry Help | - | 128 `0x005D1800` bodies-4.md §3.11 | - | S1 | 479 overseer1 S1 L1, 480 overseer2 S1 L1, 481 overseer3 S1 L1 (+3) | `gen-monskill-290` |
| 291 | Healing Vortex | - | - | healing vortex | S2 | 479 overseer1 S2 L1, 480 overseer2 S2 L1, 481 overseer3 S2 L1 (+3) | `gen-monskill-291` |
| 293 | Self-resurrect | 61 `0x005D1BF0` bodies-4.md §3.12 | - | - | S1 | 436 reanimatedhorde1 S1 L1, 437 reanimatedhorde2 S1 L1, 438 reanimatedhorde3 S1 L1 (+3) | `gen-monskill-293` |
| 295 | Overseer Whip | - | 131 `0x005D1F70` bodies-4.md §3.14 | - | A2 | 479 overseer1 A2 L1, 480 overseer2 A2 L1, 481 overseer3 A2 L1 (+4) | `gen-monskill-295` |
| 299 | Imp Fire Missile | - | 132 `0x005D2090` bodies-4.md §3.15 | - | A1 | 492 imp1 A1 L1, 493 imp2 A1 L1, 494 imp3 A1 L1 (+2) | `gen-monskill-299` |
| 300 | Impregnate | - | 133 `0x005D2250` bodies-4.md §3.16 | - | S1 | 546 putriddefiler1 S1 L1, 547 putriddefiler2 S1 L1, 548 putriddefiler3 S1 L1 (+2) | `gen-monskill-300` |
| 301 | Siege Beast Stomp | - | 134 `0x005D2320` bodies-4.md §3.17 | - | A2 | 441 siegebeast1 A2 L1, 442 siegebeast2 A2 L2, 443 siegebeast3 A2 L3 (+2) | `gen-monskill-301` |
| 308 | DeathMaul | - | 136 `0x005D25B0` bodies-4.md §3.20 | - | xx | 529 deathmauler1 seq_deathmaulerdig L1, 530 deathmauler2 seq_deathmaulerdig L2, 531 deathmauler3 seq_deathmaulerdig L3 (+3) | `gen-monskill-308` |
| 309 | Defense Curse | - | 30 `0x005C37C0` bodies.md §4.4 | - | S2 | 469 succubus1 S2 L1, 470 succubus2 S2 L1, 471 succubus3 S2 L2 (+18) | `gen-monskill-309` |
| 310 | Blood Mana | - | 30 `0x005C37C0` bodies.md §4.4 | - | S2 | 469 succubus1 S2 L1, 470 succubus2 S2 L1, 471 succubus3 S2 L2 (+18) | `gen-monskill-310` |
| 319 | MegademonInferno | 53 `0x005CC240` bodies-3.md §4.1 | 95 `0x005CC4E0` bodies-3.md §4.2 | - | S1 | 360 megademon1 S1 L6, 361 megademon2 S1 L7, 362 megademon3 S1 L8 (+4) | `gen-monskill-319` |
| 321 | CountessFirewall | - | 24 `0x005C9EA0` bodies-2.md §6.2 | - | A1 | 45 corruptrogue3 A1 L10 | `gen-monskill-321` |
| 322 | ImpBolt | - | 17 `0x005C9300` bodies-2.md §3.2 | - | A1 | 492 imp1 S2 L1, 493 imp2 S2 L1, 494 imp3 S2 L1 (+5) | `gen-monskill-322` |
| 323 | Horror Arctic Blast | 53 `0x005CC240` bodies-3.md §4.1 | 95 `0x005CC4E0` bodies-3.md §4.2 | - | xx | 501 frozenhorror1 seq_horrorarcticblast L4, 502 frozenhorror2 seq_horrorarcticblast L4, 503 frozenhorror3 seq_horrorarcticblast L5 (+3) | `gen-monskill-323` |
| 327 | Resurrect2 | - | 97 `0x005CCB10` bodies-3.md §4.11 | - | xx | 101 unraveler1 seq_mummyres L1, 102 unraveler2 seq_mummyres L1, 103 unraveler3 seq_mummyres L1 (+15) | `gen-monskill-327` |
| 328 | BloodLordFrenzy | 37 `0x005DAF40` bodies.md §7.6 | 109 `0x005CDF10` bodies-3.md §4.13 | - | A2 | 506 bloodlord1 A2 L1, 507 bloodlord2 A2 L1, 508 bloodlord3 A2 L1 (+4) | `gen-monskill-328` |
| 330 | Imp Teleport | - | 129 `0x005D1AB0` bodies-4.md §3.24 | - | S1 | 492 imp1 S1 L1, 493 imp2 S1 L1, 494 imp3 S1 L1 (+6) | `gen-monskill-330` |
| 332 | ZakarumLightning | - | - | monsterlight | S1 | 238 cantor1 S1 L1, 239 cantor2 S1 L1, 240 cantor3 S1 L1 (+3) | `gen-monskill-332` |
| 333 | VampireMissile | - | - | firehead | SC | 131 vampire1 SC L2, 132 vampire2 SC L3, 133 vampire3 SC L4 (+6) | `gen-monskill-333` |
| 335 | DoomKnightMissile | - | 148 `0x005CDFB0` bodies-4.md §3.25 | - | S1 | 311 doomknight2 S1 L3, 312 doomknight3 S1 L3, 701 dkmag1 S1 L1 (+2) | `gen-monskill-335` |
| 339 | MonBow | 4 `0x005DA8B0` bodies.md §3.4 | - | cr_arrow6 | A1 | 613 cr_archer6 A1 L1, 614 cr_archer7 A1 L1, 722 cr_archer8 A1 L1 | `gen-monskill-339` |
| 340 | MonFireArrow | 4 `0x005DA8B0` bodies.md §3.4 | - | firearrow | A1 | 613 cr_archer6 A1 L9 | `gen-monskill-340` |
| 341 | MonColdArrow | 4 `0x005DA8B0` bodies.md §3.4 | - | coldarrow | A1 | 614 cr_archer7 A1 L9, 722 cr_archer8 A1 L9 | `gen-monskill-341` |
| 342 | MonExplodingArrow | 4 `0x005DA8B0` bodies.md §3.4 | - | explodingarrow | A1 | 613 cr_archer6 A1 L7 | `gen-monskill-342` |
| 343 | MonFreezingArrow | 4 `0x005DA8B0` bodies.md §3.4 | - | freezingarrow | A1 | 614 cr_archer7 A1 L7, 722 cr_archer8 A1 L7 | `gen-monskill-343` |
| 344 | MonPowerStrike | 6 `0x005DA940` bodies.md §7.2 | 2 `0x0056F1F0` bodies.md §4.2 | - | A1 | 615 cr_lancer6 A1 L9, 617 cr_lancer8 A1 L9, 723 cr_lancer8 A1 L9 | `gen-monskill-344` |
| 345 | SuccubusBolt | 4 `0x005DA8B0` bodies.md §3.4 | - | succubusmiss | S2 | 469 succubus1 S2 L1, 470 succubus2 S2 L1, 471 succubus3 S2 L1 (+14) | `gen-monskill-345` |
| 347 | MonIceSpear | 6 `0x005DA940` bodies.md §7.2 | 2 `0x0056F1F0` bodies.md §4.2 | - | A1 | 616 cr_lancer7 A1 L5 | `gen-monskill-347` |
| 348 | ShamanIce | - | - | glacialspike | xx | 645 fallenshaman6 seq_shamanresurrect L1 | `gen-monskill-348` |
| 352 | SerpentCharge | 31 `0x005CF6B0` bodies-2.md §5.3 | 67 `0x005CF900` bodies-2.md §5.4 | - | xx | 73 clawviper1 seq_serpentcharge L1, 74 clawviper2 seq_serpentcharge L1, 75 clawviper3 seq_serpentcharge L1 (+2) | `gen-monskill-352` |
| 354 | UnHolyBoltEx | - | - | unholybolt1 | xx | 667 unraveler6 seq_mummyres L1, 668 unraveler7 seq_mummyres L1, 669 unraveler8 seq_mummyres L1 (+1) | `gen-monskill-354` |
| 355 | ShamanFireEx | - | - | shafire1 | xx | 646 fallenshaman7 seq_shamanresurrect L1, 647 fallenshaman8 seq_shamanresurrect L1 | `gen-monskill-355` |
| 356 | Imp Fire Missile Ex | - | - | impmiss21 | A1 | 688 imp6 A1 L1, 689 imp7 A1 L1, 714 imp8 A1 L1 | `gen-monskill-356` |

## Constants & data dependencies

`skills.txt` (`srvstfunc`, `srvdofunc`, `srvmissile`, `monanim`),
`monstats.txt` (`Skill1..8`, `Sk1mode..Sk8mode`, `Sk1lvl..Sk8lvl`), `skills/functions.tsv`.

## Randomness

As the bodies named in the table; the checks fix the game seed and the
spawned unit's seed at frame 30 (`tools/check-gen`, `SEEDS`).

## Edge cases & original bugs

See the sections named in the table.

## Test vectors

The 64 `traces/checks/gen/gen-monskill-<Id>.check` files, run with
`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'gen-monskill-*' --orig-cache traces/orig-cache`.

## Provenance

Tables read from the 1.14d install (`data-tool excel-dir`); function slots
from `skills/functions.tsv` (addresses from the 1.14d `Game.exe`).

## Open questions

1. PROVISIONAL (REC-2600): no cast reached. In 22 checks the 1.14d monster
   never enters a cast / skill / sequence mode or fires a missile in 500 ticks, so
   they compare the idle and think frames only (156, 158, 166, 173, 176, 180, 210, 211,
   214, 215, 284, 290, 291, 293, 295, 300, 308, 321, 323, 328, 335, 352). Several
   skills there are melee-mode skills (Fire Hit, Quick Strike family) whose cast looks like
   an attack mode, so "no cast" may be too strict; two (176, 180) spawn no
   unit of the class at all (frogdemon under the poke). A forced cast
   needs a poke that sets the monster's used skill (not available, `poke.md`).
2. DIVERGED (13 checks, causes owned elsewhere): see
   `docs/handoff/rc-spec-monskill.md`.
