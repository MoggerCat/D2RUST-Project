# Soak findings (q-chk-soak)

Campaign: `python3 tools/soak/soak.py campaign --minutes 60` on claude/integ-r6
(merged into claude/q-chk-soak), all starts incl. the 28 checkpoint saves
(`tools/checkpoints/make.py`), seeds 1-4: 107 runs, 321,000 frames, 2 signatures.
Repro logs are in `tools/soak/repro/`.

| Signature | First seen | Reduced repro | Owner | State |
|---|---|---|---|---|
| `desync:player-position` (client (4840,4217) vs server (4875,4227), 35 sub-tiles) | new-sor seed 1, step 190 | `tools/soak/repro/desync-player-position.log`, 3 actions, `--steps 590` | walk desync / re-target (owners.tsv row `claude/q-fix-client-crash`) | open, routed to coordinator |
| `room:outside:Item` (Item 23 at (4881,4220) outside RoomId(3) 40x40 at (4840,4200)) | new-bar seed 1, step 564 | `tools/soak/repro/room-outside-Item.log`, 7 actions, `--steps 964` | items (`claude/q-fix-server-store-fill`) | open, routed to coordinator |

Replay: `D2_GAME_DIR=... target/release/d2-client soak --replay <log> --steps N --keep-going`
(start args: see `soak.py` STARTS; the logs name their start in the header).

Both signatures sit next to each other (same x 4840-4881, y 4200-4227): the
item lies outside the room by the same 35-41 sub-tile amount, so they may share
a cause (position past a room edge). Not yet confirmed.

## Soak tool bug fixed
The log reader refused negative coordinates (`click L 561 -4`, which the
generator emits), so `room:outside:Item` could not be replayed or reduced
(reported as `crash:exit1`). Fixed in `crates/d2-client/src/app/soak/log.rs`
(signed parse + test `clicks_outside_the_frame_round_trip`).

## Not done
Campaign was run once (60 min). Other seeds/rerun on the final staging
candidate, and any ledger rows, remain open (no check settled: soak is
invariant-only, not compared to 1.14d).
