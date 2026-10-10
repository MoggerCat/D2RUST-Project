# rc-missile-missing hand-back

Branch `claude/rc-missile-missing` (specs-staging-7 + integ-r18). Cause queue rows 3
("missile exists in 1.14d, missing in d2rs", 191 checks on r10) and 15 ("missiles live
one tick longer", 17 checks).

## Checks (`--filter 'gen-missile-*,gen-ai-*,gen-lvl-*'`, 547 checks, 1213 runs, orig-cache)

| | EQUAL (every channel MATCH) | no channel DIVERGED | runs MATCH / PARTIAL / DIVERGED | state "missile present on one side only" |
|---|---|---|---|---|
| r18 before | 0 | 238 | 389 / 503 / 321 | 2 (gen-ai-diablo, gen-ai-megademon) |
| after | 0 | 240 | 389 / 505 / 319 | 0 |

Most of row 3 was already fixed upstream by r18: gen-missile-10 to 28, gen-ai-clawviperex,
gen-lvl-98, combat-arrow-kill and combat-champion-pack all have state equal on every frame.
EQUAL stays 0 because every gen-missile check still diverges on the S->C 0xAA AddUnit size
(12 vs 39, rc-queue row 2, 265 checks), not this cause.

## Cause and fix

- In the skill-use world, `BodyWorld::set_missile_frames` / `missile_frames` went to host
  hooks whose defaults do nothing / return 0, so the Inferno frames helper `0x005CC2E0`
  (`skills/bodies-3.md` §3.2) never reached the missile. Missiles kept `Range` instead
  of `Param2 + L − 1`: diablight 30 vs 32 frames (srvdo 152 DiabLight `0x005CC690`),
  megademoninferno 30 vs about 9 (srvdo 95 `0x005CC4E0`, an extra d2rs missile).
- `crates/d2-sim/src/wiring/interaction/skill_use.rs`: both now act on the real missile
  store: setters `0x0064A2B0` (+0x0E total) and `0x0064A330` (+0x10 current), clamped to
  ±0x7FFF; getter `0x0064A300`. The host hook is used only while the store is lent out.
  This also reaches Imp Inferno (srvdo 126) and DeathMaul (srvdo 136, getter);
  gen-ai-deathmauler's first divergence moved from f62 to f125 (game seed).
- Test `inferno_frames_reach_the_missile_store` (`wiring/interaction/tests/skill_use.rs`).
  The spec already described the behaviour; no spec change.
- Ledger part `ledger/rc-missile-missing.tsv` (5 rows).

## Open

- gen-ai-andariel f128 (class 32) and gen-ai-highpriest f85 (class 247): missile path
  target x is 6 sub-tiles off (1.14d 5139 vs d2rs 5145). Size M; probably one shared
  aim-point cause in the monster-AI missile skills.
- gen-ai-deathmauler f125 game seed, and 9 RNG draw-count rows in gen-lvl/gen-missile
  (rc-queue "game seed" row), not this cause.
- Owners: `wiring/interaction/skill_use.rs` has no owner in owners.tsv; its spec area
  `specs/skills/` belongs to q-fix-skills-bda.
- The gen families outside this sample (obj, mon, state, ...) were not re-run.
