# Hand-back: q-chk-hirelings (`claude/q-chk-hirelings`)

CHECK session for the hirelings of Acts I, II, III and V. REC-1515..1519: none
needed (no unsettled choice). Ran under Wine with `tools/scenario-diff`.

## Done

15 new checks in `traces/checks/` (all `state` channel). All run on 1.14d (Wine)
and d2rs; every verdict is DIVERGED, and every first divergence is the hire
itself, so later features are masked behind it.

| Check | What | First divergence |
|---|---|---|
| `hire-kashya`, `hire-greiz`, `hire-asheara`, `hire-qual-kehk` | hire through the NPC flow (0x13, 0x2F, 0x36) in Acts I, II, III, V, then 60 frames of follow AI | frame 14 (A3: 20) game `seed` |
| `hire-resurrect-*` (4) | hire, hireling life poked to 0, back to the seller, C->S 0x62 | same, resurrect not reached |
| `hire-follow-warp-kashya`, `hire-follow-waypoint-kashya` | follow through a level warp / a waypoint (0x49) | same |
| `hire-items-kashya` | cap taken to the cursor (0x19), given and taken back (0x61) | same |
| `merc-levelup-a1/a2/a3/a5` | saved hireling one point below level 12, one cow kill | frame 4, spawn `tx` (D4, q-scenes-compare) |

The Act III hireling now has a check (`hire-asheara`, `merc-levelup-a3`;
`merc-sorc-cow` already existed).

## The divergence (routed)

Hire (0x36, `world/npc.md` §7.3 steps 5-8, `world/hirelings.md` §3):
- the game seed after the hire: 1.14d advances it, d2rs does not (kashya
  frame 14); the hireling unit's own seed after creation is equal;
- the spawn tile differs (kashya (4892,4223) vs (4893,4228), greiz
  (5026,5049) vs (5028,5048), qual-kehk (5060,5084) vs (5060,5083));
- greiz frame 15: hireling unit seed differs (follow AI draws).

Routed: `docs/handoff/build-queue.tsv` row `q-fix-hire-create` (owner by area
`claude/q-diff-skills-2`; placement part `q-scenes-compare`). Ledger rows in
`docs/handoff/ledger/q-chk-hirelings.tsv` (13 rows, validated with `ledger.py`),
verdicts added to `docs/handoff/checks-status.md`.

## Authoring findings (fixed in the checks)

- Name ids of `0x36` are string ids: Act I 3411.., II 1019.., III 1040.. from
  `string.tbl`; Act V is `patchstring.tbl` (10850 = MercX101), not
  `expansionstring` (+20000): 22751 was refused with 0x2A code 9 on both sides.
- Act III: `poke goto` takes until frame 11, so the NPC flow starts at 14.
- Act V needs quest slot 36 bit 0 (`--quests 36.0`) and `--act 4`.

## Open

- Re-run all 15 after `q-fix-hire-create`; the masked steps (0x62, 0x49 follow,
  0x19/0x61, level-up message 0x27/0x9E) then show their own first divergence.
- `merc-levelup-*` assume experience.txt level 12 = 112725; check the level the
  save gives (not read out of the snapshot).
- No `packets` channel check for 0x36/0x62/0x61 yet (state only; the 0x2A refusal
  packets were equal).
- `poke goto unit` + `send` replace `poke msg 0x13` (adopted; no generator used,
  q-tool-check-gen not landed).

## Repro

```
export D2_GAME_DIR=/root/game CARGO_INCREMENTAL=0
sh tools/coord/session-setup.sh        # once
python3 tools/scenario-diff/scenario_diff.py traces/checks/hire-kashya.check
```
