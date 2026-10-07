# impl-triage-client: d2-client rows of `todo-spec-triage.md`

Branch `claude/impl-triage-client` (base `claude/specs-staging-6` @ 740449b).
Scope: part 1 "## d2-client" (ANSWERED) and part 2 "## d2-client" (NOT
SPECIFIED, provisional per M22). Three workers: UI / app / bridge / tests
(lead), audio (helper A), render (helper B).

## What landed — UI, app, bridge, tests (lead)

### UI widgets and panels
- `ui/widget.rs` — `Button` pressed frame (`pressed_image`, `pressed`; image
  changes only with the pressed flag, panels-2 §22 r1); `Label.pen` (the
  text rule's pen, rect = hit/clip only, §22 r2); `ScrollList::wheel` /
  `WHEEL_STEP = 0` (§22 r3); `CellGrid` geometry of `ui/inventory.md` §1 r3–r4
  (`mouse_cell` unsigned wrap, `footprint` clip test), §5 r1 (`hover_anchor`),
  §5 r3 (`cursor_cell`, incl. the "returns without change" edge), §8 r4
  (`item_draw_point`); `CellGrid::draw` draws nothing (§7, §B5);
  `TextInput::draw_caret` / `caret_visible` / `fits` (`ui/text.md` §15 r1–r3).
- `world_view/ui_bind.rs` — `UiRules::ui_image` documented to §22 r4
  (`panel_sprite` already is the plain cel draw); `Unspecified::ui_pass` = 11
  (`render/draw-order.md` §10) instead of an error.
- `ui/panel.rs`, `world_view/present.rs` — pointer button meanings
  (`ui/controls.md` §7 r5).
- `bridge/output.rs` — `CLASSES_4B1A10`, `f4b1a10(class)` (panels-2 §14 r8);
  `NpcDialog.f4b1a10` is now always captured.

### Single-player app / join
- New-character record at the join (`formats/d2s-load.md` §8 r1, r3):
  `Entry.record = new_character_record(portal_flags, None)`; portal flags from
  the `leveldefs` portal list (`WaypointTables::portal_levels`; synthetic =
  the 1.14d list → 1). The d2rs new character is not the stub path (no start
  stats / items / skill entries), so it owns no `StartSkill`: hands (0, 0),
  (0, 0), no extra 0x23.
- Store clock (`vendors.md` edge case 10): `WiredWorld::now := ms` in
  `WiredWorld::host_tick` (one line in **d2-server**
  `adapters/handlers/world/wired.rs`; its own `TODO(vendors.md edge case 10)`
  note removed). No test pins `now` across host frames.
- Save quests (`d2s-load.md` §2 quests row, `world/quests.md` §1.6): the
  save's three records `QuestFlags::copy_in(rec, true)` into the player's
  `PlayerQuests` (a stub keeps the new record).
- Unit-message rows wired: `single_player::client_unit_rows` (monstats +
  monstats2 record bytes +0x15… (`msg-units.md` §1.2 r7), itemstatcost send
  columns, objects.txt incl. `EnvEffect` / `Lit*` / `Selectable*` / color,
  shrines.txt codes) → `Bridge::set_unit_rows`, called by `play::run`. Before
  this the live app supplied no monster rows, so 0xAC created nothing.

### Bridge
- 0x51 (`msg-units.md` §1.3 r3): shrine object → shrines record of index
  interact (fatal 0x15F out of range, 0x37B bad code), `ShrineFx` OnMode at
  receive; §1.3 r5: types 0, 3, 4, 5 refused as never sent.
- 0xAC (§1.2 r2.1, r3, r4): hireling re-init (mode only, then rules 3–4 on the
  existing unit); rule 3 hireling exception (stats 6/7 kept) and flag-ex
  0x40000 clear for a hireling class in mode 1; rule 4 source-unit link
  (state 98, stat 354 := v, flag-ex 0x400; `v31` kept).
- 0x53 r4.4 (`render/lighting.md` §9.2 r4.4, OQ 11): day-period cache
  `env_period_cache` and the object refresh `0x004BC5E0` (EnvEffect objects:
  mode, flag 0x2 := `Selectable<mode>`, object light kind 2 radius `Lit`/2,
  removed for `Lit` 0), objects in bucket order.
- 0x74 (§7 r7.2): corpse `direction_of := P`.
- 0x93 `level_with_bonuses` clamped to the cap 99 (`skills/levels.md` §1 r3).
- Client room free (`model.md` §5 r5, `drlg/rooms.md` §8 r4): `room_freed`
  (flag 0x800000) and flag-ex 0x20 set; the update pass skips such a
  non-local unit and sends C→S 0x4B once, clearing both bits.
- `ClientUnit` gains `direction_of`, `flags_ex`, `room_freed`;
  `ClientWorld` gains `env_period_cache`; `ObjectRow` gains `env_effect`,
  `lit`, `selectable`, `rgb`; `ClientTables` gains `shrines`; `UnitRows`,
  `MonsterClass::from_record`.

### Tests (e2e)
- `tests/e2e_full_loop.rs` step 8: the item distance is the §9.5 unit
  distance of the path positions (player size 2, item size 1) instead of the
  staged 3; step 9 comment cites `pathing.md` §9.5 r3.

## What landed — audio (helper A; commits 69b6efe0, 35afe882, 7b3033bd, 7982b73b)

Every row of "Audio: sound table / wav", "environment", "triggers" done; no
`TODO(spec` left in `audio/` or `app/sound.rs`. Highlights: mixer device
gain and occlusion (§8.3 r1–r4) with `Cue::Device`; `game_loaded` option on
by default (§6.4 r1); pixel points, z + 640 (§5 r8, §8.1 r1); mode-0 layout 4
stereo + 12 mono (§7 r7); loop start / stream offset units (§7 r4, r8);
river point (§8.1); merged call attaches its unit (§5 r2); full cache
eviction, 25-tick preload phase (sound-table-2 §16, §10); variant pick
order (§7 r6); stream start failure (sound-table-2 §17 r2); unit request
lists (triggers-2 §19); environment `game_start`, `sound_init(-ns)`;
triggers identity type (§18), class-before-dead (§3 r2), footstep integer
types (§5 r9), NPC greetings table (`npc-greetings.tsv`), object rows
(§7 r7–r8), `jab_do` / `charge_start` (OQ 5), missile ProgSound functions
(triggers-2 §16), first-match 0x5D UI dispatch (§11).

## What landed — render (helper B; commit 6142c248)

Every row of "Render: hook declarations and rules" plus feed.rs:64 and
play.rs:60: hook docs cite their specs (implementations in `rules/`
checked against them); `NoFeed::open_mode` = 0 (panels-2 §22 r5); default
`fade_clock` instant on the host clock (draw-order §8, OQ 16); `flip_x`
always false; error messages of malformed input cite their rules.
Left in render files: only error template strings and true convention
lines. Note: `rules::unit_composite` is not wired into a `ViewRules`
component frame (the model lacks equipped items / component choices /
motion records; `model_feed::PENDING`).

## Changed test expectations
- `ui::tests::widgets_emit_requests_and_hit_by_rect`: the label's text
  request at (0, 50) (rect origin) → (4, 59) (its `pen`; panels-2 §22 r2).
- `ui::msg_ui_tests::npc_dialog_case`: a 2-entry NPC text list gave no case
  (`None`) → `B2 { m: 0x25 }` (PROVISIONAL first-entry m); new assert: empty
  list → m 0xFFFF.
- `bridge::msg::tests_ui_more` (0x8A): a present monster's `mdata_3c`
  `None` → `Some(-1)` (PROVISIONAL).

Audio (helper A; spec in parentheses): `request_fields_and_pool_limit`
position z 0 → 640 (§8.1 r1), no-unit (0,0,320) added; `fade_call` volume
40 → 100 (§5 r7); `stop_flag_stops_and_frees` cues +Param (§6.3 r6);
`fade_to_zero_runs_before_the_stop` [192,128,64,0] → [255,192,128,64,0];
`tracking_and_occlusion` z 640, tracking on by default (§6.4 r1);
`instance_rules` no-unit Defer Inst starts alongside (§6.3 r9);
`sixteen_channels_and_stealing` / `lower_priority_does_not_steal` 16 → 12
mono channels, victim (None, 80) → (0, 0) no fade (§7 r7–r8, §6.3 r6);
`async_only_*` start tick 1 → 25, renamed (§10 r6);
`runtime_fields_sample_load_state_and_last_use` +24 ticks;
`env_and_trigger_surfaces` play position 160 bytes → 40 units, z +640
(§7 r8, §5 r8); `engine_voice_log` +Param at tick 0;
`stereo_voices_get_no_pan_or_gain` stereo from the file (§7 r7);
`positional_start_and_unchanged_sends_nothing` 215 → 208, 107 → 103
(§8.2 r12); environment cue z 640 → 0; driver fixture gains a local player
(§4 r6).

Render (helper B): `feed::tests::no_player_no_camera_and_nothing_placeable`
open mode error → 0 (§22 r5). Lead, following B's changes:
`model_feed` `local_player_position_is_the_cell_centre` and
`open_mode_is_the_uis_once_handed_over` error → `OpenMode::NONE`;
`world_view::tests::unspecified_rules_draw_nothing_and_refuse_ui` now
refused at the UI image (ui/panels.md) instead of the UI pass.

Lead, join record (d2s-load §8 r3): `app_frame_loop` joined 15 → 18,
handled 20 → 23 (0x5F + two 0x23); `app_client_drlg` and `app_frame_loop`
fixtures give the bridge skill 0's row (synthetic data has no skills
table; 0x23 selects skill 0).
`bridge::tests::ui_answers_npc_dialog_with_0x31_in_order` and
`ui::msg_ui_tests::npc_dialog_answer_is_kept_for_the_bridge`: a 2-entry
list now answers B2 m 0x25 (PROVISIONAL first-entry m) instead of no case.

## PROVISIONAL (M22) — audio / render
- `audio/sound_table/system.rs` async read finish at the first preload pass
  after its start tick (sound-table OQ 10, 13, §10 r6) — recording ST-7.
- `audio/sound_table/system.rs` voice end on ticks × 40 ms (§6.6 r3, OQ 12)
  — recording ST-4.
- `audio/triggers` function 47 data +0x28 update inside the ProgSound > 0
  branch (triggers-2 §16) — triggers.md OQ 1 request log.
- `world_view/feed.rs` `event_shake` / `diablo_appears_shake`: only the
  0x5A code 0x12 site and client missile 372 at 150 frames left start a
  shake (camera §8 PROVISIONAL) — REC-62 (HIGH-PRIORITY, RNG order); not
  wired to 0x5A / the client missile layer yet.

## PROVISIONAL (M22) — lead's area
| Code | Choice | Settled by |
|---|---|---|
| `app/single_player.rs` `create_request` | 0x67 game name, template, arena, bytes 43–44 zero | packet capture of a real 0x67 (HIGH-PRIORITY, wire layout) |
| `bridge/msg/stats_items.rs` `ItemHeader` (+ cursor writes, ground position, `model_feed` cell, 0x74 step 3, 0x92, 0x3E stat 204) | item header = bit-stream head: flags 32, version 10, mode 3, ground x/y 16+16 or body 4, x 4, y 4, page+1 3; inventory placement taken as succeeding; body items = items whose last 0x9D names the owner with mode 1 | Ghidra 0x0062E410 + join/trade recording with items (HIGH-PRIORITY, wire layout) |
| `ui/msg_ui.rs` `first_m` | m = first entry's string id, empty list 0xFFFF | NPC dialogs with 2+ entries capture |
| `ui/msg_ui.rs` `ui_7c0c68` | stays 0 | Ghidra xref of `[0x007C0C68]` writers |
| `bridge/msg/roster.rs` | name compare exact bytes to NUL | Ghidra 0x00479360 |
| `bridge/world.rs` `is_dead` | flag 0x10000 read as the mode | Ghidra xref of +0xC4 writers |
| `bridge/world.rs` pet `extra` | only the three 0x81 values | hireling recording (0x7A/0x81) |
| `bridge/output.rs`, `msg/ui_npc.rs` `mdata_3c` | −1 for a monster | Ghidra xref of monster data +0x3C |
| `bridge/update.rs` | per-type client unit updates change no model field | Phase 6 unit-modes spec + client_update counter recording (HIGH-PRIORITY) |

## Coordinator follow-up (specs-staging-7 @ 431a3fd, read with `git show`)
Done (working tree): `render/lighting.md` §10 r4 — 0x89 id 13 now uses
the host wall-clock seconds (`ModelInputs::wall_seconds`, set by the app),
no RNG draw (was PROVISIONAL seed draw); `client/model.md` §14 r3 — 0x81
type 7 with the hireling in S: flags 0x2 / 0x200, mode 1 (`flag_200`, which
0xAB now tests instead of the provisional "clear"); `client/stat-lists.md`
§3 r2 — stat 172 changes the room's allied count (`room_allied`, fatal
0x27C); `client/msg-ui.md` OQ 8 — `end_of_game_result` (front-end only);
`render/camera.md` §8 — `diablo_appears_shake` (the second known site).
Not done: `client/stat-lists.md` §3 r6.9 (client monster re-init with seed
draws), r6.10 (blood spray, effects), r6.11 (skill item test); `ui/text.md`
§15 r5–r6 (edit-box keys, scroll window); `ui/automap.md` OQ 1–4 (no automap
code in my rows); the 0x81 setup's hireling skills / light / dead-flag
clear (render / no model field). The merge of origin/claude/specs-staging-7
was not made: commits are blocked in this session (see below).

## Rows left (lead's area), with reason
- `bridge/skills.rs:59`, `msg/tests_outputs.rs:153`, `bridge/world.rs`
  `total` (stat-lists §1 r1–r2, r4, §3 r6; msg-skills §2 r4): the passive
  refresh needs `eval_skill` over a client `SkillUnits` world and the full
  `SkillTables` + skill code buffer in the client, and §1 r2 a d2-sim stat
  list per client unit; a feature task, markers kept.
- `bridge/msg/units.rs` 0xAC rule 6 / 6.8 (monster set-up `0x004AE8D0`,
  monstats Skill columns): needs monstats Level / resist / Velocity / Skill
  columns, classic scaling and the animation frame count of the r6.5 seed
  draw; documented in code, markers removed in favour of a pointer here.
- `bridge/msg/units.rs` 0x15 rule 4.5 (nearest free point): needs client
  collision maps; marker kept.
- `world_view/ui_bind.rs` `UiFrame::unhandled` (controls §6): the world-click
  dispatcher's action needs client path compute and collision; marker kept
  (re-worded: answered, not built).
- Inventory grid drawing (items, tints of `ui/inventory.md` §2–§4, §6, §8):
  the `CellGrid` answers are in; no inventory-grid panel exists and `UiDraw`
  has no filled-rectangle request yet.
- `ui/text.md` §15 r4 (selection fill): `TextInput` keeps no selection.
- Not in a row but noticed: `d2-sim` `path::walk::geom::unit_distance`
  clamps a negative `dist8_unit` entry to 0 and still applies the size
  adjustments; `pathing.md` §9.5 says a negative entry returns 0 at once.
  Owner: impl-triage-sim.
- 0x9C action 1 GroundToCursor on an item already in S: §2 r5.2 says fatal
  0x3A0, not enforced: the d2rs server's pick-up sends no room delete
  notice (0x0A) before it, so the e2e pick-ups would fail; the record
  replaces the unit. Owner of the server side: impl-triage-items.
- Audio (helper A, outside its rows): `movement.rs` `neutral` compares
  group bases, not the §19 r3 range walk; `modes.rs` uses the raw unit type,
  not the §18 identity; `driver.rs` `state_duck` doc still cites OQ 6
  (answered by §6.5 r2); `soundchaosdebug` (§6.2 r3) not implemented.
- `drlg/rooms.md` §8 r4 also zeroes a dynamic-path unit's position on the
  room free; not done (not a row).
