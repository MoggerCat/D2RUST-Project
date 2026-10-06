# Handoff: character save (`.d2s`) reader, writer and dev tool — `claude/impl-d2s`

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Base: `claude/specs-staging` at
`5844674`. Repo only, no game files, no saves (M09): every claim below
holds on this branch, on synthetic data.

## 1. State

**Implemented, unverified** (M02): no 1.14d save has been compared
(`specs/formats/d2s.md` Open question 3); the checks are queued (§4).

- **`d2_formats::d2s`** (`crates/d2-formats/src/d2s.rs`, spec
  `formats/d2s.md` §1–§8, §10): `read(bytes, &ReadOptions, &dyn
  SaveTables) -> Result<D2s, D2sError>` and `write(&D2s, &dyn
  SaveTables) -> Result<Vec<u8>, WriteError>`.
  - Model: `Header` keeps every one of the 335 bytes (unread fields
    too: +0x26, +0x29, +0x30, +0x34, +0xBF.., +0xCF, +0xD0..); `Body`
    holds quests (3 × 96), waypoints (records + the 8 pad bytes), NPC
    fields A/B, `Stats::Bits` (version > 0x5E) or `Stats::Mask`
    (0x5C–0x5E), raw skill bytes, the player list, 0/1 corpse (with the
    unassigned first u32), `jf` (`Option<Option<list>>`), `kf`
    (`Golem { flag, item }`) and any trailing bytes. So parse → write is
    byte-identical for every file the reader accepts.
  - Items stay opaque `ItemEntry { bytes }` (item + its socketed
    children, §8.1 rule 2). Their length comes from
    `SaveTables::item_entry_len`; the tool and the server plug in
    `d2_proto::item_bits::save_entry_len` (below).
  - Checks in the loader's order with the internal codes of §10:
    dispatch (< 8 bytes / magic → `NotASave`, result 9; version < 0x5C →
    `Legacy`, not specified, OQ1), 0x2000 cut, header size (4),
    checksum (6), size (5), version (7), context checks (§2.2 rules 4–5,
    only when `ReadOptions::game` is set; also `check_header`), class
    > 7 (4), new-character stub / flag with more data (2), then the
    sections 15–23. `D2sError::result()` maps through the 27-entry table.
  - Helpers: `checksum`, `finish` (size + checksum), `D2s::new_stub`
    (§2.6), `Slot::encode/decode` (§2.4 rules 1, 4), `npc_bit` (§6 rule
    2), `Waypoints::bit`, `clamp_stat`, `write_stats`, `load_result`.
  - Not here (load effects, §9, `d2-server` character storage): quest
    and waypoint normalisation, gold limits, stamina/hp/mana, item index
    resolution, placement, hireling restore, the golem's skill-90 check
    (§8.5 rule 2 needs the unit's skills; the format reader reads the
    golem item whenever g ≠ 0).
- **Item bit stream save format** (`items/bitstream.md` §2 rules 2/5,
  §3 rule 6, §4.1 rule 7, §4.4 rule 3, §5):
  - writer `d2_sim::items::bitstream::write_save` / `write_save_into`
    (`SAVE_BUFFER` 0x2000, `SAVE_MARKER`); new `StreamItem` fields
    `unit28`, `save_trailer`, `children` (all defaulted);
    `BitWriter::pad_to_byte`.
  - reader `d2_proto::item_bits::decode_save_record`, `save_entry_len ->
    SaveEntry { len, item, children: Vec<SaveEntry> }`; `ItemBits`
    gains `save`, `save_unit28`, `save_trailer`; errors `BadMarker`,
    `TrailerTail`.
- **`d2s-tool`** (`tools/d2s-tool`): generates and inspects saves for local game
  testing (args hand-parsed; tables from `--game-dir` or `D2_GAME_DIR`).
  `SaveTables` from the loaded itemstatcost (`csvbits`, `csvparam`,
  `csvsigned`) and `save_entry_len` with `d2_server::adapters::item_bits::TablesLookup`.
  - `new --name N --class C -o OUT` with `--level`, `--expansion`,
    `--hardcore`/`--softcore`, `--stat ID=V` (stored units: life/mana/
    stamina in 1/256), `--gold`, `--skill I=L`, `--all-skills L`,
    `--quests none|acts=N|LIST`, `--waypoints none|all|LIST`,
    `--difficulty-unlocked normal|nightmare|hell`, `--act`,
    `--difficulty`, `--item CODE[@X,Y][:PAGE]` (0 inventory, 3 cube, 4
    stash), `--seed`, `--map-seed`, `--time`. Stats: `vitals.md` §1
    creation values, then the level's experience through §4.3 (d2-sim
    `init_player_stats`, `add_experience`). Items: `generation.md` §10.4
    via d2-sim `create_item` (quality 2, identified, ilvl = level, format
    101 expansion / 2 classic), placed by the free-position search or at
    the given cell, written with `write_save`, each re-read through
    `save_entry_len`. Refuses what would not read back: clamped values,
    `CSvBits` 0 stats, gold over level × 10,000 or stash gold outside
    0..=2,500,000 (the loader zeroes it), a town in a locked difficulty.
  - `new-stub` (§2.6), `dump FILE` (header, quests, waypoints, NPC,
    stats, skills, each item decoded), `check FILE` (read → write → first
    differing offset), `set IN -o OUT [edit flags]` (edit an existing
    save; class changes refused; `--level` sets stats 12/13 only).
  - Every file `new`/`set` writes is read back and must rewrite to
    itself before it is saved.

## 2. Tests

- `d2-formats` `d2s::tests` (27): every Test vector of the spec
  (checksums, the stub 0xC7373BCA, header error codes, stats vectors
  incl. the clamp and the 0x5E mask layout, skills, empty lists,
  `jf`/`kf` bytes, corpse count 2, hotkeys), context checks, every
  marker's code, unchecked fields, `jf`/`kf` absent at the end, classic
  game leaves them as trailing bytes, writer overflow, the result table,
  and a perturbation test (M08): flipping any byte outside +0x0C fails
  the load.
- `d2-sim` `items::bitstream::save_tests` (6) and `d2-proto`
  `item_bits::save_tests` (7): byte-exact synthetic save-format streams
  (expected bytes from an independent bit packer following the spec).
- `d2s-tool` `tests/synthetic.rs` (9, synthetic install from
  `test-fixtures`, stats 0–15 widened to the §7.1 measured widths):
  `new_save_round_trips`, `stats_level_and_gold_in_stats_section`,
  `gold_over_the_carry_limit_is_refused`, `skill_bytes`, `status_bits`,
  `items_parse_back`, `quests_and_waypoints`, `set_edits_a_save`,
  `command_line`; `tests/real_saves.rs::real_saves_round_trip`
  (`#[ignore]`, `D2_SAVE_DIR` + `D2_GAME_DIR`; claim added after the
  first local pass).
- Gate on this branch: `cargo test -p d2-formats -p d2-proto -p d2s-tool`
  and `items::bitstream` in d2-sim green; clippy `-D warnings` on
  d2-formats, d2-proto, d2-sim, d2s-tool; `cargo fmt --all --check`;
  `coverage.py --check` (5,359 claims, 0 errors); `spec_index.py
  --check`; depcheck (tool agent).

## 3. Decisions (also in `docs/PLAN.md`)

1. The format layer owns bytes, checksum and the loader's format checks;
   §9 load effects are `d2-server`'s.
2. Cases where the game reads past the buffer of a hand-made file are
   rejected with the section's code: stats without terminator (edge case
   2, as the spec says), skills section shorter than header +0x2A (edge
   case 4, rule 3), `kf` marker with no g byte. No observable output
   exists to match.
3. The writer writes the model as given (fields the game derives — status
   0x20/0x40, level byte, town byte, skill bytes per class — are the
   caller's); it stores size and checksum itself.

## 4. Local run queue (added to `docs/HANDOFF.md` §5 C and `docs/LOCAL-RUN.md`)

C65 (`docs/HANDOFF.md` §5 C; `docs/LOCAL-RUN.md` 2.18 and 6.7):
(1) `real_saves_round_trip` on the user's 1.14d saves: every file parses
in §1 order and rewrites byte for byte; record header +0x10..+0x37,
+0x88..+0xA7, stats at 0x2FD, `jf`/`kf` (d2s OQ3). (2) The `d2s-tool new`
/ `new-stub` characters load in 1.14d; after the game re-saves them,
`check` passes and `dump` ours vs the game's differs only in the save
time.

## 5. Open questions / Pending

- d2s Open questions 1–16 stand; this session settles none.
- Item save format (from the two item agents; not guessed, flagged for a
  real-save check): (a) an alt-code record ends after its base code, so
  no unit +0x28 and no trailer are written/read; (b) compact and
  alt-code records have no socketed children (count = the full record's
  "filled sockets"); (c) children's write-backs (§4.1 rule 8, §4.3 rule
  7) are not returned by `write_save`; (d) the writer does not check
  `filled == children.len()`.
- `kf` with its marker but no g byte (file ends after `6B 66`): not
  specified; d2rs rejects with 23.
- Tool (not specified, so refused or written as noted): `--quests all`
  is refused (`world/quests.md` does not say which bits a completed
  quest leaves); `acts=N` sets only bit 0 of the slots the §8.1
  transitions set (7; 10 and 15; 18 and 23; 28; bit 13 left out because
  loading clears it). Every item gets the 1-bit 0 trailer
  (`bitstream.md` OQ3). +0x88..+0xA7 of a full save are the stub's
  bytes. Item flag 0x2000 (instore, set by the creation pipeline) is
  kept; `inventory.md` §5.7 flag changes are not applied. All four are
  what C65 (2) checks.
