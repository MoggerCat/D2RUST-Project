# q-play-smoke: end-to-end play smoke test

Stitching session `q-play-smoke`, branch `claude/q-play-smoke`. Read only
`specs/`, `docs/`, `crates/`, `tools/`. Synthetic fixtures only; nothing
here is verified against 1.14d (rule 10). No code outside the new test
file changed, so no PROVISIONAL rule was added; REC-277 is unused.

## What runs

`crates/d2-client/tests/play_smoke.rs`, the play app headless (as
`d2-client play --new` wires it: bridge, in-process server on its thread,
walk prediction, original UI with memory fixtures; no window) over the
synthetic single-player game. After every step `Run::check` asserts:

- the server refused nothing: every drained C→S message is
  `Handled::System` or `Dispatched(ResultCode::Done)` (`Probe` reads
  `LocalLink::last_frame` after each pump);
- the client model has no unhandled (unowned), dropped, rejected or
  discarded S→C message (`ReceiveLog`, `bridge.md` §6);
- no system panicked (a bridge error panics through Bevy's handler).

| Step | Path | Result |
|---|---|---|
| new character (sorceress "Smoke") | 0x67 → 0x01/0x00/0x02 → 0x6B → join | clean |
| talk to Akara | `Bridge::interact` → 0x13 → S→C 0x27/0x28 → NPC menu (Talk, Trade, Cancel) | clean |
| Trade | menu row click → 0x38 | clean, but see F1 |
| waypoint | interact → S→C 0x63 → menu (UI 0x14) → 0x49 → Cold Plains | clean |
| field kill → level up → 0x3A | `the_field_leg_kills_levels_up_and_spends_a_point` | **`#[ignore]`, F3** |
| drops, equip, Town Portal, save → load | not reached | F3 blocks; F4 |

`the_scripted_play_run` passes; the field leg is in the file, ignored with
the reason naming F3, so the next session un-ignores it when F3 is fixed.

## Findings (what broke, and why it is not fixed here)

- **F1. Trade with an empty store locks the player in the interaction.**
  The shop panel opens only when store items (S→C 0x9C action 11)
  arrive (`ui/shop_ui.rs`, REC-110); with none, nothing ever sends
  C→S 0x30, the server keeps the NPC interaction, and the next
  interaction (the waypoint's 0x13) does nothing. The synthetic game's
  stores are always empty (no store item tables), so the run sends the
  chat end (`msg_chat_end`) itself after an unopened shop. Fix (UI
  owner): open the shop on the Trade / Gamble choice (`OriginalUi::open_shop`
  from `npc_menu_ui.rs`), or end the chat when the store stays empty.
  With real data a store is never empty, so the user should not hit it.
- **F2. The live-data path cannot be smoke-tested in the cloud yet.**
  `single_player::build` on the `test_fixtures::act1` install (patched as
  `play_native.rs` patches it) stops at `BuildError::Drlg(UnknownLevel(40))`:
  the play game creates every act, the fixture install has Act I only.
  A five-act fixture install (acts 2–5 exist as `test_fixtures::act2..5`
  variants, not merged into one set) would let this whole run go through
  `GameData::Live` exactly as the user's `play` does. Follow-up.
- **F3. The player's attack never starts on the play host (blocks kill,
  drops, XP, level up).** With the combat fixtures installed before the
  join (`install_combat`: bench Attack row with do function 1, monster
  class 0, vitals tables, stat table, AnimData `SOA1HTH` / `M0DTHTH`,
  server `UnitLooks`), the server list holds Attack (left and right),
  the monster reaches the client (0xAC, then 0x6D), and C→S 0x06 on it
  is `Dispatched(Done)`; but the player stays in mode 1, the skill log
  is empty and no hook error is recorded. `use.md` §1's handler returns
  0 whatever `use_on_unit` (`d2-sim` `skills/use_/mod.rs:615`) did, and
  that function has five silent exits (entry state not usable, entry
  mode 0, no row, bad type, run-to). Next step: find which one with the
  ignored test (it prints the list, mode and log), fix it in its owner.
  The 0x3A step after it is the `app_levelup.rs` path and is expected to
  pass.
- **F4. Not reachable on synthetic data without more fixtures:** a new
  character's start items (`ActionCharacter::start_items` is Unapplied),
  store contents and a Town Portal scroll (`tsc`) need item tables;
  `play` on synthetic data saves nothing (no `SaveTables`), so the save →
  load leg needs `app_save.rs`'s 32-bit table and the item tables of
  `save_full`. The client skill list of a new character stays empty on
  synthetic data (no `charstats`): the server list is made (F3 run shows
  Attack in it) but no S→C 0x94 is sent (`PLAYABLE.md`, "still missing").

Fixture gap fixed in the test itself, not a code break: the client's
synthetic monster rows have no class 0, so its 0xAC was ignored (class
without a row, `client/msg-units.md` §1.2 r2) and the following 0x6D
counted as dropped; the field leg sets the row.

## The user's local check

The test needs no game files:

```powershell
git fetch origin claude/q-play-smoke
git checkout claude/q-play-smoke
cargo test -p d2-client --test play_smoke
cargo test -p d2-client --test play_smoke -- --ignored --nocapture
```

The first command passes (1 passed, 1 ignored). The second shows F3's
failure message (the server skill list, the player's mode 1, the log).
In the window (`cargo run -p d2-client --release -- play --new sorceress
Test`): talk to Akara, Trade, close the shop, then use the waypoint to
Cold Plains; it should work with real stores (F1 only bites an empty
store). Then attack a monster in the Blood Moor: if she never swings,
that is F3 on real data too; report it.
