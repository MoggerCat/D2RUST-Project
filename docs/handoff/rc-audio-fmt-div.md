# rc-audio-fmt-div hand-back

Branch `claude/rc-audio-fmt-div`. REC-3000 (footsteps), no other REC ids used.

## Checks (EQUAL / checked)
- EQUAL 0 -> 0 for my rows; checked: wav 4 and font-tbl 2 rows NO-CHECK -> DIVERGED
  (linked to `gen-aud-town-idle` and `gen-ui-char`), d2s.10-errors and d2s-load.5
  NO-CHECK -> NO-CHECK with 19 MATCH checks `gen-fmt-refuse-*` (PARTIAL coverage).

## What changed
- Footsteps: a monster's first update in a new mode makes no footstep call (REC-3000).
- Wine audio is not a reference for start-up ticks or seed-dependent picks (rain T4 vs
  T3 on Windows; variants differ from T35). `tools/audio-diff/digest_compare.py`
  compares a Windows voice list (digests, no blobs) with a d2rs dump; PC 1 item in
  `pc1-data.md` Step 4 asks for the Windows voice lists.
- Refused loads: `Character::Refused` (d2-client), `session_flow` announces the player
  (0x59/0xAA/0x76, then 0x94 after the skills) before the 0xB4 for section errors,
  `d2s::skills_before_failure`, state-dump ends the game at tick 2, `packets_diff`
  compares S->C 0xB4 (was excluded as transport). `d2s-tool` flags `--allow-locked`,
  `--break KIND` (checksum size version magic class newflag dead short quests waypoints
  npcs stats skills items corpse hireling golem). `check_gen` REFUSE_CASES -> 19 checks,
  all MATCH in the suite (`d2s-load.md` section 5 r2a, r2b).

## Open (sizes)
- Walker NPC pause on town-ambience (38-51 in 1.14d): rc-draw-row173 (S).
- Windows voice lists for the other audio checks (PC 1) then `digest_compare` (M).
- d2s refused-load codes 3, 8, 9, 11, 12, 25, 26 and the message text: not reachable in
  single player (mode follows the character) / frontend channel (M).
- d2s-load.3/4/6 (golem re-summon, hotkey indices, runeword mismatch): `d2s-tool` needs
  a golem item in kf, hotkey item indices, a mismatching runeword item (M).
- `app::state_dump::tests::options_parse` fails on the base too (not from this branch).
- Full `check_gen.py` run deletes the netc2s checks (needs inputs); I restored them and
  edited INDEX.tsv by hand.
