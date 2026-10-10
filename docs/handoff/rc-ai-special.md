# rc-ai-special hand-back (REC-1925..1929 unused)

Two monster-AI gaps from `docs/handoff/q-run-gen-wp-shrine.md`. Checks run with
`suite.py --filter ... --orig-cache traces/orig-cache`, 1.14d recorded fresh
(the cache held neither check).

## Checks before -> after (state ticks equal of 460 / 160, rng of 145)

| Check | Before | After |
|---|---|---|
| gen-wp-18 (class 359) | 36 equal, first diff frame 37 (monster 1:6 seed) | 402 equal, first diff frame 153 (class 245 `tx`) |
| gen-wp-19..26 | frame 37 | 341 equal, frame 153 (same cause) |
| gen-lvl-132 state | 96 equal, frame 97 game seed | 143 equal, frame 144 (Baal `tx`) |
| gen-lvl-132 rng | 140/145, site `0x552e31` missing | 145/145 MATCH |

`gen-wp-*` (39 checks) re-run after the change: no check got worse; 1-17, 28-38
reach frame 400 (game seed, the separate wp cause), 0/9/27/30 PARTIAL.
EQUAL verdicts: none gained (every check still has a later divergence).

## What changed (all d2-sim, specs in our words with addresses)

- SpecialState06 `0x005E7C10` (new, `ai-bodies.md` §9.33): GoodNpcRanged with
  the attack choice from the main search (walk flags 7 / A1).
- Baal's S3 (mode 10): per-class mode record `0x005A78A0` (monstats
  `SplGetModeChart`) -> attack-family record, schedules (`units.md` §4.6).
- Frame advance `0x00623E00` sequence-less branch (`anim::advance_frame`),
  used by every monster event 0 refresh; the provisional "+0x4E := timer arg"
  (REC-701) is gone; 5 tests adapted to feed the AnimData bytes.
- Generic mode end `0x005A8030`: neutral request targets the path target unit
  or the point (0, 0); state-54 / dead guard (`ai.md` §1.4).
- Tentacles: `spawn_monster(At)` through the lent monster world (placement
  draws), source-unit link stored (+0x94/+0x98, owner via `0x00552FD0`),
  `WaitThink` effect = `0x005DE0F0`.
- Tests: SpecialState06 x3, frame advance x3; fmt, clippy, d2-sim 4743 and
  d2-server 397 tests, coverage/spec_index/ledger checks pass.

## Open

- `gen-lvl-132` frame 144: Baal walk target `tx` 15146 vs 15148 (M).
- `gen-wp-18..26` frame 153: class 245 `tx` 5150 vs 5149 (M, path target).
- Mode end does not clear the used-skill entry (builder `0x00620210(U,0)`):
  host seam `set_current_skill` takes a skill id (0 = Attack), needs a "none"
  seam in `LocalSeams` (d2-client `monster_ai.rs`) (S).
- Per-class records for class 403 (modes 4, 5, 8, 9; event-1 `0x005A8330`,
  `0x005A83E0`) not read nor implemented (S).
- `re/exports/names.tsv` does not exist in the private repo, so no names were
  appended (roles: `0x005E7C10` SpecialState06, `0x005A78A0` mode record
  selector, `0x00623E00` frame advance, `0x00553540` path target unit).
- d2-client not built (Bevy); its tests only gained `mode_chart: false`.
