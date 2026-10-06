# Handoff: spec bodies audit, quests and skill functions — `claude/spec-bodies-quests-skills`

> Not yet folded into `docs/HANDOFF.md` (§1, §3, §7) and `docs/PLAN.md` (neither is edited here); the coordinator folds it, then this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation
from clear specs, medium (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `9b49081`. Repo only, no game files (M09). Inputs: `specs/world/quests.md`
+ `quests.tsv` + `quest-messages.tsv`, `specs/skills/use.md` +
`functions.tsv`, `specs/skills/levels.md`.

Task: audit every quest callback reported `unhandled` (HANDOFF §1 row 3h)
and every `SkillFunctions` slot (`srvst` 64 / `srvdo` 152, row 3i;
`use.md` OQ10); implement each body the spec states completely (all
branches, draws in order, constants, messages) and list what is missing
for the rest.

## 1. Result

Only **one** body in either catalogue is stated completely at this commit:
`srvst` 18 (Attract), whose `functions.tsv` note gives the 1.14d bytes
`B8 01 00 00 00 C2 08 00` (`mov eax, 1; ret 8`). Every other `unhandled`
quest callback and every other filled skill slot is missing at least one
branch, constant, draw or message (§3, §4). Nothing was guessed. The
quest module is unchanged: no Act I callback that `act1.rs` reports as
`unhandled` has a complete description in `quests.md` §10, and Acts II–V
are catalogued only (§11, OQ8).

## 2. Implemented (unverified, M02)

| Slot | Body | Where | Tests |
|---|---|---|---|
| `srvst` 18 Attract `0x005C3260` | returns 1, reads no argument, changes nothing | `skills/use_/bodies.rs` `start`; called from `start_core` (§5.3 step 6.6) before the seam | `attract_start_returns_one_and_charges_at_start` (the seam set to refuse is never called; with no `usemanaondo` mana is charged at start, `(4 + 0) << 8`; `periodic` deletes type-8 arg-0 timers), `attract_start_with_usemanaondo_charges_nothing` |

Seam change (inside `skills/use_`, no public signature changed):
`bodies::start(index)` / `bodies::do_(index)` return `Some(result)` for a
slot with a body here, `None` otherwise; `start_core`, `do_core` and
`active_state_event` call the body first and the `SkillFunctions` seam
only on `None`. `DO_BODIES` is empty; the `do_` hook exists so the next
specified do body needs no core change. `wiring::interaction::UseView`
is unchanged (it still forwards every other slot to `UseRest`).

Catalogue check (M05, M08): `bodies_match_tsv_notes` requires that every
`functions.tsv` row whose notes state a body (`1.14d body is …`) has that
body in `bodies` on a filled slot, and every other slot has none;
`bodies_check_reports_perturbations` changes srvst 18's note (reports
exactly `srvst 18`) and adds a stated body to srvst 2 (reports exactly
`srvst 2`). By hand: with `18 => None` three of the four tests fail.
`function_tables_match_tsv` (the `FUNCS` check) is unchanged and passes.
RNG: the body draws nothing (no draw-order assertion applies; the start
core's only draw is `interrupt_gate`'s, unchanged).

`Covers:` claims: `specs/skills/use.md §8` (2 tests), `§5.3 r6` (2 tests).

## 3. Missing spec: quests (one line each, for the local spec coordinator)

### 3.1 Act I callbacks still `unhandled` (owner `quests.md` §10)

| Chain | Function | What is missing |
|---|---|---|
| 1 | ev 0 `0x0058FF90` | which `quest-messages.tsv` state is added per NPC and player bits (§10.4 states none) |
| 1 | ev 10 `0x005901F0` | body (player leaves game) |
| 1 | active `0x005905B0` | NPC classes and flag tests that return true |
| 1 | seq `0x00590620` (and seq fns of 2–6) | guard: from which states the `seq_id` record goes to 1 (d2rs raises only 0 → 1, TODO in `act1::sequence`) |
| 1 | ev 8 helpers `0x00590190`, `0x00590080`, `0x005900E0` | party-goal membership test, COMPLETEDNOW / log-update bytes, the sound id (OQ7) |
| 2 | ev 0 `0x00590B10`, ev 10 `0x00590C10`, active `0x00591080` | bodies |
| 2 | ev 8 `0x00590EC0` | timer 15's callback (statuses, return value), the "adjacent room" test, whether state 4 / bit 14 / 0x5D depend on a qualifying player |
| 2 | ev 3 `0x00590FA0` | the 1.14d area level (17 is D2MOO's) and which players get bits 3 / 4 (also chain 1) |
| 3 | ev 0 `0x005916A0`, 3 `0x00591810`, 4 `0x00591960`, 6 `0x00591A90`, 9 `0x00591A20`, 10 `0x00591A60`, 13 `0x00591ED0`, 14 `0x005919D0`; status `0x00591D30`; active `0x00591C30` | bodies (§10.5 covers only messages 146 / 163, the Malus and the imbue) |
| 3 | msg 163 party iterate (in `0x00591490`) | membership test of "party members get bits 13, 1" (OQ7) |
| 3 | Malus operate `0x00591AC0`, level < 8 | the sound id |
| 4 | ev 0 `0x00592580`, 3 `0x00596DE0`, 4 `0x00592E20`, 6 `0x00592E60`, 9 `0x00592C80`, 10 `0x00592CF0`, 13 `0x00597030`, 14 `0x00592B90`; active `0x00592FB0` | bodies (§10.6 covers messages 97 / 112 / 118, the stones, Wirt, the Cow King) |
| 4 | `0x00593CB0` | 0x50 bytes before the five stone values (OQ5) |
| 4 | `0x00597310` | A1Q4 act-change hook body (§8.1 names it only) |
| 4 | `0x00592F80` | `add_link` special case (chain 4, object class 61) |
| 4 | timer of period 1 (D2MOO) | 1.14d existence and callback |
| 5 | ev 0 `0x00594C50`, 10 `0x00594BB0`; active `0x005952C0`; seq `0x00595240` | bodies |
| 5 | ev 3 `0x00595010` | level ids of the Forgotten Tower and Tower Cellar 5, states and bits (§10.7 is D2MOO-derived) |
| 5 | ev 8 `0x00595710` | the Countess kill: player tests, "COMPLETEDNOW elsewhere in Act I" iterate, timer 7's callback, trap spawns (D2MOO-derived) |
| 5 | msgs 140–145 (in `0x00594960`) | bits set and the state guard (d2rs moves 4 → 5 only, TODO) |
| 5 | object 0x173 event 7 | the Countess chest trap function (§9.5) |
| 6 | ev 0 `0x00595E20`; active `0x005967F0`; seq `0x005968E0` | bodies |
| 6 | ev 3 `0x00596010` | Catacombs level ids, states, bits; and the call with Warriv's arguments from §8.1 |
| 6 | ev 8 `0x005965A0` | player bookkeeping `0x00596210`, the per-player iterates, the period-1 timer's callback |
| 25, 30 | status `0x00596BB0` | return value (§10.3 says "returns false" only for A1Q0's `0x0058FB40`) |
| 37 | ev 0 `0x0058F8F0` | which state (0 / 1) per NPC and player class, and the intro-bit test |
| 37 | ev 11 `0x0058F870` | the messages it reacts to, the intro bit it sets (§6.7), its order vs. the mercenary reward (`wire-open-seams.md`) |
| 37 | active `0x0058F9C0` | conditions |

### 3.2 Other quest hooks reported `unhandled` by `world/quests.rs`

| Function | What is missing |
|---|---|
| `0x00588C50` | A5Q2 barbarians left (0x50 / 0x5D extra) |
| `0x005BCFD0` | A3Q6 object warp (non-102 levels, §8.1) |
| `0x0059D6A0` | A2Q6 true tomb clue (`trs `, §9.4) |
| `0x005991B0`, `0x0059C3B0` | `add_link` specials of chains 8 and 12 (§4.6) |
| `0x005467E0` tyrael2 | the 0x5D sent before 0x61 (OQ10) |
| `0x005449E0` classes 0x16F, 0xBD, 0x1A, 0x7A, 0x83, 0x155, 0x178, 0x1CB–0x1CD, 0x1DA–0x1DC | per-class quest functions (§9.5) |

### 3.3 Acts II–V records (`quests.tsv` spec `catalogued`)

| Chain | Record | What is missing |
|---|---|---|
| 7 | A2Q0 Jerhyn gossip | state machine (§11, OQ8): callbacks 0 `0x005986B0`, 10 `0x005987B0`, 11 `0x00598640`, 13 `0x005987D0`; status `0x00598770`, active `0x00598780`; init state / active bytes |
| 8 | A2Q1 Radament's Lair | state machine (§11, OQ8): callbacks 0 `0x00598C40`, 2 `0x00598990`, 3 `0x00599130`, 8 `0x00599020`, 10 `0x00598980`, 11 `0x00598A70`, 13 `0x00599230`; active `0x00598910`, seq `0x005991C0`; init state / active bytes |
| 9 | A2Q2 The Horadric Staff | state machine (§11, OQ8): callbacks 0 `0x00599910`, 3 `0x00599A00`, 4 `0x00599A30`, 5 `0x00599B30`, 8 `0x00599A20`, 9 `0x0059EA20`, 10 `0x00599A10`, 11 `0x005997F0`, 13 `0x0059E850`, 14 `0x0059E970`; status `0x0059E630`, active `0x005996C0`; init state / active bytes |
| 10 | A2Q3 Tainted Sun | state machine (§11, OQ8): callbacks 0 `0x0059A270`, 3 `0x0059EE00`, 10 `0x00599FB0`, 11 `0x0059EBE0`, 13 `0x0059A5D0`; active `0x0059A1C0`, seq `0x0059A480`; init state / active bytes |
| 11 | A2Q4 Arcane Sanctuary | state machine (§11, OQ8): callbacks 0 `0x0059B1C0`, 2 `0x0059AE60`, 3 `0x0059F0C0`, 10 `0x0059AD40`, 11 `0x0059AF50`, 13 `0x0059B530`; active `0x0059ACA0`, seq `0x0059B500`; init state / active bytes |
| 12 | A2Q5 The Summoner | state machine (§11, OQ8): callbacks 0 `0x0059BBC0`, 3 `0x0059C200`, 8 `0x0059C150`, 10 `0x0059BF70`, 11 `0x0059BD70`, 13 `0x0059C220`; active `0x0059BB40`; init state / active bytes |
| 13 | A2Q6 The Seven Tombs | state machine (§11, OQ8): callbacks 0 `0x0059C3C0`, 2 `0x0059C760`, 3 `0x0059D1C0`, 8 `0x0059D050`, 10 `0x0059C6B0`, 11 `0x0059CB20`, 13 `0x0059D4F0`, 14 `0x0059D4D0`; status `0x0059CA50`, active `0x0059D300`, seq `0x0059D450`; init state / active bytes |
| 26 | A2Q7 guard gossip | state machine (§11, OQ8): callbacks 0 `0x0059E140`, 8 `0x0059E2A0`, 11 `0x0059E0E0`; status `0x0059E2B0`, active `0x0059E2C0`; init state / active bytes |
| 27 | A2Q8 guard gossip | state machine (§11, OQ8): callbacks 0 `0x0059E3F0`, 11 `0x0059E3C0`; status `0x0059E4A0`, active `0x0059E4B0`; init state / active bytes |
| 14 | A3Q0 Hratli gossip | state machine (§11, OQ8): callbacks 0 `0x005B6F20`, 11 `0x005B6ED0`, 13 `0x005B6FD0`; status `0x005B6F90`, active `0x005B6FA0`; init state / active bytes |
| 15 | A3Q1 Lam Esen's Tome | state machine (§11, OQ8): callbacks 0 `0x005B7400`, 3 `0x005B7620`, 4 `0x005B7970`, 5 `0x005B7A00`, 9 `0x005B7B80`, 10 `0x005B7610`, 11 `0x005B7740`, 13 `0x005B7C10`, 14 `0x005B7BC0`; status `0x005B7D70`, active `0x005B72B0`, seq `0x005B7CD0`; init state / active bytes |
| 16 | A3Q2 Khalim's Will | state machine (§11, OQ8): callbacks 0 `0x005BD630`, 3 `0x005B7FF0`, 4 `0x005B8210`, 10 `0x005B7FE0`, 11 `0x005B8060`, 13 `0x005B8470`; status `0x005BD850`, active `0x005BD500`, seq `0x005B8150`; init state / active bytes |
| 17 | A3Q3 Blade of the Old Religion | state machine (§11, OQ8): callbacks 0 `0x005B8EC0`, 2 `0x005B8C50`, 3 `0x005B9090`, 4 `0x005B9520`, 9 `0x005B96C0`, 11 `0x005B9240`, 13 `0x005B9700`, 14 `0x005B9680`; status `0x005B8CC0`, active `0x005B8E10`, seq `0x005B9610`; init state / active bytes |
| 18 | A3Q4 The Golden Bird | state machine (§11, OQ8): callbacks 0 `0x005B9ED0`, 3 `0x005BA0A0`, 4 `0x005BA6A0`, 9 `0x005BA9A0`, 10 `0x005BA990`, 11 `0x005BA320`, 13 `0x005BA870`, 14 `0x005BA9E0`; status `0x005BA170`, active `0x005B9D90`, seq `0x005BA7B0`; init state / active bytes |
| 19 | A3Q5 The Blackened Temple | state machine (§11, OQ8): callbacks 0 `0x005BAFD0`, 3 `0x005BB120`, 8 `0x005BBC00`, 10 `0x005BB400`, 11 `0x005BB210`, 13 `0x005BB480`; active `0x005BAD60`, seq `0x005BB410`; init state / active bytes |
| 20 | A3Q6 The Guardian | state machine (§11, OQ8): callbacks 0 `0x005BC270`, 3 `0x005BC3C0`, 8 `0x005BC8B0`, 10 `0x005BC710`, 11 `0x005BC5A0`, 13 `0x005BCC80`; active `0x005BBFE0`, seq `0x005BCC60`; init state / active bytes |
| 28 | A3Q7 Dark Wanderer | state machine (§11, OQ8): callbacks 13 `0x005BD2C0`; status `0x005BD0B0`, active `0x005BD0C0`; init state / active bytes |
| 21 | A4Q0 Tyrael gossip | state machine (§11, OQ8): callbacks 0 `0x005B36D0`, 11 `0x005B36A0`; status `0x005B3740`, active `0x005B3750`; init state / active bytes |
| 22 | A4Q1 The Fallen Angel | state machine (§11, OQ8): callbacks 0 `0x005B3BB0`, 2 `0x005B41D0`, 3 `0x005B4140`, 8 `0x005B4020`, 10 `0x005B3E30`, 11 `0x005B39A0`, 13 `0x005B4220`; active `0x005B37F0`, seq `0x005B38E0`; init state / active bytes |
| 23 | A4Q2 Terror's End | state machine (§11, OQ8): callbacks 0 `0x005B4790`, 2 `0x005B4F90`, 3 `0x005B4EA0`, 8 `0x005B52E0`, 11 `0x005B4670`, 13 `0x005B5080`, 14 `0x005B5030`; active `0x005B4450`, seq `0x005B4530`; init state / active bytes |
| 24 | A4Q3 Hell's Forge | state machine (§11, OQ8): callbacks 0 `0x005B62D0`, 2 `0x005B6440`, 3 `0x005B6080`, 4 `0x005B5A90`, 8 `0x005B65D0`, 9 `0x005B64A0`, 10 `0x005B6050`, 11 `0x005B6100`, 13 `0x005B64E0`, 14 `0x005B64C0`; active `0x005B5EE0`, seq `0x005B5F40`; init state / active bytes |
| 29 | A4Q4 Malachai | state machine (§11, OQ8): callbacks 0 `0x005B6940`; status `0x005B69E0`, active `0x005B69F0`; init state / active bytes |
| 31 | A5Q1 Siege on Harrogath | state machine (§11, OQ8): callbacks 0 `0x00586FF0`, 2 `0x00586D80`, 3 `0x005873E0`, 8 `0x00587330`, 10 `0x005870D0`, 11 `0x00586E70`, 13 `0x005875F0`; status `0x00586CE0`, active `0x005874E0`, seq `0x00587560`; init state / active bytes |
| 32 | A5Q2 Rescue on Mount Arreat | state machine (§11, OQ8): callbacks 0 `0x00587D30`, 2 `0x00587A30`, 3 `0x00588200`, 8 `0x00588040`, 10 `0x00587F20`, 11 `0x00587B00`, 13 `0x00588470`, 14 `0x00588450`; active `0x00588340`, seq `0x005883E0`; init state / active bytes |
| 33 | A5Q3 Prison of Ice | state machine (§11, OQ8): callbacks 0 `0x00589B00`, 2 `0x00588F80`, 3 `0x00589D80`, 10 `0x00589D20`, 11 `0x00589580`, 13 `0x0058A110`, 14 `0x0058A1E0`; status `0x0058A300`, active `0x00589F10`, seq `0x00589160`; init state / active bytes |
| 34 | A5Q4 Betrayal of Harrogath | state machine (§11, OQ8): callbacks 0 `0x0058B2C0`, 2 `0x0058B050`, 3 `0x0058BAB0`, 8 `0x0058B7A0`, 10 `0x0058B460`, 11 `0x0058B1C0`, 13 `0x0058B990`; status `0x0058AF00`, active `0x0058B870`, seq `0x0058B140`; init state / active bytes |
| 35 | A5Q5 Rite of Passage | state machine (§11, OQ8): callbacks 0 `0x0058C440`, 2 `0x0058BE80`, 3 `0x0058CBA0`, 8 `0x0058C9A0`, 10 `0x0058C750`, 11 `0x0058C290`, 13 `0x0058CDA0`; active `0x0058CCF0`, seq `0x0058CD30`; init state / active bytes |
| 36 | A5Q6 Eve of Destruction | state machine (§11, OQ8): callbacks 0 `0x0058D9F0`, 2 `0x0058D870`, 3 `0x0058E190`, 8 `0x0058DF20`, 10 `0x0058DCF0`, 11 `0x0058D940`, 13 `0x0058E430`, 14 `0x0058E3F0`; active `0x0058E2B0`, seq `0x0058E390`; init state / active bytes |
| 38 | Act II intro | state machine (§11, OQ8): callbacks 0 `0x005984C0`, 11 `0x005983E0`; status `0x005985B0`, active `0x005985C0`; init state / active bytes |
| 39 | Act III intro | state machine (§11, OQ8): callbacks 0 `0x005B6D30`, 11 `0x005B6C60`; status `0x005B6E20`, active `0x005B6E30`; init state / active bytes |
| 40 | Act V intro | everything: init `0x0058EA50` not disassembled (OQ6): callbacks, table, status / active fns |

## 4. Missing spec: skill function bodies (one line per filled slot)

`srvst` 18 is implemented (§2); empty slots (srvst 0, 30, 66–90; srvdo
0, 153–190) need nothing (the cores handle them). Generated from
`functions.tsv` at this commit. Priority per `use.md` OQ10: srvdo 1, 2,
65, 30, 18, then the `srvmissile` path.

| Slot | D2MOO name | Skills | What is missing |
|---|---|---|---|
| srvst 1 | SrvSt01_Attack_LeftHandSwing | Attack,Left Hand Swing | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 2 | SrvSt02_Kick | Kick | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 3 | SrvSt03_Unsummon | Unsummon | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 4 | SrvSt04_Arrow_Bolt | 18 skills (e.g. Fire Arrow,Cold Arrow,Multiple Shot,Poison Javelin) | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 5 | SrvSt05_Jab | Jab | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 6 | SrvSt06_PowerStrike_ChargedStrike | Power Strike,Charged Strike,MonPowerStrike,MonIceSpear | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 7 | SrvSt07_Impale | Impale | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 8 | SrvSt08_Strafe | Strafe | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 9 | SrvSt09_Fend | Fend | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 10 | SrvSt10_LightningStrike | Lightning Strike | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 11 | SrvSt11_Inferno_ArcticBlast | Inferno,Arctic Blast | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 12 | SrvSt12_Telekinesis_DragonFlight | Telekinesis,Dragon Flight | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 13 | SrvSt13_ThunderStorm | Thunder Storm | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 14 | SrvSt14_Hydra | Hydra | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 15 | SrvSt15_RaiseSkeleton_Mage | Raise Skeleton,Raise Skeletal Mage | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 16 | SrvSt16_PoisonDagger | Poison Dagger | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 17 | SrvSt17_Poison_CorpseExplosion | Corpse Explosion,Poison Explosion,NihlathakCorpseExplosion | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 19 | SrvSt19_BonePrison | Bone Prison | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 20 | SrvSt20_IronGolem | IronGolem | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 21 | SrvSt21_Revive | Revive | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 22 | SrvSt22_PsychicHammer | Psychic Hammer | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 23 | SrvSt23_AssasinChargeStrikes | Tiger Strike,Fists of Fire,Cobra Strike,Claws of Thunder,Blades of Ice,Royal Strike | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 24 | SrvSt24_DragonTalon | Dragon Talon | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 25 | SrvSt25_64_DragonClaw_MonFrenzy | Dragon Claw | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 26 | SrvSt26_BladeFury | Blade Fury | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 27 | SrvSt27_DragonTail | Dragon Tail | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 28 | SrvSt28_BladeShield | Blade Shield | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 29 | SrvSt29_Sacrifice | Sacrifice | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 31 | SrvSt31_Charge | Charge,SerpentCharge | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 32 | SrvSt32_Conversion_Bash_Stun_Concentrate_BearSmite | Conversion,Bash,Stun,Concentrate,BearSmite | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 33 | SrvSt33_FindPotion_GrimWard | Find Potion,Grim Ward | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 34 | SrvSt34_FindItem | Find Item | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 35 | SrvSt35_Vengeance | Vengeance | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 36 | SrvSt36_HolyShield | Holy Shield | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 37 | SrvSt37_Zeal_Fury_BloodLordFrenzy | Zeal,Fury,BloodLordFrenzy | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 38 | SrvSt38_Whirlwind | Whirlwind | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 39 | SrvSt39_Berserk | Berserk | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 40 | SrvSt40_Leap | Leap | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 41 | SrvSt41_LeapAttack | Leap Attack | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 42 | SrvSt42_FireHit | Fire Hit | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 43 | SrvSt43_MaggotEgg | MaggotEgg | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 44 | SrvSt44_MaggotUp | MagottUp | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 45 | SrvSt45_MaggotDown | MagottDown | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 46 | SrvSt46_AndrialSpray | AndrialSpray | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 47 | SrvSt47_Jump | Jump | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 48 | SrvSt48_SwarmMove | Swarm Move | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 49 | SrvSt49_Nest_EvilHutSpawner | Nest,EvilHutSpawner | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 50 | SrvSt50_QuickStrike | Quick Strike | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 51 | SrvSt51_Submerge | Submerge | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 52 | SrvSt52_Emerge | Emerge | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 53 | SrvSt53_MonInferno | FetishInferno,DiabLight,mon inferno sentry,Baal Inferno,MegademonInferno,Horror Arctic Blast | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 54 | SrvSt54_DiabRun | DiabRun | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 55 | SrvSt55_Mosquito | Mosquito | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 56 | SrvSt56_FeralRage_Maul | Feral Rage,Maul | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 57 | SrvSt57_Rabies | Rabies | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 58 | SrvSt58_FireClaws | Fire Claws | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 59 | SrvSt59_ImpInferno | Imp Inferno | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 60 | SrvSt60_SuckBlood | Suck Blood | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 61 | SrvSt61_SelfResurrect | Self-resurrect | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 62 | SrvSt62_MinionSpawner | MinionSpawner | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 63 | SrvSt63_Corpse_VineCycler | CorpseCycler,VineCycler | whole body (branches, RNG draws, constants, messages, return value) |
| srvst 64 | SrvSt25_64_DragonClaw_MonFrenzy | MonFrenzy | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvst 65 | SrvSt65_Throw_LeftHandThrow | Throw,Left Hand Throw | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvdo 1 | SrvDo001_Attack_LeftHandSwing | Attack,Left Hand Swing | the Attack do (hit / damage call into `combat/*`, dual-wield handling, return value) |
| srvdo 2 | SrvDo002_Kick_PowerStrike_MonIceSpear_Impale_Bash_Stun_Concentrate_BearSmite_Vengeance_Berserk_FireClaws | 12 skills (e.g. Kick,Power Strike,Impale,Vengeance) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 3 | SrvDo003_Throw | Throw | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 4 | SrvDo004_Unsummon | Unsummon | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvdo 5 | SrvDo005_LeftHandThrow | Left Hand Throw | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 6 | SrvDo006_InnerSight_SlowMissiles | Inner Sight,Slow Missiles | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 7 | SrvDo007_Jab | Jab | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 8 | SrvDo008_MultipleShot_Teeth_ShockWave | Multiple Shot,Teeth,PrimePoisonball,Shock Wave | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 9 | SrvDo009_Frenzy | Frenzy | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 10 | SrvDo010_GuidedArrow_BoneSpirit | Guided Arrow,Bone Spirit,MonBoneSpirit | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 11 | SrvDo011_ChargedStrike | Charged Strike | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 12 | SrvDo012_Strafe | Strafe | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 13 | SrvDo013_Fend_Zeal_Fury | Fend,Zeal,Fury | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 14 | SrvDo014_LightningStrike | Lightning Strike | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 15 | SrvDo015_Dopplezon | Dopplezon | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 16 | SrvDo016_Valkyrie | Valkyrie | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 17 | SrvDo017_ChargedBolt_BoltSentry | Charged Bolt,PrimeBolt,BoltSentry,ImpBolt | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 18 | SrvDo018_DefensiveBuff | 11 skills (e.g. Frozen Armor,Shiver Armor,Chilling Armor,Bone Armor) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 19 | SrvDo019_Inferno_ArcticBlast | Inferno,Arctic Blast | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 20 | SrvDo020_StaticField | Static Field | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 21 | SrvDo021_Telekinesis | Telekinesis | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 22 | SrvDo022_NovaAttack | 9 skills (e.g. Frost Nova,Nova,Poison Nova,Howl) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 23 | SrvDo023_Blaze_EnergyShield_SpiderLay | Blaze,Energy Shield,SpiderLay,PrimeBlaze | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 24 | SrvDo024_FireWall | Fire Wall,VampireFirewall,PrimeFirewall,CountessFirewall | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 25 | SrvDo025_Enchant | Enchant | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 26 | SrvDo026_ChainLightning | Chain Lightning | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 27 | SrvDo027_Teleport | Teleport | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 28 | SrvDo028_Meteor_Blizzard_Eruption_BaalTaunt_Catapult | 12 skills (e.g. Meteor,Blizzard,VampireMeteor,MonBlizzard) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 29 | SrvDo029_ThunderStorm | Thunder Storm | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 30 | SrvDo030_Curse | 10 skills (e.g. Amplify Damage,Dim Vision,Weaken,Iron Maiden) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 31 | SrvDo031_RaiseSkeleton_Mage | Raise Skeleton,Raise Skeletal Mage | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 32 | SrvDo032_PoisonDagger | Poison Dagger | whole body (branches, RNG draws, constants, messages, return value); no Ghidra function at the entry (OQ1) |
| srvdo 33 | SrvDo033_PsychicHammer | Psychic Hammer | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 34 | SrvDo034_TigerStrike_CobraStrike_RoyalStrike | Tiger Strike,Cobra Strike,Royal Strike | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 35 | SrvDo035_FistsOfFire_ClawsOfThunder_BladesOfIce | Fists of Fire,Claws of Thunder,Blades of Ice | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 36 | SrvDo036_ClawsOfThunder_ProgressiveFn2 | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 37 | SrvDo037_ClawsOfThunder_ProgressiveFn3 | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 38 | SrvDo038_FistsOfFire_BladesOfIce_ProgressiveFn2 | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 39 | SrvDo039_FistsOfFire_BladesOfIce_ProgressiveFn3 | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 40 | SrvDo040_RoyalStrike_ProgressiveFn1 | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 41 | SrvDo041_RoyalStrike_ProgressiveFn3 | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 42 | SrvDo042_DragonTalon | Dragon Talon | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 43 | SrvDo043_ShockField | Shock Field | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 44 | SrvDo044_BladeSentinel | Blade Sentinel | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 45 | SrvDo045_Sentry | Charged Bolt Sentry,Wake of Fire Sentry,Lightning Sentry,Inferno Sentry,Death Sentry | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 46 | SrvDo046_DragonClaw | Dragon Claw | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 47 | SrvDo047_CloakOfShadows | Cloak of Shadows | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 48 | SrvDo048_BladeFury | Blade Fury | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 49 | SrvDo049_ShadowWarrior_Master | Shadow Warrior,Shadow Master | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 50 | SrvDo050_DragonTail | Dragon Tail | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 51 | SrvDo051_MindBlast | Mind Blast | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 52 | SrvDo052_DragonFlight | Dragon Flight | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 53 | SrvDo053_Unused | (none) | whole body; no 1.14d data row uses it (lowest priority) |
| srvdo 54 | SrvDo054_BladeShield | Blade Shield | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 55 | SrvDo055_CorpseExplosion | Corpse Explosion,mon death sentry,NihlathakCorpseExplosion | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 56 | SrvDo056_Golem | Clay Golem,BloodGolem,FireGolem | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 57 | SrvDo057_IronGolem | IronGolem | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 58 | SrvDo058_Revive | Revive | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 59 | SrvDo059_Attract | Attract | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 60 | SrvDo060_BoneWall | Bone Wall | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 61 | SrvDo061_Confuse | Confuse | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 62 | SrvDo062_BonePrison | Bone Prison | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 63 | SrvDo063_PoisonExplosion | Poison Explosion | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 64 | SrvDo064_Sacrifice | Sacrifice | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 65 | SrvDo065_BasicAura | 17 skills (e.g. Might,Prayer,Resist Fire,Thorns) | §7 gives duration, stat sources, range calc, filter column, mana formula; missing: `aurafilter` bit meanings, target iteration order (rooms / units), stat-list fields (flags, owner, expiry, callbacks), behaviour on failed mana (`0x0056C110` result), return value |
| srvdo 66 | SrvDo066_HolyFire_HolyShock_Sanctuary_Conviction | Holy Fire,Holy Shock,Sanctuary,Conviction | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 67 | SrvDo067_Charge | Charge,SerpentCharge | body; only the re-request rule (`0x0057EEC0`) is specified (`use.md` §4) |
| srvdo 68 | SrvDo068_BasicShout | Shout,Battle Cry,Battle Orders,War Cry,Battle Command | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 69 | SrvDo069_FindPotion | Find Potion | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 70 | SrvDo070_DoubleSwing | Double Swing | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 71 | SrvDo071_Taunt | Taunt | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 72 | SrvDo072_FindItem | Find Item | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 73 | SrvDo073_BlessedHammer | Blessed Hammer | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 74 | SrvDo074_DoubleThrow | Double Throw | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 75 | SrvDo075_GrimWard | Grim Ward | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 76 | SrvDo076_Whirlwind | Whirlwind | body; only the re-request rule (`0x0057EEC0`) is specified (`use.md` §4) |
| srvdo 77 | SrvDo077_Leap | Leap | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 78 | SrvDo078_LeapAttack | Leap Attack | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 79 | SrvDo079_Conversion | Conversion | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 80 | SrvDo080_FistOfTheHeavens | Fist of the Heavens | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 81 | SrvDo081_HolyFreeze | Holy Freeze | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 82 | SrvDo082_Redemption | Redemption | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 83 | SrvDo083_FireHit | Fire Hit | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 84 | SrvDo084_MaggotEgg | MaggotEgg | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 85 | SrvDo085_UnholyBolt_ShamanFire | UnHolyBolt,ShamanFire | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 86 | SrvDo086_MaggotDown | MagottDown | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 87 | SrvDo087_MaggotLay | MagottLay | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 88 | SrvDo088_AndrialSpray | AndrialSpray | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 89 | SrvDo089_Jump | Jump | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 90 | SrvDo090_SwarmMove | Swarm Move | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 91 | SrvDo091_Nest_EvilHutSpawner | Nest,EvilHutSpawner | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 92 | SrvDo092_QuickStrike | Quick Strike | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 93 | SrvDo093_GargoyleTrap | GargoyleTrap | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 94 | SrvDo094_Submerge | Submerge | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 95 | SrvDo095_MonInferno | FetishInferno,mon inferno sentry,Baal Inferno,MegademonInferno,Horror Arctic Blast | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 96 | SrvDo096_ZakarumHeal_Bestow | ZakarumHeal,Bestow | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 97 | SrvDo097_Resurrect | Resurrect,Resurrect2 | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 98 | SrvDo098_MonTeleport | MonTeleport,Teleport 2,Baal Teleport,Baal Clone Teleport | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 99 | SrvDo099_PrimePoisonNova | PrimePoisonNova | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 100 | SrvDo100_DiabCold | DiabCold | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 101 | SrvDo101_FingerMageSpider | FingerMageSpider | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 102 | SrvDo102_DiabWall | DiabWall | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 103 | SrvDo103_DiabRun | DiabRun | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 104 | SrvDo104_DiabPrison | DiabPrison | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 105 | SrvDo105_DesertTurret | DesertTurret | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 106 | SrvDo106_ArcaneTower | ArcaneTower | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 107 | SrvDo107_Mosquito | Mosquito | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 108 | SrvDo108_RegurgitatorEat | RegurgitatorEat | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 109 | SrvDo109_MonFrenzy | MonFrenzy,BloodLordFrenzy | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 110 | SrvDo110_Hireable_RogueMissile | MissileSkill1,HireableMissile,RogueMissile | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 111 | SrvDo111_FetishAura | FetishAura | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 112 | SrvDo112_MonCurseCast | MonCurseCast | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 113 | SrvDo113_Scroll_Book | Scroll of Identify,Book of Identify,Scroll of Townportal,Book of Townportal | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 114 | SrvDo114_Raven | Raven | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 115 | SrvDo115_Vines | Plague Poppy,Cycle of Life,Vines | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 116 | SrvDo116_Wearwolf_Wearbear | Wearwolf,Wearbear,Delerium Change | body; only the free-while-shapeshifted mana rule is specified (`levels.md`) |
| srvdo 117 | SrvDo117_Firestorm | Firestorm | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 118 | SrvDo118_Twister_Tornado | Twister,Tornado | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 119 | SrvDo119_DruidSummon | Oak Sage,Summon Spirit Wolf,Heart of Wolverine,Summon Fenris,Spirit of Barbs,Summon Grizzly | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 120 | SrvDo120_FeralRage_Maul | Feral Rage,Maul | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 121 | SrvDo121_Rabies | Rabies | body; identity as Rabies unconfirmed (OQ2) |
| srvdo 122 | SrvDo122_Hunger | Hunger | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 123 | SrvDo123_Volcano | Volcano | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 124 | SrvDo124_Armageddon_Hurricane | Armageddon,Hurricane,Diablogeddon | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 125 | SrvDo125_WakeOfDestruction | Wake Of Destruction Sentry | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 126 | SrvDo126_ImpInferno | Imp Inferno | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 127 | SrvDo127_SuckBlood | Suck Blood | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 128 | SrvDo128_CryHelp | Cry Help | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 129 | SrvDo129_ImpTeleport | Imp Teleport | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 130 | SrvDo130_VineAttack | Vine Attack | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 131 | SrvDo131_OverseerWhip | Overseer Whip | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 132 | SrvDo132_ImpFireMissile | Imp Fire Missile | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 133 | SrvDo133_Impregnate | Impregnate | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 134 | SrvDo134_SiegeBeastStomp | Siege Beast Stomp | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 135 | SrvDo135_MinionSpawner | MinionSpawner | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 136 | SrvDo136_DeathMaul | DeathMaul | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 137 | SrvDo137_FenrisRage | fenris rage | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 138 | SrvDo138_Unused | (none) | whole body; no 1.14d data row uses it (lowest priority) |
| srvdo 139 | SrvDo139_BaalColdMissiles | Baal Cold Missiles | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 140 | SrvDo140_BaalTentacle | Baal Tentacle | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 141 | SrvDo141_BaalCorpseExplode | Baal Corpse Explode | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 142 | SrvDo142_Unused | (none) | whole body; no 1.14d data row uses it (lowest priority) |
| srvdo 143 | SrvDo143_FistsOfFire_RoyalStrike_ProgressiveFn | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 144 | SrvDo144_Hydra | Hydra | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 145 | SrvDo145_Unused | (none) | body (type-5 `srvactivefunc` of state hurricane); only the dispatch is specified (§7) |
| srvdo 146 | SrvDo146_Unused | (none) | body (type-5 `srvactivefunc` of state armageddon); only the dispatch is specified (§7) |
| srvdo 147 | SrvDo147_Unused | (none) | body (type-5 `srvactivefunc` of state attached); only the dispatch is specified (§7); also no Ghidra function (OQ1) |
| srvdo 148 | SrvDo148_DoomKnightMissile | DoomKnightMissile | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 149 | SrvDo149_NecromageMissile | NecromageMissile | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 150 | SrvDo150_Smite | Smite | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 151 | SrvDo151_Unused | (none) | whole body (branches, RNG draws, constants, messages, return value) |
| srvdo 152 | SrvDo152_DiabLight | DiabLight | whole body (branches, RNG draws, constants, messages, return value) |

## 5. Seams

Unchanged: `SkillFunctions` (every slot but srvst 18), `QuestWorld::unhandled`
(every row of §3). The parallel sessions' files are untouched; the only
existing test file edited is `skills/use_/tests.rs` (one line, `mod bodies;`,
so the new `tests/bodies.rs` reuses its fake).

## 6. Code map rows

| Path | What |
|---|---|
| `crates/d2-sim/src/skills/use_/bodies.rs` | specified per-skill bodies (`START_BODIES` = [18], `DO_BODIES` = []) |
| `crates/d2-sim/src/skills/use_/tests/bodies.rs` | 4 tests: TSV-note check + perturbation, Attract through the start core |

## 7. Gate

All pass on this branch: `cargo fmt --all -- --check`; `cargo clippy
--workspace --all-targets -- -D warnings`; `cargo test -p d2-sim` (1,236
pass, 5 ignored, 0 failed; 4 new); `cargo run -p depcheck` (OK, 8 crates);
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`
(21 OK); `python3 tools/coverage.py --check` (3,263 claims, 0 errors) and
`--selftest` (ok).
