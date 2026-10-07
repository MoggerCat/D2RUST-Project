# Handoff: missiles + monster AI implementation (`claude/impl-monsters`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Session: cloud implementation (METHODS M14, medium), 2026-10-06. Base:
`claude/bold-ptolemy-jvyvxy` at `4c7a7c7` (PR #9; same content). Inputs:
`specs/missiles/missiles.md` (+ `srvdo.tsv`, `srvhit.tsv`),
`specs/monsters/ai.md` (+ `ai-functions.tsv`). Scope of every claim
below: this branch, synthetic tests only, no game files (M09).

## 1. State

**Implemented, unverified** (M02): both specs are drafts with queued
confirmations; nothing here has been compared with a 1.14d trace.

- `d2_sim::missiles`: creation `0x0059FA30` (§R2.3 steps 1–30), missile
  init `0x0059F8A0`, per-tick class handler `0x005ADBB0` (§R3, incl. the
  1.14d player-in-town rule), default flight `0x005AE1F0` (§R4), collide
  types and the shared unit filter (§R4.2), hit handler `0x005ADF10`
  (§R5), damage fill and rolls `0x005A89A0`/`0x005A8910` (§R6.2), the
  missile-owned result flags of `0x005AD730` incl. the knockback draw
  (§R6.1), pierce test and pierce at a hit (§R8), velocity /
  acceleration arithmetic of §R4.1 (`PathVelocity`, for the path
  provider), the seeded sub-missile helper `0x005A9820` (§R9.3), and the
  three function tables (§R9.1).
- `d2_sim::monsters::ai`: think scheduling (§1: idles `0x005DE080`,
  `0x005DE0F0`, `0x005DE130`; neutral start `0x005A73E0`; knockback end
  `0x005A8520`; `0x00573780`; room entry `0x0053A8E0`; NPC interaction;
  freeze apply; state-54 rule; inline thinks `0x005A8030`; type 10),
  dispatch `0x005B1740` with prechecks A/B/C (§2), AI control, tables,
  lookup `0x005B15D0` and install `0x005B0E00` (§3), aip/aidel/aidist
  (§4), main search `0x005DD7F0` evil path (§5.2), distances (§6),
  tactics helpers with their draws (§7), commands (§8), and the 17
  `spec'd-here` AI functions of §9 (None, Idle, Buffy, Skeleton, Zombie,
  Fallen, Brute, Wraith, Goatman, Swarm, FallenShaman, QuillRat,
  Andariel, CorruptArcher, CorruptLancer, Navi, TownRogue).
- Tests: 76 new (`missiles::tests` 42, `monsters::ai::tests` 34): every
  synthetic test vector of both specs, the edge cases, the recorded full
  lifetimes (Range runs), and two catalogue checks with perturbation
  tests (M05, M08).
- Gate run: `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
  --all-targets -- -D warnings`, `cargo test -p d2-sim` (131 pass),
  `cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
  `python3 tools/methods.py check`: all clean.

Nothing outside `crates/d2-sim/src/{missiles,monsters}/` changed except
the two `pub mod` lines in `crates/d2-sim/src/lib.rs`.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/missiles/mod.rs` | `MissileData`, `MissileStore`, `Ctx`, collide-type table, missile init, class handler, `MissileDispatch` (`EventDispatch`) | `missiles/missiles.md` §R1, §R3, §R4.2 |
| `crates/d2-sim/src/missiles/{create,flight,hit}.rs` | creation + pierce test; default flight + `PathVelocity`; hit handler, damage fill/rolls, result flags | `missiles.md` §R2, §R4, §R5–§R8 |
| `crates/d2-sim/src/missiles/catalogue.rs` | server-do/server-hit tables (copied from the TSVs, test-checked), dispatch, stubs, `0x005A9820` helper | `missiles.md` §R9, `srvdo.tsv`, `srvhit.tsv` |
| `crates/d2-sim/src/missiles/seams.rs` | `MissileUnits`, `MissilePath`, `MissileRooms`, `MissileCombat`, `MissileHooks` (= `MissileWorld`) | |
| `crates/d2-sim/src/monsters/ai/mod.rs` | `AiControl`, `AiStore`, `Ctx`, aip/aidel/aidist, scheduling helpers, install, `think`, `MonsterDispatch` (`EventDispatch`) | `monsters/ai.md` §1–§4 |
| `crates/d2-sim/src/monsters/ai/{target,tactics,functions}.rs` | prechecks + main search; distances, velocity, movement, commands; per-AI functions by address | `ai.md` §2, §5–§8, `ai-bodies.md` §9 |
| `crates/d2-sim/src/monsters/ai/table.rs` | `AI_TABLE` (copied from `ai-functions.tsv`, test-checked), `SPECIAL_TABLE` | `ai.md` §3.2, §10 |
| `crates/d2-sim/src/monsters/ai/seams.rs` | `AiUnits`, `AiModes`, `AiWorld`, `AiTargets`, `AiSkills` (= `AiHost`) | |

## 3. Seams and expected providers

Wiring: the tick integrator builds `MonsterDispatch { cx, next:
&mut MissileDispatch { cx, next: &mut <rest> } }` (or calls
`missiles::class_handler` / `monsters::ai::handle_event` from its own
`EventDispatch`). Missile-class events without a callback run the
missile handler; monster-class type 2 / 10 without a callback run the
AI; everything else passes to `next`. Tables come in as `d2_data`
typed records: `&[Missiles]`; `AiTables { monstats, monstats2, levels,
skill_modes }` where `skill_modes` is `monsters::ai::skill_modes(&BinTable)`
(Sk1mode..Sk3mode at record +0x180, a callback column absent from the
typed `Monstats`). `GameInfo` carries game +0x6A, +0x74, +0x6D (not yet
in `Game`).

| Seam | Provider | Covers |
|---|---|---|
| `missiles::MissileUnits` | units/stats session | unit allocation `0x00555230` (seed derive + GUID + lists), removal `0x00555600`, unit seed, position, size, stats/base stats, states and state stat lists (87/161), unit flags +0xC4, hostility `0x00554200`, alignment, hireling test, `justhit` stat list + type-12 timer |
| `missiles::MissilePath` | units session (path) | path set-up primitives, unit step `0x00554CA0`, collision word `0x00648EB0`, crossed subtiles `0x00648F40`, target distance |
| `missiles::MissileRooms` | DRLG session | room search `0x00463740`, town test, collision masks `0x0064D9B0`/`0x0064CB30`, clear footprint `0x0064EBA0`, unit search order of `0x00641CB0` |
| `missiles::MissileCombat` | combat/skills session | damage setup `0x0059F900`, to-hit `0x0057D9B0`, server-damage functions, damage application (block/dodge … execution), unit event 0, demon/undead/monster-type bonus, target armor |
| `missiles::MissileHooks` | skills session; `monsters/init.md` | init callbacks; unique-mod hook `0x005A43B0` |
| `monsters::ai::AiUnits` | units/stats session, `monsters/init.md` | seed, class, anim mode, states, state-54 clear, dead, position/size/act/level, life, `dwAiState`, alignment, unique/champion/boss, vision record, type-10 reset, interaction/busy |
| `monsters::ai::AiModes` | units session | mode changes (success/failure), anim mode, path steps/blocked/stop, current skill + flag 0x40, `0x0046C140`, sounds, knockback→gethit, walk-in-radius geometry, doors |
| `monsters::ai::AiWorld` | DRLG / units | town, LOS draw, collision at the unit, line and melee tests, direct-reach test `0x005DC640`, teleport spot `0x0054DC40`, room last-dead GUIDs |
| `monsters::ai::AiTargets` | units (target nodes), skills (forced targets) | target-node lists, forced targets, the non-evil search, `0x005DD510`, `0x005DDC30`, `0x005DDF20`, door / special-walk / shaman corpse scans |
| `monsters::ai::AiSkills` | combat/skills session | `0x005FD470` skill usable |

State lifetime: `MissileStore` drops a missile's data when the missile
handler removes it; `AiStore` entries must be removed by the unit
removal code (`AiStore::remove`); missiles removed by other code
(room freeing) leave stale `MissileStore` entries until the units
session calls into it. AI control allocation and the first `install`
at monster creation are `monsters/init.md`'s (tests insert
`AiControl::default()` and call `install`).

## 4. Stubs (keyed by the TSVs)

- Server-do: only 1 has a body; every other non-null index (2, 3, 5–37)
  logs `missiles::Unhandled::SrvDo` and returns 1 (missile kept, no
  movement, no countdown). Null entries 4, 38–52 log `NullSrvDo`.
- Server-hit: every non-null index (1–29, 31–33, 35–40, 43–45, 47–48,
  50–59) logs `Unhandled::SrvHit` and returns the handler's `c`
  unchanged; null entries log `NullSrvHit`. Server-damage 1–14 go to the
  combat seam; 15–30 log `NullSrvDmg`.
- AI: every think address not in `functions::IMPLEMENTED` (the 131 rows
  other than `spec'd-here`, including Npc 32 `summarized`), all
  special-state thinks other than Idle, all init and alternate functions
  log `monsters::ai::Unhandled::Function { addr }` and do nothing
  (schedule nothing).
- Checks: `missiles::tests::catalogues_match_tsv` (+ perturbation),
  `monsters::ai::tests::{ai_table_matches_tsv,
  implemented_matches_catalogue}` (+ perturbation).

## 5. Open questions (code TODOs; narrowest reading taken)

Missiles:

1. Negative pierce P (§R8.1): compared signed (never pierces).
2. Collide types ≥ 9: treated as no callback, mask 0 (no live row).
3. Mastery stats of §R6.2: read from the missile (the spec names no
   unit).
4. `phys += phys × pct / 100` (§R6.2): 32-bit wrapping arithmetic.
5. §R6.1 armor: "armor −= stat 120 … clamped ≥ 0 after adding" read as
   `add_target_ac(stat 120)` (provider adds and clamps); sign to confirm.
6. Damage application without an owner (`0x005AD730` "no owner →
   nothing"): result flags 0, `apply_damage` still called with
   `owner: None` (the skills spec decides).
7. Result-flag bit values (`hit::result_flag`) are d2rs-local names.
8. Hit handler with no record (§R4 step 1 expiry): returns 1.
9. §R2.3 step 8 target-on-owner test: D2MOO order (spec open question 6).
10. §R2.3 step 11 animation frame (unit +0x44): not stored (units spec).
11. Stubbed server-do functions keep the missile forever (no countdown);
    a sim using them will accumulate missiles until their bodies land.

AI:

12. Forced target (§5.1/§5.2 step 3): returned with the melee test only;
    flags / vision not updated (ai.md open question 6).
13. Slot-9 alternative chosen: reported distance = its own no-size
    distance.
14. `0x005DE9D0` "same collision test": collision only, no can-walk test.
15. `SplEndGeneric` inline thinks (§1.4): anim mode not set to neutral.
16. Mode/move requests with target 0 (T = 0, CorruptArcher edge case 7)
    pass `ModeTarget::Point(0, 0)`; escape from target 0 uses the own
    position.
17. Navi "clamp param 1 at 0 and count it down": `max(p, 0)`, then −1
    while > 0.
18. Velocity "method": the spec names method 13 for CorruptLancer step 1
    and `0x005DED40`; every other "speed N" request passes method 0 (no
    overwrite). FallenShaman / CorruptArcher circles pass `del = false`.
19. FallenShaman step 5/6: a failed P(aip2) falls through to the next
    step.
20. Velocity assert (speed outside −126…126): logged, request ignored
    (fatal in 1.14d).
21. A think on a monster without AI control: logged, nothing runs.
22. Command list: a linear list (`SetCurrentAiCommand`'s search wrap is
    not implemented; no implemented AI uses it).

## 6. Checks to queue (`docs/HANDOFF.md` §5)

1. Missile lifetimes replay: drive `MissileDispatch` from the tick
   traces (`traces/sim/tick/`, recording `20261006-022304`) with a path
   fake and compare per-GUID run counts with missiles.md Test vectors
   (69/69). Expect: full lives = `Range` runs; early removals need
   missiles.md open question 1 (exit-path recording).
2. Think intervals replay: from the same recordings, the per-class
   type-2 delay histogram of ai.md Test vectors (zombie1 25 ×502 …)
   with an `AiHost` fake fed the recorded positions; needs the
   target-node lists (ai.md open question 7).
3. Once `MissileUnits`/`AiUnits` exist: an end-to-end RNG draw-order
   trace (missile damage rolls on the missile seed, to-hit on the owner
   seed, AI draws on the monster seed) against `rng` traces with
   positions.
4. Every spec open question listed in missiles.md (1–11) and ai.md
   (1–11) stays open; items 1–22 above add to them.
