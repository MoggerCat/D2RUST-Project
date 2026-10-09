# tools/autoplay: real-input smoke routes

Spec: `specs/tools/autoplay.md`.

`d2-client autoplay-host` runs the `play` client headless and takes mouse
and key events on stdin (line protocol `autoplay-1`), put where the
window's input goes. A route in `routes/` is a fixed list of those events
with state checks (`check level N`, `check sent 2F >= N`), recorded once.
Replaying it needs no logic: the same build, save and seed give the same
game.

```sh
cargo build --release -p d2-client -p d2s-tool
export D2_GAME_DIR=/path/to/1.14d          # the private data repo's install
python3 tools/autoplay/replay.py --all      # ~1-3 min per route
python3 tools/autoplay/replay.py tools/autoplay/routes/act2-probe.route
python3 tools/autoplay/replay.py --selftest # no game files
```

| Route | Character | What it does (milestones held, 2026-10-09) |
|---|---|---|
| `act1-probe` | fresh Amazon, seed 1234 | 5 town NPC talks, town waypoint menu, walk into Blood Moor (7) |
| `act2-probe` | level-30 sorceress in Lut Gholein | walk the town, talk to every NPC, waypoint menu, waypoint out and back (20) |
| `act3-probe` | same, Kurast Docks | as Act II, Natalya excluded (12) |
| `act4-probe` | same, Pandemonium Fortress | as Act II, Halbu and Jamella excluded (6) |
| `act5-probe` | same, Harrogath | as Act II (12) |

A failing check prints the line, the milestone and why; the host's stderr
(and `target/release/d2rs-crash.log` on a panic) is in the work dir.

A route goes stale when the build changes what the same clicks reach
(an NPC moved, a different path). Then re-record it: the recorder was
the dropped autoplay bot (git history of `tools/autoplay/autoplay.py`,
commit `bf1cdefa`); or write a route by hand from `state` reads.
Open findings: `docs/handoff/q-tool-autoplay.md`.
