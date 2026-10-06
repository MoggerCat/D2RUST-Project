# Handoff: mutation testing of d2-formats and d2-data (branch `claude/mutants-data-formats`, 2026-10-06)

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, base `claude/tender-meitner-mphas3` at `9b49081`; repo
only, no game files (M09). Method: METHODS M08 applied to the unit tests of
two crates. Tool: `cargo-mutants` 27.1.0, run as
`cargo mutants -p <crate> -j 3|4 --timeout 60 --build-timeout 300`
(`mutants.out` not committed). Parallel sessions ran the same on `d2-sim`
modules; this branch touches only the files below.

## Counts

| Run | Mutants | Caught | Missed | Unviable | Timeout |
|---|---|---|---|---|---|
| d2-formats before | 1,265 | 938 | 251 | 61 | 15 |
| d2-formats after (full run on the committed tests) | 1,262 | 1,138 | 48 | 60 | 16 |
| d2-data before | 2,766 | 1,987 | 371 | 373 | 35 |
| d2-data after (partial: stopped at the coordinator's wrap-up; bin, calc, codegen tested) | 381 | 346 | 3 | 18 | 14 |

A timeout is a mutant that hangs (an index that stops advancing); the
tests detect it, so it needs no action. The after-runs' missed mutants are
the equivalent / unobservable ones listed below, except where noted. The
d2-data after-run was stopped at 381 of 2,766 mutants. Its 3 missed are
the group E equivalents (bin.rs:326:64, calc.rs:296:52, calc.rs:616).
The other d2-data groups rest on their agents' restricted re-runs (each
group's "Verification" line). **Left to do:** a full
`cargo mutants -p d2-data` after-run (about 1.5 h at `-j 4`).
`tables/generated.rs` mutants are all caught or unviable.

**Random catches.** Before, three d2-formats mutants were caught only by
random proptest cases (`robust_tests.rs`, `PROPTEST_CASES`), and the
after-run missed them:
- `dcc.rs:300:25` (`||` → `&&` on zero frame width / height). Now killed by
  `frame_of_zero_width_or_height_is_an_error`, from dcc.md §Boxes.
- `mpq/mod.rs:391:75` (block index bound in `Archive::find`). Now killed by `lookup_rejects_block_index_equal_to_count`, from mpq.md §5 r4
  (claimed).
- `mpq/mod.rs:538:56` (sector offsets check). Now killed by `sector_offsets_validated`, from mpq.md §8
  (compressed, step 2). It has no claim because it checks only the
  rejection half of the rule. A restricted re-run of 391:75 (all three
  operators) and 538:56 catches all 4.

A count of caught mutants that depends on proptest seeds is not stable from
run to run. Each kill listed here is a deterministic unit test.

## Code fixes (category c)

1. **`d2-formats::dt1` `decode_rle`** panicked on a pair that writes no
   pixels but lands outside the block. For example, `(5, 0)` after row 32
   sliced `pixels[1029..1029]` of a 1,024-byte buffer, and `(0xFF, 0)` on
   row 31 did the same. `specs/formats/dt1.md` §Block pixels says "Moving
   past the last row is allowed if no pixel is written there". A pair with
   count 0 now writes nothing and continues, and the bounds check applies
   only to pairs that write. Test: `dt1_rle_skip_past_last_row_writes_nothing`.
2. **`d2-data::patch` `Finding::sort_key`** ordered B findings by code,
   then table. `specs/data/patch-layers.md` §8 orders them by table, so the
   `2 =>` arm is removed and B uses the same (table, row, column) key as C.
   `report_orders_b_by_table` failed before the fix.

No public signature changed.

## Spec issues found (not edited: specs are out of scope here)

- `specs/formats/mpq.md` §10 step 2c reads `l == 0x205` "(519)", but 0x205
  is 517. The code uses 0x205, which agrees with the rest of the section
  (n = l + 2 ≤ 518; LenBase[15] 0x106 + 255 = 0x205).
- Some d2-data assertions rest on statements the specs record as
  implementation choices awaiting confirmation, not confirmed 1.14d
  behaviour:
  - `patch-layers.md` Open question 5(g)/(h): the line and column of C
    findings, and the D line = row + 2 (`mutant_tests_patch.rs`).
  - `fixups.md` Open question 6, the d2rs policy for a missing monmode row
    (`speed_missing_monmode_row_is_an_error`).
  If those answers change, the tests change with them.
- `mutant_tests_bin_calc.rs` asserts nothing about PARAM in the
  callback-less calc evaluator, because `calc-expressions.md` does not
  define it (see calc.rs:616 below).

## New files

| File | Tests | Kills mutants in |
|---|---|---|
| `crates/d2-formats/src/mutant_tests_dcc.rs` | 7 | `dcc.rs` |
| `crates/d2-formats/src/mutant_tests_formats.rs` | 27 | `ds1, dt1, dc6, cof, animdata, palette, tbl` |
| `crates/d2-formats/src/mpq/mutant_tests_archive.rs` | 21 | `mpq/{mod,set}.rs` |
| `crates/d2-formats/src/mpq/mutant_tests_codecs.rs` | 8 | `mpq/{adpcm,crypto,explode,huffman}.rs` |
| `crates/d2-data/src/mutant_tests_bin_calc.rs` | 11 | `bin, calc, codegen` |
| `crates/d2-data/src/mutant_tests_compile.rs` | 14 | `compile.rs, compile/callbacks.rs` |
| `crates/d2-data/src/mutant_tests_misc.rs` | 18 | `compile_set, crosscheck, links, schema, strings, txt, tables/mod` |
| `crates/d2-data/src/mutant_tests_fixup.rs` | 27 | `fixup.rs, fixup/*` |
| `crates/d2-data/src/mutant_tests_patch.rs` | 27 | `patch.rs, patch/*` |

Each module is `#[cfg(test)] mod …;` in `lib.rs` or `mpq/mod.rs`, and lives
in-crate so that it can reach private codecs and helpers. All tests are
synthetic: they build their inputs from the spec layouts and assert what
the spec says.

**Coverage claims** (`docs/COVERAGE.md` §2, unit tier) are placed only where
the assertions check the whole rule:
- `mpq.md` §4, §5 r3, §5 r4, §5 r5, §10 r2, §10 r3, §10 r4, §12 r2, §12 r3
- `dcc.md` §stage-1-cell-colors-all-frames-in-order r4
- `callbacks.md` §4 r4, §5 r1
- `fixups.md` §10, §12 text, §13 r2
- `runtime-maps.md` §1

The §4 test was extended to check the trailing bytes before it was claimed.
The other tests carry no claim, because each checks only part of its rule.

## Dispositions per group

Line numbers are those of the before-run. In `mpq/mod.rs` they are shifted
+4 by the new `mod` lines; group C below uses the current numbers.

### A. d2-formats dcc

dcc (65): 62 killed by 6 tests in src/mutant_tests_dcc.rs; 3 equivalent:
- dcc.rs:145:68 < -> <= in Dcc::parse: equal offsets give an empty direction; the original errors on its first 32-bit read; both error.
- dcc.rs:257:18 > -> == and > -> >=: unreachable check (bytes already taken with data.get(start..end)); an optional block ending at end of data fails on the next 20-bit read; error either way, only the message differs.
Tests: frames_per_direction_limit_is_inclusive (136:33), direction_offset_may_skip_bytes_after_the_header (145:23), optional_bytes_start_at_the_next_byte_boundary (83:*, 246:42, 249:31 x2, 250:29, 255:25 x2, 257:18 <), direction_box_offset_from_origin (318:35, 345:25, 354:37, 377:43), cells_masks_raw_codes_fills_and_equal_copies (15; Covers dcc.md §stage-1-cell-colors-all-frames-in-order r4), equal_cells_copy_moved_rectangles_or_clear (27 in the stage-2 equal-cell branch 470-484).
Verification: 62 caught / 3 missed / 0 unviable / 0 timeout.
Added after the full after-run: frame_of_zero_width_or_height_is_an_error kills dcc.rs:300:25 (see Random catches).

### B. d2-formats ds1, dt1, dc6, cof, animdata, palette, tbl

small formats: ds1, dt1, dc6, cof, animdata, palette, tbl (59): 52 killed by tests in d2-formats/src/mutant_tests_formats.rs (dt1.rs:232:18 disappeared with the fix below; its test kills the replacement code); 7 equivalent; 1 code fix; no Covers claims.
Code fix (c): dt1.rs decode_rle panicked on a pair that writes no pixels but lands outside the block (e.g. (5, 0) after row 32 sliced pixels[1029..1029] of 1024; (0xFF, 0) on row 31). dt1.md §Block pixels: "Moving past the last row is allowed if no pixel is written there." Now a count-0 pair writes nothing and continues; the bounds check applies only to pairs that write. Test: dt1_rle_skip_past_last_row_writes_nothing.
Killed: animdata.rs:143:22 (animdata_query_of_eight_characters_is_not_too_long, §4); cof.rs:91:31, 131:22; dc6.rs:58:18 x2, 115:79, 165:15; ds1.rs:88 x3, 158:26 x2, 160:38, 161:29 x2, 161:49, 161:70 -, 162:46, 163:40 x2, 193:14, 241:18 x2, 269:20, 269:31, 276:35; dt1.rs:33:9, 36:9 x4, 83:37, 222:22, 232:57, 232:49, 239:11; palette.rs (pl2_with_one_text_color, pl2_with_no_text_colors, pl2_text_color_shifts_follow_the_colors); tbl.rs:46-47 key_hash x6 (expected hashes from the spec pseudocode), 89:61 +, 90:41 x2, 120:27.
Equivalent:
- animdata.rs:88:50 - -> +: looser pre-allocation fit check; c.bytes(RECORD_SIZE) still fails on a short file with an error.
- ds1.rs:101:32 - -> + in check_count: looser bound; later reads still error; otherwise only a with_capacity hint.
- ds1.rs:161:70 + -> *, 161:74 + -> - and *: only a smaller layer_count in the grid-size guard; layer reads then fail on a short file.
- ds1.rs:163:54 - -> +: same looser grid guard.
- tbl.rs:89:61 * -> /: looser hash-table bound; slot reads still fail.
Timeouts (detected, hang): dc6.rs:142:11, 153:15; palette.rs:48:31 (PL2_FIXED ~449M); dt1.rs decode_rle += -> *= (221:11, 231:11 after the fix).
Verification (92 mutants: list + all decode_rle): 83 caught / 7 missed (the equivalents) / 2 timeout / 0 unviable.

### C. d2-formats mpq archive, set

mpq archive (91; mod.rs line numbers below are current file = original +4 for the new mod lines): 63 killed by 19 tests in src/mpq/mutant_tests_archive.rs; 28 equivalent; no fix.
Killed: 122 is_empty (lookup_stops_at_empty_and_skips_deleted, Covers mpq.md §5 r3); 178 le_u16 (header_field_rules, hash_table_entries_decrypted); find_header 274-296 (header_beyond_first_megabyte, header_at_first_aligned_magic, user_data_header_at_zero, user_data_header_redirects_past_later_magic, header_ending_at_end_of_file); 315:31 header_size; 363 hash_table; 392:29 (lookup_prefers_neutral_locale, Covers §5 r5); 402 contains; 446 read_block_stats x3; 452:24 x2 (block_ending_at_end_of_file); 499/503 (single_unit_stored_and_compressed); 538:48 (empty_sector_crc_block); 563:27 (uncompressed_longer_range_is_cut_to_file_size); 597-599 (recover_key_only_for_compressed_blocks, §13); set.rs 128-183 x21 (archive_set_open_dir_and_lookup, archive_set_without_known_archives).
Equivalent / unobservable:
- 125 is_deleted -> false: block_index 0xFFFFFFFE always fails the block_index < count test.
- 143:34, 691:25/32/41/49/62, 692:49/79 | -> ^: disjoint bits.
- 193:5, 195:16 x3, 196:57 x2, 197:14, 200:14 x2: #[cfg(windows)] read_exact_at, not compiled on Linux (needs a Windows run).
- 519:27 < -> <=: all sectors empty; decompression of an empty sector already fails; only the error variant changes.
- 553:36 x2, 575:28 *=, 657:24 *=, 666:40 *=: SectorStats survey counters; no spec defines them.
- 600:13 || -> &&, 600:38 == and <=: a block under 8 bytes cannot hold offset table + data, every key fails read_block; else only Err(I/O) vs Ok(None) near EOF, not decided by the spec.
- 621:39 && -> ||, 621:65 + -> *: probe is a pre-filter; read_block validation checks the same conditions.
- 296:24 > -> ==: header past EOF still fails (I/O UnexpectedEof instead of NotAnArchive); spec sets no variant.
Timeouts (detected, hang): find_header 271:30, 288:20, 290:18 (original 267:30, 284:20, 286:18).

### D. d2-formats mpq codecs

codecs (36): 29 killed by 8 tests in src/mpq/mutant_tests_codecs.rs; 7 equivalent:
- adpcm.rs:17:20 < -> <=: 2-byte input gives empty output either way (step-2 check pos+2 > len).
- crypto.rs:18:47 | -> ^ in build_crypt_table: hi<<16 and lo<=0xFFFF share no bits.
- explode.rs:61:22 > -> >=: ExLenBits 0 gives LenBase[l] = l and a 0-bit read gives 0.
- explode.rs:72:29 and 74:37 | -> ^: the shifted value has zero low bits where the read bits go.
- huffman.rs:175:21, 176:21 || -> && in Tree::increment: defensive guard (swap with root / own parent); §11 defines no such case, valid trees never reach it.
Killed: adpcm 26:* (5), 42-68 (13) -> adpcm_initial_samples_stop_only_when_input_or_output_runs_out (Covers mpq.md §12 r2), adpcm_matches_spec_model (Covers §12 r3; model from §12 text), adpcm_single_commands, adpcm_stereo_channel_rotation; crypto next_key 71:* (7) -> decrypt_key_schedule_vectors (Covers §4; expected values from §2/§4 pseudocode, the old round-trip test used next_key both ways); explode 45:9 -> explode_ascii_literals (§10 r3), 55:21 -> explode_literals_are_capped (§10 r4), 74:23 -> explode_long_distance_copies (§10 r2).
Timeouts (detected, hang): bits.rs 30:22, 43:9, 48:20 x2, 53:9 x2 (decoder stops advancing; bits::tests::lsb_first_order fails fast on each); huffman.rs 64:23, 102:9 x2 (root is a leaf; decode repeats one symbol to max_out).
Spec issue (not fixed, specs/ off-limits): mpq.md §10 step 2c says l == 0x205 "(519)"; 0x205 = 517. Code uses 0x205, consistent with LenBase[15] 0x106 + 255.
Verification: 29 caught / 7 missed (the equivalent set) / 0 unviable / 0 timeout.

### E. d2-data bin, calc, codegen

bin/calc/codegen (45): 42 killed by 11 tests in d2-data/src/mutant_tests_bin_calc.rs; 3 equivalent; no fix; no Covers claims.
Killed: bin.rs 163 buffer_tables x3 (code_buffers_compiled_from, loading.md §4.3); 173-176 buffers_after (code_buffers_load_right_after_their_table); 315/319/326 formula_fields (formula_fields_are_the_calc_columns, calc-expressions.md §1.2); 340, 479 (gamble_codes_must_be_items, §8/§7.4); 437-446 (count_limits, §8/§10 r8); 492/494 (treasure_class_limit, §8); 507-512 hireling (hireling_checks, §8); calc.rs 399 (emit_leaves_one_value, §4.5); 611/642/643 (const_evaluator_comparisons_and_call, §3.3); 745 x2 (validator_counts_open_parens, §1.5 step 4); codegen.rs 81/82/86 (codegen_code2_and_key_str, field-types.md §3).
Equivalent:
- bin.rs:326:64 && -> ||: in the embedded schema every type-25 field of the compiled-from tables links to its buffer and no other type links one; both forms pick the same fields.
- calc.rs:296:52 + -> * (quoted-name cut): start >= 1 keeps >= 254 bytes; resolution reads at most 31 bytes or compares whole names against base/mod.
- calc.rs:616 delete 0x04..=0x06 arm in eval_const: folding runs only when no PARAM can be in the output (§4.6 End step 2); the spec does not define PARAM in the callback-less evaluator.
Timeouts (detected, hang): calc.rs 268, 276, 285, 305, 357 (Parser::next), 590:11, 590:16 (code_calls), 608, 613, 635 (eval_const), 748:11, 748:16 (validate_buffer).
Verification: bin 64 caught/1 missed(eq)/1 unviable of 66 in-scope; calc 150 caught/2 missed(eq)/1 unviable/10 timeout of 163; codegen 15/15.

### F. d2-data compile, callbacks

compile + compile/callbacks (67): 65 killed by 14 tests in d2-data/src/mutant_tests_compile.rs; 2 unobservable; no fix.
Killed: compile.rs 99/142 linker is_empty (linker_counters_start_empty, field-types §6); 383-395 FormulaLinks x15 + 415 Missiles arm (calc_formula_links, calc-expressions §4.4); 430:77 (calc_diagnostics_counted_per_cell); 517 (calc_and_param_footprints_checked, field-types §9/§8); 545:68 (type5_run_broken_at_list_end, txt-format §6.1); 605/606 (text_cut_limits, txt-format §9); 618/620 (int_range_bounds, §9); 731/736 (missing_field_callback_columns, field-types §8 r2); callbacks.rs 46:70 (special_linker_reads_records, callbacks §7); 73, 104, 132-140, 149-155, 162-169, 183 (cube_input_words, cube_output_words, callbacks §1-§3); 384 (cube_output_words); 446 x2 (skillmode_sequence_lookup, Covers callbacks.md §4 r4); 462:19 (composit_reset, Covers callbacks.md §5 r1); 476:44 (composit_four_byte_token, §5).
Unobservable: compile.rs:460:72 += -> -= and *= (StdCallbacks::unspecified counter for cb() names outside callbacks.md; a d2rs reporting convenience no spec defines).
Timeouts (detected, hang): compile.rs 35:5 code4 -> [0;4] / [1;4] (all codes collide; linker add loops ~2^32); 551:11; callbacks.rs 105:11.
Verification: 65 caught / 2 missed (the unobservable pair) / 0 unviable / 0 timeout.

### G. d2-data compile_set, crosscheck, links, schema, strings, txt, tables

misc: compile_set, crosscheck, links, schema, strings, txt, tables/mod (93): 80 killed by 18 tests in d2-data/src/mutant_tests_misc.rs; 13 equivalent/unobservable; no fix; no Covers claims.
Killed: compile_set 45 x2, 103, 104, 115, 121, 126 (compiled_set_table_by_name, special_linkers_feed_cube_inputs, treasure_class_linker_from_compiled_tables); crosscheck x55 (crosscheck_roles_counts_and_verdicts, crosscheck_monstats_707_exception, crosscheck_monstats_707_needs_both_values, crosscheck_attributes_differences; loading.md §11, field-types.md §10); links 70, 89, 93 x5, 106, 164, 227-233 x5, 252 (name_lookup_list_takes_runtime_key_size, linker_sizes_insert_and_iter, link_values_read_at_full_width, load_lookups_reads_only_by_products, validate_set_uses_lookups_for_sizes); schema 113, 131-133, 149 (lookup_types_are_exactly_the_link_ids, vocabulary_of_key_types); strings 71:22 x2 (expansion_string_id_adds_20000); tables/mod 38:27 (decode_all_refuses_other_tables, loading.md policy 2).
Equivalent / unobservable:
- compile_set.rs:73:21 == -> != on "skills": the @range linker is the same either way; only skills `range` reads it.
- links.rs:115:35 | -> ^: (v << 8) has a zero low byte.
- crosscheck.rs:300:37 < -> ==, >, <= and 285:61 + -> *: only the display examples in FieldDiff.examples change; no spec defines them.
- txt.rs:38:9 x2, 106:9, 117:12: error message text only.
Timeouts (detected, hang): txt.rs:173:27 x2, 181:15 (byte scan in TxtTable::parse never advances).

### H. d2-data fixup

fixup + fixup/{maps,qsort,records} (92): 73 killed by 27 tests in d2-data/src/mutant_tests_fixup.rs; 19 equivalent; no fix.
Killed: fixup.rs apply 157, 163, 298, all table arms, 213 x2, 272 x4 (apply_* tests; Covers fixups.md §10 on apply_clamps_missiles_and_monumod); 315:18 (apply_runs_skills_and_pet_append); maps.rs 77 x2, 93, 335, 378, 391 (equiv_bounds_and_row_zero, monseq_both_counts, hireling_id_256_is_ignored, portals_in_order); qsort.rs 12:16 x2 (qsort_matches_the_spec_steps, Covers runtime-maps.md §1; oracle written from the §1 steps on ~2,500 tie-heavy inputs); records.rs 43-90 (stat_ops_slots_entries_and_flags), 112/115 (set_attachment_to_a_later_set), 235 (speed_missing_monmode_row_is_an_error, fixups.md open question 6 d2rs policy), 239 x3 (speed_weapon_class_range), 269, 284, 312, 360-368 (tile_path_fields_and_gate, Covers fixups.md §12 text), 385 (objects_all_frame_counts, Covers §13 r2).
Equivalent:
- fixup.rs:187:27 initial hireling_first 1 vs -1: shows only with no hireling table loaded; the spec defines the map only as built by the hireling loader.
- maps.rs:118:22 < -> <=: push runs only when v != 0.
- maps.rs:289:39 < -> <=: through apply, j is always below the item count (same weapons/armor/misc records, loading.md §9).
- qsort.rs:24:21, 26:29: self-swap / self-compare (Equal).
- qsort.rs:65:28, 89:28 > -> >=: loop stops on the Equal compare at mid anyway; same push/continue choice.
- qsort.rs:69:22 h < l -> <=: h == l cannot occur there.
- qsort.rs:78:20 mid < h -> <=: h > mid always after the loop.
- qsort.rs:94:18, 94:23, 94:29 (x4): only the order the two disjoint halves are sorted; unbounded stack.
- qsort.rs:95:23, 98:22, 103:22, 106:23 < -> <=: adds size-1 ranges, which the short sort leaves unchanged.
- records.rs:90:52 | -> ^: bit is 6..8, never 5.
- records.rs:331:16 > -> >=: cutting 256 rows to 256.
Timeouts (detected, hang): maps.rs 113:15 x2, 356:17, 470:19, 470:64; qsort.rs 32, 49, 57, 64, 65:24, 78:20 x2, 80, 88.
Verification: 73 caught / 19 missed (the equivalent list) / 0 unviable / 0 timeout.

### I. d2-data patch

patch + patch/{apply,check,diff,syntax} (74): 67 killed by 27 tests in d2-data/src/mutant_tests_patch.rs (one of them, patch.rs sort_key "delete match arm 2", removed by the fix below); 7 equivalent; 1 code fix; no Covers claims.
Code fix (c): patch.rs Finding::sort_key ordered B findings by code, then table; patch-layers.md §8 orders B by table. The `2 =>` arm is removed so B uses the (table, row, column) key like C. report_orders_b_by_table failed before the fix.
Killed: patch.rs 162-164 class arms, 231, 274:33, 301 sort_report, 422 rules, 599 base_file x2; apply.rs 23 x2, 70:88, 413:96, 473:90, 129:40, 267:26, 492:50; check.rs all 22; diff.rs 20:20, 28:16 x2, 56:11 x3, 74:34, 75 x5, 76:18, 141 x2, 149 x4, 159:56; syntax.rs 119:32, 343:19, 374:19 x2, 452:19 x2, 524:65.
Assertions on C-finding line/column (check.rs) and D line = row + 2 (diff.rs 20:20) rest on patch-layers.md open question 5(g)/(h): the spec's record of how the implementation reads details the rules leave open ("confirm or restate").
Equivalent:
- patch.rs:106 describe -> "" / "xyzzy": wording is not normative (§8).
- patch.rs:166 delete arm C: falls into the default class 5 with the same key; the other class-5 codes (D) never share a report with C.
- patch.rs:274:55 || -> &&: differs only for a finding with row/column but no table; none is produced.
- apply.rs:390:94 (P09 stack position): P09 stops the stack; all P09s come from one layer.
- syntax.rs:113:30: the extra trailing empty line is blank, no finding; empty input gives P02 1:1 either way.
- syntax.rs:153:13 delete arm ']': the bare-token path gives the same P10 at the same column.
Timeout (detected, hang): syntax.rs:150:19.
Verification: 67 caught / 7 missed (equivalents) / 1 timeout / 0 unviable. tables/mod.rs:38:27 re-run: caught.

## Gates

All pass on the final commit:
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`: the whole workspace was checked on the first commit; after the later test-only edits, re-run on `-p d2-formats -p d2-data`
- `cargo test -p d2-formats -p d2-data`: d2-data lib 275 passed, d2-formats lib 214 passed; game-file tests ignored
- `cargo run -p depcheck`: OK
- `spec_index.py --check`: OK
- `methods.py check`: 21 OK
- `coverage.py --check`: 3,275 claims, 0 errors
- `coverage.py --selftest`: ok

Toolchain note: in a fresh cloud container, running `cargo install` at the same time as `tools/cloud-setup.sh` broke the pinned toolchain's rustfmt and clippy. `rustup component remove rustfmt clippy && rustup component add rustfmt clippy` repaired them.

## Next steps

1. Run the full `cargo mutants -p d2-data` after-run. Expected missed: the 44 equivalents in groups E–I.
2. Fold this note into `docs/HANDOFF.md` (§1 test state, §7 the spec issues above, §8 the lesson on random proptest catches).
3. Fix the mpq.md "(519)" typo in a spec session.
