# q-fix-room-links (REC-1140..1149; used: REC-1140)

## Done
- **q-fix-soak-static-leave-room, q-fix-soak-drop-room**: not a sim bug. The
  room-list remove `0x0064C370` never touches the path (`sim/unit-order.md`
  §5 rule 3), so an item picked up or an object out of its room list keeps
  the static path's room in 1.14d too; d2rs matches. The soak check
  `room:list-vs-path` / `room:outside` flagged exactly that (the reduced logs
  now show `room:list-vs-path:Item`, not the old signature). Fix: the check
  skips Item / Object units with no list room (`app/soak/checks.rs`,
  PROVISIONAL REC-1140). Proof: all three logs in `tools/soak/repro/`
  (pickup, drop, act change) replay with 0 findings.
- **Town objects after a waypoint return (pause blocker 6)**: already fixed by
  83bd5887 (preset flags 0x3000000, REC-287). Verified on the d2-client path:
  town -> Blood Moor waypoint -> town. Right after the return only the rooms
  near the waypoint are populated (NPCs / objects of the far rooms come back
  when the player walks there, as the rooms stream); after one `pos` into the
  west of the town all 7 NPC classes and every object are back (positions
  equal; NPCs wander). `town_round_trip` (ignored) passes.

## Open
- No 1.14d scenario-diff of the return (no way to drive 1.14d from the cloud):
  whether the original also leaves far town rooms unpopulated until entered
  is unverified (PC 1 item added below).

## Repro
```sh
export D2_GAME_DIR=/home/user/game
cargo build --release -p d2-client -p d2s-tool && python3 tools/soak/soak.py saves
for f in static-room-after-pickup drop-outside-room static-room-after-act-change; do
  target/release/d2-client soak --replay tools/soak/repro/$f.log --steps 3000 --keep-going; done
target/release/d2-client state-dump --save target/soak/saves/Soaktown1.d2s --seed 1 --ticks 1500 --out o.txt \
  --poke "5 pos @player 5684 5796" --poke "800 pos @player 5640 5830" \
  --send "20 InteractWithEntity type=2 id=11" --send "30 TakeOrCloseWp wp=11 level=3" \
  --send "600 InteractWithEntity type=2 id=25" --send "610 TakeOrCloseWp wp=25 level=1"
cargo test -p test-fixtures --test town_round_trip -- --ignored
```
