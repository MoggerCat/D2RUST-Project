# Handoff: impl-session-flow (2026-10-07)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of the tenth fold (`claude/fold-handoff-10`); this file stays as the detailed record. Its open questions are in HANDOFF §7 "Tenth set" (PC 1 / PC 2).

Cloud implementation session. Base `claude/specs-staging-5` @ `01229ee`;
branch `claude/impl-session-flow`. Task: the server session-flow items of
`docs/HANDOFF.md` §1 row "3 not implemented" (ninth fold list): the §7.3
client pass (S→C 0x67–0x6D), the single-player session sequence C→S 0x67
→ 0x6B → first tick (`intents-events.md` §8), the join messages of row
3ao the in-process server does not send yet, and S→C 0x73. Server side
only (`d2-server`, `d2-sim`); `d2-client` untouched (it does not compile
on this base).

## Stale list items (already done on this base)

- **0x69 at death** ("the §7.3 client pass 0x67–0x6D (so no 0x69 at
  death)", MB-8): already implemented and tested before this session —
  `View::monster_update` (`d2-sim/src/wiring/action/unit_update.rs`) from
  `ActionSim::send_unit_update`, with the path provider on: 0x69 code 8 in
  the kill's tick, code 9 when mode 12 is set
  (`unit_update::tests::kill_sends_code_8_then_the_death_end_code_9`,
  `test-fixtures --test monster_death`). Drop it from the list and MB-8.
- **0x01 / 0x00 / 0x02, 0x0B … 0x7E, 0x04** of row 3ao (SJ-2, SJ-3,
  SJ-4, SJ-5): already sent by `adapters::session::{create_game,
  enter_game}` and the tick's `send_load_complete` on this base; row 3ao's
  "Not sent" list is stale for them.

## What landed

1. **Session dispatch on the host drain** (`intents-events.md` §2.5).
   New defaulted `Intents::session_message(client, msg, size, out) ->
   bool` (`d2-server/src/seams.rs`); `Host::drain` offers each queue-0
   message to the game first and hands it to the `SessionHandler` only
   when the game returns false. The default is false, so every existing
   `Intents` (fakes, `d2-client`'s) behaves as before.
2. **The session sequence C→S 0x67 → 0x6B** (`intents-events.md` §8.1,
   §8.2; new `d2-server/src/adapters/session_flow.rs`). `SimGame::
   set_session(SessionFlow::new(arena_flags, loader))` turns it on
   (`SimGame::session()` reads its requests and faults).
   - 0x67: the stated checks of `0x0052C330` (locale u8@0x2D > 14, flags
     u32@0x27 without bit 1 / bit 2, class ≥ 7 → refused, nothing
     happens); then the client record (prepended, state 0), S→C 0x01
     (difficulty, arena flags, expansion = bit 20, ladder = bit 21), 0x00,
     client state 1, 0x02, queued straight to the client's buffers (no
     player yet): they leave with tick 1's flush.
   - 0x6B: no record → stop (§8.2 r1); a classic game with class ≥ 5 →
     load result 0x18; else the caller's `CharacterLoader` (`Box<dyn
     FnMut(&mut SimGame, ClientId, &CreateGame) -> Result<Loaded, u32>>`)
     creates the unplaced player and returns its `Entry`; a non-zero
     result removes the client; else the client's player is set and
     `enter_game` runs (rules 3–6), its messages queued with the next tick.
   - `SessionFault` records every refusal / failure in drain order.
3. **S→C 0x53 after 0x03** (§8.2 rule 4; `render/lighting.md` §9.1,
   §9.2 r2). New `d2_sim::world::environment::Environment` (period index,
   ticks, eclipse; created as index 2, ticks 0, eclipse 0), held per act
   in `ActEntry::environment`; `enter_game` sends its 0x53 right after
   0x03 (recorded `53 02000000 00000000 00`).
4. **Monster update §7.3 rule 2 steps 1 and 4** (`unit_update.rs`):
   flag-ex 0x10000 → S→C 0x15 (type 1, GUID, path cell, flag 1) then the
   room-change messages `0x00554670` (`pathing.md` §9.8, through
   `PathCtx::room_change_messages`); unit flag 0x100 → the overhead
   message (0x76 when no hover text, §7.9 r3). Order: step 1, step 2 (the
   mode message), step 4.

## Tests

- `test-fixtures --test synthetic_game`:
  `session_messages_run_creation_then_the_join` (0x67 in the first drain,
  0x01/0x00/0x02 with tick 1's flush, state 1, loader not yet called; 0x6B
  after tick 1 → the full join prefix 0x59 … 0x7E incl. 0x53, then one
  0x04, state 4); `session_refusals_stop_the_sequence` (0x6B without a
  record, the three 0x67 refusals, the classic/expansion-class 0x18 with
  the client removed and the loader never called; nothing reaches the
  client). `town_entry_sends_the_join_sequence` now expects 0x53 after
  0x03 (expected list moved to `join_messages`; setup split into `town`,
  `alloc_player`, `run_frames`).
- `d2-server tests::host::system_messages_reach_the_game_first` (routing:
  game first, the rest to the `SessionHandler`, drain order, flush).
- `d2-sim world::environment::tests::created_record_sends_the_recorded_join_message`.
- `d2-sim wiring::action::unit_update::tests::reassign_and_overhead_steps_of_the_monster_update`
  and its perturbation `unqueued_monster_bits_send_nothing`.

Coverage claims on each (`Covers:` lines); `py tools/coverage.py --check`
clean.

## What the app (`d2-client`) will need

- Build its `SimGame` with `set_session(SessionFlow::new(0x0010_0004,
  loader))`, the loader allocating the player (no room, (0, 0), mode 1)
  from the 0x67 class / name, or loading the `.d2s` (as
  `enter_game_from_save` does: `character::load` onto the new player, then
  return `Entry::new(report.act, save.header.name)`).
- Send C→S 0x67 (`d2_proto::client::CreateGame`: flags with bit 20 for
  expansion and bit 2 set, locale ≤ 14) through `send_system` in its
  first frame, then C→S 0x6B after the first flush that carried 0x02
  (`client/model.md` §7 rule 3), instead of calling `create_game` /
  `enter_game` directly; then 0x03 carries the real game +0x80 when the
  game was built with its object control.
- `LocalLink` / `PendingSession` keep working: the flow only takes 0x67
  and 0x6B; the other system ids still reach the `SessionHandler`.

## Spec gaps (named in code, not guessed)

| File | Section | Missing |
|---|---|---|
| `sim/intents-events.md` | §2.5 (0x67) | the name checks `0x0053EFC0(name, 16)`, `0x00538B70`, `0x00538C60` |
| `sim/intents-events.md` | §8.1 r1–r2 | the arena record (`0x0053F4B0` from u16@0x25, flags `0x0053FD40`): 0x01's u32@2 comes from `SessionFlow::arena_flags` |
| `sim/intents-events.md` / `server-messages.tsv` | §8.2 r2, row 0xB4 | the 5-byte layout of S→C 0xB4 (`0x0053B260`): the refusal is a `SessionFault`, nothing is sent |
| `sim/intents-events.md` | §8.2 r3.4, r3.8 | the stat-message builder `0x0053BE40` (0x1D / 0x1E / 0x1F choice) of `0x006258D0` |
| `sim/intents-events.md` | §8.3 | layouts / fields of 0x5B (`0x0053C940`), the join's 0x65 (`0x0053FC70`), 0x8D (`0x0055B620` → `0x0053DF00`), the join 0x5A: the join sequence hook stays empty |
| `sim/intents-events.md` | §2.5 | 0x69 leave (`0x005303D0`), 0x6A, 0x6C, 0x6E, 0x70 bodies |
| `sim/tick.md` / `render/lighting.md` | §3 step 1 / §9.3 | the server advance `0x0061C040`'s `A` / `L` arguments: the environment record never advances, so 0x53 at a join reports the creation values |
| `missiles/missiles.md` | R2.4 | 0x73's server layout: bytes 1–4 (the client skips them), which field goes to which offset (the client reader `client/msg-units.md` §7 r6 lists offsets, the server spec lists fields without offsets), and the `0x006486C0(path)` argument; 0x73 is not sent |
| `sim/intents-events.md` | §7.3 r2 steps 5–10 | 0x0C's fields (`0x00597CF0`), `0x00571740` (unit +0x6E), `0x00639F20`, `0x005711D0`, `0x00625A20` / `0x005715A0`, 0x57 (`0x00597C70`) |
| `sim/intents-events.md` / `monsters/init.md` | §7.1 r2.1, §7.2, §24 | a monster's add messages (0xAC server fields past §24): an unannounced monster and the room switch's monsters still get none (SJ-8) |

| `render/lighting.md` / `sim/intents-events.md` | §9.2 r4.2 / §8.3 | `-022633` seq 228 is a second 0x53 in frame 2 (after §8.3's seq 157–224); §8.3 names no sender for it (tick step 1's `environment_changed` sends it only on a period change). Not sent here |

## Results

At `e2ff3be` (`CARGO_INCREMENTAL=0`, `--workspace --exclude d2-client`):
`cargo fmt --all --check` clean; `cargo clippy … --all-targets -- -D
warnings` clean; `cargo nextest run --no-fail-fast`: 4,607 run, 4,602
passed, 5 failed (the 5 known on the base: `monsters::ai::tests::
specd_here_*` × 2, `scenario-run::scenarios` × 3), 198 skipped;
`py tools/coverage.py --check` 8,908 claims, 0 errors;
`py tools/spec_index.py --check` clean. `d2-client` not built (broken on
the base); its 0x53 handler (`bridge/msg/lighting.rs`) ignores a 0x53
that arrives before the player is placed, so the join's new 0x53 needs
no client change.

## Left (not done here)

- HANDOFF §1 rows 3ao and "3 not implemented", §7 MB-8 and SJ-2–SJ-5:
  fold this note (not edited here, to avoid conflicts with the parallel
  sessions).

- The §8.3 join sequence messages and the stat messages (gaps above).
- 0x73 (gap above); item 0x9C add messages (item world: impl-items-rest).
- The app-side game creation (`d2-client`, out of scope).
- Folding the staged e2e games onto the session flow (HANDOFF 7v (f)).
