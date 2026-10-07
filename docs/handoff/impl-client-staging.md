# Handoff: client + formats follow-ups from PC 1's answers — `claude/impl-client-staging`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud implementation session, 2026-10-06, task class: implementation from
specs, medium. Base: `claude/specs-staging` at `5844674`. Repo only: no
`game/`, no `re/`, no recordings (M09). Two parallel agents took
independent files (masks: `d2-proto` / `tools/`; formats: `d2-formats`).

## 1. Client model (`d2-client::bridge`)

**Implemented, unverified** (M02; neither 0x7A nor 0x81 occurs in the two
recordings, `model.md` §14 rule 5).

1. **0x7A PetAction, 0x81 AssignMerc** (`bridge/msg/pets.rs`, `model.md`
   §14 rules 1–3): `ClientWorld::pets` (newest first, `PetRecord` with
   +0x00 class, +0x04 type, +0x08 pet, +0x0C owner, +0x1C = 100, +0x20
   gone, +0x24… the three 0x81 values). Registered in `msg::HANDLERS`
   (owner `model.md`); the dispatch test now counts 53 owned ids.
2. **Hireling GUID** (`ClientWorld::hireling_guid`, §14 rule 4): used by
   0x0A (`msg-units.md` §2 rule 2: the local player's hireling is never
   removed). 0xAC still always creates: the re-initialisation
   `0x0046EC10` and the hireling class test `0x0063EE90` are not stated
   (Q1).
3. **Level from the room of the 0x15 placement** (§11, §12 rule 2 b):
   `ClientWorld::active_rooms` (`Option<Vec<ActiveRoom>>`, list order),
   `room_of_point` (the act lookup `0x00619DA0`), `local_room`,
   `player_level`. 0x15 now: unit not in S → nothing (before any room
   test); with active rooms, a non-zero point in no room → fatal 0x168;
   a local-player room change to a level whose Levels `Act` differs
   switches `palette_act` (first placement does not). 0x03 sets
   `palette_act` and never the level. `ModelFeed::levels` (`Option<Vec<
   LevelRow>>`): with rows, `blank_screen` is BlankScreen of the player's
   level, 0 with no room; without rows it goes to the inner feed (app
   unchanged). `ClientTables::levels` holds `Act` / BlankScreen per id.
   **Pending:** nothing fills `active_rooms` yet: the client DRLG act from
   the 0x03 seed (§12 rule 1) is not wired into the bridge, so in the app
   a placement is still taken as in a room and the level is unknown; the
   cell lookup (§12 rule 2 a, adjacency) and the free-point fallback
   (§12 rule 4) wait on it.

## 2. Formats (`d2-formats`)

1. `tests/game_sweep.rs` `cof_every_live_file_parses`: the `amblxbw.cof`
   assertion is gone (no such file in 1.14d); the test now asserts the
   spec's `ambl*` vector (names exactly `ambl1hs`, `ambl1ht`, `amblhth`,
   `amblxbow`; the first three parse).
2. `string_tables_every_key_resolves` follows `formats/tbl.md`: 29
   copies of 20 paths, 10 language folders (case-insensitive, ENG\BETA
   present), all under `data\local\lng\`, header checks, 63,167 used
   entries, 16,786 non-ASCII values all strict UTF-8, 0 raw `FF`, 130
   `C3 BF`. `tbl.rs` stays byte-based (the spec puts decoding at load,
   `ui/text.md` §2); its `TblEntry::value` doc no longer claims
   Windows-1252; new unit test `values_are_kept_as_bytes`.
   `docs/handoff/game-tests-client-assets.md` expected rows updated.
   **Local run queue** (game files): `D2_GAME_DIR=… cargo test -p
   d2-formats --test game_sweep -- --ignored cof_every_live_file_parses
   string_tables_every_key_resolves`; expect both to pass with the counts
   above.

## 3. S→C unwritten-byte masks

`specs/tools/scenario-masks.tsv` (read by `conformance::scenario::compare`)
and the §6 rows table of `specs/tools/scenario.md` (edited so the table and
the TSV agree, §6 rule 2; index regenerated) gain 0x21 byte 11, 0x22 bytes
2 and 10, 0x62 byte 6, 0x7E bytes 1–4 (`original-hooks.md` §6.2). Tests:
`mask_table_parses_and_is_strict` checks all 9 rows;
`unwritten_builder_bytes_are_masked_and_only_those` flips each byte of
0x21 / 0x22 / 0x62 / 0x7E (M08). `d2-proto` unchanged: none of these ids is
a typed message with `UNWRITTEN`. **Not added** (Q4–Q7).

## 4. Open questions

- Q1 `msg-units.md` §1.2 rules 2–3: what the hireling re-init
  `0x0046EC10` resets, whether rules 3–4 run after it, and the hireling
  class set of `0x0063EE90`.
- Q2 `specs/proto` 0x7A field names: the TSV names u32@5 `pet` and u32@9
  `owner`; `model.md` §14 rule 2 (and its vector) reads pet GUID u32@9,
  owner u32@5. The handler follows `model.md`; the TSV names need a fix in
  a spec session.
  **Answered** (2026-10-07, spec-senders-area): `sim/server-messages.tsv`
  already reads `owner:u32@5 pet:u32@9` (merge `2161e5b`); no change.
- Q3 Act palette switch test: `model.md` §11 rule 4 compares the Levels
  `Act` byte, `msg-units.md` §3 rule 4.4 the area byte (+2 of
  `0x0061DB70`) of the two levels. The code follows `model.md` (owner of
  the act); one of the two texts needs aligning.
  **Answered**: neither. `0x004654C0` compares byte +2 of the two
  544-byte `levels` records, which is `Pal` (`data/fields.tsv`: `Pal`
  offset 2, `Act` offset 3), and passes the new `Pal` to `0x004FB480`.
  `Pal` ≠ `Act` in 7 rows (125–127, 133–136, all Act 4). Fixed in
  `client/msg-units.md` §3 rule 4.4 (with vectors). Still to align (not
  this area): `client/model.md` §11 rules 4–5 and
  `render/composition.md` §4 say "Levels `Act` byte (`+0x02`)"; the code
  must switch on `Pal` (the vector: 109 → 133 loads act 1's palette).

- Q4 0x27 masks (7, 9, 12–39) are keyed by sender; `scenario.md` §6
  rules 1, 3 have id / offset / length only and an `s2c` record carries
  no sender; an id-only mask would hide written bytes of the list forms.
  **Answered** (`sim/intents-events.md` §6 rule 6): key on the entry
  count u8@6 = 1 (the one-entry forms always write 1; a list form with
  count 1 has 0 in every masked byte), mask 7, 9, 12–39.
- Q5 0x50 masks are keyed by the u16 at bytes 1–2; no key column in §6
  rule 3. The existing unkeyed `0x50 13 2` row already over-masks the
  fully written u16 1 form.
  **Answered** (§6 rule 6): the six call sites of `0x0053D7E0` each write
  a constant u16@1: 1 → none, 4 → 13–14, 2 / 0x24 / 13 → 5–14, 0x17 →
  3–14. The unkeyed row becomes `u16@1=0x0004`.
- Q6 0x82 (name bytes after the NUL through 20) is content-dependent; the
  format has no way to say it.
  **Answered** (§6 rule 6): from the byte after the first 0 in bytes 5–20
  through byte 20 (`0x004135D0` writes exactly through the NUL). Proposed
  columns for the cloud's format: `id key offset length source`, key `-`
  / `u8@o=v` / `u16@o=v`, offset `nul@n`, length `..n`; rows listed there.
- Q7 `scenario.md` names `tools/scenario-masks.tsv`; the file and reader
  use `specs/tools/scenario-masks.tsv`.
  **Answered** (rule): the table lives in `specs/tools/` beside the spec
  that documents it (`specs/README.md` bar 5: machine tables are sibling
  TSVs of their `.md`, embedded with `include_str!`), so
  `specs/tools/scenario-masks.tsv` is right. Spec text writes paths
  relative to `specs/` (`sim/tick.md`), so `tools/scenario-masks.tsv` in
  `scenario.md` means the same file; because the repo also has a
  top-level `tools/`, a spec naming a file under `specs/tools/` that
  code reads should write the full `specs/tools/…` path. `scenario.md`
  (not this area) should change its two mentions.
- Q8 `tbl.md` open question 3 still says "all 33 1.14d tables"; stale
  against 29. **Answered**: fixed to 29 (that line only). The key-resolves vector is scoped to eng; the sweep checks
  all 29 tables. Invalid UTF-8 handling belongs to `ui/text.md` §2 (not
  `tbl.rs`).
- d2-sim note: the 0x50 u16 4 (stone order) and u16 13 (tomb) builders
  write 0 in their unwritten bytes but no test asserts those bytes.

## 5. Checks (this branch)

`cargo test -p d2-client` (463 lib + all other targets pass), `-p
d2-proto`, `-p d2-formats`, `-p conformance --lib scenario`, `-p
scenario-run`: all pass. `cargo clippy -p d2-client -p d2-formats -p
d2-proto -p conformance -p scenario-run --all-targets -- -D warnings`
clean; `cargo fmt --check` clean; `tools/coverage.py --check` 5,263 claims,
0 errors; `tools/spec_index.py --check` ok. The client build needs the
CI system packages (`libwayland-dev libasound2-dev libudev-dev
libxkbcommon-dev pkg-config`).
