# q-save-audit: rule-by-rule audit of the `.d2s` save code

Branch `claude/q-save-audit` (from staging). No game files were used (M23 does not
apply to this audit: every check below is against `specs/formats/d2s.md` and the
specs it names; the real-save checks stay with `real_saves_round_trip` and
`tools/d2s_check.py`, which this session could not run). Goal: a save d2rs writes
must load in 1.14d.

Read against the spec, line by line: `d2-formats` `d2s.rs` (header, checksum, every
section reader and writer, stats bit packing, item framing), `d2s/appearance.rs`
(only its call sites), `d2-sim` `items/bitstream.rs` (the item record writer,
`items/bitstream.md` §1–§5), `d2-server` `adapters/character.rs` and
`character/save.rs` (load effects, item list order, jf), `d2-client`
`app/save.rs`, `save_full.rs`, `save_gaps.rs`, `hardcore.rs`, `play.rs` (Save and
Exit), and `tools/d2s-tool` (`save.rs`, `cli.rs`).

## Verdict by spec section

| Spec | Result |
|---|---|
| §1 layout, offsets, markers, sizes | matches (`QUEST_OFFSET` .. `STATS_OFFSET`, markers `gf if JM jf kf`, 0x2000 limit) |
| §2.1 header offsets and widths | matches (`Header::parse` / `to_bytes`, every field) |
| §2.2 header checks and order | matches; ladder rule 5.2 not implemented (single player has no service object, Open question 8) |
| §2.3 status word | writer ORs 0x20/0x40 only where the model says; **F7** (dead bit on softcore death) |
| §2.4 hotkeys, mouse skills | encode/decode match; **F1** (fixed), **F4** (hotkeys not live) |
| §2.5 hireling block | block layout matches; consistency with `jf`: **F2/F3** (fixed) |
| §2.6 stub | matches (`D2s::new_stub`, vector 0xC7373BCA) |
| §2.8 appearance | rebuilt on every save by `rebuild_appearance`; `d2s-tool resave` keeps loaded bytes: **F10** |
| §3 checksum | matches (rotate-left-1 plus byte, zeroed +0x0C, size first) |
| §4 quests | matches (298 bytes, `Woo!`, 6, 0x12A, 3 × 96) |
| §5 waypoints | matches (`WS`, 1, 0x50, 3 × {16 + 8}); record magic check matches `waypoints.md` §3 r2 |
| §6 NPC | layout and `npc_bit` table match; **F5** (A/B never live) |
| §7.1 stats | bit packing matches (9-bit id, `CSvParam` layer, `CSvBits` value, clamp rules, 0x1FF end, byte padding); mask layout for 0x5C–0x5E matches; **F8** (old versions) |
| §7.2 skills | matches; writer now refuses a length that differs from +0x2A (**F3**) |
| §8.1 item list | order, hands rule, count, children matches (`character/save.rs` `list_order`, `item_list`) |
| §8.2 load | flags rule matches (`item_flags_on_load`) |
| §8.3 corpse | layout matches; selection and x/y are provisional (**F9**) |
| §8.4 jf / §8.5 kf | layout matches; **F2/F3** (fixed) |
| `items/bitstream.md` §1–§5 (writer) | reviewed rule by rule: header flags, compact and full head, quality arms, runeword and names, trailer, type-specific values, property lists, terminators, children padding: no disagreement found |
| §10 errors | internal-to-result table matches (27 entries) |

## Findings

"Breaks" says what 1.14d does with the file. Fixed = fixed on this branch with a test.

| # | Spec rule | file:line | What breaks | Status |
|---|---|---|---|---|
| F1 | §2.4 r3 + test vector "left mouse skill 36 → `24 00 00 00` (no 0x8000)" | `d2-client/src/app/save_gaps.rs` `mouse_slots` (passed `left = true` into `Slot::encode`) | +0x78 and +0x80 hold `0x8024`. 1.14d decodes `code & 0x0FFF`, so it loads, but the bytes differ from the game's. Two tests asserted the wrong bytes. | **fixed**; tests changed to the spec vector, new byte-level test `mouse_words_have_no_left_flag_in_the_file` |
| F2 | §1 r2, §8.4 r4, §8.5 r3 (expansion file ends `6A 66 6B 66 00`) | `d2-client/src/app/save.rs` `fresh` (`Body::default()`: no `jf`, no `kf`) | A new expansion character saved without `jf`/`kf`. The loader treats it as absent (edge case 10), so it loads; the file differs from the game's. | **fixed** (`fresh` writes both for an expansion game) |
| F3 | §8.4 r2 (the loader reads a `jf` list exactly when it restores the header's hireling); §7.2 r2; §8.3 r4; §8.5 r2 | `d2-formats/src/d2s.rs` `write` accepted any model | Four models the writer wrote and 1.14d could not read: a `jf` marker with no list next to a hireling block (the loader takes `6B 66` for a list: error 22); a list with no block (marker check fails: 23); a golem flag without its item or the reverse (23); two corpses (21); a skills vector whose length differs from +0x2A (19 or every later section shifted). | **fixed**: `check_body` refuses them with `WriteError::Model`; `save_gaps::apply_gaps` reconciles jf with the block (a block whose items could not be read saves the empty list `4A 4D 00 00`, since `jf` cannot be omitted while `kf` follows); tests `writer_refuses_models_the_loader_cannot_frame`, `jf_and_the_hireling_block_agree`; two old tests built such models and now build valid ones (assertions unchanged) |
| F4 | §2.4 r8 (hotkeys saved as C→S 0x51 stored them) | `save_gaps.rs` module doc, `apply_live` has no hotkey source | A hotkey bound in play is lost on reload; the file keeps the loaded hotkeys. Field lost. | row `q-fix-save-hotkeys` |
| F5 | §6 r1 (fields A and B per difficulty) | `d2-server/src/adapters/character.rs` `set_npc_fields` unapplied; `apply_live` has no NPC source | NPC first-talk and intro bits never change in play, so the file keeps the loaded bits (zeros for a new character): every NPC repeats its intro after reload. Field lost. | row `q-fix-save-npc-fields` |
| F6 | §2.1 +0xAB, §2.2 r8, `d2s-load.md` §7 (map seed saved from game +0x7C and restored on every single-player load) | `d2-client/src/app/single_player.rs:1608` (`map_seed_applies: false`); `save.rs` never sets `map_seed` | A new character saves seed 0 with the town byte's 0x80 set. 1.14d loads it and applies seed 0, so every level layout differs from the session the character was played in. A game-written save loaded in d2rs plays on a different map than the game would build. Loads; world changes. | row `q-fix-save-map-seed` |
| F7 | §2.3 (bit 0x08 set at every death start, softcore too: status 0x0028 after a town respawn, measured §8.3 r6) | `d2-client/src/app/hardcore.rs` `mark_dead` (hardcore only) | A softcore death leaves 0x08 clear. No loader rule reads it for softcore; bytes differ from the game's. | row `q-fix-save-status` (with the ladder bit) |
| F8 | §1 r6, §2.1 version (the game always writes 0x60 with the bit-field stats); §7.1 r7 | `d2s.rs` `write` writes `header.version` and `Stats::Mask` as loaded; `save.rs` `apply_live` keeps both | A 0x5C–0x5E file re-saved by d2rs stays 0x5C–0x5E with sim-written (1.14d-format) item records beside the old stats layout. 1.14d accepts the version range, but the game would have upgraded to 0x60; item records written by d2rs under an older version header are unverified. | row `q-fix-save-old-version` |
| F9 | §8.3 r1 (highest repair-cost score), r3 (first u32 is stack data), r6/r7 (x, y) | `d2-client/src/app/save_full.rs` `read_corpses` | Newest corpse instead of the highest score; u32 and x, y written 0. All three are skipped by the loader (§8.3 r4), nothing breaks. Already PROVISIONAL (REC-141). | none (note) |
| F10 | §2.8 r3, `d2s-appearance.md` (mapping answered) | `tools/d2s-tool/src/save.rs` `resave` keeps +0x88..+0xA7 when an item is equipped | `resave` output differs from the game's re-save when an item is equipped; the loader never reads those bytes. | row `q-fix-save-tool-appearance` |
| F11 | §2.2 r10, edge case 11 (+0x2C is 0 in every game-written save; d2rs reproduces the 0) | `d2-client/src/app/save.rs` `apply_live` set `create_time = now` when 0 | +0x2C non-zero; loaded as the client create time. 1.14d accepts it. | **fixed** (the line removed; `live_values_overlay_the_base` expects the base's value) |
| F12 | `tick.md` §6.3 (character save every 8192 frames, `0x0052D440`) | no periodic save in `d2-client` / `d2-server` (`grep 8192`) | Progress since the last window close is lost on a crash, kill or panic. | row `q-fix-save-autosave` |
| F13 | Save and Exit: the save must happen on every way out | `d2-client/src/app/play.rs:~521`: after `app.run()` the automap teardown calls `resource_mut::<WorldViewState>()` before `saver.save()`; the known `q-fix-play-exit-resource` panic (exit 101) therefore skips the save | Whenever that resource is missing the process panics before the save and the session is lost. Not hit with a real install (the resource exists), hit by any run without it. | row `q-fix-save-exit-order` (save first; the resource row stays) |

Not findings, checked: stat clamp for `CSvBits` n < 32 (unsigned and signed), n = 32
as is; layer written only when `CSvParam` ≠ 0; zero-valued and `CSvBits` 0 stats
never written (`apply_live` drops zeros, `write_stats` skips by column); level byte
= stat 12 low byte (`min(99)`); checksum over the file with +0x0C zero; size field
= real length; town byte `act | 0x80` in the game's difficulty only; hireling block
rebuilt from the live node; item list order (rule 3.1 skip of body 4/5, hands order
by weapon in use, cursor last); `jf` list built from the hireling in the same order.

## Tests added

- `d2-formats` `d2s/tests.rs`: `writer_refuses_models_the_loader_cannot_frame`
  (F3, six refusals and the accepted forms), `swap_pairs_and_switch_byte_sit_at_their_offsets`
  (the swap-pair vector at +0x10/+0x78/+0x80/+0x84).
- `d2-client` `tests/app_save_gaps.rs`: `mouse_words_have_no_left_flag_in_the_file` (F1),
  `jf_and_the_hireling_block_agree` (F3).
- `d2-client` `tests/app_save.rs`: `a_new_character_writes_jf_and_kf_in_an_expansion_game` (F2).
- Changed to the spec (not weakened): `mouse_skills_select_with_their_item`
  and `weapon_switch_trades_the_mouse_pairs` expect the plain skill id (F1);
  `live_values_overlay_the_base` expects +0x2C unchanged (F11);
  `item_section_bytes` and `short_skills_section_is_rejected_with_19` build a
  consistent model (the second patches +0x2A in the bytes, the same assertion).

## What is left / the local check

Rows are in `docs/handoff/build-queue.tsv`. Local run (PC 1, real saves): `cargo test -p d2s-tool --test real_saves -- --ignored`
(`real_saves_round_trip`), then `cargo run -p d2s-tool -- check <save.d2s>` on a save
written by `play --new` and load it in the 1.14d game: it should load with no error;
F6 (map seed) is the one that shows (different level layouts after reload).
