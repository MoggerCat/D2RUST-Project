# Handoff: hirelings (`specs/world/hirelings.md`) — `claude/impl-hirelings`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the ninth fold (`claude/fold-handoff-night`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Ninth set" (PC 1 / PC 2).

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14); three parallel agents on independent
files (level / XP, pets + life, item swap), the coordinator on rows, the
seam and the wiring. Base: `claude/specs-staging` at `5844674`. Repo only,
no game files (M09). Inputs: `specs/world/hirelings.md`, `specs/sim/pets.md`
§5–§8, `specs/world/npc.md` §7, `specs/items/inventory.md` §7.23,
`specs/combat/vitals.md` §4, `specs/sim/server-messages.tsv`.

## 1. Result (unverified, M02)

| Spec part | Where | Tests (`world::hirelings::tests::…`, all with `// Covers:`) |
|---|---|---|
| §1 rows, §1.2 lookups, §2 offer, threshold, §9 r1 cost | `world/hirelings/rows.rs` | `rows::*` (7): bracket lookup, candidates, act of name, the npc.md seed vector (Id 1, L 6, price 217), shift / clamps, cost and threshold vectors |
| §5 pet list, §13 r1–3 messages, join sync | `world/hirelings/pets.rs` | `pets::*` (12) |
| §3.2 init / replace, §6 follow + classic act change, §8 death, §9 r3–9 revive, §10 r1–4, r7 restore steps | `world/hirelings/life.rs` | `life::*` (10): death vector `9b 21 0f 5e 1a 00 00`, replace order, new hire L 6 / exp 26460, revive order, follow, classic act change, restore |
| §4 level stats, §7 experience, §10 r5–6, §13 r4–6 messages | `world/hirelings/level.rs` | `level::*` (18): cap 229, `a2 0d … ca 01`, `a1 0d … 98`, `a0` old-value, 0x9E/0x9F/0xA0, edge cases 3, 4, 6 |
| §11 item swap | `world/hirelings/items.rs` (seam `HirelingItems`) | `items::*` (6) |
| Wiring | `wiring/interaction/hirelings.rs` (`HireView`: `HirelingWorld` on the desk), `NpcRest: HirelingRest` | `wiring::interaction::tests::{npc::hire_at_asheara_…, npc::resurrect_revives_…, quest_npc::kashya_reward_…}` now assert the pet node, flags, level and 0x81 instead of a seam log |

`cargo test -p d2-sim`: all green except the known-red tests of other
sessions (`monsters::ai::tests::specd_here_*`, `skills::use_::tests::*`,
`skills::mutant_tests::table_check_mutants::*`, `missiles::tests_bodies::*`);
`cargo test -p d2-server` green; clippy `-D warnings`, fmt,
`coverage.py --check`, `spec_index.py --check` clean.

## 2. Seams and public API

- `world::hirelings::HirelingWorld`: everything outside the spec (units,
  stats, states, skills, owner, team, AI hook, unit free, room removal,
  dismiss, warp, level events, item re-apply, players, send).
- `wiring::interaction::HirelingRest` (supertrait of `NpcRest`): the calls
  without a d2-sim provider (mode set — moved here from `NpcRest`, state
  stat, skill table and set, owner, team, AI, free, room removal, death
  event, dismiss, warp, level events, item stats). `NpcRest` lost
  `init_mercenary` / `revive_mercenary` (now real). Every `NpcRest` fake
  (d2-sim, d2-server, d2-client e2e) got an `impl HirelingRest` of no-ops.
- `InteractionState::{hirelings, hireling_tables}`. No tables → the
  mercenary calls push `InteractionError::NoHirelingTables` and do nothing.
  `HirelingTables::from_tables(hireling, pettype, max_level)` builds them
  from the fixed-up tables; no production loader sets them yet (the
  server's game creation does not build `NpcControl` from game data either).
- `NpcWorld::pet(player, 7, any)` reads the hireling list; other kinds stay
  on the rest (`sim/pets.md`, `player::pets`, parallel session).

## 3. Not wired (no caller yet)

1. §8 death: `life::death` needs the kill path (`0x0057CCB0`, action
   wiring `Pending::reaction`) to reach `InteractionState`.
2. §6 follow on teleport (`path-placement.md` rule 6) and act change:
   `life::follow`, `life::classic_act_change`.
3. §7.1 kill share: `level::kill_share` (needs ExpRatio, HL1).
4. §11 swap: `MoveWorld::equip_on_merc(merc, item)` (`items/moves/seams.rs`)
   has no player argument and no result; `items::swap` needs both. Change
   the seam, then implement `HirelingItems` over `InvDesk`.
5. §10 restore: needs the save spec (hirelings.md OQ1).
6. C→S 0x46 / 0x47 stay `NoOwner` (AI spec).

## 4. Questions (HL1–HL9)

- HL1 §7.2 r3: the `ExpRatio` step (`0x0057E390`, "shift from the MaxLvl
  row") has no formula in hirelings.md or vitals.md §4.3; `level::gain`
  takes it as a caller step.
- HL2 0x7A layout conflict: `pets.md` §8 and its vector put owner @5, pet
  @9; `hirelings.md` §13 r2 and the confirmed `server-messages.tsv` row put
  pet @5, owner @9. Code follows the TSV; `pets.md` needs fixing (or the
  TSV, if a recording says otherwise).
- HL3 §5 r3 full-list eviction: hirelings.md says "removed with kill (rule
  5)" (two broadcasts), pets.md §5 r2 says unlink (one); code follows
  pets.md.
- HL4 §5 r3 max recompute `0x00575900` (pets.md OQ2): max 0 → `basemax`.
- HL5 §3.2 r6: whether the init stops when the add fails; code continues.
- HL6 §7.1 r2: "defender level" base or total; read as base.
- HL7 §11 r4: where the copy of the old item goes relative to rule 3's
  "cursor := none"; code: after it, before the refresh calls. Also which
  unit `0x0055DF00`, `0x0055F4F0`, `0x00540E60`, `0x005417D0` act on.
- HL8 §13 r4 / OQ7: stats sent to the owner only, at once (not queued).
- HL9 §1.2 r3: the fallback `0x00656390` is not described; range test only.

## 5. Local run queue (HANDOFF §5 C90; recordings S9-A1 (6))

- Game-file test of the Test-vector table (`#[ignore]`, `D2_GAME_DIR`):
  `HirelingTables::from_tables` on the live `hireling` / `pettype`, then
  `offer` and `level::apply_level` on each row of the table; every offer
  and unit column must equal the spec's. Not written yet (cloud has no
  tables to check the decode against).
- Recordings of hirelings.md OQ9 (hire, level-up, death, resurrect, give /
  take) settle HL2, HL3, HL7, HL8.
