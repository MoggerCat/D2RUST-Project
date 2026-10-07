# Handoff: PC 1's final spec batch in code — `claude/impl-pc1-final`

Cloud implementation session, 2026-10-07. Repo only, no game files.
Base: `claude/specs-staging-3` (PC 1's `local-pc1-integration` 8654215,
then 4f3fade merged in mid-session), finally merged with
`claude/specs-staging-5` (staging-3 + PC 2's staging-4). Two subagents:
AI bodies, client bridge handlers. Everything below is **implemented,
unverified** (no trace or game-file run).

## Decision recorded for PC 1

- **`client/msg-ui.md` Open question 10 was decided as option A by the
  user** (relayed by the coordinator, 2026-10-07): `client/bridge.md` §10
  r6 stands. For 0x28's NPC-dialog branch the UI consumer does not write
  the model; it hands the branch case back and the bridge applies the
  model writes and sends C→S 0x31 with 1.14d's bytes and order. PC 1:
  record the answer in `msg-ui.md` OQ10 / §16 r5. Code: 0x28 reserves an
  `outgoing` slot right after its 0x2F (`world::DIALOG_REPLY_SLOT`);
  `Bridge::send_outgoing` stops at the slot; `Bridge::npc_dialog_branch
  (dialog, DialogCase)` applies `msg::ui_npc::apply_dialog_branch` (B0:
  flag 0x2 := 0; else `interact` class ∉ {537, 538, 539, 527} → U mode 1
  and turned toward the local player; B2: 0x31 `31 G m` into the slot;
  B3–B6: class ≠ 527 and local mode 1 / 5 → the local player turns toward
  U, U's path stops) and flushes. An unanswered slot is dropped at the
  next frame (no 0x31). Reading taken: a holding case ends the branch
  (B0 does not run the "(always next)" rows). The model has no client
  path record, so `ClientUnit::{turned_toward, path_stopped}` keep the
  turn's input and the stop call. Tests: `tests_ui_more::
  dialog_branch_writes_and_0x31_in_order`, `bridge::tests::
  dialog_reply_slot_holds_the_send_order`. **Open:** the UI layer does
  not yet evaluate the case (its UI state `[0x007C0C68]`, the NPC text
  record are not modelled; `ui/msg_ui.rs` skips `NpcDialog` by name), so
  the running app never answers and sends no 0x31.

## What changed, per PC 1 item

| Item | Code | Tests |
|---|---|---|
| Message renames (`AddStatPoint` stat/repeat, Buy/Sell/Repair fields, CreateGame, StaffInOrifice, Chat, Ping, S→C layouts) | generated.rs was already regenerated; client panel `add_stat_point`, client tests (`e2e_*`, `prop_worldsim`); staging-5's `count_minus_one` literals renamed to `repeat` | d2-proto `check_reports_exactly_a_changed_row` (PC 1's 4f3fade version kept) |
| Dangling claims (bodies-2 §44) | claims re-pointed at `monsters/umod-callbacks.md` §3.1; `FindFilter::room_flags` (G): the town-room skip tests G, not F; the corpse effect sets G := flags | `missiles/tests/ext.rs` unit-find tests |
| umod `elemental()` (`umod-init-bodies.md` §2) | monlvl row clamped to 1..rows−1, empty table returns before the tail; length stats on the clamped row; teleport (§4) bosses only | `init/tests.rs` `elemental_umods` (passes unchanged) |
| Boss mods `init.md` §14.3 | `create::boss_mods_for` (every case incl. uber branches), summoner flags, ancient barbarian equipment through seams `item_tier_code` / `create_boss_item`; `NamedIds::bloodraven` removed (nothing set it) | `boss_mods_case_table`, `boss_mods_summoner_and_ancient_equipment`; `boss_mods_bloodraven` **corrected**: BaseId 267 (was a fake id 250, now the summoner's case) |
| `hit.md` §6.4 weapon block | layer 0 assigns unconditionally, typed entries take the signed max | spec's three vectors; `dodge_avoid_evade_weapon_block` **corrected** (entry order: a trailing layer-0 entry now lowers the value) |
| `damage.md` §5.1 mode-0 attacker | returns at once, record kept, at step 4.1 and step 6 | `apply_melee_hit_steps` **corrected** (record stays) |
| `0x0066A220` no-op, kill §7.2 | `wiring::action::reaction::kill` in spec order: player guards too, distribution unless unit flag 0x04000000, arena event, then the monster death steps | `death.rs` `kill_without_experience_and_player_victims` |
| `vitals.md` §4.3–§4.7 | `combat::vitals::experience`: gain (`ExpRatio`, stat 85), distribution (credited player, hireling and party share as seams; party quotient float32 emulated exactly in integers), add with `lastexp` and L0, gold / experience death penalties, corpse experience and pickup | `experience_tests.rs` (incl. an IEEE cross-check of the float emulation); `add_experience_caps_and_levels` **corrected** (gain `i32::MAX`: the add is now 32-bit wrapping, `u32::MAX` would wrap) |
| `combat/events.md` | `combat::events`: functions 1–14, 17–31, item cast `0x005FDCA0` / `0x005FDD60` and core, Reanimate raise, iteration `0x005C0C30`, on `BodyWorld` + `EventWorld`; `use_::start_core_no_mana` | `events_tests.rs` (every spec test vector) |
| AI rows (all 148 spec'd-here) | subagent: `monsters/ai/bodies6.rs`, `bodies7.rs` (55 thinks + inits + Uber alternates), seam `AiSummons`, spawn info `common::spawn_info` (§13.1), fixes from the AI spec diffs | `tests/act6.rs`, `tests/act7.rs`; corrected: `specd_here_check_catches_perturbations`, `install_sets_think_and_alternate`, mutant `install_init_only…`, `stub_ai_logged`, `vulture_carrion_and_rooms` (now `should_panic`), `imp_combat_and_fire`, `baal_clone_spawns_a_third`, `sand_maggot_queen_lays_and_rests` (each because the spec now says otherwise) |
| Skill slots | `functions.tsv` unchanged; `skills/use_/tests/bodies.rs` and `missiles/tests_bodies.rs` pass as is | — |
| Umod callbacks | `umod-callbacks.md` §15.1 purge and every §15.2 quest-death body (seam `quest_death` replaced by `is_undead`, `missile_range`, `set_uber_death`, `game_8c`, `quest_drop`) | `questcomplete_calls` extended |
| Other newly spec'd init | `init.md` §20.1 per-hcIdx cases (seam `superunique_quest` replaced), §26 `make_unique` / `warp_eligible` / `nearest_eligible`, §27 `reinit` | `superunique_hcidx_cases`, `make_unique_and_the_warping_pick`, `class_reinit`; `superunique` **corrected** (hcIdx 3 has no case: no `su_quest` log) |
| Client bridge owners | subagent: a handler for every owned S→C id (167), new model fields and outputs | tests corrected: unowned-id fixtures moved to out-of-scope ids (0x61 / 0x5F are owned now), owned count 62 → 167, struct literals |
| d2-server gaps | `server_none_rows_include_the_never_queued_ids` (the 14 ids, 0x77 `sim`) | — |
| Cold Plains 97 vs 98 | not changed: `outdoor.md` pins the 98-room build, not our 97's cause; needs the C92 output (local queue) | — |
| Conflict markers in CI | `tools/conflict_markers.py` (+ `--selftest`), `tools/gate.sh` step, CI `check` job step | selftest |

## Open items

1. `combat::events` has no caller on the action wiring yet:
   `Pending::unit_event` still has no registry; route it to
   `events::run` with an `EventWorld` provider (handlers live in
   `ActionHooks::handlers`).
2. Death penalties / corpse experience (`vitals.md` §4.6–§4.7) are
   functions only; the player-death path (`0x00580F59`), corpse creation
   (`0x0057F700`) and pickup (`0x0057FB70`, the `MovePending::
   corpse_pickup` seam) are not wired.
3. Party-share x87 precision control is `sim/stat-lists.md` OQ1; the
   product is taken at full precision (TODO in `experience.rs`).
4. The staging-5 `vitals.md` §4.4 note (credited-player lookup gated by
   the `exp` state group) is for the `kill_credited_player` provider.
5. C→S 0x67 refusal rules (`intents-events.md` §2.5 table) are not
   implemented: the name checks `0x0053EFC0`, `0x00538B70`,
   `0x00538C60` have no spec.
6. AI: special state 6 think `0x005E7C10` stays a logged stub (no spec);
   the subagent's ambiguous readings carry `TODO(spec: ai-bodies-6/7…)`
   (pet move case 0, turret second skill check, wolf follow, CycleOfLife
   entry, QuillMother params 3–4, InvisoPet without owner, ShadowWarrior
   pettype, Raven level, ShadowMaster scoring).
7. Client: the app supplies no `objects`, `states`, `skilldesc`,
   `overlay_count` tables nor `expansion_installed`, so those handler
   parts are skipped in the running app (see the bridge subagent's TODOs).

## Local checks (queue)

- `cargo test -p d2-server --lib
  world_data::tests::game::outdoor_levels_generate_through_the_dispatcher
  -- --ignored` with `D2_GAME_DIR` (C92): the first grid
  difference names the cause of 97 vs 98 rooms.
- `combat/events.md` OQ1 and `vitals.md` §4.7 recordings (already in
  HANDOFF §5).

## Gate

See the coordinator message for the final `CARGO_INCREMENTAL=0 sh
tools/gate.sh` result on the pushed head.
