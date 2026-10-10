# rc-missile-missing hand-back

Branch `claude/rc-missile-missing` (specs-staging-7 + integ-r18, re-synced to specs-staging-7
at integ-r20). Cause queue rows 3 ("missile exists in 1.14d, missing in d2rs", 191 checks on
r10) and 15 ("missiles live one tick longer", 17 checks).

## Checks (orig-cache; EQUAL = every channel MATCH)

Sample on r18, `--filter 'gen-missile-*,gen-ai-*,gen-lvl-*'` (547 checks, 1213 runs):

| | EQUAL | no channel DIVERGED | runs MATCH / PARTIAL / DIVERGED | state: missile on one side only |
|---|---|---|---|---|
| r18 before | 0 | 238 | 389 / 503 / 321 | 2 (gen-ai-diablo, gen-ai-megademon) |
| r18 + fix | 0 | 240 | 389 / 505 / 319 | 0 |

Re-synced base (integ-r20 already has the setter half, from rc-extra-missile), `gen-ai-*`
(146 checks): DIVERGED 27 / PARTIAL 119 both before and after; ticks equal 19824 -> 19903.
gen-ai-deathmauler goes from f46 "missile 502 present in 1.14d, missing in d2rs" to f125
(game seed). No regressions in either run.

Most of row 3 was already fixed by r18: gen-missile-10 to 28, gen-ai-clawviperex, gen-lvl-98,
combat-arrow-kill and combat-champion-pack all have state equal on every frame. EQUAL stays 0
because every gen-missile check diverges on the S->C 0xAA AddUnit size (12 vs 39, rc-queue
row 2, 265 checks).

## Cause and fix

- The skill-use world's `BodyWorld::set_missile_frames` / `missile_frames` went to host hooks
  (no-op / 0), so the Inferno frames helper `0x005CC2E0` (`skills/bodies-3.md` §3.2) left
  missiles at `Range` instead of `Param2 + L − 1` (diablight 30 vs 32, megademoninferno an
  extra missile). Upstream (`ae32018f`, rc-extra-missile) fixed the setter in parallel, and
  the merge kept its version.
- Left for this branch: the getter `0x0064A300` (+0x0E total frames, 0 without missile data)
  now reads the real missile store (`wiring/interaction/skill_use.rs`). Without it DeathMaul
  (srvdo 136 `0x005D25B0`) read 0 and wrote 0 frames to its missile.
- Test `inferno_frames_reach_the_missile_store` (set, read back, ±0x7FFF clamp). No spec change
  (bodies-2.md / missiles/bodies.md already name `0x0064A300`). Ledger part: 5 rows.

## Open

- gen-ai-andariel f128 (class 32) and gen-ai-highpriest f85 (class 247): missile path target
  x is 6 sub-tiles off (1.14d 5139 vs d2rs 5145). Size M; probably one aim-point cause.
- gen-ai-deathmauler f125 game seed, and 9 RNG draw-count rows (gen-lvl/gen-missile), belong
  to the rc-queue "game seed" row.
- Owners: `wiring/interaction/skill_use.rs` has no owner in owners.tsv; its spec area
  `specs/skills/` belongs to q-fix-skills-bda.
- Gen families outside the sample (obj, mon, state, ...) were not re-run.
