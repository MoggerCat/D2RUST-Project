# Hand-back: q-tool-autostart-difficulty (2026-10-09)

Branch `claude/q-tool-autostart-difficulty`, REC-1675 (the one provisional choice: polling, below).
Tools only; no game code.

## Done

1. **autostart.py 0.4.0**: `--difficulty normal|nightmare|hell` (every recorder via `add_options`, and `--try`).
   Once the client entry `0x0044B8A0` has stored the config pointer (`[0x007A0438]`, before the tables
   load and 0x67 is built) `AutoStart.poll` writes config `+0x210` := difficulty (original-hooks.md §5.4
   rule 3). PROVISIONAL REC-1675: done by polling the pointer instead of a breakpoint on 0x0044B8A0
   (no extra debug stop; same effect, proven below). `record_state` attaches autostart when a difficulty
   is set so the first tick-return stop notes game `+0x6D`. Selftests cover parsing, write timing
   (never before the menu is left, nor before the pointer exists) and the note.
   `scenario_diff.py` passes the check's `difficulty` to the 1.14d recorder too (selftest: 470 pass).
   The save needs the difficulty unlocked: `d2s-tool new ... --difficulty-unlocked <d>` (no
   `prepare_saves.sh` change needed; each check builds its own save).
2. **Proof under Wine** (`traces/checks/autostart-difficulty-a3-nm.check`): the 1.14d log reads
   `difficulty nightmare written to config 0xabf9d0+0x210`, `game difficulty byte +0x6d = 1 (nightmare)`,
   `player in level 75` (Act III town). State compare: PARTIAL, no difference in 30 frames.
3. **36 diff checks** (`traces/checks/diff/`, from q-chk-difficulty-a3a5): all run, first divergences in
   `q-tool-autostart-difficulty-diff-firsts.tsv`. Summary:
   - `-bm` a3: frame 4 monster class 63 x 5192 vs 5180 (spawn position); a4/a5 `-bm`: game seed at frame 4.
   - town-start a3: game seed at frame 5; 1.14d drops 4 items, d2rs 0.
   - a4 non-bm: player `m` 0 vs 5 at frame 38/58 (normal variants PARTIAL).
   - a5 non-bm: class 446 hp 593221 vs 605952 (NM), 2671173 vs 2683904 (Hell), frame 60.
4. **28 nightmare/hell drop checks re-added** (`gen_drop_checks.py`, now generates all difficulties;
   `--normal-only` skips them). Verdicts per check: `q-tool-autostart-difficulty-drops.txt`. nig: 1 MATCH
   (nig-09), 12 DIVERGED, 1 PARTIAL; hel: 6 DIVERGED, 8 PARTIAL (no drop on either side).
5. Ledger part `docs/handoff/ledger/q-tool-autostart-difficulty.tsv` (ledger.py is not on the fetched
   ledger branch, so unvalidated; same columns as q-chk-items-drops).

## Open / routing

- Seed and spawn-position divergences at frames 4-5 (all difficulties' first frames): combat / state owners.
- Hell drop checks need more bolts (check-authoring): hel-01/03/06/08-13 have no drop on either side.
- 1.14d recordings are not committed (orig cache).

## Repro

```
sh tools/cloud-setup.sh; tools/cloud-game/setup_winpy.sh; export D2_GAME_DIR=$HOME/game
python3 tools/trace-recorder/autostart.py --selftest; python3 tools/scenario-diff/scenario_diff.py --selftest
python3 tools/scenario-diff/scenario_diff.py traces/checks/autostart-difficulty-a3-nm.check --work /tmp/w
python3 tools/scenario-diff/scenario_diff.py traces/checks/diff/diff-a3-nm-normal-bm.check --work /tmp/w
```
