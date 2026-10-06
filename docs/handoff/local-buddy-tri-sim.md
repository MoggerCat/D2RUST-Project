# Local buddy tri-sim 2026-10-07 (d2-sim triage re-run)

Branch `claude/local-buddy-tri-sim-2026-10-07` from main `06a9726`. 1.14d install, `D2_GAME_DIR` = main checkout `game\`. Commands exactly as in the triage "Local re-run". No expected value or test changed. Step 1 stops at the first failing test binary, so a second run with `--no-fail-fast` was added (same tests). Logs stay on the PC.

## Results vs triage "Expected"

| Test | Triage expects | Actual | vs 2026-10-07 a-sim |
|---|---|---|---|
| lib `real_grid_belt_and_type_tables` (b) | pass | PASS (8/8 lib ignored pass) | was FAIL, fixed |
| game_core (12) | pass | PASS 12/12 | same |
| game_world (b) `waypoint_objects` | 10 pass | PASS, 10/10 (cubemain, waypoint, vendor all pass) | was 7/10 |
| game_drlg_tables `act1_placement_on_live_tables` (a) | pass | PASS | was FAIL |
| game_drlg_tables `lvlprest_measurements` (b) | pass | FAIL: `game_drlg_tables.rs:234` left 80, right 82 | was 1,079 vs 82 |
| game_treasure (16) incl. `sweep_drop_quality_every_item` (b) | pass | PASS 16/16 | new |
| game_items `sweep_create_every_item_every_quality` (b+c) | pass (Edge case 3 accepted) | FAIL: 560 failures, all shown ones `affix 1 does not fit` (item 519 ibk, every quality 0-4, ilvl 1/30/60/99, exp true d 0); first 20 printed only | was 570 NullAffixGroup |
| game_items other 7, game_inventory_path 6, game_skills 15/16 | pass / known red | pass; `game_skills::every_skill_function_in_table` FAIL (known base red) | n/a |
| game_monsters `type_init_every_class` (b) | pass | PASS | was FAIL |
| game_monsters `ai_index_of_every_row` (c) | red, prints pairs | FAIL, 34 row pairs differ | same fail, now printed |
| game_monsters `real_levels_rows` (c) | red, prints list | FAIL (as expected) | same fail, now printed |
| game_monsters others | pass | 17/19 pass | was 16/19 |

Net vs yesterday: fixed/now passing: inventory lib test, waypoint_objects, vendor_columns, cubemain_vector_records (game_world 10/10), act1_placement, type_init_every_class. Still red: lvlprest_measurements (new number), sweep_create (new error class), two c rows (expected).

## Printed lines

- Crafted Edge case 3 line: `10 total {(false, 0, 1): 4, (false, 2, 1): 1, (true, 0, 1): 3, (true, 2, 1): 2}` (expansion, difficulty, ilvl). All ilvl 1; none at d 1. (Yesterday's 5 recorded lines were of this kind; the 570 failures were NullAffixGroup. Now 10 of those are accepted and 560 other failures remain.)
- `act 0 levels with WarpDist != 2025: [(15, 3800), (20, 100), (21, 100), (23, 100), (25, 100)]`
- `row N name: live AI x, tsv AI y` (34 pairs). Format `row N name: live, tsv`:
  528 drehyaiced 129/31; 708 uberandariel 44/34; 709 uberduriel 145/44; 701 dkfig2 74/72; 703 dkmag2 21/74; 413 lightningsentry 102/101; 414 bladecreeper 103/102; 415 invisopet 101/103; 417 deathsentry 105/104; 418 shadowwarrior 106/105; 419 shadowmaster 107/106; 420 druidhawk 108/107; 422 fenris 109/108; 425 oaksage 110/109; 426 plaguepoppy 111/110; 428 vinecreature 112/111; 429 druidbear 1/112; 436 barricadetower 114/113; 524 act5barb2 1/127; 527 nihlathakboss 31/128; 529 evilhut 130/129; 535 act5pow 32/131; 540 ancientstatue3 133/132; 543 ancientbarb3 134/133; 544 baalthrone 135/134; 545 baalcrab 136/135; 546 baaltaunt 137/136; 560 baalcrabstairs 61/138; 571 baalclone 141/140; 574 baalminion3 1/141; 707 uberizual 34/144; 710 uberbaal 121/145; 705 ubermephisto 147/146; 706 uberdiablo 144/147.

## Questions the output settles

- Q7: the TSV shift starts at row 413 (lightningsentry, +1 through 436 in the TSV vs live; i.e. the TSV's row numbers or AI values are off by one from 413). Rows 413-436 pair "live = tsv+1" for the listed rows (so the TSV lags), and from 524 on the TSV leads (live evilhut 529 = AI 130; live row 528 drehyaiced = AI 129 is not in the TSV at 528). The `monstats_rows` numbering in `ai-functions.tsv` needs renumbering against `monstats.txt` order; the list above is the complete mismatch set (34). The evilhut question: live row 528 has AI 129 (GenericSpawner) and row 529 evilhut has AI 130, so the spec's 528 for evilhut cannot be verified from this output alone; the AI-129 holder is row 528 drehyaiced in this test's indexing. Spec session should decide with `monstats.txt`.
- Q8: Act 0 levels with WarpDist != 2025 are 15 (3800) and 20, 21, 23, 25 (100 each). So "Act 1 WarpDist 2025" has exceptions: 15, 20, 21, 23, 25. Confirming that §8 reads the room's own level's WarpDist is not answered by this output.
- Q1/Q2/Q3: Edge case 3 occurs in only 10 requests, all ilvl 1 (4 classic d0, 1 classic d2, 3 exp d0, 2 exp d2). The triage claim "reachable on almost every low-ilvl craft" is not borne out at the sweep's ilvl points (1/30/60/99). Q3 (magic gate at M <= -100): `sweep_drop_quality_every_item` passes.
- Q4: act1 placement passes on live tables, consistent with the allocation fix.
- New finding (needs triage): `sweep_create_every_item_every_quality` has 560 failures `item 519 ibk ... affix 1 does not fit` for qualities 0..4 on item 519 (and possibly other items; only 20 lines are printed). This is a different error class from the NullAffixGroup the triage expected. The test or the code needs a look; printing all failures grouped by (item, error) would settle it.
- lvlprest: with `len > 1` the count is 80, not 82; the spec/test expectation 82 and the live data disagree by 2 (c-type finding for `preset.md` §13).

## Claims unlocked

`// Covers:` lines (were "Claim once ... passes") for passing tests only: `game_treasure.rs` (10 lines, all 16 tests pass), `game_inventory_path.rs` (4 lines with spec refs; the 2 "none" lines left), `game_items.rs` (2: `live_affix_parts`, `live_unique_rarity_32_bits`; the `sweep_create...` claim left as it fails). `game_world` has no claim lines. `py tools/coverage.py --check`: `5253 claims, 0 errors`. `--summary`: 5253 claims, verified 312/4014 rules (7.8%).
