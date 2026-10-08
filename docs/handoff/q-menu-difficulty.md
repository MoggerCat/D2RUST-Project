# q-menu-difficulty: difficulty box and game-start rules

Branch `claude/q-menu-difficulty`. File `crates/d2-client/src/ui/front_end/screens/difficulty.rs`; test `crates/d2-client/tests/front_end_difficulty.rs`.

## Connected

- `DifficultyScreen` (§F2.8): backdrop, title 10019, NORMAL/NIGHTMARE/HELL buttons (keys R/N/H, triggers `Difficulty(0..2)`), off-screen Esc control (→ char select via the existing flow row). Hell is created disabled unless `FlowCtx::difficulties_open >= 3`; a disabled button ignores its key.
- Pure rules (L1/L2, §F2.6): `ok_decision(status, expansion_install)` → `Ignored | DeadHardcore | Start | Box`; `hell_enabled`; `difficulties_open` (1 start at once, 2 box/Hell off, 3 box/Hell on — what character select stores before firing `Ok`; the flow table already opens the box above 1); `start_flags` (0x67 u32@0x27: 4 | 0x800 hardcore | 0x100000 expansion); `start_act(+0xA8..+0xAA, difficulty)`.
- Test vectors 0x0420, 0x0520, 0x0920, 0x0A20, 0x0400, 0x0804, 0x0020 classic, dead hardcore, `00 83 00` start act: all in the test file.

## PROVISIONAL (REC-182, d2rs-own, unverified)

The `difficulties_open` encoding (1/2/3) is ours; the title text colour 7 is not modelled (`Control` has no colour field).

## Left

- Character select must call `ok_decision`/`difficulties_open`/`start_flags` with the slot's status (its own session). `GameLoad` carries only the difficulty; the host computes the flags with `start_flags` and the act with `start_act`, then builds the 0x67 via `single_player::create_request_for` (not edited here: shared file).
- Message box 5304 for `DeadHardcore`.

## Local check

```
cargo nextest run -p d2-client --test front_end_difficulty
```
All tests pass; no visible change in `play` until the host runs the front end.
