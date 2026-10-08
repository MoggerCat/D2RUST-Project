# q-act-worlds: all acts in the play host

Stitching session `q-act-worlds`, branch `claude/q-act-worlds`.

## Links connected

| Link | Before | Now |
|---|---|---|
| `GameData::drlg_world` / `world_sim` | act 0 only (`Drlg::create(0, ..)`, `acts[0]`) | `drlg_world_act`, `world_sim_act` take the act; the old calls delegate with act 0 |
| `host::Session` | act 0 hard-wired (list act, `with_act(0)`, preset lookups, `UnitFacts.act`, `room_level`, `level_rect`) | `Session::new_in_act(d, setup, act)`, `Session.act`; `Session::new` = act 0, so no `Setup` literal changed |
| Act II town | no waypoint object | `act2::town()` has the town waypoint record (Act I's) |

Test: `crates/test-fixtures/tests/act2_play.rs` (a session in act 1 starts in Lut Gholein, walks to the Rocky Waste and the Dry Hills; the Rocky Waste is populated and the client gets 0xAC for its monsters; nothing rejected).

## PROVISIONAL

REC-134 (`docs/HANDOFF.md` §7). Made-up data; no `Covers:` claim.

## Left

- `single_player.rs` (the real client play world) was not touched; with game files `WorldTypes` already builds every act.
- Acts III–V sessions need their own fixture sets (only the act2 set exists); the code path takes any act 0..=4.
- Waypoint travel to Act II fields is still untested (outdoor waypoint objects).

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Take a save in Lut Gholein, walk out to the Rocky Waste and Dry Hills: expect desert tiles, monsters, no `rejected` lines. Synthetic check: `cargo nextest run -p test-fixtures --test act2_play`.
