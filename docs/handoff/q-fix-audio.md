# q-fix-audio — hand-back (2026-10-09)

Session q-fix-audio, branch `claude/q-fix-audio` (task: make the four
`traces/audio/` checks EQUAL, or list the causes). REC-1680..1684 used.
d2rs side only; the 1.14d captures are the four of q-tool-audio-diff,
re-recorded in this session (`audio_diff.py run … --orig-only`).

## Done (all four checks re-run after each fix)

| Fix | Checks it moved | REC |
|---|---|---|
| First server tick runs no sound tick: d2rs' first-tick voices start at T 0 like 1.14d's (recorded: T 0 is the second frame; C offsets; presented tick = last tick run) | all four: town1 / wilderness / fire4 / windowopen all pair at T 0 with equal device volume and pan | 1684 |
| `object_river` (2599) reads set C first (the 5 river objects are client-only; GUID 8 is also a server torch) | `riverloop.wav` no longer started (was a vol-0 voice 1.14d never makes) | 1680 |
| S→C 0x5A lines request UI sound 6 (`windowopen.wav` at T 0) | all four | 1681 |
| A monster's footstep reads f before the update's advance | every NPC footstep request of town / walk is on the recorded tick | 1682 |
| Skill start sounds from the unit's 0x15 / 0x16 mode request | `coldcast.wav` at T 18 / 58 (cast) | 1683 |
| Client quest check `0x004A4180` (new `ClientWorld` quest records) | Blood Moor entry line at T 64 (`sor_act1_entry_wilderness.wav`, `ama_…`) | – |
| Player footsteps (walk check): first at T 8 and every request equal | walk-town | (from the tick base) |
| REC-1362 decision-log entry (`docs/PLAN.md`), owners row for the client audio paths | | 1362 |

Request-level comparison (a helper in this session, not committed: orig
`request` records vs `--sound-log`) is exact for: the player's footsteps,
the music / ambience / fire / UI starts, the cast sound, the entry lines,
the monster weapon swings (`thrust` ticks, up to the monster GUID shift
below).

## State of the four checks

| Check | voices 1.14d / d2rs / paired | left |
|---|---|---|
| audio-town-ambience-ama | 23 / 25 / 5 | rain (1), NPC (1,7) footsteps (2), Warriv greeting `war_goodday.wav` T 65 (1), footstep variant picks (7) |
| audio-walk-town-ama | 32 / 37 / 7 | same plus NPC (1,7) and (1,12) walk |
| audio-monster-hit-ama | 37 / 28 / 12 | rain, birdie T 58, impact + get-hit sounds (player never hit in d2rs), roar / warcry ticks, GUID shift |
| audio-cast-frost-nova-sor | 13 / 10 / 9 | rain (1), birdie T 58 (1), `novaice.wav` T 25 / 65 (2) |

The mixed check is 16/249, 6/119, 1/199, 1/99 ticks equal: see "mixed".

## Open, with causes (not fixed here)

1. **Rain onset** (1.14d `rain2.wav` T 4, d2rs T 12–15). The weather
   (`world_view/weather_view.rs`) draws its cycle length and peak from a
   private seed `init_low(guid)`; 1.14d draws them from the local player's
   *shared* client seed, between 0x59's step, the room-change steps, the
   cursor / shake / floor steps and the sound draws
   (`audio/sound-table-2.md` §14.3). Onset needs `peak ∈ [0.4 D, 0.5 D)`
   with D = 250 + roll(250), peak = 32 + roll(224) at the first update. One
   shared stream (weather, sound, model steps in 1.14d's order) would
   decide this, and the ambience event cue (`birdie05.wav` T 58) and the
   footstep / warcry variant picks (7 + 10 + 5 variant diffs) with it.
   PC 1 item queued: the player's client seed at the first sound tick.
   Owner of weather: render (`world_view/`, not in `owners.tsv`).
2. **Nova missile sounds** (`novaice.wav` T 25 / 65, 64 requests each,
   `0x4cdb04`, TravelSound on the server missile units [3, 208 …]): d2rs
   has no missile units on the client for the nova (only visual client
   missiles in `world_view/missiles.rs`, which make no `MissileSound`
   output), so the audio feed sees nothing. Needs the server's missile
   unit packets for the nova, or the world_view missiles to emit the
   create sound. Owner: skills / missiles (`claude/q-diff-skills-1/2`).
3. **Player is never hit in d2rs** (monster-hit): 1.14d plays
   `sword1.wav` (impact, `0x4cc468`) and `soft4.wav` (get-hit) on [0, 1] at
   T 75, 96, 122, 135, 171; d2rs' player never enters mode 4, so the mode
   sounds never run. Combat outcome, not audio. Owner: monster melee
   (`claude/q-fix-b-monster-combat`, `claude/q-diff-combat-a1`).
4. **Monster GUIDs**: the three Fallen are (1, 19 / 17 / 18) in d2rs and
   (1, 21 / 19 / 20) in 1.14d (the request keys match 1:1 otherwise).
   Owner: seed / unit order (`claude/q-fix-real-unit-seed-order`).
5. **NPC (1, 7) walk** (town, walk): 1.14d's footsteps are at T 51, 59,
   72, 80, … (gaps 8 / 13), d2rs' at 46, 54, 62, … (gap 8); (1, 6) / (1, 7)
   and (1, 3) / (1, 4) also have an extra d2rs step at T 22 / 30; (1, 12)
   steps at T 84 in d2rs. NPC walk tracks. Owner: act1-town NPC tracks.
6. **Warriv greeting** (`war_goodday.wav` on the player, T 65, return
   `0x4cc05b`): the server's 0x2C event 18 for an NPC near the player (or
   the proximity rule) does not occur in d2rs at that tick. Sim side.
   Owner: NPC interact (`claude/q-fix-pc1-day3-a-r2`).
7. **mixed check**: 1.14d stops a one-shot voice when the device cursor
   passes its end (wall clock under Wine, e.g. `windowopen.wav` "stop 1"),
   d2rs ends it at the first upkeep with elapsed ticks × 40 ms ≥ its
   duration; the mix of the first ticks differs by that tail. The check's
   definition is decided (REC-1362, `docs/PLAN.md`); the stop tick of
   one-shot voices is a capture property, not a d2rs rule.
8. **0x26 chat lines** request no UI sound 6 (`ui/game_messages.rs`
   `OriginalUi::game_message`); the 0x5A sound is a stand-in in
   `world_view/present.rs` (REC-1681). Owner: client UI
   (`claude/q-fix-pc1-client-ui`).
9. **Two cast messages**: d2rs' server sends two mode messages per cast
   (code 0x15, level 0 then level 20); REC-1683 plays only the one with the
   level for the local player. Owner: skills cast (`claude/q-diff-skills-2`).

## Notes sent

- coordinator: this file.
- Owners named above: one note each (see the session transcript of
  `q-fix-audio`); the PC 1 item is in `docs/handoff/pc1-data.md` Step 4.

## Repro

```sh
export D2_GAME_DIR=$HOME/game          # assembled from the private repo
cargo build --release -p d2-client -p d2s-tool
export D2RS_BIN_DIR=$PWD/target/release
for c in town-ambience-ama walk-town-ama monster-hit-ama cast-frost-nova-sor; do
  python3 tools/audio-diff/audio_diff.py run traces/audio/audio-$c.check   # ~8 min each
done
# d2rs side only after a change (the 1.14d capture is kept in target/audio-diff):
python3 tools/audio-diff/audio_diff.py run traces/audio/audio-X.check --d2rs-only
python3 tools/audio-diff/audio_diff.py run traces/audio/audio-X.check --reuse
cargo nextest run --release -p d2-client --lib audio
```
