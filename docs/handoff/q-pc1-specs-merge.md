# q-pc1-specs-merge: PC 1's spec branch into staging

Branch `claude/q-pc1-specs-merge`. Merged `origin/claude/local-pc1-s8`
(PC 1 Lane B round 2: specs, HANDOFF §5 Done blocks, `// Covers:`
unlocks, new build-queue rows; 129 files, +10.6k / −1.9k lines) with a
merge commit (no rebase), then made the code follow the changed specs.

## Merge

- One conflict: `specs/client/model.md` section index → took theirs,
  re-ran `python3 tools/spec_index.py`. Staging's REC-288 PROVISIONAL
  paragraph (play preview walk prediction, S→C 0x0D code 1 walk-outs)
  survived the merge, after "settled by REC-51." in open question 2.
- `docs/HANDOFF.md` merged cleanly (no union needed).

## Code that followed the spec

`specs/client/bridge.md` §10 r3.1 (answers its open question 6) and the
new §10 table row `UnitFreed`; the `ServerSound` row now carries unit
x, y. This is also the whole of queue row `q-fix-audio-unit-freed`
(the coordinator can mark it done).

- `bridge/output.rs`: `Output::UnitFreed { unit }` (row 41 of 43,
  producer 0x0A, consumer audio); `ServerSound` gains
  `at: Option<(u16, u16)>` (the unit's model position at receive;
  `None` when the unit has none). `move_freed` turns the model's free
  list into `UnitFreed` outputs in free order.
- `bridge/world.rs`: `ClientWorld::freed`; `ClientWorld::remove` (the
  one free path: 0x0A, 0x74 corpse body items, 0x42 cursor item, `add`
  replacing a key) records each free.
- Producer: `receive_chunk` (after each general handler, also on a
  rejected one: the unit is gone either way), `update_pass` (after each
  drained message, each object update, the C objects, the DRLG update),
  `Bridge::frame` end and `take_outputs` (frees made outside handlers).
  Every current free path frees after the handler's own outputs, so
  appending after the sink keeps 1.14d call order.
- Consumer: `world_view/present.rs` maps it to
  `SoundRequest::UnitFreed`; `audio/driver.rs` applies
  `detach_all(U, false)` (`audio/triggers-2.md` §19 r5; the
  `0x004CC160(U, −1)` lock release is cache only, no call). A
  `ServerSound` request records its captured position, and
  `ModelSoundWorld::position` uses it when the key no longer resolves
  (§10 r3.1 (b)).
- Tests: the four `bridge::output::tests` now pass against the spec
  table (row count 42 → 43, as the table says); new
  `every_model_unit_free_appends_one_unit_freed`,
  `a_unit_freed_output_detaches_the_units_requests`,
  `a_freed_units_position_is_the_one_its_sound_captured`. Changed
  expectations (spec-given values): `outputs_keep_message_order_and_capture_at_receive`
  now ends with the 0x0A's `UnitFreed` and carries the captured
  position; the two object-13 0x2C tests carry the object's position
  (0x1214, 0x11C0).

No other spec-vs-code check broke. No PROVISIONAL added (REC-289 unused).

## Gate

`cargo fmt --all --check`, `cargo clippy -p d2-sim -p d2-server -p
d2-client -p test-fixtures --all-targets -- -D warnings`, `cargo nextest
run` on those four (all pass), `python3 tools/coverage.py --check`,
`python3 tools/spec_index.py --check`.

## New queue rows from pc1 (not implemented here)

Already fixed on `origin/claude/local-pc1-play` (commit 4dac88b8 "fix
the live-data render crash, the exit crash, the start skill and run
position refusals"): `q-fix-play-gradient-block` (live-data render
panic), `q-fix-play-exit-resource` (`--frames` exit panic),
`q-fix-set-skill-fatal` (S→C 0x23 SetSkill fatal 0x668). Done here:
`q-fix-audio-unit-freed`.

| Row | What |
|---|---|
| `q-fix-weapon-switch` | WEAPON SWITCH BODY TO SPEC |
| `q-fix-client-set-lists` | CLIENT SET-ITEM LISTS AND SET BONUSES |
| `q-fix-moving-skill-do` | MOVING SKILLS RUN THE DO ON THE WAY |
| `q-fix-item-type-test` | SKILL ITEM TYPE TEST 0x00643F80 |
| `q-fix-smite-shield` | SMITE SHIELD LOOKUP |
| `q-fix-dual-weapon-damage` | DAMAGE WEAPON AND DUAL STAT SWITCH |
| `q-fix-sentry-ai` | LAID TRAP AI IN PLAY |
| `q-fix-esc-menu` | ESC GAME MENU TO SPEC |
| `q-fix-sp-pause` | SINGLE-PLAYER PAUSE WHILE UI 9/11 OPEN |
| `q-fix-charselect-wheel` | CHARACTER SELECT MOUSE WHEEL |
| `q-fix-act-change` | ACT CHANGE AS 0x0053ACC0 |
| `q-fix-portal-pair` | PORTAL PAIR CREATION 0x0056D130 |
| `q-fix-create-flags` | 0x67 HARDCORE FLAG |
| `q-fix-recharge-order` | RECHARGE LIST ORDER |
| `q-skill-fallback` | SKILL FALLBACK IN BRIDGE FRAME |
| `q-fix-pet-palette` | PET PASS PALETTE LEVEL INVERTED |
| `q-fix-dead-flag` | CLIENT DEAD FLAG 0x10000 |
| `q-fix-client-stat-callback` | CLIENT PLAYER STAT CALLBACK (ITEM SKILLS AND CHARGES) |
| `q-drop-stale-provisional` | DROP STALE PROVISIONAL MARKERS (pc1-s8) |
| `q-fix-skill-msg-level` | SKILL MESSAGE LEVEL BYTE |
| `q-fix-door-highlight` | C→S 0x3D DOOR HIGHLIGHT |
| `q-fix-monster-update-steps` | MONSTER UPDATE STEPS 5, 8-10 |
| `q-fix-belt-ui` | BELT LABELS, HOVER TIP AND RECTANGLES TO SPEC (REC-240) |
| `q-fix-gold-prompts` | GOLD DIALOG PROMPT BY KIND |
| `q-fix-hud-popup` | CONTROL-PANEL POP-UP TEXT AND GLOBE NUMBERS (REC-238) |
| `q-fix-level-connections` | REAL LEVEL CONNECTIONS IN THE SYNTHETIC WORLD (REC-230) |
| `q-fix-player-mode-msgs` | PLAYER MODE UPDATE MESSAGES PER 1.14D TABLE (REC-95) |
| `q-fix-shadowmaster-scan` | SHADOW MASTER SCAN AND AITYPE 12 |
| `q-fix-monster-mode-missile` | MONSTER MODE MISSILE |
| `q-fix-edit-scroll` | EDIT BOX SCROLL WINDOW CASE 1 AND 3 |
| `q-fix-d2s-token-ref` | REAL 1.14D TOKEN REFERENCE TABLE |
| `q-fix-format-lookups` | TBL KEY LOOKUP AND MPQ LOCALE AS 1.14D |
| `q-controls-ingame` | CONFIGURE CONTROLS IN GAME |
| `q-options-art` | OPTIONS MENU ART |
| `q-fix-shake-starts` | SCREEN SHAKE STARTS (REC-62) |
| `q-rec-hooks` | RECORDER HOOK POINTS FOR THE PC 2 LIST |
| `q-rec-joinflags` | HARDCORE FORCED START AND JOIN RESULT |
| `q-rec-d2sitems` | D2S ITEM QUALITIES |
| `q-fix-shrine-states` | SHRINE STATES THROUGH THE STATE HELPER (REC-239) |
| `q-fix-object-label` | OBJECT MOUSE-OVER LABEL FROM 0x00454F30 (REC-239) |
| `q-fix-object-overhead` | DRAW OBJECT OVERHEAD BUBBLES IN PLAY (REC-239) |
| `q-fix-boss-restore-origin` | BOSS RESTORE NEAREST-FREE ORIGIN |
| `q-fe-text-widths` | FRONT-END TEXT WIDTHS |
| `q-fix-calc-missile-rand` | MISSILE RAND SEED |
| `q-fix-warp-tile-restore` | WARP TILES FROM THE PRESET PASS AND THE INACTIVE RESTORE (REC-230, REC-99) |
| `q-fix-state-tint` | STATE TINT SELECTION AND LOCAL PLAYER LIGHT COLOUR (REC-245) |
| `q-fix-preview-light` | PREVIEW LIGHT: WALL FADE, OPAQUE ROOFS, BLOCKS-LIGHT, NEAR-ROOM AMBIENT (REC-247) |
| `q-fix-dcc-limits` | DCC FRAME SIZE LIMIT AND CODED BYTES |
| `q-fix-questlog-latch` | QUEST-LOG LATCH AS 0/1/2 WITH ITS RESETS |
| `q-fix-shrine-onuse` | SHRINE ON-USE EFFECTS |
| `q-fix-tp-use` | TOWN PORTAL USE THROUGH THE ITEM-USE DISPATCHER (REC-117) |
| `q-fix-just-portaled` | STATE 102 ON PORTAL USE (REC-243) |
| `q-fix-vendor-buy-copy` | VENDOR BUY 0x9C ACTION 4 FOR THE COPY (REC-243 (4)) |
| `q-fix-cube-panel` | CUBE PANEL WITHOUT THE CUBE, AND ITS TOOL TIPS (REC-244) |
| `q-fix-save-gaps` | SAVE GAPS TO SPEC (REC-241) |
| `q-fix-stale-runeword` | EQUIPPED STALE RUNEWORD ON LOAD (REC-241) |
| `q-fix-los-direction` | SKILL LOS LINE RUNS FROM THE TARGET POINT TO THE CASTER (REC-248) |
| `q-fix-sweep-counts` | DS1 AND DT1 SWEEP COUNTS AND THEIR SPECS (Lane B 2026-10-08) |
| `q-fix-lvlprest-beyond-files` | LVLPREST ROWS NAMING A FILE BEYOND FILES |
| `q-fix-item-create-sweep` | ITEM-CREATION SWEEP FAILURES |
| `q-fix-cold-plains-rooms` | COLD PLAINS ROOM COUNT |
| `q-fix-wired-host-walk` | GAME_WIRED_HOST WALK TO THE TOWN WAYPOINT |
| `q-fix-placement-reader` | PLACEMENT READER ROOM DATA |
| `q-fix-preset-monster-ai` | PRESET MONSTER AI START (REC-254; supersedes the AI half of q-a2-duriel-ai) |
| `q-fix-lair-entrance` | DURIEL'S LAIR ENTRANCE AND EXITS (REC-234) |
| `q-fix-quest-items` | QUEST ITEM DELETE, WEAPON IN USE, QUEST DROP SPOT (REC-235) |
| `q-fix-options-effects` | OPTIONS ROW EFFECTS (REC-187) |
| `q-fix-fe-buttons` | FRONT-END BUTTONS TO SPEC (REC-189) |
| `q-fix-create-popups` | CREATE SCREEN POPUPS AND TEXTS (REC-231) |
| `q-fix-controls-ingame` | CONFIGURE CONTROLS IN GAME TO SPEC (REC-256) |
| `q-fix-item-tip-layout` | ITEM TIP BLOCKS, LINES AND STORE PRICE LINE (REC-242) |
| `q-fix-item-tip-props` | ITEM TIP PROPERTY ENGINE AND SET LINES (REC-242) |
| `q-fix-equip-reqs` | REQUIREMENT CHECK AND CLIENT BODY-CLICK PRE-CHECK (REC-253) |
| `q-fix-warp-tile-unit` | LIVE WARP TILE UNITS (REC-249) |
| `q-fix-quest-item-msg` | QUEST ITEM USE SENDS 5D (REC-246 (a)) |
| `q-fix-summit-gate` | SUMMIT EXITS PER 0x0058D090 (REC-246) |
| `q-skill-sequences` | PLAYER SKILL SEQUENCES (REC-232) |
| `q-fix-cain-caption` | CAIN IDENTIFY CAPTION FROM THE CLIENT QUEST RECORD (REC-255) |
| `q-fix-body-clicks` | INVENTORY BODY-LOCATION CLICKS |
| `q-fix-merc-panel-items` | HIRELING PANEL AND PORTRAIT ITEM CLICKS |
| `q-fix-client-missiles` | CLIENT MISSILE EFFECT LAYER |
| `q-fix-monster-modes` | MONSTER MODE MACHINE AND CLIENT MODE STEPS |
| `q-fix-count-minimums` | RELAX TABLE COUNT CHECKS TO THE CODE'S LIMITS |
| `q-fix-elixir-tip` | ELIXIR PROPERTY TEXT |
| `q-fix-skill-stat-callback` | SERVER SKILL STAT CALLBACK IN THE PLAY HOST (REC-266) |
| `q-fix-preview-skill-rest` | PREVIEW REST SKILL SEAMS TO SPEC (REC-266) |
| `q-fix-namekey-high` | MAP NAME-KEY BYTES >= 0x80 AS 1.14D INSTEAD OF E11 |
| `q-fix-auto-tc-names` | AUTOMATIC TC NAMES FROM THE ITEMTYPES LINKER'S STORED KEY |
| `q-fix-ground-announce` | GROUND ITEMS ANNOUNCED BY THE UNIT UPDATE, NOT A TRACKED SET (REC-260) |
| `q-fix-stat-hook` | STAT HOOK AND REQUIREMENT REFRESH TAIL |
| `q-fix-audio-unit-freed` | AUDIO UNITFREED OUTPUT |
| `q-fix-client-quest-record-writers` | CLIENT QUEST RECORD UI WRITES |
| `q-fix-client-unit-flags` | CLIENT UNIT FLAG WORD |
| `q-fix-play-gradient-block` | PLAY ON LIVE DATA PANICS IN THE RENDER SYSTEM (LOCAL-RUN 4.2, C40/C53; 3 of 3 runs, exit 101): "draw item 653: drawn area Rect{560,296,32,32} leaves the gradient block Rect{560,296,32,15}" at crates/d2-client/src/scene/mod.rs:141 |
| `q-fix-play-exit-resource` | --FRAMES EXIT PANICS: crates/d2-client/src/app/play.rs:521 resource_mut::<WorldViewState>() after app.run() (resource missing), exit 101, seen in LOCAL-RUN 4.1 synthetic play; expectation is exit 0. |
| `q-fix-set-skill-fatal` | S->C 0x23 SETSKILL REFUSED WITH FATAL 0x668 ("skill outside the table") in the session join: twice in synthetic play, three times in app_client_drlg the_session_join_on_the_install (C86) |
| `q-fix-panel-horadric-offsets` | GAME_PANELS HORADRIC FRAME OFFSETS: panel_files_frame_counts_and_sizes sees menu\horadric frame 1 offset (-205, 17); test and ui/panels.md §7 r2 say (0, 0) (game_panels.rs:99; the assert stops at the first failure, other frames may differ) |

## Local check

None needed beyond the gate (synthetic). In `play`, nothing visible
changes (sound is deferred); a 0x0A on a unit now shows a `UnitFreed`
in the frame's outputs.
