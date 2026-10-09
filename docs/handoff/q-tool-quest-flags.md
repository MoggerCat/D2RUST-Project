# q-tool-quest-flags (REC-1625)

The 1.14d state recorder now reads the quest flag field `q`; `state_diff.py` compares it.

- `tools/trace-recorder/record_state.py`: type-0 units get `q` = `[slot, word]` for each non-zero u16 of the quest
  record of the game difficulty (unit +0x14 player data, +0x10 + 4·d → 42 u16 words; d = u8 game +0x6D).
  Address and layout from `specs/world/quests.md` §1.4 and `specs/tools/state-snapshot.md` §2 (no new RE).
  `[]` when all words are 0; absent when a link is 0 or d > 2. Assumption: `0x006221A0(player)` = unit +0x14 (the
  player data pointer); confirmed by the live read below (real quest words come out).
- `state_diff.py`: `q` in `FIELDS` (compared; no longer "not compared"); selftest covers a changed word, an added
  word, an emptied record, and the perturbation of `q` for every unit.
- Selftest by perturbation (M08): every source byte of `q` (data pointer, difficulty byte, the 3 record pointers' used
  slot, all 84 record bytes), flipped alone, changes exactly `q` (`record_state.py --selftest`, 672 source bytes).
- Spec: `specs/tools/state-snapshot.md` §2 `q` row updated (both sides write it).

## Verified on 1.14d under Wine
`traces/checks/join-act2-quests-ama.check` (ScnAmb, `--quests acts=1`), state channel, 40 ticks:
1.14d records `q=[[0,24704],[1,801],[2,768],[16,24832],[17,801],[18,768],[32,24960],[33,801],[34,768]]`
(act 1 done, all three slots groups: 0–5 act 1, 16–21 and 32–37 per the save's quest block) on every frame.
d2rs state-dump of the same save gives `q=[[7,1]]` only. All other player fields (x, y, hp, seed, lv, ...) equal.
Result: `q` DIVERGED at frame 2, a real divergence: d2rs does not carry the save's quest records into the host's
per-player quests on load (only slot 7 bit 0 is set). Owner: the d2s load → `AppRest::quests` path
(`formats/d2s-load.md` §2, `world/quests.md` §1.4). Not fixed here (out of area).

## Not done
- No per-quest runtime checks (kill/complete sequences): this only adds the tool channel; q-chk-act5 can now author them.
