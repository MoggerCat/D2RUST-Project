# impl-pc1-s7: code for PC 1 session 7's specs (2026-10-07)

Branch `claude/impl-pc1-s7`, from `claude/specs-staging-7` @ `da55bc9`,
with `origin/claude/specs-staging-7` @ `431a3fd` (PC 1's final pass)
merged in. Two sub-branches merged: `claude/impl-pc1-s7-motion` (MV1–MV11,
wall follow, hit class, bodies-4 EC1) and `claude/impl-pc1-s7-automap`
(see §1 item 9). Gate: last section.

## 1. What landed

1. **0x58 `effect:u8@6`** (`intents-events.md` §6 r3, r6).
   - `d2-proto` `OpenUi` has `effect` (no unwritten bytes); audit note
     and `docs/handoff/s2c-builders.md` row updated. The sim builders
     were already right (orifice result 5 writes 1, every other code 0).
   - Comparator (`conformance::scenario::compare`): the mask table
     `specs/tools/scenario-masks.tsv` has the five columns of §6 r6
     (`id key offset length source`; key `u8@o=v` / `u16@o=v`; offset
     `nul@n`; length `..n`); keys and NUL positions read from the
     original's record, key bytes compared. Rows: 0x58 byte 6 for codes
     0/1/4/6/7 (code 5 compared whole); 0x50 keyed by u16@1 (4 → 13–14;
     2, 0x24, 13 → 5–14; 0x17 → 3–14; the unkeyed 13–14 row is gone);
     0x26 forms 5/6; 0x27 count 1; 0x82 after the name's NUL.
     `specs/tools/scenario.md` §6 r1/r3 and its rows table describe the
     five columns.
2. **`Character::New` sends 0x5F and two 0x23** (§8.2 r7,
   `formats/d2s-load.md` §1, §8 r1/r3).
   - `d2-server::adapters::session`: `Entry::new_character`,
     `PlayerRecord::new_character`, `initial_portal_flags` (1 << index of
     level 1 in the portal level list), `Entry::load_skill` (the load's
     own 0x23: hand 0, `StartSkill`, item −1, sent right after 0x76),
     `load_new_character` (runs load §1 on the action wiring). A stub
     `.d2s` (`load_save`) gets the same entry.
   - `character::load_new_character`; `LoadReport::right_skill`.
   - `d2-sim::drlg::LevelDef::portal`, `DrlgData::portal_levels()`.
   - The app's `Character::New` loader runs the stub load.
3. **Wall-follow empty follower** (`pathing.md` §12.8 r5) — motion branch.
4. **Hit-class merge** `0x005AD730`: R +0x60 := HitClass | (old & 0xF0),
   +0x64 := 1 when either has a bit in 0xF0 — motion branch.
5. **`0x0055F4F0` only under flag-ex bit 21** (`tick.md` §6 r5): it is an
   empty function and the per-client refresh hook has no implementation
   in the wiring, so only the `TickHooks::client_update_messages`
   contract (doc) changed.
6. **`event_layer_split` = (6, 0x3F)**: also landed upstream; the merge
   kept upstream's text. Test `item_event_layer_split_is_the_1_14d_stuff`.
7. **MV1–MV11** (motion branch, `wiring/path/monsters.rs` rewritten):
   units.md §4.6 r5–r14 mode starts (WL/RN/GH/attack/skill/BL/KB/SQ/S3/S4)
   and event functions (RN/KB/SQ/S3 event 0, KB event 1;
   `Pending::monster_run_event0` removed; new seams
   `monster_skill_start`, `monster_sequence_frame`); ai.md §7.5 r4–r7
   movement set-up (velocity request, t = 100 writes nothing, n default 5,
   type-15 retry, `MoveSetup`), mode set without target aims at (0, 0),
   `set_path_steps` / `stop_path` on the provider; pathing.md §13.1
   stale target via `0x00553540`, §13.2 step count low byte capped 77,
   §13.3 `path::line::line_test` as `MissileBodies::line_hits`.
8. **Session-flow bodies** (`intents-events.md` §2.5 r1–r6, §8.1 r1),
   `d2-server::adapters::session_flow`:
   - 0x67 checks in `0x0052C330` order: character name NUL, locale, game
     name NUL, flags, `0x00538B70` (client already has a record →
     `HasGame`), `0x00538C60` (another record with the same name,
     `_strnicmp` 16 → `NameTaken`), class. Arena flags = u32@0x27 &
     0x3179C7 (`SessionFlow::new` no longer takes them).
   - 0x69 leave (state 4 only): `NotSaved` fault per client with a
     player (no character-save writer), 0x05, 0x06 queued, direct 0xB0,
     the client's buffers flushed, record removed, 0x5A code 3 to the
     remaining in-game clients.
   - 0x6A: direct 0xB2 for the game, then the terminator.
   - 0x6C: chunks into `SessionFlow::uploads`, complete → flag and
     `d2s::checksum`; total ≥ 0x2000 and overflow are faults.
   - 0x6E: nothing; 0x70: `heartbeat_flag`.
   - Host: the system-queue drain passes a `SystemSink` with
     `MessageSink::send_direct` / `flush_client` (defaults refuse with
     `QueueError::NoDirect`, never a silent queue);
     `SimGame::client_list()`.
9. **Client C→S 0x67 sender** (`client/model.md` §7 r9):
   `create_request_for` fills the builder layout; expansion character →
   0x00100004, classic → 0x4 (`CREATE_FLAGS_CLASSIC`, PROVISIONAL).
10. **bodies-4 edge case 1** (user decision via the coordinator, d2rs
    decision): lightning fan / ring with progressive step ≤ 0 creates
    the i = 0 missile, stops the loop, creates nothing more and reports
    `BodyEffect::EndlessProgressive` → `WiringError::EndlessProgressive`
    (before: silently nothing). Tests: step 0 and negative, fan and ring.
11. **Automap** (`ui/automap.md`): automap branch, `crates/d2-client/src/ui/automap/`
    (`cells`, `picker`, `place`, `town`, `persist`, `options`, `view`,
    `draw`, `markers`, `header`): §1 AVL cell store and layers, §2 picker
    on `fixup::maps::Automap` (seed {0, 666}), §3–§4 tile / unit cells,
    §5 reveal by leveldefs `Layer`, §6 town art, §7 `.map` / `.ma<k>`
    files (in memory + thin fs wrapper), §8 options (`OptionStore`), §9
    view, §10 draw pass (`AutomapDraw` commands), §11–§12 markers, §13
    header, §14 lifetime; 42 tests; ignored game-file tests in
    `tests/game_automap.rs`. After merging the final spec pass, the
    answered OQ1/OQ2 gates are functions: `markers::unit_dead`
    (`0x00464820`, §11 r1), `view::mini_down` (`0x00492C10`, §9 r1).
    **Not wired** into the app / Bevy (needs bridge + `world_view`
    inputs, owned by the client triage session); unverified.

## 2. Changed test expectations

| Test | Was | Now | Why |
|---|---|---|---|
| `conformance scenario::compare::mask_table_parses_and_is_strict` | 4-column table, 9 rows | 5-column table, 23 rows incl. keyed rows | `intents-events.md` §6 r6 |
| `d2-proto s2c::tests` (unwritten-bytes case) | flipped byte 6 of 0x58 ignored | uses 0x2A bytes 3–6; 0x58 byte 6 is a field | §6 r3 (0x58 @6 is `effect`) |
| `conformance tests/s2c_builders service_result_0x58_one_maker` | `OpenUi { npc_guid, result }` | `+ effect: 0` | field added |
| `test-fixtures synthetic_game a_stub_starts_a_new_character…` | messages unchecked | load 0x23, 0x0B, 0x5F, hand 1 / hand 0 with item 0 | §8.2 r7, d2s-load §8 |
| `d2-client app_single_player the_session_flow_creates…` | log empty | log = the six named unapplied stub-load steps; 0x5F and two 0x23 present | §8.2 r7 (the stub path runs) |
| `d2-client app_frame_loop frame_loop_ticks_the_server…` | joined 15, handled 20 | 18, 23 (0x5F + two 0x23) | §8.2 r7 |
| `d2-client app_frame_loop` / `app_client_drlg` fixtures | no client skill rows | one synthetic `skills` row (`app_support::synthetic_skill_rows`) | the synthetic game has no skills rows; the client asserts a selected skill is inside its table (`msg-skills.md` §2 r3) |
| `wiring::path::motion_tests::a_mode_that_does_not_move…` | path type 0 | allocation's type kept, step counts unchanged; BL animation record added | ai.md §7.5 r4.2 |
| `wiring::action::tests::ai::good_npc_ranged_takes_ai_turns` | no think after the wander walk | think at 36, mode neutral | units.md §4.6 r5 (0 points → neutral) |
| `wiring::path::motion_tests::an_attack_end_requests_neutral` | mode set by hand | attack start sets mode 4 | units.md §4.6 r7 |

## 3. PROVISIONAL points (this branch)

- `crates/d2-server/src/adapters/character.rs` `new_character`: with no
  skill-list provider the `StartSkill` is taken as present
  (`formats/d2s-load.md` §1 r1, §8 r3; REC-02).
- `crates/d2-server/src/adapters/session_flow.rs` `SessionFlow::game_id`
  (game +0x28 = 0) and `game_list` (game +0x8C read as the client count)
  (`intents-events.md` §2.5 r3; no REC).
- `crates/d2-client/src/app/single_player.rs` `CREATE_FLAGS_CLASSIC`
  (`client/model.md` §7 r9; REC-46).
- Automap (spec-marked): `persist.rs` §7 r2/r3 header order, chain link
  = 8th u32, table entry = first record offset (automap-0003);
  `markers.rs` §11 r7 / OQ7 name colour = palette byte (automap-0004);
  `draw.rs` §10 r4 / OQ6 fade 3 reads player +0x18 as-is (automap-0001).
  Readings marked "Reading" in code (worth a spec answer): §7 r4 / EC5
  where an out-of-range cel cuts the chain; §9 r2 Left saved / restored
  once; §11 r6 disguised monster names; town art under v = 1 uses mode 5;
  a save always appends a record; load checks the town blob against the
  record's town kind; cels outside 0–2047 are group −1.

## 4. Left, with reason

- Obelisk power-up path (`world/objects-2.md` §19, the other writer of
  0x58 `effect`): `q6::item_to_object` still reports non-orifice inserts
  as unhandled; needs a cursor-removal seam (`0x0055EEA0`, items area)
  and set-stat on `QuestWorld`.
- A full save's player record at the join (`d2s-load.md` §8 r2: header
  skills, item indices resolved after the items) is not derived; only
  new characters / stubs carry one.
- The join's 0x5A (§8.3) and the rest of the join sequence `0x0052C410`
  (0x5B, 0x65, 0x8D): `TickHooks::join_sequence` has no implementation.
- The leave's character save (`0x00532400`): no save writer; recorded as
  `SessionFault::NotSaved`. The leave's client +0x3D4 bit 5 loop is not
  modelled (bit never set in d2rs).
- Motion: KB event 1's get-hit branch (clear used skill, U +0xB0 :=
  0xA0) needs Pending setters; `Pending::body_line_blocked` callers still
  answer "blocked"; d2-server / d2-client / test-fixtures hosts do not
  wire `monster_skill_start` / `monster_sequence_frame` (SQ goes neutral).
- Automap: not wired (nothing consumes `ToggleAutomap` / UI state 0x0A;
  needs unit class / mode / position and leveldefs Layer / LevelType /
  act from the bridge, DRLG callbacks +0x454 / +0x488, 0x03 act values,
  the 0x90 roster, a draw sink for cels with draw modes and lines, the
  save directory and character name); the §11 r3 relation code
  (`0x00478D90`) stays a caller input; `rules::draw_order::AutomapReveal`
  (level filter) is superseded by `ui::automap` but left with its test;
  the `.map` sub-directory fallback of §7 is not done.
- Local run queue (add to HANDOFF §5):
  `D2_GAME_DIR=<install> cargo test -p d2-client --test game_automap -- --ignored --nocapture`:
  `picker_first_record_vector` (range[1] = (0, 83); LevelType 1, `fl`,
  main 0, sub 5 → cel 2, seed {666, 0}); `cel_files_open` (the eight
  `MaxiMap` / `Act2Map` / `Act4Map` / `ExTnMap` [+ `S`] files open as
  DC6 with `.dc6` appended; record the cel counts = load limits).

## 5. Gate

GATE_PLACEHOLDER
