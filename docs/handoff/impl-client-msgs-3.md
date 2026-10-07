# Handoff: client S→C handlers for UI, audio and skills, the bridge output channel — `claude/impl-client-msgs-3`

Cloud implementation session, 2026-10-07, task class: implementation from
specs, medium. Base: `claude/specs-staging-2` at `ddbfe0b` (M09: every
claim below is about this branch on that base; repo only, no `game/`, no
`re/`). Read: `specs/client/bridge.md` §6, §10; `bridge-dispatch.tsv`;
`client/msg-ui.md`; `client/msg-skills.md`; `client/stat-lists.md`;
`audio/triggers.md` §2, §3, §11; `render/lighting.md` §9;
`ui/panels.md` §2–§4, §8, §11–§13; `world/waypoints.md` §2;
`skills/levels.md` §1, §6; `client/model.md` §1–§2, §7.

## 1. Result

The nine ids PC 1 gave owners now have handlers, registered in
`bridge::msg::HANDLERS`; `Dispatch::from_spec()` builds again, so the 85
`d2-client` reds of the base (all `TableError::Mismatch` / `NoHandler`)
are green.

| Id | Owner | Handler | Model part | Output |
|---|---|---|---|---|
| 0x21 | `client/msg-skills.md` §4 | `msg::skills::update_item_oskill` | assign (§2 r2), skill-tree flag `[0x007C0C3C]` := 0 (`ClientWorld::skill_tree_flag`) | – |
| 0x22 | §5 | `update_item_skill` | quantity of the native entry; none → fatal 0xAD6 | – |
| 0x23 | §6 | `set_skill` | select left / right (§2 r3); bad skill → fatal 0x668 | – |
| 0x94 | §3 | `base_skill_levels` | assign each entry (remove 0) | – |
| 0x2C | `audio/triggers.md` §2 r4 | `msg::sound::play_sound` | none | `ServerSound` (key, class at receive, event) |
| 0x53 | `render/lighting.md` §9.2 r4 | `msg::lighting::darkness` | the act's environment record (r4.1–r4.3, r4.5) | – |
| 0x5D | `client/msg-ui.md` §1 | `msg::ui::quest_status` | `quest_untargetable` (r4), eclipse (r3), `exit_requested` | `QuestUi` for the output rows |
| 0x63 | §2 | `waypoint_menu` | none | `WaypointMenu` |
| 0x77 | §3 | `trade_action` | none | `TradeAction` |

New code (each module names its spec):

- `bridge::output` (`bridge.md` §10): `Output` (the four variants of the
  §10 table), `Outputs` (the per-message sink the handler reaches through
  `Message::out` / `UnitMessage::out`, so handler signatures are
  unchanged), `dispatch` (list order, one consumer per variant), and the
  §10 r8 check: `parse_table` reads the §10 rows of `bridge.md` and
  `check` compares them with `ROWS` (test `variants_match_the_spec_table`,
  perturbation test `a_changed_row_is_reported`, M08).
  `receive_chunk` / `update_pass` take `&mut Vec<Output>`; a rejected
  message's outputs are dropped with its effect. `Bridge` keeps the list
  (`outputs`, not a `ClientWorld` field), `FrameReport::outputs` counts
  them, `Bridge::take_outputs` hands them over. In Bevy, `bridge_frame`
  moves them into the `FrameOutputs` resource after the frame (§10 r4);
  `world_view::present::deliver_outputs` (PreUpdate, after
  `bridge_frame`, before the input and UI systems) applies them in order:
  UI outputs to `OriginalUi::apply_output`, whose sounds are appended at
  once, and `ServerSound` to the sound request list, so `UiSounds` holds
  one ordered list (`Vec<SoundRequest>`, was `Vec<i32>`).
- `bridge::skills` (`msg-skills.md` §1–§2): `SkillList` (entries in list
  order, left / right / current as indices), `add`, `assign`, `remove`,
  `select`, `refresh`. `ClientUnit::skills` (players at 0x59, monsters at
  0xAC step 7 get an empty list); `ClientTables::skills: Vec<SkillRow>`
  (`anim`, `monanim`, `passivestate`, `maxlvl`; the skill count is the
  row count, so no table = every id outside it, as 0xAC does with no
  monster rows). `play` fills the rows from the user's `skills` table
  (`app::single_player::client_skill_rows`, `Bridge::set_skill_rows`).
- `bridge::msg::lighting`: the act's environment record is created with
  the act by 0x03 (`create_environment`, §9.1; +0x10 `GetTickCount` has no
  client reader, the bridge reads no clock: 0) and held as
  `ClientWorld::environment`; 0x53 and the 0x5D eclipse go through
  `rules::lighting::environment::Environment::set_from_server`; the
  pending flag `[0x007A060E]` is `ClientWorld::eclipse_pending`, turned
  into the setter call by the act-2 load (§9.2 r3).
- `ui::original::msg_ui` (`msg-ui.md` §1 r2, r5–r7, §2 r2, §3 r2–r3):
  the UI dispatch at delivery on `MsgUiState` (quest-log latch,
  `[0x007BF2AC]`, `[0x007BC9D8]` / `[0x007BC9D4]`, the waypoint menu's
  GUID / record (load copy through `d2_sim::world::waypoints::
  WaypointRecord::load_copy`) / tab / close latch, trade state,
  `[0x007BCE28]`, `[0x007C0E80]`, inventory mode). Stash (0x10, 0x11) and
  cube (0x15) set their flag and inventory mode; the stash and cube
  panels themselves are still not installed (`PENDING`). Parts without a
  spec are not run and are named in `UiOutcome::skipped`
  (`msg_ui::skip::*`, logged at debug by the dispatcher).
- `audio::driver`: `SoundRequest` (`Ui`, `Server`, `PlayerEvent`) and
  `SoundDriver::frame(world, levels, &[SoundRequest])`. Server events run
  `audio::triggers::events::server_event` with P = the local player at
  delivery; the events whose input the driver does not hold (12, 16 on a
  monster, 17, 18) and the follow-ups (overhead text, quest stingers,
  stinger re-arm) are skipped and named (`SoundDriver::take_skipped`,
  logged at debug in `app::sound`). `DriverError::Trigger` carries the
  fatal paths (player class ≥ 7).
- `ClientWorld::base` / `total` (`client/stat-lists.md` §1 r3) and the
  0x19–0x1F adds read `total` (`msg::stats_items`).

`ui::original::PENDING` and `audio::driver::PENDING` are updated: the
"no client handler" reasons for 0x2C, 0x5D, 0x63, 0x77, 0x94 are gone;
what still blocks each panel / feed is named.

## 2. What stays Pending (not guessed)

1. **Passive skills** (`msg-skills.md` §2 r1 state on, r2.2 state off,
   r4 refresh): the client holds no state bits and no stat lists, and
   `0x00643620`'s list creation does not give the list flags or the
   attach `reset`. A skill with `passivestate` > 0 makes the operation
   fail with `SkillError::PassiveState` *after* the list is updated
   (handler error `TODO(spec: …)`). Level-1 joins in the recordings carry
   no passive skill.
2. **Client stat list** (`stat-lists.md` §1 r2): not built as a `d2-sim`
   list: its value callback is the client callback `0x004609F0`, whose
   effects are open question 1, and no client rule can attach a list yet
   (items: the item stream, OQ 2; states: 0xA7–0xA9 stay `TBD`; passive
   skills: item 1). So `total` = `base` (§1 r3 says so until lists are
   attached). The character panel values stay in `PENDING` for the other
   inputs they need (resist effects, expansion resist penalty, language,
   popup width, string lookup in the original UI).
3. **0x53 day-period refresh** (§9.2 r4.4): the period `p` of
   `0x0061C100(act, 0)` is not given as a function of the record, and the
   object refresh `0x004BC5E0` is lighting OQ 11: not run, cache not held.
4. **Eclipse** (0x53 with u8@9 ≠ 0, the 0x5D Tainted Sun row with a client
   act, the act-2 load of a pending eclipse): the setter's eclipse branch
   calls `0x0061BDF0`, undescribed: handler error `Unspecified`, record
   unchanged. The `msg-ui.md` vector `5d 0a 01 00 0000` "eclipse set" is
   therefore a rejection test here (`quest_status_eclipse`).
5. **0x5D UI parts**: screen messages (`0x0049E3A0`), `0x0046F870(211,
   1)` (OQ 1), the video-5 / character-record path of f bit 1 c 23, the
   video-7 flag (`render/composition.md` §4 is not wired to these flags),
   the Den counter path (client quest flags, OQ 4), and every use of the
   quest-log table `0x00723F30` (not in the specs).
6. **0x63 UI parts**: the input reset `0x0044DA40`, the row rebuilds
   `0x0049C7F0`, the tab gate for an act index 1–4 (client quest flags:
   `WaypointMenuState::tab` = `None`), and the panel itself.
7. **0x77 UI parts**: codes 0x00–0x02, 0x05, 0x06 (trade helpers, OQ 5),
   0x0A (`[0x007C0E60]` has no writer), the `0x00487B30` call when ui 23
   is open, and `0x00463DF0` before the inventory toggle of close
   trade(1) (code 0x0D).
8. **0x2C events** 12 (`stsound`, triggers OQ 4), 16 on a monster
   (`monsounds`), 17 (flee voice), 18 (greeting record), and every
   follow-up (overhead text, stingers): skipped, named.
9. **Monster skills at 0xAC** (`msg-units.md` §1.2 r8): the `monstats`
   Skill / level / mode columns are not in the client tables; monsters get
   an empty list (TODO in `msg::units`).

## 3. Open questions (for PC 1 unless noted)

1. `msg-ui.md` §1 r5 vs its own Test vectors and `bridge.md` §10: rule 5
   and the table's Part column make only "output" rows a `QuestUi`
   output (implemented); the vector `5d 08 03 00 0000` says "output; UI
   does nothing", and the §10 row says the producer is "0x5D (every case
   except the eclipse)", while `5d 17 01 …` says "no output". Which is
   meant? The difference is only whether an inert output is emitted.
2. `msg-ui.md` §1 r2, f bit 1 c 23: is `[0x007BC9D4]` := 1 on the classic
   branch only (implemented) or after both branches?
3. `msg-ui.md` §1 r6: dump the quest-log table `0x00723F30` (41 × 16
   bytes: slot +2, act +3, chain +8) into a spec TSV, so T and rule 7 can
   run.
4. `msg-ui.md` §2 r2.2 `0x0044DA40` (input reset) and §3 r3 / `ui/panels.md`
   §12 r2 `0x00463DF0`: what they read and write.
5. `msg-skills.md` §2 r4 / `stat-lists.md` §1 r2: the arguments of the
   `0x00643620` list creation (`0x006251F0` flags, owner, and the
   `0x00626E10` reset), and how the client holds state bits before a
   state list exists (the `0x00639DB0` of §2 r1).
6. `msg-skills.md` §2 r2.2: a remove whose entry is the left or right
   skill when there is no native skill-0 entry (select finds nothing, the
   hand keeps the freed entry) or when the removed entry is skill 0
   itself: a dangling pointer in 1.14d? d2rs rejects the message
   (`SkillError::Dangling`).
7. `render/lighting.md` §9.2 r4.4: `0x0061C100`'s period value (0–3) as a
   function of the record (index, type, ticks), and the object refresh
   `0x004BC5E0` (lighting OQ 11).
8. `render/lighting.md` §9.2 r4.2: with no client act and an unplaced P
   both pointers are null, so the check passes and the setter gets a null
   act: crash, or a guard? d2rs rejects (`Unspecified`).
9. `audio/triggers.md` §2 r4 / §3 r2: the `ServerSound` payload has no
   mode, but §3 r2's dead check reads U's mode when U is P. d2rs reads
   P's mode at delivery (P is read at delivery per §2 r4). Confirm, or add
   the mode to the payload.
10. `client/stat-lists.md` §1 r2 with OQ 1: should d2rs build the client
    `d2-sim` list now with no value callback (and add `ValueCallback::
    Client` once `0x004609F0` is written), or wait for OQ 1?

## 4. Local run queue (proposed entries; the fold numbers them)

- **Skills rows on the install** (game files): `D2_GAME_DIR=<install>
  cargo test -p d2-client --test app_single_player -- --ignored
  client_skill_rows`. Expect pass; the printed row count is the `skills`
  record count (`[0x00744304]` +0xBA0) and the passive count is > 0.
- **B seq 227 0x94 bytes** (recording, PC 2 or any session with
  `traces/raw`): the 8-entry list of `update_item_skill_quantities` is
  synthetic (only "ends `0300 01`" is in the spec). Replace it with the
  recorded bytes of `20261006-022633` seq 227 and expect the same result.
- **`play` smoke with game files**: `cargo run -p d2-client --release -- play
  --frames 600` with `D2_GAME_DIR` set (as `docs/LOCAL-RUN.md` runs
  `play`); expect no handler rejection for 0x2C / 0x53 / 0x5D / 0x63
  / 0x77 / 0x21–0x23 / 0x94 in the log other than the named pending ones
  (the in-process server does not send the skill messages yet).

## 5. Gate (this branch)

- `CARGO_INCREMENTAL=0 cargo test --workspace`: see §6 (filled in before
  the push).
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
  --all --check`: clean.
- `python3 tools/coverage.py --check`: 8,391 claims, 0 errors.
  `python3 tools/spec_index.py --check`: ok. `python3 tools/methods.py
  check`: 21 methods OK.

Test changes that are not new tests: `bridge::tests::owned_rows_are_
exactly_the_registered_handlers` pins the new exact count (62 owned ids,
per owner 4 / 3 / 1 / 1 for the four new specs) and owner set;
`gaps_numbered_tests::client_world_holds_only_stated_fields` names the
new `ClientWorld` / `ClientUnit` fields with their owner specs;
`bridge_modules_except_mirror_have_no_bevy_type` scans the six new
modules too; `ui::original` / `audio::driver` tests take `SoundRequest`
instead of a bare id. `Environment` (one `f32`, a sine, never NaN) is now
`Eq` so `ClientWorld` stays `Eq`.

## 6. Gate result

(filled in below at push time)
