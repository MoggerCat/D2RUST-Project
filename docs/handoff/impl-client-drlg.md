# Handoff: the bridge-owned client DRLG — `claude/impl-client-drlg`

> Not yet folded into `docs/HANDOFF.md` §1–§4 (only the §5 queue entries
> C78–C80, `docs/PLAN.md` and the status line of `specs/client/model.md`
> are edited here); the coordinator folds it.

Cloud implementation session, 2026-10-07, task class: implementation
from specs, medium. Base: `claude/specs-staging` at `d33adcf` (merge of
`wire-client-staging`). Repo only: no `game/`, no `re/`, no recordings
(M09: every claim below holds on this branch, on synthetic data). Read:
`specs/client/model.md` §1, §2, §7, §9, §11, §12; `specs/drlg/rooms.md`
§1–§6, §9.2, §9.9; `specs/drlg/levels.md` §2, §3, §8;
`specs/sim/path-placement.md` §4 r1; `specs/render/composition.md` §4;
`specs/render/draw-order.md` §3, §9, §10, OQ; `specs/sim/unit-order.md`
§4–§5; `specs/audio/environment.md` §1; `specs/audio/sound-table.md`
§2, §6.4; `specs/client/msg-units.md` §1.2, Randomness.

## 1. What changed

| Path | What | Spec |
|---|---|---|
| `d2-sim` `drlg/room.rs` | `Drlg::set_in_sight_at` / `unset_in_sight_at`: get-or-allocate the level, room at the tile point with the hint only when it is in that level; set: status-1 count 0 → set-and-propagate(room, 1); unset: count ≠ 0 → count − 1, unset handler 1, unpropagate(room, 2). Returns the room (`None`: no room at the point) | `drlg/rooms.md` §4.2; `client/model.md` §9 r1–r2 |
| `d2-sim` `wiring/worldgen/levels.rs` | `WorldTypes::{ds1, subs}` are `Box<dyn … + Send + Sync>` (constructor unchanged for callers: every provider passed is plain data) | — (so the client copy fits a Bevy resource) |
| `bridge/drlg.rs` (new) | `DrlgSource` (table view, DT1 headers, level-type factory), `ActList` (the act's room list: prepend, unlink in place), `ClientDrlg` (`build` = `Drlg::create(act, seed, difficulty, town 0, client)`, `set_in_sight`, `unset_in_sight`, `active_rooms` in list order, `adjacency`, `unit_seed`), `ClientDrlgError` | `model.md` §7 r4, §9 r1–r2, §12 r1, r2, r5; `levels.md` §2 r3; `rooms.md` §4.2, §5 r5 |
| `bridge/world.rs` | `ClientWorld::drlg`; `ActiveRoom::room` (the DRLG room); `room_at` / `room_from` (§12 r2: (a) cell lookup from a start room — the local player's for §2 r7, U's own for §6 r8 — and its adjacency array, then (b) the act lookup); `unit_room`; `refresh_active_rooms`; `ModelInputs::drlg`; `LevelRow::sound_env` | `model.md` §2 r7, §12 r2; `path-placement.md` §4 r1 |
| `bridge/msg/session.rs` | 0x03 frees the act and builds the client DRLG (with a source); 0x07 / 0x08 run it after the §9 r4 record; no room at the point → `HandlerError::Unspecified` | `model.md` §7 r4, §9 |
| `bridge/msg/units.rs` | creation at a point: room of the point (fatal 0x13C when none), its active room seed stepped once, unit seed `init_low(lo')` (0x59, 0xAC, 0x51); 0x15 uses `room_at` (the §12 r2 (a) TODO is gone) | `model.md` §2 r6, §12 r2, r3, r5; `msg-units.md` §3 r4.2 |
| `bridge/check.rs` | rule 8: room' := the cell lookup from U's room, else the act lookup; none → nothing (`Checked::NoRoom`); without a client DRLG the room is taken as found (unchanged) | `model.md` §6 r8 |
| `bridge/dispatch.rs`, `bridge/mod.rs` | `HandlerError::Drlg`; `Bridge::set_drlg_source` | — |
| `app/single_player.rs` | `client_drlg_source` (synthetic: the same one-room levels; live: `LevelTables` + `WorldFiles` shared through `Arc<LiveData>`, new `WorldTypes` per act), `client_level_rows` (`Act`, `BlankScreen`, `SoundEnv`) | `model.md` §11, §12 r1 |
| `app/palette.rs` (new) | `act_palette_path` (`n = a + 1`, outside 1…5 → 1), `ActPalettes` (the five `pal.pl2` read once), `present_act_palette` (PreUpdate after `bridge_frame`: act 0 until 0x03, then the model's palette act) | `composition.md` §4; `model.md` §11 r2, r4 |
| `app/play.rs` | `add_client_data` (DRLG source + Levels rows to the bridge, rows to `ModelFeed::levels`); `run` adds it and, with game files, the act palettes | — |
| `audio/driver.rs`, `audio/sound_table/table.rs`, `app/sound.rs` | `SoundTableData::env_indoors`; `ModelSoundWorld::indoors` = `Indoors` of the `soundenviron` row `SoundEnv` of the player's level (row outside → 0; no level or no Levels row → pending); `SoundDriver::frame` takes the Levels rows | `sound-table.md` §2, §6.4 r2; `environment.md` §1 r2 |
| `world_view/model_feed.rs` | `PENDING` rewritten to the new state (see §2) | — |
| `tests/app_client_drlg.rs` (new) | app wiring over a scripted join (§11 r3 order) on synthetic data: client DRLG built, player in Cold Plains' room, BlankScreen from the rows; `#[ignore]` live test of the recorded join vector (C78) | — |

Tests added (each with its `// Covers:` claim): `d2-sim`
`drlg::tests::rooms::client_in_sight_by_coordinates`; `d2-client`
`bridge::msg::tests_drlg` (5: act build, rooms in and out of sight incl.
the status-2 keep and the client-copy free, unit seeds incl. 0x13C, the
player's level and the cell lookup, the §6 r8 room lookup), `app::palette::tests` (2: paths,
the switch in a headless app), `audio::driver::tests::indoors_is_the_player_levels_environment`,
`tests/app_client_drlg` (1 + 1 ignored). No test was weakened; the
pending-hooks test lost its `§12 r1` claim (that rule is now covered by
`tests_drlg`).

## 2. Still pending (named in code; nothing guessed)

- **`ViewFeed::near_rooms` / `map_tiles` / `tile_art` / `tile_blocks`**:
  the client DRLG now builds the rooms in sight with their tile records,
  but (a) the near-room array lists each room's **units** (room +0x74) in
  the client's order, and which client code links a client unit into a
  room (creation, 0x15, path steps) and the Y sort `0x0064C0C0` that the
  draw path calls through `0x00619EA0` are not specified (`unit-order.md`
  §5 is the server's lists); (b) a record's DT1 roof height (+0x04) and
  height (+0x08) are not in `d2_sim::drlg::TileInfo`; (c) record → DT1
  file is `draw-order.md` OQ 12.
- **`ViewFeed::light`**: act environment (S→C 0x53 unhandled), light
  records, per-unit look inputs.
- **`ViewFeed::weather_frame`**: the level is known now, but water
  floors need the near rooms and passes 4 / 9 have no art path.
- **Environment machines (music, ambience, rain)**: the level and its
  `soundenviron` row are known now, but every tick also reads the day
  phase (`environment.md` OQ 3) and the weather (OQ 4).
- **Sound occlusion `blocked`** (`0x00622AA0`): the line test over the
  client collision rooms; the collision grids exist in the client DRLG,
  but the size-shrunk line test (`draw-order-2.md` §16) is not wired to
  the sound world here (follow-up, not a spec gap).
- **The in-process server sends no join** (0x03, the join's 0x07s,
  0x15): `play` builds no client DRLG until the session code exists
  (`HANDOFF.md` §2 step 4); the app test drives a scripted join.

## 3. Questions for the spec owners

1. **0x07 / 0x08 at a point in no DRLG room of the level**
   (`model.md` §9 r1–r2): not stated whether `0x0061B640` / `0x0061B690`
   test the room for null. d2rs refuses the message
   (`HandlerError::Unspecified`) after recording it in `rooms_in_sight`.
2. **The client build timer** (`rooms.md` §4.6, `0x0061B920`): the
   original client builds status-2 rooms over time (the recording built
   all 35 town rooms); the rule is "Phase 6 client spec", unwritten. d2rs
   builds only rooms set in sight, so its active rooms are a subset of
   the original's: a creation or 0x15 at a point in a status-2 room is a
   fatal 0x13C / 0x168 here where the original may find a room, and the
   room seeds (OQ 9) may differ.
3. **Object creation and the room seed**: `model.md` §2 r6 lists
   `0x00465FD0` (objects, missiles, items, tiles) among the creators that
   step the room's seed at a non-zero point; `msg-units.md` Randomness
   names only 0x59 and 0xAC. d2rs follows `model.md` §2 r6 (0x51 steps
   the seed). Which is right?
4. **The act's room callbacks**: `rooms.md` §5 r8 (act +0x4C, set by the
   client: `0x0061AF60` from `0x00475B40`) on active-room creation is not
   specified (automap?); not run. (`model.md` §9 r2's "act's unset
   callback `[0x00744398]`" is read as unset handler 1 of the table at
   `0x00744394`, which d2rs runs.)
5. **The local player's DRLG room as the 0x07 hint** (§9 r1): the model
   holds no unit → room pointer, so the hint is the room containing the
   player's position (the act lookup). The hint only shortcuts the
   lookup; no outcome differs while DRLG rooms do not overlap.
6. **Room unit lists on the client** (§2 above, a): an owner for client
   unit ↔ room membership and the draw path's Y sort.
7. **`TileInfo` vs the draw order's DT1 fields** (§2 above, b): extend
   `d2_sim::drlg::TileInfo` with roof height and height, or have the
   client read DT1 headers itself (`draw-order.md` OQ 12 decides the
   file mapping).

## 4. Gate (this branch, head in the commit)

- `cargo test -p d2-client -p d2-sim`: 4,187 passed, 0 failed, 131
  ignored over every target; `cargo check -p d2-server --tests` clean.
- `cargo clippy -p d2-client -p d2-sim --all-targets -- -D warnings`:
  clean. `cargo fmt --all --check`: clean. `cargo run -p depcheck`: OK.
- `python3 tools/coverage.py --check`: 8,115 claims, 0 errors.
  `python3 tools/spec_index.py --check`: ok. `tools/methods.py check`: ok.

## 5. Local run queue (added to `docs/HANDOFF.md` §5)

- C78 the recorded join on game files (`app_client_drlg -- --ignored`).
- C79 `play --frames 500`: act palette presented from the first frame.
- C80 client vs server room seeds (`model.md` OQ 9, memory read).

## 6. Next steps

1. Session code that sends the single-player join (0x01 … 0x03, 0x07 ×
   n, 0x15, 0x04) from the in-process server, so `play` builds the
   client DRLG (C79 then checks the log).
2. Spec answers for §3 Q2 (client build timer) and Q6 (client room unit
   lists); then `near_rooms` / `map_tiles` from `ClientDrlg` (rooms of
   the local player's adjacency array, their tile records).
3. `TileInfo` roof height / height (Q7) and the record → DT1 file map
   (`draw-order.md` OQ 12) for `tile_art`.
4. The sound world's `blocked` through the size-shrunk line test over
   the client collision grids (`draw-order-2.md` §16).
