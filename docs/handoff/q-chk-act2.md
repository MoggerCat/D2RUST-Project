# q-chk-act2 hand-back (2026-10-09)

CHECK session, Act II (T3). Everything below ran under Wine against 1.14d
(`tools/scenario-diff`, `suite.py --filter 'a2-*' --workers 3`, 7 min).

## Done
- `tools/chk-act2/gen.py` generates 34 checks `traces/checks/a2-*.check` (plus the 4 existing
  `a2-warp-*`): NPC flows (`a2-npc-*`: Drognan, Fara (trade, heal), Elzix (trade, gamble), Lysander,
  Atma, Greiz (hire list, hire), Jerhyn, Meshif, Warriv, Cain), the six quests (`a2-quest-*`),
  nine waypoints (`a2-wp-<level>`), six superuniques (`a2-super-*`). NPCs and objects are driven with
  `poke goto unit` + `send InteractWithEntity/InitEntityChat/QuestMessage/EntityAction/...` (C->S 0x13
  etc.); kills with `pos` + `stat life` + a Fire Bolt `missile` poke (as `act2.play`).
- `tools/chk-act2/qflags.py <suite work dir>`: the quest/NPC S->C records (0x5D, 0x28, 0x27, 0x8A,
  0x2A, ...) of both packet recordings side by side. Quest flags are read from the packets
  channel because the 1.14d recorder does not write the state channel's `q` field.
- Verdicts: 38 checks, 76 channel results: DIVERGED 61, PARTIAL 15, no errors. No row is EQUAL.
  `docs/handoff/ledger/q-chk-act2.tsv` (38 rows, `ledger.py` 0 format errors).
- What matches (state channel PARTIAL, nothing differs): talk + store open for Drognan, Fara, Elzix,
  Lysander, Atma, Cain, Meshif, Greiz (hire list); Cain's quest messages 335/336 and the first
  Radament `0x5D 08 00 01 00 00`; from frame 13 the s2c/buf streams of the trade checks are equal
  (store generation for seed 1234).
- First divergences routed into `docs/handoff/build-queue.tsv` (`q-fix-a2-*`; the seed ones go to
  q-fix-real-unit-seed-order, the object one to q-prov-recording).

## Open
- Quest sequences stop at the first divergence: Radament, Summoner (seed after the warp), Tombs
  (Jerhyn at frame 14), Arcane (guard m), Tainted Sun (object order). The kill, reward and `0x5D`
  steps behind them are unproven until those land; rerun the checks.
- Not authored: identify at Cain (needs an unidentified item in the pack), resurrect at Greiz (merc
  death), imbue-style services (none in Act II), Warriv/Meshif act travel, the orifice + staff, drops
  of Duriel and the superuniques (loot compare needs a surviving kill), Fangskin and Fire Eye spawn
  (1.14d `superunique` poke fails there, `q-chk-a2-superunique-poke`), Act II guards 2/4/5 talk.
- `q` (quest words) is not recorded on the 1.14d side (`state-snapshot.md` §2): a recorder item for
  q-tool-state-diff would turn the quest compare from packets-only into a state field.
- q-tool-interact-pokes and q-tool-check-gen had not landed; regenerate with the generator when they do.

## Repro
```
sh tools/coord/session-setup.sh && export D2_GAME_DIR=$HOME/game CARGO_INCREMENTAL=0
python3 tools/chk-act2/gen.py
python3 tools/scenario-diff/suite.py --filter 'a2-*' --no-playthrough --workers 3 --md /tmp/a2.md
python3 tools/chk-act2/qflags.py traces/raw/suite/a2-quest-radament
python3 tools/trace-recorder/packets_diff.py traces/raw/suite/<check>/orig.packets.jsonl traces/raw/suite/<check>/d2rs.packets.jsonl --from 13
```
