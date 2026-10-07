# PC 1 spec batch on specs-staging-5 (2026-10-08)

Pointer: PC 1's "finish the specs so cloud can code everything" batch, branch `claude/pc1-s5`. Read "For the code" first.

Branch `claude/pc1-s5` = `origin/claude/specs-staging-5` (merged in twice, latest 11 commits included) + PC 1's `local-pc1-s3` work + the branches below. Merges only. Checks at the head: `spec_index --check` clean, `coverage --check` 0 errors, `cargo check --workspace --tests` clean, `cargo test -p d2-proto --lib` 59/59. The gap audit is `docs/handoff/pc1-spec-gaps.md`.

## For the code

- **Message tables (CODE-TABLE CHANGE).**
  - `9d063f2`: 46 S→C layouts filled in `server-messages.tsv`, from the 1.14d builders.
  - `abfd624`: 0x2A, 0x50 and 0xAC filled.
  - `d221d77`: C→S 0x32 `mode`/`cost` and 0x33 `tab`/`cost`, PC 2's names.
  - Each change has its `generated.rs` regeneration as a separate CODE-TABLE CHANGE commit: `795d8f8`, `adeed19`, `7b2ff7a`, and the regeneration in the staging-5 re-merge `cd1a436`.
  - Every row decision is recorded in `sim/intents-events.md` §3.5 r4.1.
- **The layout grammar needs a fixed-length byte-array type** (for example `bytesN`) before four rows can be filled: 0x28 @7 (96 bytes), 0x29 @1 (96), 0x52 @1 (41) and 0x5E @1 (37). 0x16 ends in a tail of 9-byte entries (type u8, GUID u32, x u16, y u16); a count of 0 is fatal 0x855.
- **Code fix `b606060`.** C→S 0x3A is `stat:u8@1 repeat:u8@2`. The character panel's `add_stat_point` and two tests are updated. Without this, staging-5's d2-client does not build.
- **Bridge §10 table (CODE-TABLE CHANGE).**
  - `4010567`: new rows `JoinRefused` (0xB4) and `TownExit`. `TownExit` has producer `update`, so `parse_table` must accept it.
  - `bridge-dispatch.tsv` makes 0xB4's owner `client/model.md`; register a system handler for it.
  - `660bbc1`: new row `StateFx`, and `ShrineFx` adds producer 0x51.
  - `eee0539`: the `StateFx` payload gains the hook number (u8) and two hook values (i32).
  - New rule `bridge.md` §10 r10 (msg-ui OQ10, the user's decision): a model write that 1.14d makes inside UI code is returned by the UI layer as a request. The bridge applies these requests before the next message.
- **`umods.tsv` (CODE-TABLE CHANGE `7afacc0`)**: new client columns `cl_phase0`–`cl_phase4`.
- **New or completed specs to code against:**
  - `sim/pathing.md` §12: every path type, including IDA* (§12.7) and wall-follow (§12.8).
  - `missiles.md` §R6.3: the 14 server-damage functions, which `srv_dmg` calls.
  - `render/overlay.md`: unit overlays.
  - `client/stat-lists.md` §3 r6: client state on/off, the setfunc and remfunc bodies (r6.5–r6.6) and the `StateFx` hook values (r6.7).
  - `skills/bodies-2b.md` §8.9: Redemption's per-corpse effect.
  - `drlg/levels.md` §10 r6–r7: positions when a level has no waypoint room.
  - `calc-expressions.md` §3.5, `ui/text.md` §15, `unit-order.md`.
- **Code that now differs from the spec:**
  - `missile.rs` does not set the room-exit flag when a path has no room (`pathing.md` §11.1).
  - `skills/bodies-4.md` Edge case 1: with a zigzag step ≤ 0 the original loops forever, while the code creates nothing.
  - Stale `TODO(spec…)` markers in the code have answers in the specs. See `damage.md` §3.1 and §4.5, `levels.md` §3.1, `use.md` §4 and §5, `vitals.md` §1 and §2, `tick.md` §6.5, `rng.md` §5.2 and `path-placement.md` §5.1.
- **impl-pc2-fixes seams (answered):**
  - `sim/units.md` §3.1 r7.1–r7.5. The unit is linked only at `SUNIT_Add`, after its per-kind init. Room and level come from the allocation's room argument. CountessChest must still miss its own GUID after the reorder.
  - `monsters/ai.md` §7.5. Every mode request except GH sets the path target, then sets the re-path budget to 20 (`0x006490E0`).
  - `drlg/rooms.md` §9.6 C1–C7. The corner hides P = R+0x20, never R's half, so `linked_found` hiding `r.half` is wrong.
- **Settled contradictions:**
  - `vitals.md` §4.4 r2: the gate is the `exp` state flag, which is PC 2's reading.
  - `camera.md` §9: players take 0–1 client path steps per update and monsters 0–2. `[0x007A04C4]` is always 0, so every step uses 0x400.

## Still Pending (binary read not enough)

- `camera.md` roof and wall block counts (OQ1, OQ7): these need game-file counts. The `mpq-tool` steps are written there.
- `drlg/rooms.md` §9.3: whether a load of an empty `LvlTypes` file is fatal.
- `drlg/outdoor-tilesub.md`: v12 Trees group N, which is leftover heap memory.
- `client/stat-lists.md`: `0x004AEDD0`, `0x004AF890`, the reader of +0xC4 bit 0x80000000, and `0x00647640` / `0x00647960` results 1, 4 and 8.
- `client/msg-ui.md` OQ2: the full unit flag word.
- `missiles.md` OQ7: whether the server reads InitSteps, Qty, SpecialSetup and ExplosionMissile.
- `path-placement.md` §10: whether any caller passes a static-path unit.
- The meaning of byte +0x14 in the death start's mode-change record (on the recording list).
