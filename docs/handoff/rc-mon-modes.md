# rc-mon-modes: hand-back (2026-10-10, branch `claude/rc-mon-modes`)

Base: `claude/specs-staging-7` + `claude/rc-mon-spawn-think` (ff) + `claude/q-fix-rd-act5`.

## Checks

| Set | Before | After |
|---|---|---|
| `gen-su-*,gen-boss-*` (91, orig-cache), EQUAL | 14 / 91 | 14 / 91 |
| same, equal ticks | 7501 | 7513 |
| gen-boss-333 (diabloclone) first divergence | frame 51 | frame 63 (`fr`, next layer) |
| play_act5 `baal_falls_and_the_game_is_finished` | throne AI stops after Decrepify (S3 never ends) | Baal's waves advance; fails later at `kill` of a bloodlord5 (510, GUID 104) whose life never drops |

No other check changed. d2-sim: 4748 tests pass.

## What changed

- Read `0x005A78A0` (both byte switches decoded from Game.exe) and the records `0x006E2260`–`0x006E23AF`:
  with `SplGetModeChart` (+0x1A5) set, Baal 543/544/570/709 S3, Diablo 243/333/705 S3+S4, shadow warrior/master
  417/418 S4 use the A-family record (start `0x005A75C0`, event 1 = mode end `0x005A8030`, schedules);
  trapped soul 403 A1/A2/S1/S2 use own ends `0x005A8330`/`0x005A83E0` (request S1 on itself, then think unless frozen).
  Spec: `sim/units.md` §4.6 "Per-class records" + Provenance. Code: `units/modes.rs` `class_mode_record`,
  `MonsterInfo::mode_chart`, `ai::trapped_soul_end`, dispatch in `wiring/path/monsters.rs`. Test `per_class_mode_records`.
- PC1 item "[q-fix-rd-act5] Per-class monster mode records" marked answered (no Windows run needed). REC-1563 not used.
- Private repo: `re/exports/names.tsv` created (0x005A78A0, 0x005A8030, 0x005A8330, 0x005A83E0).

## Open

- **Hydra** (q-fix-skills-4cls, `sor-hydra` frame 42): not this mechanism (hydra classes have no per-class record). Other cause. S-M.
- **baal_falls**: next blocker is the bloodlord5 kill in the test's `kill()` (40 swings, life stays 1<<8): staging
  (immunity / swing never hits) or damage; owner q-fix-rd-act5 / test. S.
- **Frame-31 groups** (rc-mon-spawn-think), untouched here: Megademon 362/686/687/712 (1.14d draw at `0x005E0EF4`,
  read `0x005E0C80`, S); PutridDefiler 546-550 (has `SplGetModeChart` but no per-class case, so not the records; S-M);
  VileMother 298-300/675/676 + ClawViper (1.14d mode 14, d2rs 1; M).
- gen-boss-333 frame 63: monster `fr` 960 vs 13056 after the S3/S4 start (animation rate / frame of the new record). S.

REC ids used: none (REC-1955..1959 free).
