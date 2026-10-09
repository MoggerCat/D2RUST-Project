# q-fix-quest-load (REC-1685, REC-1686)

Task: the `q` (quest words) channel showed d2rs not carrying the save's quest records into the host on
join (`join-act2-quests-ama`); fix it, then run `q` on the quest-bearing checks and fix the next first
divergence in quest state.

## Done

1. **The join "divergence" was the 1.14d recorder, not d2rs** (REC-1685). Player data +0x10 + 4·d points
   to the record's bit-buffer header {u32 buffer pointer, u32 bit count 0x300}, not to the 96 bytes. The
   old 1.14d `q` (`[0,0x6080],[1,0x321],[2,0x300]` every 32 bytes) was the three difficulties' headers:
   a heap pointer and 768. `tools/trace-recorder/record_state.py` now dereferences the buffer pointer;
   the selftest fixture has the header layer (676 source bytes, each changes exactly `q`).
   `specs/tools/state-snapshot.md` §2 `q` row says so. d2rs's load (`single_player.rs`, `copy_in` with
   normalisation, `world/quests.md` §1.6) was already right.
   Verified under Wine: `join-act2-quests-ama` 1.14d `q=[[7,1]]` on every frame = d2rs.
2. **The dialog branch's C→S 0x31 was never sent by `state-dump`** (REC-1686). 1.14d's client answers
   0x8A / 0x28 (T = 1) with 0x2F and then 0x31 {npc, m} (`client/msg-ui.md` §16 r4.3 case B2); that 0x31
   sets the gossip / quest-start bits (slot 0 bit 0 at Warriv, slot 8 bit 0 at Jerhyn, slot 9 bit 2 at
   Atma). `state-dump` dropped the bridge's outputs, so the model's reply slot stayed empty.
   `crates/d2-client/src/app/state_dump.rs` now delivers the outputs to a headless `OriginalUi`
   (`DialogUi`) and applies `take_dialog_answer` through `Bridge::npc_dialog_branch`, as `play`'s
   dispatcher does (`world_view::present::deliver`). UI errors become footer notes, once each.
   PROVISIONAL REC-1686: `expansion_installed = true` (the checks run the 1.14d LoD install; `play` reads
   it from the archives).
3. New check `traces/checks/a1-quest-andariel-reward.check` (save 6.1, talk to Warriv, msg 183):
   state 0 differences (PARTIAL only for the `own` gap); `q` 6.1 → 6.0 at frame 18 and slot 0 bit 0 at
   frame 15, as 1.14d. Packets: one first divergence, the 0x07 MapReveal one frame late during the
   `goto` poke (frames 3–4), not quest state.

4. New check `traces/checks/a1-quest-den-reward.check` (save 1.1, talk to Akara, msg 76; asked for by
   q-fix-a1-den-wp): `q` equal on all 59 frames (1.1 → 1.0 and slot 41 = 0x2002 at frame 18); state
   0 differences. Packets: the 0x07 MapReveal one frame late in d2rs during the `goto` walk (frames
   3–14), same as check 3.

## `q` results (1.14d vs d2rs, per player GUID; script logic in "Repro")

Equal on all frames: a2-npc-{atma-talk, cain-talk, drognan-trade, elzix-gamble, elzix-trade, fara-heal,
fara-trade, greiz-hire, greiz-hirelist, jerhyn-talk, lysander-trade, meshif-talk, warriv-talk},
a2-quest-{radament, staff, summoner, taintedsun, arcane}, a2-super-* (6), a4-deseis-seal-* (3),
a5-npc-* (5), milestone-* (11), join-act2-quests-ama, a1-quest-andariel-reward, a1-quest-den-reward.
(atma, jerhyn, radament: equal only with fix 2; they differed at frame 15 before.)

Open, not quest code:
- `a2-quest-tombs`: q differs at frame 138 (slot 14: 1.14d 0x0004, d2rs 0x200C) because the 1.14d player
  died in Duriel's lair (hp 0 from frame 98) and the d2rs player did not (hp 12475 at frame 96): the Tal
  Rasha / Jerhyn messages run in different states. Owner: combat (Duriel vs player damage).
- `a2-super-fangskin`, `a2-super-leatherarm`, `a2-quest-arcane`: the 1.14d player dies and its corpse
  (a second type-0 unit, `q` = []) appears; d2rs has no corpse there. Player `q` itself equal. Owner:
  death / corpse.

Not run (container restart stopped the sweep): a5-su-*, hire-*, a5-town-*, a[34]-start-*,
a5-harrogath-*, a4-fortress-*; the ~240 warp / waypoint / gen-lvl checks only load and idle (covered by
fix 1 + the join check).

## Playthrough

`den-of-evil-done`, `andariel-done`, `act2-open` (act1.play): 0/1 each, unchanged. Blocker: the headless
`--input` click at the NPC walks next to it instead of picking it (no talk is sent), not quest state.
With the talk sent (`--send` 0x13 / 0x2F / 0x31 183) d2rs sets 6.0. Owner of the click / hover pick:
input (`bridge/click.rs`, q-fix-input-lock) or the .play files could use sends.

## Repro

```sh
python3 tools/trace-recorder/record_state.py --selftest
D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/join-act2-quests-ama.check --work W
D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/a1-quest-andariel-reward.check --work W
python3 tools/scenario-diff/suite.py --filter 'a2-npc-*' --workers 3 --no-playthrough
# q per check: for each work dir, compare the `q` of each ut 0 unit by GUID `g` frame by frame
# between orig.state.jsonl and d2rs.state.jsonl.
```

## Notes for owners

- `specs/world/quests.md` §1.4 (q-fix-npc-interact): "+0x10 + 4·d holds the record" is the bit-buffer
  header pointer; the 96 bytes are behind its first u32.
